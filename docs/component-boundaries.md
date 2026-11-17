# Component ownership

The schema and contracts come from published `oracle-ir` 0.3.0 and
`oracle-ir-contract` 0.3.0 (schema 192). MTGish does not own a second AST.

| Repository | Responsibility |
|---|---|
| [oracle-grammar](https://github.com/vishvananda/oracle-grammar) | Owned AST, validation, canonicalization, migrations, Oracle parse/print and face releases |
| [mtgish fork](https://github.com/vishvananda/mtgish) | Source parser, Source → AST lowering, preservation audits and comparison against released faces |
| [Cantrip tooling branch](https://github.com/vishvananda/cantrip-rs/tree/tools/oracle-codegen-20261117) | AST → Cantrip generation, gap surveys, card lanes, dashboard, scenario validation and publication |
| [oracle-blocks](https://github.com/vishvananda/oracle-blocks) | Blocks ↔ AST, authoring UI and optional Oracle projection using the Grammar crate |

Historical reports under `docs/` describe the revisions that produced them. Their
old file paths are provenance, not active commands. Backend tests and Python
lane scripts moved with their implementations; frontend and Wasm tests moved to
Oracle Blocks. Source tests still use frozen historical wire fixtures where
needed to prove lowering and migration behavior.

## Portable exports

```sh
cargo build --locked --manifest-path rust_syntax/Cargo.toml -p mtgish-ast --bin ast
# Ordinary owned AST, as before:
/path/to/target/debug/ast export-unified data/mtgish.lines.ron cards.jsonl
# Producer-neutral records for consumers that also need layout roles/provenance:
/path/to/target/debug/ast export-ir-records data/mtgish.lines.ron records.jsonl
```

`export-ir-records` writes ordered JSONL with `format_version: 1`, `card` (the
owned UnifiedCard), `context`, `identity` and lowering `diagnostics`.
`context.primary_face` explicitly names the front, unflipped, main, or left face.
The AST's face array alone is not a front/back ordering contract. Vanguard
`hand_modifier` and `life_modifier` are signed integers. `identity` retains the
Source layout, face names and SHA-256 for traceability. It is provenance, not a
second Source tree. The output is atomic; malformed or empty corpora fail.

The generator never reads MTGish Source types. Any producer may supply this
versioned envelope using the published AST contracts. Backend capability and
engine support remain separate from successful lowering. Existing saved coverage
reports retain their historical backend fields for deserialization; MTGish no
longer runs backend capability checks.

## Extraction checkpoint (2026-11-15)

The frozen 33,529-record Source corpus exported exactly the same AST before and
after extraction. All 33,529 standalone generator rows matched the former
in-process generator byte for byte, including Rust, gaps and multi-face ordering.
No schema, numeric range, card implementation or engine feature changed during
this split. The 57-card engine-gap backlog remains paused.
