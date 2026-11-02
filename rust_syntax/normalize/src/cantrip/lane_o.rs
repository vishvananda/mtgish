//! Rules of the game, in the form they will have once Cantrip has them. A rule
//! on objects is `Rule`, on a player `PlayerRule`, and both are one word for the
//! static ability that holds them and the resolving effect that makes them for a
//! while: `object_rule(applies, rule)` and `player_rule(applies, rule)` in a
//! card's `statics`, `EffectKind::Rule(rule)` and `EffectKind::PlayerRule(rule)`
//! on the stack. Every operand is read here, as the tests and amounts it will be.
use super::*;

const M: &str = "cantrip_mechanics";

/// "Loses all other abilities and has this", as the two changes it is: they keep
/// their order in the ability layer, so what it has it keeps (CR 613.1f).
pub(super) fn in_one_effect(changes: &[Change]) -> Vec<Change> {
    let mut split = Vec::new();
    for change in changes {
        match change {
            Change::OnlyAbility(ability) => {
                split.push(Change::RemoveAllAbilities);
                split.push(grant_of(ability));
            }
            Change::OnlyAbilities(abilities) => {
                split.push(Change::RemoveAllAbilities);
                split.extend(abilities.iter().map(grant_of));
            }
            other => split.push(other.clone()),
        }
    }
    split
}

/// What is left of an ability a permanent keeps: a keyword is a change of its own.
fn grant_of(ability: &Ability) -> Change {
    match ability {
        Ability::Keyword(keyword) => Change::GrantKeyword(keyword.clone()),
        other => Change::GrantAbility(Box::new(other.clone())),
    }
}

/// The player a player rule is asked of, in a test of what they are doing.
fn players_card(id: &str) -> Candidate {
    Candidate {
        controller: "player".into(),
        ..Candidate::of(id)
    }
}

/// The `SpendAs` a rule names, in Cantrip's words.
fn spend_as(spend: &SpendAs) -> Result<String> {
    Ok(match spend {
        SpendAs::ColorAsAnyColor(mana) => {
            format!("SpendAs::ColorAsAnyColor(Color::{})", color(mana)?)
        }
        other => format!("SpendAs::{other:?}"),
    })
}

/// `Comparison::Less`, for a read of `Comparison` in code.
fn comparing(comparison: &Comparison) -> String {
    format!("Comparison::{comparison:?}")
}

/// The word for a rule's operand that names a kind of ability.
fn ability_word(operand: &Operand) -> Result<String> {
    match operand {
        Operand::Term {
            family,
            form,
            operands,
        } if operands.is_empty() => Ok(format!("{family}::{form}")),
        Operand::Vocabulary { family, form } => Ok(format!("{family}::{form}")),
        other => Err(gap(format!("ability named by {}", super::form(other)))),
    }
}

/// Whether playing cards failed for what `stated_play` can say.
pub(super) fn play_stated(feature: &str) -> bool {
    feature.starts_with("cards played by")
        || feature.starts_with("play count")
        || feature.starts_with("a land played by")
}

/// Whether a cost branch failed for what `stated_branch` can say.
pub(super) fn branch_stated(feature: &str) -> bool {
    feature.starts_with("cost branch")
        || feature.starts_with("target count by branch")
        || feature.starts_with("cost objects of Branch")
        || feature == "cleave cost other than mana"
}

/// "Any player may have Browbeat deal 5 damage to them. If no one does, ...": what
/// is asked of each player, and what the next sentence says follows.
fn any_player_may(line: &str) -> Option<(String, String)> {
    let at = line.find(" may ")?;
    if !line[..at].to_lowercase().contains("any ") {
        return None;
    }
    let rest = &line[at + " may ".len()..];
    let (ask, after) = rest.split_once('.').unwrap_or((rest, ""));
    Some((format!("{ask}?"), after.trim().to_string()))
}

/// "In addition to its other colors": `colored` sets them all, so what the values
/// have now goes in with them. An exception's colors are the card's words.
fn added_colors(values: &str, colors: &str) -> String {
    format!("{{ let plain = {values}; let had = plain.colors.unwrap_or(q.card(plain.card).colors()); plain.colored([{colors}].into_iter().fold(had, |set: ColorSet, color: Color| set.with(q.words(asker.source).color(color)))) }}")
}

