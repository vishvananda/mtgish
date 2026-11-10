#[path = "support/query_aliases.rs"]
mod query_aliases;
use mtgish_ast::{
    ir::*,
    mtg_types::OracleCard,
    pipeline, semantics, unified, validate,
    visit::{Visitor, Walk},
};
use serde_json::{json, Value};

fn choices(card: &Card) -> Vec<(ValueKind, ChoiceTerms)> {
    struct Choices(Vec<(ValueKind, ChoiceTerms)>);
    impl Visitor for Choices {
        fn operation(&mut self, operation: &Operation) -> bool {
            if let Operation::ChooseValue { kind, terms, .. } = operation {
                self.0.push((*kind, terms.clone()));
            }
            true
        }
    }
    let mut reader = Choices(vec![]);
    card.walk(&mut reader);
    reader.0
}
fn card(name: &str) -> (OracleCard, Card) {
    let source: OracleCard = include_str!("fixtures/value_choices.ronl")
        .lines()
        .map(|line| ron::from_str(line).unwrap())
        .find(|card| {
            mtgish_ast::corpus::Identity::of(card)
                .unwrap()
                .names
                .iter()
                .any(|n| n == name)
        })
        .unwrap();
    let card = pipeline::finalize(&source).unwrap();
    (source, card)
}
#[test]
fn default_one_any_number_and_exact_zero_are_distinct() {
    assert!(ChoiceTerms::default().exactly_one());
    assert_eq!(ChoiceTerms::default().ordinary_count(), Some(1));
    assert!(matches!(
        ChoiceTerms::ordinary(0, vec![]).count,
        ChoiceCount::AnyNumber
    ));
    for count in [
        ChoiceCount::Exactly(Expr::Constant(0)),
        ChoiceCount::UpTo(Expr::Constant(1)),
        ChoiceCount::Exactly(Expr::ChosenNumber),
        ChoiceCount::Exactly(Expr::Constant(-1)),
    ] {
        assert_eq!(
            ChoiceTerms {
                count,
                ..Default::default()
            }
            .ordinary_count(),
            None
        );
    }
    let choices = choices(&card("Dream Coat").1);
    assert!(matches!(choices[0].1.count, ChoiceCount::AnyNumber));
    let default = serde_json::to_value(ChoiceTerms::default()).unwrap();
    let any = serde_json::to_value(ChoiceTerms::ordinary(0, vec![])).unwrap();
    assert_ne!(default["count"], any["count"]);
}
#[test]
fn full_producer_records_keep_every_operand_except_declared_choice_schema() {
    let source: Vec<_> = include_str!("fixtures/value_choices.ronl")
        .lines()
        .collect();
    let before: Vec<_> = include_str!("fixtures/value_choices_before.jsonl")
        .lines()
        .collect();
    assert_eq!((source.len(), before.len()), (29, 29));
    for (line, before) in source.into_iter().zip(before) {
        let source: OracleCard = ron::from_str(line).unwrap();
        let mut expected: Value = serde_json::from_str(before).unwrap();
        query_aliases::value_choices(&mut expected);
        expected["schema_version"] = json!(SCHEMA_VERSION);
        let actual = pipeline::finalize(&source).unwrap();
        query_aliases::movement_defaults(&mut expected);
        // Preserve the old snapshot while accounting for this exact, separately
        // reviewed single-input exile followup; choices stay fully compared.
        if actual.name == "Stenn, Paranoid Partisan" {
            let objects = expected.pointer_mut("/abilities/2/args/body/instructions/1/args/operation/args/body/instructions/0/args/operation/args/objects/args/objects").unwrap();
            assert_eq!(
                *objects,
                json!({"kind":"One","args":{"kind":"Result","args":{"result":0,"phase":"After","index":0}}})
            );
            *objects = json!({"kind":"Transition","args":{"result":0,"phase":"After"}});
        }
        if actual.name == "Addle" {
            let instructions=expected["abilities"][0]["args"]["body"]["instructions"].as_array_mut().unwrap();
            assert_eq!(instructions[1]["args"]["operation"]["kind"], "Reveal");
            let revealed=instructions[1]["args"]["result"].clone();
            let objects=&mut instructions[2]["args"]["operation"]["args"]["objects"];
            let owner=objects["args"]["scope"]["Players"]["players"].clone();
            assert_eq!(objects["args"]["scope"]["Players"]["zone"],"Hand");
            let filter=objects["args"]["filter"].clone();
            *objects=json!({"kind":"Matching","args":{"objects":{"kind":"InZone","args":{"objects":{"kind":"Observed","args":revealed},"zone":"Hand"}},"filter":{"kind":"All","args":[{"kind":"OwnedBy","args":owner},filter]}}});
        }
        assert_eq!(
            serde_json::to_value(&actual).unwrap(),
            expected,
            "{}",
            actual.name
        );
        validate::card(&actual).unwrap();
        let unified = unified::lower(&source);
        assert!(unified.diagnostics.is_empty());
        unified::round_trip(&unified.card).unwrap();
    }
}
#[test]
fn rich_counts_flags_exclusions_order_and_result_identity_survive() {
    let secret = choices(&card("The Toymaker's Trap").1);
    assert!(secret
        .iter()
        .any(|(_, t)| t.secret && t.unchosen && t.exactly_one()));
    let ordered = choices(&card("Rite of Ruin").1);
    assert!(ordered
        .iter()
        .any(|(_, t)| t.ordered && matches!(t.count, ChoiceCount::Exactly(Expr::Constant(3)))));
    let excluded = choices(&card("Artificial Evolution").1);
    assert!(excluded
        .iter()
        .any(|(_, t)| !t.except.is_empty()
            && matches!(t.count, ChoiceCount::Exactly(Expr::Constant(2)))));
    let limited = choices(&card("Desert Cenote").1);
    assert!(limited
        .iter()
        .any(|(_, t)| matches!(t.count, ChoiceCount::UpTo(Expr::Constant(2)))));
    let random = semantics::reduce_action(
        PlayerSet::One(PlayerRef::AbilityController),
        "Action::ChooseNumberAbilitiesAtRandom".into(),
        vec![
            Operand::Number(Expr::Constant(2)),
            Operand::List(vec![
                Operand::Text("flying".into()),
                Operand::Text("trample".into()),
            ]),
        ],
    );
    assert!(matches!(
        random,
        Operation::ChooseValue {
            terms: ChoiceTerms {
                random: true,
                count: ChoiceCount::Exactly(Expr::Constant(2)),
                ..
            },
            ..
        }
    ));
    let mut count = 0;
    for line in include_str!("fixtures/value_choices.ronl").lines() {
        let source: OracleCard = ron::from_str(line).unwrap();
        count += choices(&pipeline::finalize(&source).unwrap()).len();
    }
    assert!(count > 29); // Artificial Evolution and the Ents retain separate choices/results.
}
#[test]
fn both_reducer_routes_produce_the_same_plain_choice() {
    // The rich normalizer route defaults to exactly one, just like the ordinary
    // grammar table. It must not acquire the table's zero-as-any convention.
    let actor = PlayerSet::One(PlayerRef::AbilityController);
    let ordinary = semantics::reduce_action(actor.clone(), "Action::ChooseANumber".into(), vec![]);
    let restricted = semantics::reduce_action(
        actor,
        "Action::ChooseANumberGreaterThanNumber".into(),
        vec![Operand::Integer(0)],
    );
    for op in [ordinary, restricted] {
        assert!(matches!(op,Operation::ChooseValue {terms,..} if terms.exactly_one()));
    }
}
#[test]
fn choice_codec_has_one_operation_and_typed_cardinality_tags() {
    let cfg = bincode::config::standard();
    for (count, tag) in [
        (ChoiceCount::Exactly(Expr::Constant(0)), 0),
        (ChoiceCount::UpTo(Expr::Constant(1)), 1),
        (ChoiceCount::AnyNumber, 2),
    ] {
        let bytes = bincode::encode_to_vec(&count, cfg).unwrap();
        assert_eq!(bytes[0], tag);
        let (decoded, used): (ChoiceCount, usize) =
            bincode::decode_from_slice(&bytes, cfg).unwrap();
        assert_eq!(used, bytes.len());
        assert_eq!(
            serde_json::to_value(decoded).unwrap(),
            serde_json::to_value(count).unwrap()
        );
    }
    // Schema 173 removes ChooseValueWith before RevealChoices. Binary caches
    // from older schemas must be regenerated rather than decoded as this schema.
    let bytes = bincode::encode_to_vec(
        Operation::RevealChoices {
            kind: ValueKind::Number,
        },
        cfg,
    )
    .unwrap();
    assert_eq!(bytes[0], 92);
}
#[cfg(feature = "cantrip")]
#[test]
fn entering_choices_and_rich_backend_gaps_keep_their_paths() {
    for (name, needle) in [
        ("Seal of the Guildpact", "entering.chosen_number"),
        ("Sower of Discord", "entering.chosen_player"),
        ("Greymond, Avacyn's Stalwart", "choose"),
        ("Imagecrafter", "ChooseSubtype"),
    ] {
        let (source, _) = card(name);
        let lowered = unified::lower(&source);
        let code = mtgish_ast::cantrip::whole_module(&source, &lowered.card).unwrap();
        assert!(code.contains(needle), "{name}: {code}");
        syn::parse_file(&code).unwrap();
    }
    for (name, feature) in [
        ("The Toymaker's Trap", "a value chosen in secret"),
        ("Rite of Ruin", "operation ChooseValueWith"),
        ("Desert Cenote", "operation ChooseValueWith"),
        ("Alhammarret, High Arbiter", "name choice And"),
        ("Gabriel Angelfire", "a choice of Ability"),
        (
            "Phyrexian Splicer",
            "a value chosen as an activation cost, read by its targets",
        ),
        ("Zombie Boa", "a choice of Color outside its body"),
    ] {
        let (source, _) = card(name);
        let lowered = unified::lower(&source);
        let gaps = mtgish_ast::cantrip::whole_gaps(&source, &lowered.card);
        assert!(format!("{gaps:?}").contains(feature), "{name}: {gaps:?}");
    }
}

