// `Named` forms are built from the inspected source tree, so every enum variant
// has one with no per-variant code. Operands lower with an empty Context: a
// reference that needs the enclosing ability's bindings fails and stays a Form.
fn named_value(value: &impl Serialize) -> Named {
    let inspected = crate::inspect::value(value).expect("source AST serialization");
    named_node(&inspected)
}

fn named_rule(rule: &source::Rule) -> Named {
    named_value(rule)
}

/// The source rule for an ability index of a Card face.
pub(crate) fn rule_at(card: &source::OracleCard, index: usize) -> Named {
    let source::OracleCard::Card { rules, .. } = card else {
        unreachable!("faces are Card values")
    };
    named_rule(&rules.as_deref().unwrap_or_default()[index])
}

/// A card field with no reduced form.
fn named_field(name: &str, value: &impl Serialize) -> Named {
    let inspected = crate::inspect::value(value).expect("source AST serialization");
    Named {
        source_type: "Characteristics".into(),
        form: name.into(),
        operands: vec![operand(&inspected)],
    }
}

fn named_node(value: &serde_json::Value) -> Named {
    let args = &value["args"];
    Named {
        source_type: value["enum"].as_str().unwrap_or("value").into(),
        form: value["variant"].as_str().unwrap_or("value").into(),
        operands: match args {
            serde_json::Value::Null => vec![],
            serde_json::Value::Array(items) => items.iter().map(operand).collect(),
            other => vec![operand(other)],
        },
    }
}

include!(concat!(env!("OUT_DIR"), "/vocabularies.rs"));

fn vocabulary(ty: &str, value: &serde_json::Value) -> Option<Operand> {
    let plain = VOCABULARIES.contains(&ty)
        || crate::primitive_spec::registry()
            .vocabularies
            .iter()
            .any(|v| v == ty);
    (plain && value["args"].is_null()).then(|| Operand::Vocabulary {
        family: ty.into(),
        form: value["variant"].as_str().unwrap_or_default().into(),
    })
}

/// Convert any inspected value. Total: everything lacking a typed lowering is a Form.
pub fn operand(value: &serde_json::Value) -> Operand {
    use serde_json::Value as V;
    match value {
        V::Null => Operand::Absent,
        V::Bool(b) => Operand::Bool(*b),
        V::Number(n) => n
            .as_i64()
            .map_or_else(|| Operand::Text(n.to_string()), Operand::Integer),
        V::String(s) => Operand::Text(s.clone()),
        V::Array(items) => Operand::List(items.iter().map(operand).collect()),
        V::Object(fields) => match fields.get("enum").and_then(V::as_str) {
            Some(ty) => vocabulary(ty, value)
                .or_else(|| standalone_operand(ty, value))
                .unwrap_or_else(|| Operand::Form(Box::new(named_node(value)))),
            None => Operand::Record(
                fields
                    .iter()
                    .map(|(name, value)| Field {
                        name: name.clone(),
                        value: operand(value),
                    })
                    .collect(),
            ),
        },
    }
}

fn standalone_operand(ty: &str, value: &serde_json::Value) -> Option<Operand> {
    let result = Context::default().typed_operand(ty, value)?;
    crate::validate::operand(&result).ok()?;
    Some(result)
}

