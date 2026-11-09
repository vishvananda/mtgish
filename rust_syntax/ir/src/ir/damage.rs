//! Damage in the normalized semantic model.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct DamagePart {
    pub recipients: Recipients,
    pub amount: Expr,
}

/// How one damage effect differs from dealing `amount` to each recipient.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum DamageRider {
    /// `amount` is a total shared evenly among the recipients.
    DividedEvenly(Rounding),
    /// Damage beyond what is lethal goes to this player instead.
    ExcessTo(PlayerRef),
    /// This much of the damage is prevented.
    Prevented(Expr),
    DividedBy(PlayerRef),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
#[serde(tag = "kind", content = "args")]
pub enum Recipients {
    Objects(ObjectSet),
    Players(PlayerSet),
    Targets(Slot),
    Union(Vec<Recipients>),
    /// The player, planeswalker or battle this creature is attacking (506.2).
    AttackedBy(ObjectRef),
    /// One of the things a plural slot targeted.
    Target {
        slot: Slot,
        index: u16,
    },
    /// What `ValueKind::Recipient` chose.
    Chosen,
    /// Whatever the matched damage was dealt to, player or object.
    DamagedByEvent,
    /// `from` of `DuplicateCounters`: the counters the event put, not all those on
    /// what it put them on.
    CountersTheEventPut,
}

/// Match actual positive damage, after prevention/replacement, using event
/// information (including a departed source's last-known characteristics).
/// Group only within one simultaneous damage event; never across instructions
/// or separate first-strike/regular combat damage events.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct DamageEvent {
    pub source_kind: ObjectKind,
    pub sources: ObjectFilter,
    pub recipients: DamageRecipients,
    pub kind: DamageKind,
    pub grouping: DamageGrouping,
    /// The role meant by the legacy distinguished object reference. A grouped
    /// role does not acquire a singular reference just because it has a subject.
    pub subject: DamageSubject,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum DamageRecipients {
    Any,
    /// The one player or object a damage-recipient target refers to.
    Target {
        slot: Slot,
        index: u16,
    },
    Objects(ObjectFilter),
    Players(PlayerSet),
    ObjectsAndPlayers {
        objects: ObjectFilter,
        players: PlayerSet,
    },
    /// Every target in this slot.
    Targets(Slot),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum DamageKind {
    Any,
    Combat,
    Noncombat,
    /// Only what went past lethal counts, and an amount reads that excess.
    Excess,
    ExcessNoncombat,
    /// Damage a prevention effect stopped, and an amount reads what it stopped.
    Prevented,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum DamageGrouping {
    /// One occurrence for each distinct source/recipient pair.
    SourceRecipient,
    Source,
    Recipient,
    Batch,
    /// One occurrence for each player controlling a source.
    SourceController,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum DamageSubject {
    Source,
    Recipient,
}
