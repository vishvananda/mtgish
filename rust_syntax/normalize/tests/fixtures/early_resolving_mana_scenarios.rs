use cantrip_api::Game as Rules;
use cantrip_engine::*;
use cantrip_script::{run_on, GameScript};
use std::sync::OnceLock;

fn simulate(g: &mut dyn Rules, item: &StackItem) -> Result<(), Suspend> {
    g.add_counters(item.object, CounterKind::Music, 256);
    assert_eq!(g.state().obj(item.object).counters(CounterKind::Music), 256);
    g.add_mana(item.controller, Mana::C, 255);
    let AbilityDef::Triggered(def) = cantrip_cards::all()
        .into_iter()
        .find(|card| card.name == "Musician")
        .unwrap()
        .delayed[0]
    else {
        panic!("Musician upkeep trigger")
    };
    (def.effect)(g, item)
}
static PROBE: CardDef = CardDef {
    name: "Music Payment Probe",
    oracle: true,
    has_cost: true,
    types: TypeSet::of(&[CardType::Sorcery]),
    spell: spell(&[], simulate),
    ..CardDef::BLANK
};
static FILLER: CardDef = CardDef {
    name: "Probe Land",
    oracle: true,
    types: TypeSet::of(&[CardType::Land]),
    ..CardDef::BLANK
};
#[test]
fn music_256_counters_cannot_be_paid_with_255_mana() {
    static TABLE: OnceLock<CardTable> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        CardTable::new(
            [&PROBE, &FILLER].into_iter().chain(
                cantrip_cards::all()
                    .into_iter()
                    .filter(|card| card.name == "Musician"),
            ),
        )
    });
    let text = "[Bot1 Deck]\nMusic Payment Probe\nProbe Land x19\n[Bot2 Deck]\nProbe Land x20\n[Actions]\nBot1.Hand.Music Payment Probe.Cast\nBot1.Pass\nBot2.Pass\n! Bot1.PayCost\nBot1.Decline\n[T1 Bot1.Main2]\n[Expect]\nBot1.Life = 20\n";
    run_on(
        &GameScript::parse(text, "music-payment-probe").unwrap(),
        table,
    )
    .unwrap();
}
