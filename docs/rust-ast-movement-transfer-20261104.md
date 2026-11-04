# Canonical typed transfers

Implementation `be6f556` completes the remaining normalized movement candidate
from the user's untracked `consolidation.md`, following the lexical selection
checkpoint `760c356` (integrated on main at `a924482`). The other five semantic
candidates were already integrated. The original input, source corpus, Oracle,
reference captures and earlier proof directories remain unchanged.

## Canonical contract and migration boundary

Validated cards now contain one movement operation:
`Move { objects, destination, entry, face_down, terms }`. `EntrySpec` carries
typed entry counters, tapped state, attachments and characteristics. Selection
occurs in ordered `WithSelections` steps at the same lexical execution point;
the enclosing action retains its original `ResultId`. `SelectionId` remains a
separate namespace. The previous checkpoint's simultaneous preparation,
execution barriers and pre-action/transitioned result views remain intact.

`MovementTerms` makes ordering, placement chooser and whole-set qualifications
explicit. A random subset is a chance selection, while moving every object in
a random order retains `MovementOrder::Random`. A stated top-or-bottom chooser
is independent of both selection and each owner's default placement choice.
No-selection qualifications never silently become subset selections.

Group terms now describe distinct properties, whole name classes, dynamic
totals and one choice per type. `AllWithChosenName` forces the entire offered
name class, rather than permitting a subset with one shared name. Type domains
distinguish live objects (including tokens) from historical casts. Existing
exact/partial count policies, especially costs and targets, remain distinct.

Face-down exile visibility and pile ordering are separate from battlefield
face-down characteristics. A stated creature's complete face-down shape stays
typed and unsupported when the native engine cannot express it; it is never
substituted with manifest. Unknown richer group/entry terms retain explicit
primitive diagnostics, rather than disappearing.

The approved bounded completion applies to the **validated semantic contract**.
`DraftChangeZone`, `DraftChosen` and `DraftRandom` remain hidden, clearly named
producer/old-decoder forms with their original serde names and enum positions.
Current validation rejects them; the normalizer elaborates resolution uses.
There are zero such nodes in all 33,529 normalized corpus records and no legacy
movement/selector branches in the backend. Physically deleting these draft
variants and changing every source-returning builder is a separate source API
migration. Cost, target and predicate choices are not hoisted to resolution:
manually supplied draft selectors at those sites now reject. This stricter
non-corpus validation behavior is intentional, rather than an equivalence claim.
`MoveTopCard` retains its distinct inspect/conditional-entry semantics.

Schema **176** replaces 175. Adding the movement fields changes the binary
payload despite preserving existing enum positions. Normalized files and binary
caches must be regenerated; old binary records must not be relabeled. RON/JSON
old capture decoding uses default terms, followed by normalizing and validating
the new schema. Focused RON, JSON and bincode round trips pass.

## Consumer audit

The backend's old `change_zone`, raw group/entry decoding, synthetic result-ID
selection plumbing and redundant exile-entry path are removed. Selection
preparation and ordinary movement use shared typed paths. The existing
`OneForEach` implementation is factored into a local-returning helper, preserving
its reuse/party/color policies. Historical type domains retain explicit gaps
outside the supported history/filter policy.

Every specialized movement consumer was audited. Gathered and optional
per-player moves clone the actual terms; player-dependent entry, destination
and placement terms cannot flatten across the loop. Typed player-reference
traversal replaces Debug matching in dependency detection, distinguishing object
IDs, text literals, shadowed loops and delayed captures. Independent ability
declarations remain boundaries.

Combined dig/search/library recipes require plain entry, visible movement and
default terms, and reject local selection scopes they cannot consume faithfully.
Exile-until-return, replacement destinations, self-bottom costs and start-game
recipes have corresponding default-term checks. Source/destination/prompt-only
inspections do not execute or erase movement terms. The sole protected
`lower_reflexive.rs` edit is its approved default-terms constructor field.
No numeric ranges, counter operations, native code, vocabulary or live jobs
were changed.

## Corpus and generated-output proof

An independent closed migration of the captured schema-175 native JSON matches
**all 33,529 whole records** exactly after the declared representation changes:
8,059 existing Move defaults, 131 draft zone changes across 121 records, 63 new
selection scopes, 54 entry flag/list nodes and 14 group constraints. Every other
operand, result, binding, instruction order, face and identity remains exact.
The final current-source JSON is byte-identical to that proof's after capture.
All records normalize, validate and round trip without primitives.

All capability decisions remain unchanged: **28,511 backend-capable records**.
Five held records have more precise first diagnostics for stated face-down
creature entry: Cybership, Death in Heaven, Missy, Tezzeret, Cruel Machinist and
The Cyber-Controller. Four gain an instruction location; Missy's earlier true
face-down gap now precedes its existing later per-player-choice gap.

Complete unprinted modules and full diagnostic lists were compared, preserving
all tokens/literals except the two provenance comments and the separately
classified fallback captions and verified injective private-binder renames:

| Cohort | Records | Exact | Caption only | Binder/caption | Structural | Changed full gap lists |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Legacy zone changes | 121 | 38 | 1 | 42 | 32 | 8 |
| Selection/scratch controls | 82 | 74 | 2 | 2 | 4 | 0 |
| Entry/spread controls | 266 | 233 | 33 | 0 | 0 | 0 |

