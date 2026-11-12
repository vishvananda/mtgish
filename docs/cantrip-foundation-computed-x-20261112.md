# Text-defined and bounded X

The eighteenth focused foundation commit is native `47fd50a31` on
`engine/early-set-foundation-20261112`. It adds existing-width, callback-based X
rules for spells and activated abilities. The generator's `computed-x`
candidate profile emits the actual interface. The normal upstream API pin and
AST schema 190 are unchanged.

`CardDef::x_rule` and the activated definition's corresponding field accept
`XRule::Is`, `AtMost`, or `OfPlayer`. Defined X is computed without a number
prompt; a bound constrains the existing prompt. A player-defined quantity uses
the existing player chooser and snapshots that player's value when announcing
the spell. It does not turn the chosen player into a target. Availability,
target candidates and actual cost payment all respect the rule, including XX,
zero restrictions, alternative costs and values above the ordinary X prompt's
ceiling. Mana abilities cannot retroactively change the announced cost.

There is no quantity widening or new per-object field. Eligible player values
are a sparse, deduplicated set rather than an enumeration from zero to the
largest possible count. Ordinary activations keep their existing cost fast
path. Target-dependent definitions currently require one preselected target;
cyclic target/X dependencies remain outside the interface.

Cost X and an effect's live text-defined quantity must not be confused. A
dynamic activated effect that still needs a live X read retains an explicit
generator diagnostic unless lowering already expresses that live observation.
Voodoo Doll's effect already counts its counters directly.

Four cards become generation-ready: Chromatic Armor, Voodoo Doll, Spoils of War
and Winter's Chill. Reflecting Mirror's twice-mana-value X callback also
compiles; its separate change-target-to-a-player requirement remains explicit.
The frozen scope now contains **206 ready candidates and 80 gapped cards**,
with 92 raw requirement labels. Every other prior gap list is unchanged. These
are generated candidates, not installed or fully reviewed cards.

Ten focused native tests cover fixed and bounded definitions, XX payments,
target mana value, sparse player choices, affordability, zero restrictions,
alternative costs and cost snapshots. The complete native workspace passes
22,642 tests, with 15 ignored across 236 targets. The workspace type-checks.
The complete generator workspace passes 2,018 tests (24 ignored, 197 targets),
including the accompanying Codsworth reference correction.
Four new generated scenarios exercise the actual card activations, choices,
payments and outcomes; the complete runtime probe passes 56 scenarios. All
206 ready modules and a partial Reflecting Mirror callback control type-check.

The compile/runtime probes use a names-only counter prerequisite overlay for
Pin (94) and Sleight (95), preserving existing counter IDs. The native branch's
counter vocabulary is unchanged. These declarations must accompany eventual
card delivery. They follow the existing dense counter design, so the overlay
adds two counter entries per object; it is not an additional engine feature.

All 128-game benchmark controls complete and allocate 27.04 KB per decision.
Repeated runs of the same pre-optimization binary range from 47.92 to 74.45
microseconds; optimized runs measure 48.86 and 50.89 microseconds. Shared-host
noise prevents a statistical speed claim. No .NET pass is claimed because its
SDK is unavailable.

The companion [receipt](cantrip-foundation-computed-x-20261112.proof.json)
records source hashes and final gates. Local evidence is under
`/mnt/vizier/early-set-foundation-20261112`. The six shared three-card families
remain in the [requested stopping milestone](cantrip-foundation-shared-stop-20261112.md).
Card workers remain paused; no upstream update was pulled.
