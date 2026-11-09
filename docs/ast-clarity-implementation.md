# AST clarity implementation

Active objective: implement all changes from
[the November 9 review](ast-consistency-review-20261109.md), including the agreed
Scratch-style inputs and preserved named variables. The baseline is fork main
`06ea5cd`, normalized schema 179. This document tracks completion against the
whole objective; an intermediate passing stage does not complete the goal.

| Requirement | State | Required evidence |
| --- | --- | --- |
| Validate every nested semantic entry child in its correct scope | Scope checks implemented; typed payload constraints pending | Reproduced malformed inputs rejected; valid captured/granted scopes retained; current corpus validates |
| Consistent nonnegative and ordered cardinality bounds | Implemented, verified | Exact/between/mode negative cases; dynamic bounds retain existing execution rules |
| Typed entry and copy payloads, removing backend descriptive-term decoding | Entry implemented; copy pending | No generic semantic payload escape in these families; full source/canonical corpus and generated-code comparisons |
| Preserve immutable names for targets, results, loops, and selections | Pending | Scoped document roundtrips, edits, save/import/export, shadowing/type errors, canonicalization mappings; numeric compiler IDs retained |
| Destination-specific movement state | Pending | Impossible combinations rejected by representation; exile visibility, battlefield arrival, library placement and batch identity preserved |
| Typed mode-selection mechanisms, coin-flip policies, keyword-action arguments | Pending | All existing feature combinations/results preserved; malformed combinations rejected; cohort generator comparisons |
| Consistent Rust role/type/action names and typed counter-event sources | Pending | All producers/consumers migrated; no ambiguous complete `Named` result reads or generic counter-source recipient variant |
| Clear sequence, batch, iteration, and receipt boundaries | Pending | Declaration docs and discoverable authoring families; receipt and native behavior regressions |
| Shared schema/declaration discovery and topic modules | Implemented, verified | Visitors, codec, catalog, fingerprint and frozen contract use one source registry; no declarations lost |
| Scratch-style literal/picker shadows in typed expression sockets | Implemented, verified | Real drag/drop replacement, simple-value restoration, undo/redo, persistence, validation and browser tests |
| Typed known layouts with an explicit extension contract | Pending | All 18 current layouts, metadata-only records and layout errors covered |
| Final integration, publication and deployment | Pending | Full appropriate Rust tests, 33,529 record semantic/wire checks, browser/Wasm coverage, pushed fork main and live `/blocks` smoke |

Preserve absent versus zero costs and characteristics, result provenance,
selection timing, linked abilities, instruction boundaries, and serialization
contracts. Do not widen numeric types or add unrelated backend features. Schema
changes require an explicit migration and a frozen contract. Preserve the user's
untracked `consolidation.md`.

Stage 1 evidence (2026-11-09): nested entry/face-down/rule-body/duration references
and CounterWith destinations are now checked; interval checks are shared by
counts and modes. Five focused regressions pass. Full Rust AST suite with Cantrip
features: 1,345 passed, six ignored. All 33,529 current normalized records pass
strict validation. Evidence: `/mnt/vizier/ast-review-20261109/validation-fixes.log`,
`validation-full-tests.log`, and `validation-corpus.log`. Generic entry language
shape (including rejection of arbitrary integer entry instructions) remains part
of the pending typed-entry migration. No schema or numeric type change yet.

Stage 2 evidence: the IR is grouped into 17 topic modules behind the existing
`ir::*` re-exports. The source registry supplies traversal, frozen schema,
authoring/catalog inputs and generator fingerprints, including its own routing
code. A regression checks that every external semantic module is registered.
The schema-179 frozen contract passes; post-split catalog schema and all 33,529
exported records match the previous build exactly. Combined Rust suite: 1,350
passed, seven ignored. Evidence: `topic-modules-full-tests.log` and
`final-build/{schema.json,cards.jsonl}` under the review evidence directory.
The module guide records the existing sequence/batch/receipt boundaries; the
remaining API naming and typed representation work is still open.

Stage 3 evidence: normal authoring uses editable literal/picker shadow blocks in
real typed sockets. An expression drops directly over a simple value; removal
restores the edited prior value. Amount and player expressions, action rebuilds,
undo/redo, save/restore and migration of a real presentation-3 workspace all pass
browser regressions. Common X/arithmetic blocks and full player expressions are
available in the toolbox. Fresh Wasm passes all 33,529 block roundtrips and IR
validations, 14 varied card-code examples and the four malformed nested-reference/
interval probes. Browser, authoring, targets, code and sockets suites pass.
Evidence: `*-fresh-wasm.log`, `sockets-browser.log` and the captured legacy fixture
in `tools/blockly/fixtures/workspace-v3.json`. Presentation is now version 4;
normalized schema and numeric ranges remain unchanged.

Stage 4: schema 180 replaces grammar operand lists on Conjure, MoveTopCard,
Manifest, Meld, CounterWith and Populate with EntrySpec. Grammar flag decoding
is confined to source lowering and the explicit schema-179 JSON upgrade;
the planned engine bridge encodes typed arrivals into its existing ABI.
Delay retains three time counters and granted suspend. Unreviewed legacy flags
and unknown fields fail validation. Destination-specific arrival constraints
and typed copy exceptions remain pending.

All 33,529 captured schema-179 records upgrade to exactly the schema-180 result
of fresh source lowering. The 270 affected face outputs and existing backend
diagnostics agree after excluding fingerprint comments. This comparison does
not imply that every affected card has a backend implementation. All 33,529
records roundtrip through the browser block codec and pass the real Wasm
validator. Browser startup upgrades a real schema-179 Desertion workspace;
failed upgrades retain the original draft, and strict IR import rejects unknown
fields without replacing the workspace. Immutable historical fixture files
stay unchanged; their loaders explicitly apply the entry migration. Evidence:
`typed-entry-*` under `/mnt/vizier/ast-review-20261109/`.

The final full native run passed 1,356 tests with zero failures (eight ignored).
A focused corpus run additionally executes the ignored 33,529-record migration
comparison. Top-card destinations now validate their player references as well
as their typed arrivals. Migration rejects mixed face epochs, preserves the
browser's 255-level JSON depth contract, and rejects excessive nesting before
entering Serde. An in-flight workspace upgrade cannot replace a newer block edit.
