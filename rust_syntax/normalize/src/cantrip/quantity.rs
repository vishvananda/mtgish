//! Native semantic quantities, separate from physically stored values and indexes.
//! The pinned metadata selects a coherent API contract, never an incidental `i64`.
use super::*;

pub(super) const FEATURE: &str = "wide-effect-quantities";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Domain {
    Damage,
    Prevention,
    DamageExcess,
    DamagePrevented,
    CounterPlacement,
    LifeEffect,
    /// Magnitude of an actual change between physically stored i32 totals.
    LifeChangeMagnitude,
    PoisonEffect,
    // These remain physically bounded on the wide native API.
    LifeTotal,
    ShieldCapacity,
    StoredCounter,
}
impl Domain {
    pub(super) fn kind(self) -> &'static str {
        if planned::switched_on(FEATURE) {
            match self {
                Self::LifeTotal => "i32",
                Self::LifeChangeMagnitude => "u32",
                Self::ShieldCapacity => "u16",
                Self::StoredCounter => "i16",
                _ => "i64",
            }
        } else {
            match self {
                Self::ShieldCapacity
                | Self::Damage
                | Self::Prevention
                | Self::DamageExcess
                | Self::DamagePrevented => "u16",
                Self::CounterPlacement | Self::StoredCounter => "i16",
                Self::LifeEffect | Self::LifeTotal => "i32",
                Self::LifeChangeMagnitude | Self::PoisonEffect => "u32",
            }
        }
    }
    pub(super) fn from_stored(self, code: &str) -> String {
        if self.kind() == "i64" {
            format!("i64::from({code})")
        } else {
            code.into()
        }
    }
    pub(super) fn sum_i64(self) -> &'static str {
        if self.kind() == "i64" {
            "fold(0i64, i64::saturating_add)"
        } else {
            "sum::<i64>()"
        }
    }
    pub(super) fn max(self) -> i128 {
        integer_range(self.kind())
            .expect("native quantity integer")
            .1
    }
}

/// Shared by numeric emission and PR-form constant evaluation. This describes
/// Rust's real conversion bounds, not permission to erase a narrowing clamp.
pub(super) fn integer_range(kind: &str) -> Option<(i128, i128)> {
    Some(match kind {
        "u8" => (0, u8::MAX.into()),
        "u16" => (0, u16::MAX.into()),
        "u32" => (0, u32::MAX.into()),
        "usize" | "u64" => (0, u64::MAX.into()),
        "i8" => (i8::MIN.into(), i8::MAX.into()),
        "i16" => (i16::MIN.into(), i16::MAX.into()),
        "i32" => (i32::MIN.into(), i32::MAX.into()),
        "i64" => (i64::MIN.into(), i64::MAX.into()),
        _ => return None,
    })
}

/// Does this expression use the current event's amount, rather than game state?
pub(super) fn relative_to_event(expr: &Expr) -> bool {
    use crate::visit::{Visitor, Walk};
    #[derive(Default)]
    struct Find(bool);
    impl Visitor for Find {
        fn expr(&mut self, expr: &Expr) -> bool {
            if matches!(expr, Expr::EventAmount) {
                self.0 = true;
            }
            !matches!(expr, Expr::EventHistory { .. })
        }
    }
    let mut found = Find::default();
    expr.walk(&mut found);
    found.0
}

