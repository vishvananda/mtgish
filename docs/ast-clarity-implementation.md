# AST clarity implementation

Active objective: implement all changes from
[the November 9 review](ast-consistency-review-20261109.md), including the agreed
Scratch-style inputs and preserved named variables. The baseline is fork main
`06ea5cd`, normalized schema 179. This document tracks completion against the
whole objective; an intermediate passing stage does not complete the goal.

| Requirement | State | Required evidence |
| --- | --- | --- |
| Validate every nested semantic entry child in its correct scope | Implemented, verified including private return events and fresh grants | Reproduced malformed inputs rejected; valid captured/granted scopes retained; current corpus validates |
| Consistent nonnegative and ordered cardinality bounds | Implemented, verified | Exact/between/mode negative cases; dynamic bounds retain existing execution rules |
| Typed entry and copy payloads, removing backend descriptive-term decoding | Implemented, verified including destination-specific serialization in local batch 186 | No generic semantic payload escape in these families; full source/canonical corpus and generated-code comparisons |
| Preserve binding identities and names for targets, results, loops, and selections | Implemented, verified including independent alternatives and legacy migration | Scoped document roundtrips, edits, save/import/export, shadowing/type errors, canonicalization mappings; numeric compiler IDs retained |
| Destination-specific movement state | Implemented, native/full-corpus/browser verified in local batch 186 | Impossible combinations rejected by representation; exile visibility, battlefield arrival, library placement and batch identity preserved |
| Typed mode-selection mechanisms, coin-flip policies, keyword-action arguments | Implemented; keyword/mode migrations published, coin policies verified in local batch 186 | All existing feature combinations/results preserved; malformed combinations rejected; cohort generator comparisons |
| Consistent Rust role/type/action names and typed counter-event sources | Implemented, verified and published in batch 185 | All producers/consumers migrated; no ambiguous complete `Named` result reads or generic counter-source recipient variant |
| Clear sequence, batch, iteration, and receipt boundaries | Implemented, verified without changing execution forms | Declaration docs and discoverable authoring families; receipt and native behavior regressions |
| Shared schema/declaration discovery and topic modules | Implemented, verified | Visitors, codec, catalog, fingerprint and frozen contract use one source registry; no declarations lost |
| Scratch-style literal/picker shadows in typed expression sockets | Implemented, verified | Real drag/drop replacement, simple-value restoration, undo/redo, persistence, validation and browser tests |
| Typed known layouts with an explicit extension contract | Implemented, verified and published in batch 185 | All 18 current layouts, metadata-only records and layout errors covered |
| Final integration, publication and deployment | All local integration gates pass; deployment deferred for the combined release | Full appropriate Rust tests, 33,529 record semantic/wire checks, browser/Wasm coverage, pushed fork main and live `/blocks` smoke |

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

Stage-10 publication: commits `7e57b6c`, `f37e70b` and `50da95c` are pushed to
fork main. The tested `/blocks` bundle is deployed as `20261109-50da95c`, SHA-256
`5adfca9b2f0baf743b5ac58c1514e3bd1e2a2d4333861a85ea90d9b166a958e6`.
The live file matches the tested artifact and the live copy-editing/migration
smoke passes (`copy-live-browser-copy.log`).

Stage 11 begins destination-state validation before changing its serialized
representation. Shared checks reject battlefield-only state (including nested
conditional arrivals) on hand/library/graveyard/exile movement, and exile-pile
or exile-return state on a battlefield destination. Top-card movement and
countering use their stated destination; token creation, populate, manifest and
meld use the battlefield. Conjure's library placement requires a library zone.
Selection constraints use the same checks. Counters and granted abilities on
exiled cards remain valid, including Delay's three time counters and suspend.
Specialized actions retain their existing hidden-exile encoding; ordinary Move
keeps its separate visibility flag. No schema, numeric type or generator feature
changes in this validation step.

Four focused destination regressions, the existing 16 typed-entry/copy checks
and five nested-reference/bounds checks pass. Lexical reference errors retain
precedence over destination diagnostics; the existing missing-target regression
is preserved without weakening its assertion. All 33,529 current normalized
records remain valid. Evidence is
`arrival-focused-final.log`, `arrival-corpus.log` and the inventory
`movement-destination-inventory.json` under the review directory. The full native
suite passes 1,439 tests with zero failures (14 opt-in audits ignored). All eleven fresh-Wasm browser suites pass, including three destination-error
probes, 14 real card-code examples and all 33,529 corpus block/binding/validation
checks (`arrival-final-browser-*`). The schema and all 33,529 source-derived
records match stage 10 byte for byte. This does not
complete the pending serialized destination/payload migration or the remaining
mode/coin/keyword, Rust API naming/counter-source and typed-layout work.

