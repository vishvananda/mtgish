# Native untap intervals and landwalk metadata

The combined local native checkpoint is
`01adc773bd2435046d08de1b5b698620eded28a4`, based on the previously validated
`aa0652bb3571812e7a399efc2f8bd7a0e9f25bf6`. It includes simultaneous untapping,
distinct untap events, uninterrupted source-tapped durations, native landwalk
variants, and a distinct Carrion counter kind. The workers' separate commits
were reviewed and combined before promotion; later assembled cards are excluded
from the worker snapshot.

The core uses event history to distinguish an unbroken source-tapped interval
from untapping and retapping before resolution. It handles the final tap cost,
copied abilities, control loss, phasing, a condition restarting during resolution,
and nested immediate mana abilities. Generic callback durations and other
conditional duration families retain their previous beginning semantics.

Landwalk grants retain exact keyword identity. References and removals include
the applicable snow variant; blocking still requires the right land type and
supertype. All five snow basic-landwalk variants, snow landwalk and desertwalk
have native metadata and host labels. Existing keyword IDs are preserved.
Carrion is appended as counter ID 76, distinct from Corpse.

The generator now uses those authorities: typed source-tapped durations,
`untap_together` with actual `Untapped` results, native landwalk keywords,
text-adjusted named-landwalk filters, and the complete landwalk family mask.
It also consumes ignored turbofish collections explicitly and uses the last
validated as-enters choice as its exhaustive match arm. Those two changes fix
Knowledge Vault's warning gate and Primal Clay's unreachable empty coverage arm.
The gates themselves are unchanged.

Validation:

- Combined native workspace: **17,242 passed**, 137 targets, 15 existing ignored;
  all **15,738 scenarios** passed.
- Generator workspace: **1,004 passed**, 99 targets, two existing ignored.
- All **84 exact stopped-card modules** compile against the combined native API
  with `-D unused_must_use`, with no exclusions or compatibility aliases.
- The new callback/verb, copy, duration, landwalk, warning and entry-choice
  regressions have recorded failing baselines. The native workers also retained
  forced runtime failures and negative controls.
- Native API/mechanics fingerprint:
  `ea3dd20df51ee60b3288cb5227df69cb912ff0e2a306a4ee459505548dff67e8`.

The event mask and keyword mask are now 128 bits. The encoder schema is 2, raw
observation schema 3, and event vocabulary 67: old category IDs and unknown 65
are preserved, with Untapped appended as category 66. The deterministic tiny
model migration preserves all 48 original inference references and historical
training metadata. The existing 48-bit keyword tensor remains unchanged; its
previous omission of later keywords is not expanded by this work.

Both before/after binaries completed 128-game benchmarks with verification
unset. The first pair measured 33.35/40.75 microseconds per engine decision;
the reverse-order pair pinned to one CPU measured 38.47/37.62. These varying
results do not establish a speed improvement or a stable regression.

Proofs and promotion receipt are in
`~/card-lane/pipeline-unlimited-20261103/integration/untap-landwalk-combined-proof/`.
Generator captures and compilation evidence are in
`/mnt/vizier/card-lane/generator-native-features-preflight/validation.json`.
Native publication remains local pending the remaining blocked work and final
campaign mechanics consolidation/export. The planned PR shape remains one
shared foundation with independent groups of approximately 20 cards.
