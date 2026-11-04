# Blocking status after an attacker leaves combat

Validated local Cantrip checkpoint `402b7f61accf6d5015ec340d7ca4552bd730e867`
records blocking creatures independently of their remaining attacker associations.
`Query::is_blocking` now implements CR 509.1g and 510.1d when the blocked attacker
leaves combat. Righteousness and Rally are two printed users; the same defect
also blocked Piety, Elven Fortress and Spore Cloud.

The state has a journaled `blockers` list as well as `blocks` relationships.
Combat ending, regeneration, controller changes and loss of creature type remove
blocking status at the appropriate time. Removing only the attacker preserves
its surviving blocker. Exact state hashing and normalized loop detection include
the independent status. `Combat.Blockers` and `.Count` are available for scenario
assertions. The shared attacking-or-blocking target filter, Glyph of Destruction
and Tetsuo Umezawa use the new query.

The generator gates the corrected predicate on `blocking-status`; older targets
report an explicit gap. Blocking-alone predicates read the independent list as
well. This does not supply last-known relationships for a departed blocker;
Ydwen Efreet remains held for that separate issue. Compiling a generated module
does not approve its card semantics, including pre-existing last-known-status
questions in later-set cards such as Thijarian Witness.

Validation:

- **17,172 native tests pass across 134 targets**, with 15 existing ignored
  tests. Fourteen focused tests cover attacker departure, regeneration, type
  and control changes, combat and turn ending, surviving blockers' damage,
  multiple associations, rollback/replay, exact hashing and loop detection.
  Three cases fail against the old relationship-only behavior.
- The full suite caught a missing harness registration for the new dump key;
  registration was corrected and the full suite passes afterward.
- Consecutive 128-game benchmarks with verification unset measure
  **38.16 / 34.84 microseconds per engine decision** before/after. Machine load
  varies; no speedup is inferred.
- **958 generator tests pass across 94 targets**, with two existing ignored
  tests. Tests exercise both feature states and an unchanged Lightning Bolt.
- All **33,529 ordered output identities and capability results** remain
  unchanged. Exactly 147 whole modules switch only their blocking queries;
  fourteen also receive single-expression closure-brace formatting changes.
  Nine exact full-source modules compile against the clean native checkpoint.

All five affected reserved reviews were returned with their prior work and
rejection counts retained. Piety's old parked parent was on a fork containing
two unrelated named-counter prerequisites. Its three owned files were first
replayed byte-for-byte onto the validated base; the original branch, complete
old row and exact patch remain archived. Normal review and verification still
decide card approval.

Evidence is under
`~/card-lane/pipeline-unlimited-20261103/integration/blocking-status-{proof,generator-proof}`
and adjacent baseline, focused, workspace and benchmark logs. Native changes
remain local during consolidation. The semantic schema remains 174.

Follow-up: Rally's reviewer found that the first-strike step decision still
read only remaining block associations. Native checkpoint
`956974eaf722e2021c609df481ed80d6c4bef66d` reads the independent blockers list
there too. Its full workspace passes **17,173 tests**, with fifteen focused
blocking tests, including first/double strikers with no remaining attacker and
the intervening priority window. The API fingerprint is unchanged. A 128-game
before/after pair measures 36.86 / 31.77 microseconds per engine decision;
load varies and no speedup is inferred. Rally passed independent confirmation
after returning on this checkpoint. Snow Devil's old approved static predicate
was separately reopened for repair; the two native callers were migrated in
the engine change. Evidence is in `integration/blocking-first-strike-proof`.
