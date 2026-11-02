use cantrip_api::*;
use cantrip_oracle as oracle;

/// "Creatures without flying can't block this turn." (CR 611.2c).
fn lasting_rule_0(q: &dyn Query, _: &Asker, _: Target, id: ObjectId) -> bool {
    q.chars(id).is(CardType::Creature) && !(q.chars(id).keywords.has(Keyword::Flying))
}

/// "Creatures without flying can't block this turn." (CR 608.2, 611.2c).
fn creatures_without_flying_cant_block(
    g: &mut dyn Game,
    item: &StackItem,
) -> Result<(), Suspend> {
    g.add_lasting_rule(item, 0, Target::None, Duration::EndOfTurn);
    Ok(())
}

pub static CARD: CardDef = CardDef {
    spell: spell(&[], creatures_without_flying_cant_block),
    lasting_rules: &[
        LastingRule {
            text: "Creatures without flying can't block this turn.",
            rule: LastingRuleKind::CantBlock,
            applies: lasting_rule_0,
        },
    ],
    ..oracle::FALTER
};
