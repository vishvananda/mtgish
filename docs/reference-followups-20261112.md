# Reference follow-ups, 2026-11-12

This closes the new disputed faces and same-wording groups in Oracle Grammar's
`adapters/mtgish/reference-issues.md`. The report was pinned to `5408607`; the
MTGish baseline for this work is `19027c6d4afb0109cab8c6efe5296ee329ad3eba`.
Oracle Grammar's worktree, reference pin, gold, seeded rules, and comparison
logic are untouched. The separate Cantrip foundation milestone remains paused.

## Disputed faces

| Face | Finding and correction |
| --- | --- |
| Cauldron Dance | The hand-entry paragraph read the earlier graveyard return. Successful typed entry payments now supersede ordinary entry history. Both haste and the delayed sacrifice read that payment's `CostObjects`, `Performed`, `After`. Payment IDs are lexical identities, not numeric recency: the earlier nested movement can have a larger numeric ID. |
| Lieutenants of the Guard | The strength counter counted `Feather`, absent from the ballot. The shared English template now counts `Strength`. It also receives the voting-loop correction below. |
| Phage the Untouchable Avatar | Already corrected by `d67e6d0`: the created trigger destroys `Damage(Recipient)`, not its captured targeted attacker. Preserved as a control; no additional change. |
| Seizures | The damage recipient was the Aura's controller. The English template now names `ControllerOfPermanent(ThatCreature)`, so both the payment and damage refer to the enchanted creature's controller. The Aura remains the damage source. |
| Woodland Bellower | The shared nonlegendary/color/mana-value creature-card search template omitted `IsCardtype(Creature)`. The complete filter now requires a nonlegendary green creature card with mana value at most three. |

The Cauldron correction also needs a failed-payment rule. Evershrike returns
itself, then optionally puts an Aura onto the battlefield attached to it. If
the Aura entry is declined, the later exile still names Evershrike. Lowering
retains the prior entry binding and restores it in the appropriate negative
payment branch. It can restore an earlier paid entry as well as an ordinary
movement. A subsequent ordinary entry supersedes a paid entry. Branches do not
choose a receipt by taking the greatest result number.

## One producer form per wording

| Group | Decision |
| --- | --- |
| Sacrifice, then damage equal to its property | Grab the Reins now shares Hit/Rupture's required sacrifice payment and `If PaymentChosen` guard. Its power is read from `CostObjects`, `Inputs`, `Before`. Fling retains the distinct casting-cost namespace. |
| Optional hand entry, then delayed return | Surprise Deployment now shares the typed optional entry cost and payment guard used by Ilharg, Meek Attack, and Planebound Accomplice. The delayed return captures the paid creature, and is created only in the chosen-payment branch. |
| Becomes an artifact creature | Repaired all 34 English templates still using `SetCardtypes([Artifact, Creature])`, affecting 36 cached records. CR 205.1b retains existing card types and applicable subtypes for artifact-creature wording, so these templates add Artifact, then Creature. Named creature-type changes, colors, PT, durations, and grants retain their original order and meaning. An explicit IR `SetCardTypes` still means replacement; there is no blanket reinterpretation of saved IR. |
| Starting with you, each player votes | Six templates lacked the player loop: color, carnage, feather, grace, sprout, and strength ballots. Council Guardian, Coercive Portal, Lieutenants, Magister of Worth, Messenger Jays, and Orchard Elemental now use `ForEachPlayer(All, starting_with: AbilityController)` with each bound player voting. The Source grammar also accepts the color-ballot loop. This is a rules defect, not harmless equivalent syntax (CR 701.38a). |
| Extra target constraints | Removed Spirit Flare's invented exclusion of its first target from its second target's filter. Actual “another”/“other target” controls retain their exclusions. Uchuulon's redundant common-owner constraint was already absent on the baseline; its required exile payment keeps an explicit exactly-one selection despite an up-to-one announced target. |
| Cast another spell this turn | Shared cast-spell history predicates now produce an occurrence-count comparison over `EventHistory(Spell Cast)`, with the same player, filter, and window. The producer places source exclusion before color, matching the existing controls. Non-cast histories and windows that cannot be expressed by the equivalent event-history window keep their distinct forms. |
| Random graveyard return | Already corrected by `d67e6d0`: resolution-time Chance selection with `AsManyAsCan(1)`, without redundant same-zone/owner filters. No new change or equality alias. |
| Type test after a one-card move | A proven singleton movement receipt uses `ObjectMatches(Result {phase, index: 0}, filter)` instead of counting a filtered transition and testing whether the count exceeds zero. The same rule covers top cards, bottom cards, single card targets, and one-card choices from an observation, search, or private zone. |
| Untap those creatures after control changes | In the Darkness Bind Them now shares Hideous Taskmaster's announced target population for the untap and haste follow-ups. A legal target still receives these effects when its control change cannot happen. Successful-control receipts remain available for actual “gained control this way” wording elsewhere. |
| Property of one result | A sum over a proven singleton transition becomes `ObjectProperty(Result {phase, index: 0})`, preserving the property, phase, and current-or-last-known access. This applies to both one original permanent before movement/destruction and one actual card after movement. Genuine populations retain aggregates; see below. |
| This card after its own exile | Current Lucius/Liliana movement references, Lamplight Phoenix's paid-cost reference, and Reborn Hero's graveyard destination already preserve their proper incarnation and expected zone. These are intentionally distinct action/cost/event namespaces, not alternate spellings of one receipt. No new change. |
| Starting life total plus N | Fresh producer output consistently puts `StartingLife` before the constant in that two-operand addition. No general arithmetic sorting, subtraction rewrite, or saved-card migration. |
| Prevent damage dealt to and by | Pure prevention replacements use the recipient branch before the source branch. The rewrite requires exactly two damage alternatives and a body consisting only of `PreventDamage`. Replacement programs that observe event bindings or do other work retain their branch order. |
| Perpetual change on a creature that died | Forgeborn Phoenix and Hook Horror now use typed `CreatePerpetualEffect` on the guarded `EventDestination` in the graveyard, matching Consumed by History. The old battlefield subject is not the card in the graveyard (CR 400.7). The ability granted by Forgeborn has its own source frame when it later returns its holder. |

