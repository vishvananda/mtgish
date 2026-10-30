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
        g.state.frame_push(Frame::new(FrameKind::Priority));
        g.run();
        g.verify = true;
        (g, source, target, lands)
    }
    fn announce(g: &mut RulesGame, source: ObjectId, kind: ActionKind) {
        announce_colored(g, source, kind, None);
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
}
