# Combat feature checkpoint

Cantrip's local `engine/blocked-transitions` feature commit, `ba3b4fd57`, supplies the shared engine and keyword
operations behind eleven held cards through Ice Age. It remains local during campaign consolidation.

The generator enables the following capabilities only when present in its pinned vocabulary snapshot:

| Capability | Evidence in the target source | Generated operation |
| --- | --- | --- |
| `rampage` | Public mechanics module | `rampage::trigger::<N>(text)` |
| `combat-removal` | Shared historical unblocking recipe | Core removal or `combat::remove_from_combat_and_unblock` |
| `blocked-status` | Core `become_blocked` operation | Core status change and `Event::BecameBlocked` matcher |

The metadata updater searches the API, state, and mechanics inputs it fingerprints. Older snapshots retain the
planned holes and the existing batch-based declared-block matcher. New snapshots use the actual blocked transition,
which also covers an effect making a creature blocked without a blocking creature. A trigger for becoming blocked
**by a creature** still reads each `Blocks` relationship. Seventeen complete local source/Oracle fixtures cover both
capability configurations and the Karn/Cockatrice distinction.

The historical unblocking recipe is shared by False Orders and Imprison. Another blocker leaving
earlier does not qualify an attacker that had become blocked by two creatures. The recipe asks this combat's event
history, removes the blocker, ends that instruction, and unblocks qualifying attackers together.

Ready for ordinary card implementation and review:

- Aerathi Berserker, Chromium, Craw Giant, Frost Giant, Hunding Gjornersen, Marhault Elsdragon, Wolverine Pack.
- Disharmony, Imprison, Mijae Djinn.

Ydwen Efreet additionally needs the departed-source relationship query now in native checkpoint `31abfac8`,
and its retained reviewed card fixes the unconditional final sentence. The raw producer incorrectly places
combined removal/unblocking inside the lost-flip branch; see [issue 191](https://github.com/i5jb/mtgish/issues/191).

Rapid Fire still needs the `HasKeywordFamily` filter. False Orders still needs combat reassignment. Their remaining
holes stay explicit, and their new-card intake stays deferred.

Validation evidence lives under `~/card-lane/pipeline-unlimited-20261103/integration/`:

- `blocked-transitions-workspace.log`: full checked Rust workspace, 17,103 passing tests.
- Nine focused native regressions include banding, independent rampage instances, resolution-time blocker counts,
  removing blockers through real activated abilities, historical unblocking, and search-tree undo/redo.
- `blocked-transitions-negative-control.log`: disabling the transition produces the expected behavioral failure.
- `blocked-transitions-benchmark-standard.log`: 128-game benchmark, 39.09 microseconds per engine decision versus
  40.48 before, with identical allocation per decision. Timing varies with other workers. The earlier post-feature
  benchmark inherited `CANTRIP_VERIFY=1` from the workspace check and is retained separately; it is not comparable
  to the standard baseline.
- `/mnt/vizier/combat-transition-generator-tests.log`: 40 related generator tests pass with the refreshed snapshot.
- `/mnt/vizier/integration-feature-pipeline-tests.log`: 277 pipeline tests pass, including clean tested-ancestor
  promotion and refusal of dirty, unmerged, or mismatched checkpoints.

Workers consume the exact clean tested feature checkpoint after its history is merged into integration. Card imports
that arrived later remain outside that checkpoint until separately validated. Final publication still puts shared
engine/mechanics work in the foundation before independent card groups.
