# Shared damage-to-player poison ability

Pit Scorpion and Serpent Generator's Snake have the same printed triggered
ability: damage of either kind to a player gives that player a fixed poison
count. The native shared recipe is distinct from poisonous, which triggers on
combat damage only.

Native commit `c8346d66df9889e0b2a079f8420a77559f675bdc`, based on immutable
`01adc773bd2435046d08de1b5b698620eded28a4`, adds
`cantrip_mechanics::poison::on_damage_to_player::<N>` and migrates the existing
Serpent Generator caller. The matcher admits actual positive player damage by
the same source; the resolver gives the damaged player the printed constant,
using native poison-counter authority. Its captured subject is the player.

The generator matches the complete typed event and counter instruction together.
Different damage domains, sources, grouping, subjects, recipients, dynamic
amounts, additional instructions, targeting, limits, intervening conditions,
conditional guards, or explicit functioning scopes retain their existing path.
Replacing only the matcher would be incorrect because the previous generated
resolver uses different event bindings.

The `damage-poison` feature is detected from the pinned native vocabulary. It
was disabled on the `ea3dd20d` snapshot. The reviewed helper is now promoted in
`31abfac8` and enabled by the refreshed `ed84a7b7` vocabulary. Both original whole source
records are retained as fixtures; no Oracle or reference correction is involved.

Validation:

- Before the recipe, the new sharing assertion failed; two controls passed.
- Afterward, four focused generator tests and the full suite pass: **1,009 tests
  across 100 targets**, with two existing ignored tests.
- Five native runtime cases cover combat, noncombat, a different printed count,
  zero damage, another source, and the damaged player rather than the controller.
  Mutating either the count or recipient causes three real runtime failures;
  restoring the source passes all five cases.
- The required verified native workspace passes **17,247 tests across 138
  targets**, with 15 existing ignored tests, including the retained scenarios.
- An independent read-only review by the combat-feature agent found no blocker.

Logs, exact source hashes and receipts are in
`~/card-lane/pipeline-unlimited-20261103/integration/poison-damage-proof/`.
Native work remains local during campaign consolidation. The semantic schema
remains 176.
