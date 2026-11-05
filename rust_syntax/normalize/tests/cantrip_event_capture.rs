#![cfg(feature = "cantrip")]
use mtgish_ast::{cantrip, lower, mtg_types::OracleCard};

#[test]
fn one_shot_trigger_watches_source_and_carries_an_independent_object() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/mtgish.lines.ron");
    let Ok(corpus) = std::fs::read_to_string(path) else { return };
    let line = corpus.lines().find(|line| line.starts_with("Card(Name: \"Necromancy\"")).unwrap();
    let source: OracleCard = ron::from_str(line).unwrap();
    let card = lower::card(&source).unwrap();
    let raw = cantrip::planned::with_additional_features(&[], || cantrip::module(&card)).unwrap();
    assert!(raw.contains("g.delayed_trigger_about(DelayedWhen::WhenEvent"), "{raw}");
    assert!(raw.contains("q.event_objects(item)"), "{raw}");
    assert!(!raw.contains("DelayedWhen::UntilEvent"), "{raw}");
    let printed = cantrip::pr_form::render(&raw, "Necromancy", "When this enchantment leaves the battlefield, that creature's controller sacrifices it.", cantrip::pr_form::Characteristics::Inline).unwrap();
    syn::parse_file(&printed).unwrap();
    if let Ok(path) = std::env::var("CANTRIP_NECROMANCY_PRINTED") { std::fs::write(path, printed).unwrap(); }
}

#[test]
fn creator_subject_and_independent_creature_do_not_share_one_capture_group() {
    use mtgish_ast::{ir::*, visit::{VisitorMut, WalkMut}};
    let corpus=std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/mtgish.lines.ron")).unwrap();
    let line=corpus.lines().find(|l|l.starts_with("Card(Name: \"Necromancy\"")).unwrap();
    let source: OracleCard=ron::from_str(line).unwrap(); let mut card=lower::card(&source).unwrap();
    struct Add;
    impl VisitorMut for Add { fn operation(&mut self, op:&mut Operation)->bool {
        if let Operation::CreateDelayedTrigger{body,..}=op { body.instructions.push(Instruction::Effect {result:ResultId(999),operation:Operation::Draw {players:PlayerSet::One(PlayerRef::OwnerOf(Box::new(ObjectRef::EventSubject))),amount:Expr::Constant(1)}}); return false } true
    }}
    for ability in &mut card.abilities { if matches!(ability, Ability::Triggered { .. }) { ability.walk_mut(&mut Add); } }
    let gap=cantrip::planned::with_additional_features(&[],||cantrip::module(&card)).unwrap_err();
    assert!(gap.feature.contains("carrying separate groups of objects"), "{gap:?}");
}

#[test]
fn dynamic_future_event_keeps_the_distinct_creator_event_subject() {
    use mtgish_ast::{ir::*, visit::{Visitor, Walk}};
    let corpus=std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/mtgish.lines.ron")).unwrap();
    let line=corpus.lines().find(|l|l.starts_with("Card(Name: \"Necromancy\"")).unwrap();
    let source: OracleCard=ron::from_str(line).unwrap(); let necro=lower::card(&source).unwrap();
    struct Get(Option<Operation>);
    impl Visitor for Get { fn operation(&mut self, op:&Operation)->bool {if matches!(op,Operation::CreateDelayedTrigger{..}) {self.0=Some(op.clone());return false} true} }
    let mut get=Get(None); for a in &necro.abilities {if matches!(a,Ability::Triggered{..}) {a.walk(&mut get);}}
    let mut op=get.0.unwrap(); let Operation::CreateDelayedTrigger{body,event,..}=&mut op else {panic!()};
    let EventSpec::Object { objects, .. } = event else {panic!()}; *objects = ObjectFilter::Any;
    body.instructions=vec![Instruction::Effect {result:ResultId(999),operation:Operation::Draw {players:PlayerSet::One(PlayerRef::OwnerOf(Box::new(ObjectRef::EventSubject))),amount:Expr::Constant(1)}}];
    let line=include_str!("fixtures/tapped_aura_references.ronl").lines().find(|l|l.starts_with("Card(Name: \"Kudzu\"")).unwrap();
    let source: OracleCard=ron::from_str(line).unwrap();let mut c=lower::card(&source).unwrap();
    let Ability::Triggered{body,..}=c.abilities.iter_mut().find(|a|matches!(a,Ability::Triggered{..})).unwrap() else {panic!()};
    body.instructions=vec![Instruction::Effect{result:ResultId(998),operation:op}];
    let raw=cantrip::planned::with_additional_features(&[],||cantrip::module(&c)).unwrap();
    assert!(raw.contains("item.subject.object().into_iter().collect::<Vec<ObjectId>>()"), "{raw}");
    assert!(raw.contains("g.delayed_trigger_about(DelayedWhen::WhenEvent"), "{raw}");
    assert!(raw.contains("about(q, item, 0).first().copied().map(|o: ObjectId| q.state().obj(o).owner)"), "{raw}");
    let matcher=raw.split("fn delayed_1_event").nth(1).unwrap().split("fn delayed_1(").next().unwrap();
    assert!(!matcher.contains("Some(asker.source) == Some(id)"), "{matcher}");
}

#[test]
fn mixed_future_stack_subject_is_an_explicit_gap() {
    use mtgish_ast::{ir::*, visit::{VisitorMut, WalkMut}};
    let corpus=std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/mtgish.lines.ron")).unwrap();
    let line=corpus.lines().find(|l|l.starts_with("Card(Name: \"Necromancy\"")).unwrap();
    let source: OracleCard=ron::from_str(line).unwrap(); let mut card=lower::card(&source).unwrap();
    struct Change;
    impl VisitorMut for Change { fn operation(&mut self, op:&mut Operation)->bool {
        if let Operation::CreateDelayedTrigger{body,event,..}=op {
            *event=EventSpec::SpellOrActivation {players:PlayerSet::All,spells:ObjectFilter::Any,abilities:ObjectFilter::Any};
            body.instructions=vec![Instruction::Effect{result:ResultId(999),operation:Operation::Draw{players:PlayerSet::One(PlayerRef::ControllerOf(Box::new(ObjectRef::EventStackObject))),amount:Expr::Constant(1)}}];
            return false
        } true
    }}
    for ability in &mut card.abilities { if matches!(ability, Ability::Triggered { .. }) { ability.walk_mut(&mut Change); } }
    let gap=cantrip::planned::with_additional_features(&[],||cantrip::module(&card)).unwrap_err();
    assert!(gap.feature.contains("mixed future stack subject"), "{gap:?}");
}
