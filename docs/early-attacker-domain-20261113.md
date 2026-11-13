# Keep restricted attackers inside the creature domain

Island Sanctuary's fourth retained review corrected its applicability callback:
negating the allowed-creature filter alone also selected every noncreature. The
combat procedure only asked about creatures, but the host correctly asks which
objects a lasting rule applies to when displaying their active restrictions.

The generator now emits `is_creature && !allowed` for `PlayerRule::AttackedOnlyBy`.
This preserves the explicit domain rather than taking the complement of a
creature filter over every battlefield object. Canonical IR, Oracle text and the
native API are unchanged. This is a backend lowering mistake, not a new upstream
reference defect.

The explicit native test in `tests/cantrip_backend.rs` extracts the actual emitted
callback with syn and compiles it against Cantrip. Its retained harness covers a
land, a noncreature artifact, an ordinary creature, flying, islandwalk and a
different landwalk. Before the correction, the noncreature test fails and all
creature controls pass. After it, both native tests pass. The outer test is
ignored in ordinary generator runs because it builds the native dependency; run
it explicitly with `MTGISH_CANTRIP_DIR` and `MTGISH_CANTRIP_TARGET` configured.

The complete generator suite passes 1,654 tests, with 21 explicit integration
checks ignored, across 220 targets. The additional ignored check above was run
explicitly and passed. Regenerating all 33,529 production rows changes only
Island Sanctuary. Its already independently confirmed candidate contains this
manual domain correction from review round four; the generator repair does not
rewrite or revoke that approved candidate.

Native before/after logs, full generator validation and the row-impact audit are
retained under `/mnt/vizier/early-set-campaign-plan-20261113/` as
`attacker-domain-native-*`, `fifteenth-generator-all-targets.log` and
`fifteenth-rollout-impact.json`. Later native loss-procedure work is separate
from the native checkpoint used by this callback proof.
