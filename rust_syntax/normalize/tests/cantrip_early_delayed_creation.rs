#![cfg(feature = "cantrip")]
//! Delayed-trigger creation follows the instruction's condition, not capture cardinality.
use mtgish_ast::{cantrip, ir::*, mtg_types::OracleCard, unified};
use syn::visit::{self, Visit};

fn creation_conditions(code: &str) -> Vec<usize> {
    #[derive(Default)]
    struct Calls {
        depth: usize,
        conditions: Vec<usize>,
    }
    impl<'ast> Visit<'ast> for Calls {
        fn visit_expr_if(&mut self, node: &'ast syn::ExprIf) {
            self.depth += 1;
            visit::visit_expr_if(self, node);
            self.depth -= 1;
        }
        fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
            if node.method == "delayed_trigger_about" {
                self.conditions.push(self.depth);
            }
            visit::visit_expr_method_call(self, node);
        }
    }
    let mut calls = Calls::default();
    calls.visit_file(&syn::parse_file(code).unwrap());
    calls.conditions
}

#[test]
fn an_unconditional_delayed_instruction_is_created_even_with_an_empty_capture() {
    let source: OracleCard =
        ron::from_str(include_str!("fixtures/early_delayed_creation.ronl").trim()).unwrap();
    let mut card = unified::lower(&source).card;
    let generated = cantrip::whole(&source, &card).unwrap();
    assert!(
        generated.faces[0].gaps.is_empty(),
        "{:?}",
        generated.faces[0].gaps
    );
    assert_eq!(creation_conditions(&generated.faces[0].rust), vec![0]);
    // Adding a real outer instruction condition must still gate creation.
    let body = card.faces[0]
        .abilities
        .iter_mut()
        .find_map(|ability| match ability {
            Ability::Activated { body, .. } => Some(body),
            _ => None,
        })
        .unwrap();
    let delayed = body.instructions.pop().unwrap();
    assert!(matches!(
        &delayed,
        Instruction::Effect {
            operation: Operation::CreateDelayedTrigger { .. },
            ..
        }
    ));
    body.instructions.push(Instruction::If {
        condition: Condition::PlayerIsActive(PlayerRef::AbilityController),
        then: Program {
            instructions: vec![delayed],
        },
        otherwise: Program {
            instructions: vec![],
        },
    });
    let generated = cantrip::whole(&source, &card).unwrap();
    assert!(
        generated.faces[0].gaps.is_empty(),
        "{:?}",
        generated.faces[0].gaps
    );
    assert_eq!(creation_conditions(&generated.faces[0].rust), vec![1]);
}
