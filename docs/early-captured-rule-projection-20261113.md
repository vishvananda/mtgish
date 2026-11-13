# Resolved restriction text and the current native base

Island Sanctuary's retained review exposed a presentation defect after the core
captured-word repair: a restriction created with islandwalk changed to swampwalk
enforced swampwalk correctly, but its summary and creature labels still printed
islandwalk. The diagnostic dump also evaluated membership through the live
source's words. Tower of Coireall has the corresponding Wall-to-Elf label case.

The native repair keeps rules authority in `cantrip-rules`. An inherent
`Game::lasting_rule_applies` reader evaluates the existing printed callback using
the effect's captured words, source, controller and subject. Host projections and
script dumps use that reader, and the existing shared text renderer applies the
same captured words to labels. Island Sanctuary and Falter are printed consumers
of the open-set reader. Block-restriction labels use the same text renderer.
There are no new state fields, wire messages, numeric domains or client controls.

Two forced host regressions failed before the repair and pass afterward. They
cover owner, opponent and spectator projections, JSON round trips, diagnostic
dump membership, a live source whose words differ from the effect's words, and
the captured Wall-to-Elf block label. Native receipts and full validation logs
live under `/mnt/vizier/early-set-campaign-plan-20261113/captured-rule-projection*`.

The native foundation is rebased onto Cantrip
`066f4a057def234f749ebd8f1d894d1e9fca3ee9`. Its only new rebase conflict combines
upstream simultaneous life changes with revealed-hand move metadata: apply life
changes, compute final hand visibility, set move metadata, then emit the batch.
The earlier rebase also preserves upstream's shared Extraction Specialist /
Possession Engine helper and migrates its duration without duplicating it.

The worker-history checkpoint and linear PR branch have identical Git source
trees. They retain different ancestry deliberately: stopped review retries need
their original prepared base as an ancestor, while the foundation PR needs a
focused series on current upstream. Validation must name the actual tested head
and explicitly bind the equal source tree before either is promoted or published.

The generator vocabulary changes only its API fingerprint to
`1da38802b96d02dbce9151b4ddf58f25389b7016d7c7dc10382a88d88da5797f`.
No vocabulary entries, switched features, canonical IR, Oracle data or generator
semantics change. This is a native presentation repair, so it requires no new
Oracle Grammar or MTGish reference report. Retry Island Sanctuary only after the
exact common checkpoint has passed full validation and been adopted; retain all
earlier findings and require fresh review and confirmation.