The remaining entry payload work includes `FaceDownAs`'s generic power/toughness
and creature type, `FaceDownWith.abilities`, `FaceDownLand`, `May`,
`RulesUntil.rules`, and `ExiledForWith.body`. The outer `EntrySpec` migration did
not make those internal payloads typed. Complete that typing with the
zone-specific destination payload work, using explicit archived-input origin
maps to preserve saved authoring identities. An exact 12-occurrence corpus
inventory is in `entry-inner-payload-inventory.json`. The current destination checks are
a boundary invariant for that migration, not a replacement for it. For later layout work, the inventory is also retained as `layout-contract-inventory.json`:
18 known layouts include a zero-face sticker sheet and one-/two-face token
definitions, plus typed metadata required for MeldPiece and Vanguard. Avoid a
universal face-arity rule.

Stage-11 publication: commits `593a3f9` and `197568b` are pushed to fork main.
The release `20261109-197568b` is live at `/blocks`, SHA-256
`bbbea23c1533758c3bf7d58d586cce066b0711740d0df25a76ce1b1a8f8f6d34`.
The live file matches the tested artifact; live card-code editing, 14 real code
examples and seven malformed-input probes pass (`arrival-live-browser-code.log`).

Stage 12 fixes a semantic frame defect found by the entry audit, independently of the
queued reference bugs. In Lagrella's `ExiledForWith`, canonicalization changed
`EventSubject` in the return-trigger body into `Source`, using the creating
ETB event. A spell creating the same return trigger was rejected for lacking
an outer event. Baseline reproductions are `entry-return-{original,canonical-before}.json`
and `entry-return-spell-probe.{json,log}` in the review directory.

The fix gives both the return predicate and body a distinct
exile-to-battlefield event frame. The duration retains its creator context;
target/result captures remain available, local results stay private, and damage
or other roles from the creating event are unavailable. The body must contain
a program. Four synthetic frame regressions and a real Lagrella source/authoring
regression pass. The full native suite passes 1,444 tests with zero failures
(14 opt-in audits ignored). All 33,529 records pass complete canonical origin
maps; all 82,226 definitions rename and reparse without failure. Exported records
and wire shapes are unchanged; only field documentation changes in the catalog.
Lagrella's existing generated capability gap is unchanged. The missing-target
probe now puts its reference inside a proper return program and retains its
original assertion. The transport regression also rejects substituting the
creating source for the returning permanent despite an otherwise valid ID map.
All eleven fresh-Wasm browser suites pass, including the spell-created return
scope, refusal of a non-program body, 15 real card-code examples and all 33,529
block/binding/validation checks. Evidence is in `entry-return-*` under the review
directory. This is an internal normalizer/canonicalizer defect; the captured
source grammar record is correct, so it does not require a reference correction
upstream.
Schema 181 and numeric ranges remain unchanged. This scope fix takes priority
before the remaining inner-entry typing/destination migration; it does not
complete the overall AST objective or the queued 29 reference fixes.


Stage 13 (schema 182) types the remaining inner entry payloads. `FaceDownAs`
has numeric power/toughness and an optional creature type; face-down land has a
`LandType`; grants contain fresh `Ability` scopes. Optional entry contains an
`EntrySpec` and an explicit decider separate from the controller override.
Timed rules use `ChangesUntil` and the ordinary typed `RuleChange` vocabulary.
Return callbacks contain a `Program` directly, retaining stage 12's return event
and private result scope. Unknown entry forms remain `Draft` diagnostics and
cannot validate as complete IR. The artifact flag defaults to false; zero power
and toughness remain explicit values. Numeric ranges and engine behavior are
unchanged. Characteristic decoding is shared with copy-source conversion.

A single bottom-up migration walks immutable input descriptors for 179/180/181,
checks exact fields and face epochs, then converts payloads. Mapped 180/181
upgrades record explicit origins before carrying author metadata; the plain 179
upgrade retains its existing contract without claiming a mapped authoring
migration. The replaced copy-only walker and field-name-based 179 rewrite are
removed. Shared PTX references expand to two recorded origins. An overwritten
controller reference maps to no output instead of being attached to the later
controller. Unexplained legacy origins refuse migration rather than guessing
from matching numeric IDs.

