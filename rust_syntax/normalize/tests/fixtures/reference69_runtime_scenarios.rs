use cantrip_api::*;
use cantrip_rules::{Game as RulesGame, Setup};
const A: PlayerId = PlayerId(0);
const B: PlayerId = PlayerId(1);
static FILLER: CardDef = CardDef {
    name: "Filler",
    types: TypeSet::of(&[CardType::Land]),
    ..CardDef::BLANK
};
static BEAR: CardDef = CardDef {
    name: "Bear",
    types: TypeSet::of(&[CardType::Creature]),
    power: Some(2),
    toughness: Some(2),
    ..CardDef::BLANK
};
static ARTIFACT_LAND: CardDef = CardDef {
    name: "Artifact land enchantment fixture",
    types: TypeSet::of(&[CardType::Artifact, CardType::Land, CardType::Enchantment]),
    cost: mana("{2}"),
    has_cost: true,
    ..CardDef::BLANK
};
fn no_draw(_: &dyn Query, _: &Asker, _: PlayerId, _: u32) -> bool {
    true
}
static DRAW_BAN: CardDef = CardDef {
    name: "Draw prohibition fixture",
    types: TypeSet::of(&[CardType::Enchantment]),
    cant_draw: &[no_draw],
    ..CardDef::BLANK
};
fn game(source: &'static CardDef, extra: &[&'static CardDef]) -> (RulesGame, ObjectId) {
    let mut defs = vec![&FILLER, &BEAR, &ARTIFACT_LAND, &DRAW_BAN, source];
    defs.extend(extra);
    let table = Box::leak(Box::new(CardTable::new(defs)));
    let fill = table.id_of(&FILLER);
    let mut mine = vec![fill; 30];
    mine[0] = table.id_of(source);
    let mut g = RulesGame::new(
        table,
        &Setup {
            decks: vec![mine, vec![fill; 30]],
            ..Default::default()
        },
    );
    g.state.update_turn(|t| {
        t.number = 1;
        t.active = A;
        t.priority = A;
        t.set_step(Step::PrecombatMain);
        t.began = true;
    });
    g.state.update_player(A, |p| p.turn_began = 1);
    for mana in Mana::ALL {
        g.state.add_mana(
            A,
            mana,
            100,
            ObjectId::NONE,
            None,
            Default::default(),
            false,
        );
    }
    let id = g.state.zone(Zone::hand(A))[0];
    (g, id)
}
fn put(g: &mut RulesGame, def: &'static CardDef, p: PlayerId, zone: Zone) -> ObjectId {
    let id = g.state.push_obj(Obj::new(g.cards.id_of(def), p, zone));
    g.state.zone_push(zone, id);
    id
}
fn announce(g: &mut RulesGame, id: ObjectId, kind: ActionKind, index: u8, targets: Vec<Target>) {
    g.state.see_statics();
    if g.pending().is_none() {
        g.state.frame_push(cantrip_state::Frame::new(
            cantrip_state::FrameKind::Priority,
        ));
    }
    g.run();
    let r = g.pending().cloned().expect("priority");
    let RequestKind::Priority { options } = &r.kind else {
        panic!("{:?}", r.kind)
    };
    let o = options
        .iter()
        .find(|o| o.source == Some(id) && o.kind == kind && o.index == index)
        .unwrap_or_else(|| panic!("missing action {kind:?}/{index} {options:?}"));
    let mut a = Action::of(o);
    a.targets = targets;
    g.answer(r.id, r.player, Answer::Act(a)).unwrap();
}
fn finish(g: &mut RulesGame, yes: bool) {
    finish_with_target(g, yes, None)
}
fn finish_with_target(g: &mut RulesGame, yes: bool, preferred: Option<Target>) {
    for _ in 0..200 {
        g.run();
        let Some(r) = g.pending().cloned() else {
            assert!(g.state.is_over());
            return;
        };
        let answer = match &r.kind {
            RequestKind::Priority { .. } if g.state.stack.is_empty() => return,
            RequestKind::Priority { .. } => Answer::Pass,
            RequestKind::PayCosts { options, .. } => {
                let o = options
                    .iter()
                    .find(|o| o.kind == ActionKind::AssignMana)
                    .or_else(|| {
                        options
                            .iter()
                            .find(|o| o.kind == ActionKind::ConfirmCasting)
                    })
                    .expect("payment option");
                Answer::Act(Action::of(o))
            }
            RequestKind::ChooseTarget { candidates, .. } => Answer::Targets(vec![
                preferred.filter(|t| candidates.contains(t)).unwrap_or(candidates[0]),
            ]),
            RequestKind::ChooseModes { .. } => Answer::Modes(vec![0]),
            RequestKind::ChooseCostPayment { .. } => Answer::CostPayment(0),
            RequestKind::May { .. } => {
                if yes {
                    Answer::Yes
                } else {
                    Answer::No
                }
            }
            RequestKind::ChooseCards {
                cards,
                rule,
                purpose,
                ..
            } => {
                let least = match rule.count {
                    cantrip_mechanics::select::Count::Exactly(n)
                    | cantrip_mechanics::select::Count::AsManyAsCan(n) => n.min(cards.len() as u32),
                    cantrip_mechanics::select::Count::UpTo(_)
                    | cantrip_mechanics::select::Count::AnyNumber => 0,
                    other => panic!("count {other:?}"),
                };
                {
                    let mut choices = cards.clone();
                    if *purpose == Purpose::Exile {
                        choices.sort_by_key(|&id| !g.chars(id).is(CardType::Creature));
                    }
                    Answer::Cards(choices.into_iter().take(least as usize).collect())
                }
            }
            RequestKind::OrderTriggers { sources, .. } => {
                Answer::Order((0..sources.len() as u32).collect())
            }
            other => panic!("unhandled {other:?}"),
        };
        g.answer(r.id, r.player, answer).unwrap();
    }
    panic!("did not finish")
}
fn pool_free_spell(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
    g.gain_life(item.controller, 7);
    Ok(())
}
static POOL_OFFER: CardDef = CardDef {
    name: "Pool offered spell fixture",
    types: TypeSet::of(&[CardType::Instant]),
    cost: ManaCost::ZERO,
    has_cost: true,
    spell: spell(&[], pool_free_spell),
    ..CardDef::BLANK
};
static POOL_HAND_SPELL: CardDef = CardDef {
    name: "Pool triggering spell fixture",
    ..POOL_OFFER
};
#[test]
fn pool_cast_choice_and_free_spell_belong_to_the_triggering_opponent() {
    for accept in [false, true] {
        let (mut g, _) = game(&knowledge_pool::CARD, &[&POOL_OFFER, &POOL_HAND_SPELL]);
        let pool = put(&mut g, &knowledge_pool::CARD, A, Zone::BATTLEFIELD);
        // Seed a legal, different card in Pool's linked exile, without invoking
        // its independent ETB trigger. The new hand spell must be excluded below.
        let offered = put(&mut g, &POOL_OFFER, A, Zone::exile(A));
        let link = g.text_linked(pool);
        g.state.update_obj(offered, |o| {
            o.exiled = Some(cantrip_state::ExileLink::new(link, Zone::hand(A), false))
        });
        let triggering = put(&mut g, &POOL_HAND_SPELL, B, Zone::hand(B));
        g.state.update_turn(|t| {
            t.active = B;
            t.priority = B;
        });
        g.state.update_player(B, |p| p.turn_began = 1);
        announce(&mut g, triggering, ActionKind::CastSpell, 0, vec![]);
        let mut choices = 0;
        for _ in 0..100 {
            g.run();
            let r = g.pending().cloned().expect("decision");
            let answer = match &r.kind {
                RequestKind::Priority { .. } if g.state.stack.is_empty() => break,
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
                        .expect("payment option");
                    Answer::Act(Action::of(option))
                }
                RequestKind::ChooseCards {
                    cards,
                    purpose: Purpose::CastFree,
                    ..
                } => {
                    choices += 1;
                    assert_eq!(r.player, B);
                    assert_eq!(cards.as_slice(), &[offered], "only other linked cards may be offered");
                    Answer::Cards(if accept { vec![offered] } else { vec![] })
                }
                RequestKind::May {
                    purpose: Purpose::CastFree,
                    ..
                } => {
                    choices += 1;
                    assert_eq!(r.player, B);
                    if accept {
                        Answer::Yes
                    } else {
                        Answer::No
                    }
                }
                RequestKind::OrderTriggers { sources, .. } => {
                    Answer::Order((0..sources.len() as u32).collect())
                }
                other => panic!("unhandled Pool decision {other:?}"),
            };
            g.answer(r.id, r.player, answer).unwrap();
        }
        assert_eq!(choices, 1);
        assert!(g.state.stack.is_empty());
        assert_eq!(g.state.player(A).life, 20);
        assert_eq!(g.state.player(B).life, if accept { 27 } else { 20 });
        assert!(g.state.obj(triggering).became.is_some());
        let cast: Vec<_> = g
            .history()
            .this_turn()
            .iter()
            .filter_map(|r| match r.event {
                Event::SpellCast { spell, player, .. }
                    if g.state.obj(spell).card == g.cards.id_of(&POOL_OFFER) =>
                {
                    Some(player)
                }
                _ => None,
            })
            .collect();
        assert_eq!(cast, if accept { vec![B] } else { vec![] });
    }
}

