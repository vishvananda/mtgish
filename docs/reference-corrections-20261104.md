# Reference producer corrections, 2026-11-04

The additional reference report identified three reproducible producer bugs.
The immutable reference/scorer in oracle-grammar remains untouched.

* **REF-005:** `untapped [subtype] you control` incorrectly added a creature
  restriction. A noncreature kindred permanent with that subtype can pay this
  cost. Correct the existing selector instead of adding overlapping tap-cost
  templates. Preserve untapped/controller restrictions, otherness, counts and
  other cost parts. Explicit subtype-plus-creature wording still requires a
  creature. The full corpus finds **35 affected cards**, including 13 beyond
  the report's 22; kicker, replicate, optional payments and graveyard activations
  all use the same selector. Plural subtype costs were already correct.
* **REF-006:** the manual reflexive cost path now captures the result of a
  resolution-time `ManaWithX { x: None }` payment, matching the ordinary Payment
  recipe. Its reflexive body reads `BoundValue(actual_payment_result)`.
  Electro, Assaulting Battery; Wildborn Preserver; and the back face Remnant of
  the Rising Star change only their X amounts. Electro's separate damage-source
  reference disagreement remains unchanged. Optional payment still encloses both
  payment and trigger creation. Announced X and fixed where-X costs retain their
  meanings.
* **REF-007:** Highway Reaver's alternate cost for the first unearth ability
  each turn is now inside `MaxSpeed(...)`. The existing native lowering supplies
  the conditional; flying, start your engines, the perpetual ability, zero cost,
  and the first-each-turn limit are unchanged.

`reference_costs.rs` compares complete native records, validates/round-trips the
results, and includes an earlier unrelated action so the X-payment result must
be nonzero. Go tests parse 46 whole source records and explicit creature-cost
controls. All 33,529 RON, JSON and pretty RON records were regenerated: exactly
36 records change in each export (35 subtype costs and Highway Reaver). The
normalizer-only reflexive fix does not change the source corpus.

The full Go suite passes. Nine relevant Rust suites pass **446 tests**, with
one pre-existing schema assertion filtered: `pipeline.rs` hardcodes 167 while
the baseline producer already has schema 168. The separately isolated AST
consolidation change replaces that assertion with `SCHEMA_VERSION`.

The fresh grammar derivation also refreshes stale audit evidence from earlier
producer fixes. Existing context-review matched-use hashes are unchanged. The
current Cauldron Dance MustCost template was explicitly re-reviewed: both
ability captures are Haste. Four speed-mining outliers are individually reviewed;
numeric speed and max-speed condition helpers are intentional alternatives to
the rule wrapper. No audit detector or comparison rule was weakened.

The machine-readable card list and validation limits are in
`github_issue_review/reference-costs-20261104.json`. Adjacent creature-type
additions and Either-trigger event subjects are canonicalization questions,
tracked separately from these rules corrections.
