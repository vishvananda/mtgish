# Finish shared foundation groups, then pause

The user narrowed this foundation pass on November 12: finish the five-card
and three-card groups, then pause. Independent two-card and single-card features
are deferred for focused PRs containing the engine feature and its cards.
Related smaller requirements belong in this pass only when they are part of
the same primitive. Card workers remain paused and no upstream update is pulled.

| Pending family | Affected cards | Related requirements |
| --- | --- | --- |
| Revealed hands — complete | Enduring Renewal; Revelation; Zur's Weirding | [Checkpoint](cantrip-foundation-revealed-hands-20261112.md): native `adcd8efe8`, visibility and historical knowledge agree across all readers. |
| Combat group constraints — complete | Caverns of Despair; Errantry; Orcish Conscripts | Attacker and blocker counts are one declaration-validation family, including requirement maximization. |
| Nonmana combat costs | Flooded Woodlands; Leviathan; Reclamation | Costs and their payments must use the shared combat declaration procedure. |
| Targets chosen by another player | Cuombajj Witches; Nova Pentacle; Preacher | The effect's controller and target chooser differ; choices still obey targeting legality. |
| Changing combat relationships | False Orders; General Jarkeld; Sorrow's Path | Existing combat relationships, legal reassignment and resulting events must stay consistent. |
| Noted values | Ice Cauldron; Jeweled Amulet; Tawnos's Coffin | Remembered mana and noted objects are related to the three-card Note diagnostic; they require typed, linked provenance. |

Computed X is complete at native `47fd50a31`; its
[checkpoint](cantrip-foundation-computed-x-20261112.md) clears Chromatic Armor,
Voodoo Doll, Spoils of War and Winter's Chill. Reflecting Mirror retains its
independent player-retargeting gap. The original seven-family milestone covers
23 distinct cards; the six pending families above cover 18. Finishing a family does not promise
that every affected card is ready: Reflecting Mirror, Preacher, Nova Pentacle,
Ice Cauldron and Tawnos's Coffin currently have additional diagnostics. Preserve
those unless the same primitive actually supplies them. Report the residual
card-specific work at the stopping checkpoint.

The already tested combat-limited delayed-event change is finished before this
boundary. It clears Goblin Flotilla; Melee retains blocker-choice control.
After computed X the frozen scope has 206 generation-ready candidates and
80 still gapped, with 92 raw diagnostic labels. These are combined engine/generator
diagnostics, not a count of proven independent engine features.

Batch broad rebuilds across these families. Keep focused native commits and
generator adapters reviewable, with targeted scenarios and performance checks;
run the complete native and generator gates before the requested pause.

Reflecting Mirror's invented target-mana-value-equals-X restriction was
corrected and reported as [MTGish #274](https://github.com/i5jb/mtgish/issues/274).
The corrected selector and separate twice-mana-value X definition are used in
the checkpoint; the original frozen Source is preserved as evidence.

Revealed hands is complete. Five pending three-card families (15 distinct
cards) remain; the latest frozen scope is 209 ready / 77 gapped / 91 raw labels.

Combat group constraints are complete at native `a09922634`. Four pending
three-card families (12 distinct cards) remain. The latest frozen scope is
212 ready / 74 gapped / 89 raw labels; see the [checkpoint](cantrip-foundation-combat-groups-20261112.md).
