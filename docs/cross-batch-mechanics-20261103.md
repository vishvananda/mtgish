# Shared mechanics across early-set batches

The publication audit found conflicts in 51 of the 55 pairs of open batch PRs (11 PRs). Each batch had passed
its own integration checks; publication had not checked compatibility with the other pending batches.
The exact GitHub heads and `git merge-tree` results are retained in the campaign's
`publication/shared-reconciliation/conflict-audit.json`.

[PR 1109](https://github.com/cantrip-rs/cantrip-rs/pull/1109) and
[PR 1363](https://github.com/cantrip-rs/cantrip-rs/pull/1363) conflict in Kor Firewalker, Cartouche of Solidarity,
and the shared effects and triggers modules. The trigger differences include independently introduced names,
different subject payloads, and overlapping caller migrations. Commit patch IDs cannot establish semantic
equivalence between those versions.

At upstream `b68ca30f9b385c76ab96ff997b6b4e9841c56a10`, both needed recipes already exist:

- `triggers::a_player_casts_a_spell_of_color::<COLOR>` checks any player's cast spell, using the source's color words.
- `effects::may_pay_to_gain_life(g, item, cost)` pays an optional resolution cost and gains one life if paid.

Ivory Cup and Farmstead already use the payment recipe upstream. Kor Firewalker uses the color matcher.

| Card | Current publication | Duplication to reconcile |
| --- | --- | --- |
| Ivory Cup | Closed PR 1106; already supported upstream | Retain the current upstream shared implementation. |
| Crystal Rod | PR 1109 | Private payment body; a separately introduced generic color matcher. |
| Throne of Bone | PR 1109 | Another payment function and another color-trigger wrapper. |
| Wooden Sphere | PR 1364 | Private payment body and another color-matcher name. |
| Iron Star | PR 1363 | Private payment body and a new red-only matcher. |

New publication safeguards check pending PR compatibility before integration tests and immediately before a
push. A promotion gate also requires new shared prerequisites to belong to the common validated helper pin
or already be upstream. The latter prevents an independently named shared implementation from bypassing
the compatibility check merely because Git can merge it.

Existing PRs still need consolidation and validation against one common foundation. Their original reviewed
candidates and publication manifests remain retained; adding safeguards does not repair those PRs by itself.
Publishing is paused for this reconciliation. Intake and card workers continue.
