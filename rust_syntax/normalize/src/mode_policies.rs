//! Strict source/historical mode conversion, outside the semantic IR registry.
use crate::ir::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyModeSelection {
    pub minimum: Expr,
    pub maximum: Expr,
    pub repeat: bool,
    pub modes: Vec<Mode>,
    pub implied: bool,
    pub unchosen: Option<ModeHistory>,
    pub surcharge: Option<Cost>,
    pub distinct_targets: bool,
    pub chooser: Option<PlayerSet>,
    pub random: bool,
    pub budget: Option<Expr>,
    pub may_decline: bool,
}
impl LegacyModeSelection {
    pub fn current(self) -> Result<ModeSelection, String> {
        let mechanism = if self.implied {
            if self.chooser.is_some() || self.random {
                return Err(
                    "implied modes cannot also announce a chooser or random selection".into(),
                );
            }
            ModeMechanism::Implied
        } else {
            ModeMechanism::Announced {
                chooser: self
                    .chooser
                    .map_or(ModeChooser::Controller, ModeChooser::OneOf),
                method: if self.random {
                    ModePicking::Random
                } else {
                    ModePicking::PlayerChoice
                },
            }
        };
        Ok(ModeSelection {
            count: ModeCount {
                minimum: self.minimum,
                maximum: self.maximum,
            },
            options: self.modes,
            rules: ModeRules {
                repetition: if self.repeat {
                    ModeRepetition::MayRepeat
                } else {
                    ModeRepetition::OnceEach
                },
                availability: self
                    .unchosen
                    .map_or(ModeAvailability::Any, ModeAvailability::Unchosen),
                surcharge: self.surcharge,
                targets: if self.distinct_targets {
                    ModeTargetPolicy::Distinct
                } else {
                    ModeTargetPolicy::MayShare
                },
                acceptance: if self.may_decline {
                    ModeAcceptance::MayDecline
                } else {
                    ModeAcceptance::Required
                },
            },
            mechanism,
            budget: self.budget,
        })
    }
}

/// Borrowed compiler view. Typed policies remain the sole authorable form.
pub(crate) struct Parts<'a> {
    pub minimum: &'a Expr,
    pub maximum: &'a Expr,
    pub repeat: bool,
    pub modes: &'a Vec<Mode>,
    pub implied: bool,
    pub unchosen: Option<&'a ModeHistory>,
    pub surcharge: Option<&'a Cost>,
    pub distinct_targets: bool,
    pub chooser: Option<&'a PlayerSet>,
    pub random: bool,
    pub budget: Option<&'a Expr>,
    pub may_decline: bool,
}
pub(crate) fn parts(selection: &ModeSelection) -> Parts<'_> {
    let (implied, chooser, random) = match &selection.mechanism {
        ModeMechanism::Implied => (true, None, false),
        ModeMechanism::Announced { chooser, method } => (
            false,
            match chooser {
                ModeChooser::Controller => None,
                ModeChooser::OneOf(players) => Some(players),
            },
            *method == ModePicking::Random,
        ),
    };
    Parts {
        minimum: &selection.count.minimum,
        maximum: &selection.count.maximum,
        repeat: selection.rules.repetition == ModeRepetition::MayRepeat,
        modes: &selection.options,
        implied,
        unchosen: match &selection.rules.availability {
            ModeAvailability::Any => None,
            ModeAvailability::Unchosen(history) => Some(history),
        },
        surcharge: selection.rules.surcharge.as_ref(),
        distinct_targets: selection.rules.targets == ModeTargetPolicy::Distinct,
        chooser,
        random,
        budget: selection.budget.as_ref(),
        may_decline: selection.rules.acceptance == ModeAcceptance::MayDecline,
    }
}
