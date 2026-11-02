use cantrip_api::*;
use cantrip_oracle as oracle;

/// "{T}: Each creature attacks this turn if able." (CR 611.2c).
fn lasting_rule_0(q: &dyn Query, _: &Asker, _: Target, id: ObjectId) -> bool {
    q.chars(id).is(CardType::Creature)
}

/// "{T}: Each creature attacks this turn if able." (CR 608.2, 611.2c).
fn creature_attacks_if_able(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
    g.add_lasting_rule(item, 0, Target::None, Duration::EndOfTurn);
    Ok(())
}

pub static CARD: CardDef = CardDef {
    abilities: &[
        ability(
            TAP,
            creature_attacks_if_able,
            "{T}: Each creature attacks this turn if able.",
        ),
    ],
    lasting_rules: &[
        LastingRule {
            text: "Each creature attacks this turn if able.",
            rule: LastingRuleKind::MustAttack,
            applies: lasting_rule_0,
        },
    ],
    ..oracle::GOBLIN_DIPLOMATS
};
