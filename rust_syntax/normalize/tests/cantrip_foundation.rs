#![cfg(feature = "cantrip")]
//! The native early-set foundation is an explicit candidate profile until merged upstream.
use mtgish_ast::cantrip;
use std::{collections::BTreeMap, path::PathBuf, sync::OnceLock};
const FOUNDATION: &[&str] = &["world-rule", "landwalk-blocking-exceptions", "independent-pt-settings", "no-combat-damage-assignment"];
type Rows = BTreeMap<String,serde_json::Value>;
fn read(path: &std::path::Path) -> Rows {
    std::fs::read_to_string(path).unwrap().lines().map(|line| { let row:serde_json::Value=serde_json::from_str(line).unwrap();(row["name"].as_str().unwrap().to_string(),row) }).collect()
}
fn profiles() -> &'static (Rows,Rows) {
    static ROWS:OnceLock<(Rows,Rows)>=OnceLock::new();
    ROWS.get_or_init(|| {
        let fixtures=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let source=fixtures.join("early_set_foundation.ronl");let oracle=fixtures.join("early_set_foundation.oracle.json");
        let baseline=PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-set-foundation-baseline.jsonl");
        let candidate=PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-set-foundation-candidate.jsonl");
        cantrip::planned::tracked(&source,&oracle,&baseline).unwrap();
        cantrip::planned::with_additional_features(FOUNDATION, || cantrip::planned::tracked(&source,&oracle,&candidate).unwrap());
        (read(&baseline),read(&candidate))
    })
}
fn activator_profiles() -> &'static (Rows,Rows) {
    static ROWS:OnceLock<(Rows,Rows)>=OnceLock::new();
    ROWS.get_or_init(|| {
        let fixtures=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let source=fixtures.join("early_set_activators.ronl");let oracle=fixtures.join("early_set_activators.oracle.json");
        let baseline=PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-set-activators-baseline.jsonl");
        let candidate=PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-set-activators-candidate.jsonl");
        cantrip::planned::tracked(&source,&oracle,&baseline).unwrap();
        let mut features=FOUNDATION.to_vec();features.push("other-activators");
        cantrip::planned::with_additional_features(&features, || cantrip::planned::tracked(&source,&oracle,&candidate).unwrap());
        (read(&baseline),read(&candidate))
    })
}
#[test]
fn explicit_activators_use_the_real_interface_without_dropping_unrelated_requirements() {
    let (baseline,candidate)=activator_profiles();assert_eq!(baseline.len(),8);
    assert!(baseline.values().all(|r|gaps(r).contains(&"engine: no activation by other players")));
    let blocked:Vec<_>=candidate.iter().filter(|(_,r)|!gaps(r).is_empty()).map(|(n,_)|n.as_str()).collect();
    assert_eq!(blocked,["Merseine","Personal Incarnation"]);
    for (name,row) in candidate {
        assert!(!gaps(row).contains(&"engine: no activation by other players"),"{name}");
        syn::parse_file(code(row)).unwrap_or_else(|e|panic!("{name}: {e}"));
    }
    assert!(code(&candidate["Armageddon Clock"]).contains(".any_player_may_activate()"));
    assert!(code(&candidate["Clergy of the Holy Nimbus"]).contains(".activated_by("));
    assert!(code(&candidate["Personal Incarnation"]).contains(".activated_by("));
}
fn step_profiles() -> &'static (Rows,Rows) {
    static ROWS:OnceLock<(Rows,Rows)>=OnceLock::new();
    ROWS.get_or_init(|| {
        let fixtures=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let source=fixtures.join("early_set_steps.ronl");let oracle=fixtures.join("early_set_steps.oracle.json");
        let baseline=PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-set-steps-baseline.jsonl");
        let candidate=PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-set-steps-candidate.jsonl");
        cantrip::planned::tracked(&source,&oracle,&baseline).unwrap();
        let mut features=FOUNDATION.to_vec();features.extend(["other-activators","step-deadlines","counted-untap-steps"]);
        cantrip::planned::with_additional_features(&features, || cantrip::planned::tracked(&source,&oracle,&candidate).unwrap());
        (read(&baseline),read(&candidate))
    })
}
#[test]
fn exact_step_deadlines_clear_their_requirements() {
    let (baseline,candidate)=step_profiles();assert_eq!(baseline.len(),6);
    assert!(baseline.values().all(|r|!gaps(r).is_empty()));
    for (name,row) in candidate {println!("{name}: {:?}",gaps(row));syn::parse_file(code(row)).unwrap();}
    for name in ["Elkin Bottle","Erhnam Djinn","Gabriel Angelfire","Halfdane","Telekinesis","Xenic Poltergeist"] {assert!(gaps(&candidate[name]).is_empty(),"{name}: {:?}",gaps(&candidate[name]));}
    assert!(code(&candidate["Halfdane"]).contains("Duration::UntilEndOfNextStepOf"));
    assert!(code(&candidate["Elkin Bottle"]).contains("Expiry::UntilYourNextStep"));
}
fn mana_loss_profiles() -> &'static (Rows,Rows) {
    static ROWS:OnceLock<(Rows,Rows)>=OnceLock::new();
    ROWS.get_or_init(|| {
        let fixtures=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let source=fixtures.join("early_set_mana_loss.ronl");let oracle=fixtures.join("early_set_mana_loss.oracle.json");
        let baseline=PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-set-mana-loss-baseline.jsonl");
        let candidate=PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-set-mana-loss-candidate.jsonl");
        cantrip::planned::tracked(&source,&oracle,&baseline).unwrap();
        let mut features=FOUNDATION.to_vec();features.extend(["other-activators","step-deadlines","counted-untap-steps","explicit-mana-loss"]);
        cantrip::planned::with_additional_features(&features, || cantrip::planned::tracked(&source,&oracle,&candidate).unwrap());
        (read(&baseline),read(&candidate))
    })
}
#[test]
fn explicit_loss_lands_without_landing_mana_doubling_or_forced_land_activations() {
    let (baseline,candidate)=mana_loss_profiles();assert_eq!(baseline.len(),4);
    assert!(baseline.values().all(|r|gaps(r).contains(&"engine: no effect empties a mana pool")));
    assert!(gaps(&candidate["Mana Short"]).is_empty());assert!(gaps(&candidate["Power Sink"]).is_empty());
    assert_eq!(gaps(&candidate["Drain Power"]),["keyword action ActivateManaAbility","mana production LostThisWay"]);
    assert!(gaps(&candidate["Doubling Cube"]).contains(&"engine: no effect empties a mana pool"),"a shared old requirement label must not accidentally land mana doubling");
    for (name,row) in candidate {syn::parse_file(code(row)).unwrap_or_else(|e|panic!("{name}: {e}"));}
}
fn random_cost_profiles() -> &'static (Rows,Rows) {
    static ROWS:OnceLock<(Rows,Rows)>=OnceLock::new();
    ROWS.get_or_init(|| {
        let fixtures=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let source=fixtures.join("early_set_random_cost.ronl");let oracle=fixtures.join("early_set_random_cost.oracle.json");
        let baseline=PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-set-random-cost-baseline.jsonl");
        let candidate=PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-set-random-cost-candidate.jsonl");
        cantrip::planned::tracked(&source,&oracle,&baseline).unwrap();
        let mut features=FOUNDATION.to_vec();features.extend(["other-activators","step-deadlines","counted-untap-steps","explicit-mana-loss","random-discard-costs"]);
        cantrip::planned::with_additional_features(&features, || cantrip::planned::tracked(&source,&oracle,&candidate).unwrap());
        (read(&baseline),read(&candidate))
    })
}
#[test]
fn constant_random_costs_land_but_variable_random_discard_does_not() {
    let (baseline,candidate)=random_cost_profiles();assert_eq!(baseline.len(),3);
    assert!(baseline.values().all(|r|gaps(r).contains(&"engine: no random discard cost")));
    for (name,row) in candidate {assert!(gaps(row).is_empty(),"{name}: {:?}",gaps(row));syn::parse_file(code(row)).unwrap();}
    use mtgish_ast::{corpus,ir::*,mtg_types::OracleCard,unified};
    let source=include_str!("fixtures/early_set_random_cost.ronl").lines().map(|l|ron::from_str::<OracleCard>(l).unwrap())
        .find(|s|corpus::Identity::of(s).unwrap().names.iter().any(|n|n=="Stormbind")).unwrap();
    let mut lowered=unified::lower(&source);
    let Ability::Activated {cost,..}=&mut lowered.card.faces[0].abilities[0] else {panic!()};
    let Some(CostPart::Discard {count,..})=cost.parts.iter_mut().find(|p|matches!(p,CostPart::Discard {..})) else {panic!()};
    *count=Expr::AnnouncedX;
    cantrip::planned::with_additional_features(&["random-discard-costs"],|| {
        let generated=cantrip::whole(&source,&lowered.card).unwrap();
        assert!(generated.faces.iter().any(|f|f.gaps.iter().any(|g|g.feature=="engine: no random discard cost")));
    });
}
fn keyword_family_profiles() -> &'static (Rows,Rows) {
    static ROWS:OnceLock<(Rows,Rows)>=OnceLock::new();
    ROWS.get_or_init(|| {
        let fixtures=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let source=fixtures.join("early_set_keyword_family.ronl");let oracle=fixtures.join("early_set_keyword_family.oracle.json");
        let baseline=PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-set-keyword-family-baseline.jsonl");
        let candidate=PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-set-keyword-family-candidate.jsonl");
        cantrip::planned::tracked(&source,&oracle,&baseline).unwrap();
        let mut features=FOUNDATION.to_vec();features.extend(["other-activators","step-deadlines","counted-untap-steps","explicit-mana-loss","random-discard-costs","keyword-family-metadata"]);
        cantrip::planned::with_additional_features(&features, || cantrip::planned::tracked(&source,&oracle,&candidate).unwrap());
        (read(&baseline),read(&candidate))
    })
}
#[test]
fn typed_keyword_metadata_clears_rampage_and_cumulative_upkeep_tests() {
    let (baseline,candidate)=keyword_family_profiles();assert_eq!(baseline.len(),2);
    assert!(baseline.values().all(|r|gaps(r).contains(&"object filter HasKeywordFamily")));
    for (name,row) in candidate {assert!(gaps(row).is_empty(),"{name}: {:?}",gaps(row));syn::parse_file(code(row)).unwrap();}
    assert!(code(&candidate["Rapid Fire"]).contains("Some(cantrip_api::KeywordFamily::Rampage)"));
    assert!(code(&candidate["Balduvian Shaman"]).contains("Some(cantrip_api::KeywordFamily::CumulativeUpkeep)"));
}
fn attached_cost_profiles() -> &'static (Rows,Rows) {
    static ROWS:OnceLock<(Rows,Rows)>=OnceLock::new();
    ROWS.get_or_init(|| {
        let fixtures=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let source=fixtures.join("early_set_attached_cost.ronl");let oracle=fixtures.join("early_set_attached_cost.oracle.json");
        let baseline=PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-set-attached-cost-baseline.jsonl");
        let candidate=PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-set-attached-cost-candidate.jsonl");
        cantrip::planned::tracked(&source,&oracle,&baseline).unwrap();
        cantrip::planned::with_additional_features(FOUNDATION, || cantrip::planned::tracked(&source,&oracle,&candidate).unwrap());
        (read(&baseline),read(&candidate))
    })
}
#[test]
fn controlled_enchant_hosts_use_existing_named_tap_costs() {
    let (baseline,candidate)=attached_cost_profiles();assert_eq!(candidate.len(),2);
    for (name,row) in candidate {
        assert!(gaps(row).is_empty(),"{name}: {:?}",gaps(row));
        assert!(gaps(&baseline[name]).is_empty(),"this is an existing API, not a new capability");
        assert!(code(row).contains("tap_untapped(1,"));
        assert!(code(row).contains("q.attached_to("));
        syn::parse_file(code(row)).unwrap();
    }
}
#[test]
fn attachment_control_proof_does_not_escape_its_restriction_or_payer() {
    use mtgish_ast::{corpus,ir::*,mtg_types::OracleCard,unified};
    let source=include_str!("fixtures/early_set_attached_cost.ronl").lines().map(|l|ron::from_str::<OracleCard>(l).unwrap())
        .find(|s|corpus::Identity::of(s).unwrap().names.iter().any(|n|n=="Earthlore")).unwrap();
    for variant in 0..3 {
        let mut lowered=unified::lower(&source);
        match variant {
            0 => {let Ability::Enchant {hosts:EnchantHosts::Permanents(filter)}=&mut lowered.card.faces[0].abilities[0] else {panic!()}; *filter=ObjectFilter::Any;}
            1 => {let Ability::Activated {limits,..}=&mut lowered.card.faces[0].abilities[1] else {panic!()}; limits.activators=Some(PlayerSet::All);}
            _ => {let Ability::Activated {limits,..}=&mut lowered.card.faces[0].abilities[1] else {panic!()}; limits.modifiers.push(Change::ActivationRule(ActivationRule::OnlyBy(PlayerSet::All)));}
        }
        let generated=cantrip::whole(&source,&lowered.card).unwrap();
        assert!(generated.faces.iter().any(|f|f.gaps.iter().any(|g|g.feature=="engine: a cost that taps what its payer may not control")),"variant {variant}");
    }
}
fn untap_limit_profiles() -> &'static (Rows,Rows) {
    static ROWS:OnceLock<(Rows,Rows)>=OnceLock::new();
    ROWS.get_or_init(|| {
        let fixtures=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let source=fixtures.join("early_set_untap_limits.ronl");let oracle=fixtures.join("early_set_untap_limits.oracle.json");
        let baseline=PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-set-untap-limits-baseline.jsonl");
        let candidate=PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-set-untap-limits-candidate.jsonl");
        cantrip::planned::tracked(&source,&oracle,&baseline).unwrap();
        let mut features=FOUNDATION.to_vec();features.push("untap-limits");
        cantrip::planned::with_additional_features(&features, || cantrip::planned::tracked(&source,&oracle,&candidate).unwrap());
        (read(&baseline),read(&candidate))
    })
}
#[test]
fn simultaneous_untap_limits_use_the_shared_interface() {
    let (baseline,candidate)=untap_limit_profiles();assert_eq!(candidate.len(),3);
    for (name,row) in candidate {
        assert_eq!(gaps(&baseline[name]),["a limit on what untaps in an untap step"]);
        assert!(gaps(row).is_empty(),"{name}: {:?}",gaps(row));
        assert!(code(row).contains("untap_limits: &[UntapLimit {"));syn::parse_file(code(row)).unwrap();
    }
    assert!(code(&candidate["Winter Orb"]).contains("!(q.state().obj(candidate).tapped)"));
}
fn mana_tap_profiles() -> &'static (Rows,Rows) {
    static ROWS:OnceLock<(Rows,Rows)>=OnceLock::new();
    ROWS.get_or_init(|| {
        let fixtures=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let source=fixtures.join("early_set_mana_tap.ronl");let oracle=fixtures.join("early_set_mana_tap.oracle.json");
        let baseline=PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-set-mana-tap-baseline.jsonl");
        let candidate=PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-set-mana-tap-candidate.jsonl");
        cantrip::planned::tracked(&source,&oracle,&baseline).unwrap();
        let mut features=FOUNDATION.to_vec();features.push("nonmana-mana-tap-triggers");
        cantrip::planned::with_additional_features(&features, || cantrip::planned::tracked(&source,&oracle,&candidate).unwrap());
        (read(&baseline),read(&candidate))
    })
}
#[test]
fn ordinary_mana_tap_triggers_land_without_misclassifying_mana_production() {
    let (baseline,candidate)=mana_tap_profiles();assert_eq!(candidate.len(),3);
    for name in ["Manabarbs","Burning Earth"] {
        assert!(gaps(&baseline[name]).contains(&"engine: only a mana ability triggers on tapping for mana"));
        let row=&candidate[name];assert!(gaps(row).is_empty(),"{name}: {:?}",gaps(row));
        assert!(code(row).contains("EventMask::TAPPED_FOR_MANA"));assert!(!code(row).contains(".mana_ability("));syn::parse_file(code(row)).unwrap();
    }
    assert!(gaps(&candidate["Overabundance"]).contains(&"engine: only a mana ability triggers on tapping for mana"),"mixed mana production must not be silently compiled as an ordinary stack trigger");
}
fn attack_haste_profiles() -> &'static (Rows,Rows) {
    static ROWS:OnceLock<(Rows,Rows)>=OnceLock::new();
    ROWS.get_or_init(|| {
        let fixtures=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let source=fixtures.join("early_set_attack_haste.ronl");let oracle=fixtures.join("early_set_attack_haste.oracle.json");
        let baseline=PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-set-attack-haste-baseline.jsonl");
        let candidate=PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-set-attack-haste-candidate.jsonl");
        cantrip::planned::tracked(&source,&oracle,&baseline).unwrap();
        let mut features=FOUNDATION.to_vec();features.push("attack-as-though-haste");
        cantrip::planned::with_additional_features(&features, || cantrip::planned::tracked(&source,&oracle,&candidate).unwrap());
        (read(&baseline),read(&candidate))
    })
}
#[test]
fn attack_only_permissions_do_not_grant_the_haste_keyword() {
    let (baseline,candidate)=attack_haste_profiles();assert_eq!(candidate.len(),2);
    for (name,row) in candidate {
        assert!(gaps(&baseline[name]).contains(&"static rule AttackException SummoningSickness"));
        assert!(gaps(row).is_empty(),"{name}: {:?}",gaps(row));
        assert!(code(row).contains("attacks_as_though_haste:"));assert!(!code(row).contains("Keyword::Haste"));syn::parse_file(code(row)).unwrap();
    }
}
fn gaps(row:&serde_json::Value)->Vec<&str> {
    row["faces"].as_array().unwrap().iter().flat_map(|f|f["gaps"].as_array().unwrap()).map(|g|g["feature"].as_str().unwrap()).collect()
}
fn code(row:&serde_json::Value)->&str { row["faces"][0]["rust"].as_str().unwrap() }
#[test]
fn foundation_removes_only_its_four_requirements_from_the_33_affected_cards() {
    let (baseline,candidate)=profiles();assert_eq!(baseline.len(),33);assert_eq!(candidate.len(),33);
    assert!(baseline.values().all(|r| !gaps(r).is_empty()));
    let still_blocked:Vec<_>=candidate.iter().filter(|(_,r)|!gaps(r).is_empty()).map(|(n,_)|n.as_str()).collect();
    assert_eq!(still_blocked,["Caverns of Despair","Field of Dreams","Land's Edge","Revelation"]);
    for (name,row) in candidate {
        syn::parse_file(code(row)).unwrap_or_else(|e|panic!("{name}: {e}"));
        assert!(!gaps(row).iter().any(|g|matches!(*g,"supertype World"|"static rule BlockedAsThoughLacking"|"engine: no effect sets power or toughness alone"|"continuous rule CombatDamage NoAssignment")),"{name}: {:?}",gaps(row));
    }
}
#[test]
fn candidate_uses_native_rules_and_supplies_the_required_world_field() {
    let (baseline,candidate)=profiles();
    for row in candidate.values() { assert!(code(row).contains("world: ")); }
    assert!(code(&candidate["Arboria"]).contains("world: true"));
    assert!(code(&candidate["Great Wall"]).contains("landwalk_exceptions:"));
    assert!(code(&candidate["Staff of the Ages"]).contains("Keywords::LANDWALKS"));
    assert!(!code(&candidate["Great Wall"]).contains("LoseKeywords"));
    assert!(code(&candidate["Island of Wak-Wak"]).contains("EffectKind::SetPower"));
    assert!(code(&candidate["Sentinel"]).contains("EffectKind::SetToughness"));
    assert!(code(&candidate["Gaze of Pain"]).contains("EffectKind::NoCombatDamageAssignment"));
    assert!(!code(&baseline["Great Wall"]).contains("landwalk_exceptions:"));
    assert!(!code(&baseline["Island of Wak-Wak"]).contains("world: "));
}

