# Delayed trigger text and stopped-card recovery

Blessed Wine, Lightning Blow, Panic, and Ray of Erasure created delayed draw abilities whose text described the
spell's earlier instruction. Battle Cry similarly named its temporary blocking trigger after its untap instruction.
The aligned spell paragraph did not include the later Oracle paragraph creating that trigger.

A face with one spell program now retains its otherwise unclaimed instruction paragraphs for delayed-trigger text.
Paragraphs claimed by another ability remain separate, as do keyword and activated-ability paragraphs. A later
trigger paragraph is distinguished from the enclosing triggered ability's own first sentence. This changes labels,
not the timing or effect of those abilities. Delayed and state-trigger constructors also supply their own printed
text to callback documentation.

State-trigger predicates now cite CR 603.8 from their actual `state_triggered` registration, separately from
intervening conditions and resolving effects. A casting restriction before a keyword paragraph quotes that
restriction rather than the later static ability. These roles follow the
[Comprehensive Rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260619.pdf).

The full test run also caught a granted-ability text regression introduced by the regeneration feature: capturing
the granted text consumed the level-stage line's state. That read now restores stage state along with the quoted
text cursor. The existing Lord of Shatterskull Pass and Joraga Treespeaker regressions pass without altered
expectations.

Validation evidence is under
`~/card-lane/pipeline-unlimited-20261103/integration/delayed-text-recovery/`:

- `all-generator-tests-final.log`: 929 passing tests across 87 Cargo targets; two existing ignored tests.
- Nine full source/Oracle fixtures cover the four delayed draws, Battle Cry, Blizzard, and three state triggers.
- Twenty freshly generated complete modules compile against native checkpoint `5ed784375`.
- `runtime.json`: 34 substantive scenarios from the stopped workers pass unchanged with native verification enabled.
  Four unfinished placeholders remain explicit: one each for Lapis Lazuli Talisman and Nacre Talisman, two for
  Raiding Party. Raiding Party has no completed scenario in its old packet and must establish behavior in its retry.
- `validation.json`: test log digest, native checkpoint, and limitations. The probe's generated card files are
  validation artifacts; they are not integrated card commits.

The recovery cohort also includes older citation failures already fixed by callback roles, untap-action citations,
entry-replacement citations, and shared-only card classification. Each retry pins the current generator revision,
retains the old packet, and still requires normal card gates and review:

Battle Cry, Blessed Wine, Blizzard, Island Fish Jasconius, Kjeldoran Royal Guard, Lapis Lazuli Talisman, Leshrac's
Rite, Lightning Blow, Malachite Talisman, Merchant Ship, Mudslide, Nacre Talisman, Onyx Talisman, Orc General, Panic,
Polar Kraken, Rag Man, Raiding Party, Ray of Erasure, and Sisters of the Flame.

Dandân's state-trigger citation is corrected too, but its reported duplicate bootstrap filename is an independent
preparation issue. It is not included in this recovery.
