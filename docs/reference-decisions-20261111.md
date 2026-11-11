# MTGish reference decisions for the migrated Oracle Grammar worker

The final follow-up fixes Vampiric Embrace using schema 190 event-time object capture. See [the capture contract and current adoption instructions](vampiric-embrace-capture-20261111.md). The schema 189 descriptions below document the preceding, independently validated batch.

This review uses `adapters/mtgish/reference-issues.md` from the migrated
`oracle-grammar` repository at `b18fce17`. The reference pin is `1cd0b57`;
MTGish main's `1cd42cd` adds documentation only. The compiled model starts independently pinned at `a2914e0`, schema 189. This
batch keeps the wire schema at 189 but extends shared validation to recognize
direct returns-to-hand as return-cost receipts. Alora's new reference therefore
requires advancing the compiled model provider to the fix commit once, with its
lockfile. The remaining reference-only changes do not independently require a
compiler rebuild.

The report has 36 named faces and 13 consistency groups. An uncommon representation
is not sufficient evidence of a semantic bug. The
[per-entry decision file](reference-decisions-20261111.decisions.json) covers every
named face, the evidence and every group's review requirements. No comparison
alias, reference gold edit or migration that silently repairs saved meanings is
introduced. Existing historical snapshots remain unchanged.

## Completed rules corrections

The timing and sacrifice-template defects are upstream Source bugs. Alora and
the processor corrections are local lowering/validation bugs: their upstream
resolving-cost syntax is retained. They do not warrant upstream Source reports.

| Finding | Decision |
| --- | --- |
| Balustrade Wurm; Resurrected Cultist | Retain both the activation condition and sorcery timing. The conjunction template previously dropped `ActivateOnlyAsASorcery`. |
| Demonlord of Ashmouth | Use `SacrificeAPermanent`, selecting exactly one other creature. `SacrificeEachPermanent` wrongly denoted the entire population. |
| Alora, Cheerful Scout | Keep the compulsory resolving cost and its `PaymentChosen` guard (CR 118.12). Apply a typed perpetual effect to `CostObjects(Resolution, component[0], Performed, After)`, including both components of a returned melded permanent. Do not substitute a stale target or test whether the cost actually reached Hand. |
| Foreboding Steamboat and five processor follow-ups | A resolving one-exiled-card payment uses the same typed `ExiledToGraveyard` cost as an activation. The generic action expansion had an `AsManyAsCan(1)` choice that admitted an empty action. The typed cost requires one card. Preserve payment choice and replacement semantics; do not replace genuine costs by ordinary effects indiscriminately. |

The processor family also includes Mind Raker, Murk Strider, Ruin Processor,
Ulamog's Reclaimer and Wasteland Strangler. These are producer corrections, not
new Cantrip engine features. Any existing backend capability gap remains explicit.

