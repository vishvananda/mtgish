# Oracle Grammar 0.2.11 adoption — 2026-11-16

MTGish now uses published `oracle-ir` and `oracle-ir-contract` **0.2.3**,
still schema **191**. Oracle Blocks uses those contracts and published
`oracle-ir-grammar` **0.2.11**. There are no local crate overrides, numeric-width
changes, engine implementations or reference substitutions in this update.

## Releases and input identity

The checked release is `grammar-v0.2.11`, commit
`c8c7bf0f7df14e74a26ea07694f8867aa0aac061`, and its companion
`faces-oracle-20261112220158-s191-g0.2.11-r1`. Every face-bundle asset was verified
against SHA256SUMS, its manifest and the independently retrieved GitHub asset
digest. `data/oracle_grammar_reference.json` records the release and model pins.

The active parser input remains MTGish **20261001.1**, **33,529 records**, Oracle
SHA-256 `2281f3d715e8ee6127825154b31202918ef83a9d9a3641a0d23b0163ebea3065`.
The published bundle's newer source snapshot does not replace this population.
Its native parse coverage is release metadata, not a claim of agreement with
every frozen MTGish face.

The amendment check uses Grammar's independently prepared reference over the
identical frozen Oracle input. Its 67 ledger groups, U0001–U0068 with the
withdrawn U0044 absent, name 948 faces in **947 complete records**. The complete
reference values are preserved in `grammar_adoption_expected.jsonl`, retaining
unchanged reference lines verbatim. The fixture provenance pins its prepared manifest,
ledger, selected input and checked-in bytes. Faces are paired by name, since
their array order is not a primary-face-role contract.

## Changes made

- Correct 21 English → Source templates and the corresponding typed Source
  entry-condition syntax. Eight named cards have newly reported upstream
  parser defects; shared callers receive the same fixes. Disruptor Flute was
  already corrected upstream and was stale only in our fork.
- Lower `MayHavePlayerAction` as an explicit controller decision around the
  targeted player's actions. Keep `APlayerMayAction`'s separate implied
  player-choice contract and leave Source `You` bound to the ability controller.
- Bind revealed-card P/T and ordinary random-hand reveals to their observation
  receipts. Nissa's Revelation retains the revealed card after drawing.
- Use an observed top card for a later top-card move only when its explicit
  library player agrees and no effect has intervened. Every effect invalidates
  this short identity proof; divergent child frames invalidate it too. Tests
  cover intervening draws and another player's library. Vexing Arcanix now
  follows the reference's selected form without a general live/observed alias.
- Normalize a sole optional additional cost and its sole paid/unpaid resolution
  branch before allocating payment addresses. Entry exiles publish explicit
  ChooseObjects and Move receipts. Source Group wrappers add no receipt scope.
- Remove only proved redundant live-zone and Any-filter wrappers in fresh
  producer output. Actual movement arrivals keep their destination tests.
  Existing singleton proofs can then use the compact scalar receipt form.
- Name target-card queries and actual created-token batches explicitly in the
  affected Source templates. These are representation proposals where the old
  helper's semantics have not independently been shown wrong. Mandatory costs
  in general are not rewritten as ordinary effects.

Exactly **28 Source records** change, with matching tagged JSON and RON. The
conversion also detects and repairs one pre-existing export inconsistency:
Frankenstein's Monster's JSON still used MustCost/CostWasPaid while its RON
already used Exile/DifferenceIs. That drift is local export bookkeeping, not a
new upstream parser defect. The proof retains its original record hash.

**450 AST records** change through the parser corrections, actor/observation
fixes and bounded fresh-producer normal forms. Every other export is unchanged.
The owned AST schema and representation widths are unchanged.

## Upstream reports

Each new parser report identifies the exact upstream producing rule and the
current/requested Source expression at immutable upstream commit
`9c59b9b4cbf09c2ff99003b46f3c397b0d88566a`:

