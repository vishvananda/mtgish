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

These issues were reported during the first review and subsequently fixed by
`e954881`, as recorded below. The Gustcloak
classification is also supported by Wizards' [Gustcloak ruling](https://magic.wizards.com/en/news/feature/saturday-school-2-2002-11-02):
untapping is an effect, so an already-untapped blocked creature can still be
removed from combat. No new gameplay execution is claimed for these reports.

## Fork lowering and generation corrections

The upstream typed source already expresses these five cards correctly. Their
reported failures originated in our normalized lowering or generation, so they
were not filed as upstream source errors. These corrections are now implemented:

| Cards | Fork correction |
|---|---|
| Arashin Sovereign; Avenging Angel | Dead-permanent library primitives use the first graveyard incarnation and retain the graveyard guard. |
| Kroxa and Kunoros; Young Necromancer | Guard eligibility before the fixed-number optional exile; preserve its actual receipt and reflexive announcement timing. Replacement-sensitive completion remains a separate question. |
| Yuna's Whistle | The reflexive gate uses the reveal receipt, and its amount is captured explicitly from the unique found creature before movement. |

Calibrated Blast now explicitly samples the mana value of the unique matching
nonland immediately after RevealUntil, before reordering, and its reflexive
trigger refers to the reveal receipt. Yuna's Whistle similarly samples the
matching creature before moving it into hand. Both use BindValue/BoundValue;
the generator carries that nonnegative value using Cantrip's existing counted
reflexive API. A signed-value control still reports a backend gap rather than
silently converting a negative number into an unsigned count. Existing engine
quantity limits remain in force. No engine types or schema versions changed.
The broader legacy Query(Observed) matching substitution remains in place for
callers outside these explicit captures; this is not a global query-contract
change.

The other bounded questions in Oracle's investigation index remain separate.
They are not additional proved upstream errors.

## Explicit joins and Oracle runtime adoption

Commit `afeb03f` repairs the branch-receipt migration blocker. JoinResults now
retains the actual renamed definitions from If/ChooseOne branches, including
reused numeric labels in independent branches. Ordinary reads still cannot
reach sibling-branch locals. The validator rejects dangling, future, and
unrelated-branch joins; deferred contexts do not inherit the enclosing join
candidates. Covetous Urge and Psychic Intrusion now canonically match their
saved expected programs. Four focused tests cover full programs, consistent
renaming, reused labels, invalid direct reads and unrelated branch groups.

"Oracle runtime adoption" means moving oracle-grammar's pinned MTGish AST,
validator and canonicalizer dependencies to this fork revision and rerunning
its migration checks. This repository's repairs remove the identified join
blocker; they do not update that external project's pins or imply its complete
migration suite has passed. Keep the stable runtime pin until that separate
preservation check passes.

## Source repairs and validation

Commit `e954881` applies the seven reported upstream source corrections above.
The shared another-target rule also updates seven callers whose attacking or
entering creature is the source: Cunning Coyote, Eel-Hounds, Flensing Raptor,
Foot Elite, Hardened Escort, Imperial Aerosaur and Oliphaunt. Complete-record
comparison verifies that each additional caller changes only its exclusion
reference. Fourteen parser fixtures check the complete Source records, and all
three checked-in Source formats are regenerated without reordering the corpus.

The full Cantrip-enabled AST suite passes 1,319 tests, with four opt-in gates
ignored by default. The invoked unsealed native audit validates all 32,755 faces
before and after canonicalization and checks idempotence.

The fresh parser pass succeeds for all 33,529 records. All records also lower
and validate, with no unsupported Source forms. The grammar audit retains the
same 182 findings and exact matched-use evidence, including 145 scalar and 37
context reviews; only the grammar fingerprints changed. Go tests pass.

Fifteen complete corrected ability fixtures cover the seven Source reports,
two death returns, two fixed optional exiles, Yuna and Calibrated Blast, plus
unchanged Dokuchi Silencer and Sutina controls. The generated native gate runs
16 tests against Cantrip `a2aabdfa6`, including found-card-at-top and lands-first
reveals, Yuna with no reorder remainder, all undersized graveyard populations,
and both death returns with and without another zone change before resolution.
All 16 pass. Evidence is retained under
`/mnt/vizier/oracle-fixes-20261107/`; the tests are reproducible from
`reference_contract_runtime.rs` with its documented environment variables.

## Separate upstream update

Upstream `15a265e` carries 115 card-source changes for issues 41–43. A byte-level
comparison finds 31 identical to our fork, 11 still identical to their prior
upstream versions, and 73 independently modified in our fork. Those counts are
not semantic adjudications. That batch needs a separate semantic integration
review; replacing fork records wholesale would discard reviewed local edits.

The separate upstream update remains excluded, as requested.

## Additional report review

The later Oracle packet confirms [Distracted Botanist’s population error](https://github.com/i5jb/mtgish/issues/230): its perpetual grant selects
only creature cards. The printed population is permanent cards in your graveyard.
The producing template now uses IsPermanent, preserving the controller's
private graveyard, death trigger, granted enters trigger, draw and life gain.
Only this Source record changes in a fresh 33,529-record parser pass. Its
complete Source regression fails before the edit and passes after; the complete
corrected ability validates and canonically matches the worker's minimal
proposal. This brings the batch fixtures to 15 complete Source records and 16
complete ability lists. No perpetual-effect engine support was added.

The newer canonical-73e review also correctly identified two regressions in
our explicit-join patch. Executed regressions show that an ordinary enclosing
definition made after a choice must shadow that choice's older numeric label,
and a 300-branch/256-operand expansion must refuse above the existing bounded
work policy. Both failed on73e. Ordinary definitions now take priority within
the lexical scope, mixed ordinary/branch joins validate, and the 65,536-unit
per-pass limit charges namespace probes and receipt visits. A refusal occurs
in the reading pass without replacing the caller's join list. The existing
negative controls still reject future/dangling reads and unrelated choices.

Repeated operands retain their occurrence multiplicity, matching retained607
and the current Cantrip consumer's append behavior. Reused branch labels expand
to their separate definitions for each explicit operand occurrence; no global
numeric-label lookup or output deduplication changes that meaning. Seven focused
join tests cover these contracts, canonical validity and idempotence, including
a granted ability whose local branches must beat an outer ordinary label.

Halfdane remains a separate exact-duration representation gap, reported as
[#229](https://github.com/i5jb/mtgish/issues/229). The printed effect lasts to the
end of the next upkeep, while UntilNextUpkeep currently lowers to UpkeepBegins.
Other callers correctly need the beginning, so changing that existing form
universally would introduce new errors. A distinct Source expiration and exact
normalized Duration are needed. No approximation, gold edit or new schema-valid
frame is claimed here; adding/skipping steps makes a next-draw-step substitute
unsafe. This follow-up does not alter Source/IR schema178 or quantity limits.

Final follow-up gates: 1,322 AST tests pass (four opt-in gates ignored); all
32,755 unsealed native frames validate before and after canonicalization and
remain idempotent. The uncached full Go suite passes. Evidence and the failing
before-fix join/parser logs are retained under
`/mnt/vizier/oracle-botanist-20261107/`.
