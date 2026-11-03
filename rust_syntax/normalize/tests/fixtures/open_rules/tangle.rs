use cantrip_api::*;
use cantrip_oracle as oracle;

/// "Prevent all combat damage that would be dealt this turn. Each attacking creature doesn't untap during its controller's next untap step." (CR 615.1).
fn made_1_applies(_: &dyn Query, _: &Asker, _: ObjectId, e: &DamageEvent) -> bool {
    {
        let combat = e.combat;
        if combat {
            return true;
        }
    }
    false
}

/// "Prevent all combat damage that would be dealt this turn. Each attacking creature doesn't untap during its controller's next untap step." (CR 608.2, 611.2a).
fn prevent_all_combat_damage_would(
    g: &mut dyn Game,
    item: &StackItem,
) -> Result<(), Suspend> {
    {
        let to = Some(Target::None);
        let about = to.and_then(|to: Target| to.object()).or(Some(item.object));
        if let (Some(to), Some(about)) = (to, about) {
            g.add_effect(
                item.object,
                about,
                EffectKind::Prevention {
                    card: item.ability.card,
                    index: 0,
                    to,
                    shield: Shield::All,
                },
                Duration::EndOfTurn,
            );
        }
    }
    g.end_sentence();
    let objects_2 = {
        let q: &dyn Query = g;
        q.state()
            .battlefield()
            .iter()
            .copied()
            .filter(|&id| {
                (q.state().attackers.iter().any(|a| a.id == id) || q.was_attacking(id))
                    && q.chars(id).is(CardType::Creature)
            })
            .collect::<Vec<ObjectId>>()
            .into_iter()
            .filter(|&o| q.state().obj(o).became.is_none())
            .collect::<Vec<ObjectId>>()
    };
    let kinds_2 = {
        let mut all = Vec::new();
        for &object in &objects_2 {
            all.push((object, EffectKind::DoesntUntap));
        }
        all
    };
    instruction(
        g,
        |g| {
            for &(object, kind) in &kinds_2 {
                g.add_effect(
                    item.object,
                    object,
                    kind,
                    Duration::DuringItsControllersNextUntapStep,
                );
            }
        },
    );
    Ok(())
}

pub static CARD: CardDef = CardDef {
    spell: spell(&[], prevent_all_combat_damage_would),
    granted_preventions: &[
        GrantedPreventionDef {
            hint: "Prevent all combat damage that would be dealt this turn.",
            damage: "damage",
            of_it: None,
            applies: made_1_applies,
            then: None,
            text: "Prevent all combat damage that would be dealt this turn.",
        },
    ],
    ..oracle::TANGLE
};
