//! Copy-time exceptions, distinct from later continuous or entry changes.
use super::*;

/// Ordered exceptions to the values acquired by a copy (CR 707.9). These are
/// part of its copiable values. Evaluate numeric and object reads in the
/// creating effect's scope as the copy is made, not each time it is queried.
/// Granted ability definitions have fresh lexical scopes: their source is the
/// copy, and their controller is the controller of that ability.
///
/// A continuing change belongs in `Change::Continuing`, and an additional
/// arrival instruction belongs in `EntrySpec`; neither is a copy exception.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
#[serde(tag = "kind", content = "args")]
pub enum CopyException {
    AddSupertypes(Vec<SuperType>),
    RemoveSupertypes(Vec<SuperType>),
    AddCardTypes(Vec<CardType>),
    SetCardTypes(Vec<CardType>),
    AddCreatureTypes(Vec<CreatureType>),
    /// Replace card types and subtypes, retaining supertypes. For example,
    /// "except it's a black Zombie" makes the copy only a Zombie creature;
    /// "Zombie in addition" uses `AddCreatureTypes` instead.
    SetTypes {
        card_types: Vec<CardType>,
        subtypes: Vec<SubType>,
    },
    AddArtifactTypes(Vec<crate::mtg_types::ArtifactType>),
    SetArtifactTypes(Vec<crate::mtg_types::ArtifactType>),
    AddLandTypes(Vec<crate::mtg_types::LandType>),
    /// Retain the copying object's types in addition to the copied types.
    MergeTypeline,
    Colors {
        mode: ChangeMode,
        colors: CopyColors,
    },
    KeepColors,
    SetName(String),
    KeepName,
    SetPowerToughness {
        power: Expr,
        toughness: Expr,
    },
    KeepPowerToughness,
    SetLoyalty(Expr),
    RemoveManaCost,
    AddAbilities(Vec<Ability>),
    AddCurrentAbility,
    AddChosenAbilities,
    AddAbilityIfAbsent {
        ability: Box<Ability>,
        matching: AbilityPattern,
    },
    RemoveAbilities(AbilityPattern),
    RemoveCurrentAbility,
    AddAbilitiesFrom {
        objects: ObjectSet,
        kinds: Vec<AbilityPattern>,
    },
    /// A stack-copy exception guarded by the copied spell's permanent types.
    /// This is not a condition evaluated by the permanent after it resolves.
    IfPermanentSpell(Vec<CopyException>),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
#[serde(tag = "kind", content = "args")]
pub enum CopyColors {
    /// Empty means colorless. All five colors are an ordinary explicit list.
    Listed(Vec<ManaColor>),
    ChosenColor,
    ChosenColors,
    ManaFrom(ResultId),
}

/// Which abilities a copy keeps, loses or gains. This matches ability
/// properties; it is not a filter over game objects or unreduced source syntax.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
#[serde(tag = "kind", content = "args")]
pub enum AbilityPattern {
    Keyword(Keyword),
    Mechanic(Mechanic),
    KeywordFamily(KeywordFamily),
    Category(AbilityCategory),
    All(Vec<AbilityPattern>),
    /// A quoted ability definition to compare, in its own lexical scope.
    Rule(Box<Ability>),
    Current,
    OtherThanCurrent,
    Sticker,
    AbilitySticker,
    HasTapInCost,
    Exhaust,
    Enchant(ObjectFilter),
    Landwalk(ObjectFilter),
    ProtectionFromColor(ManaColor),
    AnyProtectionFromColor,
    Chosen,
    /// The particular ability of the enclosing "each" effect.
    Eachable,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum AbilityCategory {
    Activated,
    Triggered,
    Loyalty,
    Mana,
    NonMana,
}
