# Held-card recovery, 2026-11-03

The original reserved Unlimited holds comprise 44 generation gaps and 18 cards
deferred after implementation attempts. The current generator emits complete
modules for 26 of these 62. Those 26 were explicitly returned to implementation;
the remaining 36 still have generation gaps. A generated module is not a released
card: implementation, scenario, coverage, mutation and Terra review gates remain.

## Findings from the resumed cards

Four worker reproductions exposed two generator errors, both expressible with
existing Cantrip APIs:

- **Pirate Ship and Sea Serpent:** lowering conflated a restriction on the
  defending player with one explicitly naming players and planeswalkers. A
  battle's protector is also its defending player. Schema 167 represents this
  separately as `AttackDefenders::DefendingPlayers`; emission uses the existing
  `defending_player_of` query. Explicit player/planeswalker restrictions retain
  their narrower meaning.
- **Siren's Call and Nettling Imp:** comparing only the current step allowed
  casting/activation again before an additional combat. The restriction now also
  checks this turn's history for an earlier declare-attackers step.

The battle refusal scenarios need another legal attacker to keep a declaration
question available after the fix; otherwise the engine correctly skips that
question entirely. The retained fixtures include an Ornithopter that can attack
the same battle, while the restricted creature cannot. This adds a positive
control rather than weakening the refusal.

Two further fixes address validation blockers:

- Twiddle's effect cites the untap keyword action, CR 701.26b, rather than the
  untap step's turn-based action.
- Known sacrifice counts compile to `AsManyAsCan`; the generator no longer emits
  an impossible `Exactly` alternative for them. This removes unreachable coverage
  branches in Lich and Lord of the Pit without changing selection behavior.

## Validation

- 311 generator, lowering, serialization, schema and related regression tests
  pass, including the explicit player/planeswalker control.
- All 26 freshly emitted whole modules compile against the existing Cantrip
  snapshot in `/mnt/vizier/card-lane/generator-unlimited-fixes`.
- The four combat cards pass six scenario executions. Restoring all four old
  modules makes all four distinguishing scenarios fail; restoring fresh output
  compiles successfully. The scenarios are retained under
  `rust_syntax/normalize/tests/fixtures/unlimited_followup/`.
- Local generation and before/after evidence are under
  `~/card-lane/held-followup-20261103/` (`rows.jsonl`, `fixed.json`, `negative.json`).

## Pipeline recovery

The resumed work also exposed infrastructure failures, separate from card rules:
stale coverage scratch files exhausted `/tmp` quota; implementation sandboxes
could not write their own validation state; a newly declared writable state root
had not been created before sandbox setup; and the style check rejected full
numeric-representation guards. These were corrected with retained attempts and
packets. Missing review reports now stop as infrastructure failures after one
model invocation, rather than generating repeated repair calls and false card
rejections. Targeted implementation retries can pin a generator revision without
changing other active leases.

Publishing counter-vocabulary conflicts now merge plain additions while keeping
all existing numeric IDs. A full workspace gate also found that the card-zone
contract reader missed the printed conditional "if this card is exiled"; its
positive case and transition-event control were corrected. Batch recovery still
runs all integration gates before pushing or creating a PR.

Living Artifact's erroneous optional-effect rewrite was held, restored to the
resolution-time payment required by CR 118.12, and independently confirmed.
Batch 008's prior reservation was archived before adopting the newly approved
candidate. No old approval was reused as release evidence for changed source.
