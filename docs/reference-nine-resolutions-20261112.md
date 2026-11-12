# Resolution of the final nine reference questions

**Consistency review reopened:** the gameplay-equivalence decisions below are historical.
They do not waive alternate producer syntax. All 36 retained equivalent/contract
cases are being audited under [the consistency follow-up](reference-consistency-20261112.md)
and [upstream issue 275](https://github.com/i5jb/mtgish/issues/275). Satya remains
a separate rejection of a scalar proposal with a token-multiplication counterexample.

These nine were held for a witness in the initial 53-face review. The follow-up
uses the complete native programs, generated code comparisons, local official
rulings and the rules contracts. It requires no Cantrip engine additions. The
foundation goal remains paused. The validated first batch is `2f4f673` on the
fork's main branch; this document and the two additional producer forms follow it.

| Face | Decision | Evidence and boundary |
| --- | --- | --- |
| Delif's Cube | Retain the explicit captured target. The proposed EventSubject is equivalent in this particular callback. | The delayed Object event is AttacksAndIsNotBlocked with SameObject(Target(0)). Its subject therefore is the captured original target. CR 603.7c preserves its identity; a leave-and-return is a new object. This says nothing about object subjects in delayed turn events. The generator's NoAssignment gap is separate from this identity question. |
| Long River Lurker | Retain the exact exile cost and its full After receipt. | The delayed damage event filters SameObject(Target(0)) as its source and explicitly binds the event subject to Source, so the proposed subject is the same original creature. Both forms return the full output set still in Exile, including split outputs, and exclude tokens. Replacement into another zone leaves no object for either return. Keep payment choice and performed output distinct. |
| Haunted Fengraf | Retain the existing random-choice form; no reference defect. | Both forms sample exactly one creature card from the controller's graveyard as resolution occurs and then return it. Empty pool returns nothing. The generator uses choose_at_random directly or random_of, whose multi-item case calls that same API. The official ruling confirms resolution-time selection, no target and inclusion of cards added in response. A possible difference in random-number consumption on a singleton is not a different game-rule distribution. |
| Hit | Retain the mandatory resolving sacrifice and before-cost mana value. | The target must choose one eligible artifact/creature. If none can be sacrificed, either form deals no damage: the guarded cost does not start, while the ordinary effect has no sacrificed permanent and a zero amount. If the sacrifice movement is replaced, CR 118.11 preserves the cost and its original input information. An illegal sole player target prevents resolution in both forms. Casting/activation sacrifice restrictions do not by themselves distinguish this resolving cost. This equivalence is specific to this damage-only follow-up, not a global cost-to-effect rewrite. |
| No Quarter | Retain the reference; both spellings are equivalent here. | In its blocks event, EventSubject is the blocker. Replacing that power operand by BlockPair(Blocker) generates byte-identical executable Rust, excluding only the two fingerprint header lines. A permanent regression compiles both complete cards and compares them. The mirrored becomes-blocked event still uses the attacker as its subject. |
| Satya, Aetherflux Genius | Retain the aggregate; reject the scalar proposal. | A request for one token can yield multiple actual tokens under Anointed Procession or Doubling Season. Two MV-3 tokens give a sum of 6; the proposed first-result read is only 3 while its sacrifice still addresses all created tokens. Thus the claimed zero-or-one receipt invariant is false. Keep the complete Created receipt for both the amount and the sacrifice, including its empty case and last-known identities. |
| Wandering Graybeard | Retain the guarded look and reveal form; no reference defect. | Both programs offer the optional look, and only inside that choice offer an optional reveal if the top card shares a creature type. Life gain follows the reveal. Empty library, declining the look, declining the reveal and an already-revealed top card have the same outcomes. The official kinship rulings explicitly allow declining a reveal even when eligible and revealing an already-revealed card. |
| Goblin Snowman | Consolidate to one compound prevention effect. | One unlimited, end-of-turn prevention watches combat damage to OR by the captured Snowman. This directly represents the one printed prevention instruction and agrees with the other bidirectional templates. Do not infer a general license to combine separately worded effects, limited shields, extra prevention actions or different durations. This is a chosen producer normal form, not a claim of a demonstrated gameplay defect in the old two-effect encoding. |
| Ward of Piety | Use the existing quantity-budget form. | The generic template said next N damage but emitted next occurrence with a per-occurrence cap. For N=3, successive one-point hits distinguish the contracts: the intended shield covers all three points, whereas the old form ends after the first hit. Use NextAmountOfDamageThatWouldBeDealtThisTurnToRecipient and DealToAnyTargetInstead, producing TotalDamage(N) plus RedirectDamage. The actual card's N=1 positive-damage case agrees with the old form; this is a template correction and clearer card normal form, not an invented N=1 counterexample. |

The two normal forms change only Goblin Snowman and Ward of Piety in the active
Source corpus. The other seven keep their meaning. Do not make their equality
by changing gold, adding comparison aliases or flattening event/cost scopes.
The independent grammar should adopt the retained contracts where necessary;
normal-form convergence is separate from a gameplay bug count.

## Regressions and reproducible evidence

The 66-record, 68-face packet and the complete 53 expected native programs remain
in `rust_syntax/normalize/tests/fixtures/reference53_20261112*`. The final nine
add permanent checks in `reference53_20261112.rs` for:

- No Quarter's complete generated-code equivalence;
- Satya's full creation receipt and summed energy cost;
- Ward's quantity budgets at both 1 and 3;
- Snowman's one unlimited compound prevention.

The English-parser semantic regressions also parse a synthetic three-point Ward
instruction, checking the actual parameterized template rather than only its
one-point corpus instance. Both new Source records are checked against tagged
JSON and the English parser. The other seven decisions follow from the stated
binding/choice contracts; code shape or a synthetic arithmetic calculation alone
is not presented as a gameplay simulation.

The local investigation exported both original proposals for all nine. No
Quarter's executable outputs are identical. Haunted Fengraf's generated code
uses the same candidate population and uniform random primitive. Lurker's
outputs expose the two distinct payment representations, so these are not
claimed byte-identical. Delif's Cube and Ward have independent backend gaps;
those refusals are recorded rather than treated as tests of engine execution.
No speculative engine interface was needed to decide their AST references.

Rules used: CR 118.11–12, 400.7, 603.7c/e, 608.2h, 614.5/9 and 615.7/12. The
[official comprehensive rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020261106.pdf)
provide those contracts. The [official Wilds of Eldraine release notes](https://magic.wizards.com/en/news/feature/wilds-of-eldraine-release-notes)
explain that Doubling Season changes the number of tokens made by the original
creating effect; additional tokens retain that effect's specifications. That
supports the Satya multiplicity witness, without treating a requested count of
one as a bound on actual output.

The Ward template issue is reported as [#273](https://github.com/i5jb/mtgish/issues/273) separately from the earlier Source
bugs. Goblin Snowman's consolidation is documented as consistency work, not a
new confirmed-rules-error report. Issue #106's linked-player timing and duplicate
semantics remain the separate follow-up described in the main decision document.
