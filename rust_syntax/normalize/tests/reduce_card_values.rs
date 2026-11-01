//! Values and predicates inside other forms take a typed shape, on the cards that use them.
use mtgish_ast::{ir::*, mtg_types::OracleCard, semantics, unified};
use std::path::PathBuf;
use std::sync::OnceLock;

fn corpus() -> &'static str {
    static TEXT: OnceLock<String> = OnceLock::new();
    TEXT.get_or_init(|| {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/mtgish.lines.ron");
        std::fs::read_to_string(path).unwrap()
    })
}

/// The unified card as JSON, once it round-trips.
fn json(name: &str) -> String {
    let wanted = format!("Name: {name:?}");
    let line = corpus()
        .lines()
        .find(|line| line.contains(&wanted))
        .unwrap();
    let source: OracleCard = ron::from_str(line).unwrap();
    let lowered = unified::lower(&source);
    assert!(
        unified::named_forms(&lowered.card).is_empty(),
        "{name} has named forms"
    );
    unified::round_trip(&lowered.card).unwrap();
    serde_json::to_string(&lowered.card).unwrap()
}

#[test]
fn each_card_reads_its_value_in_a_typed_shape() {
    for (name, row, shape) in [
        ("Spellweaver Helix", "TheOtherExiledCard", "\"TheOther\""),
        ("Mangara's Tome", "TopCardOfExiledPile", "\"AtEnd\""),
        (
            "Minamo's Meddling",
            "SharesANameWithACardSpliceOntoSpell",
            "\"SplicedOnto\"",
        ),
        ("Dance with Calamity", "WasExiledThisWay", "\"Exiled\""),
        (
            "Illusionary Mask",
            "ManaCostIsSubsetOfManaPaidForThisAbility",
            "\"ManaCostPayableWith\"",
        ),
        (
            "Vizier of the Anointed",
            "CardsInLibrary::HasAbility",
            "\"Eternalize\"",
        ),
        (
            "Selective Obliteration",
            "TheColorChosenByItsController",
            "\"ChosenBy\"",
        ),
        (
            "Spoils of War",
            "ANumberOfCardsInAPlayersGraveyard",
            "\"OneOf\"",
        ),
        (
            "Rebbec, Architect of Ascension",
            "AnyManaValueAmongPermanents",
            "\"AnyOf\"",
        ),
        (
            "Ever-Watching Threshold",
            "AttackingPlayerPassesFilter",
            "PlayersAndPlaneswalkers",
        ),
        ("Soul Partition", "CastByAPlayer", "\"Scoped\""),
        (
            "Mindblade Render",
            "CombatDamageWasDealtByACreature",
            "\"Damage\"",
        ),
        (
            "Way of the Mind Sculptor",
            "PlayerPaidANumberOfCountersToActivateIt",
            "\"CountersPaid\"",
        ),
        (
            "Cinderheart Giant",
            "CreatureOrPlaneswalkerChosenAtRandom",
            "\"Random\"",
        ),
        (
            "Ghyrson Starn, Kelermorph",
            "Trigger_ThatRecipient",
            "\"DamagedByEvent\"",
        ),
        (
            "Protective Sphere",
            "SharesAColorWithAColorOfManaSpendOnActivationCost",
            "\"SpentOnActivation\"",
        ),
        (
            "Caller of the Untamed",
            "AManaValueOfAnExiledCard",
            "\"AnyOf\"",
        ),
        (
            "Balduvian Fallen",
            "AmountOfManaOfTypeSpentOnCumulativeUpkeep",
            "\"SpentOnEventCost\"",
        ),
        (
            "Battlefield Thaumaturge",
            "PermanentItTargets",
            "\"TargetsOf\"",
        ),
        ("Soul Burn", "TheAmountOfColorManaSpentOnX", "\"SpentOnX\""),
        (
            "Pygmy Hippo",
            "TheAmountOfManaLostThisWay",
            "\"ManaLostThisWay\"",
        ),
        (
            "Cemetery Prowler",
            "TheNumberOfCardTypesItSharesWithAnyExiledCard",
            "\"SharedValues\"",
        ),
        (
            "Indulge",
            "TheNumberOfCreaturesThatDealtCombatDamageToAPlayer",
            "\"DistinctSources\"",
        ),
        (
            "Fade Away",
            "TheNumberOfRepeatedCostsNotPaidByPlayer",
            "\"RepeatedCostsUnpaid\"",
        ),
        (
            "Quantum Riddler",
            "WouldDrawACard_ThatMany",
            "\"EventAmount\"",
        ),
        (
            "Imodane, the Pyrohammer",
            "SingleTargetPermanentOfSpell",
            "\"SoleTargetOf\"",
        ),
        (
            "Stand or Fall",
            "InThePermanentPileChosenThisWay",
            "\"InPile\"",
        ),
        (
            "Magnigoth Treefolk",
            "IsLandTypeVariable",
            "\"SharesWithAny\"",
        ),
        (
            "C.A.M.P.",
            "SharesAColorWithAnyManaColorProduced",
            "\"ProducedByEvent\"",
        ),
        (
            "Herald of Vengeance",
            "SharesANameWithAPermanentThatDealtDamageToPlayerLastTurn",
            "\"FromHistory\"",
        ),
        (
            "Suffocation",
            "ControllerOfLastSpellThatDealtDamageToPlayerThisTurn",
            "\"Bottom\"",
        ),
        (
            "Glyph of Reincarnation",
            "ControllerOfPermanentTheLastTimeItWasBlockedByPermanentThisTurn",
            "\"LastEvent\"",
        ),
        ("Lodestone Bauble", "SingleGraveyardOwner", "\"OwnerOf\""),
        (
            "Norn's Decree",
            "Trigger_ControllerOfThoseCreatures",
            "\"AttackingPlayer\"",
        ),
        ("Tempt with Bunnies", "AllowedToDrawCards", "\"CanDraw\""),
        ("Thoughtweft's Call", "HasACardInHand", "\"InZone\""),
        (
            "Commander's Plate",
            "NotAColorInCommanderColorIdentity",
            "\"ColorOutsideCommanderIdentity\"",
        ),
        (
            "Foraging Wickermaw",
            "TheManaColorChosenThisWay",
            "\"ColorOfMana\"",
        ),
        (
            "Outlaw Stitcher",
            "TheNthSpellCastByPlayerThisTurn",
            "\"NthCast\"",
        ),
        (
            "Goblin Artisans",
            "IsntTheTargetOfAnAbility",
            "\"TargetedBy\"",
        ),
        (
            "Twinning Glass",
            "SharesANameWithASpellCastThisTurn",
            "\"FromHistory\"",
        ),
        (
            "Quicksilver Servitor",
            "WasCastFromAmongCardsPutIntoTheirHandThisTurn",
            "\"PutIntoHand\"",
        ),
        ("Wyll's Reversal", "HasOneOrMoreTargets", "\"TargetCount\""),
    ] {
        let card = json(name);
        assert!(!card.contains(row), "{name} still reads {row}");
        assert!(card.contains(shape), "{name} has no {shape}");
    }
}