#[test]
fn dynamic_choice_cardinality_uses_the_current_abilitys_result_scope() {
    fn spell(instructions: Vec<Instruction>) -> Ability {
        Ability::Spell {
            announcement: Announcement {
                targets: vec![],
                modes: None,
            },
            body: Program { instructions },
        }
    }
    let bind = Instruction::Effect {
        result: ResultId(3),
        operation: Operation::BindValue {
            value: Expr::Constant(2),
        },
    };
    let choose = Instruction::Effect {
        result: ResultId(4),
        operation: Operation::ChooseValue {
            choosers: PlayerSet::One(PlayerRef::AbilityController),
            kind: ValueKind::Ability,
            terms: ChoiceTerms {
                count: ChoiceCount::Exactly(Expr::BoundValue(ResultId(3))),
                ..Default::default()
            },
        },
    };
    let mut card = card("Addle").1;
    card.abilities = vec![spell(vec![bind.clone(), choose.clone()])];
    validate::card(&card).unwrap();
    let loaded: Card = serde_json::from_value(serde_json::to_value(&card).unwrap()).unwrap();
    validate::card(&loaded).unwrap();
    card.abilities = vec![spell(vec![choose.clone()])];
    assert!(validate::card(&card)
        .unwrap_err()
        .contains("a bound value is read before it is set"));
    card.abilities = vec![spell(vec![bind]), spell(vec![choose])];
    assert!(validate::card(&card)
        .unwrap_err()
        .contains("a bound value is read before it is set"));
}
