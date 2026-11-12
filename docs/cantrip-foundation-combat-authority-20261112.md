# Combat reassignment authority — first stage

The changing-combat family remains in progress. This checkpoint supplies the
native mutation authority; it does not enable generation of False Orders,
General Jarkeld or Sorrow's Path, or count them as ready. The frozen scope stays
217 ready / 69 gapped, with 86 diagnostic labels, out of 286 cards.

`Game::change_blocks(removed_creatures, ending_pairs, starting_pairs)` applies
one graph change before collecting its block triggers. It distinguishes ending
an association from removing a creature from combat. A shared unchanged pair
does not trigger again. A removed and reassigned blocker does, including when
it returns to the same attacker. Banding expands new relationships. Independent
blocking and blocked status survive the removal of their last association.
The authority enforces live creature roles and the existing defending-player
relationship; it is not a new blocker declaration.

The core records `CombatFirstStrike` as the first damage step begins, without a
trigger mask or per-object storage. Reassignment uses that combat's record when
a blocker's previous attacker is gone. A blocker that subsequently loses first
strike cannot deal damage twice; having double strike now still permits damage
in the regular step. Another combat or a new incarnation does not inherit the
fact. The existing small-event layout guard passes.

Twelve new native tests cover relationship and trigger identity, a shared pair,
an independent surviving blocker, band expansion, a tapped blocker, invalid
creature roles, first-strike eligibility after an attacker leaves, another
combat, and rollback/replay. The complete checked core library gates pass:
1,512 rules tests (one ignored), 86 state tests and 12 API tests. The full native
workspace and final generator gates remain required at the shared-family
milestone; these focused core gates do not replace them.

Rules evidence: CR 509.1g–h, 509.3a–d, 510.4 and 702.22h in the current
[Comprehensive Rules](https://magic.wizards.com/en/rules). The
[official 2019 Jarkeld update](https://mtg-jp.com/reading/publicity/0032495/)
also preserves its explicit legality condition and already-blocked targets.
No Source/parser/AST defect was found in these three cards.

## Next work

Implement the explicit "could block" checks for both exchange forms, using the
existing core pair restrictions, group limits and capacity authority. Keep
declaration-only costs and requirement maximization separate from changing
relationships. Cover changed evasion/protection, shared blockers, empty surviving
associations, multi-block capacity and attacking bands. The mutation alone
intentionally does not evaluate those conditions.

Then implement shared recipes and generator adapters for `CombatChange::Block`
and the two `SwapBlocks` forms. Reuse the existing historical sole-blocker
unblocking helper for False Orders. Keep the independent `Reselect`,
`AssignAttack`, `OnlyAttackers` and `OnlyBlockers` gaps visible. Add generated
runtime scenarios and the ready-module compile gate before completing the
family or updating its counts. Noted values is the other pending three-card
family. Independent two-card and single-card features remain deferred.

Evidence directory:
`/mnt/vizier/early-set-foundation-20261112/combat-reassignment`.
Published native commit: [`3918b90ab`](https://github.com/vishvananda/cantrip-rs/commit/3918b90ab422d7521c1c863227d1ae76974f1795).
The [proof receipt](cantrip-foundation-combat-authority-20261112.proof.json)
records the checks and exact evidence hashes. The default generator API pin and
all existing completed-family profiles remain unchanged.

The initial 128-game run measured 56.73 us per engine decision versus an older
checkpoint's 45.09 us. To investigate that difference, the parent revision was
rebuilt and compared with this build in eight alternating 128-game runs. All
1,024 games completed. The parent median was **50.65 us** (47.72–51.58), and this
build's median **49.03 us** (48.44–49.59). Allocations stayed **27.31 KB** per
decision in every run. The ranges overlap under shared host load; no speed
improvement is claimed. Both the initial result and the interleaved comparison
are retained. The temporary baseline checkout and executable copies were removed.
