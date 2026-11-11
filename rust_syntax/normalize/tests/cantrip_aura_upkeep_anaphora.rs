#![cfg(feature = "cantrip")]
use mtgish_ast::{cantrip, lower, mtg_types::OracleCard};

#[test]
fn upkeep_anaphor_uses_trigger_host_but_explicit_host_stays_dynamic() {
    let original = r#"Card(Name: "Probe", Typeline: (Supertypes: [], Cardtypes: [Enchantment], Subtypes: [Aura]), ManaCost: [ManaCostU], Rules: [EnchantPermanent(IsCardtype(Creature)), TriggerA(AtTheBeginningOfAPlayersUpkeep(SinglePlayer(HostController)), ActionList([PutCounters([ACounterOfTypeOnPermanent(PTCounter(-1, -1), Trigger_ThatPermanent)])]))])"#;
    let dynamic = original.replace("PTCounter(-1, -1), Trigger_ThatPermanent", "PTCounter(-1, -1), HostPermanent");
    let render = |line: &str| {
        let source: OracleCard = ron::from_str(line).unwrap();
        let card = lower::card(&source).unwrap();
        cantrip::planned::with_additional_features(&[], || cantrip::module(&card)).unwrap()
    };
    let fixed = render(original);
    let effect = fixed.split("fn ability_1(g:").nth(1).unwrap();
    assert!(effect.contains("item.subject.object()"), "{effect}");
    let dynamic = render(&dynamic);
    let effect = dynamic.split("fn ability_1(g:").nth(1).unwrap();
    assert!(effect.contains("q.attached_to(o)"), "{effect}");
}
