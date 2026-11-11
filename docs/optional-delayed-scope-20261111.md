# Optional actions and dependent delayed triggers

Project decision: retain the guarded native registration for Dawn of the Dead,
Mirror Mockery and Ilharg, the Raze-Boar. Declining the optional action skips the
dependent follow-up sequence, including creation of the cleanup trigger. A frontend
reading that places registration outside the optional sequence is not accepted as
equivalent. This resolves our representation choice; it is implementation-backed
interpretation, not a newly located case-specific Wizards ruling.

The previous oracle-grammar report correctly withdrew its earlier conclusions:
showing that an empty delayed trigger can be scheduled establishes observable
inequivalence, not which reading Oracle intends. General ordered-execution rules
alone do not resolve the anaphoric scope. We inspected two independent engine
implementations as additional evidence; both guard the entire follow-up sequence.

| Card | Native representation retained |
| --- | --- |
| Dawn of the Dead | Optional `PayCost` containing a `PlayerAction` move of the graveyard target. `PaymentChosen` guards haste and delayed exile of the move's After result. |
| Mirror Mockery | Optional `PayCost` containing token creation. `PaymentChosen` guards delayed end-of-combat exile of `Created(result)`. |
| Ilharg, the Raze-Boar | Optional direct `CostPart::Move`, choosing exactly one creature card from hand, tapped and attacking. `PaymentChosen` guards return of `CostObjects(Resolution, component:[0], Performed, After)`. This is not a PlayerAction choose-and-move payment. |

Through the Breach corroborates the same optional-entry/haste/cleanup pattern.
Astral Slide supplies an explicit “if you do” control. Feldon of the Third Path
and Shallow Grave supply mandatory controls whose cleanup registration remains
unconditional. Seven complete native-program fixtures lock these cases, and the
Rust regression checks registration ancestry as well as full canonical operands.
The Go fixtures lock parser output against the actual checked-in Oracle records.
These are native structure/validation tests, not complete gameplay simulations.

## Independent implementation evidence

XMage is pinned to `ecd61fbcac1fca5840f60c7873b2754143f2f17a`. Its optional triggered-ability
resolver returns before resolving any effect when the optional choice is declined.
Dawn, Mirror Mockery and Ilharg make the complete effect optional. Their effect
implementations create cleanup only after the applicable move/token creation.
Through the Breach's shared helper returns before the follow-ups when no card
was chosen. Inspected source files:

- [DawnOfTheDead.java](https://github.com/magefree/mage/blob/ecd61fbcac1fca5840f60c7873b2754143f2f17a/Mage.Sets/src/mage/cards/d/DawnOfTheDead.java)
- [IlhargTheRazeBoar.java](https://github.com/magefree/mage/blob/ecd61fbcac1fca5840f60c7873b2754143f2f17a/Mage.Sets/src/mage/cards/i/IlhargTheRazeBoar.java)
- [TriggeredAbilityImpl.java](https://github.com/magefree/mage/blob/ecd61fbcac1fca5840f60c7873b2754143f2f17a/Mage/src/main/java/mage/abilities/TriggeredAbilityImpl.java)
- [ThroughTheBreach.java](https://github.com/magefree/mage/blob/ecd61fbcac1fca5840f60c7873b2754143f2f17a/Mage.Sets/src/mage/cards/t/ThroughTheBreach.java)
- [MirrorMockery.java](https://github.com/magefree/mage/blob/ecd61fbcac1fca5840f60c7873b2754143f2f17a/Mage.Sets/src/mage/cards/m/MirrorMockery.java)
- [PutCardIntoPlayWithHasteAndSacrificeEffect.java](https://github.com/magefree/mage/blob/ecd61fbcac1fca5840f60c7873b2754143f2f17a/Mage/src/main/java/mage/abilities/effects/common/PutCardIntoPlayWithHasteAndSacrificeEffect.java)

Forge is pinned to `87cb532e9f088958ff367d293dc98076a091f9cb`. The three triggered cards
make the executing ability optional (`OptionalDecider`); `WrappedAbility` returns
on a declined confirmation before running that ability. Its executor includes
the cleanup. Through the Breach carries the delayed action on the chosen entry.
Inspected source files:

- [dawn_of_the_dead.txt](https://github.com/Card-Forge/forge/blob/87cb532e9f088958ff367d293dc98076a091f9cb/forge-gui/res/cardsfolder/d/dawn_of_the_dead.txt)
- [ilharg_the_raze_boar.txt](https://github.com/Card-Forge/forge/blob/87cb532e9f088958ff367d293dc98076a091f9cb/forge-gui/res/cardsfolder/i/ilharg_the_raze_boar.txt)
- [mirror_mockery.txt](https://github.com/Card-Forge/forge/blob/87cb532e9f088958ff367d293dc98076a091f9cb/forge-gui/res/cardsfolder/m/mirror_mockery.txt)
- [through_the_breach.txt](https://github.com/Card-Forge/forge/blob/87cb532e9f088958ff367d293dc98076a091f9cb/forge-gui/res/cardsfolder/t/through_the_breach.txt)
- [WrappedAbility.java](https://github.com/Card-Forge/forge/blob/87cb532e9f088958ff367d293dc98076a091f9cb/forge-game/src/main/java/forge/game/trigger/WrappedAbility.java)

Only the optional-registration behavior was checked. These implementations are
corroboration, not certification of their unrelated targeting or token behavior.
Pinned content hashes are in the adjacent evidence manifest.

## Limits and consequences

The [Comprehensive Rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020261106.pdf)
603.7a-c, 608.2c-d and 609.3 explain delayed abilities and ordered/impossible
effects, but do not explicitly answer this decline/empty-registration corner.
Wizards' [Ilharg release notes](https://magic.wizards.com/en/news/feature/ravnica-remastered-release-notes)
and [Through the Breach notes](https://magic.wizards.com/en/news/feature/innistrad-remastered-release-notes)
also do not supply that explicit ruling. Do not label the project decision as
rules-authoritative or claim those notes establish it.

No native parser/lowering change or new upstream reference-defect report is
warranted for these three cards. Keep the registration within the guarded sequence;
keep the frontend's alternate scope distinguishable rather than hiding it through
an equality alias. If authoritative contrary evidence appears, revisit the scope.
Do not generalize this into dropping every empty-capture delayed trigger: that
would change other programs. Registration placement is already explicit in the AST.
Ilharg's separate-versus-Either departure events and frontend choice cardinality
remain separate comparison obligations; this decision does not alias them away.
Identity Thief's earlier performed-object return fix is retained independently.
