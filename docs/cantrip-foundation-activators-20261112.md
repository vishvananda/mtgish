# Early-set foundation: explicit activation permissions

Cantrip commit `de790242f`, pushed on fork branch
[`engine/early-set-foundation-20261112`](https://github.com/vishvananda/cantrip-rs/tree/engine/early-set-foundation-20261112),
adds the fifth focused rules feature after the four in the
[first-batch receipt](cantrip-foundation-adoption-20261112.md).

`ActivatedDef::activators`, `.activated_by(predicate)` and
`.any_player_may_activate()` implement CR 602.2 permissions for individual
abilities. The permission predicate sees the source's current controller and a
separate proposed activator. It can exclude the source's controller, as Clergy
of the Holy Nimbus does, or select the owner after a control change, as Personal
Incarnation does. The ability's condition, costs, stack controller and effect use
the actual activator (CR 113.8, 602.2a).

Regular and mana activation, payment-window options and affordability all use
this authority. Permission is rechecked when the ability is executed. Copies,
grants, ability loss and phasing use the existing layers. Ordinary opposing
permanents need no characteristics calculation merely to discover shared
abilities. Other players' hidden cards are not searched.

The activator pays their own costs. The source keeps its controller. Timing,
activation conditions and global per-instance usage limits still apply. A
creature's tap symbol still uses its controller's summoning sickness (CR 302.6).
Explicit source return/unattach/tap costs can be paid, and exert skips the
payer's next untap (CR 701.43a). Sacrifice still requires control (CR 701.21a).
Permission does not let an opponent's artifact pay improvise or convoke.

The existing generator's planned methods now become real under capability
`other-activators`; no alternate recipe or source lowering is needed. The normal
upstream API pin remains `6814db9c8`; schema 190 and numeric bounds are unchanged.
The candidate API/state/mechanics fingerprint through this feature is
`b7a201827a4c5b025ab119576e5b0c4787ff6b9a70bbd3aaecb3daac6d0d30aa`.

| Card | Candidate result |
| --- | --- |
| Armageddon Clock | No recorded feature gap; named Doom counter declaration is a card prerequisite |
| Clergy of the Holy Nimbus | No recorded feature gap |
| Ifh-Bíff Efreet | No recorded feature gap |
| Infinite Hourglass | No recorded feature gap |
| Land's Edge | No recorded feature gap, including its earlier World prerequisite |
| Mercenaries | No recorded feature gap |
| Merseine | Still needs a cost copied from another object's mana cost |
| Personal Incarnation | Still needs its named-recipient damage redirection |

Together the first five features affect 40 distinct early-set cards, 35 of which
have no recorded generator gap. This is a generation/readiness result; none of
these cards is installed or reserved by this work. Four roadmap groups remain:
bands with other, step-bound durations/permissions, computed X constraints, and
persistent public visibility.

Nine focused native tests pass with verification enabled. All 40 candidate
modules type-check with a declarations-only overlay appending Cube and Doom to
the counter registry, preserving IDs 0–90. Those local probe IDs are not reserved
for card publication; card submissions must allocate against their actual base.
No counter behavior or declaration is added to the engine foundation. The exact
patch is retained in the accompanying proof JSON.

Twenty-seven generated runtime scenarios pass against the pristine foundation,
without the counter overlay. New cases activate an opponent's Land's Edge and
pay from the activator's hand (land and nonland alternatives), and activate an
opponent's Clergy to prohibit regeneration. The normal generator fixture tests
also retain Merseine's and Personal Incarnation's unrelated gaps. Run the native
probe with the command in the first-batch receipt; it now includes these cases.

The complete native Rust workspace passed 22,565 tests (15 ignored), and the
generator suite passed 1,570 (22 ignored). Detailed results and log hashes are
in the [proof JSON](cantrip-foundation-activators-20261112.proof.json).

Allocation remains 26.74 KB per engine decision in the 128-game benchmark.
Timing on this shared machine varies: the original baseline was 45.24 µs per
engine decision, an after-run during builds measured 48.81, and additional
runs measured 44.12–46.67 before versus 42.55–43.47 after. The additional runs'
copy/presentation phases overlapped. These are exploratory timings, not a
controlled comparison proving a speedup or regression. The initial release
rebuild exhausted rustc's default stack; it succeeded with the same 16 MiB
stack used for native validation.

The .NET SDK is unavailable; no .NET pass is claimed. Temporary counter-overlay
sources and build files were removed after archiving their exact patch.
Evidence and the reusable native foundation worktree remain under
`/mnt/vizier/early-set-foundation-20261112` and
`/mnt/vizier/card-lane/early-set-foundation-20261112`, respectively.
