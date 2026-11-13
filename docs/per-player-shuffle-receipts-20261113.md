# Per-player shuffle receipts

Winds of Change, Molten Psyche and Whirlpool Warrior use two player phases:
each player shuffles their own hand, then each player draws their own shuffled
count. Source already expresses those phases correctly. The lowerer previously
read the entire preceding loop's movement receipt as one draw amount for everyone.

Bind each drawing player before reading that amount, and read the matching
`MovedBy { result, phase: Before, player: Bound(binding) }` receipt. Count the
actual original cards shuffled, retaining identities through the zone change.
Keep the shuffle phase separate from the draw phase; do not shuffle and draw for
one player before the next player has shuffled.

Three frozen-face regressions fail before the fix and pass after it. Credit
Voucher and Whirlpool Rider retain their ordinary single-player transition
receipts. Psychic Spiral still makes its target mill the caster's shuffled count;
the mill target does not redefine the shuffle's performer. All six fixtures
validate under unchanged schema 190. The new family and related card-action and
upstream-contract suites pass 28 tests.

This uses existing IR and native receipt concepts; it does not widen quantities
or require a new engine interface. Adopt the canonical-reference amendment
through Oracle Grammar's normal update ledger.