// Inert API shape witnesses read by tools/native_quantity_contract.py. Require
// the complete set before enabling this profile. Whitespace is insignificant.
// Newer independent helpers are deliberately not treated as absent/removed.
#[allow(dead_code)]
const WIDE_CONTRACT: &[(&str, &str)] = &[
    ("cantrip-api/src/game.rs", "fn damage(&mut self, source: ObjectId, target: Target, amount: i64) -> i64;"),
    ("cantrip-api/src/game.rs", "fn damage_result(&mut self, source: ObjectId, target: Target, amount: i64) -> Option<DamageResult>;"),
    ("cantrip-api/src/game.rs", "fn damage_together(&mut self, damage: &[(ObjectId, Target, i64)]) -> Vec<i64>;"),
    ("cantrip-api/src/game.rs", "fn gain_life(&mut self, player: PlayerId, amount: i64);"),
    ("cantrip-api/src/game.rs", "fn lose_life(&mut self, player: PlayerId, amount: i64);"),
    ("cantrip-api/src/game.rs", "fn give_poison(&mut self, player: PlayerId, amount: i64);"),
    ("cantrip-api/src/game.rs", "fn add_counters(&mut self, object: ObjectId, kind: CounterKind, amount: i64);"),
    ("cantrip-api/src/game.rs", "fn add_counters_together(&mut self, counters: &[(ObjectId, CounterKind, i64)]);"),
    ("cantrip-api/src/game.rs", "fn add_counters_by(&mut self, by: PlayerId, object: ObjectId, kind: CounterKind, amount: i64);"),
    ("cantrip-api/src/game.rs", "fn add_counters_together_by(&mut self, counters: &[(PlayerId, ObjectId, CounterKind, i64)]);"),
    ("cantrip-api/src/game.rs", "fn set_life_total(&mut self, player: PlayerId, total: i32);"),
    ("cantrip-api/src/game.rs", "fn move_counters(&mut self, from: ObjectId, to: ObjectId, kind: CounterKind, amount: i16) -> i16;"),
    ("cantrip-api/src/game.rs", "fn remove_counters(&mut self, object: ObjectId, kind: CounterKind, amount: i16) -> i16;"),
    ("cantrip-api/src/card.rs", "pub counters: [i64; CounterKind::COUNT],"),
    ("cantrip-api/src/replace.rs", "pub counts: [i64; CounterKind::COUNT],"),
    ("cantrip-api/src/card.rs", "pub damage_amount: bool,"),
    ("cantrip-api/src/ability.rs", "pub const fn damage_amount(self) -> AbilityDef"),
    ("cantrip-state/src/object.rs", "pub fn matched_damage_amount(&self) -> i64"),
    ("cantrip-api/src/replace.rs", "pub amount: PreventionAmount,"),
    ("cantrip-api/src/replace.rs", "Specific(PreventionAmountFn),"),
    ("cantrip-api/src/replace.rs", "Relative(PreventionAmountFn),"),
    ("cantrip-api/src/replace.rs", "pub type PreventionAmountFn = fn(&dyn Query, &Asker, &DamageEvent) -> i64;"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, process::Command};

    #[test]
    fn wide_effects_do_not_widen_physical_storage_or_other_integer_domains() {
        for (domain, old, beyond) in [
            (Domain::Damage, "u16", 65_536),
            (Domain::Prevention, "u16", 65_536),
            (Domain::CounterPlacement, "i16", 32_768),
            (Domain::LifeEffect, "i32", i64::from(i32::MAX) + 1),
            (Domain::PoisonEffect, "u32", i64::from(u32::MAX) + 1),
        ] {
            planned::with_features(&[], || {
                assert_eq!(domain.kind(), old);
                assert!(Number::integer(beyond).quantity(domain).is_err());
            });
            planned::with_features(&[FEATURE], || {
                assert_eq!(domain.kind(), "i64");
                assert!(Number::integer(beyond).quantity(domain).is_ok());
            });
        }
        planned::with_features(&[FEATURE], || {
            assert!(Number::integer(i64::from(u32::MAX)).choice_amount().is_ok());
            assert!(Number::integer(i64::from(u32::MAX) + 1)
                .choice_amount()
                .is_err());
            assert_eq!(Domain::ShieldCapacity.kind(), "u16");
            assert_eq!(Domain::LifeTotal.kind(), "i32");
            assert_eq!(Domain::StoredCounter.kind(), "i16");
            assert!(Number::integer(32_768)
                .quantity(Domain::StoredCounter)
                .is_err());
            assert!(Number::integer(i64::from(i32::MAX) + 1)
                .quantity(Domain::LifeTotal)
                .is_err());
            assert!(Number::integer(65_536)
                .amount(u16::MAX.into(), "u16")
                .is_err());
            assert_eq!(super::super::type_max("i64"), Some(i64::MAX.into()));
        });
    }

    fn operation(operation: Operation) -> Result<String> {
        let mut compiler = Compiler::new();
        let mut out = String::new();
        compiler.operation(&operation, &ResultId(0), &mut out)?;
        Ok(out)
    }
    fn damage(n: i64) -> Operation {
        Operation::DealDamage {
            source: ObjectRef::Source,
            recipients: Recipients::Players(PlayerSet::One(PlayerRef::AbilityController)),
            amount: Expr::Constant(n),
            preventable: true,
        }
    }
    fn counters(n: i64) -> Operation {
        Operation::PutCounters {
            placements: vec![CounterPlacement {
                recipients: Recipients::Objects(ObjectSet::One(ObjectRef::Source)),
                kind: crate::counter::CounterKind::PowerToughness(1, 1),
                amount: Expr::Constant(n),
                selection: None,
            }],
        }
    }

    #[test]
    fn actual_effect_emitters_use_the_selected_argument_and_batch_contract() {
        planned::with_features(&[], || {
            assert!(operation(damage(65_536)).is_err());
            assert!(operation(counters(32_768)).is_err());
            assert!(operation(damage(3))
                .unwrap()
                .contains("Vec<(ObjectId, Target, u16)>"));
            assert!(operation(counters(3))
                .unwrap()
                .contains("Vec<(ObjectId, CounterKind, i16)>"));
        });
        planned::with_features(&[FEATURE], || {
            let dealt = operation(damage(65_536)).unwrap();
            assert!(dealt.contains("Vec<(ObjectId, Target, i64)>"), "{dealt}");
            assert!(!dealt.contains("as u16"), "{dealt}");
            let puts = operation(counters(32_768)).unwrap();
            assert!(puts.contains("Vec<(ObjectId, CounterKind, i64)>"), "{puts}");
            assert!(!puts.contains("as i16"), "{puts}");
            for op in [
                Operation::GainLife {
                    players: PlayerSet::One(PlayerRef::AbilityController),
                    amount: Expr::Constant(2_147_483_648),
                },
                Operation::LoseLife {
                    players: PlayerSet::One(PlayerRef::AbilityController),
                    amount: Expr::Constant(2_147_483_648),
                },
            ] {
                let emitted = operation(op).unwrap();
                assert!(emitted.contains("as i64"), "{emitted}");
                assert!(!emitted.contains("as i32"), "{emitted}");
            }
        });
    }

    #[test]
    fn local_proposal_quantities_are_not_recovered_from_legacy_payloads() {
        planned::with_features(&[FEATURE], || {
            let mut compiler = Compiler::new();
            compiler.event_at = EventAt::Matcher;
            compiler.frame = Frame {
                amount: Some(Carried::Data(0)),
                quantity: Some(Domain::Damage),
                ..Frame::default()
            };
            let n = compiler.number(&Expr::EventAmount).unwrap();
            assert_eq!(n.code, "event_quantity");
            assert_eq!(n.max, i128::from(i64::MAX));
            compiler.event_at = EventAt::Item;
            assert_eq!(
                compiler.number(&Expr::EventAmount).unwrap().code,
                "item.matched_damage_amount()"
            );
            compiler.frame.quantity = Some(Domain::DamageExcess);
            assert_eq!(
                compiler.number(&Expr::EventAmount).err().unwrap().feature,
                "exact wide event quantity outside its local callback"
            );
        });
    }

    #[test]
    fn emitted_numeric_boundaries_compile_and_retain_values_at_runtime() {
        // This is a typed API stand-in, not an engine semantic claim. It runs the
        // actual Number expressions and exact batch/callback types selected above.
        let expressions = planned::with_features(&[FEATURE], || {
            [
                (Domain::Damage, 65_536),
                (Domain::CounterPlacement, 32_768),
                (Domain::LifeEffect, 2_147_483_648),
                (Domain::PoisonEffect, 4_294_967_296),
                (Domain::Damage, -3),
                (Domain::Damage, i64::MAX),
            ]
            .map(|(domain, value)| {
                (
                    Number::integer(value).quantity(domain).unwrap(),
                    value.max(0),
                )
            })
        });
        let checks = expressions
            .iter()
            .map(|(code, expected)| format!("assert_eq!(accept({code}), {expected}i64);"))
            .collect::<Vec<_>>()
            .join("\n");
        let source = format!(
            r#"
            fn accept(amount: i64) -> i64 {{ amount }}
            fn prevention(_: ()) -> i64 {{ 65536 }}
            fn main() {{
                {checks}
                let callback: fn(()) -> i64 = prevention;
                let damage: Vec<(u32, u32, i64)> = vec![(1, 2, callback(()))];
                assert_eq!(damage[0].2, 65536);
                let counters: Vec<(u32, u32, i64)> = vec![(1, 2, 32768)];
                assert_eq!(counters[0].2, 32768);
                let stored: i16 = 30000;
                let placed: i64 = i64::from(stored);
                assert_eq!(placed, 30000);
            }}"#
        );
        let dir =
            std::env::temp_dir().join(format!("mtgish-quantity-contract-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("main.rs"), source).unwrap();
        let result = Command::new("rustc")
            .args(["--edition=2021", "-Dwarnings"])
            .arg(dir.join("main.rs"))
            .arg("-o")
            .arg(dir.join("witness"))
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(Command::new(dir.join("witness"))
            .status()
            .unwrap()
            .success());
        fs::remove_dir_all(dir).unwrap();
    }

    fn damage_event() -> ReplacementEvent {
        ReplacementEvent::Damage(DamageEvent {
            source_kind: ObjectKind::Permanent,
            sources: ObjectFilter::SameObject(ObjectRef::Source),
            recipients: DamageRecipients::Players(PlayerSet::All),
            kind: DamageKind::Any,
            grouping: DamageGrouping::SourceRecipient,
            subject: DamageSubject::Source,
        })
    }
    #[test]
    fn static_prevention_retains_fixed_capacity_versus_event_relative_meaning() {
        for wide in [false, true] {
            planned::with_features(if wide { &[FEATURE] } else { &[] }, || {
                for (change, variant) in [
                    (EventChange::PreventDamage, "All"),
                    (
                        EventChange::PreventSomeDamage(Expr::Constant(3)),
                        "Specific",
                    ),
                    (
                        EventChange::PreventSomeDamage(Expr::EventAmount),
                        "Relative",
                    ),
                    (
                        EventChange::PreventAllButDamage(Expr::Constant(3)),
                        "Relative",
                    ),
                ] {
                    let mut compiler = Compiler::new();
                    let mut parts = Parts::default();
                    compiler
                        .replacement(
                            "prevention",
                            &ReplacementActive::Battlefield,
                            &damage_event(),
                            &Program {
                                instructions: vec![Instruction::Effect {
                                    result: ResultId(0),
                                    operation: Operation::ModifyEvent { change },
                                }],
                            },
                            &mut parts,
                        )
                        .unwrap();
                    let defs = parts.preventions.join("\n");
                    if wide {
                        assert!(
                            defs.contains(&format!("PreventionAmount::{variant}")),
                            "{defs}"
                        );
                        if variant != "All" {
                            assert!(
                                compiler.functions.contains("-> i64"),
                                "{}",
                                compiler.functions
                            );
                        }
                    } else {
                        assert!(!defs.contains("PreventionAmount::"), "{defs}");
                    }
                }
            });
        }
    }

    #[test]
    fn actual_generated_effect_bodies_compile_and_deliver_exact_values() {
        let bodies = planned::with_features(&[FEATURE, "counter-placement-ceiling"], || {
            [
                damage(65_536),
                counters(32_768),
                Operation::GainLife {
                    players: PlayerSet::One(PlayerRef::AbilityController),
                    amount: Expr::Constant(2_147_483_648),
                },
                Operation::LoseLife {
                    players: PlayerSet::One(PlayerRef::AbilityController),
                    amount: Expr::Constant(2_147_483_648),
                },
                Operation::PutCounters {
                    placements: vec![CounterPlacement {
                        recipients: Recipients::Players(PlayerSet::One(
                            PlayerRef::AbilityController,
                        )),
                        kind: crate::counter::CounterKind::Named(
                            crate::counter::CounterName::PoisonCounter,
                        ),
                        amount: Expr::Constant(4_294_967_296),
                        selection: None,
                    }],
                },
                Operation::PutCountersCapped {
                    amount: Expr::Constant(65_535),
                    kind: crate::counter::CounterKind::PowerToughness(1, 1),
                    object: ObjectRef::Source,
                    at_most: Expr::Constant(7),
                },
            ]
            .map(|op| operation(op).unwrap())
        });
        let functions = bodies.iter().enumerate().map(|(i, body)|
            format!("fn run_{i}(g: &mut dyn Game, asker: &Asker) -> Result<(), ()> {{ {body} Ok(()) }}"))
            .collect::<Vec<_>>().join("\n");
        let source = format!(
            r#"
            #![allow(unused_variables, unused_mut, dead_code)]
            #[derive(Clone, Copy)] struct ObjectId(u32);
            type PlayerId = u8;
            #[derive(Clone, Copy)] enum Target {{ Player(PlayerId), Object(ObjectId) }}
            #[derive(Clone, Copy)] enum CounterKind {{ PlusOne }}
            struct Asker {{ source: ObjectId, controller: PlayerId }}
            struct Obj;
            impl Obj {{ fn counters(&self, _: CounterKind) -> i16 {{ 0 }} }}
            struct State;
            impl State {{ fn obj(&self, _: ObjectId) -> &Obj {{ &Obj }} }}
            trait Query {{ fn state(&self) -> &State {{ &State }} }}
            trait Game: Query {{
                fn damage_together(&mut self, rows: &[(ObjectId, Target, i64)]) -> Vec<i64>;
                fn add_counters_together(&mut self, rows: &[(ObjectId, CounterKind, i64)]);
                fn gain_life(&mut self, player: PlayerId, amount: i64);
                fn lose_life(&mut self, player: PlayerId, amount: i64);
                fn give_poison(&mut self, player: PlayerId, amount: i64);
                fn add_counters_with_limit(&mut self, object: ObjectId, kind: CounterKind, amount: i64, maximum_total: i64);
            }}
            fn is_live(_: &dyn Query, _: ObjectId) -> bool {{ true }}
            mod cantrip_mechanics {{ pub mod choose {{
                pub fn number(_: &mut dyn super::super::Game, player: u8, source: super::super::ObjectId,
                    least: u32, most: u32, _: &str) -> Result<u32, ()> {{
                    assert_eq!(player, 0); assert_eq!(source.0, 1);
                    assert_eq!((least, most), (0, 65535)); Ok(2)
                }}
            }} }}
            #[derive(Default)] struct Mock {{ values: Vec<i64> }}
            impl Query for Mock {{}}
            impl Game for Mock {{
                fn damage_together(&mut self, rows: &[(ObjectId, Target, i64)]) -> Vec<i64> {{ self.values.extend(rows.iter().map(|r| r.2)); rows.iter().map(|r| r.2).collect() }}
                fn add_counters_together(&mut self, rows: &[(ObjectId, CounterKind, i64)]) {{ self.values.extend(rows.iter().map(|r| r.2)); }}
                fn gain_life(&mut self, _: PlayerId, amount: i64) {{ self.values.push(amount); }}
                fn lose_life(&mut self, _: PlayerId, amount: i64) {{ self.values.push(amount); }}
                fn give_poison(&mut self, _: PlayerId, amount: i64) {{ self.values.push(amount); }}
                fn add_counters_with_limit(&mut self, _: ObjectId, _: CounterKind, amount: i64, maximum_total: i64) {{ self.values.extend([amount, maximum_total]); }}
            }}
            {functions}
            fn main() {{ let mut g = Mock::default(); let asker = Asker {{ source: ObjectId(1), controller: 0 }};
                run_0(&mut g, &asker).unwrap(); run_1(&mut g, &asker).unwrap(); run_2(&mut g, &asker).unwrap(); run_3(&mut g, &asker).unwrap(); run_4(&mut g, &asker).unwrap(); run_5(&mut g, &asker).unwrap();
                assert_eq!(g.values, [65536, 32768, 2147483648, 2147483648, 4294967296, 2, 7]);
            }}"#
        );
        let dir =
            std::env::temp_dir().join(format!("mtgish-effect-contract-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("main.rs"), &source).unwrap();
        let compiled = Command::new("rustc")
            .args(["--edition=2021", "-Dwarnings"])
            .arg(dir.join("main.rs"))
            .arg("-o")
            .arg(dir.join("witness"))
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}\n{source}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        assert!(Command::new(dir.join("witness"))
            .status()
            .unwrap()
            .success());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn wide_damage_result_bounds_drive_safe_following_arithmetic() {
        planned::with_features(&[FEATURE], || {
            let mut compiler = Compiler::new();
            compiler.quantity_results.insert(0, Domain::Damage);
            compiler.performed.insert(0, "dealt".into());
            let n = compiler.number(&Expr::Performed(ResultId(0))).unwrap();
            assert_eq!(n.max, i128::from(i64::MAX));
            let doubled = compiler
                .number(&Expr::multiply(
                    Expr::Performed(ResultId(0)),
                    Expr::Constant(2),
                ))
                .unwrap();
            assert!(doubled.code.contains("saturating_mul"), "{}", doubled.code);
            assert_eq!(Domain::Damage.sum_i64(), "fold(0i64, i64::saturating_add)");
            assert_eq!(
                [i64::MAX, i64::MAX]
                    .into_iter()
                    .fold(0i64, i64::saturating_add),
                i64::MAX
            );
        });
    }
    #[test]
    fn lexical_quantity_snapshot_restores_outer_bounds_and_drops_inner_domains() {
        planned::with_features(&[FEATURE], || {
            let mut compiler = Compiler::new();
            compiler.performed.insert(0, "outer_damage".into());
            compiler.quantity_results.insert(0, Domain::Damage);
            let outer = compiler.results();
            compiler.reset_results();
            compiler.performed.insert(1, "inner_damage".into());
            compiler.quantity_results.insert(1, Domain::Damage);
            compiler.restore_results(outer);
            let doubled = compiler
                .number(&Expr::multiply(
                    Expr::Performed(ResultId(0)),
                    Expr::Constant(2),
                ))
                .unwrap();
            let dir =
                std::env::temp_dir().join(format!("mtgish-quantity-scope-{}", std::process::id()));
            fs::create_dir_all(&dir).unwrap();
            let source = format!("fn main() {{ let outer_damage = std::hint::black_box(i64::MAX); let value = {}; assert_eq!(value, i64::MAX); }}", doubled.code);
            fs::write(dir.join("main.rs"), source).unwrap();
            let built = Command::new("rustc")
                .args(["--edition=2021", "-C", "overflow-checks=on"])
                .arg(dir.join("main.rs"))
                .arg("-o")
                .arg(dir.join("witness"))
                .output()
                .unwrap();
            assert!(
                built.status.success(),
                "{}",
                String::from_utf8_lossy(&built.stderr)
            );
            let ran = Command::new(dir.join("witness")).output().unwrap();
            assert!(
                ran.status.success(),
                "{}; generated {}",
                String::from_utf8_lossy(&ran.stderr),
                doubled.code
            );
            assert_eq!(
                compiler.number(&Expr::Performed(ResultId(0))).unwrap().max,
                i128::from(i64::MAX)
            );
            assert!(!compiler.quantity_results.contains_key(&1));
            fs::remove_dir_all(dir).unwrap();
        });
    }
    #[test]
    fn actual_life_delta_matchers_and_effects_roundtrip_u32_magnitudes_exactly() {
        let (gains, losses, effect) = planned::with_features(&[FEATURE], || {
            let mut compiler = Compiler::new();
            let gains = compiler
                .event(&EventSpec::Player {
                    event: PlayerEvent::GainsLife,
                    players: PlayerSet::All,
                    objects: None,
                })
                .unwrap();
            let losses = compiler
                .event(&EventSpec::Player {
                    event: PlayerEvent::LosesLife,
                    players: PlayerSet::All,
                    objects: None,
                })
                .unwrap();
            compiler.frame = losses.frame;
            compiler.event_at = EventAt::Item;
            let quantity = compiler.number(&Expr::EventAmount).unwrap();
            assert_eq!(quantity.max, i128::from(u32::MAX));
            let mut effect = String::new();
            compiler
                .operation(
                    &Operation::GainLife {
                        players: PlayerSet::One(PlayerRef::AbilityController),
                        amount: Expr::EventAmount,
                    },
                    &ResultId(0),
                    &mut effect,
                )
                .unwrap();
            (gains.history_test(), losses.history_test(), effect)
        });
        let source = format!(
            r#"
            #![allow(dead_code, unused_variables)]
            type PlayerId = u8;
            #[derive(Clone, Copy, Debug, PartialEq)] enum Target {{ Player(PlayerId) }}
            enum Event {{ LifeChange {{ player: PlayerId, delta: i64 }}, Other }}
            struct Record {{ event: Event }}
            struct Fired {{ subject: Target, data: [u32; 2] }}
            struct StackItem {{ data: [u32; 2] }}
            struct Asker {{ controller: PlayerId }}
            trait Query {{}}
            trait Game: Query {{ fn gain_life(&mut self, player: PlayerId, amount: i64); }}
            #[derive(Default)] struct Mock {{ gains: Vec<(PlayerId, i64)> }}
            impl Query for Mock {{}}
            impl Game for Mock {{ fn gain_life(&mut self, player: PlayerId, amount: i64) {{ self.gains.push((player, amount)); }} }}
            fn resolve(g: &mut dyn Game, asker: &Asker, item: &StackItem) {{ {effect} }}
            fn main() {{
                let gains = {gains}; let losses = {losses};
                let magnitude = i64::from(i32::MAX) - i64::from(i32::MIN);
                assert_eq!(magnitude, i64::from(u32::MAX));
                for n in [1, magnitude] {{
                    let positive = Record {{ event: Event::LifeChange {{ player: 1, delta: n }} }};
                    let negative = Record {{ event: Event::LifeChange {{ player: 1, delta: -n }} }};
                    assert!(gains(&negative).is_none()); assert!(losses(&positive).is_none());
                    for fired in [gains(&positive).unwrap(), losses(&negative).unwrap()] {{
                        assert_eq!(fired.subject, Target::Player(1));
                        assert_eq!(i64::from(fired.data[0]), n);
                        let mut game = Mock::default();
                        resolve(&mut game, &Asker {{ controller: 0 }}, &StackItem {{ data: fired.data }});
                        assert_eq!(game.gains, [(0, n)]);
                    }}
                }}
                let zero = Record {{ event: Event::LifeChange {{ player: 1, delta: 0 }} }};
                assert!(gains(&zero).is_none()); assert!(losses(&zero).is_none());
                // Forged history outside the native actual-delta invariant is
                // rejected by the checked conversion, never silently truncated.
                let invalid = Record {{ event: Event::LifeChange {{ player: 1, delta: magnitude + 1 }} }};
                assert!(std::panic::catch_unwind(|| gains(&invalid)).is_err());
            }}"#
        );
        let dir = std::env::temp_dir().join(format!(
            "mtgish-actual-life-contract-{}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("main.rs"), &source).unwrap();
        let built = Command::new("rustc")
            .args(["--edition=2021", "-Dwarnings"])
            .arg(dir.join("main.rs"))
            .arg("-o")
            .arg(dir.join("witness"))
            .output()
            .unwrap();
        assert!(
            built.status.success(),
            "{}\n{source}",
            String::from_utf8_lossy(&built.stderr)
        );
        let ran = Command::new(dir.join("witness")).output().unwrap();
        assert!(
            ran.status.success(),
            "{}",
            String::from_utf8_lossy(&ran.stderr)
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
