# Two-Headed Giant counts heads, rather than flips won

The English template for Two-Headed Giant's attack trigger emitted
`NumCoinFlipsWonIs` for both the heads and tails clauses. It now uses the
existing `ValueIs(TheNumberOfCoinsThatCameUpHeads, ...)` composition, comparing
against two for both heads and zero for both tails. Double strike and menace
remain separate, with the same targets, amounts, timing and duration.

The complete typed program now has two `Expr::HeadsFlipped` reads of the same
`FlipCoins` result. They replace exactly two `Expr::CoinFlipsWon(true)` leaves.
Both count comparisons, both grants and every other operand are unchanged.
This is a prerequisite for explicit coin policies: it removes a false winning
interpretation from the source rather than teaching semantic IR a card name.
The backend still uses printed text to determine coin protocol; removing that
dependency remains part of the pending typed coin-policy work.

The template regression fails with the previous grammar and passes after the
correction. All 33,529 records are freshly parsed; supplemental records are
restored to their existing order using variant/name/occurrence identity. Exactly
one complete record changes in each of the three Source formats. All 33,529
normalized records validate; their complete JSON comparison proves precisely
the two head-count substitutions above. Card-code roundtrip and canonical
idempotence pass. With actual Oracle text, the complete card generates without
gaps and uses physical coin faces. No schema, AST type, engine feature or
numeric-range change is needed; normalized schema remains 184.

All fourteen browser suites pass, including all 33,529 block roundtrips, typed
ID-port comparisons and Wasm validations. The code suite also checks the
corrected card directly among sixteen examples. The full Go suite passes. The full native IR/AST/Wasm suite passes 1,463 tests
with zero failures (15 existing opt-in audits ignored). All 182 existing grammar
finding IDs, complete matched-use records and review reasons are unchanged.
Only the changed grammar's bound input fingerprints are refreshed; literal
reviews, detector implementations and Oracle-grammar's pins/gold are untouched.
Evidence is in `/mnt/vizier/ast-review-20261109/coin-prerequisite/`.

The same wrong heads/wins composition was read from upstream grammar blob
`5d279258e4a3006f2efce5dd943c6eca641ab03e`. Existing issues were searched for the
card and for heads/won and coin/tails terms. The separately reported duplicate
second-ability capture from issue 1 is independent of this count correction.
Reported upstream as [i5jb/mtgish#238](https://github.com/i5jb/mtgish/issues/238),
with tested fork correction `a620cf8`.

This additional prerequisite is separate from checkpoint105's 29 confirmed
reference defects. It does not complete the remaining coin-policy,
destination-state, Rust naming/counter-source or layout work.

Publication: fork main contains `a620cf8`. Release `20261109-a620cf8` is live at
`/blocks`, SHA-256
`8dd92af4d0c724680215303c2b6b0210f6797b15ffccc0f193f939f818335dc9`.
The HTTP file matches the tested artifact, and the live code suite passes all
sixteen Wasm examples, including the corrected physical-face count. Machine
receipt: `coin-prerequisite/release.json` under the review evidence directory.
