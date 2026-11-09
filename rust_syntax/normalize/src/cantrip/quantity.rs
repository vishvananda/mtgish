//! Native semantic quantities, separate from physically stored values and indexes.
//! The pinned metadata selects a coherent API contract, never an incidental `i64`.
use super::*;

pub(super) const FEATURE: &str = "wide-effect-quantities";
pub(super) const BOUNDED_32: &str = "32-bit-effect-quantities";

pub(super) fn count_kind() -> &'static str { if planned::switched_on(BOUNDED_32) { "u32" } else { "u16" } }

pub(super) fn pt_kind() -> (&'static str, i128, i128) {
    if planned::switched_on(BOUNDED_32) { ("i32", i32::MIN.into(), i32::MAX.into()) }
    else { ("i16", i16::MIN.into(), i16::MAX.into()) }
}

pub(super) fn x_max() -> i128 {
    if planned::switched_on(BOUNDED_32) { u32::MAX.into() } else { u16::MAX.into() }
}

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
    PoisonProposal,
    SmallPlayerCounter,
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
                Self::ShieldCapacity | Self::SmallPlayerCounter => "u16",
                Self::StoredCounter => "i16",
                _ => "i64",
            }
        } else if planned::switched_on(BOUNDED_32) {
            match self {
                Self::CounterPlacement | Self::StoredCounter | Self::LifeEffect | Self::LifeTotal => "i32",
                Self::PoisonProposal | Self::SmallPlayerCounter => "u16",
                _ => "u32",
            }
        } else {
            match self {
                Self::ShieldCapacity
                | Self::Damage
                | Self::Prevention
                | Self::DamagePrevented
                | Self::PoisonProposal
                | Self::SmallPlayerCounter => "u16",
                Self::CounterPlacement | Self::StoredCounter => "i16",
                Self::LifeEffect | Self::LifeTotal => "i32",
                Self::LifeChangeMagnitude | Self::PoisonEffect | Self::DamageExcess => "u32",
            }
        }
    }
    pub(super) fn number(self, code: String) -> Number {
        Number {
            code,
            min: 0,
            max: self.max(),
            dynamic: true,
        }
    }
    pub(super) fn player_counter(field: &str) -> Self {
        if field == "poison" {
            Self::PoisonProposal
        } else {
            Self::SmallPlayerCounter
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

impl Compiler {
    /// A semantic numeric receipt and its bounds are one binding. Keep callers
    /// from registering a widened local while leaving it classified as a count.
    pub(super) fn report_quantity(
        &mut self,
        result: u32,
        local: impl Into<String>,
        domain: Quantity,
    ) {
        self.performed.insert(result, local.into());
        if planned::switched_on(FEATURE) {
            self.quantity_results.insert(result, domain);
        }
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
    ("cantrip-api/src/replace.rs", "pub poison: i64,"),
    ("cantrip-api/src/replace.rs", "pub energy: u16,"),
    ("cantrip-api/src/replace.rs", "pub experience: u16,"),
    ("cantrip-api/src/event.rs", "recipient_excess: i64,"),
    ("cantrip-api/src/card.rs", "pub damage_amount: bool,"),
    ("cantrip-api/src/ability.rs", "pub const fn damage_amount(self) -> AbilityDef"),
    ("cantrip-state/src/object.rs", "pub fn matched_damage_amount(&self) -> i64"),
    ("cantrip-api/src/replace.rs", "pub amount: PreventionAmount,"),
    ("cantrip-api/src/replace.rs", "Specific(PreventionAmountFn),"),
    ("cantrip-api/src/replace.rs", "Relative(PreventionAmountFn),"),
    ("cantrip-api/src/replace.rs", "pub type PreventionAmountFn = fn(&dyn Query, &Asker, &DamageEvent) -> i64;"),
];

// A separate complete contract for upstream's bounded 32-bit API.
#[allow(dead_code)]
const BOUNDED_32_CONTRACT: &[(&str, &str)] = &[
    ("cantrip-api/src/game.rs", "fn damage(&mut self, source: ObjectId, target: Target, amount: u32) -> u32;"),
    ("cantrip-api/src/game.rs", "fn damage_result(&mut self, source: ObjectId, target: Target, amount: u32) -> Option<DamageResult>;"),
    ("cantrip-api/src/game.rs", "fn damage_together(&mut self, damage: &[(ObjectId, Target, u32)]) -> Vec<u32>;"),
    ("cantrip-api/src/game.rs", "fn gain_life(&mut self, player: PlayerId, amount: i32);"),
    ("cantrip-api/src/game.rs", "fn lose_life(&mut self, player: PlayerId, amount: i32);"),
    ("cantrip-api/src/game.rs", "fn give_poison(&mut self, player: PlayerId, amount: u32);"),
    ("cantrip-api/src/game.rs", "fn add_counters(&mut self, object: ObjectId, kind: CounterKind, amount: i32);"),
    ("cantrip-api/src/game.rs", "fn add_counters_together(&mut self, counters: &[(ObjectId, CounterKind, i32)]);"),
    ("cantrip-api/src/game.rs", "fn add_counters_by(&mut self, by: PlayerId, object: ObjectId, kind: CounterKind, amount: i32);"),
    ("cantrip-api/src/game.rs", "fn add_counters_together_by(&mut self, counters: &[(PlayerId, ObjectId, CounterKind, i32)]);"),
    ("cantrip-api/src/game.rs", "fn set_life_total(&mut self, player: PlayerId, total: i32);"),
    ("cantrip-api/src/game.rs", "fn move_counters(&mut self, from: ObjectId, to: ObjectId, kind: CounterKind, amount: i32) -> i32;"),
    ("cantrip-api/src/game.rs", "fn remove_counters(&mut self, object: ObjectId, kind: CounterKind, amount: i32) -> i32;"),
    ("cantrip-api/src/card.rs", "pub counters: [i32; CounterKind::COUNT],"),
    ("cantrip-api/src/replace.rs", "pub counts: [i32; CounterKind::COUNT],"),
    ("cantrip-api/src/replace.rs", "pub poison: u16,"),
    ("cantrip-api/src/replace.rs", "pub energy: u16,"),
    ("cantrip-api/src/replace.rs", "pub experience: u16,"),
    ("cantrip-api/src/event.rs", "recipient_excess: u32,"),
    ("cantrip-state/src/object.rs", "pub fn counters(&self, kind: CounterKind) -> i32"),
    ("cantrip-state/src/object.rs", "pub damage: u32,"),
    ("cantrip-api/src/chars.rs", "pub power: Option<i32>,"),
    ("cantrip-api/src/chars.rs", "pub toughness: Option<i32>,"),
    ("cantrip-state/src/object.rs", "pub x: u32,"),
    ("cantrip-state/src/object.rs", "Next(u32),"),
    ("cantrip-api/src/ability.rs", "pub const fn counted_by(self, count: fn(&dyn crate::Query, &Asker) -> u32) -> AbilityDef"),
    ("cantrip-api/src/game.rs", "fn reflexive_trigger_counted(&mut self, source: ObjectId, controller: PlayerId, ability: AbilityRef, subject: Target, count: u32);"),
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
    fn bounded_32_profile_preserves_native_domains_and_emits_printed_effects() {
        planned::with_features(&[BOUNDED_32], || {
            for domain in [Domain::Damage, Domain::Prevention, Domain::DamageExcess, Domain::DamagePrevented, Domain::ShieldCapacity, Domain::LifeChangeMagnitude, Domain::PoisonEffect] {
                assert_eq!(domain.kind(), "u32", "{domain:?}");
            }
            for domain in [Domain::CounterPlacement, Domain::StoredCounter, Domain::LifeEffect, Domain::LifeTotal] {
                assert_eq!(domain.kind(), "i32", "{domain:?}");
            }
            assert_eq!(Domain::PoisonProposal.kind(), "u16");
            assert_eq!(Domain::SmallPlayerCounter.kind(), "u16");
            assert!(Number::integer(i64::from(u32::MAX)).quantity(Domain::Damage).is_ok());
            assert!(Number::integer(i64::from(u32::MAX) + 1).quantity(Domain::Damage).is_err());
            assert!(Number::integer(i64::from(i32::MAX)).quantity(Domain::CounterPlacement).is_ok());
            assert!(Number::integer(i64::from(i32::MAX) + 1).quantity(Domain::CounterPlacement).is_err());
            let dealt = operation(damage(65_536)).unwrap();
            let puts = operation(counters(32_768)).unwrap();
            assert!(dealt.contains("Vec<(ObjectId, Target, u32)>"), "{dealt}");
            assert!(puts.contains("Vec<(ObjectId, CounterKind, i32)>"), "{puts}");
        });
    }

    #[test]
    fn x_readers_keep_profile_specific_native_bounds() {
        for (features, expected) in [(&[][..], i128::from(u16::MAX)), (&[BOUNDED_32][..], i128::from(u32::MAX))] {
            planned::with_features(features, || {
                let mut compiler = Compiler::new();
                let value = compiler.number(&Expr::CastX(ObjectRef::Source)).unwrap();
                assert_eq!(value.max, expected);
                assert_eq!(x_max(), expected);
            });
        }
    }

    #[test]
    fn bounded_32_generated_effects_typecheck_and_run_at_old_width_boundaries() {
        let bodies = planned::with_features(&[BOUNDED_32], || {
            [damage(65_536), counters(32_768)].map(|op| operation(op).unwrap())
        });
        let functions = bodies.iter().enumerate().map(|(i, body)|
            format!("fn run_{i}(g: &mut dyn Game, asker: &Asker) -> Result<(), ()> {{ {body} Ok(()) }}"))
            .collect::<Vec<_>>().join("\n");
        let source = format!(r#"
            #![allow(unused_variables, unused_mut, unused_assignments, dead_code)]
            #[derive(Clone, Copy)] struct ObjectId(u32);
            type PlayerId = u8;
            #[derive(Clone, Copy)] enum Target {{ Player(PlayerId), Object(ObjectId) }}
            #[derive(Clone, Copy)] enum CounterKind {{ PlusOne }}
            struct Asker {{ source: ObjectId, controller: PlayerId }}
            struct Obj;
            impl Obj {{ fn counters(&self, _: CounterKind) -> i32 {{ 0 }} }}
            struct State;
            impl State {{ fn obj(&self, _: ObjectId) -> &Obj {{ &Obj }} }}
            trait Query {{ fn state(&self) -> &State {{ &State }} }}
            trait Game: Query {{
                fn damage_together(&mut self, rows: &[(ObjectId, Target, u32)]) -> Vec<u32>;
                fn add_counters_together(&mut self, rows: &[(ObjectId, CounterKind, i32)]);
            }}
            fn is_live(_: &dyn Query, _: ObjectId) -> bool {{ true }}
            #[derive(Default)] struct Mock {{ damage: Vec<u32>, counters: Vec<i32> }}
            impl Query for Mock {{}}
            impl Game for Mock {{
                fn damage_together(&mut self, rows: &[(ObjectId, Target, u32)]) -> Vec<u32> {{ self.damage.extend(rows.iter().map(|r| r.2)); rows.iter().map(|r| r.2).collect() }}
                fn add_counters_together(&mut self, rows: &[(ObjectId, CounterKind, i32)]) {{ self.counters.extend(rows.iter().map(|r| r.2)); }}
            }}
            {functions}
            fn main() {{ let mut g = Mock::default(); let asker = Asker {{ source: ObjectId(1), controller: 0 }};
                run_0(&mut g, &asker).unwrap(); run_1(&mut g, &asker).unwrap();
                assert_eq!(g.damage, [65536]); assert_eq!(g.counters, [32768]);
            }}"#);
        let dir = std::env::temp_dir().join(format!("mtgish-bounded-32-contract-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("main.rs"), &source).unwrap();
        let compiled = Command::new("rustc").args(["--edition=2021", "-Dwarnings"]).arg(dir.join("main.rs")).arg("-o").arg(dir.join("witness")).output().unwrap();
        assert!(compiled.status.success(), "{}\n{source}", String::from_utf8_lossy(&compiled.stderr));
        assert!(Command::new(dir.join("witness")).status().unwrap().success());
        fs::remove_dir_all(dir).unwrap();
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
            #![allow(unused_variables, unused_mut, unused_assignments, dead_code)]
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
    fn damage_history(measure: HistoryMeasure, recipients: DamageRecipients) -> Expr {
        Expr::EventHistory {
            event: Box::new(EventSpec::Damage(DamageEvent {
                source_kind: ObjectKind::Any,
                sources: ObjectFilter::Any,
                recipients,
                kind: DamageKind::Any,
                grouping: DamageGrouping::SourceRecipient,
                subject: DamageSubject::Source,
            })),
            window: HistoryWindow::ThisTurn,
            measure,
        }
    }

    #[test]
    fn exact_history_keeps_amount_bounds_metadata_and_source_snapshots() {
        let (total, doubled, most_pair, most_source, filtered, life) =
            planned::with_features(&[FEATURE], || {
                let compiler = Compiler::new();
                let total_expr = damage_history(HistoryMeasure::TotalAmount, DamageRecipients::Any);
                let total = compiler.number(&total_expr).unwrap();
                assert_eq!(total.max, i128::from(i64::MAX));
                let doubled = compiler.number(&Expr::multiply(total_expr.clone(), Expr::Constant(2))).unwrap();
                assert!(doubled.code.contains("saturating_mul"), "{}", doubled.code);
                let most_pair = compiler.number(&damage_history(HistoryMeasure::MostFromOneSource, DamageRecipients::Players(PlayerSet::All))).unwrap();
                let most_source = compiler.number(&damage_history(HistoryMeasure::MostFromOneSource, DamageRecipients::Any)).unwrap();
                let mut filtered = total_expr;
                let Expr::EventHistory { event, .. } = &mut filtered else { unreachable!() };
                let EventSpec::Damage(damage) = &mut **event else { unreachable!() };
                damage.kind = DamageKind::Noncombat;
                damage.sources = ObjectFilter::All(vec![
                    ObjectFilter::Color(ManaColor::Red),
                    ObjectFilter::ControlledBy(PlayerSet::One(PlayerRef::AbilityController)),
                ]);
                let filtered = compiler.number(&filtered).unwrap();
                assert!(filtered.code.contains("dealt.chars(q)"), "{}", filtered.code);
                assert!(!filtered.code.contains("q.controller(source)"));
                let life = compiler.number(&Expr::EventHistory {
                    event: Box::new(EventSpec::Player { event: PlayerEvent::GainsLife, players: PlayerSet::All, objects: None }),
                    window: HistoryWindow::ThisTurn,
                    measure: HistoryMeasure::TotalAmount,
                }).unwrap();
                assert_eq!(life.max, i128::from(i64::MAX));
                (total.code, doubled.code, most_pair.code, most_source.code, filtered.code, life.code)
            });
        planned::with_features(&[], || {
            let legacy = Compiler::new().number(&damage_history(HistoryMeasure::TotalAmount, DamageRecipients::Any)).unwrap();
            assert!(legacy.code.contains("map(|f: Fired| i64::from(f.data[0]))"));
            assert!(!legacy.code.contains("Option<(Fired, i64)>"));
            assert_eq!(legacy.max, i128::from(u32::MAX));
        });
        let source = format!(r#"
            #![allow(dead_code, unused_variables)]
            #[derive(Clone, Copy, Debug, PartialEq, Eq)] struct ObjectId(u32);
            #[derive(Clone, Copy, Debug, PartialEq, Eq)] struct PlayerId(u8);
            #[derive(Clone, Copy, Debug, PartialEq, Eq)] enum Target {{ Object(ObjectId), Player(PlayerId) }}
            #[derive(Clone, Copy)] enum Color {{ Red }}
            #[derive(Clone, Copy)] struct Colors(bool);
            impl Colors {{ fn has(self, _: Color) -> bool {{ self.0 }} }}
            #[derive(Clone, Copy)] struct Chars {{ colors: Colors }}
            struct Words;
            impl Words {{ fn color(&self, c: Color) -> Color {{ c }} }}
            #[derive(Clone, Copy)] struct Kind(bool);
            impl Kind {{ fn is_combat(self) -> bool {{ self.0 }} }}
            #[derive(Clone, Copy)] enum Event {{ Damage {{ source: ObjectId, target: Target, amount: i64, kind: Kind, controller: PlayerId, known: u32 }}, LifeChange {{ player: PlayerId, delta: i64 }}, Other }}
            struct Record {{ event: Event }}
            struct Fired {{ subject: Target, data: [u32; 2] }}
            struct Asker {{ source: ObjectId, controller: PlayerId }}
            struct Mock {{ rows: Vec<Record>, now_red: bool, now_controller: PlayerId }}
            impl Mock {{
                fn history(&self) -> History<'_> {{ History(&self.rows) }}
                fn chars(&self, _: ObjectId) -> Chars {{ Chars {{ colors: Colors(self.now_red) }} }}
                fn controller(&self, _: ObjectId) -> PlayerId {{ self.now_controller }}
                fn words(&self, _: ObjectId) -> Words {{ Words }}
            }}
            struct History<'a>(&'a [Record]);
            impl<'a> History<'a> {{ fn this_turn(&self) -> &'a [Record] {{ self.0 }} }}
            // The real native DamageDealt overlays the retained last-known row.
            // Here known=1/0 are retained red/blue rows, independent of current
            // colors/controller (and source's current battlefield membership).
            struct DamageDealt {{ source: ObjectId, target: Target, amount: i64, combat: bool, controller: PlayerId, known: u32 }}
            impl DamageDealt {{ fn chars(&self, q: &Mock) -> Chars {{ let _ = q.chars(self.source); Chars {{ colors: Colors(self.known == 1) }} }} }}
            fn total(q: &Mock, asker: &Asker) -> i64 {{ {total} }}
            fn doubled(q: &Mock, asker: &Asker) -> i64 {{ {doubled} }}
            fn most_pair(q: &Mock, asker: &Asker) -> i64 {{ {most_pair} }}
            fn most_source(q: &Mock, asker: &Asker) -> i64 {{ {most_source} }}
            fn filtered(q: &Mock, asker: &Asker) -> i64 {{ {filtered} }}
            fn life(q: &Mock, asker: &Asker) -> i64 {{ {life} }}
            fn damage(source: u32, player: u8, amount: i64, red: bool, controller: u8, combat: bool) -> Record {{ Record {{ event: Event::Damage {{ source: ObjectId(source), target: Target::Player(PlayerId(player)), amount, kind: Kind(combat), controller: PlayerId(controller), known: u32::from(red) }} }} }}
            fn main() {{
                let asker = Asker {{ source: ObjectId(99), controller: PlayerId(0) }};
                let n = i64::from(u32::MAX) + 1;
                let mut q = Mock {{ rows: vec![], now_red: false, now_controller: PlayerId(1) }};
                assert_eq!(total(&q, &asker), 0); assert_eq!(most_pair(&q, &asker), 0);
                q.rows = vec![damage(1, 0, n, true, 0, false)];
                assert_eq!(total(&q, &asker), n); assert_eq!(doubled(&q, &asker), n * 2);
                assert_eq!(filtered(&q, &asker), n); // source is now blue/opponent-controlled
                q.now_red = true; q.now_controller = PlayerId(0);
                q.rows.push(damage(2, 0, 7, false, 0, false)); // now-red does not rewrite blue history
                q.rows.push(damage(3, 0, 11, true, 1, false)); // now-controlled does not rewrite old controller
                q.rows.push(damage(1, 0, 13, true, 0, true)); // combat excluded
                assert_eq!(filtered(&q, &asker), n);
                q.rows = vec![damage(1, 0, n, true, 0, false), damage(1, 0, n, true, 0, false), damage(1, 1, 3*n, true, 0, false), damage(2, 0, 4*n, true, 0, false)];
                assert_eq!(most_pair(&q, &asker), 4*n); // source AND recipient, not one source's 5*n
                assert_eq!(most_source(&q, &asker), 5*n);
                q.rows = vec![damage(1, 0, i64::MAX, true, 0, false), damage(1, 0, i64::MAX, true, 0, false)];
                assert_eq!(total(&q, &asker), i64::MAX); assert_eq!(doubled(&q, &asker), i64::MAX);
                assert_eq!(most_pair(&q, &asker), i64::MAX); assert_eq!(most_source(&q, &asker), i64::MAX);
                q.rows = vec![Record {{ event: Event::LifeChange {{ player: PlayerId(0), delta: i64::from(u32::MAX) }} }}, Record {{ event: Event::LifeChange {{ player: PlayerId(0), delta: i64::from(u32::MAX) }} }}, Record {{ event: Event::LifeChange {{ player: PlayerId(0), delta: -7 }} }}];
                assert_eq!(life(&q, &asker), 2*i64::from(u32::MAX));
            }}"#);
        let dir = std::env::temp_dir().join(format!("mtgish-wide-history-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("main.rs"), &source).unwrap();
        let built = Command::new("rustc").args(["--edition=2021", "-C", "overflow-checks=on"]).arg(dir.join("main.rs")).arg("-o").arg(dir.join("witness")).output().unwrap();
        assert!(built.status.success(), "{}\n{source}", String::from_utf8_lossy(&built.stderr));
        let ran = Command::new(dir.join("witness")).output().unwrap();
        assert!(ran.status.success(), "{}", String::from_utf8_lossy(&ran.stderr));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn excess_history_does_not_sum_repeated_recipient_aggregates() {
        planned::with_features(&[FEATURE], || {
            for measure in [HistoryMeasure::TotalAmount, HistoryMeasure::MostFromOneSource] {
                let mut expr = damage_history(measure, DamageRecipients::Any);
                let Expr::EventHistory { event, .. } = &mut expr else { unreachable!() };
                let EventSpec::Damage(damage) = &mut **event else { unreachable!() };
                damage.kind = DamageKind::Excess;
                assert!(Compiler::new().number(&expr).err().unwrap().feature.contains("excess history grouped by simultaneous damage event"));
            }
        });
    }

    #[test]
    fn lexical_quantity_snapshot_restores_outer_bounds_and_drops_inner_domains() {
        planned::with_features(&[FEATURE], || {
            let mut compiler = Compiler::new();
            compiler.performed.insert(0, "outer_damage".into());
            compiler.quantity_results.insert(0, Domain::Damage);
            compiler
                .excess
                .insert(0, ("outer_excess".into(), "outer_targets".into(), None));
            let outer = compiler.results();
            compiler.reset_results();
            compiler.performed.insert(1, "inner_damage".into());
            compiler.quantity_results.insert(1, Domain::Damage);
            compiler
                .excess
                .insert(1, ("inner_excess".into(), "inner_targets".into(), None));
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
            assert!(compiler.excess.contains_key(&0));
            assert!(!compiler.excess.contains_key(&1));
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
    #[test]
    fn player_counter_proposals_preserve_poison_and_small_field_contracts() {
        for wide in [false, true] {
            let functions = planned::with_features(if wide { &[FEATURE] } else { &[] }, || {
                let mut functions = String::new();
                for (field, kind) in [
                    ("poison", Some(crate::counter::CounterName::PoisonCounter)),
                    ("energy", None),
                    (
                        "experience",
                        Some(crate::counter::CounterName::ExperienceCounter),
                    ),
                    ("all", None),
                ] {
                    let mut compiler = Compiler::new();
                    let event = if field == "energy" {
                        ReplacementEvent::Player {
                            event: ReplacedPlayerEvent::GetEnergy,
                            players: PlayerSet::All,
                        }
                    } else {
                        ReplacementEvent::Counters {
                            kind: kind.clone().map(crate::counter::CounterKind::Named),
                            objects: ObjectFilter::Any,
                            players: Some(PlayerSet::All),
                            by: None,
                            effect_only: false,
                        }
                    };
                    let (arms, frame) = compiler.watched(&event).unwrap();
                    let edits = compiler
                        .reshape(
                            "PlayerCounters",
                            &EventChange::SetAmount(Expr::multiply(
                                Expr::EventAmount,
                                Expr::Constant(2),
                            )),
                            frame,
                        )
                        .unwrap();
                    functions.push_str(&format!(
                        "fn double_{field}(c: &mut PlayerCounters) {{ {} {edits} }}\n",
                        arms[0].locals
                    ));
                    if let Some(kind) = kind {
                        let edits = compiler
                            .reshape(
                                "PlayerCounters",
                                &EventChange::AddCounters {
                                    kind: crate::counter::CounterKind::Named(kind),
                                    amount: Expr::Constant(7),
                                },
                                frame,
                            )
                            .unwrap();
                        functions.push_str(&format!(
                            "fn add_{field}(c: &mut PlayerCounters) {{ {} {edits} }}\n",
                            arms[0].locals
                        ));
                    }
                }
                functions
            });
            let poison_type = if wide { "i64" } else { "u16" };
            let poison = if wide { 1i64 << 32 } else { 30000 };
            let source = format!(
                r#"
                #![allow(unused_variables, dead_code)]
                enum Target {{ Player(u8) }}
                struct PlayerCounters {{ player: u8, poison: {poison_type}, energy: u16, experience: u16 }}
                {functions}
                fn fresh() -> PlayerCounters {{ PlayerCounters {{ player: 1, poison: {poison}, energy: 30000, experience: 30000 }} }}
                fn main() {{
                    let mut c = fresh(); double_poison(&mut c); assert_eq!(c.poison, {poison} * 2);
                    let mut c = fresh(); double_energy(&mut c); assert_eq!(c.energy, 60000);
                    let mut c = fresh(); double_experience(&mut c); assert_eq!(c.experience, 60000);
                    let mut c = fresh(); double_all(&mut c); assert_eq!((c.poison, c.energy, c.experience), ({poison} * 2, 60000, 60000));
                    let mut c = fresh(); add_poison(&mut c); add_experience(&mut c);
                    assert_eq!((c.poison, c.energy, c.experience), ({poison} + 7, 30000, 30007));
                }}"#
            );
            let dir = std::env::temp_dir().join(format!(
                "mtgish-player-counter-contract-{}-{wide}",
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

    #[test]
    fn supported_damage_and_excess_receipts_keep_semantic_bounds_and_saturate() {
        let functions = planned::with_features(&[FEATURE], || {
            let mut functions = String::new();
            for (tag, each, fight, read, amount) in [
                ("each_control", true, false, true, 1i64 << 32),
                ("each_max", true, false, true, i64::MAX),
                ("each_unread", true, false, false, i64::MAX),
                ("ordinary_excess", false, false, true, i64::MAX),
                ("fight_excess", false, true, true, i64::MAX),
            ] {
                let mut compiler = Compiler::new();
                compiler.selections.insert(10, "selected_sources".into());
                compiler.affected_object = Some("other");
                let receipt = if each {
                    Expr::Performed(ResultId(0))
                } else {
                    Expr::ExcessDamage {
                        result: ResultId(0),
                        to: Box::new(ObjectFilter::Any),
                    }
                };
                let after = Instruction::Effect {
                    result: ResultId(1),
                    operation: Operation::GainLife {
                        players: PlayerSet::One(PlayerRef::AbilityController),
                        amount: Expr::multiply(receipt.clone(), Expr::Constant(2)),
                    },
                };
                if read {
                    compiler.after = vec![after.clone()];
                }
                let op = if each {
                    Operation::DealDamageFromEach {
                        sources: ObjectSet::Selected(ResultId(10)),
                        recipients: Recipients::Players(PlayerSet::One(
                            PlayerRef::AbilityController,
                        )),
                        amount: Expr::Constant(amount),
                    }
                } else if fight {
                    Operation::Keyword {
                        actor: PlayerSet::One(PlayerRef::AbilityController),
                        action: KeywordAction::Fight {first:ObjectRef::Source,second:ObjectRef::Affected},
                    }
                } else {
                    damage(amount)
                };
                let mut body = String::new();
                compiler.operation(&op, &ResultId(0), &mut body).unwrap();
                if read {
                    assert_eq!(compiler.number(&receipt).unwrap().max, i128::from(i64::MAX));
                    compiler.after.clear();
                    let Instruction::Effect { operation, result } = after else {
                        unreachable!()
                    };
                    compiler.operation(&operation, &result, &mut body).unwrap();
                }
                functions.push_str(&format!("fn run_{tag}(g: &mut dyn Game, asker: &Asker, selected_sources: Vec<ObjectId>, other: ObjectId) {{ {body} }}\n"));
            }
            // A prevention's exact numeric receipt is registered at its source,
            // not reclassified as a u32 count in its continuation.
            let mut compiler = Compiler::new();
            let after = Instruction::Effect {
                result: ResultId(1),
                operation: Operation::GainLife {
                    players: PlayerSet::One(PlayerRef::AbilityController),
                    amount: Expr::multiply(Expr::Performed(ResultId(0)), Expr::Constant(2)),
                },
            };
            let body = compiler
                .then_steps(
                    Frame::default(),
                    &[after],
                    Some((0, "done_prevented", Quantity::Prevention)),
                )
                .unwrap();
            assert!(body.contains("saturating_mul"), "{body}");
            functions.push_str(&format!("fn run_prevention(g: &mut dyn Game, asker: &Asker, done_prevented: i64) {{ {body} }}\n"));
            functions
        });
        let source = format!(
            r#"
            #![allow(unused_variables, unused_mut, unused_assignments, dead_code)]
            #[derive(Clone, Copy, PartialEq, Eq)] struct ObjectId(u32);
            type PlayerId = u8;
            #[derive(Clone, Copy, PartialEq, Eq)] enum Target {{ Player(PlayerId), Object(ObjectId) }}
            impl Target {{ fn object(self) -> Option<ObjectId> {{ match self {{ Self::Object(o) => Some(o), _ => None }} }} }}
            #[derive(Clone, Copy)] enum Event {{ Damage {{ source: ObjectId, target: Target, recipient_excess: i64 }}, Other }}
            #[derive(Clone, Copy)] struct Record {{ event: Event }}
            #[derive(Default)] struct History {{ rows: Vec<Record> }}
            impl History {{ fn head(&self) -> u32 {{ self.rows.len() as u32 }} fn all(&self) -> &[Record] {{ &self.rows }} }}
            struct Obj {{ became: Option<ObjectId> }}
            struct State;
            impl State {{ fn obj(&self, _: ObjectId) -> &Obj {{ &Obj {{ became: None }} }} }}
            struct Asker {{ source: ObjectId, controller: PlayerId }}
            trait Query {{ fn state(&self) -> &State {{ &State }} }}
            trait Game: Query {{
                fn history(&self) -> &History;
                fn damage_together(&mut self, rows: &[(ObjectId, Target, i64)]) -> Vec<i64>;
                fn fight(&mut self, first: ObjectId, second: ObjectId);
                fn gain_life(&mut self, player: PlayerId, amount: i64);
            }}
            #[derive(Default)] struct Mock {{ history: History, rows: usize, gains: Vec<i64> }}
            impl Query for Mock {{}}
            impl Game for Mock {{
                fn history(&self) -> &History {{ &self.history }}
                fn damage_together(&mut self, rows: &[(ObjectId, Target, i64)]) -> Vec<i64> {{
                    self.rows += rows.len();
                    self.history.rows.extend(rows.iter().map(|&(source, target, n)| Record {{ event: Event::Damage {{ source, target, recipient_excess: n }} }}));
                    rows.iter().map(|r| r.2).collect()
                }}
                fn fight(&mut self, first: ObjectId, second: ObjectId) {{ self.damage_together(&[(first, Target::Object(second), i64::MAX), (second, Target::Object(first), i64::MAX)]); }}
                fn gain_life(&mut self, _: PlayerId, amount: i64) {{ self.gains.push(amount); }}
            }}
            {functions}
            fn main() {{
                let asker = Asker {{ source: ObjectId(1), controller: 0 }};
                let sources = vec![ObjectId(1), ObjectId(2)];
                let mut g = Mock::default(); run_each_control(&mut g, &asker, vec![ObjectId(1)], ObjectId(2)); assert_eq!(g.gains, [1i64 << 33]);
                let mut g = Mock::default(); run_each_max(&mut g, &asker, vec![ObjectId(1)], ObjectId(2)); assert_eq!(g.gains, [i64::MAX]);
                let mut g = Mock::default(); run_each_unread(&mut g, &asker, sources.clone(), ObjectId(2)); assert_eq!(g.rows, 2); assert!(g.gains.is_empty());
                let mut g = Mock::default(); run_ordinary_excess(&mut g, &asker, sources.clone(), ObjectId(2)); assert_eq!(g.gains, [i64::MAX]);
                let mut g = Mock::default(); run_fight_excess(&mut g, &asker, sources, ObjectId(2)); assert_eq!(g.gains, [i64::MAX]);
                let mut g = Mock::default(); run_prevention(&mut g, &asker, i64::MAX); assert_eq!(g.gains, [i64::MAX]);
            }}"#
        );
        let dir = std::env::temp_dir().join(format!(
            "mtgish-wide-receipt-contract-{}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("main.rs"), &source).unwrap();
        let built = Command::new("rustc")
            .args(["--edition=2021", "-Dwarnings", "-C", "overflow-checks=on"])
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
            "{}\n{source}",
            String::from_utf8_lossy(&ran.stderr)
        );
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn life_receipt_bounds_distinguish_one_actual_delta_from_multiple_players() {
        planned::with_features(&[FEATURE], || {
            for (players, max) in [
                (
                    PlayerSet::One(PlayerRef::AbilityController),
                    i128::from(u32::MAX),
                ),
                (PlayerSet::All, i128::from(i64::MAX)),
            ] {
                let mut compiler = Compiler::new();
                compiler.after = vec![Instruction::Effect {
                    result: ResultId(1),
                    operation: Operation::GainLife {
                        players: PlayerSet::One(PlayerRef::AbilityController),
                        amount: Expr::Performed(ResultId(0)),
                    },
                }];
                compiler
                    .operation(
                        &Operation::GainLife {
                            players,
                            amount: Expr::Constant(1),
                        },
                        &ResultId(0),
                        &mut String::new(),
                    )
                    .unwrap();
                assert_eq!(
                    compiler.number(&Expr::Performed(ResultId(0))).unwrap().max,
                    max
                );
            }
        });
    }
}
