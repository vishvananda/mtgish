# Optional capped counter placement

`PutCountersCapped` already means "put up to this many counters", as its IR
contract and source reduction specify. Its backend incorrectly always put the
maximum number permitted by the total-counter ceiling. It now asks the ability
controller for a number from zero through the smaller of the announced amount
and the available room, then puts the chosen number of counters.

This is a generator fix using the existing number prompt. The chosen value is
converted to the counter amount only after it is bounded. The native prompt
supports 256 values; a wider choice remains an explicit gap instead of being
truncated. A wide ceiling is supported when the amount itself bounds the choice
to that range. A source that has ceased to be its old object is skipped.

Validation:

- **960 generator tests pass across 95 targets**, with two existing ignored
  tests. Both ways of bounding the choice and rejection of a wider choice are
  covered without changing the source corpus or IR schema.
- All **33,529 ordered identities and capability results** are unchanged.
  Exactly four whole modules change: Clockwork Avian, Clockwork Beast,
  Clockwork Steed and Clockwork Swarm. All four exact modules compile against
  the native API.
- The worker's unchanged Clockwork Beast zero-choice scenario fails with the
  old module because `Number(0)` never becomes possible, and passes with the
  actual new full-source module. Three further deterministic runtime cases
  check the ceiling, the announced X limit, rejection of an excessive choice,
  choosing a positive amount, and a zero-room choice. All four pass.

Clockwork Beast and Clockwork Avian are already reserved and can return to the
normal Terra implementation/review/verification pipeline. Steed and Swarm are
outside the current set scope and were not added to intake. No engine or client
capability was added. Native runtime proofs use clean API-compatible checkpoint
`7c93fbf4052aa8074aa00b35c91d327316b2ec87`; current workers use the later
validated checkpoint `956974eaf722e2021c609df481ed80d6c4bef66d`.

Evidence is retained under
`~/card-lane/pipeline-unlimited-20261103/integration/capped-counter-choice-proof`
and adjacent focused/full generator logs. These proofs validate the generator
correction, not complete card approvals.