impl Compiler {
    /// `|q, asker, id| test` of what the filter says of the object `id`.
    pub(super) fn over(&self, filter: &ObjectFilter, id: &str) -> Result<String> {
        let test = self.filter(filter, &Candidate::of(id))?;
        Ok(format!(
            "|q: &dyn Query, asker: &Asker, {id}: ObjectId| {test}"
        ))
    }
    /// As `over`, of a card a player is about to play.
    fn over_played(&self, filter: &ObjectFilter, id: &str) -> Result<String> {
        let test = self.filter(filter, &players_card(id))?;
        Ok(format!(
            "|q: &dyn Query, asker: &Asker, player: PlayerId, {id}: ObjectId| {test}"
        ))
    }
    /// `|q, asker, defender| test` of whom an attack is declared at.
    fn defenders(&self, defenders: &AttackDefenders) -> Result<String> {
        let at = self.defended(defenders)?;
        Ok(format!(
            "|q: &dyn Query, asker: &Asker, defender: Target| {at}"
        ))
    }
    /// A number read when the rule is asked.
    fn asked(&self, n: &Expr) -> Result<String> {
        self.number(n).map(|n| n.code)
    }
    /// A filter of abilities, over `ability` and the `object` it is on.
    fn activation_bar(&self, filter: &ActivationFilter) -> Result<String> {
        Ok(format!(
            "|q: &dyn Query, asker: &Asker, object: ObjectId, ability: &ActivatedDef| {}",
            self.barred_activation(filter)?
        ))
    }
    /// A filter of abilities by what is on them, over `def` on `permanent`.
    fn ability_filter(&self, filter: &ObjectFilter) -> Result<String> {
        Ok(format!(
            "|q: &dyn Query, asker: &Asker, permanent: ObjectId, def: Option<&ActivatedDef>| {}",
            self.ability_test(filter)?
        ))
    }
    fn action_code(&self, action: &ObjectAction) -> Result<String> {
        use ObjectAction as A;
        Ok(match action {
            A::Attack(defenders) => format!("Action::Attack({})", self.defenders(defenders)?),
            A::Block(filter) => format!("Action::Block({})", self.over(filter, "attacker")?),
            A::BeBlocked(filter) => format!("Action::BeBlocked({})", self.over(filter, "blocker")?),
            A::Activate(filter) => format!("Action::Activate({})", self.activation_bar(filter)?),
            A::BeTargeted(filter) => format!(
                "Action::BeTargeted(|q: &dyn Query, asker: &Asker, by: &Asker, by_source: &Chars| {})",
                self.targeting_source(filter)?
            ),
            A::ReceiveCounters(None) => "Action::ReceiveCounters(None)".into(),
            A::ReceiveCounters(Some(kind)) => {
                format!("Action::ReceiveCounters(Some({}))", counter_kind(kind)?)
            }
            A::LoseCounters(kind) => format!("Action::LoseCounters({})", counter_kind(kind)?),
            A::BeAttachedBy {
                attachments,
                keep_existing,
            } => format!(
                "Action::BeAttachedBy {{ attachments: {}, keep_existing: {keep_existing} }}",
                self.over(attachments, "attachment")?
            ),
            other => format!("Action::{}", form(other)),
        })
    }
    fn requirement_amount(&self, amount: &RequirementAmount) -> Result<String> {
        Ok(match amount {
            RequirementAmount::AtLeast(n) => {
                format!("Required::AtLeast({})", self.asked(n)?)
            }
            RequirementAmount::Each => "Required::Each".into(),
        })
    }
    /// What a rule on objects is, whichever holds it. The reader is the holder's.
    pub(super) fn rule_code(&mut self, rule: &RuleChange) -> Result<String> {
        use RuleChange as R;
        let tap_cost = |cost: &TapPowerCost| format!("TapCost::{cost:?}");
        Ok(match rule {
            R::Primitive { name, .. } => return Err(gap(format!("primitive rule {name}"))),
            R::Prohibit(action) => format!("Rule::Cant({})", self.action_code(action)?),
            R::RequireCombat { action, amount } => format!(
                "Rule::Must({}, {})",
                self.action_code(action)?,
                self.requirement_amount(amount)?
            ),
            R::CombatCost {
                action,
                payer,
                cost,
            } => {
                let label = format!("rule_cost_{}", self.fresh());
                // None is whoever declares it (CR 508.1h, 509.1d).
                let payer = match payer {
                    PlayerRef::DeclaringPlayer => "None::<PlayerId>".to_string(),
                    other => self.certain_player(other)?,
                };
                format!(
                    "Rule::CostTo({}, {payer}, {})",
                    self.action_code(action)?,
                    self.cost(cost, &label)?
                )
            }
            R::CombatGroup {
                role,
                participants,
                others_only,
                comparison,
                amount,
            } => format!(
                "Rule::Group {{ role: CombatRole::{role:?}, counted: {}, others_only: {others_only}, count: ({}, {}) }}",
                self.over(participants, "other")?,
                comparing(comparison),
                self.asked(amount)?
            ),
            R::AttackException { ignore, defenders } => format!(
                "Rule::AttacksDespite(AttackCheck::{ignore:?}, {})",
                self.defenders(defenders)?
            ),
            R::BlockWhileTapped => "Rule::BlocksWhileTapped".into(),
            R::BlockCapacity(BlockCapacity::Additional(n)) => {
                format!("Rule::AdditionalBlocks({})", self.asked(n)?)
            }
            R::BlockCapacity(BlockCapacity::Unlimited) => "Rule::AdditionalBlocks(u32::MAX)".into(),
            R::CombatDamage(rule) => format!("Rule::CombatDamage(CombatDamageRule::{rule:?})"),
            R::UntapStep { players, behavior } => format!(
                "Rule::UntapStep {{ of: |q: &dyn Query, asker: &Asker, id: ObjectId| {}, behavior: UntapBehavior::{behavior:?} }}",
                self.untap_players(players)?
            ),
            R::GoadedBy(player) => format!("Rule::GoadedBy({})", self.certain_player(player)?),
            R::LegendRuleExempt => "Rule::LegendRuleExempt".into(),
            R::TapPowerBonus { cost, bonus } => {
                format!(
                    "Rule::TapPowerBonus({}, {})",
                    tap_cost(cost),
                    self.asked(bonus)?
                )
            }
            R::MayExert(None) => "Rule::MayExert".into(),
            R::BlockedAsThoughLacking(ability) => format!(
                "Rule::BlockedAsThoughLacking({})",
                ability_word(ability)?
            ),
            R::CantGainAbility(ability) => {
                format!("Rule::CantGainAbility({})", ability_word(ability)?)
            }
            R::DamageStaysMarked => "Rule::DamageStaysMarked".into(),
            R::CantAttackDefenderIf(condition) => format!(
                "Rule::CantAttackDefenderIf(|q: &dyn Query, asker: &Asker, player: PlayerId| {})",
                self.condition(condition)?
            ),
            R::TargetableDespite { sources, keyword } => format!(
                "Rule::TargetableDespite {{ by: |q: &dyn Query, asker: &Asker, by: &Asker, by_source: &Chars| {}, keyword: Keyword::{keyword:?} }}",
                self.targeting_source(sources)?
            ),
            R::AttachOnlyTo(filter) => {
                format!("Rule::AttachOnlyTo({})", self.over(filter, "host")?)
            }
            R::BlocksOnlyIf(condition) => format!(
                "Rule::BlocksOnlyIf(|q: &dyn Query, asker: &Asker, blocker: ObjectId, attacker: ObjectId| {})",
                self.condition(condition)?
            ),
            R::BlockedByDefender { players, unless } => format!(
                "Rule::BlockedByDefender {{ players: |q: &dyn Query, asker: &Asker, player: PlayerId| {}, unless: {unless} }}",
                self.player_member(players)?
            ),
            R::AttackDrags(filter) => format!("Rule::AttackDrags({})", self.over(filter, "other")?),
            R::AttacksWhenAttacking(filter) => {
                format!("Rule::AttacksWhenAttacking({})", self.over(filter, "other")?)
            }
            R::BlocksDespite { attackers, evasion } => format!(
                "Rule::BlocksDespite {{ attackers: {}, evasion: BlockEvasion::{evasion:?} }}",
                self.over(attackers, "attacker")?
            ),
            R::TapCostUsesToughness(cost) => {
                format!("Rule::TapCostUsesToughness({})", tap_cost(cost))
            }
            R::CounterLimit { kind, max } => format!(
                "Rule::CounterLimit({}, {})",
                counter_kind(kind)?,
                self.asked(max)?
            ),
            R::ActivationsEachTurn { abilities, times } => format!(
                "Rule::ActivationsEachTurn({}, {})",
                self.ability_filter(abilities)?,
                self.asked(times)?
            ),
            R::AssignDamageTo {
                players,
                permanents,
            } => {
                let players = match players {
                    Some(player) => self.player(player)?,
                    None => "None".to_string(),
                };
                format!(
                    "Rule::AssignDamageTo {{ player: {players}, permanents: {} }}",
                    self.over(permanents, "id")?
                )
            }
            R::Unique(rule) => format!("Rule::{rule:?}"),
            other => return Err(gap(format!("static rule {}", effects_rule_name(other)))),
        })
    }
    /// "During its controller's untap step" and the like, of the object `id`.
    fn untap_players(&self, players: &PlayerSet) -> Result<String> {
        if let PlayerSet::One(PlayerRef::ControllerOf(object)) = players {
            if matches!(**object, ObjectRef::Affected) {
                return Ok("q.state().turn.active == q.controller(id)".into());
            }
        }
        let member = format!(
            "({}).contains(&q.state().turn.active)",
            self.players(players)?
        );
        Ok(member)
    }
    /// A resolving effect's rule on an object, which `EffectKind` will hold.
    pub(super) fn stated_rule_kind(&mut self, rule: &RuleChange) -> Result<Vec<String>> {
        let feature = format!("continuous rule {}", effects_rule_name(rule));
        let saved = self.frame;
        let code = self.rule_code(rule);
        self.frame = saved;
        let code = code.map_err(|_| gap(&feature))?;
        Ok(vec![planned::planned_value(
            &feature,
            "EffectKind::CantBlock",
            &format!("EffectKind::Rule({code})"),
        )])
    }
    /// A static ability's rule on objects, with no table of its own yet. False
    /// if the rule is one the layers carry: `static_ability` does it then.
    pub(super) fn stated_rule(
        &mut self,
        function: &str,
        affected: &ObjectSet,
        rule: &RuleChange,
        parts: &mut Parts,
    ) -> Result<bool> {
        if self.layered_rule(rule) {
            return Ok(false);
        }
        let feature = format!("static rule {}", effects_rule_name(rule));
        // "Can't attack if defending player controls ...": judged of each attack
        // that could be declared, where the defending player is who it would be.
        if let RuleChange::CantAttackDefenderIf(condition) = rule {
            const DEFENDER: &str =
                "Some(asker.source).and_then(|o: ObjectId| q.defending_player(o))";
            let condition = self.condition(condition).map_err(|_| gap(&feature))?;
            let condition = condition.replace(DEFENDER, "Some(defender)");
            if !condition.contains("defending_player") {
                let (guard, test) = (self.guard()?, self.rule_subject(affected, "id")?);
                writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, id: ObjectId) -> bool {{ let source = asker.source; {guard}{test} }}").unwrap();
                writeln!(self.functions, "#[allow(unused_variables)] fn {function}_attack(q: &dyn Query, asker: &Asker, attacker: ObjectId, at: Target) -> bool {{ if !{function}(q, asker, attacker) {{ return true; }} let Some(defender) = defending_player_of(q.state(), at) else {{ return true; }}; !({condition}) }}").unwrap();
                parts.attack_rules.push(format!("{function}_attack"));
                return Ok(true);
            }
        }
        // "Can't block unless ...": a block by the affected, whatever it blocks. The
        // attacking player is the active player (506.2).
        if let RuleChange::BlocksOnlyIf(condition) = rule {
            let condition = self.condition(condition).map_err(|_| gap(&feature))?;
            let (guard, test) = (self.guard()?, self.rule_subject(affected, "id")?);
            writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, id: ObjectId) -> bool {{ let source = asker.source; {guard}{test} }}").unwrap();
            writeln!(self.functions, "#[allow(unused_variables)] fn {function}_block(q: &dyn Query, source: ObjectId, blocker: ObjectId, attacker: ObjectId) -> bool {{ let asker = &Asker::new(source, q.controller(source)); !{function}(q, asker, blocker) || ({condition}) }}").unwrap();
            parts.block_rules.push(format!("{function}_block"));
            return Ok(true);
        }
        // "Can't be blocked as long as defending player controls ...": of each block,
        // whoever is defending against that attacker (CR 506.2).
        if let RuleChange::BlockedByDefender { players, unless } = rule {
            let member = self.player_member(players).map_err(|_| gap(&feature))?;
            let allowed = if *unless { member } else { format!("!({member})") };
            let (guard, test) = (self.guard()?, self.rule_subject(affected, "id")?);
            writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, id: ObjectId) -> bool {{ let source = asker.source; {guard}{test} }}").unwrap();
            writeln!(self.functions, "#[allow(unused_variables)] fn {function}_block(q: &dyn Query, source: ObjectId, blocker: ObjectId, attacker: ObjectId) -> bool {{ let asker = &Asker::new(source, q.controller(source)); !{function}(q, asker, attacker) || q.defending_player(attacker).is_none_or(|player| {allowed}) }}").unwrap();
            parts.block_rules.push(format!("{function}_block"));
            return Ok(true);
        }
        // "Can't be turned face up" (CR 708.7) is the source's effect, no ability of
        // the permanent, and stops every way of turning it up.
        if matches!(rule, RuleChange::Prohibit(ObjectAction::TurnFaceUp)) {
            let (guard, test) = (self.guard()?, self.rule_subject(affected, "id")?);
            writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, id: ObjectId) -> bool {{ let source = asker.source; {guard}{test} }}").unwrap();
            parts.table("face_up_bans", function.to_string());
            return Ok(true);
        }
        let code = self.rule_code(rule).map_err(|_| gap(&feature))?;
        let (guard, test) = (self.guard()?, self.rule_subject(affected, "id")?);
        writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, id: ObjectId) -> bool {{ let source = asker.source; {guard}{test} }}").unwrap();
        self.plan_entry(
            &feature,
            "statics",
            &format!("object_rule({function}, {code})"),
            parts,
        );
        Ok(true)
    }
    /// A rule on a set that can still gain members (CR 611.2c): the effect holds
    /// the test, and applies to whatever passes it while it lasts.
    pub(super) fn open_rule_stated(
        &mut self,
        source: &ObjectRef,
        affected: &ObjectSet,
        changes: &[Change],
        duration: &crate::ir::Duration,
        out: &mut String,
    ) -> Result<()> {
        let test = match affected {
            ObjectSet::Battlefield(filter) => self.filter(filter, &Candidate::of("id"))?,
            other => match self.outside_subject(other, &Candidate::of("id"))? {
                Some(test) => test,
                None => return Err(gap(format!("open set {}", form(other)))),
            },
        };
        let until = self.stated_until(duration)?;
        let source = self.fetch(self.object(source)?);
        for change in changes {
            let what = match change {
                Change::Rule(rule) => effects_rule_name(rule),
                other => form(other),
            };
            let feature = format!("engine: no resolving rule on an open set: {what}");
            let kinds = self
                .effect_kinds(change)
                .map_err(|_| gap(feature.as_str()))?;
            for kind in kinds {
                self.plan_code(
                    &feature,
                    &format!("if let Some(source) = {source} {{ g.add_open_effect(source, move |q: &dyn Query, asker: &Asker, id: ObjectId| {test}, {kind}, {until}); }}"),
                    out,
                );
            }
        }
        Ok(())
    }
    // --- Operands, read strictly: what can't be read is no code at all.

    /// A read as this reader makes it: only a resolution borrows `g`.
    fn read(&self, code: impl AsRef<str>) -> String {
        match self.reader {
            Reader::Resolution => self.fetch(code),
            _ => code.as_ref().to_string(),
        }
    }
    /// A duration, in the word it will have if it has none yet.
    pub(super) fn stated_until(&mut self, duration: &crate::ir::Duration) -> Result<String> {
        use crate::ir::Duration as D;
        let failed = match self.duration(duration) {
            Ok(code) => return Ok(code),
            Err(failed) => failed,
        };
        let code = match duration {
            D::DuringControllersNextUntapStep => {
                "Duration::DuringItsControllersNextUntapStep".to_string()
            }
            D::DuringNextUntapStep(player) => format!(
                "Duration::DuringNextUntapStepOf({})",
                self.read(self.certain_player(player)?)
            ),
            D::UntilNext { point, players } => format!(
                "Duration::UntilNext(TurnPoint::{point:?}, {})",
                self.read(self.players(players)?)
            ),
            D::While(condition) => format!(
                "Duration::While(|q: &dyn Query, asker: &Asker| {})",
                self.condition(condition)?
            ),
            D::WhileItMatches(filter) => {
                format!("Duration::WhileItMatches({})", self.over(filter, "id")?)
            }
            D::WhileMatches { object, filter } => format!(
                "Duration::WhileObjectMatches({}, {})",
                self.read(self.object(object)?),
                self.over(filter, "id")?
            ),
            D::WhileExiled(object) => {
                format!("Duration::WhileExiled({})", self.read(self.object(object)?))
            }
            D::WhileAnyExiled(objects) => {
                format!(
                    "Duration::WhileAnyExiled({})",
                    self.read(self.objects(objects)?)
                )
            }
            D::UntilAnotherExiled { by, with } => format!(
                "Duration::UntilAnotherExiled {{ by: {}, with: {} }}",
                self.read(self.players(by)?),
                self.read(self.object(with)?)
            ),
            D::UntilLibraryTopChanges(player) => format!(
                "Duration::UntilLibraryTopChanges({})",
                self.read(self.certain_player(player)?)
            ),
            D::ExtraTurn(result) => format!("Duration::ThroughExtraTurn({})", result.0),
            D::ThroughNextTurn => "Duration::ThroughNextTurn".to_string(),
            D::EndOfCombatOnNextTurn(player) => format!(
                "Duration::EndOfCombatOnNextTurn({})",
                self.read(self.certain_player(player)?)
            ),
            D::UntilNextTurn(player) => format!(
                "Duration::UntilNextTurnOf({})",
                self.read(self.player(player)?)
            ),
            D::UntilEndOfNextTurn(player) => format!(
                "Duration::UntilEndOfNextTurnOf({})",
                self.read(self.player(player)?)
            ),
            D::Either(durations) => {
                let mut all = Vec::new();
                for each in durations {
                    all.push(self.stated_until(each)?);
                }
                format!("Duration::Either(&[{}])", all.join(", "))
            }
            _ => return Err(failed),
        };
        Ok(planned::planned_value(
            &failed.feature,
            "Duration::Indefinite",
            &code,
        ))
    }
    /// `part`, with the reader of a plain function: a rule asked of the game.
    fn queried<T>(&mut self, part: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        let saved = (self.reader, self.event_at);
        (self.reader, self.event_at) = (Reader::Query, EventAt::Nowhere);
        let compiled = part(self);
        (self.reader, self.event_at) = saved;
        compiled
    }

    /// An operand of a rule, as the code it will be.
    pub(super) fn stated(&mut self, operand: &Operand) -> Result<String> {
        let unread = |what: &Operand| gap(format!("operand {}", form(what)));
        Ok(match operand {
            Operand::Absent => "None".to_string(),
            Operand::Bool(value) => value.to_string(),
            Operand::Integer(value) => value.to_string(),
            Operand::Text(text) => quoted(text),
            Operand::List(list) => {
                let mut items = Vec::new();
                for item in list {
                    items.push(self.stated(item)?);
                }
                format!("&[{}]", items.join(", "))
            }
            Operand::Vocabulary { family, form } => format!("{family}::{form}"),
            Operand::Term {
                family,
                form,
                operands,
            } => {
                let mut items = Vec::new();
                for item in operands {
                    items.push(self.stated(item)?);
                }
                match items.is_empty() {
                    true => format!("{family}::{form}"),
                    false => format!("{family}::{form}({})", items.join(", ")),
                }
            }
            Operand::Object(object) => self.read(self.object(object)?),
            Operand::Objects(objects) => self.read(self.objects(objects)?),
            Operand::Player(player) => self.read(self.player(player)?),
            Operand::Players(players) => self.read(self.players(players)?),
            Operand::Number(number) => self.read(self.number(number)?.code),
            Operand::Condition(condition) => format!(
                "|q: &dyn Query, asker: &Asker| {}",
                self.condition(condition)?
            ),
            Operand::Duration(duration) => self.stated_until(duration)?,
            Operand::Filter(filter) => self.over(filter, "id")?,
            Operand::Comparison(comparison, value) => format!(
                "({}, {})",
                comparing(comparison),
                self.read(self.number(value)?.code)
            ),
            Operand::Cost(cost) => {
                let label = format!("rule_cost_{}", self.fresh());
                self.cost(cost, &label)?
            }
            Operand::Program(program) => {
                let saved = (self.reader, self.event_at);
                (self.reader, self.event_at) = match saved.1 {
                    EventAt::Item => (Reader::Resolution, EventAt::Item),
                    _ => (Reader::Resolution, EventAt::Nowhere),
                };
                let mut body = String::new();
                let compiled = self.block(program, &mut body, true);
                (self.reader, self.event_at) = saved;
                compiled?;
                format!(
                    "|g: &mut dyn Game, asker: &Asker| -> Result<(), Suspend> {{ {body} Ok(()) }}"
                )
            }
            Operand::Ability(ability) => {
                let refs = self.granted_refs(ability)?;
                match refs.as_slice() {
                    [one] => one.clone(),
                    many => format!("&[{}]", many.join(", ")),
                }
            }
            other => return Err(unread(other)),
        })
    }

    // --- Rules on players.

    fn zone_scope(&self, scope: &ZoneScope) -> Result<String> {
        Ok(match scope {
            ZoneScope::Shared(zone) => format!("Scope::Shared({})", zone_kind(zone)?),
            ZoneScope::Players { zone, players } => format!(
                "Scope::Of({}, |q: &dyn Query, asker: &Asker, player: PlayerId| {})",
                zone_kind(zone)?,
                self.player_member(players)?
            ),
        })
    }
    fn payment(&self, on: &ManaPayment) -> Result<String> {
        Ok(match on {
            ManaPayment::Anything => "Payment::Anything".into(),
            ManaPayment::Spells(filter) => {
                format!("Payment::Spells({})", self.over_played(filter, "spell")?)
            }
            ManaPayment::Abilities(filter) => {
                format!("Payment::Abilities({})", self.ability_filter(filter)?)
            }
        })
    }
    /// `bool`: whether a cost, told only what it pays for as `costing`, is one of
    /// those `on` names. Which spell or ability it pays for a `CostBan` isn't told.
    pub(super) fn cost_paid_for(&self, on: &ManaPayment) -> Result<String> {
        const NOT_MANA: &str = "!(def.is_some_and(|d| d.is_mana_ability()))";
        Ok(match on {
            ManaPayment::Anything => "true".to_string(),
            ManaPayment::Spells(ObjectFilter::Any) => "matches!(costing, CostFor::Spell)".to_string(),
            ManaPayment::Abilities(ObjectFilter::Any) => "!matches!(costing, CostFor::Spell)".to_string(),
            ManaPayment::Abilities(filter) if self.ability_test(filter)? == NOT_MANA => {
                "matches!(costing, CostFor::Ability)".to_string()
            }
            _ => return Err(gap("a cost ban on some spells or abilities")),
        })
    }
    fn prohibition(&self, action: &PlayerAction) -> Result<String> {
        use PlayerAction as A;
        let scope = |c: &Self, from: &Option<ZoneScope>| -> Result<String> {
            Ok(match from {
                Some(scope) => format!("Some({})", c.zone_scope(scope)?),
                None => "None".to_string(),
            })
        };
        Ok(match action {
            A::LoseGame { reason } => format!("Prohibition::LoseGame({})", reason.is_none()),
            A::ReceiveCounters(None) => "Prohibition::ReceiveCounters(None)".into(),
            A::ReceiveCounters(Some(kind)) => {
                format!("Prohibition::ReceiveCounters(Some({}))", counter_kind(kind)?)
            }
            A::PlayLand { from, filter } => format!(
                "Prohibition::PlayLand {{ from: {}, lands: {} }}",
                scope(self, from)?,
                self.over_played(filter, "land")?
            ),
            A::CastSpell { from, filter } => format!(
                "Prohibition::CastSpell {{ from: {}, spells: {} }}",
                scope(self, from)?,
                self.over_played(filter, "spell")?
            ),
            A::Activate(filter) => {
                format!("Prohibition::Activate({})", self.activation_bar(filter)?)
            }
            A::AttackWith(filter) => {
                format!("Prohibition::AttackWith({})", self.over(filter, "attacker")?)
            }
            A::BlockWith(filter) => {
                format!("Prohibition::BlockWith({})", self.over(filter, "blocker")?)
            }
            A::AttackInto(recipients) => format!(
                "Prohibition::AttackInto({})",
                self.fetch(self.recipients(recipients)?)
            ),
            A::PayLife(on) => format!("Prohibition::PayLife({})", self.payment(on)?),
            A::Sacrifice { objects, paying } => {
                let paying = match paying {
                    Some(on) => format!("Some({})", self.payment(on)?),
                    None => "None".to_string(),
                };
                format!(
                    "Prohibition::Sacrifice {{ objects: {}, paying: {paying} }}",
                    self.over(objects, "id")?
                )
            }
            A::PlayCard(filter) => {
                format!("Prohibition::PlayCard({})", self.over_played(filter, "card")?)
            }
            A::ExileObjects(filter) => {
                format!("Prohibition::ExileObjects({})", self.over(filter, "id")?)
            }
            A::BeTargeted(filter) => format!(
                "Prohibition::BeTargeted(|q: &dyn Query, asker: &Asker, by: &Asker, by_source: &Chars| {})",
                self.targeting_source(filter)?
            ),
            A::CastOutside(zone) => {
                format!("Prohibition::CastOutside({})", zone_kind(zone)?)
            }
            A::AttackDefender(defenders) => {
                format!("Prohibition::AttackDefender({})", self.defenders(defenders)?)
            }
            other => format!("Prohibition::{}", form(other)),
        })
    }
    /// What a permission grants, by the fields of `PlayGrant` it will have.
    fn play_grant(&mut self, action: &CardPlayAction, terms: &PlayTerms) -> Result<String> {
        let mut grant = match action {
            CardPlayAction::CastSpell => String::new(),
            CardPlayAction::Either => "lands: true, ".to_string(),
            CardPlayAction::PlayLand => "lands: true, spells: false, ".to_string(),
            other => return Err(gap(format!("play permission to {}", form(other)))),
        };
        match &terms.limit {
            None => {}
            Some(PlayLimit::Total(n)) => write!(grant, "limit: PlayLimit::Total({n}), ").unwrap(),
            Some(PlayLimit::EachTurn(n)) => {
                write!(grant, "limit: PlayLimit::EachTurn({n}), ").unwrap()
            }
            Some(PlayLimit::OnePerCardType(class)) => write!(
                grant,
                "limit: PlayLimit::OnePerCardType(TypeClass::{class:?}), "
            )
            .unwrap(),
        }
        if let Some(during) = &terms.during {
            write!(
                grant,
                "during: Some(|q: &dyn Query, asker: &Asker, player: PlayerId| {}), ",
                self.player_member(during)?
            )
            .unwrap();
        }
        match &terms.cost {
            PlayCost::Normal => {}
            PlayCost::Free => grant.push_str("instead: Some(ManaCost::ZERO), "),
            PlayCost::Alternate(cost) => {
                write!(grant, "alternate: Some({}), ", self.stated(cost)?).unwrap()
            }
            PlayCost::Additional(cost) => {
                write!(grant, "also: Some({}), ", self.stated(cost)?).unwrap()
            }
            PlayCost::Using(keyword) => {
                write!(grant, "using: Some(CostedKeyword::{keyword:?}), ").unwrap()
            }
            PlayCost::Reduced(less) => {
                write!(grant, "less: Some({}), ", self.stated(less)?).unwrap()
            }
        }
        if let Some(spend) = &terms.spend_as {
            write!(grant, "spend_as: {}, ", spend_as(spend)?).unwrap();
        }
        if terms.into_exile {
            grant.push_str("exile_instead: ExileInstead::Spell, ");
        }
        if let Some(condition) = &terms.condition {
            write!(
                grant,
                "when: Some(PlayCondition::Rule(|q: &dyn Query, asker: &Asker| {})), ",
                self.condition(condition)?
            )
            .unwrap();
        }
        if !terms.effects.is_empty() {
            let mut riders = Vec::new();
            for effect in &terms.effects {
                riders.push(self.play_rider(effect)?);
            }
            write!(grant, "rider: Some(&[{}]), ", riders.join(", ")).unwrap();
        }
        if terms.flash {
            grant.push_str("flash: true, ");
        }
        if terms.may_look {
            grant.push_str("may_look: true, ");
        }
        if let Some(then) = &terms.then {
            write!(grant, "then: Some(Destination::{}), ", form(then)).unwrap();
        }
        if let Some(face_down) = &terms.face_down {
            write!(grant, "face_down: Some({}), ", quoted(&form(face_down))).unwrap();
        }
        Ok(format!("PlayGrant {{ {grant}..PlayGrant::CAST }}"))
    }
    /// A permission to play cards, whichever holds it: the cards are asked of
    /// as they are played, so they can be ones that come there later.
    fn play_code(&mut self, rule: &PlayerRule) -> Result<String> {
        let plain = lane_h::plain_terms();
        let (permission, terms) = match rule {
            PlayerRule::PlayPermissionWith { permission, terms } => (&**permission, terms),
            other => (other, &plain),
        };
        let PlayerRule::PlayPermission {
            cards,
            from,
            action,
            filter,
        } = permission
        else {
            return Err(gap(format!("player rule {}", form(rule))));
        };
        let chosen = match filter {
            ObjectFilter::Any => cards.clone(),
            filter => ObjectSet::Matching {
                objects: Box::new(cards.clone()),
                filter: filter.clone(),
            },
        };
        let zone = match from {
            EventZone::Outside => "ZoneKind::Outside".to_string(),
            other => zone_kind(other)?,
        };
        let cards = self.cards_of_rule(&chosen)?;
        let grant = self.play_grant(action, terms)?;
        Ok(format!(
            "PlayerRule::Play {{ zone: {zone}, cards: {cards}, grant: {grant} }}"
        ))
    }
    /// The cards a rule is about. A rule is asked after the resolution that
    /// made it, when `g` is gone: it reads them then with what it is asked
    /// by, keeping the resolution's locals; and what only that resolution
    /// could read, or what reads nothing later, it keeps as they were read.
    fn cards_of_rule(&mut self, cards: &ObjectSet) -> Result<String> {
        let test = "|q: &dyn Query, asker: &Asker, card: ObjectId|";
        if !matches!(self.reader, Reader::Resolution) {
            // Of a static rule, which holds for each player it holds for.
            return Ok(format!(
                "CardsOfRule::Asked(|q: &dyn Query, asker: &Asker, player: PlayerId, card: ObjectId| ({}).contains(&card))",
                self.objects(cards)?
            ));
        }
        let gaps = self.gaps.len();
        match self.queried(|c| c.objects(cards)) {
            Ok(code) if code.contains("q.") => Ok(format!(
                "CardsOfRule::kept(move {test} ({code}).contains(&card))"
            )),
            _ => {
                self.gaps.truncate(gaps);
                Ok(format!(
                    "CardsOfRule::These({})",
                    self.fetch(self.objects(cards)?)
                ))
            }
        }
    }
    fn cost_modifier(&mut self, modifier: &SpellCostModifier) -> Result<String> {
        use SpellCostModifier as S;
        let limit = |limit: &Option<PlayLimit>| match limit {
            None => "None".to_string(),
            Some(limit) => format!("Some(PlayLimit::{limit:?})"),
        };
        Ok(match modifier {
            S::Alternate { cost, limit: most } => format!(
                "CostModifier::Alternate {{ cost: {}, limit: {} }}",
                self.play_cost(cost)?,
                limit(most)
            ),
            S::AlternateDuring {
                cost,
                limit: most,
                during,
            } => format!(
                "CostModifier::Alternate {{ cost: {}, limit: {}, during: |q: &dyn Query, asker: &Asker, player: PlayerId| {} }}",
                self.play_cost(cost)?,
                limit(most),
                self.player_member(during)?
            ),
            S::PayToReduce { cost, mana } => format!(
                "CostModifier::PayToReduce {{ cost: {}, mana: {} }}",
                self.stated(cost)?,
                self.mana_operand(mana)?
            ),
            S::Scaled { change, per } => {
                let per = match per {
                    CostScale::Times(n) => format!("CostScale::Times({})", self.asked(n)?),
                    CostScale::EachTarget => "CostScale::EachTarget".to_string(),
                };
                format!(
                    "CostModifier::Scaled {{ change: {}, per: {per} }}",
                    self.cost_change(change)?
                )
            }
            S::OptionalAdditional { cost, effects } => format!(
                "CostModifier::OptionalAdditional {{ cost: {}, then: {} }}",
                self.stated(cost)?,
                self.stated(effects)?
            ),
            S::Increase(cost) => {
                let label = format!("rule_cost_{}", self.fresh());
                format!("CostModifier::Increase({})", self.cost(cost, &label)?)
            }
            other => self.cost_delta(other)?,
        })
    }
    fn play_cost(&mut self, cost: &PlayCost) -> Result<String> {
        Ok(match cost {
            PlayCost::Normal => "PlayCost::Normal".to_string(),
            PlayCost::Free => "PlayCost::Free".to_string(),
            PlayCost::Alternate(cost) => {
                format!("PlayCost::Alternate({})", self.mana_operand(cost)?)
            }
            PlayCost::Additional(cost) => {
                format!("PlayCost::Additional({})", self.mana_operand(cost)?)
            }
            PlayCost::Using(keyword) => format!("PlayCost::Using(CostedKeyword::{keyword:?})"),
            PlayCost::Reduced(less) => format!("PlayCost::Reduced({})", self.mana_operand(less)?),
        })
    }
    /// A cost or an amount of mana, written as symbols or as a list of them.
    fn mana_operand(&mut self, operand: &Operand) -> Result<String> {
        if let Operand::List(items) = operand {
            let mut symbols = Vec::new();
            for item in items {
                symbols.push(self.mana_operand(item)?);
            }
            return Ok(format!("&[{}]", symbols.join(", ")));
        }
        self.stated(operand)
    }
    fn cost_change(&mut self, change: &ActivationCostChange) -> Result<String> {
        use ActivationCostChange as C;
        Ok(match change {
            C::Reduce(amount) => format!("CostChange::Reduce({})", self.mana_operand(amount)?),
            C::Increase(amount) => format!("CostChange::Increase({})", self.mana_operand(amount)?),
            C::ReduceX { amount, times } => format!(
                "CostChange::ReduceX {{ amount: {}, times: {} }}",
                self.mana_operand(amount)?,
                self.asked(times)?
            ),
            C::Additional(cost) => {
                let label = format!("rule_cost_{}", self.fresh());
                format!("CostChange::Additional({})", self.cost(cost, &label)?)
            }
            other => return Err(gap(format!("activation cost change {}", form(other)))),
        })
    }
    /// What a rule on players is, whichever holds it. The reader is the holder's.
    pub(super) fn player_rule_code(&mut self, rule: &PlayerRule) -> Result<String> {
        let outer = std::mem::replace(&mut self.rule_player, true);
        let code = self.player_rule_read(rule);
        self.rule_player = outer;
        code
    }
    fn player_rule_read(&mut self, rule: &PlayerRule) -> Result<String> {
        use PlayerRule as R;
        let count = |c: &Self, n: &Option<(Comparison, Expr)>| -> Result<String> {
            Ok(match n {
                Some((comparison, n)) => {
                    format!("Some(({}, {}))", comparing(comparison), c.asked(n)?)
                }
                None => "None".to_string(),
            })
        };
        Ok(match rule {
            R::Primitive { name, .. } => return Err(gap(format!("primitive player rule {name}"))),
            R::PlayPermission { .. } | R::PlayPermissionWith { .. } => self.play_code(rule)?,
            R::Prohibit(action) => format!("PlayerRule::Cant({})", self.prohibition(action)?),
            R::GrantKeyword(keyword) => match keyword {
                PlayerKeyword::Protection(quality) => {
                    format!("PlayerRule::Protection({})", self.quality_code(quality)?)
                }
                PlayerKeyword::HexproofFrom(quality) => {
                    format!("PlayerRule::HexproofFrom({})", self.hexproof_of(quality)?)
                }
                PlayerKeyword::Hexproof => "PlayerRule::Keyword(Keyword::Hexproof)".to_string(),
                PlayerKeyword::Shroud => "PlayerRule::Keyword(Keyword::Shroud)".to_string(),
            },
            R::LimitPerTurn { action, amount } => format!(
                "PlayerRule::LimitPerTurn({}, {})",
                self.prohibition(action)?,
                self.asked(amount)?
            ),
            R::HandSize(HandSizeRule::Unlimited) => "PlayerRule::HandSize(HandSize::Unlimited)".into(),
            R::HandSize(HandSizeRule::Set(n)) => {
                format!("PlayerRule::HandSize(HandSize::Set({}))", self.asked(n)?)
            }
            R::HandSize(HandSizeRule::Adjust(n)) => {
                format!("PlayerRule::HandSize(HandSize::Adjust({}))", self.asked(n)?)
            }
            R::AdditionalLandPlays(Count::Exactly(n)) => {
                format!("PlayerRule::ExtraLandPlays({})", self.asked(n)?)
            }
            R::AdditionalLandPlays(Count::AnyNumber) => "PlayerRule::AnyNumberOfLandPlays".into(),
            R::SpellCost { spells, modifier } => format!(
                "PlayerRule::SpellCost {{ spells: {}, change: {} }}",
                self.over_played(spells, "spell")?,
                self.cost_modifier(modifier)?
            ),
            R::SpellTiming { spells, timing } => format!(
                "PlayerRule::SpellTiming {{ spells: {}, timing: SpellTiming::{timing:?} }}",
                self.over_played(spells, "spell")?
            ),
            R::Skip(section) => format!("PlayerRule::Skip(TurnSection::{section:?})"),
            R::Visibility { objects, audience } => format!(
                "PlayerRule::Visibility {{ objects: {}, audience: Audience::{audience:?} }}",
                self.fetch(self.objects(objects)?)
            ),
            R::RetainMana { colors: None } => "PlayerRule::RetainMana(ManaColors::ALL)".into(),
            R::RetainMana {
                colors: Some(colors),
            } => format!("PlayerRule::RetainMana(ColorSet({}))", color_mask(colors)?),
            R::DamageDoesNotCauseLifeLoss => "PlayerRule::DamageDoesNotCauseLifeLoss".into(),
            R::UntapLimit { permanents, amount } => format!(
                "PlayerRule::UntapLimit {{ permanents: {}, amount: {} }}",
                self.over(permanents, "id")?,
                self.asked(amount)?
            ),
            R::SpellsHaveAbilities {
                spells,
                from,
                nth_each_turn,
                abilities,
            } => {
                let from = match from {
                    Some(zone) => format!("Some({})", zone_kind(zone)?),
                    None => "None".to_string(),
                };
                let nth = match nth_each_turn {
                    Some(n) => format!("Some({})", self.asked(n)?),
                    None => "None".to_string(),
                };
                format!(
                    "PlayerRule::SpellsHave(SpellGrant {{ spells: {}, from: {from}, nth_each_turn: {nth}, grants: {} }})",
                    self.over_played(spells, "spell")?,
                    self.stated(abilities)?
                )
            }
            R::SpendManaAs { spend_as: spend, on } => format!(
                "PlayerRule::SpendManaAs {{ spend_as: {}, on: {} }}",
                spend_as(spend)?,
                self.payment(on)?
            ),
            R::ActivationCost {
                abilities,
                change,
                floor,
            } => format!(
                "PlayerRule::ActivationCost {{ abilities: {}, change: {}, floor: {floor:?} }}",
                self.ability_filter(abilities)?,
                self.cost_change(change)?
            ),
            R::DamageLifeFloor(n) => format!("PlayerRule::DamageLifeFloor({})", self.asked(n)?),
            R::SpecialAction {
                body,
                cost,
                limit,
                during,
                instant_speed,
            } => {
                let cost = match cost {
                    Some(cost) => format!("Some({})", self.stated(cost)?),
                    None => "None".to_string(),
                };
                let limit = match limit {
                    Some(limit) => format!("Some(PlayLimit::{limit:?})"),
                    None => "None".to_string(),
                };
                let during = match during {
                    Some(players) => format!(
                        "Some(|q: &dyn Query, asker: &Asker, player: PlayerId| {})",
                        self.player_member(players)?
                    ),
                    None => "None".to_string(),
                };
                format!(
                    "PlayerRule::SpecialAction {{ body: {}, cost: {cost}, limit: {limit}, during: {during}, instant_speed: {instant_speed} }}",
                    self.stated(body)?
                )
            }
            R::ActivationGrant { abilities, grant } => {
                let grant = match grant {
                    ActivationGrant::InstantSpeedDuring(players) => format!(
                        "ActivationGrant::InstantSpeedDuring(|q: &dyn Query, asker: &Asker, player: PlayerId| {})",
                        self.player_member(players)?
                    ),
                    ActivationGrant::AdditionalTimes(n) => {
                        format!("ActivationGrant::AdditionalTimes({})", self.asked(n)?)
                    }
                    other => format!("ActivationGrant::{other:?}"),
                };
                format!(
                    "PlayerRule::ActivationGrant {{ abilities: {}, grant: {grant} }}",
                    self.ability_filter(abilities)?
                )
            }
            R::ActivationTiming(timing) => {
                format!("PlayerRule::ActivationTiming(SpellTiming::{timing:?})")
            }
            R::KeywordAccess {
                keyword,
                cards,
                from,
                during,
            } => {
                let during = match during {
                    Some(players) => format!(
                        "Some(|q: &dyn Query, asker: &Asker, player: PlayerId| {})",
                        self.player_member(players)?
                    ),
                    None => "None".to_string(),
                };
                format!(
                    "PlayerRule::KeywordAccess {{ keyword: CostedKeyword::{keyword:?}, cards: {}, from: {}, during: {during} }}",
                    self.over_played(cards, "card")?,
                    zone_kind(from)?
                )
            }
            R::Decides(decision) => {
                let decision = match decision {
                    GameDecision::HowCreaturesBlock(filter) => {
                        format!("Decision::HowCreaturesBlock({})", self.over(filter, "id")?)
                    }
                    GameDecision::CombatDamageOf(filter) => {
                        format!("Decision::CombatDamageOf({})", self.over(filter, "id")?)
                    }
                    GameDecision::SearchesOf(players) => format!(
                        "Decision::SearchesOf(|q: &dyn Query, asker: &Asker, player: PlayerId| {})",
                        self.player_member(players)?
                    ),
                    other => format!("Decision::{other:?}"),
                };
                format!("PlayerRule::Decides({decision})")
            }
            R::MustAttack {
                attackers,
                defenders,
                count: n,
            } => format!(
                "PlayerRule::MustAttack {{ attackers: {}, defenders: {}, count: {} }}",
                self.over(attackers, "attacker")?,
                self.defenders(defenders)?,
                count(self, n)?
            ),
            R::AttackOnly(defenders) => {
                // Who may attack is asked of the attacker: "nearest opponent".
                format!(
                    "PlayerRule::AttackOnly(|q: &dyn Query, asker: &Asker, player: PlayerId, defender: Target| {})",
                    self.defended(defenders)?
                )
            }
            R::AttackedOnlyBy(filter) => {
                format!("PlayerRule::AttackedOnlyBy({})", self.over(filter, "attacker")?)
            }
            R::AttackWithoutTapping {
                attackers,
                condition,
            } => format!(
                "PlayerRule::AttackWithoutTapping {{ attackers: {}, when: |q: &dyn Query, asker: &Asker| {} }}",
                self.over(attackers, "attacker")?,
                self.condition(condition)?
            ),
            R::BlockerLimit(n) => format!("PlayerRule::BlockerLimit({})", self.asked(n)?),
            R::AdditionalVote { optional } => {
                format!("PlayerRule::AdditionalVote {{ optional: {optional} }}")
            }
            R::UnspentManaBecomes(None) => "PlayerRule::UnspentManaBecomes(None)".into(),
            R::UnspentManaBecomes(Some(mana)) => format!(
                "PlayerRule::UnspentManaBecomes(Some(Mana::{}))",
                color(mana)?
            ),
            R::SpendColorAs {
                color: from,
                as_color,
                others_colorless,
                on,
            } => {
                let to = match as_color {
                    Some(mana) => format!("Some(Mana::{})", color(mana)?),
                    None => "None".to_string(),
                };
                format!(
                    "PlayerRule::SpendColorAs {{ color: Mana::{}, as_color: {to}, others_colorless: {others_colorless}, on: {} }}",
                    color(from)?,
                    self.payment(on)?
                )
            }
            R::RevealDraws {
                first_during,
                optional,
            } => {
                let during = match first_during {
                    Some(players) => format!(
                        "Some(|q: &dyn Query, asker: &Asker, player: PlayerId| {})",
                        self.player_member(players)?
                    ),
                    None => "None".to_string(),
                };
                format!(
                    "PlayerRule::RevealDraws {{ first_during: {during}, optional: {optional} }}"
                )
            }
            R::CantBeCaused { action, by } => format!(
                "PlayerRule::CantBeCaused {{ action: {}, by: {} }}",
                self.prohibition(action)?,
                self.over(by, "id")?
            ),
            R::TargetsMustInclude(filter) => {
                format!("PlayerRule::TargetsMustInclude({})", self.over(filter, "id")?)
            }
            R::TargetableDespite { sources, keyword } => format!(
                "PlayerRule::TargetableDespite {{ by: |q: &dyn Query, asker: &Asker, by: &Asker, by_source: &Chars| {}, keyword: Keyword::{keyword:?} }}",
                self.targeting_source(sources)?
            ),
            R::DrawsDuringDrawStepOf(players) => format!(
                "PlayerRule::DrawsDuringDrawStepOf(|q: &dyn Query, asker: &Asker, player: PlayerId| {})",
                self.player_member(players)?
            ),
            R::UntapsDuringUntapStepOf {
                permanents,
                players,
            } => format!(
                "PlayerRule::UntapsDuringUntapStepOf {{ permanents: {}, players: |q: &dyn Query, asker: &Asker, player: PlayerId| {} }}",
                self.over(permanents, "id")?,
                self.player_member(players)?
            ),
            R::UntapOnlyOneOfTypes(types) => {
                let mut words = Vec::new();
                for word in types {
                    words.push(self.stated(word)?);
                }
                format!("PlayerRule::UntapOnlyOneOfTypes(&[{}])", words.join(", "))
            }
            R::LifeForMana { life, symbol } => format!(
                "PlayerRule::LifeForMana {{ life: {}, symbol: {} }}",
                self.asked(life)?,
                self.stated(symbol)?
            ),
            R::LifeForManaAbility { life, mana } => format!(
                "PlayerRule::LifeForManaAbility {{ life: {}, mana: {} }}",
                self.asked(life)?,
                self.stated(mana)?
            ),
            R::TapOthersForMana {
                permanents,
                modifier,
            } => format!(
                "PlayerRule::TapOthersForMana {{ permanents: {}, modifier: {} }}",
                self.over(permanents, "id")?,
                self.stated(modifier)?
            ),
            R::DiceChange {
                cost,
                limit,
                reroll,
            } => {
                let limit = match limit {
                    Some(limit) => format!("Some(PlayLimit::{limit:?})"),
                    None => "None".to_string(),
                };
                format!(
                    "PlayerRule::DiceChange {{ cost: {}, limit: {limit}, reroll: {reroll} }}",
                    self.stated(cost)?
                )
            }
            R::SurveilBonus(n) => format!("PlayerRule::SurveilBonus({})", self.asked(n)?),
            R::DevotionBonus(n) => format!("PlayerRule::DevotionBonus({})", self.asked(n)?),
            R::CascadePut { filter, flags } => format!(
                "PlayerRule::CascadePut {{ cards: {}, flags: {} }}",
                self.over(filter, "id")?,
                self.stated(flags)?
            ),
            R::AsManaEmpties { mana, then } => format!(
                "PlayerRule::AsManaEmpties {{ mana: {}, then: {} }}",
                self.stated(mana)?,
                self.stated(then)?
            ),
            R::Unique(rule) => format!("PlayerRule::{rule:?}"),
            other => return Err(gap(format!("player rule {}", form(other)))),
        })
    }
    /// A static ability's rule on players that has no table of its own yet, or
    /// that one could not take. `failed` is why, and stays the requirement.
    pub(super) fn stated_player_rule(
        &mut self,
        label: &str,
        players: &PlayerSet,
        rule: &PlayerRule,
        failed: Diagnostic,
        parts: &mut Parts,
    ) -> Result<()> {
        let saved = self.gaps.len();
        let read = self.player_rule_code(rule).and_then(|code| {
            let function = format!("{label}_applies");
            let (guard, member) = (self.guard()?, self.player_member(players)?);
            writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, player: PlayerId) -> bool {{ {guard}{member} }}").unwrap();
            Ok((function, code))
        });
        let Ok((function, code)) = read else {
            self.gaps.truncate(saved);
            return Err(failed);
        };
        self.plan_entry(
            &failed.feature,
            "statics",
            &format!("player_rule({function}, {code})"),
            parts,
        );
        Ok(())
    }
    /// A resolving effect's rule on players, which `EffectKind` will hold.
    fn stated_player_effect(
        &mut self,
        feature: &str,
        source: &ObjectRef,
        players: &PlayerSet,
        rule: &PlayerRule,
        duration: &crate::ir::Duration,
        out: &mut String,
    ) -> Result<()> {
        let code = self.player_rule_code(rule)?;
        let until = self.stated_until(duration)?;
        let players = self.fetch(self.players(players)?);
        let source = self.fetch(self.object(source)?);
        self.plan_code(
            feature,
            &format!("if let Some(source) = {source} {{ for player in {players} {{ g.add_player_effect(player, source, EffectKind::PlayerRule({code}), {until}); }} }}"),
            out,
        );
        Ok(())
    }
    /// An instruction with no code, said as the code it will have if it can be.
    /// True if it was: the hole is the requirement alone.
    pub(super) fn stated_instruction(
        &mut self,
        feature: &str,
        instruction: &Instruction,
        out: &mut String,
    ) -> bool {
        let Instruction::Effect { operation, .. } = instruction else {
            return false;
        };
        let (mark, gaps) = (out.len(), self.gaps.len());
        let stated = match operation {
            Operation::CreateContinuousEffect {
                source,
                affected: EffectSubjects::Players(players),
                changes,
                duration,
                ..
            } => {
                self.note_own_turn(players, duration);
                let made = self.stated_on_players(feature, source, players, changes, duration, out);
                self.notes.until_own_turn = None;
                made
            }
            _ => Err(gap(feature)),
        };
        if stated.is_err() {
            out.truncate(mark);
            self.gaps.truncate(gaps);
        }
        stated.is_ok()
    }
    /// A resolving effect on players that one of its rules kept from compiling:
    /// the rules that compile do, and the rest are stated.
    fn stated_on_players(
        &mut self,
        feature: &str,
        source: &ObjectRef,
        players: &PlayerSet,
        changes: &[Change],
        duration: &crate::ir::Duration,
        out: &mut String,
    ) -> Result<()> {
        for change in changes {
            let Change::PlayerRule(rule) = change else {
                return Err(gap(format!(
                    "continuous change on players {}",
                    form(change)
                )));
            };
            let id = self.fresh();
            let mark = out.len();
            let scope = self.results();
            let compiled = self.continuous_on_players(
                id,
                source,
                players,
                std::slice::from_ref(change),
                duration,
                out,
            );
            if let Err(failed) = compiled {
                out.truncate(mark);
                self.restore_results(scope);
                self.stated_player_effect(&failed.feature, source, players, rule, duration, out)?;
            }
        }
        let _ = feature;
        Ok(())
    }

    // --- What an effect gives an object.

    /// What gives an object this ability, one `AbilityRef` for each ability it
    /// comes to. Abilities that use the stack are the card's `delayed` ones; what
    /// doesn't use the stack has no such table, and waits in `granted_rules`.
    pub(super) fn granted_refs(&mut self, ability: &Ability) -> Result<Vec<String>> {
        let label = format!("granted_{}", self.fresh());
        let card = self.source_card();
        let held = self.holding.replace(Vec::new());
        let grown = self.grown_parts(ability, &label);
        let entries = std::mem::replace(&mut self.holding, held).unwrap_or_default();
        let mut grown = grown?;
        let plain = entries.is_empty()
            && grown.only_abilities()
            && !grown.abilities.is_empty()
            && grown.spell == "None"
            && grown.enchant == "None"
            && grown.protection == 0;
        if plain {
            let mut refs = Vec::new();
            for def in std::mem::take(&mut grown.abilities) {
                self.delayed.push(def);
                refs.push(format!(
                    "AbilityRef::delayed({card}, {})",
                    self.delayed.len() - 1
                ));
            }
            return Ok(refs);
        }
        for (list, entry) in entries {
            grown.entry(list, entry);
        }
        if grown.spell != "None" {
            let spell = grown.spell.clone();
            grown.field("spell", spell);
        }
        if grown.enchant != "None" {
            let enchant = grown.enchant.clone();
            grown.field("enchant", enchant);
        }
        if grown.protection != 0 {
            let colors = format!("ColorSet({})", grown.protection);
            grown.field("protection", colors);
        }
        let enters = self.enters(&grown.enters);
        let rules = grown.card(&enters);
        if let Some(granted) = self.granted_attack_limit(&rules) {
            return Ok(vec![granted]);
        }
        let index = self.granted_rules;
        self.granted_rules += 1;
        let feature = lane_g::grant_gap(ability).feature;
        let mut late = Parts::default();
        self.plan_entry(&feature, "granted_rules", &rules, &mut late);
        self.late
            .extend(late.tables.remove("granted_rules").unwrap_or_default());
        Ok(vec![planned::planned_value(
            &feature,
            &format!("AbilityRef::delayed({card}, 0)"),
            &format!("AbilityRef::granted_static({card}, {index})"),
        )])
    }
    /// A granted "No more than one creature can attack this planeswalker each
    /// combat" (CR 508.1c), as Tomik, Orzhov Lawmage prints it: the grant is a
    /// static ability that changes nothing of the object, and the card's own
    /// attack limit asks whether what is attacked has it. The limit was written
    /// as the object's own, so it is asked as the object.
    fn granted_attack_limit(&mut self, rules: &str) -> Option<String> {
        const EMPTY: &str = "CardDef { keywords: Keywords::of(&[]), abilities: &[], statics: &[], replacements: &[], preventions: &[], block_rules: &[], block_requirements: &[], attack_rules: &[], untap_bans: &[], enters: None, attack_limits: &[";
        let limit = rules.strip_prefix(EMPTY)?.strip_suffix("], ..CardDef::BLANK }")?;
        if limit.is_empty() || !limit.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return None;
        }
        let line = self.line_text()?;
        let (open, close) = (line.find('"')?, line.rfind('"')?);
        let text = quoted(line.get(open + 1..close).filter(|text| !text.is_empty())?);
        let card = self.source_card();
        writeln!(self.functions, "#[allow(unused_variables)] fn {limit}_changes_nothing(q: &dyn LayerQuery, source: ObjectId, chars: &mut Chars) {{}}").unwrap();
        self.delayed.push(format!(
            "granted_static(Layer::Ability, {limit}_changes_nothing, {text})"
        ));
        let index = self.delayed.len() - 1;
        writeln!(self.functions, "#[allow(unused_variables)] fn {limit}_held(q: &dyn Query, asker: &Asker, attacked: Target) -> Option<u32> {{ let Target::Object(id) = attacked else {{ return None; }}; let source = q.obj(asker.source); let granted = AbilityRef::delayed(source.copied_card().unwrap_or(source.card), {index}); if !q.chars(id).abilities.contains(&Some(granted)) {{ return None; }} {limit}(q, &Asker::new(id, q.controller(id)), attacked) }}").unwrap();
        self.late_tables.push(("attack_limits", format!("{limit}_held")));
        Some(format!("AbilityRef::delayed({card}, {index})"))
    }
    /// The statement that gives a layer's `chars` these abilities.
    pub(super) fn grant_in_layer(&mut self, ability: &Ability) -> Result<statics::LayerChange> {
        if let Some(n) = lane_n::toxic_of(ability) {
            return Ok(statics::LayerChange::of("Ability", format!("chars.toxic += {n};")));
        }
        let card = self.source_card();
        let mut body = format!("let card = {card}; let _ = card;");
        for reference in self.granted_refs(ability)? {
            write!(body, " if let Some(slot) = chars.abilities.iter().position(|a| a.is_none()) {{ chars.abilities[slot] = Some({reference}); }}").unwrap();
        }
        Ok(statics::LayerChange::of("Ability", body))
    }
    /// The effects that give an object this ability.
    pub(super) fn grant_kinds(&mut self, ability: &Ability) -> Result<Vec<String>> {
        if let Some(n) = lane_n::toxic_of(ability) {
            return Ok(vec![format!("EffectKind::Toxic({n})")]);
        }
        // "It's an Aura with enchant creature": an enchant ability a card's effect
        // gives is `CardDef::becomes_aura_enchant` (CR 303.4, 613.1f).
        if let Ability::Enchant { hosts } = ability {
            return self.aura_enchant(hosts);
        }
        Ok(self
            .granted_refs(ability)?
            .into_iter()
            .map(|reference| format!("EffectKind::AddAbility({reference})"))
            .collect())
    }
    pub(super) fn aura_enchant(&mut self, hosts: &EnchantHosts) -> Result<Vec<String>> {
        let EnchantHosts::Permanents(filter) = hosts else {
            return Err(gap("granted Enchant"));
        };
        // Necromancy puts its creature in with it, a move this reading can't say.
        if format!("{filter:?}").contains("MadeBy") {
            return Err(gap("an Aura that enchants what it put onto the battlefield"));
        }
        let function = format!("becomes_aura_{}", self.fresh());
        let test = self.queried(|c| c.filter(filter, &Candidate::of("id")))?;
        writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, t: Target) -> bool {{ matches!(t, Target::Object(id) if cantrip_mechanics::targets::permanent(q, id) && ({test})) }}").unwrap();
        self.aura = Some(format!("Some(target({function}))"));
        Ok(vec![format!("EffectKind::Enchant({})", self.source_card())])
    }

    // --- The rest of what an effect changes.

    /// `(from, to)`: the words of a text change, as the choice that named them.
    fn replaced_words(&self, from: &Operand, to: &Operand) -> Result<(String, String)> {
        let word = |c: &Self, operand: &Operand| -> Result<String> {
            let (Operand::Vocabulary { form, .. } | Operand::Term { form, .. }) = operand else {
                return Err(gap("a word that isn't named"));
            };
            Ok(match form.as_str() {
                "TheFirstChosenColorWord" => format!("{}.0", c.chosen_local(ValueKind::ColorWord)?),
                "TheSecondChosenColorWord" => {
                    format!("{}.1", c.chosen_local(ValueKind::ColorWord)?)
                }
                "TheFirstChosenLandType" => {
                    format!("{}.0", c.chosen_local(ValueKind::BasicLandType)?)
                }
                "TheSecondChosenLandType" => {
                    format!("{}.1", c.chosen_local(ValueKind::BasicLandType)?)
                }
                "TheChosenCreatureType" => format!(
                    "Word::Subtype({})",
                    c.chosen_local(ValueKind::CreatureType)?
                ),
                word if crate::cantrip_vocab::SUBTYPES.contains(&word) => {
                    format!("Word::Subtype(Subtype::{word})")
                }
                other => return Err(gap(format!("the word {other}"))),
            })
        };
        Ok((word(self, from)?, word(self, to)?))
    }
    /// "One color word with another": the pair `text_change` asks for, chosen as
    /// one (CR 612.2): the second can't be the first.
    pub(super) fn chosen_words(
        &mut self,
        id: usize,
        actor: &PlayerSet,
        kind: ValueKind,
        count: u32,
        out: &mut String,
    ) -> Result<bool> {
        let (2, PlayerSet::One(player), true) = (count, actor, self.stack_item) else {
            return Ok(false);
        };
        let asked = match kind {
            ValueKind::ColorWord => "color_word",
            ValueKind::BasicLandType => "basic_land_type",
            _ => return Ok(false),
        };
        let player = self.fetch(self.certain_player(player)?);
        let local = format!("words_{id}");
        writeln!(out, "let player_{id} = {player}; let {local} = {M}::text_change::{asked}(g, player_{id}, asker.source)?;").unwrap();
        self.chosen.push((kind, local));
        Ok(true)
    }
    /// The word `Lost` will have for an ability the engine has no keyword for.
    fn lost_word(&self, lost: &Operand) -> Result<String> {
        let Operand::Term {
            form: word,
            operands,
            ..
        } = lost
        else {
            return Err(gap(format!("losing {}", form(lost))));
        };
        Ok(match operands.as_slice() {
            [] => format!("Lost::{word}"),
            [Operand::Objects(ObjectSet::Battlefield(ObjectFilter::Subtype(land)))] => {
                format!("Lost::{word}(Subtype::{})", name(land))
            }
            [Operand::Vocabulary { family, form }] => format!("Lost::{word}({family}::{form})"),
            _ => return Err(gap(format!("losing ability {word}"))),
        })
    }
    /// A resolving effect that takes away what isn't a keyword of the engine's.
    pub(super) fn lost_kinds(&mut self, lost: &Operand, failed: Diagnostic) -> Result<Vec<String>> {
        if let Operand::Ability(ability) = lost {
            let mut kinds = Vec::new();
            for each in self.granted_refs(ability).map_err(|_| failed.clone())? {
                kinds.push(lane_g::planned_kind(
                    &failed.feature,
                    format!("EffectKind::LosesAbility(Lost::SameAs({each}))"),
                ));
            }
            return Ok(kinds);
        }
        let Operand::Term { form: word, .. } = lost else {
            return Err(failed);
        };
        if let Some(lost) = lost_keyword(lost) {
            return Ok(vec![format!("EffectKind::LoseKeywords(Keywords::of(&[{lost}]))")]);
        }
        match word.as_str() {
            // The enchant ability is the engine's to take.
            "Enchant" => Ok(vec![format!(
                "EffectKind::LoseEnchant({})",
                self.source_card()
            )]),
            // "Loses first strike or swampwalk": the keyword chosen.
            "TheChosenAbility" if self.chosen_local(ValueKind::Ability).is_ok() => {
                let local = self
                    .chosen_local(ValueKind::Ability)
                    .map_err(|_| failed.clone())?;
                let options = self.ability_choice.clone();
                let last = options.len().saturating_sub(1);
                let mut arms = Vec::new();
                for (i, ability) in options.iter().enumerate() {
                    let Ability::Keyword(lost) = ability else {
                        return Err(failed);
                    };
                    let lost = keyword(lost).map_err(|_| failed.clone())?;
                    let pattern = if i == last {
                        "_".to_string()
                    } else {
                        i.to_string()
                    };
                    arms.push(format!(
                        "{pattern} => EffectKind::LoseKeywords(Keywords::of(&[{lost}])),"
                    ));
                }
                Ok(vec![format!("match {local} {{ {} }}", arms.join(" "))])
            }
            _ => Ok(vec![lane_g::planned_kind(
                &failed.feature,
                format!(
                    "EffectKind::LosesAbility({})",
                    self.lost_word(lost).map_err(|_| failed.clone())?
                ),
            )]),
        }
    }
    /// A static ability that takes away what isn't a keyword of the engine's.
    pub(super) fn lost_layer(
        &mut self,
        lost: &Operand,
        failed: Diagnostic,
    ) -> Result<statics::LayerChange> {
        if let Some(lost) = lost_keyword(lost) {
            let and = match lost.as_str() {
                "Keyword::Hexproof" => " chars.hexproof_from = HexproofFrom::NONE;",
                _ => "",
            };
            return Ok(statics::LayerChange::of(
                "Ability",
                format!("chars.keywords = Keywords(chars.keywords.0 & !Keywords::of(&[{lost}]).0);{and}"),
            ));
        }
        let word = self.lost_word(lost).map_err(|_| failed.clone())?;
        Ok(statics::LayerChange::of(
            "Ability",
            format!(
                "{};",
                planned::planned_value(&failed.feature, "()", &format!("chars.lose({word})"))
            ),
        ))
    }
    /// The word for each kind of ability one `GainAbilitiesOf` reaches.
    fn ability_kinds(&mut self, kinds: &Operand) -> Result<String> {
        let kinds = match kinds {
            Operand::List(_) => kinds.clone(),
            other => Operand::List(vec![other.clone()]),
        };
        self.stated(&kinds)
    }
    fn abilities_of(&mut self, change: &Change, ids: String) -> Result<String> {
        let Change::GainAbilitiesOf {
            kinds,
            once_each_turn,
            spend_as: spending,
            ..
        } = change
        else {
            return Err(gap("abilities of what"));
        };
        let kinds = self.ability_kinds(kinds)?;
        let spending = match spending {
            Some(spending) => format!("Some({})", spend_as(spending)?),
            None => "None".into(),
        };
        Ok(format!("AbilitiesOf {{ objects: {ids}, kinds: {kinds}, once_each_turn: {once_each_turn}, spend_as: {spending} }}"))
    }
    /// A static ability "has all abilities of ..." that `gain_abilities_of` can't
    /// read: `failed` stays the requirement.
    pub(super) fn gained_layer(
        &mut self,
        change: &Change,
        failed: Diagnostic,
    ) -> Result<statics::LayerChange> {
        let Change::GainAbilitiesOf { objects, .. } = change else {
            return Err(failed);
        };
        let read = self.ids_in_layer(objects).and_then(|ids| {
            let ids = format!("{{ let ids: Vec<ObjectId> = {ids}; ids }}");
            self.abilities_of(change, ids)
        });
        let Ok(abilities) = read else {
            return Err(failed);
        };
        Ok(statics::LayerChange::of(
            "Ability",
            format!(
                "{};",
                planned::planned_value(
                    &failed.feature,
                    "()",
                    &format!("chars.gain_abilities_of({abilities})")
                )
            ),
        ))
    }
    /// A chosen color or type past what `chosen_layer` has.
    pub(super) fn chosen_more(&self, change: &Change) -> Result<statics::LayerChange> {
        use statics::LayerChange as L;
        let failed = || gap(format!("static change {}", form(change)));
        Ok(match change {
            Change::ChosenColor { mode: ChangeMode::Add } => L::of(
                "Color",
                format!(
                    "if let Some(chosen) = {} {{ chars.colors = chars.colors.with(chosen); }}",
                    self.chosen_value(ValueKind::Color).map_err(|_| failed())?
                ),
            ),
            // The chosen land type replaces the others and their rules text (CR 305.7).
            Change::ChosenType {
                kind: ValueKind::LandType | ValueKind::BasicLandType,
                mode: ChangeMode::Set,
            } => {
                let chosen = self
                    .chosen_value(ValueKind::BasicLandType)
                    .or_else(|_| self.chosen_value(ValueKind::LandType))
                    .map_err(|_| failed())?;
                L::of_land(format!("if let Some(chosen) = {chosen} {{ chars.set_land_subtypes(SubtypeSet::NONE.with(chosen)); }}"))
            }
            Change::ChosenType {
                kind: ValueKind::LandType | ValueKind::BasicLandType,
                mode: ChangeMode::Add,
            } => {
                let chosen = self
                    .chosen_value(ValueKind::BasicLandType)
                    .or_else(|_| self.chosen_value(ValueKind::LandType))
                    .map_err(|_| failed())?;
                L::of_land(format!("if let Some(chosen) = {chosen} {{ chars.subtypes = chars.subtypes.with(chosen); }}"))
            }
            Change::ChosenType {
                kind: ValueKind::CardName,
                mode: ChangeMode::Set,
            } => L::of(
                "Text",
                format!(
                    "{};",
                    planned::planned_value(
                        &failed().feature,
                        "()",
                        "if let Some(chosen) = q.chosen_name(q.linked_to(source)) { chars.set_name(chosen); }",
                    )
                ),
            ),
            _ => return Err(failed()),
        })
    }
    /// The rest of what a resolving effect can change.
    pub(super) fn more_kinds(&mut self, change: &Change) -> Result<Vec<String>> {
        Ok(match change {
            Change::ReplaceWord { from, to } => {
                let (from, to) = self
                    .replaced_words(from, to)
                    .map_err(|_| gap("continuous change ReplaceWord"))?;
                vec![format!(
                    "EffectKind::ChangeText {{ from: {from}, to: {to} }}"
                )]
            }
            Change::RemoveAllLandTypes => vec!["EffectKind::LoseSubtypesOf(CardType::Land)".into()],
            Change::LandTypes {
                scope: LandTypeScope::Basic,
                mode: ChangeMode::Add,
            } => ["Plains", "Island", "Swamp", "Mountain", "Forest"]
                .iter()
                .map(|land| format!("EffectKind::AddSubtype(Subtype::{land})"))
                .collect(),
            Change::AddEnchantmentType(kind) => match &**kind {
                Operand::Vocabulary { form, .. } if form == "Aura" => {
                    vec!["EffectKind::AddSubtype(Subtype::Aura)".into()]
                }
                other => return Err(gap(format!("enchantment type {}", super::form(other)))),
            },
            // One effect: it loses the others, then has this (CR 613.1f).
            Change::OnlyAbility(ability) => {
                let mut kinds = vec!["EffectKind::LoseAllAbilities".to_string()];
                kinds.extend(self.effect_kinds(&grant_of(ability))?);
                kinds
            }
            Change::OnlyAbilities(abilities) => {
                let mut kinds = vec!["EffectKind::LoseAllAbilities".to_string()];
                for ability in abilities {
                    kinds.extend(self.effect_kinds(&grant_of(ability))?);
                }
                kinds
            }
            Change::GrantAbilityIfAbsent(ability) => {
                let feature = "continuous change GrantAbilityIfAbsent";
                let mut kinds = Vec::new();
                for reference in self.granted_refs(ability)? {
                    kinds.push(lane_g::planned_kind(
                        feature,
                        format!("EffectKind::AddAbilityIfAbsent({reference})"),
                    ));
                }
                kinds
            }
            // "... gain flying until end of turn if a creature you control has
            // flying. The same is true for ...": read now, each keyword its own.
            Change::GainAbilitiesOf {
                objects,
                kinds,
                once_each_turn: false,
                spend_as: None,
            } if lane_n::plain_keywords(kinds).is_some() => {
                let words = lane_n::plain_keywords(kinds).expect("checked");
                vec![format!(
                    "{{ let those = {}; let had: Vec<Keyword> = [{}].into_iter().filter(|k: &Keyword| those.iter().any(|&o: &ObjectId| g.chars(o).has(*k))).collect(); EffectKind::AddKeywords(Keywords::of(&had)) }}",
                    self.objects(objects)?,
                    words.join(", ")
                )]
            }
            Change::GainAbilitiesOf { objects, .. } => {
                let ids = self.fetch(self.objects(objects)?);
                let abilities = self.abilities_of(change, ids)?;
                vec![lane_g::planned_kind(
                    "continuous change GainAbilitiesOf",
                    format!("EffectKind::AddAbilitiesOf({abilities})"),
                )]
            }
            // The instruction before it added the mana; nothing else is read.
            Change::ColorOfMana { mode, .. } => vec![lane_g::planned_kind(
                "continuous change ColorOfMana",
                format!("EffectKind::ColorOfMana(ChangeMode::{mode:?}, g.mana_added_by(asker))"),
            )],
            other => return lane_m::continuous_more(other),
        })
    }
    /// The rest of what a static ability can change.
    pub(super) fn more_layers(&mut self, change: &Change) -> Result<statics::LayerChange> {
        use statics::LayerChange as L;
        let feature = format!("static change {}", form(change));
        let planned = |code: String| format!("{};", planned::planned_value(&feature, "()", &code));
        Ok(match change {
            Change::RemoveAllLandTypes => L::of(
                "Type",
                "chars.subtypes = chars.subtypes.minus(SubtypeSet::LAND_TYPES);".into(),
            ),
            Change::LandTypes {
                scope: LandTypeScope::Basic,
                mode: ChangeMode::Add,
            } => L::of(
                "Type",
                "chars.subtypes = chars.subtypes.with(Subtype::Plains).with(Subtype::Island).with(Subtype::Swamp).with(Subtype::Mountain).with(Subtype::Forest);".into(),
            ),
            // "Every land type in addition to its other types".
            Change::LandTypes {
                scope: LandTypeScope::All,
                mode: ChangeMode::Add,
            } => L::of(
                "Type",
                "chars.subtypes = chars.subtypes.union(SubtypeSet::LAND_TYPES);".into(),
            ),
            Change::LandTypes {
                scope: LandTypeScope::Nonbasic,
                mode: ChangeMode::Add,
            } => L::of(
                "Type",
                "chars.subtypes = chars.subtypes.union(SubtypeSet::LAND_TYPES.minus(SubtypeSet::of(&[Subtype::Plains, Subtype::Island, Subtype::Swamp, Subtype::Mountain, Subtype::Forest])));".into(),
            ),
            // Being all of them leaves nothing for "in addition" to add.
            Change::CreatureTypes {
                mode: ChangeMode::Add,
                types: CreatureTypeSet::All,
            } => L::of(
                "Type",
                "if chars.is(CardType::Creature) || chars.is(CardType::Kindred) { chars.subtypes = chars.subtypes.union(SubtypeSet::CREATURE_TYPES); }".into(),
            ),
            Change::CreatureTypesOf { object, mode } => {
                let object = self.object(object)?;
                L::of(
                    "Type",
                    planned(format!("chars.creature_types_of(ChangeMode::{mode:?}, {object}.map(|id| q.chars(id).subtypes))")),
                )
            }
            Change::AddNames(filter) => {
                let names = self.over(filter, "id")?;
                L::of("Text", planned(format!("chars.add_names_of({names})")))
            }
            Change::ColorlessDamageSource => {
                L::of("Color", planned("chars.colorless_damage_source = true".into()))
            }
            Change::SetController(player) => {
                let player = self.player(player)?;
                L::of("Control", planned(format!("chars.controller = {player}")))
            }
            Change::ChosenAbility => {
                let kept = self.chosen_keywords()?;
                L::of("Ability", planned(format!("chars.keywords = chars.keywords.with_all({kept})")))
            }
            other => return Err(gap(format!("static change {}", form(other)))),
        })
    }
    /// "Is a copy of ...", "has the full text of ...": layer 1, which no static
    /// ability has yet (CR 613.1a, 707.8a). False for any other static ability.
    pub(super) fn copy_static(
        &mut self,
        label: &str,
        affected: &ObjectSet,
        changes: &[Change],
        parts: &mut Parts,
    ) -> Result<bool> {
        let [change] = changes else {
            return Ok(false);
        };
        let (object, more) = match change {
            Change::CopyOf { object, effects } => (object, effects),
            Change::TextOf { object, plus } => (object, plus),
            _ => return Ok(false),
        };
        let feature = format!("static change {}", form(change));
        let (test, object, more) = self
            .queried(|c| {
                let (guard, test) = (c.guard()?, c.rule_subject(affected, "id")?);
                Ok((format!("{guard}{test}"), c.object(object)?, c.stated(more)?))
            })
            .map_err(|_| gap(&feature))?;
        let function = format!("{label}_copies");
        writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, id: ObjectId) -> bool {{ let source = asker.source; {test} }}").unwrap();
        let what = match change {
            Change::CopyOf { .. } => "copy_of",
            _ => "full_text_of",
        };
        self.plan_entry(
            &feature,
            "statics",
            &format!("{what}({function}, |q: &dyn Query, asker: &Asker| {object}, {more})"),
            parts,
        );
        Ok(true)
    }
    /// What is chosen as this entered, from among abilities: those of them chosen.
    fn chosen_keywords(&self) -> Result<String> {
        let words = self
            .ability_choice
            .iter()
            .map(|ability| match ability {
                Ability::Keyword(word) => keyword(word),
                other => Err(gap(format!("the ability {}", form(other)))),
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(format!(
            "q.chosen_abilities(asker.source, &[{}])",
            words.join(", ")
        ))
    }

    /// Whether a candidate is among objects no filter names.
    pub(super) fn stated_subject(&self, set: &ObjectSet, c: &Candidate) -> Result<String> {
        let id = &c.id;
        Ok(match set {
            ObjectSet::Union(sets) => {
                let mut tests = Vec::new();
                for each in sets {
                    tests.push(match each {
                        ObjectSet::Battlefield(filter) => self.filter(filter, c)?,
                        other => match self.outside_subject(other, c)? {
                            Some(test) => test,
                            None => self.stated_subject(other, c)?,
                        },
                    });
                }
                format!("({})", tests.join(" || "))
            }
            // The object chosen as the source entered, linked to this ability
            // (CR 607.2a): one that left and came back is a new object (CR 400.7).
            ObjectSet::One(ObjectRef::Chosen) => {
                format!("q.state().chosen_object({}) == Some({id})", self.linked())
            }
            ObjectSet::Matching { .. } | ObjectSet::Take { .. } => planned::planned_value(
                &format!("static subjects {}", form(set)),
                "false",
                &format!("({}).contains(&{id})", self.objects(set)?),
            ),
            other => return Err(gap(format!("static subjects {}", form(other)))),
        })
    }

    /// `Option<ObjectId>`: an object nothing else reads.
    pub(super) fn object_stated(&self, object: &ObjectRef) -> Result<String> {
        let feature = format!("object reference {}", form(object));
        let code = match object {
            // The spell or card a rule on many of them is being asked about.
            ObjectRef::Scoped => "q.scoped_object(asker)",
            // A choice as the source entered (CR 607.2).
            ObjectRef::Chosen => "q.chosen_object(asker)",
            _ => return Err(gap(feature)),
        };
        Ok(planned::planned_value(&feature, "None::<ObjectId>", code))
    }
    /// `Vec<ObjectId>`: objects nothing else reads.
    pub(super) fn objects_stated(&self, objects: &ObjectSet) -> Result<String> {
        let feature = format!("object collection {}", form(objects));
        let code = match objects {
            ObjectSet::MovedBy { result, player, .. } => {
                format!("q.moved_by(asker, {}, {})", result.0, self.player(player)?)
            }
            ObjectSet::Earlier {
                what,
                by: Some(players),
            } => format!(
                "q.earlier_objects_of(asker, {}, EarlierObjects::{what:?})",
                self.players(players)?
            ),
            _ => return Err(gap(feature)),
        };
        Ok(planned::planned_value(
            &feature,
            "Vec::<ObjectId>::new()",
            &code,
        ))
    }

    // --- Players with no read yet.

    /// A reference to a player nothing else reads.
    pub(super) fn player_stated(&self, player: &PlayerRef) -> Result<String> {
        let code = match player {
            PlayerRef::Affected if self.rule_player => return Ok("Some(player)".into()),
            PlayerRef::FirstWilling(players) => return self.willing_player(players),
            PlayerRef::ModeChooser => "q.mode_chooser(asker)".to_string(),
            PlayerRef::ClashOpponent => "q.clash_opponent(asker)".to_string(),
            PlayerRef::Acting => "q.acting_player(asker)".to_string(),
            PlayerRef::DefendingPlayer => "q.defending_player_now()".to_string(),
            PlayerRef::Bound(_) => "q.remembered_player(asker)".to_string(),
            PlayerRef::Performer(result) => format!("q.performer_of(asker, {})", result.0),
            PlayerRef::Named { result, index } => {
                format!("q.named_player(asker, {}, {index})", result.0)
            }
            PlayerRef::InChosenDirection { from, among } => {
                let from = match from {
                    Some(from) => self.player(from)?,
                    None if self.rule_player => "Some(player)".to_string(),
                    None => "Some(asker.controller)".to_string(),
                };
                match among {
                    Some(among) => format!(
                        "q.nearest_in_chosen_direction(asker, {from}, {})",
                        self.players(among)?
                    ),
                    None => format!("q.next_in_chosen_direction(asker, {from})"),
                }
            }
            PlayerRef::NotedFor(object) => {
                format!("q.noted_for(asker, {})", self.object(object)?)
            }
            PlayerRef::Prior(prior) => format!("q.prior_player(asker, PriorPlayer::{prior:?})"),
            PlayerRef::OtherChosen(of) if self.enters_two == Some(ValueKind::Player) => {
                return Ok(format!(
                    "{}.and_then(|p: PlayerId| {{ let (first, second) = (q.state().chosen_player({linked}), q.state().chosen_number({linked}).map(|n| PlayerId(n as u8))); if first == Some(p) {{ second }} else if second == Some(p) {{ first }} else {{ None }} }})",
                    self.player(of)?,
                    linked = self.linked()
                ));
            }
            PlayerRef::OtherChosen(of) => format!(
                "{}.and_then(|p: PlayerId| q.other_chosen_player(asker, p))",
                self.player(of)?
            ),
            PlayerRef::SoleTargetOf(of) => format!(
                "{}.and_then(|o: ObjectId| q.sole_target_player(o))",
                self.object(of)?
            ),
            other => return Err(gap(format!("player reference {}", form(other)))),
        };
        Ok(planned::planned_value(
            &format!("player reference {}", form(player)),
            "None::<PlayerId>",
            &code,
        ))
    }
    /// "Any player may": asked in turn order as the instruction begins, and the
    /// first who agrees is who it reads (CR 101.4).
    fn willing_player(&self, players: &PlayerSet) -> Result<String> {
        const FEATURE: &str = "player reference FirstWilling";
        let key = format!("willing {players:?}");
        if let Some((_, local)) = self.picked.borrow().named.iter().find(|(k, _)| *k == key) {
            return Ok(local.clone());
        }
        if !self.stack_item {
            return Ok(planned::planned_value(
                FEATURE,
                "None::<PlayerId>",
                &format!("q.first_willing_for(asker, {})", self.players(players)?),
            ));
        }
        if self.present.borrow().is_none() {
            return Err(gap(FEATURE));
        }
        let Some((ask, detail)) = self.lines_said().iter().find_map(|line| any_player_may(line))
        else {
            return Err(gap(FEATURE));
        };
        let opponents = self
            .lines_said()
            .iter()
            .any(|line| line.to_lowercase().contains("any opponent may"));
        let among = match opponents {
            // The source says "any player" of these. In turn order either way.
            true => self.fetch(self.players(&PlayerSet::OpponentsOf(PlayerRef::AbilityController))?),
            false => self.fetch(self.players(players)?),
        };
        let mut picked = self.picked.borrow_mut();
        let local = format!("willing_{}", picked.named.len());
        picked.statements.push(format!(
            "let {local}: Option<PlayerId> = {{ static ASK: Question = Question::new(\"Accept\", {}, {}, \"Yes\", \"No\"); let among = {among}; let order: Vec<PlayerId> = g.state().apnap().filter(|p| among.contains(p)).collect(); let mut willing = None; for player in order {{ if {M}::effects::ask_may(g, player, asker.source, Purpose::Ask(&ASK))? {{ willing = Some(player); break; }} }} willing }};\n",
            quoted(&ask),
            quoted(&detail),
        ));
        picked.named.push((key, local.clone()));
        Ok(local)
    }
    /// "An opponent" where nothing is resolving to ask: the player picked as the
    /// ability is announced or as the permanent enters (CR 601.2c, 614.12).
    pub(super) fn picked_elsewhere(&self, players: &PlayerSet) -> Result<String> {
        Ok(planned::planned_value(
            "player reference OneOf",
            "asker.controller",
            &format!("q.player_picked_for(asker, {})", self.players(players)?),
        ))
    }
    /// A choice made by another ability of the source and read here (CR 607.2),
    /// which the engine keeps only as the source enters.
    pub(super) fn chosen_elsewhere(&self, kind: ValueKind) -> Result<String> {
        let feature = format!("a choice of {kind:?} outside its body");
        let (word, placeholder) = match kind {
            ValueKind::Player => ("player", "None::<PlayerId>"),
            ValueKind::Color => ("color", "None::<Color>"),
            ValueKind::CreatureType | ValueKind::BasicLandType | ValueKind::LandType => {
                ("subtype", "None::<Subtype>")
            }
            ValueKind::Number => ("number", "None::<u32>"),
            _ => return Err(gap(feature)),
        };
        Ok(planned::planned_value(
            &feature,
            placeholder,
            &format!("q.chosen_{word}_by(asker)"),
        ))
    }

    // --- Costs with no field yet. Each is read as `|q, asker|`, whatever pays it.

    fn count_code(&self, count: &Count) -> Result<String> {
        Ok(match count {
            Count::Exactly(n) | Count::UpTo(n) | Count::AtLeast(n) | Count::AsManyAsCan(n) => {
                format!("Count::{}({})", form(count), self.asked(n)?)
            }
            Count::AnyNumber => "Count::AnyNumber".into(),
            Count::Between { minimum, maximum } => format!(
                "Count::Between({}, {})",
                self.asked(minimum)?,
                self.asked(maximum)?
            ),
            other => return Err(gap(format!("count {}", form(other)))),
        })
    }
    /// What a cost part picks from: a test where the set is one, or else a list.
    fn pick_from(&self, objects: &ObjectSet) -> Result<String> {
        match self.rule_subject(objects, "id") {
            Ok(test) => Ok(format!(
                "Picks::Matching(|q: &dyn Query, asker: &Asker, id: ObjectId| {test})"
            )),
            Err(_) => Ok(format!(
                "Picks::These(|q: &dyn Query, asker: &Asker| {})",
                self.objects(objects)?
            )),
        }
    }
    fn selection(&self, selection: &CostObjectSelection) -> Result<String> {
        let count = match &selection.choose {
            Some(count) => self.count_code(count)?,
            None => "Count::Each".to_string(),
        };
        Ok(format!(
            "Selection {{ objects: {}, count: {count} }}",
            self.pick_from(&selection.objects)?
        ))
    }
    fn selections(&self, selections: &[CostObjectSelection]) -> Result<String> {
        let mut read = Vec::new();
        for selection in selections {
            read.push(self.selection(selection)?);
        }
        Ok(format!("&[{}]", read.join(", ")))
    }
    fn stated_destination(&self, destination: &Destination) -> Result<String> {
        Ok(match destination {
            Destination::Battlefield(player) => {
                format!("Destination::Battlefield({})", self.player(player)?)
            }
            Destination::Hand(player) => format!("Destination::Hand({})", self.player(player)?),
            Destination::OwnersLibraryFromTop(n) => {
                format!("Destination::OwnersLibraryFromTop({})", self.asked(n)?)
            }
            Destination::OwnersPlanarDeck(end) => {
                format!("Destination::OwnersPlanarDeck(SequenceEnd::{end:?})")
            }
            other => format!("Destination::{}", form(other)),
        })
    }
    /// How a moved thing arrives, for the plain ways it can.
    fn moved_arrival(&self, entry: &EntrySpec, face_down: bool) -> Result<String> {
        if !entry.counters.is_empty() {
            return Err(gap("cost that moves with counters"));
        }
        let mut states = Vec::new();
        for state in &entry.states {
            match state {
                EntryState::ExilePile { shuffled } => {
                    states.push(format!("EntryState::ExilePile {{ shuffled: {shuffled} }}"))
                }
                other => return Err(gap(format!("cost that moves into {}", form(other)))),
            }
        }
        Ok(format!(
            "Arrival {{ tapped: {}, face_down: {face_down}, states: &[{}] }}",
            entry.tapped,
            states.join(", ")
        ))
    }
    fn counters_kind(&self, kind: Option<&crate::counter::CounterKind>) -> Result<String> {
        Ok(match kind {
            Some(kind) => format!("Some({})", counter_kind(kind)?),
            None => "None".into(),
        })
    }
    fn removal(&self, removal: &CounterRemoval) -> Result<String> {
        let Recipients::Objects(objects) = &removal.recipients else {
            return Err(gap("counters removed from what"));
        };
        let amount = match &removal.amount {
            CounterAmount::Count(count) => format!("Removed::Count({})", self.count_code(count)?),
            CounterAmount::All => "Removed::All".to_string(),
            CounterAmount::AmongThem(count) => {
                format!("Removed::AmongThem({})", self.count_code(count)?)
            }
        };
        let chosen = match &removal.selection {
            Some(selection) => format!("Some({})", self.count_code(&selection.count)?),
            None => "None".to_string(),
        };
        Ok(format!(
            "Removal {{ from: {}, kind: {}, amount: {amount}, among: {chosen} }}",
            self.pick_from(objects)?,
            self.counters_kind(removal.kind.as_ref())?
        ))
    }
    fn placement(&self, placement: &CounterPlacement) -> Result<String> {
        let Recipients::Objects(objects) = &placement.recipients else {
            return Err(gap("counters put on what"));
        };
        Ok(format!(
            "Placement {{ on: {}, kind: {}, amount: {} }}",
            self.pick_from(objects)?,
            counter_kind(&placement.kind)?,
            self.asked(&placement.amount)?
        ))
    }
    fn selection_rule(&self, constraint: &SelectionConstraint) -> Result<String> {
        Ok(match constraint {
            SelectionConstraint::TotalProperty {
                property,
                comparison,
                amount,
            } => format!(
                "SelectionRule::TotalProperty(Property::{property:?}, {}, {})",
                comparing(comparison),
                self.asked(amount)?
            ),
            SelectionConstraint::DistinctCount {
                property,
                comparison,
                amount,
            } => format!(
                "SelectionRule::DistinctCount(Property::{property:?}, {}, {})",
                comparing(comparison),
                self.asked(amount)?
            ),
            SelectionConstraint::TotalManaSymbols {
                color: symbol,
                comparison,
                amount,
            } => format!(
                "SelectionRule::TotalManaSymbols(Color::{}, {}, {})",
                color(symbol)?,
                comparing(comparison),
                self.asked(amount)?
            ),
            other => format!("SelectionRule::{}", form(other)),
        })
    }
    /// The cost a part will be, once `Cost` can say it.
    fn cost_part_code(&mut self, part: &CostPart, label: &str) -> Result<String> {
        use CostPart as P;
        Ok(match part {
            P::ManaCostOf(object) => format!(
                "Cost::mana_cost_of(|q: &dyn Query, asker: &Asker| {})",
                self.object(object)?
            ),
            P::Move {
                selections,
                destination,
                entry,
                face_down,
            } => format!(
                "Cost::moves({}, {}, {})",
                self.selections(selections)?,
                self.stated_destination(destination)?,
                self.moved_arrival(entry, *face_down)?
            ),
            P::Choice(options) => {
                let mut costs = Vec::new();
                for (i, option) in options.iter().enumerate() {
                    costs.push(self.cost(option, &format!("{label}_option_{i}"))?);
                }
                format!("Cost::one_of(&[{}])", costs.join(", "))
            }
            P::RemoveCounters { removals } => {
                let mut read = Vec::new();
                for removal in removals {
                    read.push(self.removal(removal)?);
                }
                format!("Cost::removes_counters(&[{}])", read.join(", "))
            }
            P::Life(n) => format!(
                "Cost::life(|q: &dyn Query, asker: &Asker| {})",
                self.asked(n)?
            ),
            P::Repeated { part, times } => {
                let once = Cost {
                    parts: vec![(**part).clone()],
                };
                format!(
                    "Cost::repeated({}, |q: &dyn Query, asker: &Asker| {})",
                    self.cost(&once, label)?,
                    self.asked(times)?
                )
            }
            P::WaterbendX { mana: symbols, x } => format!(
                "Cost::waterbend_x({}, |q: &dyn Query, asker: &Asker| {})",
                mana(symbols)?,
                self.asked(x)?
            ),
            P::Loyalty(n) => format!(
                "Cost::loyalty(|q: &dyn Query, asker: &Asker| {})",
                self.asked(n)?
            ),
            P::Energy(n) => format!(
                "Cost::energy(|q: &dyn Query, asker: &Asker| {})",
                self.asked(n)?
            ),
            P::Sacrifice { objects, choose } => format!(
                "Cost::sacrifices({})",
                self.selection(&CostObjectSelection {
                    objects: objects.clone(),
                    choose: choose.clone(),
                })?
            ),
            P::ChangeTapState {
                selections,
                tapped,
                symbol: false,
            } => format!(
                "Cost::tap_state({}, {tapped})",
                self.selections(selections)?
            ),
            P::Constrained { part, constraints } => {
                let inner = self.cost_part_code(part, label)?;
                let mut rules = Vec::new();
                for constraint in constraints {
                    rules.push(self.selection_rule(constraint)?);
                }
                format!("{inner}.such_that(&[{}])", rules.join(", "))
            }
            P::Reveal {
                count,
                filter,
                random,
            } => format!(
                "Cost::reveals({}, {}, {random})",
                self.count_code(count)?,
                self.over(filter, "id")?
            ),
            // Paid before anything resolves, with nothing to read but the asker.
            P::PlayerAction { player, body } => {
                let saved = std::mem::replace(&mut self.stack_item, false);
                let read = self
                    .stated(&Operand::Program(body.clone()))
                    .and_then(|body| Ok((body, self.player(player)?)));
                self.stack_item = saved;
                let (body, player) = read?;
                format!("Cost::performed(|q: &dyn Query, asker: &Asker| {player}, {body})")
            }
            P::Discard {
                count,
                filter,
                random,
            } => format!(
                "Cost::discards(|q: &dyn Query, asker: &Asker| {}, {}, {random})",
                self.asked(count)?,
                self.over(filter, "id")?
            ),
            P::Draw { count } => {
                format!(
                    "Cost::draws(|q: &dyn Query, asker: &Asker| {})",
                    self.asked(count)?
                )
            }
            P::Behold { count, what } => format!(
                "Cost::beholds(|q: &dyn Query, asker: &Asker| {}, {})",
                self.asked(count)?,
                self.stated(what)?
            ),
            P::LifeEqualToOwnManaValue => "Cost::life_equal_to_mana_value()".to_string(),
            P::ManaWithX { mana: symbols, x } => format!(
                "Cost::mana_with_x({}, {})",
                mana(symbols)?,
                match x {
                    Some(x) => format!("Some(|q: &dyn Query, asker: &Asker| {})", self.asked(x)?),
                    None => "None".to_string(),
                }
            ),
            P::ManaXBounded {
                mana: symbols,
                comparison,
                value,
            } => format!(
                "Cost::mana_x_bounded({}, {}, |q: &dyn Query, asker: &Asker| {})",
                mana(symbols)?,
                comparing(comparison),
                self.asked(value)?
            ),
            P::ManaAnyNumberOfTimes(symbols) => {
                format!("Cost::mana_any_number_of_times({})", mana(symbols)?)
            }
            P::ManaFromTreasure(symbols) => format!("Cost::mana_from_treasure({})", mana(symbols)?),
            P::ManaCostReduced { object, reduction } => format!(
                "Cost::mana_cost_reduced({}, {})",
                match object {
                    Some(object) => format!(
                        "Some(|q: &dyn Query, asker: &Asker| {})",
                        self.object(object)?
                    ),
                    None => "None".to_string(),
                },
                self.stated(reduction)?
            ),
            P::ManaReducedForEach {
                mana: symbols,
                reduction,
                per,
            } => format!(
                "Cost::mana_reduced_for_each({}, {}, |q: &dyn Query, asker: &Asker| {})",
                mana(symbols)?,
                self.stated(reduction)?,
                self.asked(per)?
            ),
            P::PutCounters(placements) => {
                let mut read = Vec::new();
                for placement in placements {
                    read.push(self.placement(placement)?);
                }
                format!("Cost::puts_counters(&[{}])", read.join(", "))
            }
            P::Keyword { action, amount } => format!(
                "Cost::keyword(CostKeyword::{action:?}, {})",
                match amount {
                    Some(amount) => format!(
                        "Some(|q: &dyn Query, asker: &Asker| {})",
                        self.asked(amount)?
                    ),
                    None => "None".to_string(),
                }
            ),
            P::Unspecialize(object) => format!(
                "Cost::unspecialize(|q: &dyn Query, asker: &Asker| {})",
                self.object(object)?
            ),
            P::RevealChoice(kind) => format!("Cost::reveal_choice(ValueKind::{kind:?})"),
            P::UnattachFrom { attachments, hosts } => format!(
                "Cost::unattach({}, {})",
                self.pick_from(attachments)?,
                self.pick_from(hosts)?
            ),
            P::EnergyOneOrMore => "Cost::energy_one_or_more()".into(),
            P::EachPlayerAction { players, action } => format!(
                "Cost::each_player(|q: &dyn Query, asker: &Asker| {}, {})",
                self.players(players)?,
                self.stated(action)?
            ),
            P::ObjectAction { objects, action } => {
                let action = match action {
                    CostObjectAction::AttachToPlayer(players) => format!(
                        "CostObjectAction::AttachToPlayer(|q: &dyn Query, asker: &Asker| {})",
                        self.players(players)?
                    ),
                    other => format!("CostObjectAction::{other:?}"),
                };
                format!(
                    "Cost::object_action({}, {action})",
                    self.pick_from(objects)?
                )
            }
            P::ShuffleFromHand => "Cost::shuffle_from_hand()".into(),
            P::PlayFromTop(filter) => format!("Cost::play_from_top({})", self.over(filter, "id")?),
            P::SinglePlayersCards(part) => {
                format!("{}.of_one_player()", self.cost_part_code(part, label)?)
            }
            P::MoveCounters {
                from,
                to,
                kind,
                amount,
            } => {
                let amount = match amount {
                    MovedCounters::Exactly(n) => {
                        format!("MovedCounters::Exactly({})", self.asked(n)?)
                    }
                    MovedCounters::AtLeast(n) => {
                        format!("MovedCounters::AtLeast({})", self.asked(n)?)
                    }
                    other => format!("MovedCounters::{}", form(other)),
                };
                format!(
                    "Cost::moves_counters({}, {}, {}, {amount})",
                    self.pick_from(from)?,
                    self.pick_from(to)?,
                    self.counters_kind(kind.as_ref())?
                )
            }
            other => return Err(gap(format!("cost part {}", form(other)))),
        })
    }
    /// A cost part with no field of `Cost`: the part as it will be. A selection
    /// rule of the part is its own requirement, and `failed` says which.
    pub(super) fn stated_cost_part(
        &mut self,
        part: &CostPart,
        label: &str,
        failed: Diagnostic,
    ) -> Result<String> {
        let code = self
            .cost_part_code(part, label)
            .map_err(|_| failed.clone())?;
        Ok(planned::planned_value(&failed.feature, "Cost::NONE", &code))
    }
    /// Paying a whole cost as an effect, which `Game` has a way to only for parts.
    pub(super) fn stated_payment(
        &mut self,
        feature: &str,
        id: usize,
        player: &PlayerRef,
        cost: &Cost,
        out: &mut String,
    ) -> Result<()> {
        let price = self
            .cost(cost, &format!("paying_{id}"))
            .map_err(|_| gap(feature))?;
        let payer = self.certain_player(player);
        let payer =
            self.fetch(
                payer.map_err(|failed| match lane_j::wants_a_player(&failed.feature) {
                    true => failed,
                    false => gap(feature),
                })?,
            );
        let paid = planned::planned_value(
            feature,
            "false",
            &format!("{M}::effects::pay(g, {payer}, asker.source, {price})?"),
        );
        writeln!(out, "let paid_{id} = {paid};").unwrap();
        Ok(())
    }
    /// A mechanic the engine takes only a mana cost for, as it will be with a
    /// `Cost` of any kind, or with the operands the failure says it can't read.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn stated_mechanic(
        &mut self,
        mechanic: Mechanic,
        cost: Option<&Cost>,
        amount: Option<&Expr>,
        filter: Option<&ObjectFilter>,
        label: &str,
        failed: Diagnostic,
        parts: &mut Parts,
    ) -> Result<()> {
        let cost_of = failed.feature.contains(" cost other than ");
        let (true, Some(cost)) = (cost_of, cost) else {
            return Err(failed);
        };
        let module = snake(&format!("{mechanic:?}"));
        let price = self.cost(cost, &format!("{label}_way"))?;
        let mut operands = vec![price];
        if let Some(amount) = amount {
            operands.push(self.queried(|c| c.asked(amount))?);
        }
        if let Some(filter) = filter {
            operands.push(self.queried(|c| c.over(filter, "id"))?);
        }
        operands.push(quoted(&format!("{mechanic:?}")));
        self.plan_entry(
            &failed.feature,
            "ways",
            &format!("{M}::{module}::way({})", operands.join(", ")),
            parts,
        );
        Ok(())
    }

    // --- Keyword actions Cantrip has no word for.

    /// A keyword action done a number of times, or of a number of things.
    pub(super) fn action_with_amount(
        &mut self,
        id: usize,
        action: keywords::Action,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        use KeywordAction as K;
        let feature = format!("keyword action {:?} with an amount", action.action);
        match action.action {
            // "Explores X times", "populate X times": the action, that many times.
            K::Explore | K::Populate | K::OpenAttraction | K::TimeTravel => {
                let times = self.times(&action)?;
                writeln!(out, "for _ in 0..({times}) {{").unwrap();
                let once = keywords::Action {
                    amount: None,
                    ..action
                };
                self.keyword_action(id, once, result, out)?;
                writeln!(out, "}}").unwrap();
            }
            K::ManifestDread => {
                let times = self.times(&action)?;
                let player = self.actor(&action)?;
                writeln!(out, "let player_{id} = {player}; let mut entered_{id}: Vec<ObjectId> = Vec::new(); for _ in 0..({times}) {{ entered_{id}.extend({M}::manifest_dread::manifest_dread_object(g, player_{id}, asker.source)?); }}").unwrap();
                self.moved.insert(result.0, format!("entered_{id}"));
            }
            K::Skip(SkippedPart::Turn) => {
                let n = self.action_amount(&action, i16::MAX.into(), "u32")?;
                let player = self.actor(&action)?;
                writeln!(out, "let amount_{id} = {n}; let player_{id} = {player}; g.skip_next_turns(asker.source, player_{id}, amount_{id} as _);").unwrap();
            }
            K::RollDie(sides) => {
                let n = self.action_amount(&action, u32::MAX.into(), "u32")?;
                let player = self.actor(&action)?;
                let total = planned::planned_value(
                    &feature,
                    "0i64",
                    &format!(
                        "{M}::dice::roll_several(g, player_{id}, {sides}, amount_{id})?.total()"
                    ),
                );
                writeln!(out, "let amount_{id} = {n}; let player_{id} = {player}; let value_{id}: i64 = {total};").unwrap();
                self.values.insert(result.0, format!("value_{id}"));
            }
            other => {
                let n = self.action_amount(&action, u32::MAX.into(), "u32")?;
                let call = match other {
                    K::RollKeepHighest(sides) => {
                        format!("{M}::dice::roll_keep_highest(g, player, {sides}, amount)?")
                    }
                    K::RollAndStoreD6 => {
                        format!("{M}::dice::roll_and_store(g, player, 6, amount, asker.source)?")
                    }
                    K::SetStake => {
                        format!("{M}::stakes::set_stake(g, player, amount, asker.source)?")
                    }
                    K::PlanarDeckPickPlane => {
                        format!("{M}::planechase::pick_from_top(g, player, amount)?")
                    }
                    K::PlanarDeckPlaneswalkToEach => {
                        format!("{M}::planechase::planeswalk_to_each_of_top(g, player, amount)?")
                    }
                    K::PlanarDeckChaosEach => {
                        format!("{M}::planechase::chaos_of_each_of_top(g, player, amount)?")
                    }
                    _ => return Err(gap(feature)),
                };
                let call = format!("let amount = {n}; {call}");
                self.plan_for_actor(&feature, &action, &call, out)?;
            }
        }
        Ok(())
    }
    /// A keyword action Cantrip has no word for, called what its rules text does.
    pub(super) fn stated_action(
        &mut self,
        kind: KeywordAction,
        action: &keywords::Action,
        out: &mut String,
    ) -> Result<()> {
        use KeywordAction as K;
        let feature = format!("keyword action {kind:?}");
        let call = match kind {
            K::ChaosEnsues => format!("{M}::planechase::chaos_ensues(g, player)?"),
            K::PlanarDeckChaos => format!("{M}::planechase::chaos_of_top(g, player)?"),
            K::PlanarDeckPlaneswalkWithoutLeaving => {
                format!("{M}::planechase::planeswalk_without_leaving(g, player)?")
            }
            K::RevealTopOfPlanarDeck => format!("{M}::planechase::reveal_top(g, player)?"),
            K::TopOfPlanarDeckToBottom => format!("{M}::planechase::top_to_bottom(g, player)?"),
            K::RollTwoInOrder(sides) => {
                format!("{M}::dice::roll_two_in_order(g, player, {sides}, asker.source)?")
            }
            K::RerollStoredDice => format!("{M}::dice::reroll_stored(g, player, asker.source)?"),
            K::DoubleStake => format!("{M}::stakes::double_stake(g, player, asker.source)?"),
            K::ShuffleIfSearched => {
                format!("{M}::library::shuffle_if_searched(g, player, asker.source)?")
            }
            K::RemainingToBottomAtRandom => {
                format!("{M}::library::rest_to_bottom_at_random(g, player, asker.source)?")
            }
            K::ChooseProtection => format!("{M}::choose::protection(g, player, asker.source)?"),
            K::ChooseDamageNumber => {
                format!("{M}::choose::damage_number(g, player, asker.source)?")
            }
            K::ChooseCopy => format!("{M}::copy::choose_copy(g, player, asker.source)?"),
            K::ChooseSector => format!("{M}::sectors::choose_sector(g, player, asker.source)?"),
            K::ClaimPrize => format!("{M}::attractions::claim_prize(g, player, asker.source)?"),
            K::TurnPileFaceUp => format!("{M}::piles::turn_pile_face_up(g, player, asker.source)?"),
            K::ActivateManaAbility => {
                let object = self.acted_object(action.object)?;
                format!(
                    "if let Some(object) = {object} {{ {M}::mana::activate_mana_ability(g, player, object)?; }}"
                )
            }
            K::SetRepeatablePlayers => {
                let players = self.fetch(self.players(action.actor)?);
                let code = format!("{M}::players::set_repeatable(g, {players}, asker.source)?;");
                self.plan_code(&feature, &code, out);
                return Ok(());
            }
            _ => return Err(gap(feature)),
        };
        self.plan_for_actor(&feature, action, &call, out)
    }

    /// What a production makes, where the engine has no plan of it.
    fn production(&mut self, mana: &ManaProduction) -> Result<String> {
        use ManaProduction as P;
        let many = |c: &mut Self, list: &[ManaProduction]| -> Result<String> {
            let mut all = Vec::new();
            for each in list {
                all.push(c.production(each)?);
            }
            Ok(all.join(", "))
        };
        Ok(match mana {
            P::Primitive { name, .. } => {
                return Err(gap(format!("primitive mana production {name}")))
            }
            P::Amount { color: of, amount } => format!(
                "Production::Amount(ManaColor::{of:?}, {})",
                self.read(self.number(amount)?.amount(u32::MAX.into(), "u32")?)
            ),
            P::All(list) => format!("Production::All(&[{}])", many(self, list)?),
            P::ChooseOne(list) => format!("Production::ChooseOne(&[{}])", many(self, list)?),
            P::Repeat {
                amount,
                production,
                choice,
            } => format!(
                "Production::Repeat({}, &{}, ManaChoiceTiming::{choice:?})",
                self.read(self.number(amount)?.amount(u32::MAX.into(), "u32")?),
                self.production(production)?
            ),
            P::SameTypeAsProduced => "Production::SameTypeAsProduced".to_string(),
            P::CouldProduce {
                objects,
                colors_only,
            } => format!(
                "Production::CouldProduce({}, {colors_only})",
                self.read(self.objects(objects)?)
            ),
            P::CommanderIdentity => "Production::CommanderIdentity".to_string(),
            P::ColorsAmong { objects, each } => format!(
                "Production::ColorsAmong({}, {each})",
                self.read(self.objects(objects)?)
            ),
            P::ManaCostOf(object) => {
                format!(
                    "Production::ManaCostOf({})",
                    self.read(self.object(object)?)
                )
            }
            P::SymbolsOf(object) => {
                format!("Production::SymbolsOf({})", self.read(self.object(object)?))
            }
            P::DifferentColors(n) => format!("Production::DifferentColors({n})"),
            P::OtherColor => "Production::OtherColor".to_string(),
            P::LostThisWay(result) => format!("Production::LostThisWay({})", result.0),
            P::LastNoted { amount } => format!("Production::LastNoted {{ amount: {amount} }}"),
            P::DraftNotedColor(note) => format!("Production::DraftNoted({})", quoted(&form(note))),
        })
    }
    /// Adding mana no plan names.
    pub(super) fn stated_mana(
        &mut self,
        players: &PlayerSet,
        mana: &ManaProduction,
        restriction: Option<&ManaUse>,
        failed: Diagnostic,
        out: &mut String,
    ) -> Result<()> {
        // "Add mana equal to enchanted permanent's mana cost": a mana of each colored
        // and Phyrexian symbol's color, {C} for each generic, and for a symbol of two
        // kinds ({W/U}, {C/W}, {2/W}: CR 107.4e) the kind its controller chooses.
        if let (ManaProduction::ManaCostOf(object), None, None) =
            (mana, restriction, &self.wanted_labels)
        {
            let object = self.fetch(self.object(object).map_err(|_| failed.clone())?);
            let players = self.fetch(self.players(players).map_err(|_| failed.clone())?);
            self.wanted_labels = Some("&[\"White\", \"Blue\", \"Black\", \"Red\", \"Green\", \"Colorless\", \"Two colorless\"]".into());
            writeln!(out, "for player in {players} {{ if let Some(o) = {object} {{ let cost = g.mana_cost(o); let card = g.card_id(&CARD); let mut adds: Vec<(Mana, u32)> = vec![(Mana::C, u32::from(cost.generic))]; for (i, &n) in cost.symbols.iter().enumerate() {{ adds.push((Mana::ALL[i], u32::from(n))); }} for (i, &n) in cost.phyrexian.iter().enumerate() {{ adds.push((Mana::ALL[i], u32::from(n))); }} for (a, b) in cost.hybrid.into_iter().flatten() {{ let pick = cantrip_mechanics::choose::one_of(g, player, asker.source, vec![Label::Card(card, a as u8), Label::Card(card, b as u8)])?; adds.push((if pick == 0 {{ a }} else {{ b }}, 1)); }} for (i, &n) in cost.twobrid.iter().enumerate() {{ for _ in 0..n {{ let pick = cantrip_mechanics::choose::one_of(g, player, asker.source, vec![Label::Card(card, i as u8), Label::Card(card, 6)])?; adds.push(if pick == 0 {{ (Mana::ALL[i], 1) }} else {{ (Mana::C, 2) }}); }} }} for (mana, n) in adds {{ if n > 0 {{ g.add_mana(player, mana, n); }} }} }} }}").unwrap();
            return Ok(());
        }
        let production = self.production(mana).map_err(|_| failed.clone())?;
        let (lasting, restriction) = lane_g::mana_lasting(restriction);
        let rule = match restriction {
            Some(rule) => format!("Some({})", self.mana_use(rule).map_err(|_| failed.clone())?),
            None => "None".to_string(),
        };
        let lasting = match lasting {
            Some(until) => format!("Some(ManaLasts::{until})"),
            None => "None".to_string(),
        };
        let code = format!(
            "for player in {} {{ {M}::mana::add_production(g, player, {production}, {rule}, {lasting})?; }}",
            self.fetch(self.players(players).map_err(|_| failed.clone())?)
        );
        self.plan_code(&failed.feature, &code, out);
        Ok(())
    }
    /// How big a token is as it is created, if its power and toughness aren't
    /// fixed (CR 111.3, 111.4).
    pub(super) fn token_sized(&mut self, token: &TokenDefinition) -> Result<Option<String>> {
        let Some((power, toughness)) = &token.power_toughness else {
            return Ok(None);
        };
        if matches!((power, toughness), (Expr::Constant(_), Expr::Constant(_))) {
            return Ok(None);
        }
        let read = |c: &mut Self, n: &Expr| -> Result<String> { Ok(c.number(n)?.code) };
        let failed = || gap("dynamic defining token characteristics");
        let power = read(self, power).map_err(|_| failed())?;
        let toughness = read(self, toughness).map_err(|_| failed())?;
        Ok(Some(format!(
            "|q: &dyn Query, asker: &Asker| ({power}, {toughness})"
        )))
    }
    /// How many cards an instruction lets be played.
    fn play_count(&mut self, count: &Count) -> Result<String> {
        Ok(match count {
            Count::Exactly(n) => format!(
                "PlayCount::Exactly({})",
                self.fetch(self.number(n)?.amount(u32::MAX.into(), "u32")?)
            ),
            Count::UpTo(n) => format!(
                "PlayCount::UpTo({})",
                self.fetch(self.number(n)?.amount(u32::MAX.into(), "u32")?)
            ),
            Count::AnyNumber => "PlayCount::AnyNumber".to_string(),
            Count::PerPlayer { players, each } => format!(
                "PlayCount::PerPlayer({}, &{})",
                self.fetch(self.players(players)?),
                self.play_count(each)?
            ),
            other => return Err(gap(format!("play count {}", form(other)))),
        })
    }
    /// "Cast one card from among them for each opponent", and the plays the
    /// offer of one card at a time doesn't say.
    pub(super) fn stated_play(
        &mut self,
        operation: &Operation,
        optional: bool,
        failed: Diagnostic,
        out: &mut String,
    ) -> Result<()> {
        let plain = lane_i::plain_play(operation);
        let Operation::PlayCards {
            actor,
            action,
            cards,
            count,
            cost,
            into_exile,
        } = plain.as_ref().unwrap_or(operation)
        else {
            return Err(failed);
        };
        let read = |c: &mut Self| -> Result<String> {
            let players = c.fetch(c.players(actor)?);
            let pool = c.fetch(c.objects(cards)?);
            let count = c.play_count(count)?;
            let cost = match cost {
                PlayCost::Free => "PlayCost::Free".to_string(),
                PlayCost::Normal => "PlayCost::Normal".to_string(),
                other => return Err(gap(format!("play cost {}", form(other)))),
            };
            Ok(format!(
                "{M}::playing::play_each(g, {players}, {pool}, CardPlayAction::{action:?}, {count}, {cost}, {optional}, {into_exile}, asker.source)?;"
            ))
        };
        let code = read(self).map_err(|_| failed.clone())?;
        let id = self.fresh();
        let played = planned::planned_value(
            &failed.feature,
            "Vec::<ObjectId>::new()",
            code.trim_end_matches(';'),
        );
        writeln!(out, "let played_{id}: Vec<ObjectId> = {played};").unwrap();
        self.played = Some(format!("played_{id}"));
        Ok(())
    }
    /// A rule of casting this card no field of the engine's holds.
    pub(super) fn stated_casting(&mut self, rule: &CastingRule, parts: &mut Parts) -> Result<()> {
        let feature = format!("casting rule {}", form(rule));
        let code = match rule {
            CastingRule::ProhibitTargets { filter, .. } => {
                format!("CastRule::ProhibitTargets({})", self.over(filter, "id")?)
            }
            CastingRule::TaxPaidAsLife(amount) => format!(
                "CastRule::TaxPaidAsLife(|q: &dyn Query, asker: &Asker| {})",
                self.asked(amount)?
            ),
            _ => return Err(gap(feature)),
        };
        self.plan_entry(&feature, "cast_rules", &code, parts);
        Ok(())
    }
    /// "If this spell was kicked, do this instead": a cost, and the spell it makes.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn stated_branch(
        &mut self,
        keyword: &str,
        cost: &Cost,
        paid: &Ability,
        unpaid: &Ability,
        label: &str,
        failed: Diagnostic,
        parts: &mut Parts,
    ) -> Result<()> {
        let read = |c: &mut Self| -> Result<String> {
            // The branch that failed may have left its own items under `label`.
            let cost = c.cost(cost, &format!("{label}_branch_cost"))?;
            let paid = c.resolved_body(paid, &format!("{label}_branch_paid"))?;
            let unpaid = c.resolved_body(unpaid, &format!("{label}_branch_unpaid"))?;
            Ok(format!(
                "CostBranch {{ keyword: {}, cost: {cost}, paid: {paid}, unpaid: {unpaid} }}",
                quoted(keyword)
            ))
        };
        let code = read(self).map_err(|_| failed.clone())?;
        self.plan_entry(&failed.feature, "cost_branches", &code, parts);
        Ok(())
    }
    /// "You may cast this card from your graveyard" on terms of its own.
    pub(super) fn stated_own_cast(
        &mut self,
        from: &EventZone,
        condition: Option<&Condition>,
        terms: Option<&PlayTerms>,
        label: &str,
        failed: Diagnostic,
        parts: &mut Parts,
    ) -> Result<()> {
        let plain = lane_h::plain_terms();
        let grant = self
            .play_grant(&CardPlayAction::CastSpell, terms.unwrap_or(&plain))
            .map_err(|_| failed.clone())?;
        let zone = zone_kind(from).map_err(|_| failed.clone())?;
        let condition = match condition {
            Some(condition) => {
                let test = self
                    .activation_condition(condition, &format!("{label}_if"))
                    .map_err(|_| failed.clone())?;
                format!("Some({test})")
            }
            None => "None".to_string(),
        };
        self.plan_entry(
            &failed.feature,
            "own_casts",
            &format!("OwnCast {{ zone: {zone}, condition: {condition}, grant: PlayGrant {{ {grant}..PlayGrant::PLAIN }} }}"),
            parts,
        );
        Ok(())
    }

    // --- Reads with no word yet.

    /// `i64`: the mana symbols of `colors` among the mana costs of `objects`.
    pub(super) fn mana_symbols(&self, objects: &ObjectSet, colors: &[ManaColor]) -> Result<String> {
        let colors = colors
            .iter()
            .map(|c| color(c).map(|c| format!("Color::{c}.mana()")))
            .collect::<Result<Vec<_>>>()?
            .join(", ");
        // A hybrid symbol of two of the colors is one symbol (CR 700.5, 107.4e).
        Ok(format!(
            "({}).into_iter().map(|id: ObjectId| {{ let cost = q.mana_cost(id); let colors = [{colors}]; colors.iter().map(|&m| i64::from(cost.symbols[m as usize]) + i64::from(cost.phyrexian[m as usize]) + i64::from(cost.twobrid[m as usize])).sum::<i64>() + cost.hybrid.iter().flatten().filter(|(a, b)| colors.contains(a) || colors.contains(b)).count() as i64 }}).sum::<i64>()",
            self.objects(objects)?
        ))
    }
    /// `i64`: the counters among these players and objects.
    pub(super) fn counters_among(
        &self,
        recipients: &Recipients,
        kind: Option<&crate::counter::CounterKind>,
    ) -> Result<String> {
        Ok(match recipients {
            Recipients::Objects(objects) => format!(
                "({}).into_iter().map(|o: ObjectId| {}).sum::<i64>()",
                self.objects(objects)?,
                read::counters_on("o", kind)?
            ),
            Recipients::Players(players) => planned::planned_value(
                "counters on a player",
                "0i64",
                &format!(
                    "({}).into_iter().map(|p: PlayerId| i64::from(q.state().player(p).counters({}))).sum::<i64>()",
                    self.players(players)?,
                    self.counters_kind(kind)?
                ),
            ),
            Recipients::Union(list) => {
                let mut all = Vec::new();
                for each in list {
                    all.push(self.counters_among(each, kind)?);
                }
                format!("({})", all.join(" + "))
            }
            _ => return Err(gap("counters on a player")),
        })
    }
    /// `Vec<PlayerId>`: a set nothing else reads.
    pub(super) fn players_stated(&self, players: &PlayerSet) -> Result<String> {
        if let PlayerSet::DamagedBy(result) = players {
            if let Some(list) = self.hurt_players.get(&result.0) {
                return Ok(format!("{list}.clone()"));
            }
        }
        let code = match players {
            PlayerSet::DamagedBy(result) => format!("q.players_damaged_by(asker, {})", result.0),
            PlayerSet::DamageRecipients => "q.recipients_of_event_damage(asker)".to_string(),
            PlayerSet::Scoped => "q.players_in_scope(asker)".to_string(),
            other => return Err(gap(format!("player collection {}", form(other)))),
        };
        Ok(planned::planned_value(
            &format!("player collection {}", form(players)),
            "Vec::<PlayerId>::new()",
            &code,
        ))
    }
    /// `Option<ObjectId>`: the spell or ability a slot targets, read as an object.
    pub(super) fn stack_object(
        &self,
        slot: &Slot,
        index: impl std::fmt::Display,
    ) -> Result<String> {
        let at = self.slot(slot)?;
        Ok(planned::planned_value(
            "a spell or ability on the stack read as an object",
            "None::<ObjectId>",
            &format!("q.stack_item_object(t_{at}_raw.get({index}).copied())"),
        ))
    }
    /// As `stack_object`, of every target of the slot.
    pub(super) fn stack_objects(&self, slot: &Slot) -> Result<String> {
        let at = self.slot(slot)?;
        Ok(planned::planned_value(
            "a spell or ability on the stack read as an object",
            "Vec::<ObjectId>::new()",
            &format!(
                "t_{at}_raw.iter().filter_map(|&t: &Target| q.stack_item_object(Some(t))).collect::<Vec<ObjectId>>()"
            ),
        ))
    }
    /// What a player did that no event of the record names.
    pub(super) fn did_stated(
        &self,
        player: &PlayerRef,
        event: &PlayerHistory,
        scope: &str,
    ) -> Result<String> {
        use PlayerHistory as H;
        let feature = format!("condition PlayerDid {}", form(event));
        let who = self.player(player)?;
        let any = |test: String| format!("{scope}.any(|r| {test})");
        let activated = |test: &str| {
            any(format!(
                "matches!(r.event, Event::AbilityActivated {{ ability, player, .. }} if Some(player) == {who} && q.activated_ability(ability).is_some_and(|d| {test}))"
            ))
        };
        // A land played: its zone change says where it came from.
        let land = |from: &str| {
            format!(
                "matches!(r.event, Event::LandPlayed {{ land, player, .. }} if Some(player) == {who} && q.history().all().iter().any(|m| matches!(m.event, Event::ZoneChange {{ new, from, .. }} if new == land && {from})))"
            )
        };
        Ok(match event {
            H::ActivatedExhaust => activated("d.exhaust"),
            H::ActivatedGraveyardAbility => activated("d.from == ZoneKind::Graveyard"),
            H::PlayedLandNotFromHand => any(land("from != Zone::hand(player)")),
            H::PlayedFromExile(filter) if matches!(**filter, ObjectFilter::Any) => any(format!(
                "{} || matches!(r.event, Event::Cast(c) if Some(c.player) == {who} && c.from == ZoneKind::Exile)",
                land("from.is(ZoneKind::Exile)")
            )),
            H::WasDealtCombatDamage => any(format!(
                "matches!(r.event, Event::Damage {{ target: Target::Player(hit), kind, .. }} if Some(hit) == {who} && kind.is_combat())"
            )),
            H::RolledDie(comparison, n) => any(format!(
                "matches!(r.event, Event::DieRolled {{ player, result, .. }} if Some(player) == {who} && {})",
                super::read::compare("i64::from(result)", comparison, &self.asked(n)?)
            )),
            H::Cycled(comparison, n, filter) => super::read::compare(
                &format!(
                    "{scope}.filter_map(|r: &Record| match r.event {{ Event::Cycled {{ card, player }} if Some(player) == {who} => Some(card), _ => None }}).filter(|&id: &ObjectId| {}).count() as i64",
                    self.filter(filter, &Candidate::of("id"))?
                ),
                comparison,
                &self.asked(n)?,
            ),
            H::AttackedDefender(filter) => any(format!(
                "matches!(r.event, Event::Attacks {{ defender: Target::Object(id), .. }} if Some(r.active) == {who} && {})",
                self.filter(filter, &Candidate::of("id"))?
            )),
            // As the turn began: whoever last became the monarch before it.
            H::WasMonarch if scope == "q.history().this_turn().iter()" => format!(
                "q.history().all().iter().rev().find_map(|r| match r.event {{ Event::BecameMonarch {{ player }} if r.turn < q.state().turn.number => Some(player), _ => None }}).is_some_and(|was: PlayerId| Some(was) == {who})"
            ),
            // Cantrip records no payment of energy, and has no word for bending.
            H::BentAllFour => planned::planned_value(
                &feature,
                "false",
                &format!("q.player_did({scope}, {who}, PlayerDeed::BentAllFour)"),
            ),
            H::PaidOrLostEnergy(comparison, n) => planned::planned_value(
                &feature,
                "false",
                &format!(
                    "q.player_did({scope}, {who}, PlayerDeed::PaidOrLostEnergy({}, {}))",
                    comparing(comparison),
                    self.asked(n)?
                ),
            ),
            _ => return Err(gap(feature)),
        })
    }
    /// What a rule on abilities asks that `ability_test` does not.
    pub(super) fn ability_stated(&self, filter: &ObjectFilter) -> Result<String> {
        let feature = format!("activation cost of abilities {}", form(filter));
        let test = match filter {
            ObjectFilter::OnStack(StackTrait::Keyword(word)) => {
                format!("def.is_some_and(|d| d.keyword_of() == Some(AbilityKeyword::{word:?}))")
            }
            ObjectFilter::NthCast {
                ordinal,
                spells,
                by,
                scope,
            } => {
                let by = match by {
                    Some(players) => format!("Some({})", self.players(players)?),
                    None => "None".to_string(),
                };
                let which = match self.ability_filter(spells) {
                    Ok(test) => test,
                    Err(_) => format!(
                        "|q: &dyn Query, asker: &Asker, id: ObjectId| {}",
                        self.activated_stated(spells)?
                    ),
                };
                format!(
                    "q.is_nth_activation(permanent, def, {}, {which}, {by}, CastScope::{scope:?})",
                    self.asked(ordinal)?
                )
            }
            _ => return Err(gap(feature)),
        };
        Ok(planned::planned_value(&feature, "false", &test))
    }
    /// What a trigger on an activation asks that `activation_test` does not.
    pub(super) fn activated_stated(&self, filter: &ObjectFilter) -> Result<String> {
        let feature = format!("activated ability {}", form(filter));
        let test = match filter {
            ObjectFilter::OnStack(StackTrait::Keyword(AbilityKeyword::PowerUp)) => {
                return Ok("def.is_some_and(|d| d.power_up)".into())
            }
            ObjectFilter::OnStack(StackTrait::Keyword(AbilityKeyword::Equip)) => {
                return Ok("def.is_some_and(|d| d.equip)".into())
            }
            ObjectFilter::OnStack(StackTrait::Keyword(AbilityKeyword::Exhaust)) => {
                return Ok("def.is_some_and(|d| d.exhaust)".into())
            }
            // No mark on the definition: these are known by their printed word.
            ObjectFilter::OnStack(StackTrait::Keyword(AbilityKeyword::Boast)) => {
                return Ok("def.is_some_and(|d| d.text.starts_with(\"Boast\"))".into())
            }
            ObjectFilter::OnStack(StackTrait::Keyword(AbilityKeyword::Outlast)) => {
                return Ok("def.is_some_and(|d| d.text.starts_with(\"Outlast\"))".into())
            }
            ObjectFilter::OnStack(StackTrait::Keyword(word)) => {
                format!("def.is_some_and(|d| d.keyword_of() == Some(AbilityKeyword::{word:?}))")
            }
            ObjectFilter::OnStack(other) => format!("q.activation_has(id, StackTrait::{other:?})"),
            ObjectFilter::ManaSpentFrom(sources) => format!(
                "q.activation_mana_from(id).any(|source: ObjectId| {})",
                self.filter(sources, &Candidate::of("source"))?
            ),
            ObjectFilter::ManaSpent {
                color: None,
                sources: None,
                payers: None,
                comparison,
                value,
            } => read::compare(
                "q.activation_mana_spent(id)",
                comparison,
                &self.number(value)?.code,
            ),
            ObjectFilter::TargetsAny(targets) => format!(
                "q.activation_targets_any(id, |t: &Target| {})",
                self.targeted(targets)?
            ),
            _ => return Err(gap(feature)),
        };
        Ok(planned::planned_value(&feature, "false", &test))
    }

    // --- Instructions that walk several players.

    /// "Each player discards a card at random": one instruction, one discard
    /// together (CR 608.2c), with the cards each picked at random.
    pub(super) fn random_discards(
        &mut self,
        id: usize,
        players: &PlayerSet,
        number: String,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        writeln!(out, "let amount_{id} = {}; let players_{id} = {}; let mut cards_{id}: Vec<ObjectId> = Vec::new(); for player in players_{id} {{ let mut hand = g.state().zone(Zone::hand(player)).to_vec(); let mut taken = 0u32; while taken < amount_{id} && !hand.is_empty() {{ let at = g.choose_at_random(hand.len()); cards_{id}.push(hand.remove(at)); taken += 1; }} }} let discarded_{id} = g.discard_together(&cards_{id});", self.fetch(number), self.fetch(self.players(players)?)).unwrap();
        self.moved.insert(result.0, format!("discarded_{id}"));
        self.shares
            .insert(result.0, (format!("discarded_{id}"), Share::Owner));
        self.left_behind(result, format!("discarded_{id}.clone()"));
        Ok(())
    }
    /// "Each opponent creates a token that's a copy of it", all at once.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn token_copy_for_each(
        &mut self,
        id: usize,
        object: &ObjectRef,
        adjustments: &[Operand],
        amount: &Expr,
        players: &PlayerSet,
        entry: &EntrySpec,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        if entry.tapped {
            return Err(gap("token copies for several players"));
        }
        let amount = self.number(amount)?.amount(u32::MAX.into(), "u32")?;
        let values = self.copy_values(&format!("original_{id}"), adjustments)?;
        writeln!(out, "let original_{id} = {}; let values_{id} = {values}; let count_{id} = {}; let players_{id} = {}; let mut copies_{id}: Vec<(Copiable, PlayerId)> = Vec::new(); if let Some(values) = values_{id} {{ for &player in &players_{id} {{ for _ in 0..count_{id} {{ copies_{id}.push((values.clone(), player)); }} }} }} let created_{id} = g.create_token_copies_together(&copies_{id})?;", self.fetch(self.damage_source(object)?), self.fetch(amount), self.fetch(self.players(players)?)).unwrap();
        self.token_state(id, entry, out)?;
        self.creations.insert(result.0, format!("created_{id}"));
        Ok(())
    }
    /// "Each player creates a token that's a copy of each of them", all at once.
    pub(super) fn token_copies_for_each(
        &mut self,
        id: usize,
        objects: &ObjectSet,
        players: &PlayerSet,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        writeln!(out, "let originals_{id} = {}; let players_{id} = {}; let mut copies_{id}: Vec<(Copiable, PlayerId)> = Vec::new(); for &original in &originals_{id} {{ for &player in &players_{id} {{ copies_{id}.push((g.copiable_values(original), player)); }} }} let created_{id} = g.create_token_copies_together(&copies_{id})?;", self.fetch(self.objects(objects)?), self.fetch(self.players(players)?)).unwrap();
        self.creations.insert(result.0, format!("created_{id}"));
        Ok(())
    }
    /// "Remove all counters from target player", and the like.
    pub(super) fn removed_from_players(
        &mut self,
        id: usize,
        removal: &CounterRemoval,
        out: &mut String,
    ) -> Result<()> {
        const FEATURE: &str = "counters removed from a player";
        let Recipients::Players(players) = &removal.recipients else {
            return Err(gap(FEATURE));
        };
        let amount = match &removal.amount {
            CounterAmount::All => "i16::MAX".to_string(),
            CounterAmount::Count(Count::Exactly(n)) | CounterAmount::Count(Count::UpTo(n)) => {
                self.fetch(self.number(n)?.amount(i16::MAX.into(), "i16")?)
            }
            other => return Err(gap(format!("counter removal {}", form(other)))),
        };
        let kind = self.counters_kind(removal.kind.as_ref())?;
        let players = self.fetch(self.players(players)?);
        let removed = planned::planned_value(
            FEATURE,
            "0i64",
            &format!("i64::from(g.remove_player_counters(player, {kind}, {amount}))"),
        );
        writeln!(
            out,
            "for player in {players} {{ removed_{id} += {removed}; }}"
        )
        .unwrap();
        Ok(())
    }

    // --- Qualities.

    /// `HexproofFrom`, of whatever quality the card names.
    pub(super) fn hexproof_of(&self, quality: &Protection) -> Result<String> {
        let failed = match lane_j::hexproof_quality(quality) {
            Ok(code) => return Ok(code),
            Err(failed) => failed,
        };
        let code = match quality {
            // "Has hexproof from each of its colors" (Tam, Mindful First-Year): the
            // colors the creature has once layer 5 is done (CR 613.1), read as the
            // ability layer applies to it.
            Protection::Objects {
                kind: ObjectKind::Any,
                filter: ObjectFilter::SharesColorWith(ObjectRef::Source),
            } if self.reader == Reader::Layer
                && self.lines_said().iter().any(|line| line.contains("hexproof from each of its colors")) =>
            {
                return Ok("HexproofFrom(chars.colors.0 as u32)".to_string());
            }
            // "Hexproof from that color": the color this ability just chose.
            Protection::Objects { filter, .. } => {
                let over = self.over(filter, "id")?;
                match over
                    .strip_suffix(".is_some_and(|chosen| q.chars(id).colors.has(chosen))")
                    .and_then(|o| o.rsplit_once("| Some("))
                    .and_then(|(_, chosen)| chosen.strip_suffix(')'))
                {
                    // As Mondo Gecko grants it: the qualities add to any it has.
                    Some(chosen) => return Ok(format!("HexproofFrom::color({chosen})")),
                    None => format!("HexproofFrom::of({over})"),
                }
            }
            Protection::Players(players) => format!(
                "HexproofFrom::of_players(|q: &dyn Query, asker: &Asker| {})",
                self.players(players)?
            ),
            Protection::Each(qualities) => {
                let mut all = "HexproofFrom::NONE".to_string();
                for each in qualities {
                    all = format!("{all}.and({})", self.hexproof_of(each)?);
                }
                return Ok(all);
            }
        };
        Ok(planned::planned_value(
            &failed.feature,
            "HexproofFrom::NONE",
            &code,
        ))
    }
    /// What a player's protection is from.
    fn quality_code(&mut self, quality: &Protection) -> Result<String> {
        Ok(match quality {
            Protection::Objects { filter, .. } => {
                format!("Protection::Objects({})", self.over(filter, "id")?)
            }
            Protection::Players(players) => {
                let gaps = self.gaps.len();
                match self.queried(|c| c.players(players)) {
                    Ok(code) => {
                        format!("Protection::Players(move |q: &dyn Query, asker: &Asker| {code})")
                    }
                    Err(_) => {
                        self.gaps.truncate(gaps);
                        format!("Protection::These({})", self.read(self.players(players)?))
                    }
                }
            }
            Protection::Each(qualities) => {
                let mut all = Vec::new();
                for each in qualities {
                    all.push(self.quality_code(each)?);
                }
                format!("Protection::Each(&[{}])", all.join(", "))
            }
        })
    }

    // --- Costs and copies.

    /// A spell costing more of what mana is not.
    pub(super) fn costlier(mut self, cost: &Cost) -> Result<String> {
        let more = self.cost(cost, "more")?;
        Ok(planned::planned_value(
            "spell cost increase other than mana",
            "CostDelta::NONE",
            &format!("CostDelta::more_cost({more})"),
        ))
    }
    /// A size in a copy that depends on the original.
    pub(super) fn stated_size(&mut self, values: String, operands: &[Operand]) -> Result<String> {
        const FEATURE: &str = "copy exception of a power and toughness not fixed";
        // "Except its power is half that creature's power": a number as it's made,
        // and from then on what the copy is (CR 707.9b).
        if let [Operand::Number(power), Operand::Number(toughness)] = operands {
            let size = |c: &Self, n: &Expr| -> Result<String> {
                Ok(format!("({}).clamp(i16::MIN.into(), i16::MAX.into()) as i16", c.number(n)?.code))
            };
            return Ok(format!("{values}.sized({}, {})", size(self, power)?, size(self, toughness)?));
        }
        let code = match operands {
            [power, toughness] => {
                format!(
                    "{values}.sized_by({}, {})",
                    self.stated(power)?,
                    self.stated(toughness)?
                )
            }
            _ => return Err(gap(FEATURE)),
        };
        Ok(planned::planned_value(FEATURE, &values, &code))
    }
    /// An exception `Copiable` has no word for.
    pub(super) fn stated_exception(
        &mut self,
        values: String,
        name: &str,
        operands: &[Operand],
    ) -> Result<String> {
        let feature = format!("copy exception {name}");
        let of = |prefix: &str, list: &[Operand]| -> Result<String> {
            Ok(copies::words(list)?
                .iter()
                .map(|word| format!("{prefix}::{word}"))
                .collect::<Vec<_>>()
                .join(", "))
        };
        let code = match (name, operands) {
            ("SetCardtypes", list) => format!(
                "{values}.typed_as(TypeSet::of(&[{}]))",
                of("CardType", list)?
            ),
            ("SetArtifactTypes", list) => format!(
                "{values}.subtyped_as(CardType::Artifact, SubtypeSet::of(&[{}]))",
                of("Subtype", list)?
            ),
            ("AddSupertypes", list) => {
                format!("{values}.with_supertypes(&[{}])", of("Supertype", list)?)
            }
            ("RemoveSupertypes", list) => {
                format!("{values}.without_supertypes(&[{}])", of("Supertype", list)?)
            }
            ("AddColor", [Operand::Term { operands, .. }]) => {
                return Ok(added_colors(&values, &of("Color", operands)?))
            }
            ("AddColor", list) => return Ok(added_colors(&values, &of("Color", list)?)),
            // "Equal to this creature's": printed on the face it is, cast prototyped or
            // not (Hulking Metamorph's ruling), and from then on what the copy is.
            ("KeepPT", []) => {
                return Ok(format!("{{ let own = q.card(q.chars(asker.source).card); {values}.sized(own.power.unwrap_or(0), own.toughness.unwrap_or(0)) }}"))
            }
            ("KeepName", []) => format!("{values}.keeping_name()"),
            // "It doesn't copy that creature's color": its own copiable color, which is
            // what a copy of it copies (Vesuvan Doppelganger's ruling).
            ("KeepColor", []) => {
                return Ok(format!("{{ let own = q.copiable_values(asker.source); {values}.colored(own.colors.unwrap_or(q.card(own.card).colors())) }}"))
            }
            ("RemoveThisAbility", []) => format!("{values}.without_ability(item.ability)"),
            ("SetLoyalty", [loyalty]) => {
                format!("{values}.with_loyalty({})", self.stated(loyalty)?)
            }
            ("AddAbility" | "AddAbilityIfItDoesntHaveAbility" | "LosesAbility", list) => {
                let method = match name {
                    "AddAbility" => "with_ability",
                    "LosesAbility" => "without_ability",
                    _ => "with_ability_if_absent",
                };
                let mut code = values.clone();
                for operand in list {
                    match operand {
                        Operand::Ability(ability) => {
                            let refs = self.granted_refs(ability).or_else(|failed| {
                                let Ability::Mechanic {
                                    mechanic,
                                    amount: Some(amount),
                                    ..
                                } = &**ability
                                else {
                                    return Err(failed);
                                };
                                Ok(vec![format!(
                                    "AbilityRef::mechanic(Mechanic::{mechanic:?}, {})",
                                    self.fetch(
                                        self.number(amount)?.amount(u32::MAX.into(), "u32")?
                                    )
                                )])
                            });
                            for each in refs? {
                                code = format!("{code}.{method}({each})");
                            }
                        }
                        lost if name == "LosesAbility" => {
                            code = format!("{code}.losing({})", self.lost_word(lost)?);
                        }
                        _ => return Err(gap(feature)),
                    }
                }
                code
            }
            _ => return Err(gap(feature)),
        };
        Ok(planned::planned_value(&feature, &values, &code))
    }

    // --- Abilities with no arm elsewhere.

    /// The one spell a `Visit`, room or the like resolves as: its effect, with
    /// the targets and modes it announces.
    fn resolved_body(&mut self, body: &Ability, label: &str) -> Result<String> {
        let (reader, event_at, frame) = (self.reader, self.event_at, self.frame);
        (self.reader, self.event_at, self.frame) =
            (Reader::Resolution, EventAt::Nowhere, Frame::default());
        let slots = std::mem::take(&mut self.slots);
        let results = self.results();
        let mut own = Parts::default();
        let compiled = self.ability(body, label, &mut own, None);
        self.restore_results(results);
        self.slots = slots;
        (self.reader, self.event_at, self.frame) = (reader, event_at, frame);
        compiled?;
        let held = own.only_abilities() && own.abilities.is_empty();
        match (held, own.modal.as_str(), own.spell.as_str()) {
            (true, "None", "None") => Err(gap(format!("{} as a body", form(body)))),
            (true, "None", spell) => Ok(spell.to_string()),
            (true, modal, _) => Ok(modal.to_string()),
            (false, _, _) => Ok(own.card("None")),
        }
    }
    /// An `AsEvent`'s or a `BeforeGame`'s list of what is done.
    fn listed(&mut self, operand: &Operand) -> Result<String> {
        match operand {
            Operand::List(_) => self.stated(operand),
            other => Ok(format!("&[{}]", self.stated(other)?)),
        }
    }
    /// What can be attacked here, as the test an attack is asked.
    fn attack_target(&self, defender: &Recipients, name: &str) -> Result<String> {
        Ok(match defender {
            Recipients::Players(players) => {
                let member = self.player_member(players)?;
                format!("matches!({name}, Target::Player(player) if {member})")
            }
            Recipients::Objects(objects) => {
                let test = self.rule_subject(objects, "id")?;
                format!("matches!({name}, Target::Object(id) if {test})")
            }
            other => return Err(gap(format!("attacked {}", form(other)))),
        })
    }
    fn subjects_test(&self, subjects: &EffectSubjects) -> Result<String> {
        Ok(match subjects {
            EffectSubjects::Players(players) => format!(
                "Subjects::Players(|q: &dyn Query, asker: &Asker, player: PlayerId| {})",
                self.player_member(players)?
            ),
            EffectSubjects::Objects(objects) => format!(
                "Subjects::Objects(|q: &dyn Query, asker: &Asker, id: ObjectId| {})",
                self.rule_subject(objects, "id")?
            ),
        })
    }
    /// An ability nothing else compiles, as Cantrip will have it.
    pub(super) fn stated_ability(
        &mut self,
        ability: &Ability,
        label: &str,
        parts: &mut Parts,
    ) -> Result<()> {
        let feature = format!("ability {}", form(ability));
        match ability {
            // The two abilities of one printed ability, each with its own event.
            Ability::Triggers(triggers) => {
                for (i, trigger) in triggers.iter().enumerate() {
                    self.ability(trigger, &format!("{label}_{i}"), parts, None)?;
                }
            }
            Ability::FlashIf(condition) if format!("{condition:?}").contains("AnnouncedX") => {
                return Err(gap("a limit on the X announced"));
            }
            Ability::FlashIf(condition) => {
                let function = format!("{label}_flash");
                let test = self.queried(|c| c.activation_condition(condition, &function))?;
                parts.field("flash_if", format!("Some({test})"));
            }
            Ability::Visit { body, prize } => {
                let visit = self.resolved_body(body, &format!("{label}_visit"))?;
                let prize = match prize {
                    Some(prize) => format!(
                        "Some({})",
                        self.resolved_body(prize, &format!("{label}_prize"))?
                    ),
                    None => "None".to_string(),
                };
                let text = quoted(&spell_words(body));
                self.plan_entry(
                    &feature,
                    "abilities",
                    &format!("{M}::attractions::visit({visit}, {prize}, {text})"),
                    parts,
                );
            }
            Ability::DungeonRoom {
                name,
                body,
                leads_to,
            } => {
                let room = self.resolved_body(body, &format!("{label}_room"))?;
                let leads_to = leads_to.iter().map(|n| quoted(n)).collect::<Vec<_>>();
                self.plan_entry(
                    &feature,
                    "rooms",
                    &format!(
                        "Room {{ name: {}, effect: {room}, leads_to: &[{}] }}",
                        quoted(name),
                        leads_to.join(", ")
                    ),
                    parts,
                );
            }
            Ability::DraftRule(actions) => {
                let actions = self.queried(|c| c.stated(&Operand::List(actions.clone())))?;
                self.plan_entry(
                    &feature,
                    "draft_rules",
                    &format!("{M}::draft::rule({actions})"),
                    parts,
                );
            }
            Ability::ProtectionExcept { from, keeps } => {
                let (from, keeps) =
                    self.queried(|c| Ok((c.stated(from)?, c.rule_subject(keeps, "id")?)))?;
                self.plan_entry(
                    &feature,
                    "statics",
                    &format!("protection_keeping({from}, |q: &dyn Query, asker: &Asker, id: ObjectId| {keeps})"),
                    parts,
                );
            }
            Ability::AsEvent {
                object,
                event,
                actions,
            } => {
                let event = match event {
                    AsEvent::BecomesAttachedTo(filter) => format!(
                        "AsEvent::BecomesAttachedTo({})",
                        self.queried(|c| c.over(filter, "id"))?
                    ),
                    other => format!("AsEvent::{other:?}"),
                };
                let object = self.fetch(self.object(object)?);
                let actions = self.listed(actions)?;
                self.plan_entry(
                    &feature,
                    "as_events",
                    &format!("AsEventDef {{ event: {event}, object: |g: &mut dyn Game, asker: &Asker| {object}, actions: {actions} }}"),
                    parts,
                );
            }
            Ability::BeforeGame { when, body } => {
                let body = self.listed(body)?;
                self.plan_entry(
                    &feature,
                    "before_game",
                    &format!("(BeforeGameMoment::{when:?}, {body})"),
                    parts,
                );
            }
            // Judged as the card exists where it is (CR 101.2): `EnterRule`.
            Ability::CantEnter { cards, from } => {
                let (cards, from) = self.queried(|c| {
                    let cards = c.over(cards, "id")?;
                    let from = match from {
                        Some(scope) => c.in_scope(scope, "id")?,
                        None => "true".to_string(),
                    };
                    Ok((cards, from))
                })?;
                let function = format!("{label}_enter");
                writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, id: ObjectId) -> bool {{ !({from} && ({cards})(q, asker, id)) }}").unwrap();
                parts.table("enter_rules", function);
            }
            // "No more than two creatures can attack you each combat" (CR 508.1c),
            // of what has the ability: a rule of the combat, asked of what is attacked.
            Ability::AttackCap {
                defender:
                    defender @ (Recipients::Players(PlayerSet::One(PlayerRef::AbilityController))
                    | Recipients::Objects(ObjectSet::One(ObjectRef::Source))),
                amount: Expr::Constant(most),
            } => {
                let attacked = match defender {
                    Recipients::Players(_) => "Target::Player(asker.controller)",
                    _ => "Target::Object(asker.source)",
                };
                writeln!(
                    self.functions,
                    "#[allow(unused_variables)] fn {label}_limit(q: &dyn Query, asker: &Asker, attacked: Target) -> Option<u32> {{ (attacked == {attacked}).then_some({most}) }}"
                )
                .unwrap();
                parts.table("attack_limits", format!("{label}_limit"));
            }
            Ability::AttackCap { defender, amount } => {
                let (at, amount) = self
                    .queried(|c| Ok((c.attack_target(defender, "defender")?, c.asked(amount)?)))?;
                self.plan_entry(
                    &feature,
                    "attack_caps",
                    &format!("AttackCap {{ defender: |q: &dyn Query, asker: &Asker, defender: Target| {at}, amount: |q: &dyn Query, asker: &Asker| {amount} }}"),
                    parts,
                );
            }
            Ability::PayToIgnore {
                subjects,
                rules,
                payer,
                cost,
                until,
            } => {
                let (subjects, rules) =
                    self.queried(|c| Ok((c.subjects_test(subjects)?, c.stated(rules)?)))?;
                let payer = match payer {
                    Some(payer) => format!(
                        "Some(|q: &dyn Query, asker: &Asker, id: ObjectId| {})",
                        self.queried(|c| c.player(payer))?
                    ),
                    None => "None".to_string(),
                };
                let cost = self.cost(cost, &format!("{label}_ignore"))?;
                let until = self.stated_until(until)?;
                self.plan_entry(
                    &feature,
                    "pay_to_ignore",
                    &format!("PayToIgnore {{ subjects: {subjects}, rules: {rules}, payer: {payer}, cost: {cost}, until: {until} }}"),
                    parts,
                );
            }
            Ability::CostOptions { keyword, options } => {
                let mut ways = Vec::new();
                for (i, (cost, then)) in options.iter().enumerate() {
                    let label = format!("{label}_option_{i}");
                    let then = self.resolved_body(then, &label)?;
                    ways.push(format!("({}, {then})", self.cost(cost, &label)?));
                }
                self.plan_entry(
                    &feature,
                    "cost_options",
                    &format!(
                        "CostOptions {{ keyword: {}, options: &[{}] }}",
                        quoted(keyword),
                        ways.join(", ")
                    ),
                    parts,
                );
            }
            Ability::WhileOnTopOfLibrary(abilities) => {
                let mut own = Parts::default();
                for (i, each) in abilities.iter().enumerate() {
                    self.ability(each, &format!("{label}_top_{i}"), &mut own, None)?;
                }
                let enters = self.enters(&own.enters);
                self.plan_entry(&feature, "on_top_of_library", &own.card(&enters), parts);
            }
            Ability::KickedHas { cost, ability } => {
                let cost = self.cost(cost, &format!("{label}_kicked"))?;
                let has = self.resolved_body(ability, &format!("{label}_has"))?;
                self.plan_entry(&feature, "kicked_has", &format!("({cost}, {has})"), parts);
            }
            Ability::StartingHandSize(amount) => {
                let amount = self.queried(|c| c.asked(amount))?;
                self.plan_entry(
                    &feature,
                    "statics",
                    &format!("starting_hand_size(|q: &dyn Query, asker: &Asker| {amount})"),
                    parts,
                );
            }
            Ability::VillainousChoiceTwice(players) => {
                let member = self.queried(|c| c.player_member(players))?;
                self.plan_entry(
                    &feature,
                    "statics",
                    &format!("villainous_choice_twice(|q: &dyn Query, asker: &Asker, player: PlayerId| {member})"),
                    parts,
                );
            }
            Ability::ExilesSearchedCards { searching, player } => {
                let (searching, player) =
                    self.queried(|c| Ok((c.player_member(searching)?, c.player(player)?)))?;
                self.plan_entry(
                    &feature,
                    "statics",
                    &format!("exiles_searched_cards(|q: &dyn Query, asker: &Asker, player: PlayerId| {searching}, |q: &dyn Query, asker: &Asker| {player})"),
                    parts,
                );
            }
            Ability::Unique(rule) => {
                self.plan_entry(
                    &feature,
                    "statics",
                    &format!("game_rule(GameRule::{rule:?})"),
                    parts,
                );
            }
            other => return Err(gap(format!("ability {}", form(other)))),
        }
        Ok(())
    }
    /// A triggered or activated ability that works in the command zone (CR 903.9),
    /// which the engine doesn't look in for one. False if that isn't what it is.
    pub(super) fn from_command_zone(
        &mut self,
        zones: &[EventZone],
        condition: Option<&Condition>,
        ability: &Ability,
        label: &str,
        parts: &mut Parts,
    ) -> Result<bool> {
        const FEATURE: &str = "ability functioning in the command zone";
        let triggered = matches!(
            ability,
            Ability::Triggered { .. } | Ability::Activated { .. }
        );
        let named = zones.iter().map(zone_kind).collect::<Result<Vec<_>>>();
        let Err(failed) = zone_list(zones) else {
            return Ok(false);
        };
        let (true, None, FEATURE, Ok(named)) =
            (triggered, condition, failed.feature.as_str(), named)
        else {
            return Ok(false);
        };
        let mut own = Parts::default();
        self.ability(ability, label, &mut own, None)?;
        let zones = named.join(", ");
        for def in own.abilities {
            self.plan_entry(
                FEATURE,
                "abilities",
                &format!("{def}.from_zones(&[{zones}])"),
                parts,
            );
        }
        Ok(true)
    }
}