#[test]
#[ignore="execute the candidate profile against MTGISH_CANTRIP_DIR; use the foundation branch"]
fn foundation_generated_cards_execute_in_cantrip() {
    use std::{fs,process::Command};
    let checkout=PathBuf::from(std::env::var_os("MTGISH_CANTRIP_DIR").expect("foundation checkout"));
    let root=PathBuf::from(std::env::var_os("MTGISH_FOUNDATION_PROBE").unwrap_or_else(||PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("foundation-runtime").into_os_string()));
    fs::create_dir_all(root.join("src")).unwrap();
    let fixture=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let old=root.join("earlier-candidate.jsonl");
    cantrip::planned::with_additional_features(FOUNDATION,||cantrip::planned::tracked(&fixture.join("early_set_generator_gaps.ronl"),&fixture.join("early_set_generator_gaps.oracle.json"),&old).unwrap());
    let mut cards=read(&old);cards.extend(profiles().1.clone());cards.extend(activator_profiles().1.clone());cards.extend(step_profiles().1.clone());cards.extend(mana_loss_profiles().1.clone());cards.extend(random_cost_profiles().1.clone());cards.extend(keyword_family_profiles().1.clone());cards.extend(attached_cost_profiles().1.clone());cards.extend(untap_limit_profiles().1.clone());cards.extend(mana_tap_profiles().1.clone());cards.extend(attack_haste_profiles().1.clone());
    // Modules actually used by these scenarios; candidate modules also have separate compile probes.
    // Cube and Doom counter declarations are intentionally separate from these runtime cases.
    let modules=["Orcish Farmer","War Barge","Merieke Ri Berit","Ray of Command","Dreams of the Dead","Mirror Universe","Necropolis","Illusionary Terrain","Nameless Race","Forgotten Lore","Infernal Darkness","Tetravus","Frankenstein's Monster","Game of Chaos","Clockwork Beast","Anti-Magic Aura","Soul Exchange","Vision Charm","Island Sanctuary","Great Wall","Gaze of Pain","Arboria","Concordant Crossroads","Island of Wak-Wak", "Clergy of the Holy Nimbus","Land's Edge","Elkin Bottle","Halfdane","Telekinesis","Xenic Poltergeist","Mana Short","Power Sink","Coral Helm","Draconian Cylix","Stormbind","Balduvian Shaman","Rapid Fire","Earthlore","Tourach's Gate","Smoke","Damping Field","Winter Orb","Manabarbs","Burning Earth","Instill Energy","Chaos Lord"];
    let mut lib=String::from("#![allow(dead_code,unused_imports,unused_variables,unused_mut)]\n");
    for name in modules {
        let row=&cards[name];assert!(gaps(row).is_empty(),"{name}: {:?}",gaps(row));
        let stem:String=name.chars().map(|c|if c.is_ascii_alphanumeric(){c.to_ascii_lowercase()}else{'_'}).collect();
        let stem=stem.split('_').filter(|s|!s.is_empty()).collect::<Vec<_>>().join("_");
        lib.push_str(&format!("mod {stem} {{\n{}\n}}\n",code(row)));
    }
    lib.push_str(include_str!("fixtures/early_set_generator_scenarios.rs"));
    lib.push_str(include_str!("fixtures/early_set_foundation_scenarios.rs"));
    lib.push_str(include_str!("fixtures/early_set_activators_scenarios.rs"));
    lib.push_str(include_str!("fixtures/early_set_steps_scenarios.rs"));
    lib.push_str(include_str!("fixtures/early_set_mana_loss_scenarios.rs"));
    lib.push_str(include_str!("fixtures/early_set_random_cost_scenarios.rs"));
    lib.push_str(include_str!("fixtures/early_set_keyword_family_scenarios.rs"));
    lib.push_str(include_str!("fixtures/early_set_attached_cost_scenarios.rs"));
    lib.push_str(include_str!("fixtures/early_set_untap_limits_scenarios.rs"));
    lib.push_str(include_str!("fixtures/early_set_mana_tap_scenarios.rs"));
    lib.push_str(include_str!("fixtures/early_set_attack_haste_scenarios.rs"));
    fs::write(root.join("src/lib.rs"),lib).unwrap();
    let mut manifest=String::from("[package]\nname=\"mtgish-foundation-runtime\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n");
    for name in ["cantrip-api","cantrip-state","cantrip-mechanics","cantrip-rules"] {manifest.push_str(&format!("{name}={{path={:?}}}\n",checkout.join("crates").join(name)));}
    fs::write(root.join("Cargo.toml"),manifest).unwrap();
    let result=Command::new("cargo").arg("test").current_dir(&root).env("CARGO_TARGET_DIR",std::env::var_os("MTGISH_CANTRIP_TARGET").expect("separate native target")).env("CANTRIP_VERIFY","1").env("RUST_MIN_STACK","33554432").output().unwrap();
    assert!(result.status.success(),"{}\n{}",String::from_utf8_lossy(&result.stdout),String::from_utf8_lossy(&result.stderr));
    println!("{}",String::from_utf8_lossy(&result.stdout));
}

#[test]
fn a_landwalk_exception_does_not_unlock_ignoring_an_unrelated_ability() {
    use mtgish_ast::{corpus,ir::*,mtg_types::OracleCard,unified};
    let source=include_str!("fixtures/early_set_foundation.ronl").lines().map(|line|ron::from_str::<OracleCard>(line).unwrap())
        .find(|s|corpus::Identity::of(s).unwrap().names.iter().any(|n|n=="Great Wall")).unwrap();
    let mut lowered=unified::lower(&source);
    let Ability::Static {changes,..}=&mut lowered.card.faces[0].abilities[0] else {panic!()};
    let Change::Rule(RuleChange::BlockedAsThoughLacking(ability))=&mut changes[0] else {panic!()};
    *ability=Box::new(Operand::Vocabulary {family:"CheckHasable".into(),form:"Flying".into()});
    cantrip::planned::with_additional_features(FOUNDATION,|| {
        let generated=cantrip::whole(&source,&lowered.card).unwrap();
        assert!(generated.faces.iter().any(|f|f.gaps.iter().any(|g|g.feature=="static rule BlockedAsThoughLacking")));
    });
}
