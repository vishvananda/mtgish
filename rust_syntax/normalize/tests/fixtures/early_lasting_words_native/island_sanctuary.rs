use cantrip_api::*;


static INSTEAD_0: Question = Question::new(
    "Instead",
    "Skip this draw to restrict which creatures can attack you until your next turn?",
    "Draw the card as normal.",
    "Skip the draw",
    "Draw",
);

/// "If you would draw a card during your draw step, instead you may skip that draw. If you do, until your next turn, you can't be attacked except by creatures with flying and/or islandwalk." (CR 614.1a).
fn if_would_draw_card_during_applies(
    q: &dyn Query,
    asker: &Asker,
    proposal: &Proposal,
) -> bool {
    let Proposal::Draw(d) = proposal else { return false };
    if q.state().turn.step() == Step::Draw && q.state().turn.active == d.player
        && asker.controller == d.player
    {
        return true;
    }
    false
}

/// "If you would draw a card during your draw step, instead you may skip that draw. If you do, until your next turn, you can't be attacked except by creatures with flying and/or islandwalk." (CR 614.1a).
fn if_would_draw_card_during_apply(_: &dyn Query, _: &Asker, proposal: &mut Proposal) {
    let seen = *proposal;
    let Proposal::Draw(_) = &seen else { return };
    let Proposal::Draw(d) = proposal else { return };
    d.replaced = true;
}

/// "If you would draw a card during your draw step, instead you may skip that draw. If you do, until your next turn, you can't be attacked except by creatures with flying and/or islandwalk." (CR 611.2c).
fn permitted_attackers_0(
    q: &dyn Query,
    asker: &Asker,
    _: Target,
    object: ObjectId,
) -> bool {
    !((q.chars(object).is(CardType::Creature)
        && (q.chars(object).keywords.has(Keyword::Flying)
            || Keyword::landwalk(q.words(asker.source).subtype(Subtype::Island))
                .is_some_and(|walk| q.chars(object).keywords.matches(walk)))))
}

/// "If you would draw a card during your draw step, instead you may skip that draw. If you do, until your next turn, you can't be attacked except by creatures with flying and/or islandwalk." (CR 614.1a).
fn if_would_draw_card_during_then_steps(
    g: &mut dyn Game,
    asker: &Asker,
    _: Target,
    _: [u32; 2],
) -> Result<(), Suspend> {
    instruction(
        g,
        |g| {
            let rule_card_1_0 = g.card_id(&CARD);
            g.add_player_effect(
                asker.controller,
                asker.source,
                EffectKind::LastingRule {
                    card: rule_card_1_0,
                    index: 0,
                    about: Target::Player(asker.controller),
                },
                Duration::UntilNextTurnOf(asker.controller),
            );
        },
    );
    Ok(())
}

/// "If you would draw a card during your draw step, instead you may skip that draw. If you do, until your next turn, you can't be attacked except by creatures with flying and/or islandwalk." (CR 614.1a).
fn if_would_draw_card_during_then(
    g: &mut dyn Game,
    asker: &Asker,
    before: &Proposal,
    _: &Proposal,
) {
    let Proposal::Draw(d) = before else { return };
    let subject = Target::Player(d.player);
    let data = [1u32, u32::MAX];
    let _ = if_would_draw_card_during_then_steps(g, asker, subject, data);
}

pub static CARD: CardDef = CardDef {
    replacements: &[
        replacement(
                ReplacementKind::Draw,
                if_would_draw_card_during_applies,
                if_would_draw_card_during_apply,
                "If you would draw a card during your draw step, instead you may skip that draw. If you do, until your next turn, you can't be attacked except by creatures with flying and/or islandwalk.",
            )
            .then(if_would_draw_card_during_then)
            .optional(&INSTEAD_0),
    ],
    lasting_rules: &[
        LastingRule {
            text: "If you would draw a card during your draw step, instead you may skip that draw. If you do, until your next turn, you can't be attacked except by creatures with flying and/or islandwalk.",
            rule: LastingRuleKind::CantAttackYou,
            applies: permitted_attackers_0,
        },
    ],
    ..PRINTED
};

const PRINTED: CardDef = CardDef { name: "Island Sanctuary", token_words: &[], oracle: true, cost: ManaCost { generic: 1, x: 0, symbols: [1, 0, 0, 0, 0, 0], hybrid: [None,None,None,None,None,None], twobrid: [0, 0, 0, 0, 0], phyrexian: [0, 0, 0, 0, 0], snow: 0, ..ManaCost::ZERO }, has_cost: true, types: TypeSet::of(&[CardType::Enchantment]), subtypes: SubtypeSet::of(&[]), supertypes: Supertypes { basic: false, legendary: false, snow: false, world: false }, colors: None, power: None, toughness: None, loyalty: None, defense: None, keywords: Keywords::of(&[]), protection: ColorSet(0), ..CardDef::BLANK };