/// `SpliceOnto` as `splice_onto`, the name of its module.
fn snake(word: &str) -> String {
    let mut snake = String::new();
    for (i, letter) in word.chars().enumerate() {
        if letter.is_uppercase() && i > 0 {
            snake.push('_');
        }
        snake.extend(letter.to_lowercase());
    }
    snake
}

/// What a body with an effect is called: the words of a `Spell`'s program.
fn spell_words(body: &Ability) -> String {
    match body {
        Ability::Spell { body, .. } => ability_text(body),
        other => form(other),
    }
}

/// `rule_name` of `effects`: the words a rule's gaps go by.
fn effects_rule_name(rule: &RuleChange) -> String {
    super::effects::rule_name(rule)
}

/// The keyword a loss of `lost` is. Losing hexproof loses every "hexproof from
/// [quality]" too (CR 702.11e), and `LoseKeywords` says so of that bit.
fn lost_keyword(lost: &Operand) -> Option<String> {
    let Operand::Term {
        form: word,
        operands,
        ..
    } = lost
    else {
        return None;
    };
    match (word.as_str(), operands.as_slice()) {
        ("AnyHexproof", []) => Some("Keyword::Hexproof".to_string()),
        ("Landwalk", [Operand::Objects(ObjectSet::Battlefield(ObjectFilter::Subtype(land)))]) => {
            match name(land).as_str() {
                "Plains" => Some("Keyword::Plainswalk".to_string()),
                "Island" | "Swamp" | "Mountain" | "Forest" => {
                    Some(format!("Keyword::{}walk", name(land)))
                }
                _ => None,
            }
        }
        _ => None,
    }
}
