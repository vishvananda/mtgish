# Published face-data adoption

The first face release is now published through CI:
[faces-oracle-20261112220158-s190-g0.1.0-r1](https://github.com/vishvananda/oracle-grammar/releases/tag/faces-oracle-20261112220158-s190-g0.1.0-r1).
`data/oracle_grammar_reference.json` pins its manifest and original source hashes,
producer/exporter revisions, schema, model versions and allowed population.
All release assets were downloaded and verified before the actual comparison.
The existing exact registry dependencies remain at 0.1.0; no schema or runtime
AST layout change was required.

## Adopted correction

Oracle Grammar's reviewed U0001 normalization is implemented in the shared
cast-history reducer: existence is `occurrences >= 1`, consistently with other
history predicates. This changes Dream Thief, Hotheaded Giant, I Bask in Your
Silent Awe, Laboratory Drudge and Spider-Man 2099. Event operands, windows,
players, source exclusions and all effects remain identical.

The entire existing 33,529-record Source corpus was freshly lowered, validated
and binary-roundtripped. Exactly those five records change, only in the threshold
operator and constant; 33,524 records remain byte-identical. Eight full-card
regression fixtures include the five corrections plus Patrician's Scorn, Soul
Reap and Talara's Battalion as existing controls. Historical saved IR and old
snapshot files are preserved. The two old Dream Thief expectations have explicit,
input-checked test-only transitions after their earlier reviewed migration.

## Published-data audit

Fresh MTGish parsing and lowering used the release's original 39,288-record,
42,679-face Scryfall snapshot, without overwriting MTGish's existing source data.
The strict comparison remains **failed**, with every outcome visible:

| Outcome | Faces |
| --- | ---: |
| Exact structural agreement | 1,812 |
| Canonical-only agreement | 18,706 |
| Structural mismatch | 4,044 |
| Missing MTGish occurrence | 163 |
| Native ambiguous | 984 |
| Native refused | 16,970 |

These are not 4,044 confirmed rules bugs. For example Storm Crow retains Flying
inside the native `WithReminder` wrapper, while MTGish omits that presentation;
Nissa, Worldsoul Speaker retains an ability-word `WithPresentation` wrapper.
Unused result IDs and ordering account for many canonical-only differences.
The raw expected values are not rewritten, no reading is selected from ambiguity,
and neither missing records nor refusals count as agreement. The remaining
structural differences need separate producer/presentation/input-policy triage.
Three comparable U0001 faces now canonically agree; I Bask in Your Silent Awe and
Spider-Man 2099 are native refusals, so their correctness is covered by the
independent full-card/whole-corpus regressions, not a false exported agreement.

## Tool fixes and upstream

A real fresh-snapshot failure was reproduced at upstream
`5a02adf9cd2c13d5028acc57e8eb9e663ecfb34b`: unsupported Scryfall layouts print a
warning before the JSON array on stdout. Hela, Mistress of Darkness // Dark
Tutor's Vengeance face triggers this case. The warning now goes to stderr and
is retained in comparison evidence. Unsupported-layout acceptance is unchanged.
The fix is reported as [MTGish #277](https://github.com/i5jb/mtgish/issues/277).

The full release also exposed a comparison-tool size bug. Requests now use
JSON Lines, keeping the shared 2 MiB/depth bound for each request rather than
rejecting an entire 46 MiB corpus document. Regression tests cover large batches,
oversized rows, invalid input, independent JSON Lines and preprocessing output.

All **2,006 workspace tests pass** (20 ignored, 204 targets), without a global
stack-size override. All eight Python comparison-tool tests pass. The full
published-data audit deliberately retains its nonzero comparison exit; tests
passing does not claim raw export agreement. Exact hashes and changed paths are
in [the proof](oracle-face-adoption-20261113.proof.json).

## Grammar worker handoff

The existing pending U0001 overlay is now adopted by MTGish. Keep it while its
reference still pins the old export. When adopting a newly generated MTGish IR
reference, remove U0001 in the same input-checked reference update; do not alter
the published 0.1.0 bundle or apply the correction a second time.

The Cantrip foundation and card workers are the next task. This adoption changes
no Source cache records, card code, native quantity types, generator semantics,
Wasm or browser workspaces.
