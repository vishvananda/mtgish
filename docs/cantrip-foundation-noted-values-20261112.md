# Typed noted values and activation payments

The noted-values primitive records exact activation payments and immutable linked
notes. Jeweled Amulet now generates without recorded gaps. Ice Cauldron clears
its note and last-noted-mana requirements, retaining its spending predicate.
Tawnos's Coffin clears its counter note, retaining three independent delayed
return/arrival/attachment requirements. The fixed 286-card scope is now **221
ready candidates / 65 gapped cards / 83 raw diagnostic labels**. No unrelated
gap list changes. These are generation candidates, not installed card commits.

The existing cost scheduler returns the exact mana spent on a locked activation
cost. Only an ability marked `tracks_mana_spent` captures that pool, with its
actual stack serial. This includes cost increases/reductions, rather than
reconstructing mana from X or reading the compact source/type history tally.
The latter remains narrow and unchanged. Nested or later payments do not share
the receipt. Paying a cost does not itself note anything: only resolution's
explicit note instruction records the new last note. A copy was not activated
and has no mana spent on its own activation cost (CR 707.10). The receipt protocol
is specifically for stack activations, and rejects immediate mana abilities.

Each payload carries the original object incarnation, printed text and acquired
ability link (CR 607.2e, 607.5). Source departure does not invalidate a captured
note, but a returned object or independently acquired text cannot read it as its
own. Empty notes replace old values; charge counters added by another effect
create no note. Type-only notes retain mana kinds, including colorless, rather
than paid amounts. Production adds the exact recorded pool or one mana of a
noted kind. A multi-kind one-mana production chooses a kind. No note produces no
mana. The existing could-produce authority executes the real mana callback;
payment advice retains its existing unknown-yield behavior.

Tawnos's Coffin's generated note reads the original creature's retained counters
after exile, not the new exiled card's counterless row. The sparse counter
snapshot is immutable and has its own payload index for a later closure to
capture. Recording a counter snapshot does not supply the still-missing return
procedure. A separate runtime probe executes only the real exile and note
instructions; it deliberately omits the unsupported delayed return and never
claims that the whole card is executable.

Payloads are append-only and journalled; fork, undo, redo, equality and state
hashing retain their exact contents. The game state gains one empty vector
header (24 bytes); ordinary activations allocate no payload. An opted-in payment
and its note each use a 48-byte record, with a separate sparse vector for used
counter kinds. Objects remain 848 bytes, stack items 320 bytes and events 28
bytes. No per-object counter, numeric widening or AST layout/schema change is
introduced. Reading a note uses the tracked history API so the existing loop
detector cannot equate positions whose history-dependent answers differ.

The explicit candidate capability `noted-values` enables only mana-spent and
counter-snapshot notes and their last-mana production. Other note kinds retain
their gaps. The ordinary generator API pin is unchanged. The current typed IR
already expresses these semantics correctly, so there is no new MTGish parser
or reference defect to report upstream for this family.

Focused validation passes 13 native checks, 31 generator checks and 77 actual
generated runtime scenarios. The last three scenarios cover Jeweled Amulet's
colored/colorless notes, charge counters from another effect, no prior note,
and the Coffin partial note probe. All 221 ready modules plus Reflecting
Mirror's existing partial callback control compile against this native branch
with the existing Pin/Sleight names-only counter overlay.

The [proof receipt](cantrip-foundation-noted-values-20261112.proof.json) records
final full-workspace gates, revisions, performance and hashes. Local evidence
is under `/mnt/vizier/early-set-foundation-20261112/noted-values`.

Published native commits: `7815d013f` (typed notes/payment capture), `714048e93`
(tracked history reads), and `b5000fa91` (four fixture initializers required by
the preceding target-chooser feature). No gameplay expectation or mana-window
answer was changed in those fixture edits. Generator implementation is
`d7c4203d1e2ab4dfa2af07c74c1c18c1f9fdcba6`. The complete native workspace passes
**22,724 tests** (15 ignored, 238 targets), and the complete Rust generator
workspace passes **2,079 tests** (24 ignored, 202 targets). The final 128-game
benchmark completes every game at **50.74 us** engine answer time and **27.31 KB**
allocated per decision; allocation is unchanged from the preceding checkpoint.
See the [shared-family stopping checkpoint](cantrip-foundation-shared-milestone-20261112.md).
