# Oracle-owned model and consumer migration

Oracle Grammar owns the portable semantic model. This fork consumes its
`oracle-ir` and `oracle-ir-contract` packages at exact published versions,
recorded with registry checksums in Cargo manifests and `rust_syntax/Cargo.lock`.
See [published dependencies and the export-comparison process](oracle-registry-consumers-20261113.md)
for the subsequent registry migration. A branch name, local
path patch, copied schema, or independently edited validator is not a model pin.

## Current migration

- `mtgish-ir` remains a compatibility crate that re-exports `oracle-ir`. It no
  longer defines enums, freezes schemas, or generates visitors.
- `mtgish-ast` re-exports shared validation, canonicalization, movement contracts,
  binding maps, saved-input readers and compact authoring from
  `oracle-ir-contract`. Existing consumer module paths remain usable.
- Legacy Source records reuse the owned game vocabulary. Their grammar-specific
  counter input and Source reduction recipes remain adapter responsibilities.
  Shared semantic counter kinds have the same Rust type identity everywhere.
- The Blockly Wasm bridge depends directly on `oracle-ir-contract`. It does not
  need MTGish Source types, lowering, corpus tools, or the Cantrip generator.
- Source vocabulary discovery includes the owned declarations as well as local
  grammar constructors. The generator fingerprint includes the dependency lock,
  so advancing the owned contracts invalidates previous generation receipts.

Schema 190, numeric domains, serialized meaning, and native engine APIs are
preserved. This is ownership/dependency migration, not additional engine work.
The ordinary MTGish Source and card-data caches remain unchanged.

## Long-term component homes

| Component | Recommended owner | Contract |
| --- | --- | --- |
| AST, wire schema, visitors, validation, binding/receipt semantics, canonicalization, compact card-code codec | Oracle Grammar workspace | Shared `oracle-ir` / `oracle-ir-contract` packages |
| Oracle parser, context binder and Oracle printer | Oracle Grammar frontend | Original Oracle text to/from owned IR |
| Blockly editor, toolbox, search UI, workspace presentation, browser deployment and Wasm entry points | A dedicated `oracle-blocks` repository | Owned IR plus versioned card catalog artifacts |
| Cantrip Rust code generator, engine capability map, API fingerprints and generated-card runtime tests | `tools/oracle-codegen` in the Cantrip repository | Owned IR to the matching Cantrip engine API |
| Legacy MTGish parser, Source recipes, Source-to-owned-IR lowering and reference export | This MTGish fork | Explicit legacy adapter and independently versioned reference artifacts |

These locations are the recommended direction for planning; this migration does not
create another repository, move Cantrip engine files, or redeploy the editor.
The model crates must never depend on any of these consumer applications.

## Blocks extraction

Move the editor assets and the thin Wasm wrapper together. Keep editor-specific
labels, categories, block layout, dropdowns and optional-field controls in the
editor. Derive the structural catalog and validation from the owned model;
never hand-maintain a competing JavaScript schema or validator. Compact card
code and binding identities remain model contracts, shared with native tools.

Card search consumes a versioned catalog containing IR, display text, schema,
producer revision and checksums. Producing that catalog can use MTGish or Oracle
Grammar independently; the deployed editor must not need a checkout of either
producer. Oracle text parsing can become an optional frontend integration.

The first concrete extraction step is already covered here: the Wasm package
uses the owned contracts directly. Native bridge tests and a real wasm32 build
must pass before moving its assets and build/deployment scripts.

## Generator extraction

The existing generator has both typed IR compilation and legacy conveniences:
whole-Source entry points, corpus iteration, PR rendering/cache, planned-card
tracking, vocabulary decoding and batch export. These need explicit boundaries
before its move. Changing only its crate name would retain the dependency.

Cantrip already has a `tools/` tree and an explicit runtime workspace. Keep
the generator as a standalone tooling crate under `tools/oracle-codegen`,
excluded from ordinary engine/card builds, with a dedicated CI gate. Runtime
crates must not acquire a dependency on the parser, AST or generator merely
to execute generated cards.

The future `cantrip-oracle-codegen` library accepts validated owned IR and emits
Rust plus structured capability diagnostics. Engine code generation,
capabilities and validation against the actual engine API belong together.
Planned future APIs remain explicit and must not count as current runtime
support. A generator-specific diagnostic type must not depend on the legacy
Source inventory report.

Source loading, Source recipes, print metadata preparation, and reference
export stay in a separate adapter here. Batch/catalog orchestration calls the
IR-only generator. Generated-card runtime tests then live next to the engine
and compile generated code against that exact engine revision.

Before moving, freeze a Source/IR/generation comparison: same accepted cards,
explicit same gaps, output and API fingerprints, and generated runtime results.
The move does not authorize widening engine numbers or resuming card lanes.

## Verification of the initial Git-provider migration

The initial migration pinned `4174f5e4fa73ff569dccb931a61a63004323d22e`, an
ancestor of [the ownership PR](https://github.com/vishvananda/oracle-grammar/pull/1).
There are no local dependency patches.

- Entire MTGish workspace: **1,994 tests passed / 20 ignored / 204 targets**.
  The earlier 2,103 count includes 93 contract regressions, four schema checks
  and 12 vocabulary export cases now owned by the provider. The provider's CI
  passes its model/contract/frontend checks and adds an independent extraction
  regression. Its validation evidence is recorded in that repository.
- All **33,529** cached Source records lower without diagnostics, validate and
  binary-roundtrip. The complete JSONL export is **byte-identical** to the
  baseline. Source cache data is unchanged.
- The frozen **286-card** generator scope remains **221 ready / 65 gapped**.
  Executable Rust, rendered card files, all gap lists and normalized IR hashes
  are identical. The only generated-text change is the generator provenance
  SHA256 header, which deliberately includes the new dependency lock. The
  Cantrip API fingerprint is unchanged.
- The browser bridge's three native tests pass and its real wasm32 build
  succeeds. Its standalone normal dependency graph contains no MTGish packages.

[The proof record](oracle-owned-model-consumers-20261113.proof.json) contains
artifact hashes and verification scope. Local full exports and logs are kept
outside Git. No engine files or card lanes were changed, and the editor was
not redeployed. Component relocation remains a separate proposed next step.
