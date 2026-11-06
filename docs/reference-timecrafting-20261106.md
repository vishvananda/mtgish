# Timecrafting retains the printed existing-counter requirement

Timecrafting's put-counter mode requires a permanent with a time counter on it. The English template captured that noun but replaced it with `TargetPermanent(AnyPermanent)`. It now uses the captured `Permanent[0]`, which supplies `HasACounterOfType(TimeCounter)`. A permanent with no time counters is ineligible even when X is zero; typeless battlefield permanents with a time counter remain eligible.

The remove mode remains unqualified. Both suspended-card alternatives retain their Card/Exile domain and Suspended filter. Both modes retain their target counts, X amounts, counter types and operation operands. The full corpus comparison proves that exactly one source record and one IR record change, exclusively at the put-mode permanent target filter. There is no schema or public type change.

The same template defect was confirmed on upstream main `bdac5ca64c01f1acb02405839e3633c98c48bcf3` and reported in [issue 206](https://github.com/i5jb/mtgish/issues/206). The actual English-parser regression and a separate typed-IR regression fail before the correction. Checked-in source formats and the local regression fixture are regenerated from fresh parser output. All 185 template-audit findings retain identical complete usage evidence; only the reviewed grammar-input hashes change, with no added exception or literal review.

The full Go suite passes, as do 1,245 AST tests with three existing ignored tests. All 33,529 corpus records remain structurally preserved and semantically complete. Receipts are in `/mnt/vizier/reference-timecrafting-20261106`. Timecrafting has no overlap with the 601 pending publication cards at `4a007d87`.

The follow-up catalog has 88 entries: 31 reported corrections fixed in the current producer and 57 pending. Solid Ground and the other event-domain cases withheld by the scoped permanent-selector correction remain pending.
