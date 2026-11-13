use cantrip_api::*;
fn all(_: &dyn Query, _: &Asker, _: PlayerId) -> bool { true }
pub static CARD: CardDef = CardDef { name: "Proof Skip Untap", oracle: true, has_cost: true,
    types: TypeSet::of(&[CardType::Enchantment]), skip_steps: &[SkipStep { applies: all, step: Step::Untap }], ..CardDef::BLANK };
