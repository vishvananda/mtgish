// A reveal reflexive retains its actual reveal receipt and the found card's value.
impl Context {
    fn reveal_reflexive(
        &mut self,
        mut instructions: Vec<Instruction>,
        condition: Condition,
        actions: &source::Actions,
    ) -> Result<Instruction> {
        fn find(steps: &[Instruction]) -> Option<(ResultId, ObjectFilter)> {
            for step in steps {
                match step {
                    Instruction::Effect {
                        result,
                        operation:
                            Operation::RevealUntil {
                                filter,
                                count: Expr::Constant(1),
                                ..
                            },
                    } => return Some((result.clone(), filter.clone())),
                    Instruction::Group(body) => {
                        if let Some(found) = find(&body.instructions) {
                            return Some(found);
                        }
                    }
                    _ => {}
                }
            }
            None
        }
        fn uses_value(v: &serde_json::Value) -> bool {
            if v["enum"] == "GameNumber"
                && matches!(
                    v["variant"].as_str(),
                    Some(
                        "ManaValueOfTheCardFoundThisWay"
                            | "ManaValueOfTheFoundCard"
                            | "ManaValueOfTheCardRevealedThisWay"
                    )
                )
            {
                return true;
            }
            match v {
                serde_json::Value::Object(m) => m.values().any(uses_value),
                serde_json::Value::Array(a) => a.iter().any(uses_value),
                _ => false,
            }
        }
        fn insert(steps: &mut Vec<Instruction>, result: u32, capture: &Instruction) -> bool {
            for i in 0..steps.len() {
                if matches!(&steps[i], Instruction::Effect { result: r, .. } if r.0==result) {
                    steps.insert(i + 1, capture.clone());
                    return true;
                }
                if let Instruction::Group(body) = &mut steps[i] {
                    if insert(&mut body.instructions, result, capture) {
                        return true;
                    }
                }
            }
            false
        }
        let Some((revealed, filter)) = find(&instructions) else {
            return self.reflexive_after(instructions, Some(condition), actions);
        };
        let previous = self.reveal_reflexive_mana.take();
        if uses_value(&crate::inspect::value(actions).map_err(|e| invalid(&e.to_string()))?) {
            let id = ResultId(self.result);
            let capture = self.effect(Operation::BindValue {
                value: Expr::ObjectProperty {
                    object: ObjectRef::Query {
                        objects: Box::new(ObjectSet::Matching {
                            objects: Box::new(ObjectSet::Observed(revealed.clone())),
                            filter,
                        }),
                        index: 0,
                    },
                    property: Property::ManaValue,
                    information: Information::CurrentOrLastKnown,
                },
            });
            if !insert(&mut instructions, revealed.0, &capture) {
                return Err(invalid("missing reveal for captured mana value"));
            }
            self.reveal_reflexive_mana = Some(Expr::BoundValue(id));
        }
        let trigger = self.reflexive_on(revealed, Some(condition), actions);
        self.reveal_reflexive_mana = previous;
        instructions.push(trigger?);
        Ok(Instruction::Group(Program { instructions }))
    }

    fn optional_fixed_exile_guard(body: &Program) -> Option<Condition> {
        fn scan(
            body: &Program,
            picks: &mut Vec<(u32, ObjectSet, Expr)>,
            moves: &mut Vec<(u32, u32)>,
            triggered: &mut Vec<u32>,
        ) {
            for step in &body.instructions {
                match step {
                    Instruction::Effect {
                        result,
                        operation:
                            Operation::ChooseObjects {
                                objects,
                                count: Count::AsManyAsCan(n @ Expr::Constant(2..)),
                                ..
                            },
                    } => picks.push((result.0, objects.clone(), n.clone())),
                    Instruction::Effect {
                        result,
                        operation:
                            Operation::Move {
                                objects: ObjectSet::Selected(pick),
                                destination: MovementDestination::Exile { .. },
                                ..
                            },
                    } => moves.push((result.0, pick.0)),
                    Instruction::Effect {
                        operation:
                            Operation::ReflexiveTrigger {
                                performed: Some(r), ..
                            },
                        ..
                    } => triggered.push(r.0),
                    Instruction::Group(inner) => scan(inner, picks, moves, triggered),
                    _ => {}
                }
            }
        }
        let (mut picks, mut moves, mut triggered) = (vec![], vec![], vec![]);
        scan(body, &mut picks, &mut moves, &mut triggered);
        let guards: Vec<_> = picks
            .into_iter()
            .filter(|(p, _, _)| {
                moves
                    .iter()
                    .any(|(m, selected)| selected == p && triggered.contains(m))
            })
            .map(|(_, objects, n)| Condition::Compare { left: Expr::CountObjects(Box::new(objects)), test: NumberTest::Compare { comparison: Comparison::GreaterOrEqual, value: n } })
            .collect();
        match guards.len() {
            0 => None,
            1 => guards.into_iter().next(),
            _ => Some(Condition::AllOf(guards)),
        }
    }
}
