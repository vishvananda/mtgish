# Generator citations for early-set cards

Implementation workers reported several cards as generator bugs because their comments cited a rule for the wrong
kind of callback. An Aura's enchant filter, an activation cost filter and an activation restriction were documented
as resolving effects; replacement callbacks were documented as static abilities. Cards built entirely from shared
helpers always cited static abilities, including Boomerang and activated mana abilities.

The formatter now derives those roles from callback registrations: `enchant`/`target`, cost builders,
`cast_only_if`, and an ability's or trigger's `.condition`. Proposal callbacks cite replacement effects. This
distinguishes activation restrictions from intervening trigger conditions despite their identical Rust signatures.
Shared-only card comments distinguish keywords, spells, triggers, activated mana abilities, other activated
abilities and static abilities. Exiling an object cites CR 701.13a rather than the rule about face-down exile.
Existing untap-action citation handling also resolves older cached output that cited the untap step.

Regression coverage uses 28 complete source/Oracle records, plus callbacks whose ambiguous names cannot determine
their role. The focused and related generator suites pass 78 tests. The pipeline suite passes 271 tests, including
subgame exclusion, Jumpstart theme-card disambiguation and interrupted intake recovery. No card passes review
merely because its citation blocker has been removed; regenerated candidates still take the normal lane gates.

The intake expansion catalogs all 14 core and expansion editions released through Ice Age, including the foreign
border and Summer Magic editions. Local MTGJSON files supply available membership data; complete fetched
printing lists are cached for the remaining editions. Their union contains 1,331 names: 11 exclusions, 510
already queued cards, 193 other supported cards, and 617 additional candidates. Of those candidates, 405 have
complete generated output and 212 have generator gaps. This is an intake inventory, not a completion claim.

Ante, subgame and dexterity cards are excluded. Flavor bans are included. Generator gaps remain unreserved.
Intake uses the validated worker snapshot to recognize support, and a replay reconciles an existing queue write
before rereading remote claims. An ambiguous API response remains recorded in the journal for a fresh claim
lookup before retrying; it is never treated as proof that a card was reserved.
