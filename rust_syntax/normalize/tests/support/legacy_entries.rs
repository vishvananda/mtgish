//! Explicit entry/copy/characteristic upgrades for immutable historical captures.
#![allow(dead_code)]
use mtgish_ast::{copying, ir::*, movement,keyword_actions,mode_policies};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Deserialize)]
struct OldShape {
    power_toughness: Operand,
    creature_type: Option<Operand>,
    artifact: bool,
}
impl OldShape {
    fn current(self) -> mtgish_ast::ir::FaceDownAs {
        movement::face_down_from_source(
            &self.power_toughness,
            self.creature_type.as_ref(),
            self.artifact,
        )
        .unwrap()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OldKeyword {
    actor:PlayerSet, action:keyword_actions::KeywordKind,
    object:Option<ObjectRef>, other:Option<ObjectRef>, amount:Option<Expr>, subtype:Option<String>,
}
#[derive(Serialize)]
struct KeywordArgs { actor:PlayerSet,action:KeywordAction }
impl OldKeyword {
    fn current(self)->KeywordArgs {KeywordArgs {actor:self.actor,action:keyword_actions::from_parts(self.action,self.object,self.other,self.amount,self.subtype).unwrap()}}
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OldEach { actor:PlayerSet, action:keyword_actions::KeywordKind, objects:ObjectSet,until:Option<Duration> }
#[derive(Serialize)]
struct EachArgs {actor:PlayerSet,action:ObjectKeywordAction,objects:ObjectSet}
impl OldEach {
    fn current(self)->EachArgs {EachArgs {actor:self.actor,action:keyword_actions::each_from_parts(self.action,self.until).unwrap(),objects:self.objects}}
}

const OPERATIONS: &[&str] = &[
    "Conjure",
    "MoveTopCard",
    "Manifest",
    "Meld",
    "CounterWith",
    "Populate",
];

fn copy_json(fields: &mut serde_json::Map<String, Value>, old: &str, single: bool) {
    if fields.contains_key("exceptions") {
        return;
    }
    let Some(value) = fields.remove(old) else {
        return;
    };
    let values = if single { json!([value]) } else { value };
    let operands: Vec<Operand> = serde_json::from_value(values).unwrap();
    fields.insert(
        "exceptions".into(),
        serde_json::to_value(copying::from_source(&operands).unwrap()).unwrap(),
    );
}
pub fn json(value: &mut Value) {
    // Quoted abilities can themselves contain old arrivals/copies. Upgrade
    // children before a containing Operand is deserialized into today's types.
    match value {
        Value::Object(fields) => fields.values_mut().for_each(json),
        Value::Array(items) => items.iter_mut().for_each(json),
        _ => (),
    }
    api_names(value);
    if value.get("minimum").is_some() && value.get("modes").is_some() {
        let old:mode_policies::LegacyModeSelection=serde_json::from_value(value.clone()).unwrap();
        *value=serde_json::to_value(old.current().unwrap()).unwrap();
    }
    if value
        .get("kind")
        .and_then(Value::as_str)
        .is_some_and(|k| OPERATIONS.contains(&k))
    {
        if let Some(entry) = value.get_mut("args").and_then(|a| a.get_mut("entry")) {
            if entry.is_array() {
                let flags: Vec<Operand> = serde_json::from_value(entry.clone()).unwrap();
                *entry = serde_json::to_value(movement::entry_from_source(&flags)).unwrap();
            }
        }
    }
    let kind = value.get("kind").and_then(Value::as_str).map(str::to_owned);
    if kind.as_deref()==Some("Keyword") && value["args"].get("object").is_some() {
        let args:OldKeyword=serde_json::from_value(value["args"].clone()).unwrap();
        value["args"]=serde_json::to_value(args.current()).unwrap();
    }
    if kind.as_deref()==Some("KeywordEach") && value["args"].get("until").is_some() {
        let args:OldEach=serde_json::from_value(value["args"].clone()).unwrap();
        value["args"]=serde_json::to_value(args.current()).unwrap();
    }

    if matches!(kind.as_deref(), Some("CopyOnStack" | "CopyOf")) {
        if let Some(fields) = value.get_mut("args").and_then(Value::as_object_mut) {
            copy_json(fields, "effects", kind.as_deref() == Some("CopyOf"));
        }
    }
    for variant in ["Copy", "CopyEach", "CopyOne", "CopyNamed", "CopyOf"] {
        if let Some(fields) = value.get_mut(variant).and_then(Value::as_object_mut) {
            copy_json(
                fields,
                if variant == "CopyOf" {
                    "effects"
                } else {
                    "adjustments"
                },
                variant == "CopyOf",
            );
        }
    }
    if value.get("power_toughness").is_some()
        && value.get("creature_type").is_some()
        && value.get("artifact").is_some()
    {
        let old: OldShape = serde_json::from_value(value.clone()).unwrap();
        *value = serde_json::to_value(old.current()).unwrap();
    }
    if let Some(fields) = value.as_object_mut() {
        if fields.contains_key("tapped")
            && fields.contains_key("counters")
            && fields.contains_key("states")
        {
            fields.entry("controller").or_insert(json!(null));
        }
    }
}

#[derive(Clone, Copy)]
enum Upgrade {
    Entry,
    Copy(bool),
    Shape,
    Modes,
    Keyword(bool),
}

fn owner(input: &str, open: usize) -> Option<&str> {
    let after = input[open + 1..].trim_start();
    if let Some(after) = after.strip_prefix("kind:") {
        return after
            .split_once(',')
            .map(|(name, _)| name.trim().trim_matches('"'));
    }
    let before = &input[..open];
    let start = before
        .rfind(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .map_or(0, |i| i + 1);
    (start < before.len()).then_some(&before[start..])
}

fn has_legacy_keyword_field(input:&str,each:bool)->bool {
    let field=if each {"until:"}else{"other:"};
    let (mut depth,mut quoted,mut escaped)=(0usize,false,false);
    for (i,c) in input.char_indices() {
        if quoted {if escaped {escaped=false;}else if c=='\\' {escaped=true;}else if c=='"' {quoted=false;}continue;}
        if c=='"' {quoted=true;continue;}
        if depth==1 && input[i..].starts_with(field) {return true;}
        match c {'('| '['|'{' =>depth+=1,')'|']'|'}'=>{depth-=1;if depth==0 {return false;}},_=>{}}
    }
    false
}

/// RON's deserializer consumes each old payload with its actual type. Process
/// the deepest eligible field first, including old fields in quoted abilities;
/// draft ChangeZone's intentionally generic entry vector remains untouched.
pub fn ron<T: serde::de::DeserializeOwned>(input: &str) -> T {
    let mut input = ron_api_names(input);
    for _ in 0..65_536 {
        let bytes = input.as_bytes();
        let (mut i, mut quoted, mut escaped) = (0, false, false);
        let mut stack = Vec::new();
        let mut deepest = None;
        while i < bytes.len() {
            let c = bytes[i];
            if quoted {
                if escaped {
                    escaped = false;
                } else if c == b'\\' {
                    escaped = true;
                } else if c == b'"' {
                    quoted = false;
                }
                i += 1;
                continue;
            }
            if c == b'"' {
                quoted = true;
                i += 1;
                continue;
            }
            if c >= 128 {
                i += input[i..].chars().next().unwrap().len_utf8();
                continue;
            }
            let field = ["entry:", "effects:", "adjustments:", "power_toughness:", "actor:", "minimum:"]
                .into_iter()
                .find(|f| input[i..].starts_with(f));
            if let Some(field) = field {
                let value_start = i + field.len();
                let context = stack.iter().rev().find_map(|&open| owner(&input, open));
                let upgrade = match (field, context) {
                    ("minimum:",_) if stack.last().is_some_and(|&open| {
                        input[open+1..].trim_start().starts_with("minimum:") && {
                            let mut de=ron::de::Deserializer::from_str(&input[open..]).unwrap();
                            mode_policies::LegacyModeSelection::deserialize(&mut de).is_ok()
                        }
                    }) => Some(Upgrade::Modes),
                    ("actor:",Some("Keyword"|"KeywordEach")) if stack.last().is_some_and(|&open| {
                        if !has_legacy_keyword_field(&input[open..],context==Some("KeywordEach")) {return false;}
                        let mut de=ron::de::Deserializer::from_str(&input[open..]).unwrap();
                        if context==Some("Keyword") {OldKeyword::deserialize(&mut de).is_ok()} else {OldEach::deserialize(&mut de).is_ok()}
                    })=>Some(Upgrade::Keyword(context==Some("KeywordEach"))),
                    ("entry:", Some(name))
                        if OPERATIONS.contains(&name)
                            && input[value_start..].trim_start().starts_with('[') =>
                    {
                        Some(Upgrade::Entry)
                    }
                    ("effects:", Some("CopyOnStack")) => Some(Upgrade::Copy(false)),
                    ("effects:", Some("CopyOf")) => Some(Upgrade::Copy(true)),
                    ("adjustments:", Some("Copy" | "CopyEach" | "CopyOne" | "CopyNamed")) => {
                        Some(Upgrade::Copy(false))
                    }
                    ("power_toughness:", _)
                        if stack.last().is_some_and(|&open| {
                            input[open + 1..]
                                .trim_start()
                                .starts_with("power_toughness:")
                                && input[value_start..].trim_start().starts_with("(kind:")
                        }) =>
                    {
                        Some(Upgrade::Shape)
                    }
                    _ => None,
                };
                if let Some(upgrade) = upgrade {
                    let (start, value_start) = if matches!(upgrade, Upgrade::Shape|Upgrade::Keyword(_)|Upgrade::Modes) {
                        let open = *stack.last().unwrap();
                        (open, open)
                    } else {
                        (i, value_start)
                    };
                    if deepest
                        .as_ref()
                        .is_none_or(|(depth, _, _, _)| stack.len() >= *depth)
                    {
                        deepest = Some((stack.len(), start, value_start, upgrade));
                    }
                }
            }
            match c {
                b'(' | b'[' | b'{' => stack.push(i),
                b')' | b']' | b'}' => {
                    stack.pop().unwrap();
                }
                _ => (),
            }
            i += 1;
        }
        let Some((_, start, value_start, upgrade)) = deepest else {
            return ron::from_str(&input).unwrap();
        };
        let tail = &input[value_start..];
        let mut de = ron::de::Deserializer::from_str(tail).unwrap();
        let (name, replacement) = match upgrade {
            Upgrade::Modes => ("",ron::to_string(&mode_policies::LegacyModeSelection::deserialize(&mut de).unwrap().current().unwrap()).unwrap()),
            Upgrade::Keyword(each) => ("",if each {ron::to_string(&OldEach::deserialize(&mut de).unwrap().current()).unwrap()}else{ron::to_string(&OldKeyword::deserialize(&mut de).unwrap().current()).unwrap()}),
            Upgrade::Shape => (
                "",
                ron::to_string(&OldShape::deserialize(&mut de).unwrap().current()).unwrap(),
            ),
            Upgrade::Entry => {
                let flags = Vec::<Operand>::deserialize(&mut de).unwrap();
                (
                    "entry:",
                    ron::to_string(&movement::entry_from_source(&flags)).unwrap(),
                )
            }
            Upgrade::Copy(single) => {
                let operands = if single {
                    vec![Operand::deserialize(&mut de).unwrap()]
                } else {
                    Vec::<Operand>::deserialize(&mut de).unwrap()
                };
                (
                    "exceptions:",
                    ron::to_string(&copying::from_source(&operands).unwrap()).unwrap(),
                )
            }
        };
        let end = value_start + tail.len() - de.remainder().len();
        input = format!("{}{name}{replacement}{}", &input[..start], &input[end..]);
    }
    panic!("historical payload upgrade limit exceeded");
}

/// Spelling/role adaptation for immutable pre-authoring producer snapshots.
/// Unlike the live migration, these captures have several older schema shapes;
/// change only the enumerated aliases and retain every other subtree verbatim.
fn api_names(value: &mut Value) {
    let Some(kind)=value.get("kind").and_then(Value::as_str).map(str::to_string) else {return;};
    let rename=match kind.as_str() {
        "Tap"=>Some("SetTapped"), "KeywordHost"=>Some("AbilityHolder"), "Granter"=>Some("GrantingObject"),
        "Power" if value["args"].as_array().is_some_and(|a|a.len()==2)=>Some("Exponentiate"),
        "Named" if value["args"].is_number() || (value["args"].get("result").is_some()&&value["args"].get("by").is_some())=>Some("Recorded"),
        _=>None,
    };
    if let Some(new)=rename {value["kind"]=json!(new);}
    let roles:&[(&str,&str)]=match kind.as_str() {
        "Search"=>&[("player","searcher"),("library","library_owner")],
        "ReorderLibrary"=>&[("player","chooser"),("library","library_owner")],
        "ChooseValue"|"ChooseValueWith"=>&[("actor","choosers")],_=>&[],
    };
    for (old,new) in roles {
        if let Some(args)=value["args"].as_object_mut() {if let Some(v)=args.remove(*old) {args.insert((*new).into(),v);}}
    }
    if kind=="DuplicateCounters" && value["args"]["from"].get("kind").is_some() {
        let from=value["args"]["from"].take();
        value["args"]["from"]=if from["kind"]=="CountersTheEventPut" {json!("EventPut")}else{json!({"On":from})};
    }
}

/// Lexical spelling adaptation confined to these immutable, pre-authoring RON
/// fixtures. Quoted text and unrelated enums (e.g. CounterKind::Named) retain
/// their original bytes; operation role fields are changed only at args depth 1.
fn ron_api_names(input: &str) -> String {
    let mut out=input.to_string();
    if let Some(start)=out.find("layout:") {
        let start=start+"layout:".len();let tail=&out[start..];
        let mut de=ron::de::Deserializer::from_str(tail).unwrap();
        let name=String::deserialize(&mut de).unwrap();
        let end=start+tail.len()-de.remainder().len();
        out.replace_range(start..end,&ron::to_string(&Layout::from_name(&name).unwrap()).unwrap());
    }
    let mut mask=out.as_bytes().to_vec();let(mut quoted,mut escaped)=(false,false);
    for b in &mut mask {
        let c=*b;
        if quoted {if escaped {escaped=false;}else if c==b'\\' {escaped=true;}else if c==b'"' {quoted=false;}*b=b' ';}
        else if c==b'"' {quoted=true;*b=b' ';}
    }
    let mask=String::from_utf8(mask).unwrap();let mut edits=vec![];
    for (old,new) in [("Tap","SetTapped"),("Granter","GrantingObject"),("KeywordHost","AbilityHolder"),("Power","Exponentiate"),("Named","Recorded")] {
        let pattern=format!("kind:{old},args:");
        for (at,_) in mask.match_indices(&pattern) {
            let payload=&mask[at+pattern.len()..];
            if old=="Named" && !(payload.starts_with("(result:")||payload.starts_with("((")||payload.as_bytes().get(1).is_some_and(u8::is_ascii_digit)) {continue;}
            edits.push((at+5,at+5+old.len(),new.to_string()));
        }
        if matches!(old,"Granter"|"KeywordHost") {for (at,_) in mask.match_indices(&format!("kind:{old})")) {edits.push((at+5,at+5+old.len(),new.to_string()));}}
    }
    for (op,roles) in [("Search",vec![("player","searcher"),("library","library_owner")]),("ReorderLibrary",vec![("player","chooser"),("library","library_owner")]),("ChooseValue",vec![("actor","choosers")])] {
        let pattern=format!("kind:{op},args:(");
        for (at,_) in mask.match_indices(&pattern) {
            let mut depth=1;let mut i=at+pattern.len();let bytes=mask.as_bytes();
            while i<bytes.len() && depth>0 {
                if depth==1 {for (old,new) in &roles {if mask[i..].starts_with(&format!("{old}:")) && (bytes[i-1]==b'('||bytes[i-1]==b',') {edits.push((i,i+old.len(),(*new).into()));}}}
                match bytes[i] {b'('|b'['|b'{'=>depth+=1,b')'|b']'|b'}'=>depth-=1,_=>{}}i+=1;
            }
        }
    }
    edits.sort_by_key(|(a,_,_)|*a);edits.dedup();
    for (a,b,v) in edits.into_iter().rev(){out.replace_range(a..b,&v);}
    out
}
