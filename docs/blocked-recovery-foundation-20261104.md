# Blocked-card foundation, 2026-11-04

The 34 held generator-gap reservations have been released, archived and removed
from the campaign queue at the user's request. Intake journal exclusions prevent
new plans and stale frozen plans from reclaiming them. The remaining blocked
work, AST consolidation and final foundation/sibling PR export remain in scope.

## Validated native checkpoint

`6ae17e0eae80e8cc7f60bbfb7a8dd0d90ba558f4` combines:

- `153f7324`: individual numeric refusals in the scenario harness. A bare
  `! Bot1.Number` still refuses the question; `! Bot1.Number(1)` tests that answer
  and leaves the question available. Four focused tests pass. The actual
  Clockwork no-room scenario passes, catches the counter-floor mutation and
  passes after restoration. The standalone workspace run passed 17,177 tests.
- `a7e77d9b`: resolved prevention and replacement effects retain their creation
  text through the existing `ContinuousEffect::words`. Matching, applying,
  outcome queries, shield advice and follow-up instructions use that text.
  Other abilities asked during a follow-up read their own current text. Three
  baseline regressions fail; four final tests pass, alongside the existing core
  tests. The standalone workspace run passed 17,181 tests. The retained White
  Circle scenario fails before the engine patch and passes afterward with its
  candidate source and expectations unchanged.
- `77fe58d6`: one colored-source prevention recipe, taken from the retained
  White/Blue helper proposal. Other Circle proposals must migrate to it rather
  than adding aliases or duplicate recipes. The printed-card probe includes
  this same prerequisite in both versions.
- `291831bd`: regeneration shields and regeneration prohibitions are shown on
  affected permanents through existing granted-ability text, including duration.
  A host regression checks both players, the public view and removal. It fails
  without the projection. The browser test checks both battlefield rows and
  disappearance on the next view, waits for the startup overlay to disappear,
  and captures the rendered table. The standalone workspace run passed 17,178
  tests; the web build and the targeted browser test passed.

The combined exact checkpoint passes **17,182 tests across 135 targets**, with
15 existing ignored tests. The paired 128-game release measurements are
37.48→38.18 and 38.90→33.06 microseconds per engine decision. Their variation does
not establish a reliable performance change.

The generator snapshot refresh changes only `API_FINGERPRINT` to
`152f4651d82b88dd6aeea358f46fd1678369a58d84985e650ca051dedbf63979`.
Vocabulary names, feature flags and emission logic are unchanged. The merged
generator suite passes **985 tests across 97 targets**, with two existing ignored
tests.

## Retained receipts

`~/card-lane/pipeline-unlimited-20261103/released-held-20261104` contains the
frozen release plan, original queue, intake journal and all 34 GitHub release
receipts with clear post-release holder checks. The pipeline suite has 295 passing
tests after the exclusion guard.

Native, browser, paired benchmark, printed-card and generator receipts are in
`~/card-lane/pipeline-unlimited-20261103/blocked-focus-20261104`.
The native checkpoint remains local until the requested final PR packaging.
