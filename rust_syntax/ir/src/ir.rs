//! Portable semantic card definitions. No function pointers or runtime object
//! IDs belong here. Strict lowering rejects unsupported source constructs.
//! The normalization crate represents unsupported forms as `Named`: typed,
//! reported as unreduced, and never accepted as complete card definitions.

use crate::mtg_types::{CardType, CreatureType, SubType, SuperType};
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 179;

/// Typed pattern for the permanent query of the shared battlefield. Card-only,
/// player-scoped and other-zone queries deliberately do not match.
#[macro_export]
macro_rules! battlefield_query {
    ($($filter:tt)+) => {
        $crate::ir::ObjectSet::Zone {
            scope: $crate::ir::ZoneScope::Shared($crate::ir::EventZone::Battlefield),
            kind: $crate::ir::ObjectKind::Permanent,
            filter: $($filter)+
        }
    };
}

pub mod cards;
pub use self::cards::*;
pub mod abilities;
pub use self::abilities::*;
pub mod program;
pub use self::program::*;
pub mod selection;
pub use self::selection::*;
pub mod movement;
pub use self::movement::*;
pub mod objects;
pub use self::objects::*;
pub mod players;
pub use self::players::*;
pub mod values;
pub use self::values::*;
pub mod events;
pub use self::events::*;
pub mod changes;
pub use self::changes::*;
pub mod costs;
pub use self::costs::*;
pub mod mana;
pub use self::mana::*;
pub mod damage;
pub use self::damage::*;
pub mod keywords;
pub use self::keywords::*;
pub mod actions;
pub use self::actions::*;
pub mod conditions;
pub use self::conditions::*;
pub mod tokens;
pub use self::tokens::*;
