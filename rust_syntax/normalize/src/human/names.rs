//! One vocabulary for card code, block labels and toolbox discovery.
use serde_json::{json, Value};
pub fn action_name(wire: &str) -> &str {
    match wire {
        "Draw" => "DrawCards",
        "Discard" => "DiscardCards",
        "Mill" => "MillCards",
        "Tap" => "SetTapped",
        "Move" => "MoveObjects",
        "Destroy" => "DestroyObjects",
        "Sacrifice" => "SacrificeObjects",
        _ => wire,
    }
}
pub fn action_wire(name: &str) -> &str {
    match name {
        "DrawCards" => "Draw",
        "DiscardCards" => "Discard",
        "MillCards" => "Mill",
        "SetTapped" => "Tap",
        "MoveObjects" => "Move",
        "DestroyObjects" => "Destroy",
        "SacrificeObjects" => "Sacrifice",
        _ => name,
    }
}
pub fn category(wire: &str) -> &'static str {
    match wire {
        "Draw" | "DrawUpTo" | "DrawThenDiscard" | "Discard" | "DiscardObjects"
        | "DiscardAllBut" | "Mill" | "MillUntil" | "Scry" | "Surveil" | "LookAt" | "Reveal"
        | "RevealUntil" | "RevealUntilGroup" | "RevealFromHand" | "RevealChoices" | "Shuffle"
        | "ReorderLibrary" | "Search" | "Seek" | "PickFromHand" => "Cards & libraries",
        "DealDamage"
        | "DealDamageParts"
        | "DealDamageFromEach"
        | "DealDamageWith"
        | "GainLife"
        | "LoseLife"
        | "SetLifeTotal"
        | "RedistributeLife"
        | "FightEach"
        | "CreateRegenerationShields" => "Damage & life",
        "Move" | "MoveTopCard" | "Destroy" | "Sacrifice" | "SacrificeAllBut" | "ReturnAfter"
        | "Conjure" | "CreateTokens" | "CopyCards" | "CopyEach" | "Tap" | "Attach"
        | "AttachChosen" | "AttachEach" | "Manifest" | "TurnFaceDownAs" | "GiveControl"
        | "Exchange" | "Meld" | "Populate" => "Objects & movement",
        "PutCounters"
        | "RemoveCounters"
        | "MoveCounters"
        | "DoubleCounters"
        | "DuplicateCounters"
        | "CreateContinuousEffect"
        | "CreatePerpetualEffect"
        | "EffectOn"
        | "ChangeSpell"
        | "DistributeCounters"
        | "PutCountersEach"
        | "PutChosenCounters"
        | "PutCountersCapped"
        | "PutSticker"
        | "PutStickers"
        | "CreateBoon"
        | "Intensify" => "Counters & changes",
        "ChooseObjects"
        | "ChooseValue"
        | "ChooseValueWith"
        | "ChoosePowerToughness"
        | "ChooseAction"
        | "ChooseAtRandom"
        | "SeparatePiles"
        | "Vote"
        | "FlipCoin"
        | "FlipCoins"
        | "CoinFlips"
        | "Clash"
        | "NoteObjects"
        | "BindValue"
        | "ChooseAndPay"
        | "ChooseDefender"
        | "ChoosePile"
        | "Piles"
        | "Bid"
        | "Guess"
        | "Note"
        | "JoinResults" => "Choices & values",
        "AddMana"
        | "PayCost"
        | "PayAnyAmount"
        | "PayRepeatedly"
        | "PlayCards"
        | "PlayWith"
        | "CopyOnStack"
        | "Retarget"
        | "RetargetEach"
        | "TakeSpell"
        | "SetX"
        | "LimitX"
        | "Counter"
        | "CounterWith"
        | "AnyPlayerMayPay"
        | "PlayersPayRepeatedly" => "Mana & casting",
        "CreateDelayedTrigger"
        | "CreateTrigger"
        | "ReflexiveTrigger"
        | "CreateReplacement"
        | "ModifyEvent"
        | "WithReflexive"
        | "DelayedUnlessPaid" => "Triggers & replacements",
        "RepeatProcess" | "Repeat" | "Together" | "InOrder" | "PlayersDo" | "EachPlayer"
        | "EachObject" | "EachCounterKind" | "EachNumber" | "WithSelections" | "TowardPlayer" => {
            "Control flow & batches"
        }
        "Keyword" | "KeywordEach" | "GameEffect" | "AbandonScheme" | "BeginOnBattlefield"
        | "CreateEmblem" | "AddPhases" | "ChangeCombat" | "ReduceSpeed" | "RestartGame"
        | "Wish" | "DraftFromSpellbook" => "Game & keyword actions",
        _ => "Producer forms",
    }
}
pub fn reference_name(ty: &str, wire: &str) -> Option<&'static str> {
    match (ty, wire) {
        ("PlayerRef", "EventSubject") => Some("EventPlayer"),
        ("ObjectRef", "EventSubject") => Some("EventObject"),
        ("ObjectRef", "KeywordHost") => Some("AbilityHolder"),
        ("ObjectRef", "Granter") => Some("GrantingObject"),
        _ => None,
    }
}
pub fn reference_wire<'a>(ty: &str, name: &'a str) -> &'a str {
    match (ty, name) {
        ("PlayerRef", "EventPlayer") => "EventSubject",
        ("ObjectRef", "EventObject") => "EventSubject",
        ("ObjectRef", "AbilityHolder") => "KeywordHost",
        ("ObjectRef", "GrantingObject") => "Granter",
        _ => name,
    }
}
pub fn references() -> Value {
    json!({"PlayerRef":{"EventSubject":"EventPlayer"},"ObjectRef":{"EventSubject":"EventObject","KeywordHost":"AbilityHolder","Granter":"GrantingObject"}})
}
pub fn label(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if i > 0 && c.is_uppercase() {
            out.push(' ');
        }
        out.push(c);
    }
    out
}
pub fn action(wire: &str) -> Value {
    json!({"name":action_name(wire),"label":label(action_name(wire)),"category":category(wire)})
}
pub fn field(owner: &str, name: &str) -> String {
    match (owner, name) {
        ("Card", "characteristics") => "properties".into(),
        (_, "body") => "do".into(),
        ("Operation::DealDamage" | "Operation::DealDamageParts", "source") => "from".into(),
        ("Operation::DealDamage", "recipients") => "to".into(),
        (_, "preventable") => "unpreventable".into(),
        _ => name.into(),
    }
}
