# Random returns and created damage triggers

Follow-up to the other worker's `upstream-other-targets.md` note. The earlier
[other-target corrections](reference-other-targets-20261112.md) already resolve
the Jaheira/Kinetic Ooze antecedent question and the operation-dependent optional
target split, including Amy Rose and every movement destination. They remain in
effect. Scalar identity fields and required payment cardinality stay explicit.

## Random selections

Both random graveyard-return Source templates now produce one resolution-time
`WithSelections` preparation, with `chooser: Chance`, `AsManyAsCan(1)`, and a
move of its prepared selection. There is no separate target or player-choice
receipt. The six previously exact-count faces are Charmbreaker Devils, Exalted
Flamer of Tzeentch, Ghoulraiser, Nephalia, Viconia, Drow Apostate, and Woodland
Sleuth. Haunted Fengraf and Surreal Memoir are unchanged controls. Make a Wish
and Reap the Past keep their larger or variable as-many-as-possible counts.

The same random single-card effect helper also corrects Chronicler of Worship's
selection from the top seven cards. It keeps the top-seven boundary and Shrine
filter; an empty matching pool supplies no card. This is the resolving-effect
contract in CR 609.3, not optional selection when a matching card exists.

For a chance preparation with a scoped zone pool, a repeated identical zone and
player filter is redundant. Fossil Find, The Deck of Many Things, and Trash Bin
now use `Any` within their already scoped own-graveyard pool, matching Nephalia
and the plural forms. The typed simplification requires a proven identical scope;
it preserves a filter for a different player. Other qualifications, quantities,
movement destinations, and subsequent instructions are unchanged.

## Phage the Untouchable Avatar

The second activated ability's created trigger matches the targeted creature as
the **damage source** and a creature as the **recipient**. Its destruction
now names `Damage(Recipient)`. The previous output incorrectly named the captured
target, destroying the attacking creature instead.

The Source record is correct under its existing role convention:
`WhenACreatureDealsCombatDamageToAPermanent(..., ...)` uses
`Trigger_ThatCreature` for the source and `Trigger_ThatPermanent` for the recipient.
Printed triggers already installed that distinction. Created triggers installed
the event but omitted its legacy role bindings; subsequent captured-subject
normalization therefore rewrote the incorrect generic subject into the target.
Printed and created triggers now share the role-binding setup. A created trigger
also resets damage roles inherited from its creator.

The Avatar's first ability still destroys the creature that deals combat damage to its
controller. The ordinary Phage card still destroys the damaged creature.
Delif's Cube/Cone and the other captured-target controls retain their proven
target identity. No blanket `EventSubject`/`Target` alias is introduced.

## Scope and validation

These are Rust production/lowering corrections. No additional Go Source defect
or parser change was found, so a new upstream parser bug ticket is unwarranted.
The upstream consistency thread receives the decision and implementation link.
Oracle Grammar's worktree, gold, reference pin, and scorer remain untouched.

Portable schema 190 and the AST's fields and variants are unchanged. The new
scope checks traverse typed values without serialization, card cloning, or new
runtime representation fields. Saved-card migration is unchanged.

The before/after corpus comparison changes exactly eleven of 33,529 records:
the six random-return counts, Chronicler's count, three redundant pool filters,
and Phage's recipient. The other 33,518 records are byte-identical in the unified
RON export. All 33,529 records lower, validate, and round-trip without unreduced
forms or primitive dependencies. The final full Rust workspace passes **2,072
tests**, with 24 ignored, across 201 targets. The
[proof receipt](reference-random-damage-20261112.proof.json) records the hashes
and exact corpus scope.

None of the eleven changed faces belongs to the frozen 286-card early-set
foundation scope. No Cantrip backend or engine feature is changed in this pass.

Eight focused regressions include a complete 23-record Source packet, strict/unified agreement
for ordinary cards, serialization round trips, equivalent return templates,
different-player pool filters, both Phage abilities, distinct damage participants,
and a created trigger that must not inherit its creator's damage role.
Evidence is in `/mnt/vizier/reference-random-damage-20261112`.
