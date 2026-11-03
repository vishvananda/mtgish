# Generator review feedback, 2026-11-03

This pass checks review findings against freshly generated modules rather than
copying workers' final implementations. The 26 source/Oracle records are in
`rust_syntax/normalize/tests/fixtures/generator_review_feedback.*`.

## Semantic fixes

- Preserve bounds before narrowing numeric casts. The tidy pass previously
  removed the generated `i16::MAX` bound from Living Artifact's recorded damage;
  a positive damage amount could become a negative counter adjustment.
- Exclude spell copies as well as tokens from card-only target and zone queries.
  Fresh Raise Dead and Regrowth already excluded tokens; the new target fix is
  the missing spell-copy exclusion.
- Use `entering.controller` for Jihad's entry choice, before the permanent has a
  battlefield controller.
- Carry both the referenced object and triggering player for upkeep triggers
  expressed through an object's controller. Wanderlust's original source-choice
  scenario is valid: an object referred to by a pending ability remains a legal
  source choice under CR 609.7a even after leaving the battlefield. The damage
  source remains the Aura.
- Preserve Paralyze's triggering player and referenced creature after attachment
  or control changes. Its payment must not untap the Aura's new host.
- Skip a recorded draw recipient who has left the game. The Super Intelligence
  test checks an actual empty-library draw-attempt flag on that departed player.

## Shared recipes and findings that were already correct

Printed colored protection now uses the shared protection recipe. The former
`CardDef.protection` representation also passes the text-change and ability-loss
test; this is sharing, not evidence of a former protection defect. Source and
target regeneration, simple constant single-target damage, and common permanent
target specifications also use existing shared recipes. Restricted target
filters are retained.

Chosen-value reads delegate to the existing linked-value reader. Query and layer
interfaces intentionally take different arguments (`asker` and `asker.source`).
Both forms compile and are regression-tested.

Existing saturating P/T arithmetic is retained. The ordinary `plus_pt` helper
uses plain addition; replacing saturation with it would change boundary behavior.
This pass neither changes engine number widths nor standardizes those helpers.

Living Artifact's optional counter removal remains `may_pay`. The suggestion to
replace it with a plain yes/no choice was rejected: CR 118.12 defines this
optional action followed by "if you do" as a resolution-time cost. The pinned
engine already suppresses mana actions for a cost with no mana component. The
previously approved, unpublished candidate containing that rewrite is held for
correction in job `f95089e8866d668bb86f`; its old approval is not a release gate.

Rules source: [Magic Comprehensive Rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt),
118.12 and 609.7a.

## Validation and release scope

- 88 generator regression tests pass with `--features cantrip`, including the
  new 13-test feedback suite, the prior Avatar and Unlimited suites, and related
  target, reader, cost and lane suites.
- All 26 freshly emitted modules compile against Cantrip helper snapshot
  `f6ca552288cff0ec7fc32a9a5b7470667c1d502e`. The isolated probe adds only the
  missing Vitality counter vocabulary entry; it changes no engine procedure.
- The 26 card corpus runs pass: 129 scenario executions, no failures or missing
  cards. The original Wanderlust scenario is retained.
- Five native engine tests in `generator_review_feedback_runtime.rs` pass when
  copied into the probe's `crates/cantrip-script/tests/` directory. They exercise
  the registered generated cards. Restoring old counter/card-target/Paralyze
  output and removing the departed-player guard yields four failing tests and
  the passing protection control. Restoring fresh output yields five passes.

The local probe is `/mnt/vizier/card-lane/generator-feedback`; complete logs and
the before/after modules are under
`~/card-lane/generator-feedback-20261103/`. Runtime evidence is recorded in
`runtime-final.json` and `engine-negative-final.json`.

This validates generator changes, not release approval for 26 cards. The live
intake's pinned generator revision and held-card queue remain unchanged. Held
cards still require their normal implementation, coverage, mutation and Terra
review gates before publication. Remaining generator gaps and engine blockers
stay separate from this pass.
