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
            Some(ty) => typed_operand(ty, value)
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

fn typed_operand(ty: &str, value: &serde_json::Value) -> Option<Operand> {
    use crate::pipeline::source_tree::decode;
    let context = Context::default();
    let result = match ty {
        "Permanent" => Operand::Object(context.object(&decode(value).ok()?).ok()?),
        "Permanents" => Operand::Objects(context.objects(&decode(value).ok()?).ok()?),
        "Player" => Operand::Player(context.player(&decode(value).ok()?).ok()?),
        "Players" => Operand::Players(context.players(&decode(value).ok()?).ok()?),
        "GameNumber" => Operand::Number(context.number(&decode(value).ok()?).ok()?),
        "Condition" => Operand::Condition(context.condition(&decode(value).ok()?).ok()?),
        "Expiration" => Operand::Duration(context.duration(&decode(value).ok()?).ok()?),
        "Cost" => Operand::Cost(context.cost(&decode(value).ok()?).ok()?),
        "Rule" => {
            let rule: source::Rule = decode(value).ok()?;
            Operand::Ability(Box::new(Context::default().ability(&rule).ok()?))
        }
        _ => return None,
    };
    crate::validate::operand(&result).ok()?;
    Some(result)
}

impl Context {
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
