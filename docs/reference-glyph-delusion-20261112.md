# Glyph of Delusion: permanent grants and ordered target availability

The early-set foundation probe found three independent problems. None needs a
new AST form or a wider numeric type.

| Layer | Finding | Correction |
| --- | --- | --- |
| English Source template | The two granted abilities expired at end of turn even though Oracle specifies no duration. | Produce `CreatePermanentLayerEffect`, lowering to `Duration::Indefinite`. |
| Cantrip generator | A source-only conditional `DoesNotUntap` rule was rejected despite existing support for granted static abilities. | Use the same conditional internal-rule-keyword path as `CantBlock` and `CantBeBlocked`. |
| Native availability | Required target slots were tested independently with an empty earlier-target list. This hid the spell despite a Wall having blocked a legal creature. | Reuse the bounded ordered announcement search for multiple slots. The actual announcement still supplies targets in slot order. |

The local Oracle wording gives the creature the untap restriction while it has
at least one glyph counter, plus an upkeep trigger removing a glyph counter.
Neither grant has a duration. They remain after the casting turn, and the
upkeep trigger belongs to the recipient's controller. The source-reference
error is reported at [MTGish #269](https://github.com/i5jb/mtgish/issues/269).
The generator and native availability fixes are local backend repairs.

Only Glyph's record changes in each active Source representation: line RON,
tagged JSON and pretty RON. The other 33,528 line records remain byte-for-byte
unchanged; historical snapshots remain untouched. The targeted English grammar
rule and the audit's reviewed/input hashes are updated, preserving original
review provenance. The Go parser regression parses the entire Oracle text;
Rust tests verify the indefinite typed grant, agreement between Source
representations, and generated native static-ability code.

The generated runtime scenario casts the real generated card after an actual
block, gives the opponent's creature two counters, follows it through two
later upkeeps, and verifies it untaps only after both counters are gone. It
passes with the other 43 foundation scenarios. All 189 currently gap-free
scoped modules compile against native foundation `c97d4ac45` without an overlay.

The native repair also serves Goblin Welder's dependent target slots. Its
regressions cover legal and impossible complete announcements, backtracking
past a first target without a partner, optional empty-slot positions, and an
unannounced X. It reuses the existing search budget and conservative exhaustion
behavior; ordinary single-slot checks keep their direct path.

For the Oracle Grammar worker: adopt the corrected Source record when advancing
the reference. Do not normalize an end-of-turn grant into an indefinite grant
in a comparison adapter, and do not infer that the source bug was a lowering
error. Frozen early-set membership remains 286. The audit preserves its original
input and records a one-record corrected input in
`/mnt/vizier/early-set-foundation-20261112/full-scope/source-corrections.json`.
