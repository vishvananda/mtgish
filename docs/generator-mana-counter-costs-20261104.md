# Variable counter costs in mana abilities

The local Cantrip foundation now announces and pays variable counter-removal
costs for mana abilities. The generator targets tested local checkpoint
`def1558de5a5ebd33e4e85b51d3d6abfb22406ea`, with API fingerprint
`d2b7b69f6afb81feb4d77f3a843726cd8ce0a907656e9c688a5bc00b5c018240`.
Cantrip changes remain local; publication is disabled during consolidation.

## Cost and value contract

The generator uses `VariableCost::RemoveAnyNumberOfCounters` and reads
`Query::mana_ability_x`. It does not overwrite or read the source permanent's
independent casting X. The paid-counter read must identify the activation's
one variable component, its exact index, a named counter kind, and removal
from the source by its controller. Casting, resolution, branch, fixed-removal,
nested-component and duplicate-variable references do not acquire this shortcut.
Old targets retain the explicit gap.

The generator emits `x_is_mana_amount` only when the body proves that positive X
changes mana amounts rather than possible mana types: direct additions with
nonnegative constants, X, the matching paid-counter read, addition,
multiplication and the corresponding production lists/repeats. Conditional
bodies and other unproved forms remain held. The engine's production query
ignores whether the costs are payable, including a storage land with zero
counters, and includes the multiple-mana outcome needed for Damping Sphere.

An additional bug surfaced while testing the newly supported modern forms:
combination-mana generation capped the amount at 255 because one color question
has an eight-bit count. Large amounts now keep their unsigned 32-bit amount and
ask successive questions of at most 255 colors. Small statically bounded
productions retain their previous code. The handwritten Wizard's Rockets caller
was migrated in the native checkpoint and its previous truncation fixed too.

## Native capability correction

The broad `lasting-rules` switch previously treated every proposed constant
`LastingRuleKind` as implemented. The pinned engine supplies only `CantBlock`,
`MustAttack` and `DoesntUntap`. The vocabulary tool now snapshots these inert
variant names; absent kinds keep both their proposed table and operation in
explicit holes. Targets predating the enum snapshot an empty list.

This corrects thirteen false whole-file capabilities, including Festival's
missing `CantAttack`, and the proposed `CantBeBlocked` and `MustBlock` variants.
Festival remains unreserved. This pass does not substitute a closed current set
of creatures for an open-set rule that must also reach later arrivals.

## Validation and recovery scope

- **17,146 native Rust tests** pass across 134 targets, with 15 existing ignored
  tests. Thirteen focused engine tests cover payment, cancellation, bounded X,
  repeated/nested activations, independent casting X, sacrifice/exile, combined
  costs and production queries. The printed Wizard's Rockets regression produces
  all 256 mana across two color questions.
- The standard 128-game benchmark ran with `CANTRIP_VERIFY` unset. A consecutive
  before/after pair measured **38.49 / 38.91 microseconds per engine decision**,
  about 1.1% apart. Earlier unpaired measurements were load-sensitive; no speedup
  is inferred from them.
- **954 generator/normalizer tests** pass across 91 targets, with two existing
  ignored tests. Both old-target gaps and new-target generation are exercised.
- All **33,529** ordered full-corpus output identities are preserved. There are
  **25 newly whole cards**, thirteen correctly held false positives, and twelve
  other complete-module changes from the uncapped combination-mana emission or
  the new activation-X reader.
- **31 complete generated modules** compile in an independent native probe and
  exactly match the final full-corpus output. The unchanged printed 256-mana
  regression also passes against the actual generated Wizard's Rockets module.
  Named-counter/token prerequisites were supplied through the normal tooling.
- Eleven unchanged worker scenarios pass for Touch of Vitae, Thelon's Curse,
  Thallid Devourer, Wall of Pine Needles and Vodalian Knights. Their reported
  citation, regeneration-helper and battle-defender bugs were already corrected
  by newer generator code; they can receive fresh normal lane attempts.

The through-Ice-Age recovery is the five batteries (Black, Blue, Green, Red and
White Mana Battery) and Bottomless Vault, Dwarven Hold, Hollow Trees, Icatian Store
and Sand Silos. A fresh source-name audit also found twelve unreserved whole
cards using earlier tested support: Arcum's Whistle, Barl's Cage, Celestial Sword,
Deep Spawn, Dread Wight, Elvish Hunter, Homarid Warrior, Ice Floe, Mole Worms,
Norritt, Phyrexian Gremlins and Spore Cloud. Readiness is not card approval;
implementation, review and verification remain with the normal Terra pipeline.

Proofs/logs are retained under
`~/card-lane/pipeline-unlimited-20261103/integration/mana-counter-{proof,recovery}`
and `integration/current-scope-audit-20261104`. The latter records and rejects
an initial audit that incorrectly treated refreshed subset-relative cache line
numbers as full-corpus positions. The corrected audit matches all 1,320 requested
source names exactly and verifies every generated identity. No queue changes
used the rejected audit.

Semantic schema remains 174. No source corpus or immutable reference was edited.
.NET suites were not run in this environment.
