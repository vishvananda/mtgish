# Ordinary creature-type replacements

The schema-177 reference comparison exposed another parser error: ordinary effects that say an object “becomes” specified creature types were adding those types instead of replacing its previous creature types. [CR 205.1a](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt) requires replacement; CR 205.1b separately preserves types when the text explicitly retains them.

Four shared English templates now use `SetCreatureTypes`. The complete corpus identifies seven affected cards and nine corrected type-change nodes:

| Card | Corrected nodes |
| --- | ---: |
| Ageless Sentinels | 1 |
| Ascendant Spirit | 2 |
| Evolved Sleeper | 2 |
| Figure of Destiny | 1 |
| Figure of Fable | 1 |
| Tenth District Hero | 1 |
| Warden of the First Tree | 1 |

Ageless Sentinels must stop being a Wall. Ascendant Spirit and Figure of Destiny also have official rulings explicitly establishing that their abilities replace other creature types. For example, using Figure of Destiny's first ability after its last must remove Warrior and Avatar; the previous addition left them behind.

Validation:

- Six named-card parser regressions fail before the fix and pass afterward. The full corpus identifies Warden of the First Tree as another caller, and it has its own regression too.
- Six explicit-retention controls pass: Fendeep Summoner, Hedge Whisperer, Myth Realized, Doc Ock, Vihaan and Gideon Jura. Their existing types remain additive.
- The full Go suite passes. All 33,529 source records pass typed binary/JSON and pretty RON conversion and unified IR export. Exactly the seven listed records change in each representation.
- Comparing complete lowered cards before and after finds exactly nine `Add` to `Set` mode changes; every other field remains identical. All 37 guarded audit evidence hashes remain unchanged.

The Rust source, AST forms and schema are unchanged. Oracle-grammar's pinned immutable reference remains historical until its owner refreshes the source pin/export; its reference, gold, scorer and active grammar work were not edited here.

Reported upstream as [#195](https://github.com/i5jb/mtgish/issues/195), after verifying all four affected productions on current upstream and checking for existing reports.
