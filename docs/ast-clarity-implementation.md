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
| Typed entry and copy payloads, removing backend descriptive-term decoding | Entry and copy implemented, verified | No generic semantic payload escape in these families; full source/canonical corpus and generated-code comparisons |
| Preserve binding identities and names for targets, results, loops, and selections | Implemented, verified including independent alternatives and legacy migration | Scoped document roundtrips, edits, save/import/export, shadowing/type errors, canonicalization mappings; numeric compiler IDs retained |
| Destination-specific movement state | Pending | Impossible combinations rejected by representation; exile visibility, battlefield arrival, library placement and batch identity preserved |
| Typed mode-selection mechanisms, coin-flip policies, keyword-action arguments | Pending | All existing feature combinations/results preserved; malformed combinations rejected; cohort generator comparisons |
| Consistent Rust role/type/action names and typed counter-event sources | Pending | All producers/consumers migrated; no ambiguous complete `Named` result reads or generic counter-source recipient variant |
| Clear sequence, batch, iteration, and receipt boundaries | Implemented, verified without changing execution forms | Declaration docs and discoverable authoring families; receipt and native behavior regressions |
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

Stage 5 introduces `human::document::Document` (authoring version 1). It stores
scoped, typed names and ID-use sites alongside the unchanged schema-180 card.
Formatting and JSON serialization preserve explicit names, including unused
results. Single and batch renames update all uses atomically and reject
collisions, wrong namespaces, forged sites and references moved across scopes.
Player and object loop bindings have separate semantic lookup roles even when
the engine labels are identical. Parser binders activate after their inputs;
loop starting players and selection inputs retain their outer context.

The document's card is read-only. `rebind_ids` accepts a validated pure compiler
ID renumbering and retains names; structural edits require an explicit site
mapping. Semantic comparisons canonicalize a copy of the card, excluding names.
The new CLI commands are `format-card-code`, `read-card-document` and
`format-card-document`. The Wasm bridge returns the document from code parsing
and exposes describing bindings and formatting a saved document. The existing
editor still uses its semantic-only code flow; editor wiring is not complete.

The rename cohort found a canonicalization bug on Explosion of Riches: sibling
reflexive targets shared the same renumbering map. Created-trigger target maps
now inherit their creator's context and restore it afterward. A second
regression covers overlapping player/object loop labels; canonicalization now
uses their semantic reference roles. Both fixes preserve numeric types and the
wire contract. A schema-derived test requires a role for each Binding position.
Nested object iterations now restore a shadowed outer binding. Each player-action
payment part sees the pre-payment result context; move receipts publish after
all parts, as in native validation. Quoted programs and after-event programs keep
private result definitions. The named-document tests cover these scopes, with
separate canonicalization regressions for shadowing and deferred receipt export.
The all-binding audit also exposed Warrior's Blades: activation cost modifiers
were visited before their target labels were established. Canonicalization and
the authoring reader now visit the announcement before activation modifiers,
matching their validated scope. Arbitrary numeric labels and the shorthand
`Targets(recipient)` both have focused regressions.

Remaining work for named authoring at the stage-5 checkpoint (completed by stages 6–8):

- Attach a stable binding identity to each block's ID port in `extraState`.
  Regenerate document paths from the current tree, including statement chains,
  combined effect blocks, single-face views, options, enum wrappers and IDs
  embedded in literal target pickers. Metadata
  must move with its block during edits; a global path/name table would go stale.
- Reuse the native document/bridge for validation, formatting and exports. A
  code apply must install the returned document, and saved workspace restoration
  must preserve names even after the model fingerprint changes.
- Show declaration names and typed reference choices. Keep compiler ID fields
  in IR details, and preserve explicit versus automatic result naming.
- Preserve identities during constructor/domain rebuilds, undo/redo, copies,
  save/import/export and fresh worker initialization. Metadata bootstrap should
  merge into the current history entry, not add a spurious undo step.
- Implement structural canonicalization/migration mappings before changing the
  authoring document's semantic structure. Keep the rejection for unmapped edits;
  the current `rebind_ids` API deliberately handles only ID changes.
- Verify real browser edits, repeated names in independent scopes, reference
  movement, rename collisions, persistence and the full corpus with fresh Wasm.

Evidence for stage 5 is under `/mnt/vizier/ast-review-20261109/names-*`.

The full native stage-5 suite passes 1,383 tests with zero failures (10 opt-in
audits ignored), including private after-event results and target-dependent
activation modifiers. The fresh release Wasm passes all eight browser suites, including all
33,529 block roundtrips and validations. CLI formatting through saved document
JSON preserves both `victim` and an explicit unused `hit`. This native milestone
does not change the live editor's save/code flow or the deployed `/blocks` page.

Final name-audit evidence: every binding in all 33,529 normalized records was
renamed to a distinct explicit symbol, formatted, reparsed and compared by
canonical meaning, with zero failures. The audit reports every failure before
failing, rather than stopping at the first card. Evidence:
`names-all-renamed-final.log`, `names-publish-native.log`, and
`names-publish-browser-*.log`. Editor integration and structural transport remain
open; this checkpoint does not complete the overall clarity objective.

