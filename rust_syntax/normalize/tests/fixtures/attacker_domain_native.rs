//! The callback below is emitted by the generator, not handwritten for this test.
#![allow(dead_code, unused_imports)]
use cantrip_api::*;
use cantrip_rules::{Game as Engine, Setup};

__GENERATED_CALLBACK__

fn board() -> (Engine, Asker, Vec<ObjectId>) {
    static CREATURE: CardDef = CardDef { name: "Ordinary creature", types: TypeSet::of(&[CardType::Creature]), power: Some(2), toughness: Some(2), ..CardDef::BLANK };
    static FLYER: CardDef = CardDef { name: "Flying creature", keywords: Keywords::of(&[Keyword::Flying]), ..CREATURE };
    static ISLAND: CardDef = CardDef { name: "Islandwalking creature", keywords: Keywords::of(&[Keyword::Islandwalk]), ..CREATURE };
    static SWAMP: CardDef = CardDef { name: "Swampwalking creature", keywords: Keywords::of(&[Keyword::Swampwalk]), ..CREATURE };
    static LAND: CardDef = CardDef { name: "Ordinary land", types: TypeSet::of(&[CardType::Land]), ..CardDef::BLANK };
    static ARTIFACT: CardDef = CardDef { name: "Ordinary artifact", types: TypeSet::of(&[CardType::Artifact]), ..CardDef::BLANK };
    let defs = [&CREATURE, &FLYER, &ISLAND, &SWAMP, &LAND, &ARTIFACT];
    let cards = Box::leak(Box::new(CardTable::new(defs)));
    let mut game = Engine::new(cards, &Setup { decks: vec![vec![], vec![]], ..Default::default() });
    let ids = defs.into_iter().map(|def| {
        let id = game.state.push_obj(Obj::new(cards.id_of(def), PlayerId(0), Zone::BATTLEFIELD));
        game.state.zone_push(Zone::BATTLEFIELD, id);
        id
    }).collect::<Vec<_>>();
    let asker = Asker::new(ids[5], PlayerId(0));
    (game, asker, ids)
}

#[test]
fn noncreatures_are_outside_the_restriction() {
    let (game, asker, ids) = board();
    for &id in &ids[4..] {
        assert!(!__CALLBACK__(&game, &asker, Target::Player(PlayerId(0)), id), "{} is not a creature", game.name(id));
    }
}

#[test]
fn ordinary_creatures_are_restricted_and_only_named_exceptions_are_allowed() {
    let (game, asker, ids) = board();
    for (i, expected) in [(0, true), (1, false), (2, false), (3, true)] {
        assert_eq!(__CALLBACK__(&game, &asker, Target::Player(PlayerId(0)), ids[i]), expected, "{}", game.name(ids[i]));
    }
}
