# Finish shared foundation groups, then pause

The user narrowed this foundation pass on November 12: finish the five-card
and three-card groups, then pause. Independent two-card and single-card features
are deferred for focused PRs containing the engine feature and its cards.
Related smaller requirements belong in this pass only when they are part of
the same primitive. Card workers remain paused and no upstream update is pulled.

| Pending family | Affected cards | Related requirements |
| --- | --- | --- |
| Computed X values and limits | Chromatic Armor; Reflecting Mirror; Voodoo Doll; Spoils of War; Winter's Chill | Three ability-X diagnostics and two spell-X diagnostics; announcement, cost and target authority must agree. |
| Revealed hands | Enduring Renewal; Revelation; Zur's Weirding | Continuous visibility, moves and historical knowledge must agree across host, observation and hidden-world sampling. |
| Combat group constraints | Caverns of Despair; Errantry; Orcish Conscripts | Attacker and blocker counts are one declaration-validation family, including requirement maximization. |
| Nonmana combat costs | Flooded Woodlands; Leviathan; Reclamation | Costs and their payments must use the shared combat declaration procedure. |
| Targets chosen by another player | Cuombajj Witches; Nova Pentacle; Preacher | The effect's controller and target chooser differ; choices still obey targeting legality. |
| Changing combat relationships | False Orders; General Jarkeld; Sorrow's Path | Existing combat relationships, legal reassignment and resulting events must stay consistent. |
| Noted values | Ice Cauldron; Jeweled Amulet; Tawnos's Coffin | Remembered mana and noted objects are related to the three-card Note diagnostic; they require typed, linked provenance. |

These families affect 23 distinct cards. Finishing a family does not promise
that every affected card is ready: Reflecting Mirror, Preacher, Nova Pentacle,
Ice Cauldron and Tawnos's Coffin currently have additional diagnostics. Preserve
those unless the same primitive actually supplies them. Report the residual
card-specific work at the stopping checkpoint.

The already tested combat-limited delayed-event change is finished before this
boundary. It clears Goblin Flotilla; Melee retains blocker-choice control.
After that change the frozen scope has 202 generation-ready candidates and
84 still gapped. There are 94 raw diagnostic labels: 18 shared by multiple cards,
covering 35 cards, and 49 cards with only unique labels. Of the 84, 58 have one
remaining diagnostic and 26 have multiple. These are combined engine/generator
diagnostics, not a count of proven independent engine features.

Batch broad rebuilds across these families. Keep focused native commits and
generator adapters reviewable, with targeted scenarios and performance checks;
run the complete native and generator gates before the requested pause.

Before implementing computed X for Reflecting Mirror, investigate its existing
Source reference: it restricts the target spell to mana value X while separately
defining X as twice that spell's mana value. The local Oracle text has no such
target mana-value restriction. Confirm and correct/report that representation
instead of making the engine compensate for it.
