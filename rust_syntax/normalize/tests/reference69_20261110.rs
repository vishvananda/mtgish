#[path = "support/epoch189.rs"]
mod epoch189;
use mtgish_ast::{canonical, corpus, ir::*, migrate, mtg_types as source, unified, validate};
use serde_json::{json, Value};

fn cards() -> Vec<Card> {
    include_str!("fixtures/reference69_20261110.ronl").lines().flat_map(|line| {
        let source: source::OracleCard = ron::from_str(line).unwrap();
        unified::lower(&source).card.faces
    }).collect()
}
fn card(name: &str) -> Card { cards().into_iter().find(|c| c.name == name).unwrap() }
fn canonical_abilities(mut value: Value) -> Value {
    let abilities: Vec<Ability> = serde_json::from_value(value.clone()).unwrap();
    validate::abilities(&abilities).unwrap();
    canonical::abilities(&mut value).unwrap();
    value
}
#[test]
fn all_58_independent_whole_card_proposals_match() {
    corpus::on_large_stack(|| {
        let expected: Value = serde_json::from_str(include_str!("fixtures/reference58_expected_20261110.json")).unwrap();
        let mut count = 0;
        let mut differences = vec![];
        for card in cards() {
            let Some(want) = expected.get(&card.name) else { continue };
            validate::card(&card).unwrap_or_else(|e| panic!("{}: {e}",card.name));
            let actual = canonical_abilities(serde_json::to_value(&card.abilities).unwrap());
            let want = canonical_abilities(want.clone());
            count += 1;
            if actual != want {
                if let Ok(dir)=std::env::var("REFERENCE_FIX_EVIDENCE") {
                    std::fs::write(format!("{dir}/{}.actual.json",card.name),serde_json::to_vec_pretty(&actual).unwrap()).unwrap();
                    std::fs::write(format!("{dir}/{}.expected.json",card.name),serde_json::to_vec_pretty(&want).unwrap()).unwrap();
                }
                differences.push(card.name);
            }
        }
        assert_eq!(count,58);
        assert!(differences.is_empty(),"whole-card differences: {differences:?}");
    });
}
fn values<'a>(value: &'a Value, kind: &str, out: &mut Vec<&'a Value>) {
    match value {
        Value::Object(fields) => {
            if value["kind"] == kind { out.push(value); }
            for value in fields.values() { values(value,kind,out); }
        },
        Value::Array(items) => for value in items { values(value,kind,out); },
        _ => {},
    }
}
fn nodes<'a>(value: &'a Value, kind: &str) -> Vec<&'a Value> { let mut out=vec![]; values(value,kind,&mut out);out }
#[test]
fn shield_destroys_the_first_participant_in_both_combat_directions() {
    corpus::on_large_stack(|| {
        let shield=card("Mirror Shield"); validate::card(&shield).unwrap();
        let value=serde_json::to_value(shield).unwrap();
        let destroys=nodes(&value,"Destroy"); assert_eq!(destroys.len(),1);
        assert_eq!(destroys[0]["args"]["objects"],json!({"kind":"One","args":{"kind":"EventSubject"}}));
        let text=serde_json::to_string(&value).unwrap();
        assert!(text.contains("Blocks") && text.contains("BecomesBlockedBy"));
        assert!(!text.contains("BlockPair"));
    });
}
#[test]
fn bombshell_captures_the_triggering_controller_and_does_not_rebind_on_resolution() {
    corpus::on_large_stack(|| {
        let mut bomb=card("Bronze Bombshell"); validate::card(&bomb).unwrap();
        let Ability::Triggered {event: EventSpec::StateWithPlayer { condition:_, player },body,..}=&mut bomb.abilities[0] else {panic!("{bomb:?}")};
        assert_eq!(serde_json::to_value(&*player).unwrap(),json!({"kind":"ControllerOf","args":{"kind":"Source"}}));
        let body=serde_json::to_value(body).unwrap();
        assert!(nodes(&body,"EventSubject").len()>=2);
        assert!(nodes(&body,"ControllerOf").is_empty(),"live resolution-time controller query");
        // The capture expression is evaluated BEFORE the captured binding exists.
        *player=PlayerRef::EventSubject;
        assert!(validate::card(&bomb).is_err());
    });
}
#[test]
fn aura_host_legality_has_explicit_inverse_direction_and_exact_dead_aura() {
    corpus::on_large_stack(|| {
        for name in ["Infectious Rage","Takklemaggot"] {
            let card=card(name);validate::card(&card).unwrap_or_else(|e|panic!("{name}: {e}"));
            let value=serde_json::to_value(card).unwrap();
            let hosts=nodes(&value,"CouldBeEnchantedBy");assert_eq!(hosts.len(),1);
            assert_eq!(hosts[0]["args"],json!({"kind":"AuraAfterEnchantedLeft","args":{"aura":{"kind":"Source"},"host":{"kind":"EventSubject"}}}));
            assert!(nodes(&value,"CouldEnchant").is_empty());
        }
    });
}
#[test]
fn ancestry_excludes_the_aura_and_infinity_controls_ability_presence() {
    corpus::on_large_stack(|| {
        let mask=serde_json::to_value(card("Ancestral Mask")).unwrap();
        let excludes=nodes(&mask,"Not");assert_eq!(excludes.len(),2);
        for exclusion in excludes {assert_eq!(exclusion["args"],json!({"kind":"SameObject","args":{"kind":"Source"}}));}
        let soul=card("The Soul Stone");validate::card(&soul).unwrap();
        let Ability::Conditional {condition, then, otherwise}=&soul.abilities[3] else {panic!("{soul:?}")};
        assert!(otherwise.is_empty());assert_eq!(then.len(),1);
        assert!(format!("{condition:?}").contains("Harnessed"));
        assert!(format!("{condition:?}").contains("Battlefield"));
        let Ability::Triggered { event,intervening_if,.. }=&then[0] else {panic!()};
        assert!(matches!(event,EventSpec::Turn {..}));assert!(intervening_if.is_none());
    });
}
#[test]
fn dual_exile_origins_have_one_ability_and_one_simultaneous_batch() {
    corpus::on_large_stack(|| {
        for name in ["Ranar the Ever-Watchful","Hero of Bretagard"] {
            let card=card(name);validate::card(&card).unwrap();
            let events=card.abilities.iter().filter_map(|a|if let Ability::Triggered {event,..}=a {Some(event)}else{None}).collect::<Vec<_>>();
            assert_eq!(events.len(),1,"{name}");
            let EventSpec::Batch {event,per:BatchPer::Whole,count:None,including:None} = events[0] else {panic!("{events:?}")};
            let EventSpec::AnyOf(branches)=event.as_ref() else {panic!()};assert_eq!(branches.len(),2);
            assert!(matches!(branches[0],EventSpec::ZoneChange {from:Some(EventZone::Hand),to:EventZone::Exile,..}));
            assert!(matches!(branches[1],EventSpec::Caused {..}));
        }
    });
}
#[test]
fn saved187_meaning_and_origins_stay_exact_instead_of_silently_repairing_old_cards() {
    corpus::on_large_stack(|| {
        let mut count=0;
        for line in include_str!("fixtures/reference69_before_v187.jsonl").lines() {
            let mut old:Value=serde_json::from_str(line).unwrap();
        epoch189::tree(&mut old);
            let mapped=migrate::mapped_read_json(line).unwrap();assert_eq!(mapped.source_version,187);epoch189::assert_origins(&serde_json::from_str(line).unwrap(), &mapped);
            old["schema_version"]=json!(SCHEMA_VERSION);
            for face in old["faces"].as_array_mut().unwrap() {face["schema_version"]=json!(SCHEMA_VERSION);}
            assert_eq!(serde_json::to_value(mapped.card).unwrap(),old);
            assert!(mapped.map.introduced.is_empty());
            assert!(mapped.map.sites.values().all(|new|new.len()==1));count+=1;
        }
        assert_eq!(count,69);
    });
}

