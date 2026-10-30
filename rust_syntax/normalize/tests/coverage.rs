use mtgish_ast::report::{CoverageDelta, Report};
use std::{fs, path::PathBuf};
#[test]
fn corpus_reports_ast_and_backend_capability_separately_and_counts_nested_forms() {
    let input = std::env::temp_dir().join(format!("mtgish-coverage-{}.ronl", std::process::id()));
    let names = [
        "Kessig Wolf Run",
        "Divine Offering",
        "Heart-Piercer Manticore",
    ];
    let selected = include_str!("fixtures/core.ronl")
        .lines()
        .filter(|l| {
            names
                .iter()
                .any(|name| l.contains(&format!("Name: {name:?}")))
        })
        .collect::<Vec<_>>();
    assert_eq!(selected.len(), 3);
    fs::write(&input, selected.join("\n")).unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let report = Report::check(&input, &root, None).unwrap();
    assert!(!report.backend_checked);
    assert_eq!(report.backend_capable, 0);
    assert!(report.backend_unsupported.is_empty());
    assert!(report
        .records
        .iter()
        .all(|r| r.backend_diagnostic.is_none()));
    #[cfg(feature = "cantrip")]
    let backend = Report::check_with_backend(&input, &root, None).unwrap();
    #[cfg(not(feature = "cantrip"))]
    assert!(Report::check_with_backend(&input, &root, None).is_err());
    fs::remove_file(input).unwrap();
    assert_eq!(report.total, 3);
    assert_eq!(report.complete, 3);
    #[cfg(feature = "cantrip")]
    {
        assert!(backend.backend_checked);
        assert_eq!(backend.complete, report.complete);
        assert_eq!(backend.backend_capable, 1);
        assert!(backend.records.iter().any(|r| r
            .identity
            .names
            .iter()
            .any(|n| n == "Kessig Wolf Run")
            && r.complete
            && !r.backend_capable
            && r.backend_diagnostic.is_some()));
        assert!(backend
            .records
            .iter()
            .any(|r| !r.complete && !r.backend_capable && r.backend_diagnostic.is_none()));
    }
    assert!(report.forms["Keyword"].contains_key("Trample"));
    assert!(report.forms["Expr"].contains_key("AnnouncedX"));
    assert!(report.forms["CostPart"].contains_key("Mana"));
    assert!(report.forms["Instruction"].contains_key("Effect"));
    assert!(report.forms["Change"].contains_key("GrantKeyword"));
    assert!(report.forms["ObjectFilter"].contains_key("CardType"));
    assert_eq!(report.forms["Operation"], report.operations);
}

#[test]
fn coverage_compare_matches_cards_by_name_and_requires_a_review_for_losses() {
    let report = |json: &str| -> Report {
        serde_json::from_str(&format!(
            r#"{{"schema_version":1,"corpus_sha256":"","inputs":{{}},"total":2,"complete":1,"backend_capable":0,
            "backend_unsupported":{{}},"forms":{{}},"unsupported":{{}},"operations":{{}},"records":{json}}}"#
        ))
        .unwrap()
    };
    let record = |name: &str, hash: &str, complete: bool| {
        format!(
            r#"{{"line":1,"identity":{{"layout":"Card","names":["{name}"],"source_sha256":"{hash}"}},
            "complete":{complete},"diagnostics":[],"backend_capable":false,"backend_diagnostic":null}}"#
        )
    };
    let before = report(&format!(
        "[{},{}]",
        record("Lost", "a", true),
        record("Gained", "b", false)
    ));
    let after = report(&format!(
        "[{},{}]",
        record("Lost", "c", false),
        record("Gained", "d", true)
    ));
    let delta = CoverageDelta::between(&before, &after);
    assert_eq!(delta.gained, ["Gained"]);
    assert_eq!(delta.lost, ["Lost"]);
    assert_eq!(delta.unreviewed_losses(&Default::default()), ["Lost"]);
    let reviewed = [("Lost".to_string(), "restored a dropped form".to_string())].into();
    assert!(delta.unreviewed_losses(&reviewed).is_empty());
}
