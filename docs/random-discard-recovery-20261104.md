# Recorded random discards

The shared `discard_at_random` recipe shuffled a hand through `Game::random`, bypassing the existing
`Game::choose_at_random` API. Fixed scenario choices did not select the requested card, and the history contained
no `ChosenAtRandom` records. This was a shared helper defect, rather than a missing core primitive or a card-worker
coding mistake.

Local Cantrip commit `149f230a575ae196f8e241fe517790dab0982e0c` selects uniformly without replacement through the
recorded-choice API. It selects only the needed subset and then discards the cards together. Discarding nothing or
the whole hand makes no random choice. Hymn to Tourach, Mind Twist, and Hypnotic Specter already use this recipe;
Gwendlyn Di Corci uses the same operation. Random-discard costs still require their separate cost-system support.

Evidence under `~/card-lane/pipeline-unlimited-20261103/integration/`:

- `random-discard-proof/validation.json`: 17,132 passing tests across 133 checked Rust workspace targets; log digest
  and exact tested native commit. Fifteen existing ignored tests are recorded separately.
- Five focused tests cover every selectable card, all twelve forced paths through the six two-card subsets of four
  cards, no-choice cases, larger hands, and selection before the discard instruction. Four fail with the old helper.
- `random-discard-recovery/before.json` and `after.json`: both unchanged Gwendlyn scenarios fail with the old helper
  and pass with the new one. No seed search or expectation change was used.
- The first workspace run exposed an existing Seedshark scenario's reliance on Mind Twist randomly preserving the
  blocker it needed later. Its fixture now forces two Island selections and explicitly checks the discard count
  and surviving blocker. Its original incubator and flying assertions remain. The final workspace run passes.

The exact clean tested native history is merged locally into integration and promoted as the worker checkpoint.
Later imported cards remain outside that tested checkpoint. No Cantrip push or PR is part of this recovery. The
generator vocabulary refresh changes only its API-source fingerprint; vocabulary names and feature gates are
identical. Gwendlyn keeps its existing reservation, packet evidence, and normal card gates on retry.

Separately, Power Artifact and Powerleech were retried after verifying that their only failed gate was profiling
with an LLVM disk-quota error. Current lane initialization places temporary profiles in writable `target/tmp` before
constructing tool environments. Their generated rows and revisions remain unchanged; all normal gates still apply.
Receipts are in `profiler-infrastructure-recovery-20261104/`.
