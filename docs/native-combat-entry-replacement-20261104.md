# Common native checkpoint for departed sources and entry actors

Validated worker checkpoint `31abfac8662681d8f9a204e3552e2e68af63efa9`
combines four independently reviewed local changes above `01adc773`:

| Commit | Behavior | Waiting callers |
| --- | --- | --- |
| `3b979791` | Directional combat relationships retain exact-incarnation last-known information, using the existing journalled member arena. Current live relationships remain current. | Abu Ja'far, Tinder Wall, Ydwen Efreet |
| `70b4e94a` | Battlefield zone-change events record the player performing the put separately from entering control. Explicit-player operations, token creation, replacements, and automatic returns preserve that provenance. | Thelon's Chant, Tourach's Chant |
| `6218bd72` | Replacement consumption, shields and healing stay inside their core instruction while replacement callbacks retain their own instruction checks. Unannounced caller/callback mutations still fail verification. | Eye for an Eye, Pyramids |
| `c8346d66` | The exact printed player-damage poison ability is shared; Serpent Generator's Snake uses it. | Pit Scorpion, Serpent Generator |

The only merge conflict was adjacent test-module declarations; both modules
were retained. The merged callbacks preserve both entry provenance and
source-specific instruction checks. `Entered` remains record-only. No event
identifier, observation layout, model input or client protocol changes here.

The generator now reads native directional relationships in ordinary query
filters, including target revalidation after sacrificing the source. Layer
filters retain their existing live-state path. A player's put event reads the
recorded `put_by`; an ordinary controller-entry event still reads entering
control. The vocabulary detects each capability independently. Its current
API/mechanics fingerprint is
`ed84a7b794c8b9352a6c488fc970af6436c00cd43ad24f5eb734e3a1fe5da3e2`.

Validation of the exact combined commit:

- Verified checked workspace: **17,287 passed**, 138 targets, 15 existing ignored.
- Complete .NET solution: zero build warnings/errors; **1,216 passed**, one existing skip.
- Release benchmark: matched 128-game runs, CPU 0, verification removed from the
  environment, both with zero unfinished games. Engine time measured
  42.81 → 34.85 microseconds/decision. Prior paired timings varied; no speedup
  claim is made.
- Generator: **1,012 passed**, 101 targets, two existing ignored. Its original
  two authority assertions failed before the wiring while the controller-entry
  control passed. Lesser Werewolf's P/T-counter feature test now holds combat
  support constant in both configurations; all counter assertions remain.
- All **84** retained implementation-stop records regenerate without holes and
  compile as complete native modules with `-D unused_must_use`, with no exclusions.

Proofs are under
`~/card-lane/pipeline-unlimited-20261103/integration/combat-entry-replacement-poison-proof/`
and `/mnt/vizier/card-lane/generator-combat-entry-preflight-20261104/`.
The promotion receipt pins the tested commit, rather than the moving collector.
Native commits remain local during campaign consolidation.

Ydwen Efreet also has a separate producer error: its final unblocking sentence
is unconditional, whereas the original source places a combined removal and
unblocking action inside the lost-flip branch. This is reported as
[mtgish issue 191](https://github.com/i5jb/mtgish/issues/191).
The retained reviewed card has already corrected this sentence placement;
finishing it must preserve that correction and use the new relationship query.
The uncorrected raw source is retained as evidence, not presented as a finished
generated implementation of Ydwen Efreet.