Return follow-ups preserve the full After receipt. A resolving cost's
`PaymentChosen` tests choice/start, not successful arrival; replacement of the
move does not cancel that guard. Do not add a blanket Hand filter or replace
these costs with effects. Ordinary return references and cost output references
keep their separate receipt kinds. See [CR 118.11–12](https://media.wizards.com/2026/downloads/MagicCompRules%2020261106.pdf).
Seek/conjure receipts are retained: creation and return transitions are different
result contracts. An intermediate overbroad interpretation was rejected by the
independent whole-program comparison before commit.

## Equivalent forms unified now

| Forms | Canonical producer output and boundary |
| --- | --- |
| Ordinary resolution paragraphs split across Spell abilities | One ordered Spell body, assembled from Source actions before receipt/target allocation. This fixes the producer form, rather than relying on the Cantrip-only join. Barrage, Determined, Draconic Intervention, Suffocation and Temporal Firestorm are covered. |
| Add(All creature types) versus Set(All) | Set(All). Both denote the complete type universe; this is representation consistency, not a separate gameplay correction. Maskwood Nexus's card definition now agrees with its other definitions. |
| Adjacent Add Creature / Add Artifact in one change list | Artifact then Creature. This preserves one effect and its timestamp; changes and nested effects remain barriers. Phyrexian Totem is the only affected card. |
| Generic perpetual term after a return versus typed perpetual effect | `CreatePerpetualEffect` over a full Transition for an ordinary return, or full CostObjects/Performed/After for a return cost. Keep the payment guard and do not use a scalar index for multiple output cards. |
| One-exiled-card payment expanded as a choice/action versus typed cost | `ExiledToGraveyard`, preserving its one-card cost contract in all payment scopes. |

The spell join is deliberately structural: ordinary `ActionList` paragraphs
combine. Modal/targeted wrappers remain barriers until their announcement and
binding maps are merged explicitly. A casting declaration after a paragraph is
also a barrier, preserving existing structural casting-payment addresses. This
batch does not invent a lossy general join of arbitrary spell announcements.
The previous consumer fallback remains for legacy saved cards.

These are producer normal forms. General AST operations such as Add and Set remain available where they have different meanings; equivalent card outputs converge without removing useful operations.

## Distinctions retained

- Alliance of Arms keeps `PerformedTotal`. It is the total of all players' mana
  payments; `Performed` in a later per-player instruction denotes that player's
  own value. The proposed replacement contradicts the model's explicit contract.
- Vulshok Factory keeps the sacrifice component's Inputs/Before receipt with
  CurrentOrLastKnown. That preserves the source's counters before sacrifice.
- Veteran Ghoulcaller keeps `DuplicateLastKnown`. It explicitly retains the
  departed card's definition and perpetual changes without inspecting a later
  hidden-zone incarnation. A plain live duplicate is not a justified substitute.
- Karn's Touch keeps `Affected`, the documented object whose characteristics
  are being evaluated. A single-target spelling is not inherently more correct.
- Draining Whelk's existing consumer saves the target before attempting the
  counter and reads that saved target's mana value, even if countering fails.
  This agrees with the [official Time Spiral Remastered notes](https://media.wizards.com/2021/downloads/TSR_Release_Notes/EN_MTGTSR_FAQ_20210118.pdf).
- Ebonblade Reaper's consumer already introduces the appropriate player row.
  Its first life calculation uses the controller and its second uses the damaged
  player. The legacy `Acting` spelling alone does not establish a wrong player.

Redundant scoped predicates, valid Recorded/Result alternatives and dual combat
requirements are not declared bugs solely because another form is more common.
Examples are Forbidden Crypt, Molly Hayes, Nemesis Mask and the target/control
change receipts. The decision file records the witness needed for each remaining
question. Explicit player/target bindings remain the preferred form for newly
owned frontend rules; that preference is not a global equivalence rule.

## Vampiric Embrace: captured-host follow-up completed

The old counter recipient is the dying victim. The counter belongs to the
relevant enchanted creature. Replacing it with a live `AttachedTo(Source)` read
would still be wrong when the Aura moves after triggering: the recipient must
be the exact host incarnation captured when the trigger fires.

The independent [XMage implementation](https://github.com/magefree/mage/blob/87ab0a3fcc65f09d05b19af0d75efeedabca88f8/Mage/src/main/java/mage/abilities/common/DealtDamageAttachedAndDiedTriggeredAbility.java)
records a `FixedTarget(creatureMOR)` during trigger matching; Vampiric Embrace
selects that permanent pointer. This is implementation evidence, not a newly
found official case-specific ruling. Direct Gatherer retrieval returned 403.

The follow-up adds `EventSpec::WithOtherObject` under schema 190 and reuses
`EventOther` for the captured host. The parser and complete native program are
corrected without moving the trigger off the Aura. Cantrip's runtime capture gap
remains explicit. See the [capture contract, migrations and evidence](vampiric-embrace-capture-20261111.md).

## Decisions for the 13 broader groups

1. Self references: prefer Source only when it is proved to be the same ability
   holder in that event frame. Reflexive actions and compound events can bind a
   different EventSubject. Granted abilities retain their holder/creator split.
2. Optional action plus follow-ups: preserve optional choice, attempted payment
   and actual success separately. The earlier guarded Dawn/Mirror/Ilharg decision
   stands. A representation-frequency argument does not reopen it.
3. Random return: a chance selection and an AtRandom choice can both be valid.
   Choose one frontend spelling after checking population, cardinality and batch
   semantics; no reference correction follows from the wrapper alone.
4. Delayed targets: keep an explicit captured target/object identity. A delayed
   turn event does not supply an object subject of its own.
5. Compound self enters/attacks: unify self spelling only after checking both
   event branches and the granted-holder context.
6. Type tests after exile: Before and After can differ under replacements and
   characteristic changes. The printed condition determines the correct view.
7. Untap after control change: a target and a named affected receipt may coincide.
   Test a failed control change or vanished target before rewriting the receipt.
8. Returned creature types: arrival changes affect entry characteristics; later
   animations do not. Preserve the timing distinction, as for Emergent Sequence.
9. Scalar versus aggregate property: unify only for a proved zero-or-one set and
   matching empty-case behavior. Keep genuinely plural sets plural.
10. Sacrificed fixed objects: preserve the original incarnation's last-known view.
    Source, Recorded and Transition are not blanket aliases.
11. Self exile: preserve the actual post-cost/post-exile object and its expected
    zone. The Kogla correction remains authoritative.
12. Block pairs: preserve attacker/blocker roles; mirrored clauses may need
    different references.
13. Addition order: choose a consistent spelling only under identical arithmetic
    domains/overflow behavior. Mere operand order is not a card-rules defect.

## Validation and adoption

The Source export changes three of 33,529 records. Normalized output changes 21;
33,508 remain exact. An independent transformation of the previous complete
programs accounts for every changed field. Only the five joined spell faces
need canonical result renumbering. The proof preserves casting addresses,
characteristics, layouts, targets and all unrelated abilities.

The regression packet includes 57 complete Source records / 60 faces: changed
families, all 36 reported faces, and controls for the contracts retained above.
All new forms use schema 189. The shape and browser-facing DSL/document APIs
remain unchanged. Shared return-cost validation is extended, so Wasm is rebuilt
once for the final batch. Existing saved-card representations and historical
migration meanings remain unchanged.
See the accompanying proof and upstream receipts for final check results.

For the Oracle Grammar worker: consume a fresh published normalized artifact;
verify its hash independently, then advance the reference lock and recount the
explicitly allowed population. For Alora's newly valid direct return-cost receipt,
also advance the compiled model provider and Cargo.lock to the fix commit once.
This is a validation bug fix under the same wire schema, not a schema migration.
Do not edit gold or add local equality aliases to make these changes disappear.

The subsequent Vampiric fix advances the final compiled model to schema 190.
Across both commits, four Source records and 22 normalized records have reviewed
meaning/normal-form changes; all records also carry the new schema version.
Use the capture follow-up proof for the final artifact and current adoption steps.

Upstream Source reports: [timing #266](https://github.com/i5jb/mtgish/issues/266),
[sacrifice #267](https://github.com/i5jb/mtgish/issues/267). Captured host: [#268](https://github.com/i5jb/mtgish/issues/268).
