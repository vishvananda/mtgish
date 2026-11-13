# Defender history, destruction choices and immutable arrival receipts

## Shared native foundation

The common native branch adds record-only `AttackAssigned` facts when a creature
enters attacking or an effect reselects its combat target. The normal `Attacks`
event stays reserved for declarations. `Query::defending_player` first reads the
current attacker, then the latest declaration or assignment for the same object
identity. Removal, regeneration, leaving the battlefield, and planeswalker
removal no longer erase the defender. A new zone incarnation inherits no combat
history. Reassignment also records all members of an attacking band.

This repairs the shared prerequisite for Floral Spuzzem, Mindstab Thrull, Necrite
and Kjeldoran Guard. Cards still need independent review against the new base;
in particular, replace manual declaration-only history scans and any temporary
card-specific workaround with the appropriate shared query and combat scope.

Three forced native positions fail before the change and six entry/combat tests
pass afterward, including multiplayer target reselection and zone incarnation.
The existing 28-byte event-size limit still passes. The retained Floral Spuzzem
and Necrite scenarios fail with the old engine and pass with the new engine using
actual generated card definitions. No attack declaration was fabricated.

`Query::can_destroy` now supplies the same destruction permission to the core
verb and direct destruction choices. Drop of Honey and Porphyry Nodes first
compute least power over all creatures, then ask this permission within that
pool. Ordinary independent choose-then-destroy instructions keep their original
choices. Tests distinguish destruction permission from target legality, reject
stale/graveyard objects, and check indestructible being added and removed in the
layers. The card backend uses this core query instead of hand-reading the keyword
in those direct-choice pools, satisfying the card lint policy.

The already independently reviewed Infernal Darkness mana-and-life cumulative
upkeep prerequisite was also moved into this common branch. It remains separate
from the card's commit. No numeric domains were widened.

At native `bcf28136db4fa063161515cdaa402db3a6f55fb3`, the full `tools/test.sh`
passes: 23,680 Rust tests, 15 ignored, 246 targets; 1,525 managed tests, one existing
skip. Three interleaved optimized 128-game runs for each binary complete every
game. Median engine time is 48.74 microseconds/decision before and 46.72 afterward;
all six runs allocate 29.34 KB/decision. These are local measurements, not a claim
of a universal speedup. The foundation PR is cantrip-rs/cantrip-rs#2875.

## Backend arrival counts

A transition's `After` identities retain their original arrival zone after later
moves. Matching such a receipt against that zone must not additionally require
`became.is_none()`. The former predicate erased an exile from a later count once
the card returned to hand. Explicit `ObjectSet::InZone` membership still requires
a live incarnation; a card that left exile is not still exiled.

`receipt_arrival_counts.ronl` captures both contracts. A native regression runs
real generated code: exile a creature card, return it, then draw the count of
creature cards exiled this way. Restoring the exact former live-identity predicate
makes the test expect six hand cards but get five; the corrected generated module
passes. A companion conditional using “still exiled” draws nothing in both cases.
The old predicate reproduction is an explicit mutation of the generated module,
not a claim that a fresh compiler export is a historical binary artifact.

No IR variant or schema change is needed. Other zone-filter contexts retain
their current-object checks. Frozen Source and the Oracle snapshot stay fixed.

## Reproducing the native regressions

The harnesses and two retained combat scenarios are under
`rust_syntax/normalize/tests/fixtures/early_backend_runtime_20261113/`.
In an isolated native checkout at the common foundation:

1. Copy the two Rust harnesses into `crates/cantrip-script/tests/`.
2. Create `tests/fixtures/defender-history/` within that crate and copy the
   floral/necrite scenarios there. Generate `floral_spuzzem.rs` and `necrite.rs`
   with `ast cantrip-module data/mtgish.lines.ron <name>`; the output is a complete
   card definition, used only in the test table.
3. Create `tests/fixtures/receipt-arrival/` and generate `arrival.rs` and
   `membership.rs` with `ast cantrip-module` using
   `tests/fixtures/receipt_arrival_counts.ronl` and each of its two card names.
4. Run `cargo test --profile checked -p cantrip-script --test
   defender_history_regressions --test receipt_arrival_regressions`.

The stage receipts retain exact source/log hashes and the old/fixed counterexample
runs. These probes do not register unreviewed production cards in the foundation.
