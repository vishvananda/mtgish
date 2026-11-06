# Printed selector domains and target sets

Independent review of complete local Oracle text and current grammar templates confirms six pending catalog findings and one additional Radiance caller. These are upstream English-template defects; no Rust lowering or public schema change is required.

| Card | Correction | Preserved operands |
| --- | --- | --- |
| Bathe in Light | Reinsert the second captured permanent selector into the other shared-color recipients; its actual value is Creature. | Announced creature target, chosen color, protection and end-of-turn duration. |
| Wojek Apothecary | Require Other and Creature in the shared-color prevention branch. | Target appears once, one damage prevented for each recipient, unrestricted damage sources, this-turn duration and tap cost. |
| Fated Clash | Union the two selected target identities instead of intersecting them. | Conditional flash, both opposing-control creature targets, indestructible, duration and subsequent destruction of all creatures. |
| Rukarumel, Biologist | Restore you-control on the creature-spell type-change branch. | Enter choice, Sliver/nontoken-creature battlefield union, owned nonbattlefield cards, chosen-type addition and token activation. |
| Soul Diviner | Restore you-control on the artifact/creature/land/planeswalker cost selector. | Four-type union, tap cost, one counter of unspecified kind removed and one card drawn. |
| Tarrian's Soulcleaver | Remove the unprinted exclusion of equipped creature from the death event. Another continues to exclude the source Equipment. | Artifact-or-creature union, battlefield-to-graveyard event, equipped-creature counter recipient, vigilance and equip ability. |
| Tranquil Domain | Use the existing negative Aura subtype predicate. | Enchantment restriction, destruction operation, regeneration behavior and spell scope. |

All nine local Oracle cards with this Radiance color-sharing wording were inspected. Brightflame, Cleansing Beam, Incite Hysteria, Rally the Righteous, Surge of Zeal, Wojek Embermage and Wojek Siren remain unchanged. Wojek Apothecary is a new independently confirmed finding discovered while reviewing Bathe in Light; it is recorded explicitly in the catalog. Every changed template's full-corpus caller set is included in the exact seven-record delta proof.

Seven Go parser regressions and seven normalized IR regressions fail before these corrections. Assertions include costs, recipient exclusions, controllers, ordering, limits and unrelated zone scopes. Fresh complete parsing changes exactly seven source records, each equal to its prior record with only the documented selector replacement. Full typed export changes those same seven records at exactly seven selector sites; all other complete operands and all other records are identical. Generated source formats and current fixtures are refreshed. All 185 audit findings retain identical complete usage; only input hashes are refreshed. One narrow literal-review entry is appended for the changed Wojek template: the existing mining pair requires the word deal while the printed prevention phrase says dealt. The exact MultipleRecipients event is independently validated above, and all prior review history is preserved. No semantic exception is added.

The public source/IR schema remains 178. No grammar-worker files, pins, reference gold, scoring comparisons, backend contracts or publication artifacts are changed. Read-only source-name and face-name comparison against the 601 pending publication cards at `4a007d87` finds no overlap. Catalog status is now 99 cards: 55 fixed and 44 pending, with eight separately unresolved contract questions. The withheld erased-domain cases and Zask rules review remain pending.

Detailed before/after receipts are in `/mnt/vizier/reference-selectors-20261106`; small durable receipts are copied under the integration evidence's `selector-domains` directory after final validation.

Final validation passes the full Go parser suite and 1,266 AST tests with Cantrip enabled (three existing native-runtime tests ignored). All 33,529 records remain structurally preserved and semantically complete.

Upstream report: [issue 211](https://github.com/i5jb/mtgish/issues/211).
