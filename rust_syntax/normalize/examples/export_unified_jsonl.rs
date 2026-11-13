//! Export fresh, validated IR for local corpus audits without changing saved IR.
use mtgish_ast::{corpus, mtg_types::OracleCard, unified, validate};
use std::{
    fs::File,
    io::{BufRead, BufReader, BufWriter, Write},
};
fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(
        args.len(),
        3,
        "usage: export_unified_jsonl <source.ronl> <output.jsonl>"
    );
    corpus::on_large_stack(|| {
        let input = BufReader::new(File::open(&args[1]).unwrap());
        let mut output = BufWriter::new(File::create(&args[2]).unwrap());
        for line in input.lines() {
            let source: OracleCard = ron::from_str(&line.unwrap()).unwrap();
            let lowered = unified::lower(&source);
            assert!(
                lowered.diagnostics.is_empty(),
                "{:?}: {:?}",
                corpus::Identity::of(&source),
                lowered.diagnostics
            );
            validate::unified(&lowered.card).unwrap();
            unified::round_trip(&lowered.card).unwrap();
            writeln!(output, "{}", serde_json::to_string(&lowered.card).unwrap()).unwrap();
        }
    });
}
