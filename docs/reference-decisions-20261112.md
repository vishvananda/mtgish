## November 12 consistency follow-up

Four Source-template families are now fixed across 30 cached records; 32 of the
reopened faces remain pending. See [the current family worklist](reference-consistency-20261112.md)
for current decisions and final validation rather than the historical gameplay-only
classifications below.

# Decisions for the 53 Oracle Grammar mismatches

**Consistency review reopened:** the gameplay-equivalence decisions below are historical.
They do not waive alternate producer syntax. All 36 retained equivalent/contract
cases are being audited under [the consistency follow-up](reference-consistency-20261112.md)
and [upstream issue 275](https://github.com/i5jb/mtgish/issues/275). Satya remains
a separate rejection of a scalar proposal with a token-multiplication counterexample.

The foundation goal is paused. This pass reviews the exact 53 differing faces
from Oracle Grammar `1fe5782`, reference/model pin `7228b84`, schema 190. The
allowed population contains 32,755 faces: 32,702 equal, 53 different, no refusals.
Those are disagreements, not 53 established reference defects. The
[per-card ledger](reference-decisions-20261112.json) covers all 53 with decisions
and evidence. The [final-nine follow-up](reference-nine-resolutions-20261112.md)
closes the questions left open in the initial batch; no Oracle Grammar gold, grammar, pins or worker files were changed.

## Correctness changes

| Cards | Correction |
| --- | --- |
| Collective Voyage, Minds Aglow, Shared Trauma | Read the sum of every player's mana payment with `PerformedTotal`. Alliance of Arms already does this. |
| Leela, Sevateem Warrior; Xyris, the Writhing Storm | Trigger on all opponent draws except the first in each of that opponent's own draw steps. Draws outside draw steps must trigger. |
| Paths of Tuinvale; Veko, Death's Doorkeeper | Ordinary returns record a complete After Transition for perpetual changes, including every card resulting from returning one melded permanent. |
| Veko | “Becomes a Spirit” sets creature types; it does not retain Zombie or other prior creature types. |
| Portal to Phyrexia | “It's a Phyrexian” applies as the nontoken creature enters, rather than afterward. |
| Omen Machine | A card's characteristics after exile come from After. Keep Before for the distinct permanent-before-exile predicate. |
| Know Naught but Fire | Explicitly bind each damage recipient to their own hand count. The scheme damage fast path bypassed the existing per-recipient lowering. |
| Lamplight Phoenix | Return the exact card produced by its own exile cost: Resolution(0), component [0], Performed, After. Keep the collect-evidence component and payment guard separate. |

These fix meanings on **11 of the 53 faces**. Lamplight is a Source binding
clarification using the existing `TheExiledDeadPermanent` constructor; the
returned card is neither the generic Source nor the old graveyard incarnation.
The independent proposal's graveyard query is also unsuitable. Omen retains a
full-transition type count rather than adopting its alternative scalar spelling.

The entry correction also applies to seven matching records outside the 53:
Ascent of the Worthy; Ashiok, Nightmare Weaver; Chainer, Dementia Master; Grave
Betrayal; Liliana, Death's Majesty; Lorcan, Warlock Collector; Sorin, Vengeful
Bloodlord. Eight templates preserve their existing controllers, entry counters,
colors and delayed-return timing while adding the stated characteristics to the
entry flags. [CR 611.2e and 614.12](https://media.wizards.com/2026/downloads/MagicCompRules%2020261106.pdf)
distinguish “is/has” on entry from later “becomes/gains”. Metallic Mimic naming
Phyrexian supplies a witness for Portal: the entering creature receives its
additional counter only if it already has that creature type.

The return tracking also repairs Candela's ordinary-return receipt. The generic
card-after-exile correction changes other fresh producers, including Nyla and
Negative Zone Portal; their old persisted snapshots and migration origin maps
remain unchanged. Their tests explicitly distinguish preserved saved Before
meaning from newly corrected After output. No migration silently fixes a saved
card's meaning.

No AST fields, schema version, quantity domains, engine APIs or engine features
change. The additional lowering uses existing typed operations and result IDs;
there is no larger runtime representation or new allocation-bearing AST field.

## Equivalent outputs consolidated

Five faces receive producer normal forms, not additional gameplay bug claims:

- Old Man Willow uses one optional sacrifice over “another creature or token”,
  followed by one reflexive trigger. The previous choice of sacrifice branches
  also sacrificed at most one permanent; it did not sacrifice both.
- Jace, Cunning Castaway's Illusion self-target trigger and Tale of Katara and
  Toph's granted self-tap trigger use EventSubject for “it”. This applies only
  to these holder-own events, without globally merging Source and EventSubject.

The final-nine follow-up also consolidates Goblin Snowman to one compound
prevention and Ward of Piety to an explicit damage-quantity budget.

The full-program regression compares all 53 outputs to reviewed expected
programs, preserving the other 37. Alongside the 11 correctness updates, this
makes **16 changed faces** in the original packet. Only **14** now exactly match
the independent proposal: Omen and Lamplight still have intentionally different
corrected representations. A 53-to-39 exact-difference count is therefore not a
claim that 39 reference bugs remain, nor a fresh full-corpus score.

## What remains

The final ledger retains **37** other outputs: 16 preserve valid cost, identity,
keyword or receipt contracts, 20 are equivalent or redundant forms, and Satya's
aggregate is retained because the proposed scalar fails token multiplication.
Source and EventSubject stay distinct outside a proven self frame; resolving
payments keep their guards; last-known receipts do not become live-zone reads.

All nine questions from the initial batch are now dispositioned in the
[follow-up](reference-nine-resolutions-20261112.md): seven retained outputs and
two producer normal forms. This closes the nine investigation questions; it does
not assert that nine gameplay bugs were fixed. No engine feature was needed.

## Upstream coordination

- Join-forces totals already have [issue #139](https://github.com/i5jb/mtgish/issues/139);
  do not create a duplicate.
- Draw exceptions: [#270](https://github.com/i5jb/mtgish/issues/270).
- Veko Set versus Add: [#271](https://github.com/i5jb/mtgish/issues/271).
- Nontoken entry characteristics: [#272](https://github.com/i5jb/mtgish/issues/272).
- Generic next-N redirection template: [#273](https://github.com/i5jb/mtgish/issues/273).
- Ordinary-return tracking, card After tests and per-recipient scheme damage
  are our Rust lowering fixes, not additional upstream Source bug reports.

The upstream grammar was inspected read-only for the reported templates. No
upstream git fetch, merge or update was pulled into the fork.

### Issue #106: linked players need a separate contract

The [maintainer's comment](https://github.com/i5jb/mtgish/issues/106#issuecomment-6814335356)
replaces a single remembered player with an accumulating player list and retains
the printed 6/3 gain. Accumulation is a real concern, but the suggested bag
multiplicity is explicitly tentative. Our single `NoteKind::Player` plus
`PriorPlayer::Remembered` cannot faithfully implement that list; a deduplicating
`PlayerSet::Union` cannot faithfully stand in for a bag either.

There is a second, independently established timing case. The official
2018-03-16 ruling, preserved in the
[Scryfall ruling feed](https://api.scryfall.com/cards/0e7ed355-894a-4592-9a64-ef061550d560/rulings),
allows the leave-trigger's gain to use the entry-trigger's current target while
the entry-trigger is still pending. A note action run only on entry-trigger
resolution is too late. Fixed gain also must not depend on life actually lost.

A [2016 discussion](https://www.mtgsalvation.com/forums/magic-fundamentals/magic-rulings/761121-panharmonicon-questions-i-havent-seen-asked)
reports a Wizards customer-service answer giving one gain when the same player
was targeted twice, contrary to bag iteration. That is secondary evidence, not
a primary ruling settling the corner case. CR 607.3 discusses multiple exiled
objects and does not directly settle repeated linked players. The
[upstream follow-up](https://github.com/i5jb/mtgish/issues/106#issuecomment-6814939068)
asks for explicit pending-target and multiplicity semantics. We retain the fixed
6/3 gain and mark the single-player model incomplete for repeated entry triggers;
we do not claim that list semantics have been implemented.

A subsequent linked-ability change must specify: ability-pair identity, source
incarnation, announcement-time targets and retargeting, pending/resolved/countered
entry triggers, duplicate-player handling and separate fixed amounts. Test two
different targets, the same target twice, leave-before-entry-resolution,
countered entries, retargeting and prevention/locked life before selecting a wire
representation. Do not generalize “players who lost life”: it would incorrectly
make the linked information depend on successful life loss.

## Adoption and validation

Use the fix commit in this document's git history as the next reference producer.
Refresh only `adapters/mtgish/reference.json` and its normalized reference inputs,
then re-prepare/recount. This batch keeps the existing schema-190 model contract,
so a reference update alone does not require changing `pins/model.json` or
rebuilding the independent grammar model. Do not edit reference gold or add
comparison aliases.

The checked-in packet has 66 Source records and 68 faces, including named
controls, Run accompanying Hit, and Green Goblin accompanying Norman Osborn.
The 20 changed Source records are compared against the
English parser and tagged JSON. The immutable original 53 and corpus provenance
are recorded separately in the ledger and local evidence directory. Validation
results and checksums are in [the proof receipt](reference-decisions-20261112.proof.json).
