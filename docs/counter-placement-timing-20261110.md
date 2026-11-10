# Coordinated counter-placement timing — 2026-11-10

All six questions in oracle-grammar's `counter-placement-timing-questions.json`
resolve to one simultaneous counter-placement instruction containing two
placement contributions. The current source parser incorrectly emitted two
successive instructions. This is a parser/reference correction; the source AST,
normalized IR and existing engine API already express the required batch.

| Card | Recipient identity and conclusion |
| --- | --- |
| River Heralds' Boon | Both slots can name one Merfolk. Keep both targets and put both contributions in one batch. |
| Serrated Biskelion | Its target can be itself. Both -1/-1 contributions belong to one batch. |
| Sumala Sentry | A face-up Sentry can trigger for its own turn-face-up event. Keep event subject and source as separate contributions in one batch. |
| Drillworks Mole | A commander copying the Mole retains commander designation and can target itself. Preserve its optional target and one placement batch. |
| Juniper Order Ranger | “Another” makes the recipients distinct; the shared placement instruction still processes them simultaneously. |
| X-23, Deadly Weapon | “Another” likewise makes the recipients distinct, without making the coordinated placements successive instructions. |

## Rules reasoning

These phrases share one placement verb and coordinate its counter/recipient
operands. Under CR608.2c the whole instruction must be read in context;
CR608.2f processes one action on multiple objects simultaneously when possible.
They do not contain a second placement instruction or a “then” boundary.
This is the rules interpretation applied to the six complete reviewed texts,
not a rule that every English “and” denotes simultaneity.

CR614.5 permits a replacement effect to affect an event only once. If both
Boon targets name the same Merfolk, its two +1/+1 contributions form a placement
of two counters on that object. Hardened Scales changes that to three, rather
than changing two successive placements of one into four counters.
CR700.1 still allows a trigger to distinguish individual recipients within a
simultaneous happening; one batch does not mean every possible trigger fires once.

Primary sources:

- [Current official Comprehensive Rules, 2026-11-06](https://media.wizards.com/2026/downloads/MagicCompRules%2020261106.docx): CR603.10, 608.2b/c/f, 614.5, 700.1, 708.8 and 903.3.
- [Ixalan release notes](https://magic.wizards.com/en/news/feature/ixalan-release-notes): Boon may choose one Merfolk as both targets.
- [Double Masters 2022 release notes](https://magic.wizards.com/en/news/feature/double-masters-2022-release-notes-2022-06-24): Hardened Scales adds one to the counters placed on a creature.
- [Murders at Karlov Manor release notes](https://magic.wizards.com/en/news/feature/murders-at-karlov-manor-release-notes): Sentry's wording and the independent treatment of an event subject that has left the battlefield.

## Exact implementation boundary

Only the six shared-verb English templates change. Each now returns one source
`PutCounters([first, second])` instead of two source `PutCounters` actions.
Do not merge arbitrary adjacent instructions in lowering or canonicalization.
Targets, filters, amounts, counter kinds, optionality, activators and other
abilities remain exact. The independent packet's complete hypothetical batch
programs are now the expected programs for the reviewed source fixtures.

The two contributions remain present in the IR, even when their selectors are
identical. Runtime recipient overlap is resolved before replacement processing.
No broad canonical alias, target deduplication or compile-time recipient guess
is introduced. `Glorious Purpose` stays an unchanged single-batch control;
`Biogenic Upgrade` retains its explicit sequencing.

Cantrip's existing `Game::add_counters_together` collects counts by object before
`counters::put_together` applies replacements and announces the simultaneous
batch. Its seven counter-batching tests pass, including overlapping contributions
and per-object replacement processing. No engine edit is needed.
Five complete generated modules change from two batching calls to one. Drillworks
Mole remains backend-unsupported because commander queries are not implemented;
its AST and normalized IR validate. That unrelated backend limitation is preserved.
These are generator and core contract checks, not six executed card scenarios.

Upstream report: [259](https://github.com/i5jb/mtgish/issues/259).

## Validation and provenance

- Four focused Rust tests pass: complete six-card expected programs, overlapping
  contributions retained, explicit sequencing retained, and three valid controls.
- Full Go suite passes, including fresh parsing of all six complete cards and
  controls. Grammar-audit fingerprints are refreshed only after proving none of
  the six changed templates overlaps the 182 saved reviewed findings; review
  entries and evidence remain untouched.
- Fresh full source export: 33,529 records, exactly these six changed; 33,523
  unchanged. Tagged JSON preserves every other line byte-for-byte; pretty RON is
  freshly exported from the corrected source.
- Fresh normalized corpus: exactly the same six changed; the other 33,523 records
  and schema188 are unchanged. No AST schema, numeric-width or engine changes.
- Existing schema188 Wasm is reused for the browser catalog; no Wasm rebuild.

Evidence: `/mnt/vizier/counter-timing-20261110`, including the original packet's
complete expected operands, `source-proof.json`, `native-corpus-proof.json`,
`audit-review-proof.json`, `generator-proof.json`, Go/Rust logs and engine logs.
The original oracle-grammar question packet and its gold remain untouched.
