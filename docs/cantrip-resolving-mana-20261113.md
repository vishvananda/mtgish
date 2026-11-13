# Resolving mana payments

Power Sink and Condescend offer a payment of the already announced X; they must
not offer a new X announcement. The aggregate-cost path now substitutes the
stack item's X using the same existing `ManaCost` expression as `ManaWithX`.
Musician's one generic mana repeated once per music counter uses the existing
u32 generic field directly. This removes an arbitrary 255 cap without widening
any native types or changing other repeated-mana operations.

The production-profile generation fixture covers all three cards. Runtime A/B
proofs against the early-set native foundation use actual generated definitions:

- Power Sink at X=2 with only one available mana: the worker's original
  `! Bot1.PayCost` assertion fails before the repair and passes afterwards.
  The exact scenario is retained alongside the fixtures as
  `early_resolving_mana_power_sink.cantrip`.
- Musician with 256 music counters and 255 mana: old output wrongly offers
  payment; the repaired definition rejects payment. The exact probe is retained
  in `rust_syntax/normalize/tests/fixtures/early_resolving_mana_scenarios.rs`.
  Install it under `crates/cantrip-script/tests/` in a native checkout with the
  generated Musician registered, then run that test target with the cards profile.

Native proof logs are retained under
`/mnt/vizier/early-set-campaign-plan-20261113/`:
`power-sink-{baseline,fixed}.log` and
`musician-256-{baseline,fixed}.log`.
These are generator defects; no new engine API or representation is needed.
