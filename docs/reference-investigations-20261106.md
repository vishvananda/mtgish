# Executed reference investigation fixes — 2026-11-06

The 34-card investigation established parser, lowering and generator defects. These fixes preserve schema 178 and use the existing Cantrip API. No engine changes or numeric widening are required.

* **Laquatus’s Creativity:** bind the requested draw count before drawing and reuse it for discard. Both the English template’s live recount and the independent grammar’s actual-draw-transition count fail replacement/prohibition controls.
* **Ambergris family:** model hand discard plus drawing as one resolving payment, check draw legality before discarding, and use its chosen-payment receipt for If/When-you-do followups. Replaced draws do not cancel the payment. Legacy Optional and PlayerAction inputs also receive the affordability guard. Both reflexive cards lower through the compound-cost path.
* **Abstract Performance:** carry both piles across the player-choice closure. Generated Rust now compiles and native execution leaves unrelated exile untouched. This withdraws the earlier unexecuted gold-domain-error claim. Independent grammar references now mark ChoosePile instead of drifting to later instructions.
* **Missy:** preserve the villainous marker and both option programs during lowering. Full native generation still encounters explicit unsupported operations.
* **Canonical result IDs:** mark the nearest visible definition as live, register an Effect result after its inputs, and restore result bindings after nested bodies/branches. Allocation stays deterministic; duplicate definitions remain invalid. A dead creator result no longer becomes live because a delayed invocation reuses its numeric label. Captured outer values remain live.
* **Artifact-creature layers:** clarify SetCardTypes documentation to agree with existing additive artifact-plus-creature behavior under CR205.1b. Land/enchantment retention execute correctly; no backend rewrite was needed.

## Validation

`normalize/tests/reference_contract_runtime.rs` is an explicit native gate, ignored in ordinary AST-only runs. Set `MTGISH_CANTRIP_DIR` to an isolated checkout and `MTGISH_CANTRIP_TARGET` to its cache, then run with `--include-ignored`. It generates fresh Rust from the current lowerer. The validated native checkout is `a2aabdfa695a46c4e2184dd75814b96670955107`. Eleven native scenarios cover normal/forbidden/replaced/partial draws, declined payments, reflexive receipts, pile isolation, linked-token provenance and type retention.

The full AST suite passes 1,298 tests (four existing/explicit ignored tests). Six ordinary focused tests and definition-scope tests in `canonical/result_scopes.rs` guard the producers. The English parser retains all 33,529 records: only Laquatus and Ambergris Law change source meaning. Full structural and semantic corpus checks report zero unsupported/primitive operations. The refreshed audit has 182 reviewed findings and zero unresolved findings; three obsolete saved findings were already eliminated by the preceding damage-capture commit.

The independent Oracle grammar corrects its grammar/binding producers and expected local readings. Frozen reference gold, adapter pins and equality aliases are untouched. Tests cover every complete reading, edited-operand printing, payment receipts and actual pile-choice references.

## Limits and evidence

Grant, protection, perpetual, combat and shared-count witnesses that could not execute remain separate investigations. A backend refusal establishes neither a reference error nor semantic agreement. The earlier report and pinned reproduction remain historical before-fix evidence.

Local logs/generated artifacts: `/mnt/vizier/reference-contract-fixes-20261106`. Historical packet: `/mnt/vizier/reference-contract-investigations-20261106`.

## Upstream reports

The English-template defects remain present on upstream `bdac5ca64c01f1acb02405839e3633c98c48bcf3` and are reported as [Laquatus #217](https://github.com/i5jb/mtgish/issues/217) and [Ambergris #218](https://github.com/i5jb/mtgish/issues/218). Canonicalizer, generator and Missy/reflexive lowering fixes are in the fork’s Rust implementation; they are not reported as additional upstream English bugs.
