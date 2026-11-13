# MTGish #120 and #125: contract clarification, not parser defects

Reviewed the maintainer responses on 2026-11-13 against our frozen Source and
its current lowering. This review does not advance the upstream pin or Oracle
snapshot.

## Spikeshell Harrier — #120

[Maintainer response](https://github.com/i5jb/mtgish/issues/120#issuecomment-6840758311):
`ReducePlayersSpeed` has an intrinsic floor of 1. Speed 0 represents a player
whose speed has not started; reducing speed must not start their engines.
The earlier allegation that omitting an explicit floor necessarily permits a
value below 1 was incorrect. There is no upstream parser defect to reopen.

Our frozen Source spells the operation
`ReducePlayersSpeedToNoLessThan(player, Integer(1), Integer(1))`; the owned IR
already has `ReduceSpeed { player, amount, floor }`, with both numeric operands
1. That is a compatible, explicit expansion of the operation's meaning. No AST
variant removal, schema change, reference replacement or numeric widening is
needed. Keep the minimum in the interpreter/backend contract when adopting an
upstream two-argument spelling later. Do not conflate this issue with #109,
which concerned whose speed the card reads/reduces.

## Maelstrom Archangel Avatar — #125

[Maintainer response](https://github.com/i5jb/mtgish/issues/125#issuecomment-6840799027):
`ManaCostsOfCombatCreatures` already denotes creatures that participated in
combat earlier in the turn. Its operand is evaluated historically, not as a
fresh battlefield query. The original upstream allegation misread that contract.

Our `CreateCopiesOfRandomCardsWithManaCosts` lowerer did make the erroneous
interpretation: `self.objects(creatures)` produced a current battlefield
population filtered by damage history. It omitted departed creatures and tested
controller/type predicates at resolution instead of at the relevant damage event.

The correction must select the sources of combat-damage-to-player events in
`HistoryWindow::ThisTurn`, with the creature/controller predicates inside the
historical event matcher. Return distinct original object identities, so double
strike, multiple combats and multiple recipients do not create extra copies for
the same creature. Continue using the existing mana-cost comparison and copy
operations. The existing `ObjectSet::FromHistory`, `EventSpec::Damage` and set
union provide these concepts; no new model variant or backend feature is required
for the IR correction. Do not substitute a search across current zones, sum mana
values, count every damage occurrence, or require that the creature remain under
the player's control.

Oracle Grammar owns the canonical reference. Report this as a reference/lowering
correction there; preserve its pinned export and apply any reference amendment
through its normal updates ledger, rather than editing exported records in place.

## Completed validation and handoff

The two-face frozen fixture reproduces the original error on Maelstrom Archangel
Avatar, while Spikeshell Harrier already passes. With the history correction,
all 34 tests in `rejected_history_reports`, `reduce_card_actions` and
`cast_history` pass. Both faces validate against schema 190. The Source records
and grammar require no change for these two issues.

[Oracle Grammar #14](https://github.com/vishvananda/oracle-grammar/issues/14)
tracks the reference amendment and the distinguishing damage-time cases.
Oracle Grammar's checkout and its updates ledger were left to its owning worker.
