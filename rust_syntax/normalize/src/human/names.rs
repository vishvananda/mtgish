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
        "Tap" => "SetTapped",
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
        | "Conjure" | "CreateTokens" | "CopyCards" | "CopyEach" | "SetTapped" | "Attach"
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
        | "ChooseAndPay"
        | "ChooseDefender"
        | "ChoosePile"
        | "Piles"
        | "Bid"
        | "Guess"
        | "Note" => "Choices & values",
        "NoteObjects" | "BindValue" | "JoinResults" => "Results & receipts",
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
        "Together" | "InOrder" => "Sequence & batches",
        "RepeatProcess" | "Repeat" | "PlayersDo" | "EachPlayer" | "EachObject"
        | "EachCounterKind" | "EachNumber" | "TowardPlayer" => "Iteration & repetition",
        "WithSelections" => "Local selections",
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
        ("ObjectRef", "AbilityHolder") => Some("AbilityHolder"),
        ("ObjectRef", "GrantingObject") => Some("GrantingObject"),
        _ => None,
    }
}
pub fn reference_wire<'a>(ty: &str, name: &'a str) -> &'a str {
    match (ty, name) {
        ("PlayerRef", "EventPlayer") => "EventSubject",
        ("ObjectRef", "EventObject") => "EventSubject",
        ("ObjectRef", "KeywordHost") => "AbilityHolder",
        ("ObjectRef", "Granter") => "GrantingObject",
        ("ObjectRef" | "ObjectSet", "Named") => "Recorded",
        ("Expr", "Power") => "Exponentiate",
        _ => name,
    }
}
pub fn references() -> Value {
    json!({"PlayerRef":{"EventSubject":"EventPlayer"},"ObjectRef":{"EventSubject":"EventObject","AbilityHolder":"AbilityHolder","GrantingObject":"GrantingObject"}})
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
    let category = category(wire);
    let control = matches!(
        category,
        "Sequence & batches" | "Iteration & repetition" | "Local selections" | "Results & receipts"
    );
    json!({"name":action_name(wire),"label":label(action_name(wire)),"category":category,"control":control})
}
/// Specific keyword starters use the same topic names as ordinary actions.
pub fn keyword_action(wire:&str,each:bool)->Value {
    let category=match wire {
        "Fight"|"RemoveAllDamage"=>"Damage & life",
        "Investigate"|"Amass"|"ManifestDread"|"Incubate"|"Populate"=>"Copies & tokens",
        "Proliferate"|"Monstrosity"|"EmpowerJace"|"Adapt"|"Bolster"|"Endure"|"Blight"|"TimeTravel"|"Intensify"|"GainEnergy"|"GetTickets"=>"Counters & changes",
        "RollDie"|"RollKeepHighest"|"RollTwoInOrder"|"RollPlanarDie"|"VisitAttractions"|"ChooseDamageNumber"|"ChooseSector"|"ChooseProtection"|"ChooseCopy"|"SetStake"|"DoubleStake"|"RerollStoredDice"|"RollAndStoreD6"=>"Choices & values",
        "Explore"|"Connive"|"Discover"|"Learn"|"Fateseal"|"CollectEvidence"|"ShuffleGraveyard"|"ShuffleIfSearched"|"ReorderGraveyard"|"DiscardAnyNumberAtRandom"|"RemainingToBottomAtRandom"|"TurnPileFaceUp"=>"Cards & libraries",
        "Transform"|"Convert"|"Prepare"|"Unprepare"|"PhaseIn"|"PhaseOut"|"TapOrUntap"|"ExchangeControl"|"Airbend"|"Flip"|"Awaken"|"TurnFaceUp"|"TurnFaceDown"|"Unattach"|"UnlockDoor"|"LockOrUnlockDoor"|"SetSchemeInMotion"=>"Objects & movement",
        "Goad"|"Ungoad"|"Suspect"|"Unsuspect"|"Detain"|"Saddle"|"Harness"|"BecomeBlocked"|"RemoveFromCombat"|"RemoveFromCombatUnblocking"=>"Combat & restrictions",
        _=>"Game & keyword actions",
    };
    let label=if each {format!("{} each object",label(wire))}else{label(wire)};
    json!({"name":wire,"label":label,"category":category})
}
/// Instruction discovery shares the action families without conflating their
/// execution boundaries. Labels describe a constructor, not a wire rename.
pub fn instruction(wire: &str) -> Value {
    let (label, category) = match wire {
        "Group" => ("Sequence of instructions", "Sequence & batches"),
        "If" => ("If / otherwise", "Conditions & choices"),
        "Optional" => ("Optional instructions", "Conditions & choices"),
        "ChooseOne" => ("Choose an option", "Conditions & choices"),
        "ForEachPlayer" => ("For each player", "Iteration & repetition"),
        "ForEachObject" => ("For each object", "Iteration & repetition"),
        "Repeatable" => ("Repeatable process", "Iteration & repetition"),
        "Times" => (
            "Repeatable process with a run limit",
            "Iteration & repetition",
        ),
        _ => (wire, "Instructions"),
    };
    json!({"name":wire,"label":label,"category":category})
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

/// Read-only compatibility aliases. Writers and editor metadata use the
/// current semantic roles, so aliases cannot reintroduce the old wire fields.
pub fn legacy_field<'a>(owner: &str, field: &'a str) -> Option<&'a str> {
    match (owner,field) {
        ("Operation::Search","searcher") | ("Operation::ReorderLibrary","chooser") => Some("player"),
        ("Operation::Search" | "Operation::ReorderLibrary","library_owner") => Some("library"),
        ("Operation::ChooseValue","choosers") => Some("actor"),
        _ => None,
    }
}