| Report | Concrete requested fix |
|---|---|
| [MTGish #298](https://github.com/i5jb/mtgish/issues/298) | Restore token/controller filters on Chief Magistrate of Mercadia, Fecund Greenshell and Kindred Charge. |
| [MTGish #299](https://github.com/i5jb/mtgish/issues/299) | Match any player's cast for Hesitation, rather than only You. |
| [MTGish #300](https://github.com/i5jb/mtgish/issues/300) | Copy onto Kimahri itself; remove Renegade Doppelganger's unprinted copy exception retaining its ability. |
| [MTGish #301](https://github.com/i5jb/mtgish/issues/301) | Restore the second optional bottom placement on Kenessos and Lantern of Revealing. |

Existing Source proposals receive separately qualified follow-ups:
[chosen-value CDA forms on #260](https://github.com/i5jb/mtgish/issues/260#issuecomment-6907994757)
and [bounded normalization forms on #275](https://github.com/i5jb/mtgish/issues/275#issuecomment-6907995123).
Our actor-decider and receipt-binding fixes are not reported as upstream parser
bugs. The reporting distinctions from the
[actionability review](mtgish-report-actionability-20261115.md) still apply.

## Remaining reference decisions

**935 complete amended records agree. Twelve retain exact, input-checked
differences**, recorded with both values in `grammar_adoption_remaining.json`.
The regression compares every other field of their complete ability lists.
Production comparison is unchanged and still reports these differences.

| Owner report | Records and decision needed |
|---|---|
| [Grammar #31](https://github.com/vishvananda/oracle-grammar/issues/31) | Mimeoplasm, Revered One: preserve/clarify the Exile-arrival qualification when counting actual exiled creature cards. Our destination guard remains; redirected arrivals must not be silently treated as successful exiles. |
| [Grammar #32](https://github.com/vishvananda/oracle-grammar/issues/32) | The five Alchemy Gates: define whether basic-land mana abilities are derived from subtypes or explicitly serialized. MTGish deliberately omits the redundant printed lines, so this is a representation-contract question. |
| [Grammar #33](https://github.com/vishvananda/oracle-grammar/issues/33) | Chandra, Gustha's Scepter, Northampton Farm, Ugin's Labyrinth, Vault 13 and Summon: Esper Maduin: finish consistent live-zone, singleton and stable-observation normal forms across the remaining reference operands. These are qualified normalization proposals. |

These reports are the handoff to the Grammar worker. Do not edit gold to match
our output or call the qualified proposals twelve newly proved gameplay bugs.
Total War, Frankenstein's Monster and Vexing Arcanix themselves now agree with
the normal forms selected in the closed Grammar #29.

## Validation and performance

All **33,529** current records export validated schema-191 AST with **zero
lowering diagnostics**. Full parser regeneration reproduces the checked-in RON.
All **182** existing audit finding identities and matched-use evidence remain
unchanged, with zero unreviewed findings.

The complete Rust workspace passes **1,595 tests**, with 10 ignored across 140
targets. Go parser tests and all eight Python comparison tests pass. The full
amendment regression and positive/negative observation-window controls pass.
Historical fixture bytes stay unchanged: 26 captured programs have 32 explicitly
reviewed before/after producer paths in the existing test-only delta mechanism.
All other fields and migration-origin mappings must still match. The old U0035
Thought Prison leaf address is checked through its complete independent U0049
reference after that frame was superseded. No diagnostic collection bypass is
retained. Commands and log hashes are in the accompanying proof JSON.

Changed token-copy vocabulary also crosses one lexical-mining threshold. That
new finding is reviewed as a false inference: “copy” does not imply a graveyard
domain when the printed choice explicitly includes a battlefield permanent.
There are zero unreviewed literal findings.

Six interleaved exports on identical original Source input, eight threads, use
immutable helpers linked to the cached pre-update and final libraries. The old
helper reproduces the original baseline byte for byte. Median wall time is
**4.21 s before / 4.17 s after**, with median peak RSS approximately **121 MiB**
for each. The small differences are within measurement noise; there is no
measurable slowdown. The measurements reuse cached libraries without rebuilding
dependencies for the benchmark.

## Oracle Blocks

`0375015` fixes CI's stale-validity race: Blockly dispatches a target dropdown
edit asynchronously, so the test must wait for the edited nested target's value
and validation, not only validity of the previous card. Its GitHub CI passed.

`d431f21` adopts the published 0.2.11 grammar and 0.2.3 model contracts. All five
Rust tests, the catalog fixture test and all **29 browser suites** pass on the
new release Wasm. GitHub CI
[43807733773](https://github.com/vishvananda/oracle-blocks/actions/runs/43807733773)
passes too. The existing 33,529-record catalog is preserved and validated.
The tested standalone HTML is deployed at https://tetrarcum.com/blocks; an HTTPS
download matches its build bytes exactly.

Scratch evidence, release assets and API receipts live under
`/mnt/vizier/grammar-adoption-20261116/`. No Cantrip branch, engine feature,
reservation or worker queue changed. The 57-card backlog remains paused.
