# Complete the outstanding early-set cards

The active goal covers every outstanding card in the frozen 1,320-name
Ice Age-and-earlier population. The user explicitly excludes ante, dexterity
and Shahrazad, while retaining flavor-banned cards. The goal is not limited to
the five candidates completed in the preceding pass.

The current reconciliation at native
`024cf79bab93eae28f3c145f34f228ad2b1b4858` finds 1,144 names upstream, 116 in
previously validated ready packets, and 60 still needing implementation.
The 29 published PRs were read back: 14 merged and 15 open; open heads still
match their validation receipts. Publication is not a claim that all packets
have merged or were tested together in one combined workspace.

The full-scope ledger, reproducible reconciliation script, live PR snapshots,
read-only native source, cached rulings and feature-family preflights live at
`/mnt/vizier/ice-age-completion-20261114`. Preserve the complete ledger between
continuations; the goal remains active until every required card is exact,
independently reviewed, validated and submitted ready or merged.

## First packet: cumulative-upkeep-only mana

Adarkar Unicorn #3038 and Snowfall #3039 are reserved. The shared keyword's
mana payment currently needs a distinct purpose so restricted mana is usable
for cumulative upkeep without becoming ordinary spell/activation mana. The
engine work carries that purpose through actual payment, preview and nested
mana abilities, reusing the existing restriction storage and preserving the
native numeric domains. Its client/wire changes are part of the same feature.

The portable AST already represents this with
`ManaUse::Purpose(ManaPurpose::CumulativeUpkeep)`. The backend now emits
`ManaRestriction::CumulativeUpkeep` only for targets advertising
`Game::may_pay_cumulative_upkeep`, under the `cumulative-upkeep-mana` capability.
The default private API snapshot does not enable that capability. Focused tests
check the positive mapping, refusal on old targets, and that other payment
purposes are not accidentally unlocked.

The distinct `{U}` versus `{C}{U}` bundles now generate through the native
`add_one_bundle_of(&[ManaPool(...)], first_label)` recipe, behind the separate
`mana-bundle-choices` capability detected from that constructor. Both capabilities
remain off in the default pinned snapshot. The recipe owns production and
advice from one definition; generation does not attach a guessed yield to an
opaque callback or split one ability into two. Only fixed, full-width,
nonnegative bundles with no additional instructions or extended lifetime use
this path. Existing equal-amount color choices retain their compact recipe.
Label offsets let multiple abilities retain distinct choice words.

Six focused generation tests cover purpose and bundle capabilities separately,
the whole Unicorn, and multiple abilities. The complete supported-feature Rust
workspace passes: 2,083 tests, zero failures, 21 ignored. The tracked candidate
has no Unicorn gaps and produces its normal `..oracle::ADARKAR_UNICORN` card
file. Native review, compilation of the final card, scenarios and publication
remain separate gates; this is not a completed-card claim.

Snowfall still needs generator support for optional, conditional triggered
mana and a choice of different amounts. These remain explicit gaps. Its
one-blue option for snow was separately corrected in the Source parser and
reference export; see [the decision](snowfall-optional-mana-20261114.md).
Whole-card native validation, including preview and actual production, is still
required for both cards. Neither is counted complete from a restriction test.

Frozen Oracle and rulings files already exist under
`/home/vishvananda/card-lane/scryfall`. The native tools' expected
`/home/vishvananda/cantrip/target/scryfall` path now links to that data cache;
this is shared card data, not a shared Cargo target. No bulk refresh was run.
Adarkar Unicorn has no rulings in that snapshot. Snowfall's 2008-08-01 Wizards
ruling confirms that its added mana is always blue, independently of what its
Island produced.

## Subsequent feature packets

Source preflights cover all 60 outstanding names across mana, damage, static,
history/capture and unusual-procedure families. These are evidence and proposed
dependencies, not runtime completion claims. Candidate shared packets include
event-relative next-time prevention (Dark Sphere/Forcefield), chosen land-type
landwalk (Barbarian Guides/Illusionary Presence), temporary special actions,
scoped damage prevention, linked captures and cost attribution. Exact rulings
and real scenarios remain gates for each implementation.

Use fresh Terra authors and independent reviewers, one exclusive native writer
at a time, and focused Engine/Mechanics/Card commits. Reuse the same physical
worktree's local warm target between packets; never copy or share a Cargo
target. The new worktree was created without cloned build artifacts. Batch
broad validation after reviewed prerequisites and cards are together, then
publish focused ready PRs. Keep the retired automated lanes disabled and leave
the already published campaign branches untouched.
