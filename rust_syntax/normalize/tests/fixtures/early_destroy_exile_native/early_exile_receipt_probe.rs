use cantrip_engine::*;
use cantrip_script::{GameScript, run_on};
use std::sync::OnceLock;

fn redirected(_: &dyn Query, _: &Asker, p: &Proposal) -> bool {
    matches!(p, Proposal::ZoneChange(z) if z.is_card && z.from.is(ZoneKind::Graveyard) && z.to.is(ZoneKind::Exile))
}
fn to_hand(q: &dyn Query, _: &Asker, p: &mut Proposal) {
    if let Proposal::ZoneChange(z) = p { z.to = Zone::hand(q.obj(z.object).owner); }
}
static REROUTER: CardDef = CardDef { name: "Probe Rerouter", oracle: true, has_cost: true, types: TypeSet::of(&[CardType::Enchantment]), replacements: &[replacement(ReplacementKind::ZoneChange, redirected, to_hand, "Send graveyard exile to hand")], ..CardDef::BLANK };
fn receipt_lki(g: &mut dyn cantrip_api::Game, item: &StackItem) -> Result<(), Suspend> {
    let target = g.state().zone(Zone::graveyard(item.controller)).iter().copied().find(|&id| g.chars(id).is(CardType::Creature)).unwrap();
    let arrival = g.exile_all(&[target])[0];
    assert!(g.obj(arrival).zone.is(ZoneKind::Exile));
    let new = g.return_to_hand(&[arrival])[0];
    assert!(g.obj(new).zone.is(ZoneKind::Hand));
    assert!(g.obj(arrival).became.is_some());
    assert!(g.obj(arrival).zone.is(ZoneKind::Exile));
    assert!(g.chars(arrival).is(CardType::Creature));
    Ok(())
}
static PROBE: CardDef = CardDef { name: "Probe Receipt", oracle: true, has_cost: true, types: TypeSet::of(&[CardType::Sorcery]), spell: spell(&[], receipt_lki), ..CardDef::BLANK };
fn table() -> &'static CardTable { static TABLE: OnceLock<CardTable> = OnceLock::new(); TABLE.get_or_init(|| CardTable::new(cantrip_cards::all().into_iter().chain([&REROUTER, &PROBE]))) }
fn play(name: &str, text: &str) { let script = GameScript::parse(text, name).unwrap(); if let Err(f) = run_on(&script, table()) { panic!("{}", f.report); } }
#[test]
fn redirect_must_not_count_as_exiled() {
    play("redirect", "[Bot1 Deck]\nSwamp\nEntomb\nBlack Lotus\nMox Jet\nFrankenstein's Monster\nProbe Rerouter\nSwamp\nGrizzly Bears\nSwamp x12\n[Bot2 Deck]\nIsland x20\n[Actions]\nBot1.Hand.Probe Rerouter.Cast\nBot1.Hand.Swamp.Play\nBot1.Battlefield.Swamp.Activate\nBot1.Hand.Entomb.Cast\nBot1.Search(Grizzly Bears)\nBot1.Hand.Black Lotus.Cast\nBot1.Hand.Mox Jet.Cast\nBot1.Battlefield.Black Lotus.Activate(Color: Black)\nBot1.Hand.Frankenstein's Monster.Cast(X: 1)\nBot1.Choose(Confirm X)\nBot1.Pass\n[Expect]\nExile.Count = 0\nBot1.Hand.Grizzly Bears.Colors = Green\nBot1.Graveyard = Frankenstein's Monster; Black Lotus; Entomb\n");
}
#[test]
fn arrival_object_retains_its_exile_zone_after_a_later_move() {
    play("lki", "[Bot1 Deck]\nSwamp\nEntomb\nProbe Receipt\nSwamp\nSwamp\nSwamp\nSwamp\nGrizzly Bears\nSwamp x12\n[Bot2 Deck]\nIsland x20\n[Actions]\nBot1.Hand.Swamp.Play\nBot1.Battlefield.Swamp.Activate\nBot1.Hand.Entomb.Cast\nBot1.Search(Grizzly Bears)\nBot1.Hand.Probe Receipt.Cast\nBot1.Pass\n[Expect]\nBot1.Hand.Grizzly Bears.Colors = Green\n");
}
