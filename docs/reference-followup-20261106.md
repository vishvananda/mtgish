# Further reference corrections, 2026-11-06

## Canonicalizer regression

Fixed in f906c3b. The lexical-result fix in 0e03840 isolated immediate control flow as though it
were a deferred invocation. That discarded receipts the validator deliberately
makes available later: moves from optional bodies and resolution-time player
payments, and gathered outcomes from immediate loops, including nested optional
outcomes. It made 211 otherwise valid native faces invalid after normalization.

Canonicalization now preserves exactly those exported mappings. Deferred bodies,
program operands and non-exported branch/local definitions remain isolated.
Definition-aware liveness and allocation remain separate from the old numeric
labels. The existing delayed-collision, outer-capture and sibling-branch tests
remain required; a new negative test checks an optional numeric local cannot
shadow a subsequent payment receipt. Five complete native fixtures cover direct
and nested immediate exports.

The explicit unsealed audit validates all 32,755 native faces both before and
after canonicalization and checks idempotence. The first attempted repair fixed
200 cases; nested optional loop outcomes accounted for the remaining eleven.
The final audit has zero failures. This corrects our runtime regression; it is
separate from upstream card-reference errors.

## Card operands

- **Effluence Devourer:** grant the perpetual ability to the sacrifice's first
  public-zone incarnation. Its granted activation remains graveyard-only.
- **Prowling Geistcatcher:** exile the post-sacrifice card, retaining the departing
  object for the separate token test and preserving its linked return ability.
- **Inscription of Insight:** in both kicked and ordinary branches, the targeted
  player creates the token. Its size still reads that same player's hand.
- **Rakdos the Defiler:** the combat-damage trigger counts and selects only
  non-Demon permanents controlled by the damaged player. Its attack trigger is
  unchanged. Three eligible permanents require two sacrifices, regardless of
  five eligible permanents controlled by other players.

The independent worker's complete old/proposed IR programs are retained in
regression fixtures. Rakdos additionally makes the printed controller qualifier
explicit in the candidate set, as well as in its numerator; the minimal reported
proposal is retained separately. No canonical equality aliases or reference
fixtures are changed to mask mismatches.

One legacy source action is appended for the triggered-card perpetual effect;
normalized schema 178 and numeric ranges are unchanged. The perpetual target
correction is validated structurally; it does not add digital engine features.
No Cantrip engine files are changed.

## Validation and upstream reports

All 1,303 AST tests pass (four explicit corpus/native gates are ignored by
default); the complete 33,529-record source corpus parses, normalizes and
validates with zero unreduced forms. All Go parser tests pass. Fresh audit
identities and full matched-use evidence are unchanged for all 182 findings;
parser fingerprints are updated without adding exceptions. Exactly four source
records were regenerated in RON, compatibility JSON and pretty RON.

The sacrifice callers extend [#219](https://github.com/i5jb/mtgish/issues/219#issuecomment-6697319944).
Inscription is reported as [#220](https://github.com/i5jb/mtgish/issues/220), and
Rakdos as [#221](https://github.com/i5jb/mtgish/issues/221).
