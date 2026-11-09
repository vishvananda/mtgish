//! Typed target construction over the stable normalized wire representation.
use crate::ir::*;
use serde::{Deserialize, Serialize};

/// The domain and its applicable restrictions travel together. An object filter
/// never silently applies to players; damage targets have two separate domains.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "args")]
pub enum TargetSelector {
    Objects {
        domain: ObjectDomain,
        filter: ObjectFilter,
    },
    Players {
        filter: Option<PlayerSet>,
    },
    Damageable {
        objects: ObjectFilter,
        players: Option<PlayerSet>,
    },
}
impl TargetSelector {
    pub fn any_target() -> Self {
        Self::Damageable {
            objects: ObjectFilter::Any,
            players: None,
        }
    }
    pub fn creature() -> Self {
        Self::Objects {
            domain: ObjectDomain::permanent(),
            filter: ObjectFilter::CardType(crate::mtg_types::CardType::Creature),
        }
    }
    pub fn planeswalker() -> Self {
        Self::Objects {
            domain: ObjectDomain::permanent(),
            filter: ObjectFilter::CardType(crate::mtg_types::CardType::Planeswalker),
        }
    }
    pub fn player() -> Self {
        Self::Players { filter: None }
    }
    pub fn into_spec(self, slot: TargetSlot) -> TargetSpec {
        let mut spec = TargetSpec {
            slot,
            kind: TargetKind::Player,
            count: Count::Exactly(Expr::Constant(1)),
            filter: ObjectFilter::Any,
            players: None,
            distribute: None,
            constraints: vec![],
            chooser: None,
        };
        spec.set_selector(self);
        spec
    }
}
impl TargetSpec {
    pub fn selector(&self) -> Result<TargetSelector, String> {
        Ok(match &self.kind {
            TargetKind::Object(domain) => {
                if self.players.is_some() {
                    return Err("object targets cannot carry a player filter".into());
                }
                TargetSelector::Objects {
                    domain: domain.clone(),
                    filter: self.filter.clone(),
                }
            }
            TargetKind::Player => {
                if !matches!(self.filter, ObjectFilter::Any) {
                    return Err("player targets cannot carry an object filter".into());
                }
                TargetSelector::Players {
                    filter: self.players.clone(),
                }
            }
            TargetKind::DamageRecipient => TargetSelector::Damageable {
                objects: self.filter.clone(),
                players: self.players.clone(),
            },
        })
    }
    pub fn set_selector(&mut self, selector: TargetSelector) {
        let (kind, filter, players) = match selector {
            TargetSelector::Objects { domain, filter } => {
                (TargetKind::Object(domain), filter, None)
            }
            TargetSelector::Players { filter } => (TargetKind::Player, ObjectFilter::Any, filter),
            TargetSelector::Damageable { objects, players } => {
                (TargetKind::DamageRecipient, objects, players)
            }
        };
        self.kind = kind;
        self.filter = filter;
        self.players = players;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn changing_domain_replaces_only_applicable_filters() {
        let mut spec = TargetSelector::creature().into_spec(TargetSlot(7));
        spec.count = Count::UpTo(Expr::Constant(3));
        spec.set_selector(TargetSelector::player());
        assert!(matches!(
            spec.selector().unwrap(),
            TargetSelector::Players { filter: None }
        ));
        assert_eq!(spec.slot.0, 7);
        assert!(matches!(spec.count, Count::UpTo(Expr::Constant(3))));
        spec.filter = ObjectFilter::CardType(crate::mtg_types::CardType::Creature);
        assert!(spec.selector().is_err());
        spec.set_selector(TargetSelector::creature());
        spec.players = Some(PlayerSet::All);
        assert!(spec.selector().is_err());
    }
}
