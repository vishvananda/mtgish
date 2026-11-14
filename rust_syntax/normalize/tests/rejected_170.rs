#![cfg(feature = "cantrip")]
use mtgish_ast::{cantrip, corpus, lower, mtg_types::OracleCard, validate};

#[test]
fn stardew_controller_chooses_the_player_who_may_accept_control() {
    corpus::on_large_stack(|| {
        let source = include_str!("fixtures/rejected_170.ronl");
        assert!(source
            .contains("APlayerMayAction(Other(You), GainControlOfPermanent(Ref_TargetPermanent))"));
        assert!(!source.contains("ChooseAPlayer("));
        let source: OracleCard = ron::from_str(source.trim()).unwrap();
        let card = lower::card(&source).unwrap();
        validate::card(&card).unwrap();
        let abilities = serde_json::to_value(&card.abilities).unwrap();
        let body = &abilities[2]["args"]["body"]["instructions"];
        assert_eq!(body[0]["args"]["operation"]["kind"], "Draw");
        let action = &body[1]["args"]["operation"];
        assert_eq!(action["kind"], "PlayersDo");
        assert_eq!(action["args"]["any_one"], true);
        assert_eq!(action["args"]["optional"], true);
        assert_eq!(
            action["args"]["players"],
            serde_json::json!({
                "kind":"OthersThan","args":{"kind":"AbilityController"}
            })
        );
        let code = cantrip::module(&card).unwrap();
        syn::parse_file(&code).unwrap();
        // The original controller chooses the doer. The doer's local frame
        // supplies both the acceptance decider and the new control recipient.
        let choose = code
            .find("choose::player(g, asker.controller, asker.source, among_")
            .unwrap();
        let frame = code[choose..]
            .find("Asker::new(asker.source, doer_")
            .unwrap()
            + choose;
        let accept = code[frame..].find("ask_may(g, decider_").unwrap() + frame;
        assert!(choose < frame && frame < accept);
        assert!(!code.contains("planned!"));
    });
}
