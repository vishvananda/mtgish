#![cfg(feature = "cantrip")]
use mtgish_ast::{cantrip, corpus, ir::*, lower, mtg_types::OracleCard};

fn card(name: &str) -> Card {
    include_str!("fixtures/poison_damage.ronl").lines().find_map(|line| {
        let source: OracleCard = ron::from_str(line).unwrap();
        corpus::Identity::of(&source).unwrap().names.iter().any(|n| n == name)
            .then(|| lower::card(&source).unwrap())
    }).unwrap()
}

#[test]
fn both_printed_damage_poison_users_share_the_native_recipe() {
    cantrip::planned::with_features(&["damage-poison"], || {
        for name in ["Pit Scorpion", "Serpent Generator"] {
            let generated = cantrip::module(&card(name)).unwrap();
            assert!(generated.contains("cantrip_mechanics::poison::on_damage_to_player::<1>"), "{name}: {generated}");
            assert!(!generated.contains("g.give_poison("), "{name}: {generated}");
        }
    });
}

#[test]
fn the_recipe_keeps_different_damage_domains_recipients_counts_and_additional_actions() {
    cantrip::planned::with_features(&["damage-poison"], || {
        for variant in 0..10 {
            let mut c = card("Pit Scorpion");
            let Ability::Triggered { event, body, limits, .. } = &mut c.abilities[0] else { panic!("trigger") };
            let EventSpec::Damage(damage) = event else { panic!("damage") };
            match variant {
                0 => damage.kind = DamageKind::Combat,
                1 => damage.recipients = DamageRecipients::Players(PlayerSet::One(PlayerRef::AbilityController)),
                2 => damage.sources = ObjectFilter::Any,
                3 => damage.source_kind = ObjectKind::Spell,
                4 => damage.grouping = DamageGrouping::Batch,
                5 => damage.subject = DamageSubject::Recipient,
                6 => body.instructions.push(Instruction::Effect { result: ResultId(1), operation: Operation::GainLife { players: PlayerSet::One(PlayerRef::AbilityController), amount: Expr::Constant(1) } }),
                7 | 8 => {
                    let Instruction::Effect { operation: Operation::PutCounters { placements }, .. } = &mut body.instructions[0] else { panic!("counter placement") };
                    if variant == 7 { placements[0].amount = Expr::EventAmount; }
                    else { placements[0].recipients = Recipients::Players(PlayerSet::One(PlayerRef::AbilityController)); }
                }
                9 => limits.per_turn = Some(1),
                _ => unreachable!(),
            }
            if let Ok(generated) = cantrip::module(&c) {
                assert!(!generated.contains("poison::on_damage_to_player"), "variant {variant}: {generated}");
            }
        }
    });
}

#[test]
fn the_shared_recipe_requires_the_actual_pinned_native_feature() {
    cantrip::planned::with_features(&[], || {
        let generated = cantrip::module(&card("Pit Scorpion")).unwrap();
        assert!(!generated.contains("poison::on_damage_to_player"), "{generated}");
        assert!(generated.contains("g.give_poison("), "{generated}");
    });
}

#[test]
fn the_complete_recipe_preserves_a_different_fixed_poison_count() {
    cantrip::planned::with_features(&["damage-poison"], || {
        let mut c = card("Pit Scorpion");
        let Ability::Triggered { body, .. } = &mut c.abilities[0] else { panic!("trigger") };
        let Instruction::Effect { operation: Operation::PutCounters { placements }, .. } = &mut body.instructions[0] else { panic!("counters") };
        placements[0].amount = Expr::Constant(2);
        let generated = cantrip::module(&c).unwrap();
        assert!(generated.contains("poison::on_damage_to_player::<2>"), "{generated}");
    });
}
