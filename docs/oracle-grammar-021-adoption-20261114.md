# Oracle Grammar 0.2.1 consumer decisions

The consumer adopts `grammar-v0.2.1`, commit
`113228bef8457a9fd0e7f5d30e35d50841e42bd7`, using published
`oracle-ir-contract = "=0.2.1"`. `oracle-ir` stays at `=0.2.0` and schema
191. The Blockly bridge uses the same contract package. No AST layout,
quantity width, browser workspace migration or Cantrip engine change belongs
to this adoption.

The matching published bundle is
[faces-oracle-20261112220158-s191-g0.2.1-r1](https://github.com/vishvananda/oracle-grammar/releases/tag/faces-oracle-20261112220158-s191-g0.2.1-r1).
All six assets and the manifest are checked; the manifest SHA-256 is
`c0fdd3bef9aef1446c1c6287d396f71574522b54cbea0e207d1586e452251b4a`.
The original Oracle snapshot and allowed population are unchanged. Updating the
bundle pin does not import that snapshot into MTGish or turn a broader snapshot
comparison into an adoption gate. The adoption baseline is the existing frozen
33,529-record MTGish Source corpus.

## Lowering changes

The five input-checked amendment groups U0032–U0036 contain 122 operand
replacements on 106 faces. The regression fixture preserves their exact owned
from/to operands and evidence.

- Octomancer's gift is one 8/8 blue Octopus. The defect is the shared
  `data/tokens.json` definition, which said 1/1, rather than the article count
  or the gift reducer. Correct the shared token and its cached Source record;
  preserve quantity, recipient, colors, types, entry and the end-step trigger.
- Own-spell X becomes AnnouncedX inside that Spell's announcement and resolving
  program. Independent abilities, costs, deferred programs, other objects and
  captured information retain their explicit queries. Banefire's separate
  stack-scoped static ability therefore keeps its original representation.
- A proved exactly-one activation exile payment uses the first card of its own
  Activation/Performed/After receipt for a property, rather than a sum over that
  singleton. The proof uses the owner's typed cost-history API. Optional,
  plural, other-payment and independent ability contexts are excluded.
- A choice immediately following revelation of the whole hand selects from
  that observation, guarded by current Hand zone and original owner. Partial
  and private observations retain their distinct meanings; this does not
  replace them with a fresh hand query.
- A plain distinct exchange pair becomes one target group of exactly two.
  The published canonicalizer proves the pair is equivalent before fresh
  lowering changes its announcement and second target index. This avoids a
  second local proof of target domains, filters, distinctness and slot uses.

The same family fixes also cover seven adjacent frozen records outside the
amendment list: Down, Down to Goblin-town; Martial Coup; Null Summoner; Render
Speechless; Solve for Disappointment; Stinging Vitriol; and Vision Quest. The
first, third, fourth, fifth and sixth are the same revealed-hand choice; Martial
Coup and Vision Quest are own-spell X. They must not retain alternate forms
merely because they were absent from the supplier's allowed amendment cohort.

The latter four are this adapter's lowering/normal-form changes. They do not
imply an error in MTGish's source grammar and require no new MTGish parser issue.
The owned canonicalizer remains responsible for equality and saved-value
normalization; fresh producer changes are not silent semantic repairs to saved
incorrect cards.

## Rejected issue 169

The maintainer's [clarification](https://github.com/i5jb/mtgish/issues/169#issuecomment-6884910096)
rejects our earlier interpretation. After looking at Bison Whistle's top card,
the controller can decline the battlefield/hand options and put **any** card
into the graveyard, or leave it on top. Our Bison-or-creature gate was wrong.
Restore the original four-option private menu. Keep its per-option Bison and
creature restrictions, but remove the gate around the menu and the restriction
on its graveyard option. Preserve unrelated fixes from commit 00c879b3.

The full Source parser and typed lowering have explicit regressions for this
case. Historical saved values keep their old meaning. Oracle Grammar should
apply the corrected reference using its checked amendment process, rather than
add an equality alias between the two different meanings.

## Reports and other-worker handoff

- [MTGish #291](https://github.com/i5jb/mtgish/issues/291) reports the shared
  Octopus token defect, reproduced against current upstream token blob
  `e44bd01d496257952a9532c1ec2a0a766a02d888`.
- [Oracle Grammar #27](https://github.com/vishvananda/oracle-grammar/issues/27)
  records the rejected #169 disposition and required reference correction.
- U0032 already fixes Octomancer in the owned reference. Its root-cause notes
  should distinguish the wrong predefined token data from quantity lowering.
- U0033–U0036 are adopted here. Retain them while the adapter still pins an old
  MTGish export; retire them in the same checked update that adopts a new export.
  Do not apply them twice or overwrite immutable release assets.

No Oracle Grammar working files were edited; its worker's uncommitted grammar
changes remain intact. The paused 57-card implementation backlog stays paused.

## Validation

All 33,529 current Source records freshly parse and match the cached Source
exactly. Only Bison Whistle and Octopus Token change. The existing 182 audit
findings are reviewed successfully, with zero unresolved findings. All 33,529
new unified values validate. Applying only the 122 supplier amendments, the
seven adjacent same-family changes and the two Source corrections reproduces
the complete new export: 115 records change and 33,414 remain byte-identical.
Every other field is checked unchanged.

The initial complete Rust workspace run has 2,098 passes, 21 ignored and three
failed historical producer comparisons. Those compare a saved value to current
Source: Bison Whistle, Red Sun's Twilight and Collective Brutality. An explicit,
input-checked test-only ledger now records their reviewed changes; their saved
Source/AST fixtures are preserved. All 22 tests in those three affected suites
then pass. Combined unique results are 2,101 passed and 21 ignored across the
240 workspace targets. No production inputs changed after the full run.
The full Go suite, eight Python export-tool tests and the actual wasm32 bridge
build also pass.

A focused comparison uses the newly published bundle only where the original
Oracle wording exactly matches the frozen MTGish input. It retains release
occurrences, including alternate layouts: 77 unique outcomes agree semantically
(72 canonical-only, five presentation-only). Four ambiguous and 24 refused
outcomes remain visible and do not count as matches; 27 differently worded
occurrences are excluded. The strict structural gate remains false. This is
not a claim of whole-release agreement or adoption of a fresh Oracle snapshot.

See [the proof record](oracle-grammar-021-adoption-20261114.proof.json) for
checksums, changed records and validation scopes. Existing pending discard
backend work was present during the workspace check and remains separate from
this adoption commit. Its generator-only target-profile probes are unrelated to
the reference corrections and do not count as implemented cards.
