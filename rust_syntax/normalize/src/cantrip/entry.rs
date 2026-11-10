//! Encode typed arrivals for the existing planned engine ABI. There is no
//! grammar-term decoding here: every input is a checked semantic field.
use super::*;

fn flag(form: &str, operands: Vec<Operand>) -> Operand {
    Operand::Term {
        family: "EnterFlag".into(),
        form: form.into(),
        operands,
    }
}
fn counter(kind: &crate::counter::CounterKind) -> Operand {
    use crate::counter::CounterKind;
    match kind {
        CounterKind::Named(kind) => Operand::Vocabulary {
            family: "CounterType".into(),
            form: format!("{kind:?}"),
        },
        CounterKind::PowerToughness(power, toughness) => Operand::Term {
            family: "CounterType".into(),
            form: "PTCounter".into(),
            operands: vec![
                Operand::Integer(i64::from(*power)),
                Operand::Integer(i64::from(*toughness)),
            ],
        },
    }
}

impl Compiler {
    pub(super) fn entry_operands(
        &self,
        entry: &dyn crate::movement::ArrivalView,
    ) -> Result<Vec<Operand>> {
        let entry_view = entry.entry_view();
        let entry: &EntrySpec = &entry_view;
        let mut flags = Vec::new();
        if let Some(controller) = &entry.controller {
            flags.push(match controller {
                EntryController::Owner => flag("EntersUnderOwnersControl", vec![]),
                EntryController::Player(player) => flag(
                    "EntersUnderPlayersControl",
                    vec![Operand::Player(player.clone())],
                ),
            });
        }
        if entry.tapped {
            flags.push(flag("EntersTapped", vec![]));
        }
        for (kind, amount) in &entry.counters {
            flags.push(flag(
                "EntersWithNumberCounters",
                vec![Operand::Number(amount.clone()), counter(kind)],
            ));
        }
        for state in &entry.states {
            flags.push(match state {
                EntryState::Transformed => flag("EntersTransformed", vec![]),
                EntryState::Flipped => flag("EntersFlipped", vec![]),
                EntryState::FaceDown => Operand::Vocabulary {
                    family: "EntryState".into(),
                    form: "FaceDown".into(),
                },
                EntryState::Attacking {
                    player: None,
                    or_planeswalker: false,
                } => flag("EntersAttacking", vec![]),
                EntryState::Attacking {
                    player: Some(player),
                    or_planeswalker,
                } => flag(
                    if *or_planeswalker {
                        "EntersAttackingPlayerOrPlaneswalkerControlledBy"
                    } else {
                        "EntersAttackingPlayer"
                    },
                    vec![Operand::Player(player.clone())],
                ),
                EntryState::Blocking(object) => flag(
                    "EntersBlockingAttacker",
                    vec![Operand::Object(object.clone())],
                ),
                EntryState::AttachedTo(object) => flag(
                    "EntersAttachedToPermanent",
                    vec![Operand::Object(object.clone())],
                ),
                EntryState::AttachedToChosen(filter) => flag(
                    "EntersAttachedToAPermanent",
                    vec![Operand::Objects(ObjectSet::AnyZone(filter.clone()))],
                ),
                EntryState::AttachedToPlayer(player) => flag(
                    "EntersAttachedToPlayer",
                    vec![Operand::Player(player.clone())],
                ),
                EntryState::Changes(changes) => Operand::List(
                    changes
                        .iter()
                        .map(|change| match change {
                            Change::GrantAbility(ability) => Ok(Operand::Term {
                                family: "ExiledCardEffect".into(),
                                form: "AddAbility".into(),
                                operands: vec![Operand::Ability(ability.clone())],
                            }),
                            _ => Err(gap("planned typed entry changes")),
                        })
                        .collect::<Result<Vec<_>>>()?,
                ),
                _ => return Err(gap("planned typed arrival state")),
            });
        }
        // The planned countering ABI takes its counter placement before its
        // granted abilities. Keep this shape while making the AST fully typed.
        if let ([(kind, amount)], [EntryState::Changes(changes)]) =
            (entry.counters.as_slice(), entry.states.as_slice())
        {
            if entry.controller.is_none()
                && !entry.tapped
                && changes.iter().all(|c| matches!(c, Change::GrantAbility(_)))
            {
                return Ok(vec![
                    Operand::Number(amount.clone()),
                    counter(kind),
                    flags.pop().unwrap(),
                ]);
            }
        }
        Ok(flags)
    }
}