Six focused regressions cover 16 authentic schema-181 cards (17 faces), their
named authoring documents, typed shapes/scopes, cloned reads, overwritten
controllers, strict families and unknown fields. Immutable old fixtures and
real Magar/Lagrella workspace captures remain unchanged. All 33,529 schema-180
and schema-181 inputs migrate to exactly fresh source lowering with complete ID
maps; all 33,529 historical schema-179 upgrades also agree. Complete canonical
origin maps pass for all records. All 82,226 definitions rename and reparse
successfully. Full native validation passes 1,450 tests with zero failures
(15 opt-in audits ignored in that run). Evidence is `entry182-*` under
`/mnt/vizier/ast-review-20261109/`.

The 16 affected generator outputs retain their prior emitted code/capability
status: 15 compare identically after removing schema/fingerprint comments;
Lagrella retains the same move-arrival gap with a more precise instruction path.
The old planned return-callback template is explicitly gated until the engine
can supply its return event. This AST work does not add that backend feature.

The queued reference work is checkpoint105's 29 confirmed open defects against
reference revision `8126cc9792747df0b8fdd02d202b025854023b3f`. Its seven unresolved
rows remain separate. Revalidate findings on our fork and report required
reference changes upstream after completing the broader AST/editor objective.
Destination-specific serialization, mode/coin/keyword policies, Rust API role
names/counter sources, typed layouts, and final integration remain open.

All twelve fresh-Wasm browser suites pass, including typed entry sockets, direct
return instruction chains, named schema-181 import/startup migration, unchanged
binding keys and rejection without replacing the workspace. The catalog suite
roundtrips all 33,529 block models, compares their typed ID ports and validates
them with Wasm, with zero failures. Card-code coverage includes 15 real examples
and eight malformed-input probes. Browser receipts are
`entry182-browser-results.json` and `entry182-browser-*.log`. Authoring document
2 and workspace presentation 6 remain unchanged.

Stage-13 publication: commit `b740ba7` is pushed to fork main. Release
`20261109-b740ba7` is live at `/blocks`; SHA-256 is
`ef65c6b490a19a00196ed8bd80a7187d8eb57a0e88ce6eab85ddb7d8d4639a02`.
The live file matches the tested artifact. Live card-code and typed-entry/named
schema-181 migration smoke suites both pass (`entry182-live-{code,entry}.log`).
The machine-readable release receipt is `entry182-release.json`.


Stage 14 is implemented and verified: schema 183 places keyword parameters in their action
variants. Scalar actions use `KeywordAction`; `KeywordEach` has a separate
`ObjectKeywordAction` so actions on a set retain a single operation/result and
the existing duration contracts. Fight/exchange have two explicit objects;
amass has a numeric amount and creature-type vocabulary; adapt and monstrosity
retain an optional explicit subject; counted forms retain
absent versus explicit repetition. The old four optional operation fields are
removed. Source/legacy tags and strict converters live at `keyword_actions`,
outside complete IR; the backend borrows the same typed parameters rather than
introducing engine features. Primitive escapes for these known actions are
rejected. Amass source vocabulary must belong to `CreatureType`.

Baseline captures precede all model edits: 151 source records/named documents
cover all 158 observed parameter shapes, with authentic schema-182 workspaces
for fight, connive and amass. The inventory has 2,492 keyword occurrences across
2,234 records. All 2,255 generator outputs (2,252 distinct display names) compare
identically after schema/fingerprint headers are removed, including repeated
names. Migration composes explicit parameter-field origin moves with the earlier
entry/copy conversions. A full audit caught schema-182 entries being sent through
the older entry-payload converter again; entry conversion now runs only through
181. Sixteen authentic schema-182 entry captures cover that version boundary.

Focused keyword cases pass, including every captured source shape and all named
documents. The new browser suite passes specific starters, literal sockets,
creature-type editing, named import/startup, binding-key preservation and failed
upgrade retention. An intermediate corrected full native run passed 1,456 tests with zero
failures (15 opt-in audits ignored). Native canonical origin maps pass for all
33,529 records and all 82,226 definitions rename/reparse successfully. Final
historical migration and fresh-Wasm checks are recorded below. Evidence prefix: `keyword183-` under the review
directory. This stage does not complete the pending mode/coin, destination,
Rust naming/counter-source, layout or final-integration requirements, or the
queued 29 confirmed reference defects.

