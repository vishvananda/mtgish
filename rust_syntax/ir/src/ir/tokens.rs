//! Tokens in the normalized semantic model.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct TokenSpec {
    pub amount: Expr,
    /// Text-changing effects on the creating spell do not rewrite a predefined
    /// token's rules-defined recipe (CR 111.10, 612.2a).
    pub words: TokenWords,
    pub recipe: TokenRecipe,
}

impl TokenSpec {
    pub fn definition(&self) -> Option<&TokenDefinition> {
        match &self.recipe {
            TokenRecipe::Defined(definition) => Some(definition),
            _ => None,
        }
    }
    pub fn definition_mut(&mut self) -> Option<&mut TokenDefinition> {
        match &mut self.recipe {
            TokenRecipe::Defined(definition) => Some(definition),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum TokenRecipe {
    Defined(TokenDefinition),
    /// A copy of `object` (707.2), with `adjustments` to the copiable values.
    Copy {
        object: ObjectRef,
        adjustments: Vec<Operand>,
    },
    /// One copy of each of `objects`.
    CopyEach {
        objects: ObjectSet,
        adjustments: Vec<Operand>,
    },
    /// The token Oracle gives this name, defined outside the card that makes it.
    Named(String),
    /// A copy of one of `objects`, picked as the effect resolves.
    CopyOne {
        objects: ObjectSet,
        adjustments: Vec<Operand>,
    },
    /// A copy of the Oracle card with this name, which is not in the game.
    CopyNamed {
        name: String,
        adjustments: Vec<Operand>,
    },
    /// A copy of a random Oracle card matching `cards`.
    CopyAtRandom {
        cards: Operand,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum TokenWords {
    CreatingEffect,
    Rules,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct TokenDefinition {
    pub name: TokenName,
    pub colors: Vec<ManaColor>,
    /// Also the color chosen earlier, which `colors` can't name.
    pub chosen_color: bool,
    /// Also the creature type chosen earlier, which `subtypes` can't name.
    pub chosen_creature_type: bool,
    pub supertypes: Vec<SuperType>,
    pub card_types: Vec<CardType>,
    pub subtypes: Vec<SubType>,
    pub power_toughness: Option<(Expr, Expr)>,
    /// Fresh scopes: "this" is the token, and "you" its ability's controller.
    pub abilities: Vec<Ability>,
    /// "And all triggered abilities of the exiled cards": objects and kinds of
    /// abilities read in the creating effect's scope as the token is made.
    pub abilities_of: Option<(ObjectSet, Box<Operand>)>,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum TokenName {
    /// CR 111.4: subtype names followed by "Token".
    FromSubtypes,
    Explicit(String),
}
