# Joint offers, delayed recipients and blink cardinality

Reviewed Oracle commit aeea4b5 and its Ambergris joint-offer,
optional-blink-cardinality and delayed-combat-sentence packets. Oracle's code,
reference inputs, pins and reports were left untouched. Upstream main f9ab9f0 was
inspected through GitHub's read-only API; no upstream update was pulled or merged.

## Source corrections

Ambergris, Agent of Progress and Ambergris, Citadel Agent offer discarding the
whole hand **and** drawing as one optional action. The Source template instead
made only the discard optional, then drew under CostWasPaid. It now emits
MayActions containing both operations. A draw prohibition prevents choosing the
whole offer; an empty hand does not, and a replaced draw is not a prohibition.
The separate conditional and reflexive compound-payment forms remain unchanged.

Basalt Golem's Wall belongs to the player who sacrifices the blocker. The cost
already named the blocking creature's controller, but the token continuation
implicitly named Basalt's ability controller. An explicit PlayerAction now gives
the token to the same ControllerOf(BlockPair(Blocker)) as the cost, with the
controller's last-known information retained after sacrifice. Cost, timing,
paid guard and complete token recipe are unchanged.

A fresh parse retains all 33,529 records and changes exactly those three Source
records. All three Source formats are regenerated. Every one of the 182 reviewed
grammar findings and its matched-use evidence remains identical; only input
fingerprints change. The parser implementation fingerprints remain recorded.

Upstream reports: [Ambergris #234](https://github.com/i5jb/mtgish/issues/234) and
[Basalt Golem #235](https://github.com/i5jb/mtgish/issues/235). Source corrections
are committed as 20f1624 on fork main.

## Shared lowering correction

Conjurer's Closet and Restoration Angel exiled one permanent but selected only
index zero of its actual After receipt for return. A melded permanent becomes
two cards, and both must be returned. Cloudshift shared the immediate-exile form
of the defect; Brokers' Safeguard also needs its perpetual grant applied to both
cards before returning both.

Lowering now proves that an exile had one input object and retains the complete
post-movement group for the followup. Optional paid exiles use the complete
CostObjects Performed/After component, rather than Query index zero. Exile-zone
guards still reject diverted moves and do not chase a later incarnation.
Explicit first/second/chosen-card queries and unproven multiple-input cases retain
their scalar interpretation. The new proof names its ResultId, so a later loop
or joined receipt cannot inherit a different movement's single-input proof.

No Source or normalized enum, schema version, numeric range, Cantrip engine
feature or generator implementation changes. Existing collection-capable code
generation consumes the corrected AST. The Source model is unchanged and the normalized schema remains 179.

The earlier full Brokers' Safeguard fixture now expects the reviewed group for
both grant and return. Two historical full-card snapshots are retained unchanged;
their test harnesses adapt only one asserted exile-followup operand in Jugan's
Saga face and Stenn. All remaining operands remain fully compared.

Rules: [CR 121.3, 608.2d and 712.21c](https://media.wizards.com/2026/downloads/MagicCompRules%2020260619.pdf).
The meld rule specifies finding both constituent cards and taking the same action
on each when an effect finds the original object after a zone change.

## Validation

Six complete Source/native pairs match independently reviewed expected programs,
validate, and remain canonically idempotent. Citadel's full AST includes Specialize;
its unrelated backend gap is not hidden or implemented for this review.

The native gate executes generated Progress attack, Basalt block/delayed, and
three complete blink implementations against the existing a2aabdfa6 engine.
Progress and Basalt use explicit callback fixtures, so those witnesses test their
resolution and delayed behavior rather than full attack/block dispatch. Blink
cases exercise actual casting, ETB and end-step dispatch, single-card and melded
creatures, and both optional choices. Meld fixtures use the engine's existing
merged-object representation with two physical cards. No engine checkout is edited.

Full current Source normalization: 33,529/33,529, no unsupported forms or primitive
dependencies. The uncached full Go suite passes. The full AST suite passes 1,328 tests
with four opt-in gates ignored. The explicit native gate passes all 19 runtime
tests, including Basalt's owner/controller distinction after sacrifice and no
Wall when the blocker leaves before the delayed sacrifice.

Evidence: `/mnt/vizier/oracle-joint-blink-20261107/`.
