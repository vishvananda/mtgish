# Reassigning blocking relationships

Native `3918b90ab` supplies the mutation authority and `5a4655b56` adds the
explicit legality query and shared card recipes. False Orders, General Jarkeld
and Sorrow's Path now generate without recorded gaps. The unchanged 286-card
scope has **220 generation-ready candidates, 66 gapped cards and 85 raw diagnostic
labels**. Every other card's gap list is unchanged. These are generation
candidates, not installed or independently reviewed card implementations.

`Game::change_blocks` changes the complete relationship graph atomically. It
keeps blocking status separate from the remaining edges, expands attacking
bands, and retains an attacker's blocked status unless an instruction explicitly
makes it unblocked. Structural rules still apply to an effect-created blocker;
declaration-time evasion, untapped eligibility, requirements and payment are
separate. False Orders and Balduvian Warlord use the shared non-target attacker
selection recipe after their preceding removal instructions.

General Jarkeld and Sorrow's Path explicitly require their exchanges to be
legal. `block_reassignment_allowed` reuses the existing blocking constraint
solver for pair restrictions, minimum/maximum blocker counts, multi-block
capacity, bands and whole-group restrictions. It tests the proposed graph,
without adding gameplay blocking requirements, untapped eligibility or costs.
An unrelated retained relationship is allowed to persist if it became illegal
later; the exchanged relationships must pass their current restrictions. An
illegal complete exchange does nothing.

Jarkeld preserves blocking creatures and unchanged shared edges. Sorrow's Path
removes both blockers before assigning their exchanged blocks. Consequently
“blocks” and “blocks a creature” triggers reflect actual new blocking status
and edges rather than an arbitrary delete/reinsert sequence. No false unblocked
trigger is emitted. The separate Sorrow's Path tap trigger still deals its damage.

The first-strike damage step records its participants in existing event history.
A blocker removed and reassigned after that step keeps its first-strike
eligibility history, including after its old attacker dies. It cannot deal
ordinary combat damage a second time just because the old edge was replaced.
The history event is not a trigger event and preserves the 28-byte small-event
bound. There is no new per-object field or persistent participant counter.

The generator evaluates exchange operands before mutably borrowing the game,
then uses its existing acted-on population query to discard targets that became
illegal at resolution. A Jarkeld target gaining shroud therefore does not swap
blocks merely because its stale target id still exists. The explicit candidate
capability `blocking-reassignment` enables only Block and SwapBlocks. Independent
attacking/reselection/control forms retain their diagnostics; the broad
`operation ChangeCombat` label is not globally enabled.

The runtime work also uncovered a separate existing lowering error in
“defending player”. False Orders, Blaze of Glory and Yare now use the existing
combat-role state, as Stand or Fall does, rather than asking whether that player
is personally attacked. See the [reference decision](reference-defending-player-20261112.md).
Only those three unified corpus records change; no Go Source, schema, numeric
width or AST layout change is required.

The complete Rust workspace passes **2,076 tests** (24 ignored, 202 targets)
at generator `6f9f2ef`.

Validation: **26 focused combat-change tests**, **1,526 rules tests** (one
existing ignored test), **86 state tests**, **12 API tests**, and the checked
native workspace type-check pass. The generator packet passes **28 foundation
checks** and **two defender-role checks**. Its explicit native probe passes
**74 generated runtime scenarios**, including seven new blocking/defender
scenarios. All **220 ready modules**, plus Reflecting Mirror's existing partial
callback control, compile against this exact native foundation with the
existing names-only Pin/Sleight counter prerequisite overlay.

The final 128-game control completes every game at **48.12 us** engine answer
time and **27.31 KB** allocated per decision. The preceding stage's interleaved
baseline/candidate controls are retained in the
[first-stage receipt](cantrip-foundation-combat-authority-20261112.proof.json).
These shared-host observations support no statistical speed claim. A stale
cards build-script path to a removed benchmark checkout was resolved by cleaning
only that release package's generated artifacts and rebuilding; no source or
card behavior changed for that workaround.

The [proof receipt](cantrip-foundation-blocking-reassignment-20261112.proof.json)
records the exact revisions and checks. Local evidence is under
`/mnt/vizier/early-set-foundation-20261112/combat-reassignment`. The normal API pin
is unchanged. The only pending shared three-card family is noted values: Ice
Cauldron, Jeweled Amulet and Tawnos's Coffin. The complete native workspace test
gate still belongs at the final shared-family milestone, before the requested
pause; the last full native gate covers feature 19. Workers remain paused and
no card PRs restart.
