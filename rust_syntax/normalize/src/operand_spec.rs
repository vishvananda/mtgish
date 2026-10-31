//! Reviewed cost/condition operands. Tables select shared semantic recipes;
//! generated Rust verifies source shapes while validation accounts for fields.
use crate::number_spec::{Formula, Object, Players};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    pub version: u32,
    pub rules: Vec<Rule>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub id: String,
    pub domain: Domain,
    pub source: String,
    pub arity: usize,
    pub recipe: Recipe,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Domain {
    Cost,
    Condition,
}
impl Domain {
    pub fn name(self) -> &'static str {
        match self {
            Self::Cost => "Cost",
            Self::Condition => "Condition",
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Collection {
    Permanent,
    Permanents,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum AmountOperation {
    Life,
    Draw,
    Mill,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Selection {
    Exactly { amount: Formula },
    AnyNumber,
    OneOrMore,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum DiscardFilter {
    Any,
    Cards { argument: usize },
    HandCard { argument: usize },
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum MatchDomain {
    Permanents,
    Spells,
    Cards,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Zone {
    Battlefield,
    Graveyard,
    Hand,
    Library,
    Stack,
    Exile,
    Command,
    Ante,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Recipe {
    CostAll {
        argument: usize,
    },
    CostChoice {
        argument: usize,
    },
    CostMana {
        argument: usize,
    },
    CostManaX {
        mana: usize,
        variable: usize,
    },
    CostTapPermanent {
        argument: usize,
        tapped: bool,
    },
    CostTapObjects {
        subjects: Collection,
        argument: usize,
        selection: Option<Selection>,
        tapped: bool,
        group: Option<usize>,
    },
    CostAmount {
        operation: AmountOperation,
        amount: Formula,
    },
    CostLoyalty {
        argument: usize,
    },
    CostOwnMana,
    CostSacrifice {
        subjects: Collection,
        argument: usize,
        selection: Option<Selection>,
        group: Option<usize>,
    },
    CostDiscard {
        amount: Formula,
        filter: DiscardFilter,
        random: bool,
        group: Option<usize>,
    },
    CostReturn {
        subjects: usize,
        amount: Formula,
    },
    CostExile {
        items: usize,
        flags: usize,
    },
    CostRemoveCounters {
        argument: usize,
    },
    CheckAll {
        argument: usize,
    },
    CheckEither {
        argument: usize,
    },
    CheckPayment {
        negated: bool,
    },
    CheckCompare {
        left: Formula,
        comparison: usize,
    },
    CheckSpellX {
        comparison: usize,
    },
    CheckMatches {
        object: Object,
        domain: MatchDomain,
        filter: usize,
    },
    CheckActive {
        players: Players,
        negated: bool,
    },
    CheckPlayer {
        players: Players,
        filter: usize,
    },
    CheckSourceZones {
        zones: Vec<Zone>,
    },
    CheckGraveyard {
        object: Option<usize>,
        owner: Option<usize>,
    },
}
impl Selection {
    fn arguments(&self, out: &mut Vec<usize>) -> Result<(), String> {
        if let Self::Exactly { amount } = self {
            amount.arguments(out)?;
        }
        Ok(())
    }
    fn generate(&self) -> String {
        match self {
            Self::Exactly { amount } => format!("CostSelection::Exactly({})", amount.generate()),
            Self::AnyNumber => "CostSelection::AnyNumber".into(),
            Self::OneOrMore => "CostSelection::OneOrMore".into(),
        }
    }
}
impl Recipe {
    fn arguments(&self, out: &mut Vec<usize>) -> Result<(), String> {
        match self {
            Self::CostAll { argument }
            | Self::CostChoice { argument }
            | Self::CostMana { argument }
            | Self::CostTapPermanent { argument, .. }
            | Self::CostLoyalty { argument }
            | Self::CostRemoveCounters { argument }
            | Self::CheckAll { argument }
            | Self::CheckEither { argument } => out.push(*argument),
            Self::CostManaX { mana, variable } => out.extend([mana, variable]),
            Self::CostAmount { amount, .. } => amount.arguments(out)?,
            Self::CostOwnMana | Self::CheckPayment { .. } => {}
            Self::CostSacrifice {
                argument,
                selection,
                group,
                ..
            }
            | Self::CostTapObjects {
                argument,
                selection,
                group,
                ..
            } => {
                out.push(*argument);
                if let Some(s) = selection {
                    s.arguments(out)?;
                }
                out.extend(group);
            }
            Self::CostDiscard {
                amount,
                filter,
                group,
                ..
            } => {
                out.extend(group);
                amount.arguments(out)?;
                match filter {
                    DiscardFilter::Any => {}
                    DiscardFilter::Cards { argument } | DiscardFilter::HandCard { argument } => {
                        out.push(*argument)
                    }
                }
            }
            Self::CostReturn { subjects, amount } => {
                out.push(*subjects);
                amount.arguments(out)?;
            }
            Self::CostExile { items, flags } => out.extend([items, flags]),
            Self::CheckCompare { left, comparison } => {
                left.arguments(out)?;
                out.push(*comparison);
            }
            Self::CheckSpellX { comparison } => out.push(*comparison),
            Self::CheckMatches { object, filter, .. } => {
                object.arguments(out)?;
                out.push(*filter);
            }
            Self::CheckActive { players, .. } => out.push(players.argument()),
            Self::CheckPlayer { players, filter } => {
                out.push(players.argument());
                out.push(*filter);
            }
            Self::CheckSourceZones { zones } => {
                if zones.is_empty() {
                    return Err("empty source zone condition".into());
                }
            }
            Self::CheckGraveyard { object, owner } => {
                out.extend(object);
                out.extend(owner);
            }
        }
        Ok(())
    }
    fn generate(&self) -> String {
        let recipe=match self {
            Self::CostAll {argument}=>format!("CostRecipe::All(a{argument}.iter().map(cost_tree).collect())"),
            Self::CostChoice {argument}=>format!("CostRecipe::Choice(a{argument}.iter().map(cost_tree).collect())"),
            Self::CostMana {argument}=>format!("CostRecipe::Mana(a{argument}.clone())"),
            Self::CostManaX {mana,variable}=>format!("CostRecipe::ManaX {{mana:a{mana}.clone(),variable:source::GameNumber::clone(a{variable})}}"),
            Self::CostTapPermanent {argument,tapped}=>format!("CostRecipe::TapPermanent {{object:source::Permanent::clone(a{argument}),tapped:{tapped}}}"),
            Self::CostTapObjects {subjects,argument,selection,tapped,..}=>format!("CostRecipe::TapObjects {{subjects:super::Subjects::{subjects:?}(source::{subjects:?}::clone(a{argument})),selection:{},tapped:{tapped}}}",selection.as_ref().map_or("None".into(),|s|format!("Some({})",s.generate()))),
            Self::CostAmount {operation,amount}=>format!("CostRecipe::Amount {{operation:AmountOperation::{operation:?},amount:{}}}",amount.generate()),
            Self::CostLoyalty {argument}=>format!("CostRecipe::Loyalty(i64::from(*a{argument}))"),Self::CostOwnMana=>"CostRecipe::OwnMana".into(),
            Self::CostSacrifice {subjects,argument,selection,..}=>format!("CostRecipe::Sacrifice {{subjects:super::Subjects::{subjects:?}(source::{subjects:?}::clone(a{argument})),selection:{}}}",selection.as_ref().map_or("None".into(),|s|format!("Some({})",s.generate()))),
            Self::CostDiscard {amount,filter,random,..}=> {
                let filter=match filter {DiscardFilter::Any=>"CostFilter::Any".into(),DiscardFilter::Cards {argument}=>format!("CostFilter::Cards(super::selectors::cards_tree(a{argument}))"),DiscardFilter::HandCard {argument}=>format!("CostFilter::HandCard(source::CardInHand::clone(a{argument}))")};
                format!("CostRecipe::Discard {{amount:{},filter:{filter},random:{random}}}",amount.generate())
            }
            Self::CostReturn {subjects,amount}=>format!("CostRecipe::Return {{subjects:source::Permanents::clone(a{subjects}),amount:{}}}",amount.generate()),
            Self::CostExile {items,flags}=>format!("CostRecipe::Exile {{items:a{items}.clone(),flags:a{flags}.clone()}}"),Self::CostRemoveCounters {argument}=>format!("CostRecipe::RemoveCounters(a{argument}.clone())"),
            Self::CheckAll {argument}=>format!("CheckRecipe::All(a{argument}.iter().map(check_tree).collect())"),Self::CheckEither {argument}=>format!("CheckRecipe::Either(a{argument}.iter().map(check_tree).collect())"),
            Self::CheckPayment {negated}=>format!("CheckRecipe::Payment {{negated:{negated}}}"),
            Self::CheckCompare {left,comparison}=>format!("CheckRecipe::Compare {{left:{},comparison:source::Comparison::clone(a{comparison})}}",left.generate()),
            Self::CheckSpellX {comparison}=>format!("CheckRecipe::SpellX(source::Comparison::clone(a{comparison}))"),
            Self::CheckMatches {object,domain,filter}=> {
                let tree=match domain {MatchDomain::Permanents=>"permanents_tree",MatchDomain::Spells=>"spells_tree",MatchDomain::Cards=>"cards_tree"};
                format!("CheckRecipe::Matches {{object:{},domain:MatchDomain::{domain:?},filter:super::selectors::{tree}(a{filter})}}",object.generate())
            }
            Self::CheckActive {players,negated}=>format!("CheckRecipe::Active {{players:{},negated:{negated}}}",players.generate()),
            Self::CheckPlayer {players,filter}=>format!("CheckRecipe::Player {{players:{},filter:source::Players::clone(a{filter})}}",players.generate()),
            Self::CheckSourceZones {zones}=>format!("CheckRecipe::SourceZones(vec![{}])",zones.iter().map(|z|format!("ir::EventZone::{z:?}")).collect::<Vec<_>>().join(",")),
            Self::CheckGraveyard {object,owner}=>format!("CheckRecipe::Graveyard {{object:{},owner:{}}}",object.map_or("None".into(),|i|format!("Some(source::CardInGraveyards::clone(a{i}))")),owner.map_or("source::Player::You".into(),|i|format!("source::Player::clone(a{i})"))),
        };
        let group = match self {
            Self::CostTapObjects { group, .. }
            | Self::CostSacrifice { group, .. }
            | Self::CostDiscard { group, .. } => *group,
            _ => None,
        };
        group.map_or(recipe.clone(),|i|format!("CostRecipe::Constrained {{cost:Box::new({recipe}),group:source::GroupFilter::clone(a{i})}}"))
    }
    fn is_cost(&self) -> bool {
        matches!(
            self,
            Self::CostAll { .. }
                | Self::CostChoice { .. }
                | Self::CostMana { .. }
                | Self::CostManaX { .. }
                | Self::CostTapPermanent { .. }
                | Self::CostTapObjects { .. }
                | Self::CostAmount { .. }
                | Self::CostLoyalty { .. }
                | Self::CostOwnMana
                | Self::CostSacrifice { .. }
                | Self::CostDiscard { .. }
                | Self::CostReturn { .. }
                | Self::CostExile { .. }
                | Self::CostRemoveCounters { .. }
        )
    }
}
impl Registry {
    pub fn parse(text: &str) -> Result<Self, String> {
        let r: Self = serde_json::from_str(text).map_err(|e| e.to_string())?;
        r.validate()?;
        Ok(r)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 3 {
            return Err("unsupported cost/condition registry version".into());
        }
        let mut ids = BTreeSet::new();
        let mut sources = BTreeSet::new();
        for r in &self.rules {
            if r.id.is_empty() || !ids.insert(&r.id) || !sources.insert((r.domain, &r.source)) {
                return Err("empty/duplicate cost/condition rule".into());
            }
            if r.source.is_empty()
                || !r.source.chars().enumerate().all(|(i, c)| {
                    c == '_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit())
                })
            {
                return Err("invalid Rust operand variant".into());
            }
            if r.recipe.is_cost() != (r.domain == Domain::Cost) {
                return Err("operand domain/recipe mismatch".into());
            }
            if matches!(
                &r.recipe,
                Recipe::CostTapObjects {
                    subjects: Collection::Permanent,
                    selection: Some(_),
                    ..
                }
            ) {
                return Err("object cost selection requires a collection".into());
            }
            let mut args = vec![];
            r.recipe.arguments(&mut args)?;
            let mut used = BTreeSet::new();
            for i in args {
                if i >= r.arity || !used.insert(i) {
                    return Err("invalid/reused operand field".into());
                }
            }
            if used.len() != r.arity {
                return Err("unconsumed operand field".into());
            }
        }
        Ok(())
    }
    pub fn generate(&self) -> String {
        let mut out = "// Generated from rewrites/operands.json5.\n".to_string();
        for (domain, name, output) in [
            (Domain::Cost, "generated_cost", "CostRecipe"),
            (Domain::Condition, "generated_check", "CheckRecipe"),
        ] {
            out.push_str(&format!(
                "fn {name}(value:&source::{})->Option<{output}> {{Some(match value {{\n",
                domain.name()
            ));
            for r in self.rules.iter().filter(|r| r.domain == domain) {
                let args = if r.arity == 0 {
                    String::new()
                } else {
                    format!(
                        "({})",
                        (0..r.arity)
                            .map(|i| format!("a{i}"))
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                };
                out.push_str(&format!(
                    "source::{}::{}{args}=>{},\n",
                    domain.name(),
                    r.source,
                    r.recipe.generate()
                ));
            }
            out.push_str("_=>return None,\n})}\n");
        }
        out
    }
}
