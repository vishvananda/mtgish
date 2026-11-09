// A melded permanent produces several cards on exile. Followup actions on the
// returned incarnation act on all of them, without widening identity/property
// reads or distributing an arbitrary scalar conditional over an entire group.
impl Context {
    fn returned_followup_group(&self, value: &source::Permanent) -> Option<ObjectSet> {
        if !matches!(
            value,
            source::Permanent::ThePermanentPutOnTheBattlefieldThisWay
        ) {
            return None;
        }
        let result = self.entered.as_ref()?;
        self.entered_from_one
            .as_ref()
            .filter(|one| one.0 == result.0)?;
        Some(ObjectSet::Transition {
            result: result.clone(),
            phase: ObjectPhase::After,
        })
    }

    fn returned_conditional(
        &mut self,
        condition: &Condition,
        then: &Program,
        otherwise: &Program,
    ) -> Option<Instruction> {
        let Condition::ObjectMatches { object, filter } = condition else {
            return None;
        };
        let objects = self
            .returned_followup_group(&source::Permanent::ThePermanentPutOnTheBattlefieldThisWay)?;
        let original = self
            .object(&source::Permanent::ThePermanentPutOnTheBattlefieldThisWay)
            .ok()?;
        if serde_json::to_value(object).unwrap() != serde_json::to_value(&original).unwrap() {
            return None;
        }
        let mut child = self.clone();
        child.entered_from_one = None;
        let yes = child.effect(Operation::NoteObjects {
            objects: ObjectSet::Matching {
                objects: Box::new(objects.clone()),
                filter: filter.clone(),
            },
        });
        let Instruction::Effect { result: yes_id, .. } = &yes else {
            unreachable!()
        };
        let yes_objects = ObjectSet::Selected(yes_id.clone());
        let no = (!otherwise.instructions.is_empty()).then(|| {
            child.effect(Operation::NoteObjects {
                objects: ObjectSet::Difference {
                    objects: Box::new(objects),
                    excluding: Box::new(yes_objects.clone()),
                },
            })
        });
        let mut yes_body = then.clone();
        if !retarget_return_actions(&mut yes_body, &original, &yes_objects) {
            return None;
        }
        let mut no_body = otherwise.clone();
        if let Some(Instruction::Effect { result, .. }) = &no {
            if !retarget_return_actions(
                &mut no_body,
                &original,
                &ObjectSet::Selected(result.clone()),
            ) {
                return None;
            }
        }
        // Snapshot BOTH partitions before either branch acts. Counter-dependent
        // characteristics/control changes must not change the other partition.
        let mut instructions = vec![yes];
        instructions.extend(no);
        instructions.extend(yes_body.instructions);
        instructions.extend(no_body.instructions);
        self.result = child.result;
        self.invalidate_child_bindings(&child);
        Some(Instruction::Group(Program { instructions }))
    }
}

fn retarget_return_actions(body: &mut Program, original: &ObjectRef, group: &ObjectSet) -> bool {
    let same = |objects: &ObjectSet| {
        matches!(objects, ObjectSet::One(object)
        if serde_json::to_value(object).unwrap() == serde_json::to_value(original).unwrap())
    };
    for instruction in &mut body.instructions {
        let Instruction::Effect { operation, .. } = instruction else {
            return false;
        };
        match operation {
            Operation::PutCounters { placements } => {
                for placement in placements {
                    let Recipients::Objects(objects) = &mut placement.recipients else {
                        return false;
                    };
                    if !same(objects)
                        || !matches!(placement.amount, Expr::Constant(_))
                        || placement.selection.is_some()
                    {
                        return false;
                    }
                    *objects = group.clone();
                }
            }
            Operation::SetTapped { objects, .. } if same(objects) => *objects = group.clone(),
            _ => return false,
        }
    }
    true
}

