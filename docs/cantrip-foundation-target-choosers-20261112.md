# Targets chosen by another player

Native `4712aa560` separates a target slot's chooser from the spell or ability's
controller. Cuombajj Witches and Preacher become generation-ready candidates;
Nova Pentacle clears its chooser requirement and retains its independent delayed
target-capture gap. The frozen 286-card scope is now **217 ready / 69 gapped**,
with **86 raw diagnostic labels**. No unrelated card's gap list changes.

`TargetSpec::chosen_by` names one player. `chosen_by_one_of` lets the controller
select an eligible player first, then asks that player for the slot's targets.
A target is still targeted by the original spell or ability: choosing your own
hexproof creature does not make it legal for an opponent-controlled ability.
The actual chooser cannot cancel someone else's announcement. Ordinary target
slots retain their existing controller/trigger-subject choice procedure.

Preacher's “they control” reads `Query::target_chooser()` in the target filter.
The generator uses the current target specification's existing OneOf identity
key, so it binds to that choice rather than asking again or enabling unrelated
OneOf contexts. The normal snapshot and portable schema 190 stay unchanged;
`other-player-targets` is an explicit candidate feature.

The engine retains the chosen player with each target occurrence in an optional
immutable arena. Per-occurrence storage matters when one printed spec expands
into different opponents' slots. Copies keep the choices, retargeting checks
them, and each resolving mode or fused half sees its own target slice. Thus
Preacher's target does not stay legal just because it moved from the selected
opponent to a different opponent. The arena is part of the state hash and
journal, including undo, redo and search forks. It represents the stack item's
current announcement configuration, alongside its targets and shares.

The availability search branches by chooser and by target set. Two opponents
with one creature each cannot collectively satisfy a slot where one opponent
must choose two creatures they control. Players without a legal completion are
excluded from the controller's chooser menu. The bounded search retains its
existing budget/defer behavior.

Ten focused native rules tests and two state regressions pass. All **1,500 rules
tests** (one existing ignored test), **86 state tests**, and the native workspace
type-check pass. The generator packet passes **26 checks**, including **67 actual
generated runtime scenarios**, and six feature-profile tests pass. All **217
ready modules** plus the existing Reflecting Mirror partial callback control
compile against the candidate foundation. New generated runtime cases execute
Cuombajj Witches' two players' damage choices and Preacher gaining control then
releasing it when the source untaps.

Storage adds no per-object field. A stack item grows from **312 to 320 bytes**;
the game state gains a 24-byte empty vector header, and each used chooser entry
is two bytes. Ordinary declarations allocate no chooser arena data. The final
128-game benchmark completes every game and measures **45.09 us** of engine
answer time and **27.31 KB** allocated per decision. The preceding checkpoint
measured 48.70 us / 27.31 KB; these individual shared-host runs support no
statistical speed claim.

A first divided-target slot with another chooser retains a separate diagnostic:
that protocol must distinguish target selection from share allocation. Later
ordinary slots and per-opponent expansions use this interface. Nova Pentacle's
remaining delayed capture requirement is likewise explicit. Generation-ready
candidates have not been installed or independently reviewed as card commits.

The [proof receipt](cantrip-foundation-target-choosers-20261112.proof.json)
records revisions, checks and hashes. Local evidence is under
`/mnt/vizier/early-set-foundation-20261112/other-player-targets`. Full native and
generator workspace test gates remain batched until combat reassignment and
noted values are finished, before the requested pause. The last complete native
gate still covers feature 19; current focused passes do not replace it.

No parser or Source reference correction was needed for this family, so no new
MTGish reference bug report is warranted. Oracle Grammar, native main, paused
card workers, user `consolidation.md` and native `research/` remain untouched.
