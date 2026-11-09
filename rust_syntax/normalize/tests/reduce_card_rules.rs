use mtgish_ast::{corpus, ir::*, lower, mtg_types as source, semantics, validate};

fn unearth() -> source::OracleCard {
    include_str!("fixtures/scopes.ronl")
        .lines()
        .map(|l| ron::from_str::<source::OracleCard>(l).unwrap())
        .find(|c| {
            corpus::Identity::of(c)
                .unwrap()
                .names
                .iter()
                .any(|n| n == "Unearth")
        })
        .unwrap()
}

/// One rule on a fixture card, through the whole lowering and the validator.
fn lowered(rule: &str) -> String {
    let mut c = unearth();
    let source::OracleCard::Card { rules, .. } = &mut c else {
        panic!()
    };
    *rules = Some(vec![ron::from_str(rule).unwrap()]);
    let card = lower::card(&c).unwrap_or_else(|e| panic!("{e:?}"));
    validate::card(&card).unwrap_or_else(|e| panic!("{e:?}"));
    let text = format!("{card:?}");
    assert!(!text.contains("Primitive"), "{text}");
    text
}

#[test]
fn a_spell_or_ability_narrows_the_event_it_would_cause() {
    let silhouette = semantics::reduce_replacement(
        "EventPreventDamage::ASpellOrAbilityWouldCauseASourceToDealDamageToRecipient".into(),
        vec![
            Operand::Term {
                family: "SpellsAndAbilities".into(),
                form: "TargetsPermanent".into(),
                operands: vec![Operand::Object(ObjectRef::Source)],
            },
            Operand::Filter(ObjectFilter::Any),
            Operand::Recipients(Recipients::Objects(ObjectSet::One(ObjectRef::Source))),
        ],
    );
    assert!(
        matches!(
            &silhouette,
            ReplacementEvent::Caused { cause: ObjectFilter::TargetsAny(_), event }
                if matches!(**event, ReplacementEvent::Damage(_))
        ),
        "{silhouette:?}"
    );
    assert!(semantics::with_replacement(&Default::default(), &silhouette).stack_object);
    let zabaz = lowered(
        "ReplaceWouldPutCounters(AnAbilityWouldPutCountersOfTypeOnAPermanent(And([ModularAbility, TriggeredAbility]), PTCounter(+1, +1), And([IsCardtype(Creature), ControlledByAPlayer(SinglePlayer(You))])), [PutNewAmount(Plus(ThatManyCounters, Integer(1)))])",
    );
    assert!(zabaz.contains("Caused {"), "{zabaz}");
    assert!(zabaz.contains("SetAmount(Add(EventAmount"), "{zabaz}");
}

#[test]
fn each_point_of_damage_and_a_noncombat_amount_are_measured_events() {
    let echo = lowered(
        "SpellActions(ActionList([CreateReplaceWouldDealDamageUntil(Each1DamagePlayerWouldBeDealt(You), [CancelThatDamage], UntilNextUpkeep(You))]))",
    );
    assert!(echo.contains("EachPoint(Damage("), "{echo}");
    let ojer = lowered(
        "ReplaceWouldDealDamage(AnAmountOfNonCombatDamageWouldBeDealtByASourceToARecipient(LessThan(Integer(4)), And([IsColor(Red), ControlledByAPlayer(SinglePlayer(You))]), APlayer(Opponent)), [DealDamageInstead(Integer(4))])",
    );
    assert!(ojer.contains("Measured { event: Damage("), "{ojer}");
    assert!(ojer.contains("kind: Noncombat"), "{ojer}");
}

#[test]
fn a_life_total_compares_what_it_would_become() {
    let angel = lowered(
        "ReplaceWouldReduceLife(PlayersLifeTotalWouldBeReducedToNumberOrLess(You, Integer(0)), [SetLifeTotal(Integer(3))])",
    );
    assert!(angel.contains("Player { event: ReduceLifeTotal"), "{angel}");
    assert!(angel.contains("comparison: LessOrEqual"), "{angel}");
    assert!(angel.contains("change: Cancel"), "{angel}");
}

