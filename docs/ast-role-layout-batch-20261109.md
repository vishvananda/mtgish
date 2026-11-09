# Combined role, counter-source, layout and receipt batch

This batch moves normalized IR from schema 184 to 185. It combines the pending
coin-receipt validation with two AST clarity requirements so they share one
native/Wasm release validation rather than separate rebuilds and deployments.
No numeric types or engine features change. Coin protocols/repetition and
serialized destination-specific movement state remain separate open work.

Rust names now reflect their roles: `TargetSlot`, `LoopBinding`,
`ObjectRef::Recorded`, `ObjectSet::Recorded`, `AbilityHolder`, `GrantingObject`,
`SetTapped` and arithmetic `Exponentiate`. Search distinguishes `searcher` from
`library_owner`; library reordering distinguishes `chooser` from
`library_owner`; value choices name their `choosers`. Input-only card-code
aliases retain the old spellings; serialization and current editor metadata
use the new names. Typed binding-role lookup retains historical ID type names
when interpreting old saved schemas.

`CounterSource::On(Recipients)` and `CounterSource::EventPut` distinguish counters
currently on entities from exactly the counters placed by an event. The latter
can no longer appear as a damage recipient and requires a counter-placement
event. Placement receipts survive event wrappers; alternatives expose them only
when every alternative provides the receipt. Counter replacement proposals do
not automatically become actual placement receipts.

`Layout` enumerates all eighteen known layouts. Built-in JSON layout spellings
stay unchanged. Face counts and layout-specific metadata are checked, including
Vanguard modifiers, meld partners, stickers and transforming token definitions.
An explicit `Extension { namespace, name }` uses a dotted namespace and unique
metadata names. An extension cannot impersonate a built-in through its spelling;
the Cantrip backend reports unsupported extension layouts explicitly. Unknown
bare layout strings fail instead of silently becoming extensions.

The migration walks immutable schema-179 through schema-184 descriptors before
renaming nodes or wrapping counter sources. Each moved typed ID subtree has an
explicit origin map. Mode conversion is gated through schema 183, so already
typed schema-184 modes are not fed back through the old flag decoder. Earlier
captures and schemas remain unchanged.

Focused evidence: twenty-nine genuine schema-184 records cover all eighteen
layouts and eleven API cases. All migrate to exactly the complete result of
fresh Source lowering, with complete ID origins. Twenty-nine genuine saved
named documents retain all 121 explicit names and scope identities. Six native
API/layout regressions and seven coin-receipt regressions pass. Six authentic
schema-184 Blockly workspaces are captured for browser import/startup checks.
Evidence is under `/mnt/vizier/ast-review-20261109/api185/`.

The first broad native pass exposed old test expectations using previous API
spellings and three synthetic one-face Transforming scope probes. The immutable
captures remain intact. Snapshot adapters now apply the declared aliases, and
those three malformed layouts must fail migration; their mode-scope tests then
use an explicitly corrected Card copy. Historical layout fields must also be
strings: a new Extension object cannot bypass an old string descriptor by being
labeled with an older schema version. Focused regressions pass after these
corrections. The compiler/validator rules are retained rather than relaxed to
accept malformed fixtures.

Final validation passes: 1,480 native tests, zero failures and 15 opt-in audits
ignored. Every one of the 33,529 schema-184 records migrates to exactly fresh
schema-185 Source output, with complete typed ID origins. A separate complete
canonical-origin audit passes on all 33,529 records. The 29 representative
printed-generator records, including their gap diagnostics, are unchanged after
excluding schema/fingerprint comments.

All fifteen browser suites pass. All 33,529 records roundtrip through blocks,
match typed binding ports and validate in Wasm. Six authentic schema-184 named
workspaces import and reload with their identities intact. Earlier saved entry,
copy, mode, keyword and branch workspaces also upgrade successfully. A hidden
single-card projection tag remains a literal Card value; general records use
the typed Layout field. That compatibility fix needed only repacking JavaScript,
not another Rust/Wasm rebuild. Evidence: `validation.json`, `native-final.log`,
`corpus-migration.log`, `canonical-corpus.log`, `generator-comparison.json` and
`browser-results-final.json` under the evidence directory.

The whole clarity goal remains open: coin policies/repetition,
destination-specific movement serialization and final integration remain.
The subsequent 29 confirmed reference defects and seven separate unresolved
reference questions remain queued. No upstream changes were pulled.
