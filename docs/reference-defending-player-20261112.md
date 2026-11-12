# Defending players are combat roles

False Orders, Blaze of Glory and Yare use “defending player”. The shared
`SpellDefendingPlayer` Source condition previously lowered to `BeingAttacked`,
which asks whether a player is personally being attacked. That loses a defender
whose planeswalker or protected battle is attacked, and loses the role when the
attacked permanent leaves combat. A defending player is established for the
combat phase, not by a direct attack at that player's life total (CR 506.2,
802.2). The existing `PossibleDefender` state already represents that role for
Stand or Fall; all four now share it.

The Cantrip predicate checks the combat phase and the active player's opponents.
It holds before attackers are declared, survives an attacked planeswalker's
departure, and ends with combat. The distinct “being attacked” state stays in
Commander Liara Portyr and Suppressor Skyguard. No broad equivalence or scoring
alias is added. Multiplayer variants still require the engine's own defender
selection procedure; this patch does not add such a procedure.

This is a Rust lowering correction. The Go Source records are unchanged, and
there is no parser defect to report as a separate upstream issue. Portable
schema 190, enum layout and numeric widths remain unchanged. The existing role
predicate adds no per-object state or production serialization pass.

A six-record complete Source packet verifies strict/unified agreement,
validation and round trips. The corpus export changes exactly False Orders,
Blaze of Glory and Yare; the other 33,526 records are byte-identical to the
preceding export. All 33,529 records still normalize, validate and round-trip
without unreduced forms or primitive dependencies. Historical snapshots and
saved migration inputs are unchanged.

Generated False Orders scenarios exercise a planeswalker defender both while
the planeswalker remains and after it leaves; a direct generated target-filter
probe checks the role before attacks, outside combat and with either active
player. These are part of the combat-reassignment checkpoint. Reproducible
local evidence is under
`/mnt/vizier/early-set-foundation-20261112/combat-reassignment`, including
`defenders-delta.json`, `defenders-check.json`, and
`runtime-defenders-final.log`. Oracle Grammar can refresh its producer
expectations after adopting this implementation; its worktree, reference pin,
gold and comparison logic are not edited here.

Published correction: `6f9f2ef98b9ab64cba5d57ef6510a60d40c3e05c`.
The complete Rust workspace passes **2,076 tests** (24 ignored, 202 targets).
Upstream adoption note: [MTGish #275 follow-up](https://github.com/i5jb/mtgish/issues/275#issuecomment-6836737643).
