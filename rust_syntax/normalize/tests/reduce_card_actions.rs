//! One-card actions, costs and entry steps read as general shapes.
use mtgish_ast::{corpus, ir::*, mtg_types as source, semantics, unified, validate};

fn sources() -> impl Iterator<Item = source::OracleCard> {
    include_str!("fixtures/reduce_card_actions.ronl")
        .lines()
        .map(|l| ron::from_str(l).unwrap())
}

fn text(found: &source::OracleCard) -> String {
    let lowered = unified::lower(found);
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    validate::unified(&lowered.card).unwrap();
    format!("{:?}", lowered.card)
}

fn card(name: &str) -> String {
    let found = sources()
        .find(|c| {
            corpus::Identity::of(c)
                .unwrap()
                .names
                .iter()
                .any(|n| n == name)
        })
        .unwrap_or_else(|| panic!("{name} is not in the fixture"));
    text(&found)
}

fn says(name: &str, parts: &[&str]) {
    let text = card(name);
    for part in parts {
        assert!(text.contains(part), "{name} lacks {part}: {text}");
    }
}

#[test]
fn no_card_here_keeps_a_primitive() {
    for found in sources() {
        let text = text(&found);
        assert!(!text.contains("Primitive"), "{text}");
    }
}

#[test]
fn the_rest_are_sacrificed_once_for_all_players() {
    for (name, slots) in [
        ("Global Ruin", "Subtype(Forest)"),
        ("Ajani, Nacatl Pariah", "CardType(Planeswalker)"),
    ] {
        says(
            name,
            &[
                "ForEachPlayer",
                "OneForEach",
                slots,
                "Sacrifice { players",
                "objects: Unselected",
            ],
        );
    }
}

#[test]
fn a_pick_of_each_card_type_is_one_exile() {
    says(
        "Grime Gorger",
        &[
            "UpTo(Constant(9))",
            "OneForEach",
            "CardType(Kindred)",
            "destination: Exile",
            "Transition",
        ],
    );
}

#[test]
fn a_spell_moves_as_it_resolves() {
    says(
        "Feather, the Redeemed",
        &["AsResolves", "objects: One(Affected)", "destination: Exile"],
    );
}

#[test]
fn a_token_reads_abilities_from_the_exiled_cost() {
    says(
        "The Book of Vile Darkness",
        &[
            "abilities_of: Some((CostObjects",
            "form: \"TriggeredAbility\"",
        ],
    );
}

#[test]
fn each_listed_ability_is_its_own_effect() {
    let text = card("Akroma, Vision of Ixidor");
    assert_eq!(text.matches("CreateContinuousEffect").count(), 14, "{text}");
    assert!(text.contains("HasKeywordFamily(Partner)"), "{text}");
}

#[test]
fn players_named_by_who_did_not_pay_are_those_offered() {
    says(
        "Zoyowa Lava-Tongue",
        &[
            "optional: true",
            "Choice",
            "Intersection([OpponentsOf(AbilityController), Matching",
            "PlayerTook",
        ],
    );
}

#[test]
fn the_chosen_direction_orders_the_loop() {
    says(
        "Order of Succession",
        &["in_chosen_direction: true", "InChosenDirection"],
    );
}

#[test]
fn an_exiled_card_goes_on_the_stack_as_its_spell() {
    says(
        "Ertai's Meddling",
        &[
            "CopyOnStack { actor: One(ControllerOf(Target",
            "card: Some(Result",
        ],
    );
}

#[test]
fn a_restart_keeps_the_exiled_cards_for_the_new_game() {
    says(
        "Karn Liberated",
        &["RestartGame { keep: Zone", "then: ChangeZone"],
    );
}

#[test]
fn each_number_in_a_range_runs_the_action() {
    says(
        "Ornate Imitations",
        &[
            "EachNumber { from: Constant(1), to: AnnouncedX",
            "EachValue",
        ],
    );
}

#[test]
fn an_untap_step_can_be_watched() {
    says(
        "Undiscovered Paradise",
        &["UntapPermanents", "NextOccurrence", "DuringNextUntapStep"],
    );
}

#[test]
fn a_custom_waterbend_carries_its_x() {
    says(
        "Hama, the Bloodbender",
        &["WaterbendX { mana: [X], x: Earlier"],
    );
}

#[test]
fn costs_spelled_as_one_form_become_their_parts() {
    says(
        "Back from the Brink",
        &["destination: Exile", "ManaCostOf(Earlier { what: Exiled"],
    );
    says(
        "Dakra Mystic",
        &[
            "LibraryTops(All)",
            "destination: OwnersGraveyard",
            "optional: true",
        ],
    );
    says(
        "Manabond",
        &[
            "Reveal { count: Exactly(CountObjects",
            "destination: OwnersBattlefield",
        ],
    );
}

#[test]
fn x_limits_stay_with_the_effect() {
    says(
        "Mind Grind",
        &["LimitX { comparison: Greater, value: Constant(0) }"],
    );
}

#[test]
fn entry_steps_flip_change_and_count_in_this_scope() {
    says(
        "Molten Sentry",
        &["FlipCoin", "CoinFlip", "SetBasePowerToughness"],
    );
    says(
        "The Fourteenth Doctor",
        &["ChangesUntil", "GrantKeyword(Haste)"],
    );
    says(
        "Indominus Rex, Alpha",
        &[
            "ModifyEvent",
            "Named(VigilanceCounter)",
            "Named(FlyingCounter)",
        ],
    );
    says(
        "Sin, Unending Cataclysm",
        &["amount: All", "count: AnyNumber", "CountersRemoved"],
    );
}

#[test]
fn a_commander_put_from_the_command_zone_is_your_own() {
    for name in ["Next of Kin", "Hellkite Courser"] {
        says(
            name,
            &["Shared(Command)", "OwnedBy(One(AbilityController))"],
        );
    }
}

#[test]
fn abilities_other_than_this_one_are_named() {
    says("Ulalek, Fused Atrocity", &["Not(OnStack(ThisAbility))"]);
}

#[test]
fn stickers_carry_their_ticket_limit() {
    says(
        "Pin Collection",
        &["kind: Some(Ability)", "ticket_cost: Some((LessOrEqual"],
    );
}

#[test]
fn a_noted_card_stays_on_a_face_down_entry() {
    says("Magar of the Magic Strings", &["noted: Some(Target"]);
}

#[test]
fn tokens_copy_a_commander_you_own_from_any_zone() {
    says(
        "I Am Never Alone",
        &["CopyOne { objects: AnyZone", "State(Commander)", "OwnedBy"],
    );
}

#[test]
fn optional_top_card_moves_are_paid_for_first() {
    says(
        "Wand of Denial",
        &[
            "LibraryTops",
            "PaymentChosen",
            "destination: OwnersGraveyard",
        ],
    );
}

#[test]
fn an_exile_of_an_unknown_row_stays_primitive() {
    let actor = PlayerSet::One(PlayerRef::AbilityController);
    assert!(matches!(
        semantics::reduce_action(actor, "Action::NotARealForm".into(), vec![]),
        Operation::Primitive { .. }
    ));
}
