# Published Schema 191 adoption

The model dependencies now use Oracle Grammar's published oracle-ir and
oracle-ir-contract 0.2.0. Schema 191 appends HistoryWindow::LastTurnOf(PlayerRef):
the player's most recent completed turn, excluding the current turn and empty
before they have had a turn. The existing ObjectWindow reader and new event
history reader share one implementation. No extra per-card field, numeric width,
or runtime history cache is introduced.

The 0.1.0 and 0.2.0 release manifests have identical source_oracle and population
identities and identical oracle-source.jsonl.gz checksum:
a44b7e9a00570d2c627188fcdf740921caa60c14b16d90fc4bc2302cf99a8ee7.
The new faces bundle checksum is
2535b3857b0f737f1c327302e646b48b49c372fc1aef5e8a517a661333a6c9e9.
The existing data/mtgish.lines.ron and data/oracle.json are unchanged. This adopts
the published model and reference correction without taking a new Oracle snapshot
or importing new cards into the campaign.

Arboria's legacy lowering had conflated a player putting a permanent onto the
battlefield with a permanent entering under their control. Those are distinct.
The first now uses EventHistory(PlayerEvent::PutsOntoBattlefield) in LastTurnOf,
with the existing native ZoneChange.put_by receipt. The second keeps its
controller-based PlayerDid interpretation. The frozen Arboria Source now matches
the entire published Schema 191 reading exactly, including bindings and types;
the fixture records that reference. No additional upstream parser/reference bug
is asserted for this already-published correction.

The retained native scenario uses Flash to put Flickerwisp onto the battlefield
on another player's turn. Its delayed ability returns that player's old Memnite
under their control. They neither cast nor put a permanent there that turn, so
Arboria must protect them. A third player who played a land remains a legal
attack destination, keeping an actual declaration question open for the refusal;
the test attacks them as a positive control. A later land play by the protected
player permits attacking that player too. The same complete scenario FAILS old
production output at the refusal and PASSES both new raw output and the actual
new production-row module, using the same native core. It needs no new engine API.

Install the .cantrip fixture under
crates/cantrip-script/tests/fixtures/arboria-last-turn/ and its harness under
crates/cantrip-script/tests/. Generate Arboria from the retained
arboria_schema191.ronl with ast cantrip-module and put that module at
fixtures/arboria-last-turn/arboria.rs. Run the arboria_last_turn_regressions target.
The first exploratory script used unavailable Vedalken Orrery and was replaced
with supported Flash. The initial two-player refusal reached no attackers prompt
when every attack was forbidden; the retained three-player scenario corrects
that test precondition rather than weakening the refusal.

This batch also identifies LastingBlockRule.allows by its native API position
for generated documentation. Its callback implements a resolved continuous rule
(CR 611.2c), not a printed static ability (CR 604.1). The same callback in a printed
block_rules entry still cites 604.1. This repairs Tower of Coireall's citation
blocker without altering its rules behavior.

Validation passes 1,654 generator tests (20 ignored, 220 targets), 3 WASM bridge
host tests, the exact canonical Arboria comparison, and the native old-fail/new-pass
scenario. Native API fingerprint stays f0d4b95df473e8a32dfb25d5d7a9b5c3e5f8af332b8e9e92aa47e5f1bb4a31ee
for the current validated campaign base f92dfc37d. The newer rebased foundation
is a separate rollout and must refresh that pin only when adopted.

Across 33,529 generated rows, ignoring metadata, 29 cards change only citation
comments. Seven cards change code: Arboria and six created-token references
(Arcane Artisan, Dance of Many, Dual Nature, Saproling Burst, Tetravus, Tombstone
Stairwell). Those six retain the necessary token-only intersection from 639c2ea9;
linked-ability identity remains unresolved in Oracle Grammar #20. None of these
36 changed rows is an approved campaign card. Only Arboria and Tower of Coireall
are eligible for a retry; Tetravus remains held. No card is reapproved by this
migration, and no card or data bundle is automatically added to intake.
