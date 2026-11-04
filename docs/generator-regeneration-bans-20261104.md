# Regeneration prohibition checkpoint

The local Cantrip feature `5ed784375ecf45e6d518bec0f4f081c9d62ed142` adds regeneration prohibitions to
resolved effects, printed and emblem static rules, and granted static abilities. It is merged into the campaign's
local integration branch and pinned as the exact tested worker checkpoint. Cantrip publication remains deferred.

The generator enables this lowering only when its vocabulary contains `regeneration-bans`. Older target snapshots
retain their explicit capability gaps. A granted prohibition uses `granted_regeneration_ban`, preserving ability
identity, removal, and the granted ability's printed text and static-rule citation. Ordinary duration effects use
`EffectKind::CantBeRegenerated`.

This makes seven cards ready for normal implementation and review: Bone Shaman, Disintegrate, Gravebind, Hurr Jackal,
Incinerate, Lim-Dûl's Cohort, and Orcish Healer. Disintegrate already has a reservation; the other six use canonical
intake. Clergy of the Holy Nimbus still needs activation by other players. Whippoorwill still needs its prohibition
on damage prevention and redirection. Those independent gaps remain explicit and do not enter new intake.

The engine also corrects direct static regeneration: it heals, taps, and removes the creature from combat without
requiring or consuming an unrelated regeneration shield. Prohibition is checked when destruction replacements are
proposed, including simultaneous destruction of the prohibition's source. Creating a shield remains legal.

Evidence under `~/card-lane/pipeline-unlimited-20261103/integration/`:

- `regeneration-proof/validation.json`: full checked Rust workspace, 17,127 passing tests, with log digest.
- Fourteen focused native regressions cover resolved, printed, emblem, and granted prohibitions; ability loss and
  regrant; simultaneous moves; ordinary regeneration; direct static regeneration; shield counters; and lethal or
  deathtouch state-based destruction.
- `regeneration-proof/negative-controls.json`: disabling prohibitions causes nine behavioral failures; forcing
  direct regeneration to consume a shield causes two. Restoring exact source bytes restores all fourteen passes.
- `regeneration-proof/benchmarks.json`: verification disabled, standard 128-game runs. The first paired sample is
  63.78 to 64.02 microseconds per decision with identical allocation; a second pair varies with host load, so it does
  not establish a speedup.
- `regeneration-generator-final.log`: all 32 tests across seven relevant generator suites pass, including nine full
  source/Oracle fixtures with both older and newer target capabilities and granted-static citation checks.
- `regeneration-recovery/probe-modules.json` and `module-probe.log`: all seven freshly generated whole modules
  compile together against the exact native feature commit. These are validation-only modules; card scenarios,
  coverage, review, and verification remain the card pipeline's responsibility.

The two remaining gaps are not waived, and neither compilation nor intake constitutes card approval.