The 32 legacy structural differences are exhaustively classified:

- Seventeen are alpha-exact after eliminating provably unread, pure export
  bookkeeping: Caravan Vigil, Coax from the Blind Eternities, Deadly Brew,
  Death Wish, Eivor, Haunting Voyage, Karn the Great Creator, Michelangelo,
  North Wind Avatar, Phyrexian Portal, Ruin in Their Wake, Scholar of New
  Horizons, See the Unwritten, Sphinx Ambassador, The Raven's Warning,
  Treasure Chest and Verdant Mastery. Action calls, live result reads and
  predicates are preserved; the checker never deletes a live export.
- Seven use the existing random helper for the same without-replacement
  subset: Charmbreaker Devils, Exalted Flamer, Ghoulraiser, Orcish Librarian,
  Parallel Thoughts, Viconia and Woodland Sleuth. Counts and recorded random
  pool sizes remain identical. Mangara's Tome and Parallel Thoughts also
  acquire the correct search destination prompt and inline the same exile link.
- Six avoid offering a meaningless may-move question for an empty pool:
  Coral Fighters, Draugr Thought-Thief, Eye Spy, Jace, Lurking Informant and
  Precognition. Nonempty moves use the same action and chooser.
- Karn, Scion of Urza uses the shared simultaneous counter application on
  the new exile identities. Its redundant stale-ID predicate is unnecessary:
  the native `exile_all` already filters through `live_ones`, with existing
  `stale_move_tests` covering this contract. Its result complement is unchanged.

The four selection-cohort differences are Lich's Mastery's corrected Exile
purpose/caption and Hustle, Ugin's Mastery and Zimone's equivalent optional
one-object choice without a separate may question. None changes the selected
pool, cardinality, chooser, action or result provenance.

Changed full gap lists are Cybership, Death in Heaven, Only I Know What Awaits,
Tempest Efreet, Tezzeret, The Celestial Toymaker, The Cyber-Controller and The
Moonbase. These remain held: typed generic movement removes the duplicate raw
ChangeZone fallback gap; live type-domain and linked-pile operations can use
existing engine paths, while independent layout/event/history/runtime gaps stay
explicit. No whole-card readiness increase is claimed.

Printed production output independently covers the same 121 affected records:
**78 ready before and after**, 10 exact complete modules, 36 verified alpha
changes, 32 classified structural changes, 42 identical first gaps and one
more specific reference guard. Across 266 printed controls, **219 complete
modules are byte-exact** after provenance removal and **47 gaps are identical**.
Current unprinted module bodies also match the exact captures compiled natively.

## Native and test evidence

The final generator run passes **985 tests across 92 targets**, with two existing
ignored tests. Seven transfer regressions add actual old-producer whole-record
captures, draft rejection, idempotence/codecs, optimization negative controls,
random subset/order separation, placement chooser separation, entry/hidden pile
terms, forced name classes, token-inclusive domains and no-selection qualifiers.
Typed traversal and reference-guard negative controls cover unrelated IDs,
quoted strings, lexical shadowing, delayed captures and named-subset pools.
The final layout-only cleanup has a separate successful cargo check.

An isolated native crate points to immutable engine
`def1558de5a5ebd33e4e85b51d3d6abfb22406ea`; no donor changes were made.
It compiles **341 exact before/after card module pairs plus 18 probe modules
(700 modules)** and passes **nine real-engine tests**. Tests exercise whole
name classes; distinct assignments of multitype objects; chance subsets with
following-result counts and recorded RNG; replay-equivalent random order;
stated versus owner placement; shuffled linked face-down exile; counters on new
exile identities; token-only live type domains; and counters on every entering
permanent. The last two prove bounded generator recoveries using existing APIs.

Four excluded modules are checked in separate before/after crates with identical
12-error signatures: Descent into Madness (two missing Despair references),
Haphazard Bombardment (three Aim references), Ian Malcolm (one ExileCast.instead
initializer; already fixed on later main), and Emergent Ultimatum (one obsolete
Choice.player field plus five pre-existing inline query borrow conflicts).
No enum was aliased and no emitted card semantics was edited to make them pass.

## Separate reference finding and remaining work

Technomancer remains held with
`a narrowed choice pool where the card says from your graveyard`. Local Oracle
text says “then return any number of artifact creature cards with total mana
value 6 or less from your graveyard to the battlefield.” Source corpus record
28539 (line 28540) instead selects `Named(0)`, the preceding Mill result. It
therefore excludes pre-existing graveyard cards. The typed guard preserves the
old safety decision and gives a specific diagnostic. The source, Oracle and
reference files are untouched; root owns the separate upstream reference report.

The six original **normalized semantic consolidation candidates are complete**
under the approved boundary. Remaining work is source/parser API cleanup and
independent existing generator/runtime/reference gaps, not overlapping validated
movement nodes. Root must preserve its newer vocabulary fingerprint and native
features when integrating this isolated older-base commit. No remote writes were
performed and all build/proof handles are terminal.

Machine-readable hashes and exact receipts are in the adjacent proof JSON.
Large captures, independent comparators, complete code/diffs and native scaffold
are retained under `target/movement-transfer-proof/`; earlier receipts remain
under `target/movement-proof/`.
