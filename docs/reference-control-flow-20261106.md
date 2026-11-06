# Printed conditions, target counts and added phases

Four English templates selected the wrong existing operand. The current-producer review confirms each error against complete local Oracle text and the same template on upstream main `bdac5ca64c01f1acb02405839e3633c98c48bcf3`.

| Card | Correction | Upstream report |
| --- | --- | --- |
| Satya, Aetherflux Genius | Sacrifice the created token when payment is declined. `Unless(CostWasntPaid)` introduced a double negation; use `Unless(CostWasPaid)`. | [208](https://github.com/i5jb/mtgish/issues/208) |
| Titan Hunter | Check zero creature deaths this turn. The old condition checked at least one. Use the existing count comparison `EqualTo(0)`. | [208](https://github.com/i5jb/mtgish/issues/208) |
| Bridgeworks Battle | Require the creature being pumped; allow zero or one fight partner. The two target constructors were reversed. | [209](https://github.com/i5jb/mtgish/issues/209) |
| Resurgence | Add the printed following main phase as well as combat, using the existing combat-and-main form. | [210](https://github.com/i5jb/mtgish/issues/210) |

No public schema or source variant is added. In particular, Titan Hunter uses the existing number-of-permanents-died comparison; the source model does not have a generic negated Condition constructor. Its creature/death/this-turn history, end-step event, damage recipient and independent activation remain unchanged. Bridgeworks retains both control filters, the pump recipient and fight operands. Resurgence retains its controlled-creature recipients, both keywords and duration. Satya retains its copy, energy gain, payment amount and actual-created-token receipt. Its multiple-created-token amount contract remains unresolved and is not certified by this polarity correction.

Actual English-parser and typed-IR regressions fail before all four repairs. The positive creature-death control remains unchanged. A fresh full parse and typed export change exactly these four records, only at the listed counts, phase and condition fields. Every other complete record and operand remains identical. The checked-in RON, pretty RON, JSON and regression fixtures are regenerated from fresh parser output.

All 185 audit findings retain identical complete usage. The existing Satya `TotalManaValueOfPermanents`/“total” literal finding receives one exact new review key because the output's payment polarity changes. Its old review history is retained. Complete source/IR comparison proves the amount expression itself is unchanged; the new lexical review explicitly leaves the multiple-token amount contract unresolved. There is no new semantic-audit exception or comparison alias.

The full Go suite and 1,250 AST tests pass, with three existing ignored tests. All 33,529 records remain structurally preserved and semantically complete. Validation receipts are in `/mnt/vizier/reference-control-flow-20261106`. None of the changed records' faces overlaps the 601 pending publication cards at `4a007d87`. The catalog has 89 entries: 36 reported corrections fixed in the current producer and 53 pending. Withheld erased-domain cases and separate contract questions remain visible.
