# Coin procedures and destination-specific arrivals

Schema 186 combines the two remaining structural AST clarity changes into one
native/Wasm release batch. Source grammars, card data, engine features and
numeric widths are unchanged.

Coin operations carry an explicit called/uncalled protocol. Fixed batches retain
their count, while series express a finite count, per-object extent, bounded
procedure or named stopping condition. Outcome counts distinguish heads/tails
from wins/losses, and named receipts from the current program's implicit tally.
Protocol inference occurs only at Source and historical-input boundaries, using
scoped typed observations. Current authoring and generation consume its explicit
policy. Missing backend channels remain capability diagnostics.

Movement carries the arrival state belonging to its destination. Battlefield
arrivals support permanent entry state; exile arrivals support visibility,
piles and return programs; other card arrivals carry card changes and counters.
Library destinations own their position, ordering and placement chooser.
Conjuring keeps ownership independent from the battlefield controller and
library position. Ordinary movement, specialized top-card/counter/conjure
actions, costs and legality constraints use these types. Battlefield creators
and entering-event replacement state use battlefield-only payloads.

Historical EntrySpec/EntryState are input/backend adapters rather than portable
IR. Shared checked converters handle source flags, saved JSON, readable legacy
code and backend views. Saved binding origins follow structural moves, including
private return programs; numeric IDs are never used to guess lexical identity.
The editor hides empty arrivals, offers zone-specific sockets, and upgrades
older named workspaces through native validation. Bare default card-code
destinations avoid empty tuple placeholders.

The generator audit caught a Guild Feud regression in an existing Debug-string
receipt lookup. Lowering now records typed battlefield-controller metadata and
uses it to select each player's actual transition. A genuine schema 185 full-card
capture protects the behavior. The full affected generator cohort is unchanged
following this fix.

Native verification passes 1,497 tests with zero failures and 17 opt-in audits
excluded from the default run. The separately invoked migration and canonical
origin audits each pass all 33,529 records; migrated records equal fresh Source
output exactly. All 10,156 affected generator rows match 185 after excluding only
schema/fingerprint comments: 8,506 modules and 1,650 unchanged capability gaps.
Authentic coin 76 and movement 87 named documents and ten schema 185 workspaces
are retained as migration fixtures; historical captures are unchanged.

All 17 browser suites pass against the same release Wasm and final local HTML,
including all 33,529 block roundtrips, independently checked binding ports and
native/Wasm validations. Actual constructor edits retain default visibility,
fill destination choices, hide empty arrivals, preserve incompatible nonempty
state and undo correctly. All ten genuine 185 named workspaces import and start
with their binding identities intact. Only one Rust release Wasm build was
needed; the final editing correction only repacked JavaScript.

Local receipt: `/mnt/vizier/ast-review-20261109/movement186-local-validation.json`.
The final HTML SHA-256 is
`92b0e7c69a6c8276f80ab818c69ad684244016e1d7316f27f6a4450aaaf45a3d`.
Publication of the site/Wasm is deferred at the user's request while remaining
work is batched. The live site remains schema 185. The original 29 reference
fixes remain a separate subsequent pass; the Oracle worker has since confirmed
three additional cases. No Oracle worker reports/pins were edited.
