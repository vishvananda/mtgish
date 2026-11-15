# Oracle Grammar 0.2.2 adoption and 0.2.3 preparation — 2026-11-15

This checkout remains the MTGish Source parser, Source → owned AST lowerer, and comparison adapter. Generator/lane work belongs in the Cantrip fork; Blocks belongs in oracle-blocks. No backend, browser, engine feature, numeric width, or schema migration is part of this update.

## Release checkpoint

The adopted published bundle is `faces-oracle-20261112220158-s191-g0.2.2-r1`, from commit `47086128025088c55e5c9999ef721393f87f1308`. Its dependencies are **oracle-ir 0.2.1** and **oracle-ir-contract 0.2.2**, with schema **191**. Every bundle asset and the independently pinned manifest checksum were verified. `data/oracle_grammar_reference.json` records the bundle and its native parse outcomes; it is an optional native-export audit pin, not a replacement for our input.

The inspected 0.2.3 code consists of `1792711` (grammar rules), `fafc89a00840f825405ed8843f8731e8146370cd` (explicit player actors and U0046), and `4638473df6ffb4f725304b57046d84d303f34f4f` (release/model bump). The upcoming versions are grammar **0.2.3**, oracle-ir **0.2.2**, contract **0.2.2**, still schema **191**. The IR change documents Acting inside PlayersDo; it adds no field or enum layout. At this checkpoint its release workflow is running: https://github.com/vishvananda/oracle-grammar/actions/runs/43681590790 . Do not use local path patches or pin unpublished crates. The lowering is prepared for the actor contract now; adopt the matching published crate and verified faces manifest when publication completes.

## Frozen population and amendment evidence

Keep MTGish Oracle **20261001.1**, 33,529 records, SHA-256 `2281f3d715e8ee6127825154b31202918ef83a9d9a3641a0d23b0163ebea3065`. No new Scryfall comparison population was substituted.

We reviewed all **45 published amendment groups**, plus committed **U0046**: **1,006 amended leaves on 901 faces**. The regression selects the 900 complete reference records containing those faces. Its prepared-reference manifest pins the identical frozen MTGish Oracle input and exact amendment ledger, rather than assuming the newer release snapshot has identical card wording. `grammar_023_provenance.json` records the hashes. The complete reference values are preserved unchanged.

The whole-ability comparison agrees for **896 records**. Four records retain exact, input-checked differences: Infernal Denizen (contested payment amendment), Total War, Frankenstein’s Monster, and Vexing Arcanix (reported normalization questions). The test checks both sides of each difference before comparing every remaining field. These exceptions exist only in the regression evidence; production comparison and saved reference values are not repaired. Drach’Nyen, Protection Racket and Ensnared by the Mara have additional focused scope/population controls.

The shared contract canonicalizer stays at the comparison boundary. Applying all its reductions to fresh lowering would rewrite thousands of unrelated outputs and erase useful producer distinctions. This update does not introduce that broad producer rewrite.

## Decisions implemented

* Bind explicitly shared X once on Bone Devourer, Necropotence Avatar, Emergent Woodwurm (including backup), Kiora and Thassa’s Oracle. Later selections and comparisons read the binding.
* Limited Resources completes all choices before every player sacrifices the rest. Creeping Inn counts only linked creature cards. Death Charmer’s life loss belongs to the damaged creature’s controller.
* High Tide, Bubbling Muck and Overabundance give mana to the tapper. The first two were already corrected upstream but missed in our fork.
* Maelstrom Archangel Avatar retains separate positive permanent and instant/sorcery copy tests. The accepted historical combat-participant query from rejected #125 is retained.
* Static linked-card formulas use existing explicit aggregate Source constructors. Phyrexian Ingester sums creature cards; Drach’Nyen sums all linked cards, including Vehicles with printed power in exile. Scalar immediate-exile, paid-object and LKI expressions on other cards are preserved. No lowerer heuristic guesses a linked population from “static ability”.
* Fixed-player next-untap effects retain that player in both the rule and its deadline. Live controller rules retain their own meaning.
* Player-action policies bind the doer as Acting without rebinding Source You. An ordered “repeat this process for each opponent” loop only binds EachablePlayer: Protection Racket still reveals YOUR library while the opponent pays. Ensnared by the Mara still lets YOU cast the opponent’s exiled card. Fresh token abilities retain their own ability controller.
* Avoid redundant max(0, n) only when n is proved nonnegative (counts, unspent mana, or suitable arithmetic on such values). Signed power/toughness expressions and genuine negative-count clamping are preserved.

The fresh parser changes **14 Source records**, with matching legacy tagged JSON and RON. All 182 existing template findings retain their IDs and matched evidence, with zero unreviewed template or literal findings. The final proof JSON contains the complete changed-record inventory and validation counts.

## Validation

The full Rust adapter suite passes **1,201 tests** (10 ignored), across 133 targets including doctests. Go tests and all eight Python comparison tests pass. A fresh export validates all **33,529 records** with **zero diagnostics**, in 6 seconds with about 155 MiB peak RSS.

Exactly **73 IR records** change: 14 parser corrections, 9 player-scope corrections (the eight U0046 faces plus Protection Racket), 24 fixed-player untap rules, and 26 proved-redundant nonnegative clamps. Every other IR record is unchanged. The proof asserts that every clamp removal is exactly max(0,n) → n, and classifies every other changed record. No general canonicalizer was added to fresh lowering.

## Reports and remaining work

New MTGish issues: [#293](https://github.com/i5jb/mtgish/issues/293) Limited Resources, [#294](https://github.com/i5jb/mtgish/issues/294) Creeping Inn, [#295](https://github.com/i5jb/mtgish/issues/295) Death Charmer, [#296](https://github.com/i5jb/mtgish/issues/296) positive copy guards, [#297](https://github.com/i5jb/mtgish/issues/297) explicit linked-card populations. Existing reports received [shared-X examples on #239](https://github.com/i5jb/mtgish/issues/239#issuecomment-6889639683) and [the adjacent Overabundance case on #148](https://github.com/i5jb/mtgish/issues/148#issuecomment-6889639870).

Oracle Grammar: [#29](https://github.com/vishvananda/oracle-grammar/issues/29) requests consistent, scope-proved representations for Total War, Frankenstein’s Monster and Vexing Arcanix; [#30](https://github.com/vishvananda/oracle-grammar/issues/30) reports the adjacent Drach’Nyen reference correction.

**Do not blindly adopt U0044.** On rechecking [the current rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020261106.pdf), CR 118.3 and 118.12 support the existing mandatory resolution-time cost representation. An actual sacrifice count is not generally a payment-start receipt under replacements. We withdrew the earlier certainty about partial payment in [oracle-grammar #24](https://github.com/vishvananda/oracle-grammar/issues/24#issuecomment-6889686959), retained PayCost/PaymentChosen, and independently adopted U0046’s valid opponent-actor correction. The native packet’s one-Swamp scenario is not independent rules evidence. Reconcile that claim before propagating it to another consumer or editing an engine.

Next publication work is limited to adopting the completed 0.2.3 registry/bundle pins and reconciling the reported ledger/reference questions. Do not resume the paused 57-card backlog, update Cantrip integration pins, or publish browser/Wasm changes as a side effect of this adapter task.
