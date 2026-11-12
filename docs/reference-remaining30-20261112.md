# Remaining reference consistency cases

This pass addresses the remaining 30 of the reopened 36 faces. The original
53-face gameplay classifications remain historical; equivalence alone does not
close a consistency case. Current producer changes and matching controls, rather
than edits to Oracle Grammar's independent gold, establish the selected forms.

All 30 cases are closed with implemented producer-family changes and passing
validation. The reopened 36-face review is now 36 completed, zero pending.
This does not mean there were 36 additional gameplay bugs.

| Packet face(s) | Producer contract and matching evidence |
| --- | --- |
| Cryptic Coat | A statically singular cloak result names `Result(After, index 0)` directly, like matching singular manifest/cloak consumers. Other recorded operations retain their own receipt contracts. |
| Ebonblade Reaper; Personal Incarnation; Skybreen | Singular life loss names its recipient in quantities as well as the operation. Scourge of the Skyclaves keeps one simultaneous plural operation with recipient-relative `Acting`. |
| Liliana, Death Mage | Up-to-one target populations use `Targets`, including the zero-target case. Singular life loss/graveyard counts bind the named target player. Mandatory single targets remain scalar. |
| Deathbringer Regent; Dream Thief | Lift free conditions out of object filters and bind the outer Candidate before lifting. Nested filters retain their own candidates. A condition with unresolved nested/history candidate captures keeps its original binding wrapper. Optional targets/empty results keep their existence guards. Regent retains its cast-from-hand test and other-creature exclusion. |
| Delif's Cube; Long River Lurker | An independently created event matching exactly one captured target uses that captured target in its body. Delif's Cone, Niko Aris and Barreling Attack share the form. Arbitrary matching creatures retain EventSubject. Zone changes also retain their event subject: a matcher can test an old object while binding its actual new arrival. Lurker keeps its exact optional exile payment and full Performed/After return set. |
| Electro; Passenger Ferry; Resplendent Marshal | Reflexive closures retain a proven self object; payments do not establish new object subjects. Sparktongue Dragon, Cyclops Superconductor, Ambergris and Leyline Tyrant now share this producer contract. Object substitutions stop at independent abilities, created event triggers and replacements; player event bindings are not rewritten. |
| Draining Whelk | The counter follow-up reads the targeted spell's mana value with current/LKI information, like Spell Swindle and Plasm Capture, including when countering fails. |
| Forbidden Crypt | An own-graveyard payment removes only the identical redundant InZone restriction. A different player's graveyard remains a real intersection. Payment and its guard stay explicit. |
| Haunted Fengraf | Random movement uses one operation with a resolution-time Chance preparation, rather than an unrelated choice receipt. The untargeted own-graveyard pool and as-many-as-possible count handle an empty pool. |
| Hit | Mandatory one-sacrifice damage follow-ups keep a resolving payment, PaymentChosen and exact Inputs/Before information. Rupture now shares this contract. Casting sacrifices such as Fling retain their distinct Casting namespace. No general cost-to-effect rewrite is introduced. |
| In Thrall to the Pit | Control, untap, haste and the captured delayed sacrifice name the original target. A failed control change does not change the pronoun's referent. |
| Dawn of the Dead; Ilharg; Mirror Mockery | Optional entries/creations with dependent follow-ups use an explicit payment and guard. Matching hand-entry templates now use the same Source form. A narrow producer rule also handles optional sequences whose remaining actions operate exclusively on that entry's result (Hellkite Courser); independent later work is excluded. Mirror Mockery retains the whole created-token set for multiplication. |
| Wandering Graybeard | Kinship templates have the same optional look, eligibility test, optional reveal and reveal-dependent follow-up. Kithkin Zephyrnaut and the remaining matching kinship templates adopt the existing explicit two-guard form. |
| Molly Hayes | A singular top-card exile's permission names its singular After result. Three matching Source templates use the singular permission; plural exiles remain aggregate. |
| Myriad Construct | The generic “sacrifice it ... equal to its power” template names that original object with current/LKI information, independent of whether a sacrifice transition was produced. It does not query a later graveyard incarnation. |
| Nemesis Mask | All-able-blockers requirements are consistently attached to the creature to be blocked, matching Lure. Targeted, host and self variants retain their exact blocker filters and durations. |
| No Quarter | Mirrored blocking-pair events name explicit Attacker and Blocker roles, including quantities in their predicates. |
| Norman Osborn | Connive names its object and that object's current/LKI controller as actor. Proven self damage triggers name Source; arbitrary events retain their own objects. Backend disregard of an actor field is not used as justification. |
| Skeletal Swarming | One static ability contains keyword, attack requirement and pump in printed order. Source gains `StaticLayerEffect::Rule`, which lowers directly to the existing portable `Change::Rule`; no sorting or new portable AST field is needed. Avatar of Slaughter and Hellraiser Goblin exercise matching mixed static definitions. |
| Veteran Ghoulcaller | Graveyard-to-hand and library-to-hand duplication use the same recorded-definition/LKI contract, including perpetual changes, without inspecting a later hidden incarnation. Kithkin Brinefarer is the matching control. Graveyard-origin filters preserve the actual stated InZone predicate instead of replacing it with ownership. |
| Vulshok Factory | An explicitly named sacrificed source is Source when the activation cost proves that identity; its counter count retains current/LKI evaluation. Other sacrificed objects and arbitrary cost components retain exact receipt references. |
| Worzel | Basic Plains retains Basic and Plains qualifications without an extra Land test. The separate planeswalker alternative is unchanged. |

