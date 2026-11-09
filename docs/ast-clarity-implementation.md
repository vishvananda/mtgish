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
| Typed entry and copy payloads, removing backend descriptive-term decoding | Pending | No generic semantic payload escape in these families; full source/canonical corpus and generated-code comparisons |
| Preserve immutable names for targets, results, loops, and selections | Pending | Scoped document roundtrips, edits, save/import/export, shadowing/type errors, canonicalization mappings; numeric compiler IDs retained |
| Destination-specific movement state | Pending | Impossible combinations rejected by representation; exile visibility, battlefield arrival, library placement and batch identity preserved |
| Typed mode-selection mechanisms, coin-flip policies, keyword-action arguments | Pending | All existing feature combinations/results preserved; malformed combinations rejected; cohort generator comparisons |
| Consistent Rust role/type/action names and typed counter-event sources | Pending | All producers/consumers migrated; no ambiguous complete `Named` result reads or generic counter-source recipient variant |
| Clear sequence, batch, iteration, and receipt boundaries | Pending | Declaration docs and discoverable authoring families; receipt and native behavior regressions |
| Shared schema/declaration discovery and topic modules | Pending | Visitors, codec, catalog, fingerprint and frozen contract use one source registry; no declarations lost |
| Scratch-style literal/picker shadows in typed expression sockets | Pending | Real drag/drop replacement, simple-value restoration, undo/redo, persistence, validation and browser tests |
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