#[test]
fn lethal_loyalty_reads_the_counters_of_the_damaged_planeswalker() {
    let event = semantics::reduce_replacement(
        "EventReplaceWouldDealDamage::LethalLoyaltyDamageWouldBeDealtToAPlaneswalker".into(),
        vec![Operand::Objects(mtgish_ast::battlefield_query!(ObjectFilter::Any))],
    );
    let ReplacementEvent::Measured {
        comparison, amount, ..
    } = &event
    else {
        panic!("{event:?}")
    };
    assert!(matches!(comparison, Comparison::GreaterOrEqual));
    assert!(matches!(amount, Expr::CountCounters { .. }), "{amount:?}");
    let bound = semantics::with_replacement(&Default::default(), &event);
    assert!(bound.object, "{bound:?}");
}

#[test]
fn turning_up_lets_the_event_go_on() {
    let event = semantics::reduce_replacement(
        "EventWouldMask::PermanentWouldAssignDamageDealDamageBeDealDamageOrBecomeTapped".into(),
        vec![Operand::Object(ObjectRef::Source)],
    );
    let ReplacementEvent::Either(events) = &event else {
        panic!("{event:?}")
    };
    assert_eq!(events.len(), 4);
    assert!(!semantics::body_replaces(&event), "{event:?}");
    assert!(matches!(
        semantics::reduce_replacement_action(
            PlayerSet::One(PlayerRef::AbilityController),
            "ReplacementActionWouldMask::TurnItFaceUpInstead".into(),
            vec![],
        ),
        Operation::Keyword {
            action: KeywordAction::TurnFaceUp {object:ObjectRef::EventSubject},
            ..
        }
    ));
    let wolverine = lowered(
        "ReplaceWouldDealDamage(DamageWouldBeDealtToRecipient(Permanent(ThisPermanent)), [DealDamageButHealPreviousDamage])",
    );
    assert!(wolverine.contains("RemoveAllDamage"), "{wolverine}");
    assert!(!wolverine.contains("Cancel"), "{wolverine}");
}

#[test]
fn a_permanent_put_onto_the_battlefield_can_be_followed() {
    let equilibrium = lowered(
        "ReplaceWouldPutAPermanentOnTheBattlefield(APlayerWouldPutAPermanentOnTheBattlefield(And([Opponent, ControlsNum(GreaterThanOrEqualTo(TheNumberOfPermanentsOnTheBattlefield(And([IsCardtype(Land), ControlledByAPlayer(SinglePlayer(You))]))), IsCardtype(Land))]), IsCardtype(Land)), [WouldPutPermanentOnBattlefield_PutPermanentOnBattlefield, PlayerAction(WouldPutAPermanentOnBattlefield_ThatPlayer, SacrificeAPermanent(IsCardtype(Land)))])",
    );
    assert!(
        equilibrium.contains("PutOntoBattlefield {"),
        "{equilibrium}"
    );
    assert!(equilibrium.contains("change: Then("), "{equilibrium}");
    assert!(equilibrium.contains("Sacrifice {"), "{equilibrium}");
    assert!(!equilibrium.contains("Cancel"), "{equilibrium}");
}

#[test]
fn replaced_mana_keeps_how_much_was_made() {
    let dawn = lowered(
        "SpellActions(ActionList([CreateReplaceWouldProduceManaUntil(ASpellOrAbilityWouldProduceColoredMana(ControlledByAPlayer(SinglePlayer(You))), [WouldProduceMana_ReplaceColor(White)], UntilEndOfTurn)]))",
    );
    assert!(
        dawn.contains("ReplaceColoredMana(Amount { color: White"),
        "{dawn}"
    );
}

#[test]
fn a_cost_can_scale_with_symbols_in_a_spell_or_an_activation() {
    let spells = lowered(
        "StackSpellsEffect(AnySpell, [AdditionalCostForEachColorManaSymbolInCosts(SacrificeAPermanent(IsLandType(Swamp)), Black)])",
    );
    assert!(spells.contains("Repeated {"), "{spells}");
    assert!(spells.contains("CountManaSymbols {"), "{spells}");
    let abilities = lowered(
        "ActivatedAbilityEffect(AnyAbility, AdditionalCostForEachColorManaSymbolInCosts(SacrificeAPermanent(IsLandType(Swamp)), Black))",
    );
    assert!(abilities.contains("ActivationCost {"), "{abilities}");
    assert!(
        abilities.contains("ActivationCostSymbols([Black])"),
        "{abilities}"
    );
}