Stage 6 integrates names into the editor. `bindings.js` stores stable identities
on block ports and recomputes their current typed wire paths. The normal code
apply, import, formatting, export and workspace-save paths retain authoring
names. Declaration fields accept atomic native-checked renames; reference menus
probe candidates through the same native scope/type validator. Results need no
name until used or explicitly named through the ordinary menu. Engine IDs,
schema 180 and numeric ranges stay unchanged; workspace presentation is now 5.

Copies get new identities and IDs, with internal references remapped and external
captures retained. Foreign clipboard data is also deconflicted. Known deleted
identities distinguish a cut/paste from a copy. A moved reference with a reused
numeric ID cannot silently attach to another declaration: export is blocked,
and the native context remains available to explicitly choose its new binding.
View/domain rebuilds and model-fingerprint upgrades carry matching typed-site
metadata forward. Undo/redo normalizes restored snapshots so serialization alone
does not create another undo step or truncate redo history.

Evidence: `names-ui-editor-captures.log` covers real text/menu interactions,
optional result naming, all binding kinds, equal-ID player/object loops,
collision rollback, copies, cuts, scope rejection/recovery, saves/reloads,
fingerprint upgrades and named code export. `names-ui-port-catalog.log` compares
every binding port independently with wire types for all 33,529 records, with
zero failures; all records also roundtrip and validate in Wasm. The final browser
suites all pass and are recorded in `names-ui-publish-*.log` (nine suites including
the naming workflow). No Rust source changed in this
stage; the preceding 1,383-test native evidence still applies.

Remaining: native mappings through structural canonicalization and schema
migration, typed copy exceptions, destination-specific state, selection/coin/
keyword policies, Rust role/type renames, batch/receipt documentation and typed
layouts. This editor milestone does not complete the full objective.

Stage 7 carries authoring identities through structural canonicalization.
`canonical::mapped_unified` returns an unchanged numeric card representation
and a complete typed-ID origin map. Source sites can move, clone, coalesce or
be removed. Explicitly introduced sites are supported by `binding_map::SiteMap`,
and maps compose with `then`. The traced canonicalizer uses the ordinary typed
preparation rewrite's path moves and checks its entire output against ordinary
canonicalization. Typed ability discovery includes single abilities and
metadata-only records, rather than searching for JSON fields named `abilities`.

`Document::transport` retains binding indices and explicit names atomically.
It rejects incomplete or mistyped mappings, changed meaning, ambiguous binding
merges/splits and removal of explicit names. Callers retain the original document
and the full mapping report to resolve those decisions. Automatic names adopt
the canonical label; names remain outside semantic equality. CLI commands
`canonical-ir` and `transport-card-document` expose the workflow. Canonical maps
address the exact current input epoch; migration is never silently performed
before assigning origins. Each future schema migration must supply its own
explicit map if binding paths change.

The transport regressions exposed six existing preparation errors. Hoisting
missed references in later choosers and future programs, could capture inherited
or nested selections, and left consecutive preparation layers for another pass.
The authoring reader discarded valid outer selection names on entry to a
nested preparation. Scoped collection, capture-avoiding renaming and sequential
binder activation now preserve these meanings. IDs retain their original widths.

Evidence: 14 transport regressions and six preparation regressions pass;
the CLI roundtrip preserves `chosen_cards`, rejects incomplete maps and unknown
fields, and requires explicit upgrades of legacy inputs. Fresh schema and all
33,529 source-derived records match the previous catalog byte for byte. The
all-binding name audit renames and reparses all 33,529 records with zero failures.
The final native suite passes 1,403 tests with zero failures (11 opt-in audits
ignored). Explicit mapped-corpus validation proves complete maps, equality with
plain canonicalization and idempotence for all 33,529 records. All nine browser
suites pass, including 33,529 block roundtrips, typed binding-port comparisons
and real Wasm validations. Receipts are under
`/mnt/vizier/ast-review-20261109/maps-*`.

An additional scope audit after stage 7 reproduced an authoring-identity gap:
two `If` branches or `ChooseOne` options that each declare and read `picked`
produce one `BindingName` in their parent scope. Native validation correctly
checks those programs in independent cloned scopes. The named reader must keep
their local identities separate, while allowing explicit `JoinResults` to refer
to the appropriate alternative receipts. Raw numeric receipt labels may be
reused by multiple alternatives, so resolving these requires the join's branch
group, not a global name-to-ID lookup. Reproductions are
`names-branch-{if,choice}.{card,json}` in the evidence directory. This follow-up
takes priority over the next typed-copy migration. The existing passing corpus
audits do not prove this synthetic independent-branch case.

Stage 8 separates `If` branches and `ChooseOne` options in native documents and
the editor. Document version 2 adds readable scope paths and explicit receipt
group expressions, retaining independent declaration identities even when
several alternatives reuse one numeric result label. Formatting joins expands
the member names and qualifies repeated names with `ReceiptFrom(scope, name)`.
Qualified ordinary references remain subject to native visibility. Transport
keeps declaration names through canonical group expansion/coalescing and rejects
forged members or scopes. No engine IDs, numeric ranges or schema 180 fields
change.

