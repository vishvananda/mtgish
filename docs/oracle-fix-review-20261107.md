# Oracle fix review, 2026-11-07

## Incorporated mode repair

Fork branch `fix/mode-result-scope-20261107`, commit `8a4151d`, is incorporated
into main. Its result-map boundary matches the validator's independent Mode
scopes. Seven synthetic regressions cover unused-label alpha equivalence,
mode-local reads, exported immediate receipts, legitimate outer captures,
invalid leaks and duplicate definitions. All other published
`fork/oracle-grammar-*` branches were already ancestors of main at review time.

Review found the same missing boundary for target-slot maps. Reusing a target
label in two independent modes versus alpha-renaming the second label produced
different canonical programs, despite both inputs being valid. The additional
repair restores the enclosing slot map after each Mode, preserving outer target
captures. Three regressions cover alpha equivalence, leaks/duplicates, and genuine
outer targets. The alpha test failed before the repair and passes afterward.

Validation: 1,313 AST tests pass, with four explicit gates ignored by default.
The invoked unsealed native audit validates all 32,755 supplied faces before and
after normalization and checks idempotence. No schema, source-model, numeric-range
or Cantrip engine changes are involved.

## Upstream source findings

The latest upstream main, `15a265e`, still has these seven source-level errors.
Complete worker packets and the actual upstream source were checked before
filing; the issue list was searched for prior card-specific reports.

| Card | Incorrect operand or frame | Report |
|---|---|---|
| Guardian of the Ages | The attacker receives Guardian's defender/trample changes. | [#222](https://github.com/i5jb/mtgish/issues/222) |
| Gladiolus Amicitia | Another-target excludes the entering land instead of Gladiolus. | [#223](https://github.com/i5jb/mtgish/issues/223) |
| River Kelpie | Both unqualified graveyard-entry branches require your graveyard. | [#224](https://github.com/i5jb/mtgish/issues/224) |
| Brokers' Safeguard | The exiled creature returns under the caster's control instead of its owner's. | [#225](https://github.com/i5jb/mtgish/issues/225) |
| Gustcloak Cavalier | Optional untapping is encoded as a prerequisite payment. | [#226](https://github.com/i5jb/mtgish/issues/226) |
| Lukamina, Crocodile Form | Its duration survives a control change. | [#227](https://github.com/i5jb/mtgish/issues/227) |
| Myojin of Towering Might | Only the first announced creature gets trample. | [#228](https://github.com/i5jb/mtgish/issues/228) |

These issues are reported, not implemented in this review. The Gustcloak
classification is also supported by Wizards' [Gustcloak ruling](https://magic.wizards.com/en/news/feature/saturday-school-2-2002-11-02):
untapping is an effect, so an already-untapped blocked creature can still be
removed from combat. No new gameplay execution is claimed for these reports.

## Fork lowering and generation queue

The upstream typed source already expresses these five cards correctly. Their
reported failures originate in our normalized lowering or generation, so they
were not filed as upstream source errors:

| Cards | Fork correction needed |
|---|---|
| Arashin Sovereign; Avenging Angel | Dead-permanent library primitives use the departed battlefield object. Use the first graveyard incarnation, retaining the graveyard guard. |
| Kroxa and Kunoros; Young Necromancer | Their fixed-number optional exile can partially execute with too few cards. Add correct eligibility before the optional action; preserve reflexive receipt and announcement timing. Replacement-sensitive completion remains a separate question. |
| Yuna's Whistle | The reflexive gate and value read use the reorder receipt instead of the reveal/found-card receipt. Coordinate explicit found-card value capture and consumer semantics; the worker's ID-only edit is not a complete fix. |

Calibrated Blast's declared ordered-query contract versus the emitter's matching
substitution remains a contract question, as do the other bounded questions in
Oracle's investigation index. They are not additional proved upstream errors.

## Current runtime migration blocker: explicit joins

The mode worker's historical packet recovers all eight modal matches and all 47
old alternatives. Its current full saved-IR audit still loses Covetous Urge and
Psychic Intrusion. Both join results produced in mutually exclusive branches.
The canonicalizer restores those branch-local maps before visiting JoinResults,
so the explicit join retains original numeric labels instead of the renamed
branch receipts. The validator currently checks only that a join has two inputs;
it does not catch these dangling references.

An independent synthetic probe against this checkout confirms the defect:
branch results 11/22 are renamed to unused canonical IDs, while the join still
reads 11/22. Consistently alpha-renaming the input to 111/222 changes canonical
equality. Both input/output pairs pass the existing validator. This is a fork
canonicalizer/validation defect, separate from the mode repair; no gameplay
failure was executed.

Keep the stable Oracle runtime pin until explicit branch-receipt joins are
handled without exporting unrelated branch locals and the full migration
preservation checks pass. The native validity/idempotence audit alone does not
settle this. Evidence and the probe source are retained in
`/mnt/vizier/oracle-fix-review-20261107/`.

## Separate upstream update

Upstream `15a265e` carries 115 card-source changes for issues 41–43. A byte-level
comparison finds 31 identical to our fork, 11 still identical to their prior
upstream versions, and 73 independently modified in our fork. Those counts are
not semantic adjudications. That batch needs a separate semantic integration
review; replacing fork records wholesale would discard reviewed local edits.