#[test]
fn color_words_change_on_spells_and_permanents() {
    let swirl = lowered(
        "EachPermanentAndSpellEffect(AnyPermanentOrSpell, ReplaceAllColorWordsWithNewColorWord(TheChosenColor))",
    );
    assert!(swirl.contains("ReplaceWord {"), "{swirl}");
    assert!(swirl.contains("Union(["), "{swirl}");
}

#[test]
fn a_static_can_outlast_its_source() {
    let song = lowered(
        "EachPermanentStickyLayerEffect(And([IsNonCardtype(Creature), IsCardtype(Artifact)]), [LosesAllAbilities, AddCardtype(Artifact), AddCardtype(Creature), SetPowerAndToughnessBoth(Integer(2))], UntilEndOfTurn)",
    );
    assert!(song.contains("ContinuesAfterLeaving(EndOfTurn)"), "{song}");
    assert!(song.contains("RemoveAllAbilities"), "{song}");
    assert!(song.contains("SetBasePowerToughness"), "{song}");
}

#[test]
fn covercast_watches_from_every_zone_but_the_battlefield() {
    let ceremony = lowered(
        "TriggerI_Covercast(WhenAPlayerCastsASpell(SinglePlayer(You), Or([IsCardtype(Instant), IsCardtype(Sorcery)])), SpellPassesFilter(Trigger_ThatSpell, AnAmountOfManaWasSpentToCastIt(GreaterThanOrEqualTo(Integer(5)))), ActionList([IntensifyCard(ThisCard)]))",
    );
    assert!(
        ceremony.contains("zones: [Hand, Library, Graveyard, Exile, Stack]"),
        "{ceremony}"
    );
    assert!(ceremony.contains("Not(SameObject(Source))"), "{ceremony}");
    assert!(ceremony.contains("intervening_if: Some("), "{ceremony}");
}

#[test]
fn a_modal_trigger_is_each_of_its_triggers() {
    let ability = semantics::reduce_ability(
        "Rule::TriggerModalA".into(),
        vec![Operand::List(vec![Operand::Ability(Box::new(
            Ability::Triggers(vec![]),
        ))])],
    );
    assert!(matches!(ability, Ability::Triggers(ref each) if each.len() == 1));
    let wrong = semantics::reduce_ability(
        "Rule::TriggerModalA".into(),
        vec![Operand::List(vec![Operand::Text("no".into())])],
    );
    assert!(matches!(wrong, Ability::Primitive { .. }), "{wrong:?}");
}

#[test]
fn an_ability_can_target_only_one_kind() {
    let wall = lowered(
        "PermanentRuleEffect(ThisPermanent, [CantBeTheTargetOfSpells(CanTargetOnly(IsCreatureType(Wall))), CantBeTheTargetOfAbilities(CanTargetOnly(IsCreatureType(Wall)))])",
    );
    assert_eq!(wall.matches("CanTargetOnly").count(), 2, "{wall}");
}

#[test]
fn permissions_keep_their_zone_limit_and_player() {
    let realm = lowered(
        "PlayerEffect(You, [MayPlayLandsFromOutsideTheGame(And([IsSupertype(Basic), IsCardtype(Land)]))])",
    );
    assert!(realm.contains("from: Outside"), "{realm}");
    assert!(realm.contains("action: PlayLand"), "{realm}");
    let kagha = lowered(
        "PlayerEffect(You, [MayPlayALandOrCastASpellFromAmongCardsInTheirGraveyardThatWerePutThereFromTheirLibraryOnceEachPlayers(IsCardtype(Land), IsPermanent, AnyCard, You)])",
    );
    assert!(
        kagha.contains("ArrivedThisTurn { from: Some(Library) }"),
        "{kagha}"
    );
    assert!(kagha.contains("limit: Some(EachTurn(1))"), "{kagha}");
    let multiverse = lowered(
        "PlayerEffect(You, [MayCastASpellFromHandOrTopOfLibraryWithoutPayingOnceEachPlayersTurn(AnySpell)])",
    );
    assert!(multiverse.contains("AlternateDuring {"), "{multiverse}");
    assert!(multiverse.contains("cost: Free"), "{multiverse}");
}