#[cfg(test)]
mod return_followup_tests {
    use super::*;
    fn moved(objects: ObjectSet, destination: Destination) -> Operation {
        Operation::Move {
            objects,
            destination,
            entry: EntrySpec::default(),
            face_down: false,
            terms: MovementTerms::default(),
        }
    }
    fn returned() -> Context {
        let mut c = Context::default();
        c.effect(moved(ObjectSet::One(ObjectRef::Source), Destination::Exile));
        let group = c
            .exiled_followup_group(&source::CardInExile::TheCardExiledThisWay)
            .unwrap();
        c.effect(moved(c.still_exiled(group), Destination::OwnersBattlefield));
        c
    }
    fn reference() -> source::Permanent {
        source::Permanent::ThePermanentPutOnTheBattlefieldThisWay
    }
    #[test]
    fn proof_names_the_actual_return_and_does_not_widen_identity_reads() {
        let mut c = returned();
        assert!(matches!(
            c.returned_followup_group(&reference()),
            Some(ObjectSet::Transition {
                result: ResultId(1),
                phase: ObjectPhase::After
            })
        ));
        assert!(matches!(
            c.object(&reference()).unwrap(),
            ObjectRef::Result {
                result: ResultId(1),
                index: 0,
                ..
            }
        ));
        assert!(c
            .returned_followup_group(&source::Permanent::ThisPermanent)
            .is_none());
        c.effect(moved(
            ObjectSet::One(ObjectRef::Source),
            Destination::OwnersBattlefield,
        ));
        assert!(c.returned_followup_group(&reference()).is_none());
    }
    #[test]
    fn plural_exiles_and_unguarded_returns_have_no_single_input_proof() {
        let mut c = Context::default();
        c.effect(moved(ObjectSet::Targets(TargetSlot(0)), Destination::Exile));
        c.effect(moved(
            c.still_exiled(ObjectSet::Transition {
                result: ResultId(0),
                phase: ObjectPhase::After,
            }),
            Destination::OwnersBattlefield,
        ));
        assert!(c.returned_followup_group(&reference()).is_none());
        let mut c = returned();
        c.effect(moved(
            ObjectSet::Transition {
                result: ResultId(0),
                phase: ObjectPhase::After,
            },
            Destination::OwnersBattlefield,
        ));
        assert!(c.returned_followup_group(&reference()).is_none());
        assert!(returned()
            .fresh()
            .returned_followup_group(&reference())
            .is_none());
    }
    #[test]
    fn prior_return_proof_does_not_leak_into_iteration_or_a_scalar_branch() {
        let mut c = returned();
        c.for_each_object(ObjectSet::One(ObjectRef::Source), |child| {
            assert!(child.returned_followup_group(&reference()).is_none());
            Ok(Program {
                instructions: vec![],
            })
        })
        .unwrap();
        c.conditional(
            Condition::Compare {
                left: Expr::Constant(1),
                comparison: Comparison::Equal,
                right: Expr::Constant(1),
            },
            |child| {
                assert!(child.returned_followup_group(&reference()).is_none());
                Ok(Program {
                    instructions: vec![],
                })
            },
            |_| {
                Ok(Program {
                    instructions: vec![],
                })
            },
        )
        .unwrap();
    }
    #[test]
    fn conditional_repair_refuses_unrelated_targets_and_dynamic_amounts() {
        let mut c = returned();
        let condition = Condition::ObjectMatches {
            object: c.object(&reference()).unwrap(),
            filter: ObjectFilter::Any,
        };
        let body = Program {
            instructions: vec![c.effect(Operation::SetTapped {
                objects: ObjectSet::One(ObjectRef::Source),
                tapped: true,
            })],
        };
        let empty = Program {
            instructions: vec![],
        };
        assert!(c.returned_conditional(&condition, &body, &empty).is_none());
        let body = Program {
            instructions: vec![c.effect(Operation::PutCounters {
                placements: vec![CounterPlacement {
                    recipients: Recipients::Objects(ObjectSet::One(
                        c.object(&reference()).unwrap(),
                    )),
                    kind: crate::counter::CounterKind::PowerToughness(1, 1),
                    amount: Expr::AnnouncedX,
                    selection: None,
                }],
            })],
        };
        assert!(c.returned_conditional(&condition, &body, &empty).is_none());
    }
}
