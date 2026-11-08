# Rolling Hamsphere and Knowledge Pool reference actors

The two new reports in oracle-grammar's `reference-fix-followup.md` are confirmed
against fork c0e902b. The Source template corrections and regenerated data are
published as [160d35d](https://github.com/vishvananda/mtgish/commit/160d35d).
Oracle-grammar's files, pins, gold and comparison rules are untouched.

## Rolling Hamsphere

The attack body creates three Hamsters and then the Vehicle deals damage. Its
Source template instead emitted `EachPermanentDealsDamage` with the created
batch as sources. The corrected template emits `PermanentDealsDamage` with
`ThisPermanent`. The existing amount expression still counts every Hamster you
control after creation; the token recipe, target, static pump and Crew are
unchanged. With zero/two earlier Hamsters, this produces 3/5 damage once rather
than 9/15 from three token sources.

[Official Bloomburrow release notes](https://magic.wizards.com/en/news/feature/bloomburrow-release-notes)
identify the Vehicle as the source and specify when X is determined.
Upstream parser report: [i5jb/mtgish#236](https://github.com/i5jb/mtgish/issues/236).

## Knowledge Pool

The event correctly matches any player casting from hand, but the template's
unqualified `MustCost` and `MayAction` assign the payment, decision and free cast
to the ability controller. `PlayerMustCost(ThatPlayer, ...)` and
`PlayerMayAction(ThatPlayer, ...)` preserve the triggering player's role.

The normalized change consists of exactly three PlayerRef leaves:
`PayCost.player`, `Optional.decider`, and `PlayCards.actor`. Each changes from
`AbilityController` to `EventSubject`, the spell-cast event actor. The separate
ObjectRef `EventSubject` continues to name the triggering spell. The ETB ability,
cast matcher, hand filter, exile action, payment guard, linked pool, count and
free cost remain unchanged.

This is an actor-only repair. The separate missing “other cards” exclusion is an
unresolved linked-set contract question and is deliberately retained; complete
card correctness is not claimed. Runtime tests choose a different, legal card
already in linked exile, so they do not assume a resolution of that question.
Upstream parser report: [i5jb/mtgish#237](https://github.com/i5jb/mtgish/issues/237).

## Validation

- Both full Source regressions fail before the template changes and pass after.
  Unchanged full-card controls cover Possibility Storm's explicit cast-event
  actor and The Western Cloud's damage from each created Treasure.
- Both complete native programs match independent actor-only/source-correct
  proposals, validate, and normalize canonically and idempotently.
- Both full cards generate executable Cantrip modules. No generator, engine,
  AST type or schema change is needed; normalized schema remains 179.
- Full AST suite: 1,335 passed, four existing opt-in gates ignored. Uncached full
  Go suite passes. The fresh parser processes 33,529 records, changing exactly
  these two. Source record order is retained across the supplemental inputs.
- All three Source formats are regenerated. All 33,529 records normalize
  completely, without unsupported or primitive forms. Complete native export
  comparison changes exactly the same two records and no others.
- All 182 reviewed grammar finding IDs and matched-use evidence remain identical;
  only the changed grammar's bound fingerprints are refreshed. Current parser
  implementation hashes are retained and verified.
- The explicit native reference gate passes all 24 tests against the existing
  a2aabdfa6 engine. Hamsphere's generated attack body runs through a targeted
  callback fixture, independently of combat damage, and asserts both amount and
  source identity with zero/two earlier Hamsters. Knowledge Pool uses actual
  spell-cast dispatch with A controlling Pool and B casting, covers accept and
  decline, and verifies B owns the choice and subsequent cast (including its
  life-gain effect). No Cantrip checkout is modified.

Reproduce:

```sh
(cd go_mtg_parser && go test -count=1 ./...)
cargo test --manifest-path rust_syntax/Cargo.toml -p mtgish-ast --features cantrip
```

The existing opt-in `reference_contract_runtime` gate additionally accepts
`MTGISH_CANTRIP_DIR`, `MTGISH_REFERENCE_PROBE_DIR` and `MTGISH_CANTRIP_TARGET`;
run that test with `-- --ignored --nocapture` for native execution. Its new
scenarios are checked in as `fixtures/reference_actor_scenarios.rs`.
Evidence and exact source/native deltas: `/mnt/vizier/oracle-actors-20261108/`.
