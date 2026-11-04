# Generator callback roles and graveyard moves

The implementation retries exposed two generator defects. Knowledge Vault
constructed graveyard destinations instead of using the existing rules verb.
Vodalian War Machine's payment filters and several source-tapped duration
predicates received citations for resolving spells because their names fell
through the callback classifier.

Callback citations now follow their API position: `Lasting.holds` uses CR 611.2b,
`LastingRule.applies` uses CR 611.2c, and permanent/card payment filters use
CR 118.1 and 601.2h. Names and matching Rust signatures do not determine these
roles. Duration citations alone do not repair the separate engine requirement
for uninterrupted source-tapped intervals.

Graveyard moves now use `Game::put_into_graveyard` when their typed selector
proves the inputs are outside the battlefield. That verb refuses battlefield
permanents, so potentially mixed sets keep one simultaneous `move_together`.
Both paths retain only actual graveyard arrivals in their result, including
when a replacement redirects a move elsewhere.

The two new citation tests fail before the correction (six existing tests pass).
The graveyard regression fails before its correction and passes afterward,
covering exile, filtered exile, an explicit zone intersection, nonbattlefield
unions, and battlefield/mixed controls. A following life-gain instruction
consumes the movement result so the graveyard-arrival filter remains live.

The complete generator suite passes: **998 tests across 98 targets**, with two
existing ignored tests. Exact source captures and logs are under
`/mnt/vizier/card-lane/generator-blocked-fixes-20261104/`. No reference fixture,
Oracle data, or card-lane gate was weakened.

All 83 affected/stopped modules that compile against the frozen AA native
foundation still compile with the fresh output. Osai Vultures remains explicitly
excluded from this baseline check because it requires the separate Carrion
counter prerequisite. This compilation check does not establish the other
stopped cards' gameplay correctness.
