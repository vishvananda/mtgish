# Early-set counter, target, and citation fixes

The validated local Cantrip counter feature is `8b64d5720fc89d01a56846c94f9bebb17e2529b3`. Its exact tested ancestor
is the worker checkpoint, after merging into local integration. No Cantrip changes were pushed during consolidation.

`counter_kinds!` now accepts signed power/toughness metadata. Existing ids remain unchanged; the other printed P/T
counter kinds are appended. Layer 7c adds each kind's own deltas, with wide intermediate arithmetic before applying
the stored stat width. Only the +1/+1 and -1/-1 pair annihilates. Other opposite kinds remain available for costs and
proliferate. A private current-state flag maintained by the counter put/remove/clear paths makes the empty-counter
guard constant-time. It is restored with the object's journal entries.

This removes the generator gap for Armor Thrull, Balduvian Hydra, Clockwork Avian, Dwarven Armorer, Dwarven Armory,
Ebon Praetor, Essence Flare, Lesser Werewolf, Living Armor, and Spirit Shackle. The generator requires both the native
`pt-counters` capability and the exact kind in its vocabulary. Unsupported pairs remain explicit holes.

Two target-count branches used `cantrip_mechanics::ANY_NUMBER` as a TargetSpec maximum. That value means COUNTED to
TargetSpec, which made five one-or-more-creature spells behave as if their maximum were their unannounced X, zero.
They now use `TargetSpec::ANY_NUMBER`, preserving the minimum of one. The affected cards are Heaven's Gate, Dwarven
Song, Sea Kings' Blessing, Sylvan Paradise, and Touch of Darkness. An X-counted target regression remains COUNTED.
Draft scenarios that supplied variable targets inline with Cast were corrected to answer the separate Target prompt;
two colorless cleanup expectations now explicitly check absence of the color field and presence of the creature.
The same corrected cases pass with fresh output, fail behaviorally with the original generated modules, and pass
again after restoration.

Entry citations now distinguish CR 614.1d's enters-tapped effect from CR 614.1c's enters-with/as effects. The pass
inspects assignments to the actual typed Entering parameter: Dwarven Ruins and Ebon Stronghold cite 614.1d, Peat Bog's
combined entry cites both, and Dark Depths' counters cite 614.1c alone. Eleven citation-recovery cards pass their
unchanged draft scenarios. Eater of the Dead compiles with its citation corrected, but its draft contains only a TODO
scenario; its ordinary implementation worker still needs to complete behavioral validation.

Evidence under `~/card-lane/pipeline-unlimited-20261103/integration/`:

- `pt-counter-workspace-cached.log`: 17,113 checked Rust tests pass, including ten focused P/T-counter regressions.
- `pt-counter-negative-control.json`: removing counter deltas makes the behavior regression fail.
- `pt-counter-module-probe.json`: ten fresh whole generated modules compile against the exact feature sources.
- `pt-counter-benchmark-comparison.json`: the initial implementation cost about 10% in repeated measurements.
  After the presence cache, the three measurements are 37.63, 38.41, and 38.94 microseconds per engine decision;
  baseline measurements are approximately 40. No measured slowdown remains. Both stages are retained.
- `pt-counter-citation-probe/runtime.json`: actual citation-recovery scenario outcomes, including the TODO failure.
- `any-target-recovery/`: corrected fixtures and original/fresh/restored behavior evidence for all five spells.
- `/mnt/vizier/early-set-generator-final-tests.log`: 73 related generator regressions pass.
- `/mnt/vizier/pt-counter-generator-control.log`: an unknown P/T pair remains held.
- `/mnt/vizier/pt-counter-pipeline-tests.log`: 279 pipeline tests pass. Appending ordinary named counters preserves
  P/T metadata; associated metadata constants are not mistaken for counter kinds. Driver reloads include the shared
  counter/token preparation helper and happen at clean card boundaries.

The probes are validation artifacts. Card implementation, full card gates, and independent Terra review remain
required before a card is accepted for final publication.
