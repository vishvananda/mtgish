# Finish shared foundation groups, then pause

The user narrowed this foundation pass on November 12: finish the five-card
and three-card groups, then pause. Independent two-card and single-card features
are deferred for focused PRs containing the engine feature and its cards.
Related smaller requirements belong in this pass only when they are part of
the same primitive. Card workers remain paused and no upstream update is pulled.

| Family | Affected cards | Current status |
| --- | --- | --- |
| Computed X — complete | Chromatic Armor; Reflecting Mirror; Voodoo Doll; Spoils of War; Winter's Chill | Native `47fd50a31`; Reflecting Mirror retains its independent retargeting requirement. [Checkpoint](cantrip-foundation-computed-x-20261112.md). |
| Revealed hands — complete | Enduring Renewal; Revelation; Zur's Weirding | Native `adcd8efe8`; visibility and historical knowledge agree across readers. [Checkpoint](cantrip-foundation-revealed-hands-20261112.md). |
| Combat group constraints — complete | Caverns of Despair; Errantry; Orcish Conscripts | Native `a09922634`; complete attack/block declaration constraints and requirement maximization. [Checkpoint](cantrip-foundation-combat-groups-20261112.md). |
| Nonmana attack costs — complete | Flooded Woodlands; Leviathan; Reclamation | Native `210d1eefa`; shared declaration payment scheduler. [Checkpoint](cantrip-foundation-nonmana-combat-20261112.md). |
| Targets chosen by another player — complete | Cuombajj Witches; Nova Pentacle; Preacher | Native `4712aa560`; chooser provenance and targeting legality stay separate. [Checkpoint](cantrip-foundation-target-choosers-20261112.md). |
| Changing combat relationships — complete | False Orders; General Jarkeld; Sorrow's Path | Native `5a4655b56`; atomic graph changes, explicit shared legality and generated runtime cases. [Checkpoint](cantrip-foundation-blocking-reassignment-20261112.md). |
| Noted values — complete | Ice Cauldron; Jeweled Amulet; Tawnos's Coffin | Native `b5000fa91`; exact opted-in activation payments and immutable linked mana/counter notes. [Checkpoint](cantrip-foundation-noted-values-20261112.md). |

All seven shared families are complete; none remain pending. The
latest frozen scope has **221 generation-ready candidates, 65 gapped cards and
83 raw diagnostic labels**, out of the original 286 cards. These labels combine
engine and generator requirements; they do not count independent engine features.

The original seven-family milestone covers 23 distinct cards. Finishing a
family does not promise that every affected card is ready: Reflecting Mirror,
Nova Pentacle, Ice Cauldron and Tawnos's Coffin have additional
diagnostics. Preserve those unless the same primitive actually supplies them.
Report residual card-specific work at the stopping checkpoint. Hipparion's
blocker-payment procedure is still separate from the completed attack costs.

Batch broad rebuilds across these families. Keep focused native commits and
generator adapters reviewable, with targeted scenarios and performance checks;
the final complete native and generator gates pass: 22,724 native tests
(15 ignored) and 2,079 generator tests (24 ignored). See the
[stopping checkpoint](cantrip-foundation-shared-milestone-20261112.md). No card workers or card PRs restart at this milestone.

Reflecting Mirror's invented target-mana-value-equals-X restriction was
corrected and reported as [MTGish #274](https://github.com/i5jb/mtgish/issues/274).
The corrected selector and separate twice-mana-value X definition are used in
the checkpoint; the original frozen Source is preserved as evidence.
