use cantrip_api::*;
use cantrip_oracle as oracle;

/// "Creatures and lands target opponent controls don't untap during their next untap step." (CR 115.1).
fn creatures_lands_target_opponent_controls_target_0(
    q: &dyn Query,
    asker: &Asker,
    t: Target,
) -> bool {
    matches!(
        t, Target::Player(p) if q.state().opponents(asker.controller).collect:: < Vec <
        PlayerId > > ().contains(& p)
    )
}

/// "Creatures and lands target opponent controls don't untap during their next untap step." (CR 611.2c).
fn lasting_rule_0(q: &dyn Query, _: &Asker, about: Target, id: ObjectId) -> bool {
    (q.chars(id).is(CardType::Creature) || q.chars(id).is(CardType::Land))
        && about.player() == Some(q.controller(id))
}

/// "Creatures and lands target opponent controls don't untap during their next untap step." (CR 608.2, 611.2c).
fn creatures_lands_target_opponent_controls(
    g: &mut dyn Game,
    item: &StackItem,
) -> Result<(), Suspend> {
    let t_0 = vec![g.state().target(item, 0)];
    let about_0 = t_0
        .get(0)
        .copied()
        .and_then(|t: Target| t.player())
        .map(Target::Player)
        .unwrap_or(Target::None);
    let Some(named_3) = t_0.get(0).copied().and_then(|t: Target| t.player()) else { return Ok(()) };
    let until_0 = Duration::DuringNextUntapStepOf(named_3);
    g.add_lasting_rule(item, 0, about_0, until_0);
    Ok(())
}

pub static CARD: CardDef = CardDef {
    spell: spell(
        &[target(creatures_lands_target_opponent_controls_target_0)],
        creatures_lands_target_opponent_controls,
    ),
    lasting_rules: &[
        LastingRule {
            text: "Creatures and lands target opponent controls don't untap during their next untap step.",
            rule: LastingRuleKind::DoesntUntap,
            applies: lasting_rule_0,
        },
    ],
    ..oracle::EXHAUSTION
};
