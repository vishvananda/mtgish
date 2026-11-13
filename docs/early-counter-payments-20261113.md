# Variable source-counter payments

Tetravus's optional upkeep payment used the existing `may_pay_x` interface with
`RemoveAnyNumberOfCounters`. The resolution picker offered four counters when
only three were available, capped actual counts at the mana picker's unrelated
limit, and opened a mana-ability window for a cost containing no mana.

The foundation shares `source_counter_x_limit` between resolving payments and
storage-land activations such as Bottomless Vault. It reads counters on the live,
unphased source, reserves fixed removals, and accounts for repeated independent
atomic cost components. The bound is recomputed after mana abilities when the
cost includes mana. Counter-only costs offer no mana abilities. Mana X retains
its existing picker limit; no numeric field is widened. Paying zero still returns
`Some(0)` and declining returns `None`; neither changes the source's cast X.

`may_pay_counted` is not a substitute for this interface: it does not announce
this variable source-counter count. A fixed `remove_counters` cost holds `u8` and
must not be used to truncate an arbitrary available count. Triskelavus has a
fixed one-counter activation and is not the second consumer of this bound;
the storage lands are.

Five initial core regressions produced three old-code failures (the excess
count, mana window, and unrelated 1,000 limit), with payment/zero controls passing.
The fixed code passes those probes. The final suite also covers departed/phased
sources and two explicit atomic counter-removal components. `COUNTERS.and(COUNTERS)`
coalesces the same variable cost; the latter test deliberately uses two entries
in `Cost.components` to represent two independent payments.

The retained Tetravus scenario now fails at the illegal four-counter choice on
the old engine and passes on the fixed engine with the same generated module.
It pays two, creates two flying artifact creatures, and reclaims both later.
Two later assertions in the stopped worker's scenario also needed correction:
colorless permanent colors are absent in the dump, and “can't be enchanted” does
not forbid targeting with an Aura spell. The test casts Flight at a token and
checks that it fails to attach and ends in the graveyard; a second Flight attaches
to Tetravus as the positive control. This is the distinction documented by native
`layers::enchanting_forbidden` (CR 303.4a, 303.4i).

The backend uses a counter-removal question instead of the generic mana-cost
question, so it no longer calls this “pay {0}”. The amount picker and payment
remain engine-owned. The native runtime fixture and harness are in
`rust_syntax/normalize/tests/fixtures/early_counter_payment_native/`; generate
Tetravus from the existing `early_set_generator_gaps.ronl` fixture and register
its `EXTRA` token definitions in the test-only table.

The same foundation batch projects the already-existing
`NoCombatDamageAssignment` effect through `granted_abilities`, including its
expiry. Orcish Squatters needs this visible state after accepting its trigger.
The host regression checks every viewer, serialization, and removal of the
label with the effect. The client already displays all granted-ability lines;
no new wire message or client control is introduced.

The compound mana replacement grouping defect remains separate and unresolved:
[Oracle Grammar #19](https://github.com/vishvananda/oracle-grammar/issues/19).
Naked Singularity and Reality Twist remain blocked until their single replacement
can offer a genuine output choice; distinct labels alone are insufficient.

Full validation after filesystem recovery passes 23,692 Rust tests (including
21,052 scenarios) and 1,525 managed tests; 15 Rust tests are ignored and one
existing managed test is skipped. The backend suite passes 1,651 tests with
20 ignored. Card review and confirmation still run independently after rollout.
