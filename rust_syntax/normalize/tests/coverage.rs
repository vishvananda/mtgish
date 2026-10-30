use mtgish_ast::report::Report;
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
    assert_eq!(report.complete, 2);
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