The final contract review also retains adapt/monstrosity's explicit other-object
subject, which the backend already distinguishes as a capability gap. Two named
authoring documents captured with the real schema-182 binary cover this non-corpus form;
its target origins, names and existing engine gap must survive. Prior optional
fields are removed where irrelevant, not where they encode this valid intent.
This receiver contract is included in the final verification below.

Stage-14 final verification uses the frozen optional-subject contract. All 1,457
native tests pass with zero failures (15 opt-in audits ignored in that run).
The four historical epochs 179/180/181/182 each migrate all 33,529 records to
exact fresh source output; mapped epochs 180/181/182 have complete typed origins.
Complete canonical maps pass for all 33,529 records, and all 82,226 definitions
rename/reparse without failure. All thirteen fresh-Wasm browser suites pass,
including all 33,529 block roundtrips, binding-port comparisons and validations.
The keyword suite additionally checks both optional-subject authoring documents
through Wasm. All 2,255 generated outputs remain identical after removing the
schema/fingerprint headers. Final receipts/logs use `keyword183-release-*` in the
review directory. The earlier in-progress evidence above records the route to
this final verified contract; all remaining AST requirements stay open.

Stage-14 publication: commit `1bf77dc` is pushed to fork main. Release
`20261109-1bf77dc` is live at `/blocks`, SHA-256
`cab4901650df882eeaaab0ba9e29ee8157a39261aa18c1f6902d30eb6c52857a`.
The live artifact matches the tested file. Live card-code and keyword editing,
legacy workspace and optional-subject document migration suites both pass.
Release receipt: `keyword183-release.json`; live logs:
`keyword183-live-{code,keywords}.log`. The user's `consolidation.md` is untouched.
When schema 183 becomes a historical input in the next migration, gate keyword
payload conversion to epochs through 182, just as entry conversion is gated to
181; do not replay a payload migration on its already typed representation.

Stage 15 is implemented and verified: schema 184 groups mode bounds, options, rules,
selection mechanism and optional weight budget. Repetition, historical
availability, surcharge, target sharing and acceptance use typed policies.
Announced selection retains both an explicit chooser and a random/player
method; implied modes have no announced chooser. All valid independent rules
can coexist. Absent and explicit controller choosers, absent and free surcharge,
and separate minimum/maximum expressions retain their distinct representations.
The source/historical flag adapter lives outside semantic IR.

The grouping preserves scope-bearing traversal order: bounds, option scopes,
historical availability and surcharge, chooser, then budget. This avoids
special reader traversal or scope metadata rewrites. Captures made with the
schema-183 binary cover 178 source records and 182 detailed shapes, their named
documents, and three synthetic combinations with nested cost/chooser scopes and
creator captures. Real schema-183 workspaces capture Season of Gathering's
budget and Shadrix Silverquill's optional distinct-target choice. Migration
tracks explicit subtree origins. Keyword payload conversion is now gated to
historical versions through 182, so already typed schema-183 keywords survive.
The validator additionally checks LastCombat's player reference.

The editor has a Choices & modes category with a modal ability starter and
specific rule blocks. Ordinary count/rules/mechanism defaults stay out of the
compact view. This stage does not complete coin policies, serialized movement
states, role/counter names, known layout typing or final integration. The coin
prerequisite notes are in `coin-policy-prerequisite.md` under the review evidence
directory. The 29 confirmed reference defects remain queued after these updates;
the checkpoint's seven unresolved investigations are separate.

Stage-15 final validation passes 1,462 native tests with zero failures (15
opt-in audits ignored in the normal run). All 33,529 inputs from each of
179/180/181/182/183 migrate to exactly fresh lowering; mapped epochs 180–183
have complete typed origins. Complete canonical maps pass for every record,
and all 82,226 definitions rename and reparse without failure. All fourteen
browser suites pass, including the full catalog's block roundtrips, ID-port
comparisons and Wasm validation. The final independent rebuild has identical
catalog, descriptors, Wasm, glue and HTML hashes to the tested artifact.
Evidence uses `mode184-*` in `/mnt/vizier/ast-review-20261109/`.

All 797 affected generated outputs compare identically after excluding only
schema and generator fingerprint headers. A separate combined-rule regression
fixes an early return after LastCombat history: additional distinct-target,
chooser, random and budget policies now retain their capability diagnostics,
rather than disappearing behind an apparently complete generated module. No
engine feature or numeric-range change is introduced. Existing synthetics and
assertions use the new field paths; authentic historical captures are unchanged.

