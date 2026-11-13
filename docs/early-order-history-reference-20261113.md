# Early-set ordering, historical events, and characteristic definitions

The current combat is not the entire turn. Total War's trigger destroys untapped
creatures that did not attack in that combat, even if they attacked in a prior
combat. Its English template and frozen Source now use `Not(AttackedThisCombat)`;
explicit “didn't attack this turn” predicates elsewhere retain their old meaning.
The existing `ThisCombat` IR/history contract needs no schema or engine change.
Reports: [Oracle Grammar #17](https://github.com/vishvananda/oracle-grammar/issues/17)
and [MTGish #286](https://github.com/i5jb/mtgish/issues/286).

Shapeshifter's final sentence defines its power and toughness, functioning in all
zones in layer 7a (CR 604.3, 604.3a, 613.4a). Its template and frozen Source now use
`CDA_PowerToughness`, instead of an ordinary battlefield-only layer-7b effect.
An unchosen number is zero (CR 107.2). No other self-setting effect is reclassified.
Reports: [Oracle Grammar #18](https://github.com/vishvananda/oracle-grammar/issues/18)
and [MTGish #287](https://github.com/i5jb/mtgish/issues/287). Both wrong outputs were
also verified against upstream grammar blob `88c75d8a2d794d558e7dc8b7fba443fa17125bf4`;
our upstream and Oracle snapshot pins were not advanced.

A fresh full parser export exactly matches the corrected 33,529 ordered cached
records; only these two records changed. All 182 capture-audit identities and
matched uses are unchanged. The literal miner flags the singular “power is ...
and toughness is ...” wording because most CDA templates use plural “are”. The
one exact new finding is reviewed with that explanation; the detector is intact.
Oracle Grammar retains ownership of the canonical export and its update ledger.

The same batch corrects four backend interpretations without changing their IR:

- An explicitly ordered player process completes one player's actions before the
  next player's choices. Eureka and Thieves' Auction retain per-player transition
  receipts. Optional-result locals are scoped to that player's iteration, so a
  later decline cannot inherit a previous successful move or prevent termination.
- A “before blockers” or “before combat damage” deadline does not reopen in a later
  combat. History keeps it closed even when the earlier relevant step was skipped.
  Explicit “during combat before blockers” windows keep their separate contract.
- “When you lose control” also watches a battlefield departure, using the departing
  incarnation's last-known controller. This is distinct from only watching a
  `ControlChanged` event while the permanent remains on the battlefield (Seraph).
- A historical damage event can name an opponent who subsequently left the game.
  Historical opponent membership does not discard that recipient using today's
  live-seat iterator. Ordinary current opponent queries are unchanged (Whirling
  Dervish).

## Native proof

The six actual generated card modules fail six retained counterexamples before
these fixes. Afterward all nine retained scenarios pass, including Seraph's
simultaneous-death and Total War's ordinary/tapped-creature controls. Two additional controls prove Eureka stops when everyone declines and Thieves'
Auction completes its ordered tapped entries; all eleven tests pass. No generated
effect body was rewritten by hand. The Shapeshifter harness supplies printed
`*/7-*` defaults of `Some(0)/Some(0)`, exactly as the native Oracle builder does;
using the full Source module's numeric fallback 0/7 would mask its missing hand
CDA. The identical printed-stat wrapper is used before and after.

Copy the harness from `tests/fixtures/early_order_history_native/` into an isolated
native checkout's `crates/cantrip-script/tests/`. Copy its scenarios to
`tests/fixtures/early-order-history/` below that crate. Generate the seven complete
card modules there with `ast cantrip-module`, using the combined
`early_order_history_20261113.ronl` and `early_combat_cda_reference_20261113.ronl`
fixtures. Run `CANTRIP_VERIFY=1 cargo test --profile checked -p cantrip-script
--test early_order_history_regressions`. These definitions only extend the test
table; they do not register unreviewed production cards in the foundation.

Stage receipts under `/mnt/vizier/early-set-campaign-plan-20261113/` retain exact
module hashes, the old/fixed native logs, parser output, capture audit, and test
results. Full driver validation and normal independent card review are still
required before publication.
