use cantrip_api::*;
use cantrip_mechanics::targets;
fn take(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
    if let Some(land) = targets::object(g, item, 0) {
        g.add_effect(item.object, land, EffectKind::Control(item.controller), Duration::Indefinite);
    }
    Ok(())
}
pub static CARD: CardDef = CardDef { name: "Proof Take Land", oracle: true, has_cost: true,
    types: TypeSet::of(&[CardType::Instant]), spell: spell(&[targets::LAND], take), ..CardDef::BLANK };
