use cantrip_api::*;
use cantrip_mechanics::targets;


/// "{T}: Target creature can't be blocked by Walls this turn." (CR 604.1).
fn block_rule_0(
    q: &dyn Query,
    source: ObjectId,
    blocker: ObjectId,
    attacker: ObjectId,
) -> bool {
    let asker = &Asker::new(source, q.controller(source));
    !(attacker == source
        && q.chars(blocker).has_subtype(q.words(asker.source).subtype(Subtype::Wall)))
}

/// "{T}: Target creature can't be blocked by Walls this turn." (CR 608.2, 611.2a).
fn target_creature_cant_be_blocked(
    g: &mut dyn Game,
    item: &StackItem,
) -> Result<(), Suspend> {
    let objects_0 = targets::object(g, item, 0);
    let kind_0_0_0 = {
        let q: &dyn Query = g;
        EffectKind::BlockRestriction {
            card: {
                let o = q.state().obj(item.object);
                o.copied_card().unwrap_or(o.card)
            },
            index: 0,
        }
    };
    instruction(
        g,
        |g| {
            if let Some(object) = objects_0 {
                g.add_effect(item.object, object, kind_0_0_0, Duration::EndOfTurn);
            }
        },
    );
    Ok(())
}

pub static CARD: CardDef = CardDef {
    abilities: &[
        ability(
                TAP,
                target_creature_cant_be_blocked,
                "{T}: Target creature can't be blocked by Walls this turn.",
            )
            .targets(&[targets::CREATURE]),
    ],
    lasting_block_rules: &[
        LastingBlockRule {
            text: "Can't be blocked by Walls",
            allows: block_rule_0,
        },
    ],
    ..PRINTED
};

const PRINTED: CardDef = CardDef { name: "Tower of Coireall", token_words: &[], oracle: true, cost: ManaCost { generic: 2, x: 0, symbols: [0, 0, 0, 0, 0, 0], hybrid: [None,None,None,None,None,None], twobrid: [0, 0, 0, 0, 0], phyrexian: [0, 0, 0, 0, 0], snow: 0, ..ManaCost::ZERO }, has_cost: true, types: TypeSet::of(&[CardType::Artifact]), subtypes: SubtypeSet::of(&[]), supertypes: Supertypes { basic: false, legendary: false, snow: false, world: false }, colors: None, power: None, toughness: None, loyalty: None, defense: None, keywords: Keywords::of(&[]), protection: ColorSet(0), ..CardDef::BLANK };
