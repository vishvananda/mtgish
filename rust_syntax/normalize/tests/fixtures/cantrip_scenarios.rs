#[cfg(test)]
mod scenarios {
    use super::*;
    use cantrip_api::*;
    use cantrip_rules::{Game as RulesGame, Setup};
    use cantrip_state::{Frame, FrameKind};
    const ME: PlayerId = PlayerId(0);
    const THEM: PlayerId = PlayerId(1);
    static FILLER: CardDef = CardDef {
        name: "Fixture filler",
        has_cost: false,
        ..CardDef::BLANK
    };
    static CHEAP_CREATURE: CardDef = CardDef {
        name: "One mana creature fixture",
        oracle: true,
        cost: mana("{1}"),
        has_cost: true,
        types: TypeSet::of(&[CardType::Creature]),
        power: Some(1),
        toughness: Some(1),
        ..CardDef::BLANK
    };
    static CHEAP_ARTIFACT: CardDef = CardDef {
        name: "One mana artifact fixture",
        types: TypeSet::of(&[CardType::Artifact, CardType::Creature]),
        ..CHEAP_CREATURE
    };
    static ARTIFACT: CardDef = CardDef {
        name: "Four mana artifact creature",
        oracle: true,
        cost: mana("{4}"),
        types: TypeSet::of(&[CardType::Artifact, CardType::Creature]),
        power: Some(2),
        toughness: Some(2),
        ..CardDef::BLANK
    };
    static INDESTRUCTIBLE: CardDef = CardDef {
        name: "Four mana indestructible artifact",
        keywords: Keywords::of(&[Keyword::Indestructible]),
        ..ARTIFACT
    };
    static LAND: CardDef = CardDef {
        name: "Choice fixture land",
        oracle: true,
        has_cost: false,
        types: TypeSet::of(&[CardType::Land]),
        keywords: Keywords::of(&[Keyword::Shroud]),
        ..CardDef::BLANK
    };
    fn intermediate_board(q: &dyn Query, asker: &Asker) -> bool {
        let powers: Vec<_> = q
            .permanents_of(asker.controller)
            .into_iter()
            .filter(|&id| q.chars(id).is(CardType::Creature))
            .map(|id| q.chars(id).power)
            .collect();
        q.state().player(asker.controller).life == 20
            && powers.iter().filter(|&&p| p == Some(5)).count() == 1
            && powers.iter().filter(|&&p| p == Some(6)).count() == 1
    }
    fn observed_intermediate(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
        g.gain_life(item.controller, 1);
        Ok(())
    }
    static OBSERVER: CardDef = CardDef {
        name: "State boundary observer",
        has_cost: false,
        types: TypeSet::of(&[CardType::Enchantment]),
        abilities: &[state_triggered(
            intermediate_board,
            observed_intermediate,
            "When exactly one creature has been pumped, gain 1 life.",
        )],
        ..CardDef::BLANK
    };
    fn separate_pumps(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
        let creatures: Vec<_> = g
            .permanents_of(item.controller)
            .into_iter()
            .filter(|&id| g.chars(id).is(CardType::Creature))
            .collect();
        for object in creatures {
            cantrip_mechanics::pump(g, item.object, object, 1, 0);
            g.end_sentence();
        }
        Ok(())
    }
    static SEPARATE: CardDef = CardDef {
        name: "Separate pump probe",
        abilities: &[ability(
            Cost::NONE,
            separate_pumps,
            "Pump creatures in separate instructions.",
        )],
        ..collection_pump::CARD
    };
    fn prepare_replay(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
        let land = g
            .permanents_of(item.controller)
            .into_iter()
            .find(|&id| g.chars(id).is(CardType::Land))
            .unwrap();
        g.add_counters(land, CounterKind::Stun, 2);
        Ok(())
    }
    static PREPARE: CardDef = CardDef {
        name: "Prepare replay fixture",
        has_cost: true,
        types: TypeSet::of(&[CardType::Sorcery]),
        spell: spell(&[], prepare_replay),
        ..CardDef::BLANK
    };
    fn all_three_present(q: &dyn Query, source: ObjectId, event: &Event) -> Option<Fired> {
        if matches!(event,Event::ZoneChange{new,to,reason:ZoneReason::Created,..} if to.is(ZoneKind::Battlefield)&&q.obj(*new).token)
            && q.permanents_of(q.controller(source))
                .iter()
                .filter(|&&id| q.obj(id).token && q.chars(id).is(CardType::Creature))
                .count()
                == 3
        {
            Fired::PLAIN
        } else {
            None
        }
    }
    static TOKEN_OBSERVER: CardDef = CardDef {
        name: "Token batch observer",
        has_cost: false,
        types: TypeSet::of(&[CardType::Enchantment]),
        abilities: &[triggered(
            when(EventMask::ZONE_CHANGE, all_three_present),
            observed_intermediate,
            "When a token enters and all three are present, gain 1 life.",
        )],
        ..CardDef::BLANK
    };
    fn sequential_tokens(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
        for def in menace::EXTRA {
            let id = g.card_id(def);
            g.create_tokens(id, item.controller, 1, false)?;
            g.end_sentence();
        }
        Ok(())
    }
    static SEQUENTIAL_TOKENS: CardDef = CardDef {
        name: "Sequential token probe",
        spell: spell(&[], sequential_tokens),
        ..menace::CARD
    };
    fn game(hand: &str, permanent: Option<&str>) -> (RulesGame, ObjectId, Option<ObjectId>) {
        let (g, source, target, _) = game_with_lands(hand, permanent, 0);
        (g, source, target)
    }
    fn game_with_lands(
        hand: &str,
        permanent: Option<&str>,
        land_count: usize,
    ) -> (RulesGame, ObjectId, Option<ObjectId>, Vec<ObjectId>) {
        game_with_extras(hand, permanent, land_count, &[])
    }
    fn game_with_extras(
        hand: &str,
        permanent: Option<&str>,
        land_count: usize,
        extras: &[(&str, PlayerId)],
    ) -> (RulesGame, ObjectId, Option<ObjectId>, Vec<ObjectId>) {
        game_with_pool(hand, permanent, land_count, extras, 100)
    }
    fn game_with_pool(
        hand: &str,
        permanent: Option<&str>,
        land_count: usize,
        extras: &[(&str, PlayerId)],
        starting_mana: u32,
    ) -> (RulesGame, ObjectId, Option<ObjectId>, Vec<ObjectId>) {
        game_with_zones(hand, permanent, land_count, extras, starting_mana, &[], &[])
    }
    fn game_with_graveyard(
        hand: &str,
        permanent: Option<&str>,
        land_count: usize,
        extras: &[(&str, PlayerId)],
        starting_mana: u32,
        graveyard: &[&str],
    ) -> (RulesGame, ObjectId, Option<ObjectId>, Vec<ObjectId>) {
        game_with_zones(
            hand,
            permanent,
            land_count,
            extras,
            starting_mana,
            graveyard,
            &[],
        )
    }
    fn game_with_zones(
        hand: &str,
        permanent: Option<&str>,
        land_count: usize,
        extras: &[(&str, PlayerId)],
        starting_mana: u32,
        graveyard: &[&str],
        also_in_hand: &[&str],
    ) -> (RulesGame, ObjectId, Option<ObjectId>, Vec<ObjectId>) {
        let mut definitions = vec![
            &divination::CARD,
            &shivan::CARD,
            &offering::CARD,
            &unsummon::CARD,
            &murder::CARD,
            &drake::CARD,
            &whale::CARD,
            &palinchron::CARD,
            &replay_probe::CARD,
            &FILLER,
            &ARTIFACT,
            &INDESTRUCTIBLE,
            &LAND,
            &collection_pump::CARD,
            &OBSERVER,
            &SEPARATE,
            &PREPARE,
            &alarm::CARD,
            &menace::CARD,
            &shadow::CARD,
            &bake::CARD,
            &servo::CARD,
            &posse::CARD,
            &pair::CARD,
            &TOKEN_OBSERVER,
            &SEQUENTIAL_TOKENS,
            &elves::CARD,
            &birds::CARD,
            &ring::CARD,
            &lotus::CARD,
            &ritual::CARD,
            &morph::CARD,
            &sphere::CARD,
            &cradle::CARD,
            &opportunist::CARD,
            &workshop::CARD,
            &hawker::CARD,
            &signet::CARD,
            &pirates::CARD,
            &CHEAP_CREATURE,
            &CHEAP_ARTIFACT,
            &visionary::CARD,
            &traveler::CARD,
            &anthem::CARD,
            &strength::CARD,
            &command::CARD,
            &raise::CARD,
            &tutor::CARD,
            &gate::CARD,
            &seer::CARD,
            &splitter::CARD,
            &cave::CARD,
            &bolt::CARD,
            &blade::CARD,
            &bears::CARD,
            &melee::CARD,
            &crucible::CARD,
            &mending::CARD,
            &blink::CARD,
            &looting::CARD,
            &reap::CARD,
            &wastes::CARD,
            &cliffs::CARD,
            &impulse::CARD,
            &cindermaw::CARD,
            &ghostway::CARD,
            &wine::CARD,
            &index::CARD,
            &seal::CARD,
            &impulse_look::CARD,
            &stirrings::CARD,
            &litho::CARD,
            &agent::CARD,
            &priest::CARD,
            &spitemare::CARD,
            &faith::CARD,
            &golem::CARD,
            &good::CARD,
            &fling::CARD,
            &truths::CARD,
            &strength_two::CARD,
            &salvo::CARD,
        ];
        for extras in [
            alarm::EXTRA,
            menace::EXTRA,
            shadow::EXTRA,
            bake::EXTRA,
            servo::EXTRA,
            posse::EXTRA,
            pair::EXTRA,
            opportunist::EXTRA,
            pirates::EXTRA,
            traveler::EXTRA,
            litho::EXTRA,
        ] {
            definitions.extend_from_slice(extras);
        }
        let table = Box::leak(Box::new(CardTable::new(definitions)));
        let filler = table.find("Fixture filler").unwrap();
        let mut mine = vec![table.find(hand).unwrap(), table.find(PREPARE.name).unwrap()];
        mine.resize(24, filler);
        let mut theirs = vec![table.find("Unsummon").unwrap()];
        theirs.resize(24, filler);
        let mut g = RulesGame::new(
            table,
            &Setup {
                decks: vec![mine, theirs],
                shuffle: false,
                ..Setup::default()
            },
        );
        g.state.update_turn(|t| {
            t.number = 1;
            t.active = ME;
            t.priority = ME;
            t.set_step(Step::PrecombatMain);
            t.began = true;
        });
        g.state.update_player(ME, |p| p.turn_began = 1);
        for p in [ME, THEM] {
            for mana in Mana::ALL {
                g.state
                    .add_mana(p, mana, starting_mana, ObjectId::NONE, None, false);
            }
        }
        let source = *g
            .state
            .zone(Zone::hand(ME))
            .iter()
            .find(|&&id| g.state.obj(id).card == table.find(hand).unwrap())
            .unwrap();
        let target = permanent.map(|name| {
            let id = g
                .state
                .push_obj(Obj::new(table.find(name).unwrap(), ME, Zone::BATTLEFIELD));
            g.state.zone_push(Zone::BATTLEFIELD, id);
            id
        });
        let lands: Vec<_> = (0..land_count)
            .map(|i| {
                let owner = if i < 8 { ME } else { THEM };
                let mut obj = Obj::new(table.find(LAND.name).unwrap(), owner, Zone::BATTLEFIELD);
                obj.tapped = i + 1 < land_count;
                let id = g.state.push_obj(obj);
                g.state.zone_push(Zone::BATTLEFIELD, id);
                id
            })
            .collect();
        for &(name, owner) in extras {
            let id = g.state.push_obj(Obj::new(
                table.find(name).unwrap(),
                owner,
                Zone::BATTLEFIELD,
            ));
            g.state.zone_push(Zone::BATTLEFIELD, id);
        }
        for name in graveyard {
            let dead = Obj::new(table.find(name).unwrap(), ME, Zone::graveyard(ME));
            let id = g.state.push_obj(dead);
            g.state.zone_push(Zone::graveyard(ME), id);
        }
        for name in also_in_hand {
            let card = Obj::new(table.find(name).unwrap(), ME, Zone::hand(ME));
            let id = g.state.push_obj(card);
            g.state.zone_push(Zone::hand(ME), id);
        }
        g.state.see_statics();
        g.state.frame_push(Frame::new(FrameKind::Priority));
        g.run();
        g.verify = true;
        (g, source, target, lands)
    }
    fn announce(g: &mut RulesGame, source: ObjectId, kind: ActionKind) {
        announce_colored(g, source, kind, None);
    }
    /// Announce with `target` rather than the first candidate.
    fn announce_at(g: &mut RulesGame, source: ObjectId, kind: ActionKind, target: Target) {
        let request = g.pending().cloned().unwrap();
        let RequestKind::Priority { options } = &request.kind else {
            panic!("{:?}", request.kind)
        };
        let option = options
            .iter()
            .find(|o| o.kind == kind && o.source == Some(source))
            .unwrap_or_else(|| panic!("no action {kind:?} for {source:?}: {options:?}"));
        assert!(option.target_candidates.contains(&target));
        let mut action = Action::of(option);
        action.targets = vec![target];
        g.answer(request.id, request.player, Answer::Act(action))
            .unwrap();
    }
    fn announce_colored(
        g: &mut RulesGame,
        source: ObjectId,
        kind: ActionKind,
        color: Option<Color>,
    ) {
        let request = g.pending().cloned().unwrap();
        let RequestKind::Priority { options } = &request.kind else {
            panic!("{:?}", request.kind)
        };
        let option = options
            .iter()
            .find(|o| o.kind == kind && o.source == Some(source))
            .unwrap_or_else(|| panic!("no action {kind:?} for {source:?}: {options:?}"));
        let mut action = Action::of(option);
        if !option.color_choices.is_empty() {
            action.color = color.or_else(|| {
                Color::ALL
                    .into_iter()
                    .find(|&c| option.color_choices.has(c))
            });
        }
        action.targets = option
            .target_candidates
            .iter()
            .take(option.targets_required as usize)
            .copied()
            .collect();
        g.answer(request.id, request.player, Answer::Act(action))
            .unwrap();
    }
    fn resolve(g: &mut RulesGame, target: Option<ObjectId>, respond: bool) {
        resolve_with_choices(g, target, respond, &mut Vec::new());
    }
    fn resolve_with_choices(
        g: &mut RulesGame,
        target: Option<ObjectId>,
        respond: bool,
        choices: &mut Vec<Vec<ObjectId>>,
    ) {
        resolve_options(g, target, respond, choices, &mut Vec::new());
    }
    fn resolve_options(
        g: &mut RulesGame,
        target: Option<ObjectId>,
        respond: bool,
        choices: &mut Vec<Vec<ObjectId>>,
        colors: &mut Vec<Vec<Color>>,
    ) {
        let mut saw_stack = false;
        let mut responded = false;
        for _ in 0..96 {
            g.run();
            saw_stack |= !g.state.stack.is_empty();
            let request = g.pending().cloned().expect("pending answer");
            let answer = match &request.kind {
                RequestKind::Priority { options } => {
                    if saw_stack && g.state.stack.is_empty() {
                        return;
                    }
                    if respond && !responded && request.player == THEM {
                        let option = options
                            .iter()
                            .find(|o| {
                                o.kind == ActionKind::CastSpell
                                    && o.source.is_some_and(|id| {
                                        g.state.obj(id).card == g.cards.find("Unsummon").unwrap()
                                    })
                            })
                            .expect("opponent can respond with Unsummon");
                        responded = true;
                        let mut action = Action::of(option);
                        action.targets = vec![Target::Object(target.unwrap())];
                        Answer::Act(action)
                    } else {
                        Answer::Pass
                    }
                }
                RequestKind::ChooseTarget { candidates, .. } => {
                    let selected = Target::Object(target.expect("fixture target"));
                    assert!(candidates.contains(&selected));
                    Answer::Targets(vec![selected])
                }
                RequestKind::OrderTriggers { sources, .. } => {
                    Answer::Order((0..sources.len() as u32).collect())
                }
                RequestKind::ChooseCards {
                    purpose: Purpose::Untap,
                    cards,
                    rule,
                    ..
                } => {
                    assert!(matches!(
                        rule.count,
                        cantrip_mechanics::select::Count::UpTo(5)
                    ));
                    let chosen = choices.remove(0);
                    assert!(chosen.iter().all(|id| cards.contains(id)));
                    Answer::Cards(chosen)
                }
                RequestKind::PayCosts { options, .. } => {
                    let option = options
                        .iter()
                        .find(|o| o.kind == ActionKind::AssignMana)
                        .or_else(|| {
                            options
                                .iter()
                                .find(|o| o.kind == ActionKind::ConfirmCasting)
                        })
                        .unwrap_or_else(|| panic!("payment {options:?}"));
                    Answer::Act(Action::of(option))
                }
                RequestKind::ChooseColors {
                    choices,
                    count,
                    purpose: ColorUse::AddMana,
                    ..
                } => {
                    let selected = colors.remove(0);
                    assert_eq!(selected.len(), *count as usize);
                    assert!(selected.iter().all(|&c| choices.has(c)));
                    Answer::Colors(selected)
                }
                other => panic!("unexpected question {other:?}"),
            };
            g.answer(request.id, request.player, answer).unwrap();
        }
        panic!("fixture did not resolve");
    }
    fn finish_payment(g: &mut RulesGame, requires_empty_stack: bool) {
        for _ in 0..32 {
            g.run();
            if requires_empty_stack {
                assert!(g.state.stack.is_empty(), "mana ability used the stack");
            }
            let request = g.pending().cloned().unwrap();
            let options = match &request.kind {
                RequestKind::Priority { .. } => return,
                RequestKind::PayCosts { options, .. } => options,
                _ => panic!("unexpected payment question {:?}", request.kind),
            };
            let option = options
                .iter()
                .find(|o| o.kind == ActionKind::AssignMana)
                .or_else(|| {
                    options
                        .iter()
                        .find(|o| o.kind == ActionKind::ConfirmCasting)
                })
                .unwrap();
            g.answer(request.id, request.player, Answer::Act(Action::of(option)))
                .unwrap();
        }
        panic!("payment did not finish");
    }
    #[test]
    fn generated_draw_changes_real_hand_and_library() {
        let (mut g, spell, _) = game("Divination", None);
        let hand = g.state.zone(Zone::hand(ME)).len();
        let library = g.state.zone(Zone::library(ME)).len();
        announce(&mut g, spell, ActionKind::CastSpell);
        resolve(&mut g, None, false);
        assert_eq!(g.state.zone(Zone::hand(ME)).len(), hand - 1 + 2);
        assert_eq!(g.state.zone(Zone::library(ME)).len(), library - 2);
    }
    #[test]
    fn generated_activated_pump_pays_mana_and_changes_power() {
        let (mut g, _, dragon) = game("Divination", Some("Shivan Dragon"));
        let dragon = dragon.unwrap();
        assert_eq!(g.chars(dragon).power, Some(5));
        let red = g.state.player(ME).pool.0[3];
        announce(&mut g, dragon, ActionKind::ActivateAbility);
        resolve(&mut g, None, false);
        assert_eq!(g.chars(dragon).power, Some(6));
        assert_eq!(g.chars(dragon).toughness, Some(5));
        assert_eq!(g.state.player(ME).pool.0[3], red - 1);
    }
    #[test]
    fn generated_offering_reads_lki_and_does_not_require_destruction_success() {
        for name in [
            "Four mana artifact creature",
            "Four mana indestructible artifact",
        ] {
            let (mut g, spell, target) = game("Divine Offering", Some(name));
            let life = g.state.player(ME).life;
            announce(&mut g, spell, ActionKind::CastSpell);
            resolve(&mut g, target, false);
            assert_eq!(g.state.player(ME).life, life + 4);
            assert_eq!(
                g.state.obj(target.unwrap()).became.is_some(),
                name == "Four mana artifact creature"
            );
        }
    }
    #[test]
    fn all_illegal_targets_prevent_the_generated_body_from_resolving() {
        let (mut g, spell, target) = game("Divine Offering", Some("Four mana artifact creature"));
        let life = g.state.player(ME).life;
        announce(&mut g, spell, ActionKind::CastSpell);
        resolve(&mut g, target, true);
        assert_eq!(g.state.player(ME).life, life);
        assert!(g.state.obj(target.unwrap()).became.is_some());
    }
    #[test]
    fn untargeted_etb_choice_accepts_zero_or_a_subset_including_opponents_lands() {
        for choose_none in [false, true] {
            let (mut g, spell, _, lands) = game_with_lands("Peregrine Drake", None, 10);
            let selected = if choose_none {
                vec![]
            } else {
                vec![lands[0], lands[8]]
            };
            let mut choices = vec![selected.clone()];
            announce(&mut g, spell, ActionKind::CastSpell);
            resolve_with_choices(&mut g, None, false, &mut choices);
            assert!(
                choices.is_empty(),
                "ETB prompt was answered during resolution"
            );
            for (i, &land) in lands.iter().enumerate() {
                assert_eq!(
                    g.state.obj(land).tapped,
                    i + 1 < lands.len() && !selected.contains(&land)
                );
            }
            assert!(g
                .state
                .battlefield()
                .iter()
                .any(|&id| g.state.obj(id).card == g.cards.find("Peregrine Drake").unwrap()));
        }
    }
    #[test]
    fn two_resolution_prompts_replay_without_drawing_or_untapping_twice() {
        let (mut g, spell, _, lands) = game_with_lands("Replay probe", None, 10);
        let preparation = *g
            .state
            .zone(Zone::hand(ME))
            .iter()
            .find(|&&id| g.state.obj(id).card == g.cards.find(PREPARE.name).unwrap())
            .unwrap();
        announce(&mut g, preparation, ActionKind::CastSpell);
        resolve(&mut g, None, false);
        assert_eq!(g.state.obj(lands[0]).counters(CounterKind::Stun), 2);
        let hand = g.state.zone(Zone::hand(ME)).len();
        let library = g.state.zone(Zone::library(ME)).len();
        let mut choices = vec![vec![lands[0]], vec![lands[1]]];
        announce(&mut g, spell, ActionKind::CastSpell);
        resolve_with_choices(&mut g, None, false, &mut choices);
        assert!(choices.is_empty());
        assert_eq!(g.state.zone(Zone::hand(ME)).len(), hand - 1 + 1);
        assert_eq!(g.state.zone(Zone::library(ME)).len(), library - 1);
        // Unlike an ordinary untap, a second application would consume another
        // stun counter, so this detects replayed mutations even when idempotent
        // tap flags alone would hide them.
        assert!(g.state.obj(lands[0]).tapped);
        assert_eq!(g.state.obj(lands[0]).counters(CounterKind::Stun), 1);
        assert!(!g.state.obj(lands[1]).tapped);
        assert!(g.state.obj(lands[2]).tapped);
    }
    #[test]
    fn collection_pump_is_not_observed_half_applied_by_a_state_trigger() {
        for (name, expected_life) in [("Collection pump probe", 20), ("Separate pump probe", 21)] {
            let (mut g, _, source, _) = game_with_extras(
                "Divination",
                Some(name),
                0,
                &[("Shivan Dragon", ME), ("State boundary observer", ME)],
            );
            announce(&mut g, source.unwrap(), ActionKind::ActivateAbility);
            resolve(&mut g, None, false);
            let creatures: Vec<_> = g
                .permanents_of(ME)
                .into_iter()
                .filter(|&id| g.chars(id).is(CardType::Creature))
                .collect();
            assert_eq!(creatures.len(), 2);
            assert!(creatures.iter().all(|&id| g.chars(id).power == Some(6)));
            assert_eq!(g.state.player(ME).life, expected_life, "{name}");
        }
    }
    #[test]
    fn generated_tokens_preserve_defined_colors_types_keywords_and_tapped_entry() {
        for (name, subtype, artifact, tapped, flying) in [
            ("Raise the Alarm", Subtype::Soldier, false, false, false),
            ("Servo Exhibition", Subtype::Servo, true, false, false),
            ("Shadow Summoning", Subtype::Spirit, false, true, true),
        ] {
            let (mut g, spell, _) = game(name, None);
            announce(&mut g, spell, ActionKind::CastSpell);
            resolve(&mut g, None, false);
            let tokens: Vec<_> = g
                .permanents_of(ME)
                .into_iter()
                .filter(|&id| g.state.obj(id).token)
                .collect();
            assert_eq!(tokens.len(), 2);
            for id in tokens {
                let c = g.chars(id);
                assert!(c.has_subtype(subtype));
                assert!(c.is(CardType::Creature));
                assert_eq!(c.is(CardType::Artifact), artifact);
                assert_eq!(c.colors.has(Color::White), !artifact);
                assert_eq!(c.power, Some(1));
                assert_eq!(c.toughness, Some(1));
                assert_eq!(c.has(Keyword::Flying), flying);
                assert_eq!(g.state.obj(id).tapped, tapped);
                assert_eq!(g.state.obj(id).owner, ME);
                assert!(!g.cards.get(g.state.obj(id).card).oracle);
            }
        }
    }
    #[test]
    fn heterogeneous_token_definitions_enter_together_and_see_the_whole_batch() {
        for (name, life) in [("Bestial Menace", 23), ("Sequential token probe", 21)] {
            let (mut g, spell, _, _) =
                game_with_extras(name, None, 0, &[(TOKEN_OBSERVER.name, ME)]);
            announce(&mut g, spell, ActionKind::CastSpell);
            resolve(&mut g, None, false);
            assert_eq!(g.state.player(ME).life, life, "{name}");
            let mut power: Vec<_> = g
                .permanents_of(ME)
                .into_iter()
                .filter(|&id| g.state.obj(id).token)
                .map(|id| g.chars(id).power.unwrap())
                .collect();
            power.sort();
            assert_eq!(power, vec![1, 2, 3]);
        }
    }
    #[test]
    fn generated_food_keeps_its_own_activation_and_is_created_after_failed_destruction() {
        let (mut g, spell, target) = game("Bake into a Pie", Some(INDESTRUCTIBLE.name));
        announce(&mut g, spell, ActionKind::CastSpell);
        resolve(&mut g, target, false);
        assert!(g.state.obj(target.unwrap()).became.is_none());
        let food = g
            .permanents_of(ME)
            .into_iter()
            .find(|&id| g.chars(id).has_subtype(Subtype::Food))
            .unwrap();
        let life = g.state.player(ME).life;
        let mana = g.state.player(ME).pool.0.iter().copied().sum::<u32>();
        announce(&mut g, food, ActionKind::ActivateAbility);
        resolve(&mut g, None, false);
        assert_eq!(g.state.player(ME).life, life + 3);
        assert!(g.state.obj(food).became.is_some());
        assert_eq!(
            g.state.player(ME).pool.0.iter().copied().sum::<u32>(),
            mana - 2
        );
    }
    #[test]
    fn exported_registry_includes_tokens_and_keeps_same_named_definitions_distinct() {
        let definitions = exported::definitions();
        assert_eq!(definitions.len(), 14);
        assert_eq!(definitions.iter().filter(|d| d.oracle).count(), 6);
        let mut soldiers: Vec<_> = definitions
            .iter()
            .filter(|d| d.name == "Soldier Token")
            .map(|d| d.power.unwrap())
            .collect();
        soldiers.sort();
        assert_eq!(soldiers, vec![1, 2]);
        let table = CardTable::new(definitions);
        assert!(table.find("Raise the Alarm").is_some());
        assert!(table.find("Larger Soldier probe").is_some());
        assert!(table.find("Food Token").is_some());
    }
    #[test]
    fn mercenary_definition_keeps_its_targeted_sorcery_speed_tap_ability() {
        let (mut g, _, mercenary) = game("Divination", Some("Mercenary Token"));
        let id = mercenary.unwrap();
        assert_eq!(g.chars(id).power, Some(1));
        announce(&mut g, id, ActionKind::ActivateAbility);
        resolve(&mut g, Some(id), false);
        assert_eq!(g.chars(id).power, Some(2));
        assert_eq!(g.chars(id).toughness, Some(1));
        assert!(g.state.obj(id).tapped);
    }
    #[test]
    fn fixed_and_chosen_mana_abilities_resolve_without_a_stack_entry() {
        for (name, color, index, amount) in [
            ("Llanowar Elves", None, 4, 1),
            ("Sol Ring", None, 5, 2),
            ("Birds of Paradise", Some(Color::Red), 3, 1),
            ("Gilded Lotus", Some(Color::Blue), 1, 3),
        ] {
            let (mut g, _, permanent) = game("Divination", Some(name));
            let id = permanent.unwrap();
            let before = g.state.player(ME).pool.0;
            announce_colored(&mut g, id, ActionKind::ActivateAbility, color);
            finish_payment(&mut g, true);
            for i in 0..6 {
                assert_eq!(
                    g.state.player(ME).pool.0[i],
                    before[i] + if i == index { amount } else { 0 },
                    "{name}, mana {i}"
                );
            }
            assert!(g.state.obj(id).tapped);
        }
    }
    #[test]
    fn a_dynamic_mana_ability_is_still_offered_when_it_currently_makes_nothing() {
        for (extra, expected) in [(vec![], 0), (vec![("Shivan Dragon", ME)], 1)] {
            let (mut g, _, source, _) =
                game_with_extras("Divination", Some("Gaea's Cradle"), 0, &extra);
            let before = g.state.player(ME).pool.0[4];
            announce(&mut g, source.unwrap(), ActionKind::ActivateAbility);
            finish_payment(&mut g, true);
            assert_eq!(g.state.player(ME).pool.0[4], before + expected);
        }
    }
    #[test]
    fn mana_producing_spells_use_the_stack_and_add_the_whole_bundle() {
        let (mut g, spell, _) = game("Dark Ritual", None);
        announce(&mut g, spell, ActionKind::CastSpell);
        finish_payment(&mut g, false);
        assert_eq!(g.state.stack.len(), 1);
        let before = g.state.player(ME).pool.0[2];
        resolve(&mut g, None, false);
        assert_eq!(g.state.player(ME).pool.0[2], before + 3);
    }
    #[test]
    fn any_combination_choices_can_produce_two_different_colors_and_draw_once() {
        let (mut g, spell, _) = game("Manamorphose", None);
        let hand = g.state.zone(Zone::hand(ME)).len();
        let library = g.state.zone(Zone::library(ME)).len();
        announce(&mut g, spell, ActionKind::CastSpell);
        finish_payment(&mut g, false);
        let before = g.state.player(ME).pool.0;
        let mut colors = vec![vec![Color::Blue, Color::Red]];
        resolve_options(&mut g, None, false, &mut Vec::new(), &mut colors);
        assert!(colors.is_empty());
        assert_eq!(g.state.player(ME).pool.0[1], before[1] + 1);
        assert_eq!(g.state.player(ME).pool.0[3], before[3] + 1);
        assert_eq!(g.state.zone(Zone::hand(ME)).len(), hand);
        assert_eq!(g.state.zone(Zone::library(ME)).len(), library - 1);
    }
    #[test]
    fn chromatic_sphere_uses_the_stack_and_chooses_its_mana_on_resolution() {
        let (mut g, _, source) = game("Divination", Some("Chromatic Sphere"));
        let id = source.unwrap();
        let library = g.state.zone(Zone::library(ME)).len();
        let hand = g.state.zone(Zone::hand(ME)).len();
        announce(&mut g, id, ActionKind::ActivateAbility);
        finish_payment(&mut g, false);
        assert_eq!(g.state.stack.len(), 1);
        assert!(g.state.obj(id).became.is_some());
        assert_eq!(g.state.zone(Zone::library(ME)).len(), library);
        let before = g.state.player(ME).pool.0[1];
        let mut colors = vec![vec![Color::Blue]];
        resolve_options(&mut g, None, false, &mut Vec::new(), &mut colors);
        assert!(colors.is_empty());
        assert_eq!(g.state.player(ME).pool.0[1], before + 1);
        assert_eq!(g.state.zone(Zone::hand(ME)).len(), hand + 1);
        assert_eq!(g.state.zone(Zone::library(ME)).len(), library - 1);
    }
    #[test]
    fn a_generated_treasure_sacrifices_for_its_chosen_mana_without_using_the_stack() {
        let (mut g, spell, _) = game("Prosperous Pirates", None);
        announce(&mut g, spell, ActionKind::CastSpell);
        resolve(&mut g, None, false);
        let treasures: Vec<_> = g
            .permanents_of(ME)
            .into_iter()
            .filter(|&id| g.chars(id).has_subtype(Subtype::Treasure))
            .collect();
        assert_eq!(treasures.len(), 2);
        assert!(g.state.obj(treasures[0]).token);
        let before = g.state.player(ME).pool.0[4];
        announce_colored(
            &mut g,
            treasures[0],
            ActionKind::ActivateAbility,
            Some(Color::Green),
        );
        finish_payment(&mut g, true);
        assert_eq!(g.state.player(ME).pool.0[4], before + 1);
        assert!(g.state.obj(treasures[0]).became.is_some());
        assert!(g.state.obj(treasures[1]).became.is_none());
    }
    #[test]
    fn powerstone_mana_pays_for_artifacts_but_cannot_pay_for_nonartifact_spells() {
        for (name, allowed) in [(CHEAP_ARTIFACT.name, true), (CHEAP_CREATURE.name, false)] {
            let (mut g, spell, source, _) =
                game_with_pool(name, Some("Powerstone Token"), 0, &[], 0);
            announce(&mut g, source.unwrap(), ActionKind::ActivateAbility);
            finish_payment(&mut g, true);
            assert_eq!(g.state.player(ME).pool.0[5], 0);
            let restricted: Vec<_> = g.state.restricted_of(ME).collect();
            assert_eq!(restricted.len(), 1);
            assert_eq!(restricted[0].0, ManaRestriction::NotNonartifactSpells);
            assert_eq!(restricted[0].1 .0[5], 1);
            let RequestKind::Priority { options } = &g.pending().unwrap().kind else {
                panic!()
            };
            assert_eq!(
                options
                    .iter()
                    .any(|o| o.kind == ActionKind::CastSpell && o.source == Some(spell)),
                allowed
            );
            if allowed {
                announce(&mut g, spell, ActionKind::CastSpell);
                resolve(&mut g, None, false);
                assert_eq!(g.state.player(ME).pool.0[5], 0);
                assert_eq!(
                    g.state
                        .restricted_of(ME)
                        .map(|(_, pool)| pool.0[5])
                        .sum::<u32>(),
                    0
                );
            }
        }
    }
    /// Resolve the stack, answering everything but priority and mana through
    /// `answer`, for cards whose questions the fixed answers above do not know.
    fn resolve_asking(
        g: &mut RulesGame,
        answer: &mut dyn FnMut(&RulesGame, &RequestKind) -> Answer,
    ) {
        let mut saw_stack = false;
        for _ in 0..96 {
            g.run();
            saw_stack |= !g.state.stack.is_empty();
            let request = g.pending().cloned().expect("pending answer");
            let reply = match &request.kind {
                RequestKind::Priority { .. } if saw_stack && g.state.stack.is_empty() => return,
                RequestKind::Priority { .. } => Answer::Pass,
                RequestKind::PayCosts { options, .. } => {
                    let option = options
                        .iter()
                        .find(|o| o.kind == ActionKind::AssignMana)
                        .or_else(|| {
                            options
                                .iter()
                                .find(|o| o.kind == ActionKind::ConfirmCasting)
                        })
                        .unwrap_or_else(|| panic!("payment {options:?}"));
                    Answer::Act(Action::of(option))
                }
                other => answer(g, other),
            };
            g.answer(request.id, request.player, reply).unwrap();
        }
        panic!("fixture did not resolve");
    }
    fn permanents_named(g: &RulesGame, name: &str) -> Vec<ObjectId> {
        let card = g.cards.find(name).unwrap();
        g.state
            .battlefield()
            .iter()
            .copied()
            .filter(|&id| g.state.obj(id).card == card)
            .collect()
    }
    #[test]
    fn enters_trigger_draws_a_card_for_its_controller() {
        let (mut g, spell, _) = game("Elvish Visionary", None);
        let hand = g.state.zone(Zone::hand(ME)).len();
        announce(&mut g, spell, ActionKind::CastSpell);
        resolve(&mut g, None, false);
        assert_eq!(permanents_named(&g, "Elvish Visionary").len(), 1);
        assert_eq!(g.state.zone(Zone::hand(ME)).len(), hand);
    }
    #[test]
    fn dies_trigger_leaves_a_flying_token_behind() {
        let (mut g, spell, target) = game("Doom Blade", Some("Doomed Traveler"));
        announce(&mut g, spell, ActionKind::CastSpell);
        resolve(&mut g, target, false);
        assert!(g.state.obj(target.unwrap()).became.is_some());
        let spirits: Vec<_> = g
            .state
            .battlefield()
            .iter()
            .copied()
            .filter(|&id| g.state.obj(id).token)
            .collect();
        assert_eq!(spirits.len(), 1);
        let spirit = g.chars(spirits[0]);
        assert_eq!((spirit.power, spirit.toughness), (Some(1), Some(1)));
        assert!(spirit.keywords.has(Keyword::Flying));
    }
    #[test]
    fn anthem_pumps_only_its_controllers_creatures() {
        let (g, _, _, _) = game_with_extras(
            "Divination",
            Some("Glorious Anthem"),
            0,
            &[("Grizzly Bears", ME), ("Grizzly Bears", THEM)],
        );
        let powers: Vec<_> = permanents_named(&g, "Grizzly Bears")
            .into_iter()
            .map(|id| {
                (
                    g.state.obj(id).controller,
                    g.chars(id).power,
                    g.chars(id).toughness,
                )
            })
            .collect();
        assert_eq!(powers, [(ME, Some(3), Some(3)), (THEM, Some(2), Some(2))]);
    }
    #[test]
    fn aura_attaches_to_its_target_and_changes_it() {
        let (mut g, spell, target) = game("Holy Strength", Some("Grizzly Bears"));
        announce(&mut g, spell, ActionKind::CastSpell);
        resolve(&mut g, target, false);
        let bears = g.chars(target.unwrap());
        assert_eq!((bears.power, bears.toughness), (Some(3), Some(4)));
        assert_eq!(permanents_named(&g, "Holy Strength").len(), 1);
    }
    #[test]
    fn modal_spell_runs_only_the_chosen_modes() {
        let (mut g, spell, target) = game("Kolaghan's Command", Some("Grizzly Bears"));
        let life = g.state.player(THEM).life;
        let hand = g.state.zone(Zone::hand(THEM)).len();
        announce(&mut g, spell, ActionKind::CastSpell);
        let bears = target.unwrap();
        resolve_asking(&mut g, &mut |_, kind| match kind {
            RequestKind::ChooseModes { .. } => Answer::Modes(vec![1, 3]),
            RequestKind::ChooseTarget { candidates, .. } => {
                let wanted = if candidates.contains(&Target::Object(bears)) {
                    Target::Object(bears)
                } else {
                    Target::Player(THEM)
                };
                Answer::Targets(vec![wanted])
            }
            RequestKind::ChooseCards { cards, .. } => Answer::Cards(vec![cards[0]]),
            other => panic!("unexpected question {other:?}"),
        });
        assert!(g.state.obj(bears).became.is_some());
        assert_eq!(g.state.zone(Zone::hand(THEM)).len(), hand - 1);
        assert_eq!(g.state.player(THEM).life, life);
    }
    #[test]
    fn graveyard_target_returns_to_hand() {
        let (mut g, spell, _, _) = game_with_graveyard(
            "Raise Dead",
            None,
            0,
            &[],
            100,
            &["Fixture filler", "Grizzly Bears"],
        );
        let graveyard = g.state.zone(Zone::graveyard(ME)).to_vec();
        let (filler, dead) = (graveyard[0], graveyard[1]);
        let RequestKind::Priority { options } = &g.pending().unwrap().kind else {
            panic!()
        };
        let option = options.iter().find(|o| o.source == Some(spell)).unwrap();
        assert!(!option.target_candidates.contains(&Target::Object(filler)));
        announce_at(&mut g, spell, ActionKind::CastSpell, Target::Object(dead));
        resolve(&mut g, None, false);
        assert_eq!(g.state.zone(Zone::graveyard(ME)).len(), 2);
        let card = g.cards.find("Grizzly Bears").unwrap();
        assert!(g
            .state
            .zone(Zone::hand(ME))
            .iter()
            .any(|&id| g.state.obj(id).card == card));
    }
    #[test]
    fn tutor_finds_the_chosen_card_and_shuffles() {
        let (mut g, spell, _) = game("Diabolic Tutor", None);
        let hand = g.state.zone(Zone::hand(ME)).len();
        let library = g.state.zone(Zone::library(ME)).len();
        announce(&mut g, spell, ActionKind::CastSpell);
        resolve_asking(&mut g, &mut |_, kind| match kind {
            RequestKind::SearchLibrary { library, .. } => Answer::Cards(vec![library[0]]),
            other => panic!("unexpected question {other:?}"),
        });
        assert_eq!(g.state.zone(Zone::hand(ME)).len(), hand);
        assert_eq!(g.state.zone(Zone::library(ME)).len(), library - 1);
    }
    #[test]
    fn replacement_makes_a_land_enter_tapped() {
        let (mut g, land, _) = game("Azorius Guildgate", None);
        announce(&mut g, land, ActionKind::PlayLand);
        g.run();
        let gate = permanents_named(&g, "Azorius Guildgate");
        assert_eq!(gate.len(), 1);
        assert!(g.state.obj(gate[0]).tapped);
    }
    #[test]
    fn sacrifice_cost_removes_the_chosen_creature() {
        let (mut g, _, seer, _) = game_with_extras(
            "Divination",
            Some("Viscera Seer"),
            0,
            &[("Grizzly Bears", ME)],
        );
        let id = permanents_named(&g, "Grizzly Bears")[0];
        announce(&mut g, seer.unwrap(), ActionKind::ActivateAbility);
        resolve_asking(&mut g, &mut |_, kind| match kind {
            RequestKind::ChooseCards { cards, .. } if cards.contains(&id) => {
                Answer::Cards(vec![id])
            }
            RequestKind::DistributeCards { cards, .. } => Answer::Placements {
                placements: vec![vec![], cards.clone()],
                shuffle: false,
            },
            other => panic!("unexpected question {other:?}"),
        });
        assert!(g.state.obj(id).became.is_some());
        assert_eq!(permanents_named(&g, "Viscera Seer").len(), 1);
    }
    #[test]
    fn equip_moves_the_bonus_to_the_equipped_creature() {
        let (mut g, _, splitter, _) = game_with_extras(
            "Divination",
            Some("Bonesplitter"),
            0,
            &[("Grizzly Bears", ME)],
        );
        let bears = permanents_named(&g, "Grizzly Bears")[0];
        announce(&mut g, splitter.unwrap(), ActionKind::ActivateAbility);
        resolve(&mut g, Some(bears), false);
        assert_eq!(g.chars(bears).power, Some(4));
        assert_eq!(g.chars(bears).toughness, Some(2));
    }
    #[test]
    fn cycling_discards_itself_and_draws_from_hand() {
        let (mut g, cave, _) = game("Forgotten Cave", None);
        let hand = g.state.zone(Zone::hand(ME)).len();
        announce(&mut g, cave, ActionKind::ActivateAbility);
        resolve(&mut g, None, false);
        assert_eq!(g.state.zone(Zone::hand(ME)).len(), hand);
        assert!(g.state.obj(cave).became.is_some());
    }
    #[test]
    fn damage_to_any_target_kills_a_creature() {
        let (mut g, spell, target) = game("Lightning Bolt", Some("Grizzly Bears"));
        let bears = target.unwrap();
        announce_at(&mut g, spell, ActionKind::CastSpell, Target::Object(bears));
        resolve(&mut g, None, false);
        assert!(g.state.obj(bears).became.is_some());
    }
    /// Pass until `done`, as far as the turn goes, declaring nothing.
    fn advance(g: &mut RulesGame, done: &dyn Fn(&RulesGame) -> bool) {
        advance_answering(g, done, &mut |_| None)
    }
    /// `advance`, with `answer` first in line for each question.
    fn advance_answering(
        g: &mut RulesGame,
        done: &dyn Fn(&RulesGame) -> bool,
        answer: &mut dyn FnMut(&RequestKind) -> Option<Answer>,
    ) {
        for _ in 0..512 {
            g.run();
            if done(g) {
                return;
            }
            let request = g.pending().cloned().expect("pending answer");
            if let Some(reply) = answer(&request.kind) {
                g.answer(request.id, request.player, reply).unwrap();
                continue;
            }
            let reply = match &request.kind {
                RequestKind::Priority { .. } => Answer::Pass,
                RequestKind::DeclareAttackers { .. } => Answer::Attack(vec![]),
                RequestKind::DeclareBlockers { .. } => Answer::Block(vec![]),
                RequestKind::DiscardToHandSize { .. } => Answer::Cards(vec![]),
                RequestKind::OrderTriggers { sources, .. } => {
                    Answer::Order((0..sources.len() as u32).collect())
                }
                other => panic!("unexpected question {other:?}"),
            };
            g.answer(request.id, request.player, reply).unwrap();
        }
        panic!("the game did not get there");
    }
    fn hand_card(g: &RulesGame, name: &str) -> ObjectId {
        let card = g.cards.find(name).unwrap();
        *g.state
            .zone(Zone::hand(ME))
            .iter()
            .find(|&&id| g.state.obj(id).card == card)
            .unwrap()
    }
    fn offers(g: &RulesGame, kind: ActionKind, source: ObjectId) -> bool {
        let RequestKind::Priority { options } = &g.pending().unwrap().kind else {
            panic!("{:?}", g.pending())
        };
        options
            .iter()
            .any(|o| o.kind == kind && o.source == Some(source))
    }
    #[test]
    fn a_static_cost_reduction_is_taken_off_what_is_paid() {
        let (mut g, spell, _) = game("Divination", Some("Arcane Melee"));
        let before: u32 = g.state.player(ME).pool.0.iter().sum();
        announce(&mut g, spell, ActionKind::CastSpell);
        resolve(&mut g, None, false);
        let after: u32 = g.state.player(ME).pool.0.iter().sum();
        assert_eq!(before - after, 1);
    }
    #[test]
    fn a_player_rule_stops_life_gain_for_everyone() {
        let (mut g, spell, _) = game("Blessed Wine", Some("Giant Cindermaw"));
        let life = g.state.player(ME).life;
        announce(&mut g, spell, ActionKind::CastSpell);
        resolve(&mut g, None, false);
        assert_eq!(g.state.player(ME).life, life);
    }
    #[test]
    fn a_land_in_the_graveyard_can_be_played_by_a_permission() {
        let (mut g, _, _, _) = game_with_graveyard(
            "Divination",
            Some("Crucible of Worlds"),
            0,
            &[],
            100,
            &["Azorius Guildgate"],
        );
        let land = g.state.zone(Zone::graveyard(ME))[0];
        assert!(offers(&g, ActionKind::PlayLand, land));
        announce(&mut g, land, ActionKind::PlayLand);
        g.run();
        assert_eq!(permanents_named(&g, "Azorius Guildgate").len(), 1);
    }
    #[test]
    fn a_shield_prevents_only_the_damage_it_has_left() {
        let (mut g, spell, bears, _) = game_with_zones(
            "Mending Hands",
            Some("Grizzly Bears"),
            0,
            &[],
            100,
            &[],
            &["Lightning Bolt", "Lightning Bolt"],
        );
        let bears = bears.unwrap();
        announce_at(&mut g, spell, ActionKind::CastSpell, Target::Object(bears));
        resolve(&mut g, None, false);
        let bolt = hand_card(&g, "Lightning Bolt");
        announce_at(&mut g, bolt, ActionKind::CastSpell, Target::Object(bears));
        resolve(&mut g, None, false);
        assert_eq!(g.state.obj(bears).damage, 0);
        let bolt = hand_card(&g, "Lightning Bolt");
        announce_at(&mut g, bolt, ActionKind::CastSpell, Target::Object(bears));
        resolve(&mut g, None, false);
        assert!(permanents_named(&g, "Grizzly Bears").is_empty());
    }
    #[test]
    fn flashback_casts_from_the_graveyard_and_exiles_after() {
        let (mut g, _, _, _) =
            game_with_graveyard("Divination", None, 0, &[], 100, &["Faithless Looting"]);
        let spell = g.state.zone(Zone::graveyard(ME))[0];
        let hand = g.state.zone(Zone::hand(ME)).len();
        announce(&mut g, spell, ActionKind::CastSpell);
        resolve_asking(&mut g, &mut |_, kind| match kind {
            RequestKind::ChooseCards { cards, .. } => Answer::Cards(cards[..2].to_vec()),
            other => panic!("unexpected question {other:?}"),
        });
        assert_eq!(g.state.zone(Zone::hand(ME)).len(), hand);
        assert!(g
            .state
            .zone(Zone::graveyard(ME))
            .iter()
            .all(|&id| g.state.obj(id).card != g.cards.find("Faithless Looting").unwrap()));
        assert_eq!(g.state.zone(Zone::exile(ME)).len(), 1);
    }
    #[test]
    fn an_additional_cost_is_paid_as_the_spell_is_cast() {
        let (mut g, spell, bears) = game("Altar's Reap", Some("Grizzly Bears"));
        let hand = g.state.zone(Zone::hand(ME)).len();
        let bears = bears.unwrap();
        announce(&mut g, spell, ActionKind::CastSpell);
        resolve_asking(&mut g, &mut |_, kind| match kind {
            RequestKind::ChooseCards { cards, .. } => {
                assert_eq!(cards, &vec![bears]);
                Answer::Cards(vec![bears])
            }
            other => panic!("unexpected question {other:?}"),
        });
        assert!(g.state.obj(bears).became.is_some());
        assert_eq!(g.state.zone(Zone::hand(ME)).len(), hand - 1 + 2);
    }
    #[test]
    fn a_painland_hurts_for_colored_mana_only() {
        let (mut g, _, wastes) = game("Divination", Some("Adarkar Wastes"));
        let wastes = wastes.unwrap();
        let life = g.state.player(ME).life;
        let request = g.pending().cloned().unwrap();
        let RequestKind::Priority { options } = &request.kind else {
            panic!()
        };
        let abilities: Vec<_> = options
            .iter()
            .filter(|o| o.source == Some(wastes) && o.kind == ActionKind::ActivateAbility)
            .collect();
        assert!(abilities.len() >= 2, "{options:?}");
        let mut action = Action::of(abilities[abilities.len() - 1]);
        action.color = Some(Color::White);
        g.answer(request.id, request.player, Answer::Act(action))
            .unwrap();
        finish_payment(&mut g, true);
        assert_eq!(g.state.player(ME).life, life - 1);
    }
    #[test]
    fn a_fastland_counts_the_other_lands_as_it_enters() {
        for (lands, tapped) in [(2, false), (3, true)] {
            let extras = vec![("Azorius Guildgate", ME); lands];
            let (mut g, land, _, _) = game_with_extras("Blackcleave Cliffs", None, 0, &extras);
            announce(&mut g, land, ActionKind::PlayLand);
            g.run();
            let cliffs = permanents_named(&g, "Blackcleave Cliffs");
            assert_eq!(g.state.obj(cliffs[0]).tapped, tapped);
        }
    }
    #[test]
    fn impulse_lets_the_exiled_cards_be_played_this_turn() {
        let (mut g, spell, _) = game("Act on Impulse", None);
        announce(&mut g, spell, ActionKind::CastSpell);
        resolve(&mut g, None, false);
        let exiled = g.state.zone(Zone::exile(ME)).to_vec();
        assert_eq!(exiled.len(), 3);
        assert_eq!(g.state.permissions.len(), 3);
    }
    #[test]
    fn a_delayed_ability_brings_back_what_its_maker_exiled() {
        let (mut g, spell, bears) = game("Ghostway", Some("Grizzly Bears"));
        let bears = bears.unwrap();
        announce(&mut g, spell, ActionKind::CastSpell);
        resolve(&mut g, None, false);
        assert!(g.state.obj(bears).became.is_some());
        assert!(permanents_named(&g, "Grizzly Bears").is_empty());
        advance(&mut g, &|g| {
            !permanents_named(g, "Grizzly Bears").is_empty()
        });
        assert_eq!(g.state.turn.step(), Step::End);
    }
    #[test]
    fn a_delayed_draw_waits_for_the_next_upkeep() {
        let (mut g, spell, _) = game("Blessed Wine", None);
        let life = g.state.player(ME).life;
        let hand = g.state.zone(Zone::hand(ME)).len();
        announce(&mut g, spell, ActionKind::CastSpell);
        resolve(&mut g, None, false);
        assert_eq!(g.state.player(ME).life, life + 1);
        assert_eq!(g.state.zone(Zone::hand(ME)).len(), hand - 1);
        advance(&mut g, &|g| g.state.zone(Zone::hand(ME)).len() == hand);
        assert_eq!(g.state.turn.step(), Step::Upkeep);
        assert_eq!(g.state.turn.active, THEM);
    }
    fn blink(kicked: u32) -> (usize, usize) {
        let (mut g, spell, bears) = game("Blink of an Eye", Some("Grizzly Bears"));
        let hand = g.state.zone(Zone::hand(ME)).len();
        let bears = bears.unwrap();
        announce(&mut g, spell, ActionKind::CastSpell);
        if kicked > 0 {
            g.run();
            let request = g.pending().cloned().unwrap();
            assert!(
                matches!(request.kind, RequestKind::PayCosts { .. }),
                "{:?}",
                request.kind
            );
            g.answer(request.id, request.player, Answer::Announce(kicked))
                .unwrap();
        }
        resolve_asking(&mut g, &mut |_, kind| match kind {
            RequestKind::ChooseTarget { .. } => Answer::Targets(vec![Target::Object(bears)]),
            other => panic!("unexpected question {other:?}"),
        });
        assert!(g.state.obj(bears).became.is_some());
        (hand, g.state.zone(Zone::hand(ME)).len())
    }
    #[test]
    fn a_kicked_branch_is_not_taken_unless_the_kicker_is_paid() {
        // The bounced Bears replaces the spell; the kicker's draw is on top.
        let (before, after) = blink(0);
        assert_eq!(after, before);
        let (before, after) = blink(1);
        assert_eq!(after, before + 1);
    }
    fn top_of(g: &RulesGame, count: usize) -> Vec<ObjectId> {
        g.state.zone(Zone::library(ME))[..count].to_vec()
    }
    #[test]
    fn cards_looked_at_go_back_in_the_chosen_order() {
        let (mut g, spell, _) = game("Index", None);
        let top = top_of(&g, 5);
        let mut reversed = top.clone();
        reversed.reverse();
        let chosen = reversed.clone();
        announce(&mut g, spell, ActionKind::CastSpell);
        resolve_asking(&mut g, &mut |_, kind| match kind {
            RequestKind::DistributeCards { cards, .. } => {
                assert_eq!(cards, &top);
                Answer::Placements {
                    placements: vec![chosen.clone()],
                    shuffle: false,
                }
            }
            other => panic!("unexpected question {other:?}"),
        });
        assert_eq!(top_of(&g, 5), reversed);
    }
    #[test]
    fn a_tutored_card_stays_on_top_of_the_shuffle() {
        let (mut g, spell, _) = game("Imperial Seal", None);
        let life = g.state.player(ME).life;
        let mut found = None;
        announce(&mut g, spell, ActionKind::CastSpell);
        resolve_asking(&mut g, &mut |_, kind| match kind {
            RequestKind::SearchLibrary { library, .. } => {
                let card = *library.last().unwrap();
                found = Some(card);
                Answer::Cards(vec![card])
            }
            other => panic!("unexpected question {other:?}"),
        });
        assert_eq!(top_of(&g, 1), vec![found.unwrap()]);
        assert_eq!(g.state.player(ME).life, life - 2);
    }
    #[test]
    fn the_rest_of_a_look_goes_to_the_bottom_in_order() {
        let (mut g, spell, _) = game("Impulse", None);
        let top = top_of(&g, 4);
        let hand = g.state.zone(Zone::hand(ME)).len();
        announce(&mut g, spell, ActionKind::CastSpell);
        let rest: Vec<ObjectId> = top[1..].iter().rev().copied().collect();
        let order = rest.clone();
        resolve_asking(&mut g, &mut |_, kind| match kind {
            RequestKind::ChooseCards { cards, .. } => {
                assert_eq!(cards, &top);
                Answer::Cards(vec![top[0]])
            }
            RequestKind::DistributeCards { cards, .. } => {
                assert_eq!(cards.len(), 3, "what was taken is not in the rest");
                Answer::Placements {
                    placements: vec![order.clone()],
                    shuffle: false,
                }
            }
            other => panic!("unexpected question {other:?}"),
        });
        assert_eq!(g.state.zone(Zone::hand(ME)).len(), hand);
        let library = g.state.zone(Zone::library(ME));
        assert_eq!(library[library.len() - 3..], rest);
    }
    #[test]
    fn nothing_to_take_still_sends_the_look_to_the_bottom() {
        let (mut g, spell, _) = game("Ancient Stirrings", None);
        let top = top_of(&g, 5);
        announce(&mut g, spell, ActionKind::CastSpell);
        resolve_asking(&mut g, &mut |_, kind| match kind {
            RequestKind::ChooseCards { .. } => Answer::Cards(vec![]),
            RequestKind::DistributeCards { cards, .. } => Answer::Placements {
                placements: vec![cards.clone()],
                shuffle: false,
            },
            other => panic!("unexpected question {other:?}"),
        });
        let library = g.state.zone(Zone::library(ME));
        assert_eq!(library[library.len() - 5..], top);
    }
    #[test]
    fn a_reflexive_ability_runs_when_the_may_was_done() {
        for accept in [true, false] {
            let (mut g, spell, bears) = game("Lithobraking", Some("Grizzly Bears"));
            let bears = bears.unwrap();
            announce(&mut g, spell, ActionKind::CastSpell);
            resolve_asking(&mut g, &mut |_, kind| match kind {
                RequestKind::May { .. } if accept => Answer::Yes,
                RequestKind::May { .. } => Answer::No,
                RequestKind::ChooseCards { cards, .. } => Answer::Cards(vec![cards[0]]),
                other => panic!("unexpected question {other:?}"),
            });
            assert_eq!(g.state.obj(bears).became.is_some(), accept);
        }
    }
    #[test]
    fn a_spell_that_targets_this_is_seen_as_it_is_cast() {
        for at_agent in [true, false] {
            let (mut g, bolt, agent, _) = game_with_extras(
                "Lightning Bolt",
                Some("Agent of the Fates"),
                0,
                &[("Grizzly Bears", THEM), ("Grizzly Bears", ME)],
            );
            let bears = permanents_named(&g, "Grizzly Bears");
            let theirs = *bears
                .iter()
                .find(|&&id| g.state.obj(id).owner == THEM)
                .unwrap();
            let mine = *bears
                .iter()
                .find(|&&id| g.state.obj(id).owner == ME)
                .unwrap();
            let target = if at_agent { agent.unwrap() } else { mine };
            announce_at(&mut g, bolt, ActionKind::CastSpell, Target::Object(target));
            resolve_asking(&mut g, &mut |_, kind| match kind {
                RequestKind::ChooseCards { cards, .. } => Answer::Cards(vec![cards[0]]),
                other => panic!("unexpected question {other:?}"),
            });
            assert_eq!(g.state.obj(theirs).became.is_some(), at_agent);
        }
    }
    #[test]
    fn what_is_exiled_comes_back_when_its_jailer_leaves() {
        let (mut g, priest, _, _) = game_with_zones(
            "Banisher Priest",
            None,
            0,
            &[("Grizzly Bears", THEM)],
            100,
            &[],
            &["Doom Blade"],
        );
        let bears = permanents_named(&g, "Grizzly Bears")[0];
        announce(&mut g, priest, ActionKind::CastSpell);
        resolve_asking(&mut g, &mut |_, kind| match kind {
            RequestKind::ChooseTarget { .. } => Answer::Targets(vec![Target::Object(bears)]),
            other => panic!("unexpected question {other:?}"),
        });
        assert!(permanents_named(&g, "Grizzly Bears").is_empty());
        let blade = hand_card(&g, "Doom Blade");
        let jailer = permanents_named(&g, "Banisher Priest")[0];
        announce_at(&mut g, blade, ActionKind::CastSpell, Target::Object(jailer));
        resolve(&mut g, None, false);
        assert_eq!(permanents_named(&g, "Grizzly Bears").len(), 1);
    }
    #[test]
    fn a_damage_trigger_reads_how_much() {
        let (mut g, bolt, spitemare) = game("Lightning Bolt", Some("Spitemare"));
        let spitemare = spitemare.unwrap();
        let life = g.state.player(THEM).life;
        announce_at(
            &mut g,
            bolt,
            ActionKind::CastSpell,
            Target::Object(spitemare),
        );
        resolve_asking(&mut g, &mut |_, kind| match kind {
            RequestKind::ChooseTarget { .. } => Answer::Targets(vec![Target::Player(THEM)]),
            other => panic!("unexpected question {other:?}"),
        });
        assert_eq!(g.state.player(THEM).life, life - 3);
    }
    #[test]
    fn a_cycling_trigger_sees_the_card_where_it_ended_up() {
        let (mut g, card, _) = game("Renewed Faith", None);
        let life = g.state.player(ME).life;
        announce(&mut g, card, ActionKind::ActivateAbility);
        resolve_asking(&mut g, &mut |_, kind| match kind {
            RequestKind::May { .. } => Answer::Yes,
            other => panic!("unexpected question {other:?}"),
        });
        assert_eq!(g.state.player(ME).life, life + 2);
    }
    #[test]
    fn a_sacrificed_creature_is_read_as_it_was() {
        let (mut g, _, good, _) = game_with_extras(
            "Divination",
            Some("Greater Good"),
            0,
            &[("Grizzly Bears", ME)],
        );
        let good = good.unwrap();
        let bears = permanents_named(&g, "Grizzly Bears")[0];
        let library = g.state.zone(Zone::library(ME)).len();
        announce(&mut g, good, ActionKind::ActivateAbility);
        resolve_asking(&mut g, &mut |_, kind| match kind {
            RequestKind::ChooseCards { purpose, .. } if *purpose == Purpose::SacrificeCost => {
                Answer::Cards(vec![bears])
            }
            RequestKind::ChooseCards { cards, .. } => Answer::Cards(cards[..3].to_vec()),
            other => panic!("unexpected question {other:?}"),
        });
        assert_eq!(g.state.zone(Zone::library(ME)).len(), library - 2);
    }
    #[test]
    fn a_spell_reads_what_it_sacrificed() {
        let (mut g, fling, bears) = game("Fling", Some("Grizzly Bears"));
        let bears = bears.unwrap();
        let life = g.state.player(THEM).life;
        announce_at(&mut g, fling, ActionKind::CastSpell, Target::Player(THEM));
        resolve_asking(&mut g, &mut |_, kind| match kind {
            RequestKind::ChooseCards { .. } => Answer::Cards(vec![bears]),
            other => panic!("unexpected question {other:?}"),
        });
        assert_eq!(g.state.player(THEM).life, life - 2);
    }
    #[test]
    fn converge_counts_the_colors_spent() {
        let (mut g, spell, _) = game("Painful Truths", None);
        let life = g.state.player(ME).life;
        let hand = g.state.zone(Zone::hand(ME)).len();
        announce(&mut g, spell, ActionKind::CastSpell);
        resolve(&mut g, None, false);
        let colors = life - g.state.player(ME).life;
        assert!(colors >= 1, "at least the black spent");
        assert_eq!(
            g.state.zone(Zone::hand(ME)).len() as i32,
            hand as i32 - 1 + colors
        );
    }
    #[test]
    fn another_target_is_not_the_one_already_chosen() {
        let (mut g, spell, _, _) = game_with_extras(
            "Consume Strength",
            None,
            0,
            &[("Grizzly Bears", ME), ("Grizzly Bears", THEM)],
        );
        let bears = permanents_named(&g, "Grizzly Bears");
        announce(&mut g, spell, ActionKind::CastSpell);
        resolve_asking(&mut g, &mut |_, kind| match kind {
            RequestKind::ChooseTarget { candidates, .. } => {
                if candidates.len() == 1 {
                    assert!(!candidates.contains(&Target::Object(bears[0])));
                    Answer::Targets(vec![candidates[0]])
                } else {
                    Answer::Targets(vec![Target::Object(bears[0])])
                }
            }
            other => panic!("unexpected question {other:?}"),
        });
        assert_eq!(g.state.obj(bears[1]).became.is_some(), true);
    }
    #[test]
    fn an_action_stands_in_for_a_cost_if_they_take_it() {
        for accept in [true, false] {
            let (mut g, spell, _, _) =
                game_with_extras("Blazing Salvo", None, 0, &[("Spitemare", THEM)]);
            let creature = permanents_named(&g, "Spitemare")[0];
            let life = g.state.player(THEM).life;
            announce_at(
                &mut g,
                spell,
                ActionKind::CastSpell,
                Target::Object(creature),
            );
            resolve_asking(&mut g, &mut |_, kind| match kind {
                RequestKind::May { .. } if accept => Answer::Yes,
                RequestKind::May { .. } => Answer::No,
                RequestKind::ChooseTarget { .. } => Answer::Targets(vec![Target::Player(ME)]),
                other => panic!("unexpected question {other:?}"),
            });
            let (lost, dealt) = if accept { (5, 0) } else { (0, 3) };
            assert_eq!(g.state.player(THEM).life, life - lost);
            assert_eq!(g.state.obj(creature).damage, dealt);
        }
    }
    #[test]
    fn being_blocked_by_two_creatures_triggers_once() {
        let (mut g, _, golem, _) = game_with_extras(
            "Divination",
            Some("Flint Golem"),
            0,
            &[("Grizzly Bears", THEM), ("Spitemare", THEM)],
        );
        let golem = golem.unwrap();
        let blockers = [
            permanents_named(&g, "Grizzly Bears")[0],
            permanents_named(&g, "Spitemare")[0],
        ];
        let library = g.state.zone(Zone::library(THEM)).len();
        advance_answering(
            &mut g,
            &|g| g.state.zone(Zone::library(THEM)).len() < library,
            &mut |kind| match kind {
                RequestKind::DeclareAttackers { .. } => {
                    Some(Answer::Attack(vec![(golem, Target::Player(THEM))]))
                }
                RequestKind::DeclareBlockers { .. } => Some(Answer::Block(
                    blockers.iter().map(|&b| (b, golem)).collect(),
                )),
                _ => None,
            },
        );
        assert_eq!(g.state.zone(Zone::library(THEM)).len(), library - 3);
        assert!(g.state.stack.is_empty());
    }
}
