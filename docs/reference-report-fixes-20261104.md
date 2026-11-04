# Reference report corrections, REF-008 through REF-012

The newer report groups reproduce on current fork/main, independently of the older immutable reference used by
oracle-grammar. The fixes cover the English parser and two Rust lowering errors; upstream does not contain our Rust
normalizer, so the three English defects were reported there separately.

| Report | Correction | Upstream |
| --- | --- | --- |
| REF-008 | Keep the enclosing entry trigger's cast-X binding when constructing modal branches. Orcus's two effects and target-count/total-mana-value constraints all retain that binding. Activated modal abilities keep their own announced X. | Local Rust lowering |
| REF-009 | Pass the captured creature/even-mana-value filter to Gyruda's milled-card selection. | [#192](https://github.com/i5jb/mtgish/issues/192) |
| REF-010 | Exclude battlefield objects from `CardEffect` populations. Preserve the independently controlled battlefield and stack populations. `SelfEffect` remains unchanged. | Local Rust lowering |
| REF-011 | Remove the erroneous optional wrapper from Diregraf Horde's token creation and Glissa's Retriever's self-exile, preserving the reflexive triggers and later up-to target choices. | [#193](https://github.com/i5jb/mtgish/issues/193) |
| REF-012 | Use any player in the four conditional spell-cast productions. Preserve conditions, event subjects and independent abilities. | [#194](https://github.com/i5jb/mtgish/issues/194) |

A complete parser run found another caller of the spell-trigger defect: **Vision, Synthezoid Avenger**. It now has
its own semantic regression alongside the four reported cards.

The parser changes exactly eight source records: Gyruda, the two mandatory-action cards, and the five spell-trigger
cards. The source identities, layouts and ordering of all 33,529 records are preserved. RON lines, JSON lines and
pretty RON change the same eight records; typed binary/JSON round trips agree.

The shared `CardEffect` correction affects 17 normalized card records, including the nine reported cross-zone
characteristic effects. Together with Orcus and the eight parser corrections, 26 complete unified records change.
All 33,529 unified records export and validate at schema 176. No source forms, IR variants or serialization layouts
are added by this repair.

Validation:

- Fresh English parser regressions: seven reported-card cases fail before the repair. The additional Vision caller
  was discovered by the full corpus delta and is also tested.
- The complete Go suite passes. All 37 guarded audit findings retain exactly their prior matched-evidence hashes
  in a fresh 33,529-card parse. The audit's current grammar fingerprints are refreshed only after that comparison;
  unchanged parser-code fingerprints are checked separately.
- The AST regression has two genuine baseline assertion failures and a passing activated-X control. The initial
  malformed synthetic activation cost is excluded from the credited baseline.
- All 1,023 AST/compiler tests pass, with two existing ignored tests, across 104 test summaries. Six focused tests
  cover the complete reported cards, modal X provenance, controlled populations and the additional Vision caller.
- Full source identity/layout verification and both before/after unified exports succeed for 33,529 records.

The oracle-grammar repository was checked through `94bcd76`. Its catalog still ends at REF-012; the newer hand and
combat-binding reports add no further reference bugs. Its immutable reference, gold, scorer and pins are untouched.
The separately reported Ydwen Efreet producer correction ([#191](https://github.com/i5jb/mtgish/issues/191)) remains
outside this five-group repair.

Full receipts are in `~/card-lane/pipeline-unlimited-20261103/integration/reference-008-012-current-audit/`.