#[test]
fn optional_modes_retained_subtypes_actual_counter_outcomes_and_forward_enchant_controls_stay_exact() {
    corpus::on_large_stack(|| {
        let expected:Value=serde_json::from_str(include_str!("fixtures/reference69_controls_20261110.json")).unwrap();
        let mut count=0;
        for line in include_str!("fixtures/reference69_controls_20261110.ronl").lines() {
            let source:source::OracleCard=ron::from_str(line).unwrap();
            for card in unified::lower(&source).card.faces {
                validate::card(&card).unwrap();
                assert_eq!(canonical_abilities(serde_json::to_value(&card.abilities).unwrap()),canonical_abilities(expected[&card.name].clone()),"{}",card.name);count+=1;
            }
        }
        assert_eq!(count,19);
    });
}

#[test]
fn linked_pool_excludes_the_new_exile_incarnation_from_this_payment() {
    corpus::on_large_stack(|| {
        let pool=card("Knowledge Pool");validate::card(&pool).unwrap();
        let value=serde_json::to_value(pool).unwrap();
        let play=nodes(&value,"PlayCards");assert_eq!(play.len(),1);
        let exclusions=nodes(play[0],"Not");assert_eq!(exclusions.len(),1);
        let transitions=nodes(exclusions[0],"CostObjects");assert_eq!(transitions.len(),1);
        assert_eq!(transitions[0]["args"],json!({"payment":{"Resolution":0},"component":[0],"view":"Performed","phase":"After"}));
        assert_eq!(nodes(play[0],"ExiledWith").len(),1);
        assert!(nodes(exclusions[0],"EventSubject").is_empty(),"old stack identity is not its new exile identity");
    });
}
#[test]
fn overseer_captures_its_original_population_and_zabaz_retains_an_explicit_permanent_domain() {
    corpus::on_large_stack(|| {
        for name in ["Overseer of Vault 76","Zabaz, the Glimmerwasp"] { validate::card(&card(name)).unwrap(); }
        let overseer=serde_json::to_value(card("Overseer of Vault 76")).unwrap();
        let notes=nodes(&overseer,"NoteObjects");assert_eq!(notes.len(),1);
        let selected=nodes(&overseer,"Selected");assert_eq!(selected.len(),2);assert_eq!(selected[0],selected[1]);
        let zabaz=serde_json::to_value(card("Zabaz, the Glimmerwasp")).unwrap();
        let objects=&zabaz["abilities"][1]["args"]["event"]["Caused"]["event"]["Counters"]["objects"];
        assert_eq!(objects["kind"],"InSet");assert_eq!(objects["args"]["kind"],"Zone");
        assert_eq!(objects["args"]["args"]["scope"],json!({"Shared":"Battlefield"}));
        assert_eq!(objects["args"]["args"]["kind"],"Permanent");
    });
}
#[cfg(feature="cantrip")]
#[test]
fn captured_state_player_is_an_honestly_gated_planned_interface() {
    use mtgish_ast::cantrip;
    corpus::on_large_stack(|| {
        for (name,feature,call) in [
            ("Bronze Bombshell","state-trigger-player-capture","state_triggered_with_player("),
        ] {
            let mut card=card(name);
            // An arbitrary captured player still needs the general future API.
            // The source controller uses the engine's existing frozen controller.
            let current = cantrip::module(&card).unwrap();
            assert!(current.contains("state_triggered("));
            assert!(!current.contains("state_triggered_with_player("));
            let Ability::Triggered { event: EventSpec::StateWithPlayer { player, .. }, .. } = &mut card.abilities[0] else { panic!() };
            *player = PlayerRef::OwnerOf(Box::new(ObjectRef::Source));
            let old=cantrip::module(&card).unwrap_err();
            let code=cantrip::planned::with_additional_features(&[feature],||cantrip::module(&card))
                .unwrap_or_else(|e|panic!("{name}: {e:?}; old {old:?}"));
            syn::parse_file(&code).unwrap();assert!(code.contains(call),"{name}: {code}");
            assert!(!code.contains("planned_value!"),"{code}");
            if let Ok(dir)=std::env::var("REFERENCE_FIX_EVIDENCE") {
                std::fs::write(format!("{dir}/{name}.enabled.rs"),code).unwrap();
            }
        }
    });
}
