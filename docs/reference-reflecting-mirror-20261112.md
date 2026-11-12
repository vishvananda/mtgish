# Reflecting Mirror: target restriction and computed X

Confirmed and reported as [MTGish issue 274](https://github.com/i5jb/mtgish/issues/274).

The reference required the target spell's mana value to equal X, and separately
defined X as twice that mana value. This synthetic restriction excludes every
nonzero-mana-value target. The Oracle text requires a spell with a single target;
it contains no mana-value restriction on that target.

The correction changes only the target selector to
`TargetSpell(TargetsOnlyASingleTarget)`. It preserves the resolution-time test
that the spell targets you, the restriction that the new target is a player,
and the definition of X as twice the target spell's mana value. In particular,
the “if that target is you” condition is not moved into target legality.

The English template and all three current Source representations are corrected.
A fresh parser regression compares the complete corrected Source. A typed IR
regression checks the unrestricted single-target selector and the separate
twice-mana-value definition, retaining the conditional retargeting body. The
uncached Go suite passes (72.980 seconds), and the new IR regression passes.
AST schema 190 is unchanged.

For the Oracle Grammar worker: this is an additional defect discovered during
foundation validation, rather than a newly unresolved question in the previous
53-card report. Adopt the corrected reference when updating the MTGish pin.
No Oracle Grammar files or pins were changed here.

The frozen early-set membership and original `source.ronl` are preserved.
`full-scope/source-corrected.ronl` now differs on two records: Glyph of Delusion
and Reflecting Mirror, with both issues recorded in `source-corrections.json`.
Mirror still has explicit computed-X and player-retargeting backend gaps until
those interfaces actually work. Do not compensate for the old reference in the
engine, or report Mirror as generation-ready merely because its selector is fixed.
