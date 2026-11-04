use mtgish_ast::{
    ir::*,
    visit::{Visitor, VisitorMut, Walk, WalkMut},
};
fn effect(operation: Operation) -> Instruction {
    Instruction::Effect {
        result: ResultId(0),
        operation,
    }
}
#[test]
fn mutable_traversal_preserves_declaration_order_and_runs_postorder_after_children() {
    struct Rewrite(Vec<&'static str>);
    impl VisitorMut for Rewrite {
        fn operation(&mut self, _: &mut Operation) -> bool {
            self.0.push("operation");
            true
        }
        fn expr(&mut self, n: &mut Expr) -> bool {
            if let Expr::Constant(n) = n {
                *n += 1;
                self.0.push("expr");
            }
            true
        }
        fn operation_end(&mut self, op: &mut Operation) {
            let Operation::Draw {
                players,
                amount: Expr::Constant(3),
            } = op
            else {
                panic!("{op:?}")
            };
            *op = Operation::GainLife {
                players: players.clone(),
                amount: Expr::Constant(3),
            };
            self.0.push("operation_end");
        }
    }
    let mut p = Program {
        instructions: vec![effect(Operation::Draw {
            players: PlayerSet::One(PlayerRef::AbilityController),
            amount: Expr::Constant(2),
        })],
    };
    let mut rewrite = Rewrite(vec![]);
    p.walk_mut(&mut rewrite);
    assert_eq!(rewrite.0, vec!["operation", "expr", "operation_end"]);
    assert!(matches!(
        &p.instructions[0],
        Instruction::Effect {
            operation: Operation::GainLife {
                amount: Expr::Constant(3),
                ..
            },
            ..
        }
    ));
}
#[test]
fn cost_and_ability_declarations_can_stop_mutation_without_stopping_lexical_programs() {
    struct Rewrite;
    impl VisitorMut for Rewrite {
        fn ability(&mut self, _: &mut Ability) -> bool {
            false
        }
        fn cost(&mut self, _: &mut Cost) -> bool {
            false
        }
        fn expr(&mut self, n: &mut Expr) -> bool {
            if let Expr::Constant(n) = n {
                *n += 10;
            }
            true
        }
    }
    let choice = effect(Operation::Draw {
        players: PlayerSet::One(PlayerRef::AbilityController),
        amount: Expr::Constant(1),
    });
    let mut operands = vec![
        Operand::Cost(Cost {
            parts: vec![CostPart::Life(Expr::Constant(2))],
        }),
        Operand::Ability(Box::new(Ability::Spell {
            announcement: Announcement::empty(),
            body: Program {
                instructions: vec![choice.clone()],
            },
        })),
        Operand::Program(Program {
            instructions: vec![choice],
        }),
    ];
    operands.iter_mut().for_each(|v| v.walk_mut(&mut Rewrite));
    assert!(
        matches!(&operands[0],Operand::Cost(Cost{parts}) if matches!(parts.as_slice(),[CostPart::Life(Expr::Constant(2))]))
    );
    let Operand::Ability(a) = &operands[1] else {
        panic!()
    };
    let Ability::Spell { body, .. } = a.as_ref() else {
        panic!()
    };
    assert!(matches!(
        &body.instructions[0],
        Instruction::Effect {
            operation: Operation::Draw {
                amount: Expr::Constant(1),
                ..
            },
            ..
        }
    ));
    let Operand::Program(body) = &operands[2] else {
        panic!()
    };
    assert!(matches!(
        &body.instructions[0],
        Instruction::Effect {
            operation: Operation::Draw {
                amount: Expr::Constant(11),
                ..
            },
            ..
        }
    ));
}
#[test]
fn noop_mutable_walk_matches_immutable_nodes_and_preserves_whole_native_records() {
    struct Count(usize);
    impl Visitor for Count {
        fn operation(&mut self, _: &Operation) -> bool {
            self.0 += 1;
            true
        }
        fn expr(&mut self, _: &Expr) -> bool {
            self.0 += 1;
            true
        }
        fn object_set(&mut self, _: &ObjectSet) -> bool {
            self.0 += 1;
            true
        }
    }
    struct CountMut(usize);
    impl VisitorMut for CountMut {
        fn operation(&mut self, _: &mut Operation) -> bool {
            self.0 += 1;
            true
        }
        fn expr(&mut self, _: &mut Expr) -> bool {
            self.0 += 1;
            true
        }
        fn object_set(&mut self, _: &mut ObjectSet) -> bool {
            self.0 += 1;
            true
        }
    }
    for line in include_str!("fixtures/ability_aliases.ronl").lines() {
        let source = ron::from_str(line).unwrap();
        let mut card = mtgish_ast::unified::lower(&source).card;
        let before = serde_json::to_value(&card).unwrap();
        let mut count = Count(0);
        card.walk(&mut count);
        let mut count_mut = CountMut(0);
        card.walk_mut(&mut count_mut);
        assert_eq!(count.0, count_mut.0);
        assert_eq!(before, serde_json::to_value(&card).unwrap());
    }
}