fn stone_target(q: &dyn Query, _: &Asker, t: Target) -> bool { t.object().is_some_and(|o| cantrip_mechanics::targets::permanent(q,o)) }
fn set_harnessed(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
    let object=g.state().target(item,0).object().unwrap();
    g.add_effect(object,object,EffectKind::Harnessed,Duration::Indefinite);
    Ok(())
}
fn sacrifice_stone(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
    let object=g.state().target(item,0).object().unwrap();
    g.sacrifice(item.controller,&[object]);
    Ok(())
}
static HARNESS_CALLBACK:CardDef=CardDef {
    name:"Harness presence callback",types:TypeSet::of(&[CardType::Instant]),
    has_cost:true,spell:spell(&[target(stone_target)],set_harnessed),
    ..CardDef::BLANK
};
static SACRIFICE_CALLBACK:CardDef=CardDef {
    name:"Sacrifice presence callback",spell:spell(&[target(stone_target)],sacrifice_stone),
    ..HARNESS_CALLBACK
};
#[test]
fn infinity_stones_gain_an_actual_ability_only_after_harnessing() {
    for stone in [&soul_stone::CARD, &mind_stone::CARD] {
        let (mut g, harness)=game(&HARNESS_CALLBACK,&[stone,&SACRIFICE_CALLBACK]);
        let sacrifice=put(&mut g,&SACRIFICE_CALLBACK,A,Zone::hand(A));
        let object=put(&mut g,stone,A,Zone::BATTLEFIELD);
        let other=put(&mut g,stone,B,Zone::BATTLEFIELD);
        g.state.see_statics();
        let count=|g:&RulesGame,object|g.chars(object).abilities()
            .filter(|a|matches!(g.cards.ability(*a),AbilityDef::Triggered(_))).count();
        assert_eq!(count(&g,object),0,"unharnessed {} has its infinity ability",stone.name);
        announce(&mut g,harness,ActionKind::CastSpell,0,vec![Target::Object(object)]);
        finish(&mut g,true);
        assert_eq!(count(&g,object),1,"harnessed {} lacks its infinity ability",stone.name);
        assert_eq!(count(&g,other),0,"harnessing leaked to another {}",stone.name);
        announce(&mut g,sacrifice,ActionKind::CastSpell,0,vec![Target::Object(object)]);
        finish(&mut g,true);
        let departed=g.state.obj(object).became.expect("new graveyard incarnation");
        assert!(g.state.obj(departed).zone.is(ZoneKind::Graveyard));
        assert_eq!(count(&g,departed),0,"off-battlefield {} retains its infinity ability",stone.name);
    }
}