#[test]
fn relentless_dead_compares_with_each_mana_value_in_its_graveyard() {
    let card = json("Relentless Dead");
    assert!(!card.contains("AnyManaValueAmongCardsInPlayersGraveyard"));
    assert!(card.contains("\"AnyOf\""));
}

#[test]
fn a_filtered_reflexive_trigger_counts_what_passed_its_filter() {
    let card = json("Seasoned Cryomancer");
    assert!(!card.contains("Trigger_ThatMuch"));
    let upto = card.find("\"UpTo\"").unwrap();
    assert!(card[upto..upto + 80].contains("\"CountObjects\""));
}

#[test]
fn kytheon_counts_attacks_not_survivors() {
    let card = json("Kytheon, Hero of Akros");
    assert!(!card.contains("NumCombatPermanentsPassFilter"));
    let condition = card.find("\"intervening_if\"").unwrap();
    assert!(card[condition..].contains("\"Attacks\""));
}

#[test]
fn forms_on_faces_a_card_export_cannot_reach_reduce_directly() {
    let rooms = Operand::Objects(ObjectSet::Battlefield(ObjectFilter::Any));
    assert!(matches!(
        semantics::reduce_condition(
            "Condition::ThereAreNumberNamesAmongUnlockedDoorsOfPermanents".into(),
            vec![
                Operand::Comparison(Comparison::GreaterOrEqual, Expr::Constant(8)),
                rooms.clone()
            ],
        ),
        Condition::Compare {
            left: Expr::CountDistinct {
                property: DistinctProperty::UnlockedDoorNames,
                ..
            },
            ..
        }
    ));
    assert!(matches!(
        semantics::reduce_condition(
            "Condition::CopiedCardPassesFilter".into(),
            vec![Operand::Filter(ObjectFilter::Any)],
        ),
        Condition::ObjectMatches {
            object: ObjectRef::Chosen,
            ..
        }
    ));
}

#[test]
fn counters_paid_keep_their_kind() {
    let reduced = semantics::reduce_condition(
        "Condition::PlayerPaidANumberOfCountersToActivateIt".into(),
        vec![
            Operand::Player(PlayerRef::AbilityController),
            Operand::Comparison(Comparison::GreaterOrEqual, Expr::Constant(2)),
            Operand::Vocabulary {
                family: "CounterType".into(),
                form: "LoyaltyCounter".into(),
            },
        ],
    );
    let Condition::Compare {
        left: Expr::CountersPaid { kind, .. },
        ..
    } = reduced
    else {
        panic!("{reduced:?}");
    };
    assert!(format!("{kind:?}").contains("Loyalty"), "{kind:?}");
}

#[test]
fn a_comparison_row_with_no_shape_stays_with_its_row() {
    assert!(semantics::reduce_comparison_value("Comparison::NotARealForm", &[]).is_none());
}