## Source and representation scope

Fresh before/after parsing succeeds on the same 33,570 inputs and refuses the
same 1,477. Exactly 42 cached Source records change; the other 33,487 remain
byte-identical. RON lines, tagged JSON and pretty RON carry the same changes.
The fresh full parse is independent of the regression fixture.

The regression packet contains 58 Source records/60 faces, plus a separate
42-record packet covering every changed Source record. Matching static and
optional-entry controls are part of the checks. Meek Attack exposed a missing
`Cards::TotalPowerAndToughnessIs` input variant when its hand filter was used as
a payment; that existing semantic predicate is now admitted in Cards too.
Both new Source variants are appended to preserve existing binary enum indices.

The portable schema remains 190. No new portable AST variants/fields, numeric
widening, engine APIs or runtime engine features are introduced. Saved portable
cards, migrations and browser workspaces are unchanged. The static consolidation
reduces Skeletal Swarming from two static payloads to one. Optional follow-ups
keep their semantic payment records rather than discarding guards for brevity.

No comparison-only alias is added. The historical independent proposal fixture
and Oracle Grammar's worktree, gold and pin remain untouched. Our current
producer snapshots are reviewed against the affected-family matrix separately.
Explicit before/after test data distinguishes fresh producer changes from the
unchanged saved migration inputs. The independent Kharasha Foothills proposal
also stays frozen; its dependent optional token creation receives an explicit
reviewed comparison against the shared payment form.

Two newly mined literal-audit outliers are statistical possessive-word matches:
the detector associates PowerOfTheSacrificedCreature with the word `s` from
“creature's”. The existing activation templates say “its power” instead and
correctly use the sacrificed input. Their exact findings receive documented
reviews; the detector and its thresholds are unchanged.

## Validation and coordination

Validation: the exact reference working tree passes **2,046 Rust tests, zero
failures, 24 ignored, across 199 targets**; the uncached Go suite passes in
84.587 seconds. All **33,529** cached records lower, validate and round-trip,
with no unreduced forms or primitive dependencies. This is IR validation, not
a claim that all cards have engine implementations. The unrelated revealed-hand
foundation generator WIP was saved separately before this final workspace run.

The [proof receipt](reference-remaining30-20261112.proof.json) pins the inputs
and successful logs. Local evidence is under
`/mnt/vizier/reference-remaining30-20261112`. Intermediate failed logs remain
investigation evidence, not successful validation receipts.

After this reference pass, resume the shared foundation milestone described in
[the foundation stop plan](cantrip-foundation-shared-stop-20261112.md).