Checkpoint105 was reread and still has exactly 29 confirmed open reference
defects, seven unresolved questions and one upstream-fixed/frontend-refused
case. Those are not conflated. The remaining broader AST work and subsequent
reference queue remain open. When schema 184 becomes an old input, gate mode
payload conversion to epochs through 183 rather than applying its historical
flag decoder to already typed ModeSelection records.

Stage-15 publication: commit `ecef36f` is pushed to fork main. Release
`20261109-ecef36f` is live at `/blocks`; its SHA-256 is
`c7814885e42b96578cb05f438c32cb14bc098515eea877ed4b910fc9a9b2fc48`.
HTTP returns the exact tested artifact. Live card-code and mode editing/named
schema-183 migration suites pass (`mode184-live-{code,modes}.log`). Release
receipt: `mode184-release.json`. The untracked user `consolidation.md` is intact.


Stage 16 resolves a coin-policy prerequisite without changing schema 184:
[Two-Headed Giant's source template](reference-coin-faces-20261109.md) now
compares the actual heads count, rather than wins. The complete 33,529-record
Source/native comparison changes exactly this card's two quantity reads. This
is a source correction, so earlier epoch migrations continue to preserve the
historical input's meaning; they do not silently rewrite an old saved card by
name to match a newer Oracle correction. The explicit coin-policy migration
must use the corrected source as its fresh baseline and distinguish historical
source deltas from structural schema upgrades. Mode and keyword work are
complete; the broader AST goal and the 29 confirmed reference defects remain
open. No upstream update was pulled into the fork.

Stage-16 publication: `a620cf8` is pushed to fork main and deployed as
`20261109-a620cf8`. The HTTP artifact matches the tested file and the live code
suite passes sixteen examples. Upstream report:
[i5jb/mtgish#238](https://github.com/i5jb/mtgish/issues/238). Full receipts and
source/native comparison evidence are in `coin-prerequisite/` under the review
evidence directory. Schema and Wasm bytes remain identical to stage 15; only
the catalog's corrected card changes. The original 29 confirmed defects remain
queued after the broader AST/editor work.

Stage 17 resolves the other coin-policy prerequisite without changing schema
184: [collective heads checks](coin-batch-heads-20261109.md) now retain their
explicit participants rather than masquerading as losses. The full comparison
changes exactly one condition subtree on Mana Clash; all coin producers and
receipts are retained. Contextless and stale checks fail rather than guessing.
Native tests: 1,467 passed, zero failed, 15 ignored. All fourteen browser suites
pass, including the complete catalog and seventeen card-code examples. Authentic
corrected schema-184 coin records and named documents are captured before the
next structural migration. The remaining broader AST work and the 29 confirmed
reference defects remain open; this Rust-only correction needs no upstream
reference grammar report.

Stage-17 publication: `8094fd0` is pushed to fork main and deployed as
`20261109-8094fd0`. HTTP serves the exact tested artifact; the live code suite
passes seventeen examples and ten malformed inputs. Full release evidence is
`mana-clash/release.json`. The whole AST/editor objective remains active.

Stage 18 is being validated as one combined batch: [role names, typed counter
sources, typed layouts and coin receipts](ast-role-layout-batch-20261109.md).
Normalized schema 185 carries one explicit migration, with archived schema-184
descriptors and real named documents/workspaces. Focused native checks pass for
29 complete records covering all 18 layouts and 11 API cases; all 121 explicit
names are retained. Full validation and publication are pending. Coin policy
and destination-state typing are still open; no schema-185 deployment is claimed
by this checkpoint.

Stage-18 final verification: 1,480 native tests pass, zero fail, 15 opt-in audits
ignored. Both complete 33,529-record migration and canonical-origin audits pass;
migration equals fresh Source output exactly. Twenty-nine printed generator
records/diagnostics are unchanged after excluding schema/fingerprint comments.
All fifteen browser suites pass, including the complete catalog and six genuine
schema-184 named workspace imports/startups. The single-card projection retains
its hidden literal Card tag for backward decoding; general records use the typed
layout enum. Source/reference grammars, numeric widths and engine features are
unchanged. The remaining coin and destination migrations and subsequent
reference queue remain open.

Stage-18 publication: `4f8192f` is pushed to fork main and live as
`20261109-4f8192f`. HTTP matches the tested artifact. Live card-code and API/layout
suites pass, including named schema-184 workspace import/startup. Full receipt:
`api185/release.json`. Rust role names/counter sources and typed known layouts
are complete. Coin protocol/repetition and destination-state typing remain open,
followed by final integration and the 29 confirmed reference corrections. The
user's untracked `consolidation.md` remains untouched.

Unpublished batch 186 progress (2026-11-09): coin procedures now carry an explicit
called/uncalled protocol, repetition has finite/per-object/bounded/until forms
with named stopping rules, and tallies name their outcome instead of using a
win/loss boolean. The backend selects its API from this protocol without printed
text. Unsupported observation channels remain explicit backend gaps; native IR
can describe both physical faces and wins/losses. All 76 genuine schema-185 coin
records migrate to exact fresh Source output with complete binding origins,
and all 76 original named documents retain their names. The focused native run
passed 20 tests, including standalone Source operands and explicit current-program tallies. All 76 whole-card generator outputs and gaps are unchanged
(61 gap-free). Evidence: `coin186-focused3.log`, `coin186-generated-final-focused.jsonl`, and
`coin186-fixtures/coin185-generated.jsonl` under the review evidence directory.
The implicit current-program tally remains distinct from a required receipt;
empty/conditional histories remain legal IR and an unsupported backend reports
a gap. No narrower receipt-scope rule is imposed on this legacy tally.

Movement work is in progress in the same unpublished batch. Zone-specific
arrival types and shared forward/projection converters have been added, with
library ordering/placement confined to library destinations and separate
conjure ownership/position rules. **The operations have not yet been switched
to these types.** This is preparation, not completion of the serialized
destination migration. The captured schema-185 comparison cohort has 87 records
covering 89 movement contexts, including Delay, Desertion, Lagrella, Magar,
conditional arrivals and random library order. The full native/Wasm rebuild,
schema-186 freeze, browser migration proof and deployment are intentionally
held until the representation and its consumers are integrated, to avoid
rebuilding the same dependencies for each change. Published/live remains 185.

The movement core audit passed five tests, including all 33,529 original
schema-185 records: 8,230 movement destinations and 191 conjure destinations
roundtrip without losing state. The six original workspaces and 87 named
documents (348 explicit names) were captured with the shipped schema-185
Wasm/editor. They are checked-in fixtures for the pending integrated migration,
not evidence that the operation switch or browser migration already passes.
Evidence: `movement186-core3.log`, `capture_movement185.log`, and
`batch186-progress.json`. No full Wasm rebuild or deployment occurred.


Batch186 integration now wires zone-specific destinations into the serialized
operations, costs and constraints, including battlefield-only creator payloads
and replacement entry state. Historical EntrySpec/EntryState live only in the
normalizer adapter. Source lowering, native reference validation, scoped
canonicalization, backend views and explicit migration origins consume the
new types. Legacy human code is resolved by the normal scoped reader before
using the same checked migration. All native test targets compile; runtime,
full-corpus and browser validation are pending. Schema186 remains unpublished.
The generator comparison baseline includes all10156 affected source records.


Integrated schema186 validation: 1,497 native tests pass, with17 opt-in audits
excluded from the normal run. The explicit migration audit separately proves
all33,529 genuine schema185 records equal fresh Source output with complete
binding origins. The independent canonical-origin audit passes all33,529.
All10,156 affected generated card outputs and diagnostics are unchanged after
excluding only schema/fingerprint comments (8,506 files and1,650 existing gaps).
The comparison caught a Guild Feud receipt regression caused by a lookup of
old destination Debug text. It now uses typed battlefield-controller metadata,
with a saved185 full-card regression. Historical test captures remain unchanged;
their loaders explicitly upgrade old destination/cost/constraint shapes.
The release Wasm build and17 browser suites remain pending; schema186 is not
published by this native checkpoint.


Final local schema186 integration passes all17 browser suites, including33,529
block/binding/Wasm checks and genuine185 named workspace upgrades. Destination
constructor edits create their default children, hide empty arrivals, retain
nonempty incompatible state and support undo. Only one release Wasm compilation
was needed; the editing fix was JavaScript-only. Final local artifact SHA-256:
`92b0e7c69a6c8276f80ab818c69ad684244016e1d7316f27f6a4450aaaf45a3d`.
Receipt: `movement186-local-validation.json`. All reviewed representation changes
are implemented. Site/Wasm deployment is deferred at the user's request while
remaining work is batched; live remains185. The reference pass now has32
confirmed open defects (the original29 plus three newer findings), separate
from unresolved representation questions. No upstream update was pulled.