impl Context {
    fn typed_operand(&self, ty: &str, value: &serde_json::Value) -> Option<Operand> {
        use crate::pipeline::source_tree::decode;
        Some(match ty {
            "Permanent" => Operand::Object(self.object(&decode(value).ok()?).ok()?),
            "Permanents" => Operand::Objects(self.objects(&decode(value).ok()?).ok()?),
            "Player" => Operand::Player(self.player(&decode(value).ok()?).ok()?),
            "Players" => Operand::Players(self.players(&decode(value).ok()?).ok()?),
            "GameNumber" => Operand::Number(self.number(&decode(value).ok()?).ok()?),
            "Condition" => Operand::Condition(self.condition(&decode(value).ok()?).ok()?),
            "Expiration" => Operand::Duration(self.duration(&decode(value).ok()?).ok()?),
            "Cost" => Operand::Cost(self.cost(&decode(value).ok()?).ok()?),
            "Spells" => Operand::Filter(self.spell_filter(&decode(value).ok()?).ok()?),
            "Cards" => Operand::Filter(self.card_filter(&decode(value).ok()?).ok()?),
            "Comparison" => {
                let (comparison, expression) = self.comparison(&decode(value).ok()?).ok()?;
                Operand::Comparison(comparison, expression)
            }
            "Rule" => {
                let rule: source::Rule = decode(value).ok()?;
                Operand::Ability(Box::new(self.fresh().ability(&rule).ok()?))
            }
            "Spell" => Operand::Object(self.spell(&decode(value).ok()?).ok()?),
            "CardInGraveyards" => Operand::Object(self.grave_card(&decode(value).ok()?).ok()?),
            "CardInExile" => Operand::Object(self.exiled_card(&decode(value).ok()?).ok()?),
            "Actions" => {
                let actions: source::Actions = decode(value).ok()?;
                let mut child = self.nested_program();
                let body = child.actions(&actions).ok()?;
                // A body announcing its own targets is an ability, not a nested step.
                if child.targets.len() == self.targets.len() && child.modes.is_none() {
                    Operand::Program(body)
                } else {
                    Operand::Ability(Box::new(Ability::Spell {
                        announcement: Announcement {
                            targets: child.targets,
                            modes: child.modes,
                        },
                        body,
                    }))
                }
            }
            "Action" => {
                let action: source::Action = decode(value).ok()?;
                let mut child = self.nested_program();
                let actor = PlayerSet::One(PlayerRef::AbilityController);
                let instruction = child.action(&action, &actor).ok()?;
                Operand::Program(Program {
                    instructions: vec![instruction],
                })
            }
            _ => return None,
        })
    }

    /// A nested body closes over this scope; its results stay local to it.
    fn nested_program(&self) -> Self {
        let mut child = self.clone();
        child.result += 1;
        child
    }

    /// Lower an operand in this scope, or fail. A primitive may not hold a `Form`.
    fn bound_operand(&self, value: &serde_json::Value) -> Option<Operand> {
        use serde_json::Value as V;
        Some(match value {
            V::Null => Operand::Absent,
            V::Bool(b) => Operand::Bool(*b),
            V::Number(n) => Operand::Integer(n.as_i64()?),
            V::String(s) => Operand::Text(s.clone()),
            V::Array(items) => Operand::List(
                items
                    .iter()
                    .map(|v| self.bound_operand(v))
                    .collect::<Option<_>>()?,
            ),
            V::Object(fields) => match fields.get("enum").and_then(V::as_str) {
                Some(ty) => vocabulary(ty, value)
                    .or_else(|| self.typed_operand(ty, value))
                    .or_else(|| self.term(ty, value))?,
                None => Operand::Record(
                    fields
                        .iter()
                        .map(|(name, value)| {
                            Some(Field {
                                name: name.clone(),
                                value: self.bound_operand(value)?,
                            })
                        })
                        .collect::<Option<_>>()?,
                ),
            },
        })
    }

    /// A descriptive enum the registry lists, with every operand bound.
    fn term(&self, ty: &str, value: &serde_json::Value) -> Option<Operand> {
        if !crate::primitive_spec::registry()
            .terms
            .iter()
            .any(|t| t == ty)
        {
            return None;
        }
        let operands = match &value["args"] {
            serde_json::Value::Null => vec![],
            serde_json::Value::Array(items) => items
                .iter()
                .map(|v| self.bound_operand(v))
                .collect::<Option<_>>()?,
            other => vec![self.bound_operand(other)?],
        };
        Some(Operand::Term {
            family: ty.into(),
            form: value["variant"].as_str()?.into(),
            operands,
        })
    }

    /// The row name and bound operands of `value`, if it has a reviewed primitive.
    fn primitive(&self, family: &str, value: &impl Serialize) -> Option<(String, Vec<Operand>)> {
        self.primitive_inspected(family, &crate::inspect::value(value).ok()?)
    }

    fn primitive_inspected(
        &self,
        family: &str,
        inspected: &serde_json::Value,
    ) -> Option<(String, Vec<Operand>)> {
        let name = format!("{family}::{}", inspected["variant"].as_str()?);
        crate::primitive_spec::lookup(&name)?;
        let operands = match &inspected["args"] {
            serde_json::Value::Null => vec![],
            serde_json::Value::Array(items) => items
                .iter()
                .map(|v| self.bound_operand(v))
                .collect::<Option<_>>()?,
            other => vec![self.bound_operand(other)?],
        };
        Some((name, operands))
    }