Version-1 document migration validates legacy grouping before splitting names;
presentation-6 workspace migration uses that same native migration and retains
the original draft until it succeeds. Copies of complete receipt groups get
fresh declaration identities and a consistent fresh group label. Save/reload,
imports, independent renames and qualified reference selection have real browser
coverage.

The scope audit also aligned result exports with validation for optional,
repeated and loop bodies, future programs, and root versus nested `PayCost`.
The canonicalizer previously exported private nested payment receipts; it now
respects the instruction boundary. Numeric declaration reservation prevents
automatic names from colliding with later explicit IDs; numeric references use
the actual visible declaration in the correct binding namespace.

Final stage-8 evidence is in `branches-final-*` under the review evidence
directory. The native run passes 1,424 tests with zero failures (11 opt-in audits
ignored), including 21 focused branch regressions. The explicit audits map all
33,529 records and independently rename/reparse all 82,226 definitions with zero
failures. Fresh source lowering and the schema match the prior catalog byte for
byte. The fresh Wasm browser run covers ten suites, including branch editing and
migration plus all 33,529 block roundtrips, binding-port checks and validations.
Typed copy exceptions and the other pending AST requirements remain open; this
verified naming milestone does not complete the whole objective.

Stage 9 documents instruction sequences, single-instruction ordered clauses,
simultaneous batches, alternative scopes, loop exports, fixed repetition and
repeatable processes at their declarations and in the IR guide. Shared native
metadata supplies labels and discovery families for every control instruction
as well as related operations. The Control menu now includes sequences, option
choices, both explicit loop forms, bounded processes, local selection scopes,
and receipt tools. Search includes instructions and their Rust documentation;
option-choice starters supply two empty alternatives.

Evidence: `control-native.log` passes 33 authoring/branch/join regressions;
`control-contract.log` passes three registry/target/frozen-schema checks. All
ten fresh-Wasm browser suites pass (`control-browser-*`), including actual menu
search and inserting a sequence around an existing action without changing its
receipt. The 33,529 records and complete wire descriptors match the preceding
build; all records pass block roundtrips, binding-port comparison and validation.
This stage changes documentation and authoring discovery, not execution forms.

Still open: typed copy exceptions, destination-specific state, mode/coin/keyword
policies, Rust role/type renames and typed counter sources, and typed known
layouts. The migration-map contract must be applied to later schema changes.
Stages 8 and 9 complete the identified naming/scope and control-discovery work,
not the overall objective.

Follow-up requested after the AST/editor clarity work: investigate and fix the
29 confirmed reference defects cataloged in
`~/oracle-grammar/reports/analysis/upstream-reference/current-reviews-checkpoint105.json`.
That report reviews reference revision `8126cc9` and separately lists seven
unresolved card rows. Revalidate each confirmed defect against the current fork
before fixing it, retain the local source and proposal evidence, and report
upstream when the defect belongs there. Do not mix unresolved questions into the
29 confirmed count or pull upstream updates as part of this work.

Stage 10 replaces the descriptive payloads in token recipes, continuous copies,
copy arrivals and stack copies with ordered `CopyException` values (schema 181).
`CopyColors` and `AbilityPattern` express their own domains. Creator expressions
retain the creating scope; granted/quoted rules define fresh scopes. Validation
rejects unrelated payloads, mismatched type/subtype combinations, invalid color
lists and color-of-mana reads from non-mana results. Copy-family source decoding
is restricted to the lowering and explicit legacy-import boundary. The generator
consumes typed variants and retains the existing engine numeric ranges and
capability gaps; no engine feature is added.

The immutable schema-180 machine descriptors drive an explicit typed migration
and complete ID-site origins, including cloned and removed reads. Schema-180
named documents and real saved browser workspaces carry names, receipt-group
identity and stable block keys through that map. Historical schema-179 imports
remain supported without falsely claiming mapped origins for their older entry
migration. Wrong families, invented forms, unknown fields and mixed face epochs
are rejected. Historical fixture captures remain unchanged; only their explicit
compatibility loaders and current-schema test constructors change.

All 684 affected generated card modules and their capability diagnostics agree
with the preceding generator, excluding only schema/fingerprint comments. A
full 33,529-record migration audit agrees exactly with fresh source lowering and
validates complete typed ID origins. The final native run passes 1,435 tests with zero failures (13 opt-in audits
ignored). Independent audits rename and reparse all 82,226 definitions and check
complete canonical origin maps across all 33,529 records, with zero failures.
All eleven fresh-Wasm browser suites pass, including typed copy editing, toolbox
search, named schema-180 workspace import/startup migration, and all 33,529 block
roundtrips, binding-port comparisons and validations. Evidence is in
`copy-final-*` under `/mnt/vizier/ast-review-20261109/`.
Destination-specific state, mode/coin/keyword policies, API role names and typed
counter sources, typed layouts, and final integration remain open. The queued
29 reference defects follow completion of the broader AST/editor work.
