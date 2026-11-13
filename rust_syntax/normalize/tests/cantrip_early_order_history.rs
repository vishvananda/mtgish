#![cfg(feature = "cantrip")]
use mtgish_ast::{cantrip, corpus, lower, mtg_types as source, validate};

fn module(name: &str) -> String {
    let source: source::OracleCard = include_str!("fixtures/early_order_history_20261113.ronl")
        .lines()
        .chain(include_str!("fixtures/early_combat_cda_reference_20261113.ronl").lines())
        .map(|line| ron::from_str(line).unwrap())
        .find(|card| corpus::Identity::of(card).unwrap().names.iter().any(|n| n == name))
        .unwrap();
    let card = lower::card(&source).unwrap();
    validate::card(&card).unwrap();
    let module = cantrip::module(&card).unwrap();
    syn::parse_file(&module).unwrap();
    module
}

#[test]
fn explicit_player_processes_move_before_the_next_player_chooses() {
    struct InPlayerLoop(bool);
    impl<'ast> syn::visit::Visit<'ast> for InPlayerLoop {
        fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
            if quote::quote!(#node).to_string().contains("bound_0") {
                struct Entry(bool);
                impl<'ast> syn::visit::Visit<'ast> for Entry {
                    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
                        self.0 |= node.method.to_string().starts_with("put_")
                            && node.method.to_string().contains("battlefield");
                        syn::visit::visit_expr_method_call(self, node);
                    }
                }
                let mut entry = Entry(false);
                syn::visit::Visit::visit_block(&mut entry, &node.body);
                self.0 |= entry.0;
            }
            syn::visit::visit_expr_for_loop(self, node);
        }
    }
    for name in ["Eureka", "Thieves' Auction"] {
        let text = module(name);
        assert!(!text.contains("planned!"), "{name}: {text}");
        let mut entry = InPlayerLoop(false);
        syn::visit::Visit::visit_file(&mut entry, &syn::parse_file(&text).unwrap());
        assert!(entry.0, "entry must happen within one player's iteration: {name}: {text}");
    }
    let text = module("Eureka");
    assert!(text.contains("again_") && text.contains("each_"), "repeat reads each player's actual result: {text}");
}

#[test]
fn before_combat_deadlines_do_not_reopen_in_a_later_combat() {
    let rapid = module("Rapid Fire");
    assert!(rapid.contains("Step::DeclareBlockers | Step::EndOfCombat"), "{rapid}");
    let berserk = module("Berserk");
    assert!(berserk.contains("Step::EndOfCombat"), "{berserk}");
}

#[test]
fn loses_control_watches_departures_as_well_as_controller_changes() {
    let text = module("Seraph");
    for required in ["Event::ControlChanged", "Event::ZoneChange { old, from, .. }",
                     "let player = q.controller(id)", "EventMask::ZONE_CHANGE", "from.is(ZoneKind::Battlefield)"] {
        assert!(text.contains(required), "missing {required}: {text}");
    }
}

#[test]
fn historical_opponent_recipients_include_a_player_who_later_left() {
    let text = module("Whirling Dervish");
    let condition = text.split("fn ability_1_if").nth(1).unwrap().split("fn ability_1_event").next().unwrap();
    assert!(condition.contains("q.state().players.len()"), "{condition}");
    assert!(!condition.contains(".opponents("), "the historical event already establishes that the recipient was present: {condition}");
}

#[test]
fn defining_stats_and_current_combat_use_the_existing_ir_contracts() {
    let shifter = module("Shapeshifter");
    assert!(shifter.contains("characteristic_defining(Layer::PtDefine"), "{shifter}");
    let war = module("Total War");
    assert!(war.contains("cantrip_mechanics::history::this_combat(q)"), "{war}");
}