## Cardinality is a proof, not a requested amount

The singleton pass runs on fresh typed IR after movement lowering, so identical
movement contracts receive the same treatment regardless of the Source recipe
that produced them. It tracks target domains, bounded choices, observations,
prepared selections, and before/after movement receipts. It does not inspect
card names, serialize trees to discover their shapes, rewrite saved documents,
or change comparison equivalence rules.

The following batch forms remain necessary:

- Satya's created set can contain several tokens after a doubling replacement.
  Both the energy amount and sacrifice continue to read the entire created set.
- Heed the Mists, Infernal Genesis, and Mindshrieker request a mill of one, but
  an effect such as Bruvac can increase the actual mill. Their actual receipt
  remains a population. Explicitly moving one physical card is a different
  operation contract.
- Dracoplasm, Kylox, Reign of the Pit, and Shadowgrange Archfiend can aggregate
  multiple sacrifices; Stitcher Geralf can exile two cards; Gilded Ambusher and
  Szat's Will have genuinely plural exile populations.
- One pre-move permanent can be a melded permanent and leave two cards. Its
  before-state can be singular while its after-state is plural. The pass does
  not turn a one-permanent exile into a one-card post-exile proof.
- A singleton in each iteration is a batch outside a player/object loop or
  repeatable process. A branch's new receipts do not prove anything about a
  sibling or enclosing scope. Independent abilities reset the proof frame;
  created closures capture the outer frame and can shadow result numbers.

These are semantic distinctions. They are not per-card exceptions for the same
contract. The portable schema stays at 190 and the runtime AST layout is
unchanged. A defaulted typed `Condition` visitor hook makes all condition sites
reachable without serialization or condition-specific searches.

## Source caches and preserved evidence

The RON, tagged JSON, and pretty RON caches are synchronized from fresh English
parsing. Existing current-producer Source fixtures receive the same repairs.
Historical saved IR and independent proposals are preserved. Exact test-only
before/after expectations extend the existing reviewed-change ledger.

Two historical Source recipe inputs are deliberately retained: Darksteel
Mutation in `ability_aliases.ronl` is paired with the old ability-expansion
snapshot, and Surprise Deployment in `pipeline_generated.ronl` covers the
legacy `CardsInHand.IsNonColor` recipe. The new parser packet tests their current
English output separately. Retaining these inputs tests the old explicit Source
contract, rather than pretending it was always the corrected English output.

The original analysis and full before/after artifacts are in
`/mnt/vizier/reference-followups-20261112`. The checked-in JSON ledger records
identities, input hashes, changed paths, and final verification counts.

## Verification and upstream handoff

- Fresh English parser: **33,570 accepted / 1,477 refused**, identical name
  sets to the baseline. **48** cached Source records change; **33,481** do not.
- Entire cached corpus: **33,529 / 33,529** lower, validate, and round-trip;
  every record is unified and complete without primitive dependencies, with
  **zero** unsupported Source or unreduced forms.
- Unified export: **159** records change; **33,370** are byte-identical. All
  identities, layouts, and characteristics are preserved. The changed families
  are fully listed in the JSON ledger. Evershrike, Phage Avatar, Uchuulon, Satya,
  and the self-exile controls remain unchanged.
- Complete Rust workspace with Cantrip enabled: **2,097 passed / 24 ignored**,
  **203** test/doc-test targets. This includes the **146-record / 15-test**
  end-to-end packet and three cardinality/scope boundary tests.
- Uncached complete Go suite: **PASS** (89.513 seconds). After enlarging the
  packet, its fresh-parser test also passes for all 146 Source records.
- Original saved IR and proposals are unedited. Sixteen exact test-only paths
  extend the reviewed snapshot ledger; Negative Zone Portal's separate coin
  migration test explicitly preserves its old `Before` origin while checking
  today's scalar `After` producer form.

Oracle Grammar should adopt the published fork commit, prepare its reference
again, and remove resolved seeded alternatives rather than teach its scorer
that different receipt namespaces or batch semantics are equal. The upstream
handoff is tracked in [MTGish #275](https://github.com/i5jb/mtgish/issues/275).
