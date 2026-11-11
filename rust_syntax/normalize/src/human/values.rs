//! Authoring-only roles for numeric values and predicates. This metadata is built
//! once for clients; it adds nothing to stored cards or native AST traversal.
use serde_json::{json, Value};
pub(super) fn metadata() -> Value {
    json!({
        "binary":["Add","Subtract","Multiply","Minimum","Maximum","Exponentiate","Difference"],
        "variables":["AnnouncedX","EventAmount","ChosenNumber","ChosenParity","TurnNumber","TimesActivatedThisTurn","TimesResolvedThisTurn"],
        "labels":{
            "Constant":"number","Add":"+","Subtract":"−","Multiply":"×","Minimum":"min","Maximum":"max","Exponentiate":"^","Difference":"absolute difference","Divide":"÷",
            "AnnouncedX":"X","EventAmount":"event amount","ChosenNumber":"chosen number","ChosenParity":"chosen parity","TurnNumber":"turn number","TimesActivatedThisTurn":"activations this turn","TimesResolvedThisTurn":"resolutions this turn",
            "ObjectProperty":"property","PlayerNumber":"player value","CountPlayers":"number of players","Conditional":"if","Compare":"compare","Not":"not","AllOf":"all of","AnyOf":"any of","AnyProperty":"matches a property value",
            "PlayerIsActive":"is the active player","PlayerIn":"is among","ObjectMatches":"matches","PlayerState":"player has","Window":"during"
        },
        "list_types":["Condition","NumberTest"],
        "list_variants":["AllOf","AnyOf"],
        "predicate_type":"NumberTest",
        "comparison_fields":{"comparison":"","value":""},
        "property_fields":{"object":"of","property":"","information":"read using"}
    })
}