#[test]
fn a_program_a_cast_gets_reads_that_spell() {
    let galea = lowered(
        "PlayerEffect(You, [MayCastSpellsFromTopOfLibrary_SpellsWithTrigger(Or([IsEnchantmentType(Aura), IsArtifactType(Equipment)]), IsArtifactType(Equipment), ActionList([CreateSpellEffect(Trigger_ThatSpell, [AddAbility([Vigilance])])]))])",
    );
    assert!(galea.contains("PlayPermissionWith"), "{galea}");
    assert!(galea.contains("EventSubject"), "{galea}");
}

#[test]
fn durations_end_on_the_words_their_cards_print() {
    let face_up = semantics::reduce_duration(
        "Expiration::AsLongAsPlaneIsFaceUp",
        &[Operand::Vocabulary {
            family: "Plane".into(),
            form: "ThisPlane".into(),
        }],
    );
    assert!(matches!(
        face_up,
        Some(Duration::WhileMatches {
            object: ObjectRef::Source,
            ..
        })
    ));
    let rolled = semantics::reduce_duration(
        "Expiration::UntilPlayerRollsValueWhileRollingToVisitAttractions",
        &[
            Operand::Player(PlayerRef::AbilityController),
            Operand::Comparison(Comparison::LessOrEqual, Expr::Constant(3)),
        ],
    );
    let Some(Duration::UntilEvent(event)) = rolled else {
        panic!("{rolled:?}")
    };
    assert!(matches!(*event, EventSpec::Filtered { .. }), "{event:?}");
}

#[test]
fn future_events_say_how_long_they_wait() {
    let name = "FutureTrigger::WhenPermanentWinsAFightAgainstPermanent";
    let fight = semantics::reduce_future_event(
        name.into(),
        vec![
            Operand::Object(ObjectRef::Source),
            Operand::Object(ObjectRef::EventSubject),
        ],
    );
    assert!(matches!(fight, EventSpec::Filtered { .. }), "{fight:?}");
    assert!(matches!(
        semantics::future_expiry(name),
        Some(Duration::UntilStateBasedActionsChecked)
    ));
    assert!(matches!(
        semantics::future_expiry(
            "FutureTrigger::WhenPlayerNextActivatesAnAbilityBySpendingAnAmountOfMana"
        ),
        Some(Duration::EndOfTurn)
    ));
    assert!(semantics::future_expiry("FutureTrigger::WhenAPlayerPlaneswalks").is_none());
    assert!(matches!(
        semantics::reduce_future_event(
            "FutureTrigger::WhenAPlayerPlaneswalks".into(),
            vec![Operand::Players(PlayerSet::All)],
        ),
        EventSpec::Player {
            event: PlayerEvent::Planeswalks,
            ..
        }
    ));
}

#[test]
fn draft_notes_keep_the_card_they_were_noted_for() {
    let paliano = semantics::reduce_mana_production(
        "ManaProduce::AnyManaColorChosenByPlayerDuringDraft",
        &[
            Operand::Player(PlayerRef::AbilityController),
            Operand::Text("Paliano, the High City".into()),
        ],
    );
    assert!(
        matches!(paliano, Some(ManaProduction::DraftNotedColor(ref note)) if note.card == "Paliano, the High City"),
        "{paliano:?}"
    );
}

#[test]
fn grants_keep_their_keyword_and_cost() {
    let picker = lowered(
        "StackSpellsEffect(And([IsCardtype(Creature), CastByPlayer(You)]), [StickerKicker(PayMana([ManaCostGeneric(1)]))])",
    );
    assert!(picker.contains("mechanic: StickerKicker"), "{picker}");
    assert!(picker.contains("Generic(1)"), "{picker}");
}