fn regain_bombshell(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
    let object=g.state().target(item,0).object().unwrap();
    g.add_effect(item.object,object,EffectKind::Control(A),Duration::Indefinite);
    Ok(())
}
static REGAIN_BOMBSHELL:CardDef=CardDef {
    name:"Regain Bombshell response",spell:spell(&[target(stone_target)],regain_bombshell),
    ..HARNESS_CALLBACK
};
#[test]
fn bombshell_remembers_the_controller_when_it_triggered() {
    let (mut g,response)=game(&REGAIN_BOMBSHELL,&[&bombshell::CARD]);
    let object=put(&mut g,&bombshell::CARD,A,Zone::BATTLEFIELD);
    g.state.update_obj(object,|o|o.controller=B);
    // The state trigger is stacked before A receives priority for this response.
    announce(&mut g,response,ActionKind::CastSpell,0,vec![Target::Object(object)]);
    assert!(g.state.stack.iter().any(|item|item.object==object&&item.controller==B));
    finish(&mut g,true);
    assert!(g.state.obj(object).zone.is(ZoneKind::Battlefield));
    assert!(g.state.obj(object).became.is_none(), "Bombshell must remain on the battlefield after control changes");
    assert_eq!(g.controller(object),A);
    assert_eq!(g.state.player(A).life,20);
    assert_eq!(g.state.player(B).life,20);
}
