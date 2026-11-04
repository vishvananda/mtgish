# Reserved-card gap audit, 2026-11-04

Priority is completing the reserved cards before expanding intake into the 163 unreserved cards.

The held queue contained 35 entries. A fresh complete report finds 39 distinct gap diagnostics across 34 cards, with 61 emitted occurrences. Clockwork Beast already generates a complete module; its exact committed-generator output compiles against the validated worker API and it has returned to implementation. A queue hold is not itself evidence of a current engine gap.

Diagnostic types overlap: one shared capability can resolve several rows, while one card can require several capabilities. These are generator reports, not 39 independently established engine defects.

| Diagnostic | Reserved cards |
|---|---|
| ability ProtectionExcept | Black Ward, Blue Ward, Green Ward, Red Ward, White Ward |
| granted ProtectionExcept | Black Ward, Blue Ward, Green Ward, Red Ward, White Ward |
| engine: no effect empties a mana pool | Drain Power, Mana Short, Power Sink |
| a limit on what untaps in an untap step | Smoke, Winter Orb |
| engine: no resolving effect sets land types | Cyclopean Tomb, Gaea's Liege |
| a lasting block rule reading what its ability chose | Raging River |
| a pile's name kept on its objects for a lasting rule | Raging River |
| continuous rule BlockCapacity | Blaze of Glory |
| continuous rule RequireCombat Block each | Blaze of Glory |
| counters removed from an object, which the history does not record | Cyclopean Tomb |
| duration WhileMatches | Illusionary Mask |
| engine: no activation by other players | Personal Incarnation |
| engine: no record of the board as the turn began | Power Surge |
| engine: no rule on the mana spent on a spell | Drain Life |
| engine: no static prevention with an effect besides | Rock Hydra |
| engine: only a mana ability triggers on tapping for mana | Manabarbs |
| event trigger for Indefinite | Cyclopean Tomb |
| keyword action ActivateManaAbility | Drain Power |
| mana production LostThisWay | Drain Power |
| object filter ManaCostPayableWith | Illusionary Mask |
| operation ChangeCombat | False Orders |
| operation DealDamageWith | Power Leak |
| operation GameEffect | Camouflage |
| operation PickFromHand | Word of Command |
| operation PlayWith | Illusionary Mask |
| player collection Scoped | Raging River |
| player rule SpendColorAs | Sunglasses of Urza |
| player rule for a while AttackedOnlyBy | Island Sanctuary |
| player rule for a while SpecialAction | Guardian Angel |
| prevention of part of the damage made at resolution | Forcefield |
| replacement event Discard | Library of Leng |
| replacement of AssignsCombatDamage | Illusionary Mask |
| replacement of BecomesTapped | Illusionary Mask |
| replacement of damage to a named recipient | Personal Incarnation |
| skipping a step of the turn | Stasis |
| static rule AttackException SummoningSickness | Instill Energy |
| static rule BlockCapacity | Two-Headed Giant of Foriys |
| static rule Prohibit BeAttachedBy | Consecrate Land |
| target reference outside a resolving ability | Jade Monolith |

The largest shared groups are the Wards, mana-pool operations, land-type changes and untap-step limits. Combat capacity also connects the held Blaze of Glory and Two-Headed Giant of Foriys to the already reserved blocked Spitting Slug. Further work should check existing native support before classifying each report as an engine change.

The earlier blocking-status defect was separately reproduced and fixed on validated native checkpoint `402b7f61accf6d5015ec340d7ca4552bd730e867`. Rally, Righteousness, Elven Fortress, Piety and Spore Cloud have returned for normal review/fix/verification. Ydwen Efreet still needs last-known blocking relationships.

Frozen source subsets, complete per-face gap reports, summary JSON and Clockwork recovery evidence are retained under `~/card-lane/pipeline-unlimited-20261103/integration/reserved-held-gap-audit-20261104`. No additional reservations were made for this audit.