    /// Its result is unknown, so earlier antecedents are forgotten as for a named action.
    fn primitive_action(
        &mut self,
        value: &source::Action,
        actor: &PlayerSet,
    ) -> Option<Instruction> {
        let (name, operands) = self.primitive("Action", value)?;
        self.forget_results();
        let result = ResultId(self.result);
        self.result += 1;
        if name.starts_with("Action::RollAD") {
            self.rolled = Some(result.clone());
        }
        Some(Instruction::Effect {
            result,
            operation: Operation::Primitive {
                actor: actor.clone(),
                name,
                operands,
            },
        })
    }

    fn primitive_ability(&self, value: &source::Rule) -> Option<Ability> {
        let (name, operands) = match value {
            source::Rule::DeckConstruction(deck) => self.primitive("DeckConstruction", deck)?,
            _ => self.primitive("Rule", value)?,
        };
        Some(Ability::Primitive { name, operands })
    }

    fn primitive_cost(&self, value: &source::Cost) -> Option<Cost> {
        let (name, operands) = self.primitive("Cost", value)?;
        Some(Cost {
            parts: vec![CostPart::Primitive { name, operands }],
        })
    }

    fn primitive_condition(&self, value: &source::Condition) -> Option<Condition> {
        let (name, operands) = self.primitive("Condition", value)?;
        Some(Condition::Primitive { name, operands })
    }

    fn primitive_filter(&self, value: &source::Permanents) -> Option<ObjectFilter> {
        self.primitive_predicate("Permanents", value)
    }

    fn primitive_predicate(&self, family: &str, value: &impl Serialize) -> Option<ObjectFilter> {
        let (name, operands) = self.primitive(family, value)?;
        Some(ObjectFilter::Primitive { name, operands })
    }

    fn primitive_number(&self, value: &source::GameNumber) -> Option<Expr> {
        let (name, operands) = self.primitive("GameNumber", value)?;
        Some(Expr::Primitive { name, operands })
    }

    fn primitive_change(&self, value: &source::LayerEffect) -> Option<Change> {
        let (name, operands) = self.primitive("LayerEffect", value)?;
        Some(Change::Primitive { name, operands })
    }

    /// The tested player comes first, then the form's own operands.
    fn primitive_player_check(
        &self,
        player: &PlayerRef,
        value: &source::Players,
    ) -> Option<Condition> {
        let (name, mut operands) = self.primitive("Players", value)?;
        operands.insert(0, Operand::Player(player.clone()));
        Some(Condition::Primitive { name, operands })
    }

    fn primitive_trigger(&self, value: &source::Trigger) -> Option<EventSpec> {
        let (name, operands) = self.primitive("Trigger", value)?;
        Some(EventSpec::Primitive { name, operands })
    }

    fn named_action(&mut self, value: &source::Action, actor: &PlayerSet) -> Instruction {
        self.forget_results();
        let result = ResultId(self.result);
        self.result += 1;
        Instruction::Effect {
            result,
            operation: Operation::Named {
                actor: actor.clone(),
                form: named_value(value),
            },
        }
    }

    /// The named action may have produced what a later "that" means, so anything
    /// bound before it could be the wrong antecedent. Later references must fail.
    fn forget_results(&mut self) {
        self.created_tokens = None;
        self.payment = None;
        self.exiled = None;
        self.exiled_permanents = false;
        self.entered = None;
        self.inspection_hand = None;
        self.paid.values_mut().for_each(|binding| *binding = None);
        self.acted.clear();
    }
}

#[cfg(test)]
mod named_tests {
    use super::*;

    #[test]
    fn a_named_action_keeps_its_actor_and_clears_antecedents() {
        let mut context = Context {
            created_tokens: Some(ResultId(0)),
            exiled: Some(ResultId(0)),
            entered: Some(ResultId(0)),
            payment: Some(ResultId(0)),
            result: 3,
            ..Context::default()
        };
        let actor = PlayerSet::OpponentsOf(PlayerRef::AbilityController);
        let instruction = context.named_action(&source::Action::Recruit, &actor);
        let Instruction::Effect { result, operation } = instruction else {
            panic!("named actions are effects")
        };
        let Operation::Named { actor: kept, form } = operation else {
            panic!("expected a named operation")
        };
        assert_eq!(result.0, 3);
        assert_eq!(context.result, 4);
        assert_eq!(form.form, "Recruit");
        assert_eq!(
            serde_json::to_value(kept).unwrap(),
            serde_json::to_value(actor).unwrap()
        );
        assert!(context.created_tokens.is_none());
        assert!(context.exiled.is_none());
        assert!(context.entered.is_none());
        assert!(context.payment.is_none());
    }
}
