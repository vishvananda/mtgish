# Retained permanent domains in three event families

A newer worker packet reports extra current-card-type restrictions on Herald's Reveille, The Balrog, Durin's Bane and Yuna, Grand Summoner. Independent review confirms these three. Full typed caller review finds additional occurrences on It That Betrays and Sawblade Skinripper. The source already names permanent selectors; this is a fork binding correction and needs no upstream English-parser issue.

The previous permanent-domain fix conservatively kept the six-type filter at erased-domain boundaries. Three event constructors independently retain the missing domain:

- `WhenAPermanentIsPutIntoAPlayersGraveyard` lowers to a zone change with explicit `from: Battlefield`. This covers Yuna's trigger and preserves its pre-departure view.
- `WhenAPlayerSacrificesAPermanent` lowers to the existing Sacrifices event. The unchanged Cantrip matcher already requires `from.is(ZoneKind::Battlefield)`, consistent with CR 701.21a. This covers Balrog's history query, It That Betrays' trigger and both Sawblade Skinripper history queries.
- `WhenAPermanentExplores` lowers to the existing Explores event. Its unchanged backend row names `Event::Explored { permanent, .. }`, consistent with CR 701.44. This covers Herald's Reveille's history query. That backend row is already planned; this correction makes no runtime execution or new engine-support claim.

The rules were checked in the [pinned primary text](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt). These are source-contract and normalized-meaning checks. No backend contracts or generator modules change.

A small internal filter helper clears the conservative binding guard only inside those three constructors. It interprets the existing `PermanentDomain` provenance as Any because the selected event independently provides the permanent domain. Literal six-card-type disjunctions remain literal filters. Permanent-card triggers from anywhere retain both their six-type requirement and token exclusion. Other raw events, conditions, counter replacements and targeting events retain their old guards; no general erased-domain relaxation is made.

The complete IR comparison changes exactly five records at six sites, replacing only the exact six-card-type disjunctions with Any. All other operands and every other record are identical. Herald keeps the controller and this-turn exploration predicate; Balrog keeps its sacrifice window, generic price reduction and all other abilities; Yuna keeps Other(Source), controller, battlefield origin, graveyard destination, departing-counter test and target; It That Betrays keeps opponent, nontoken, existing source exclusion and return instruction. Sawblade keeps identical condition and damage history, its controller, turn window, one-or-more threshold, damage source and any-target announcement. The existing source exclusion on It That Betrays is outside this correction and preserved; its opponent qualifier already prevents its own controller's sacrifice from matching.

Five actual-card regressions fail before/pass after. Two negative-control tests pass before and after, preserving explicit card types in all three event families and the permanent-card from-anywhere domain. The existing permanent-domain suite also passes, including unchanged raw counter, targeting and counter-replacement guards. Of the original conservative cases, 27 filter sites in 27 records remain withheld, including Solid Ground. `remaining-withheld.json` records every retained site separately.

The complete parser source corpus, English/syntax grammars, audit usages and all review history are byte-identical to `e7203ef`; no parser regeneration is required. No public type or schema changes, reference-gold edits, equality aliases, exceptions, worker changes or publication-artifact edits are involved. Schema remains 178. The five source names and face names have zero overlap with the 601 pending publication cards at `4a007d87`.

The catalog now has 104 cards: 60 fixed and 44 pending, with eight separately unresolved contract questions. Detailed receipts are under `/mnt/vizier/reference-permanent-events-20261106`; small durable receipts are copied into the integration evidence's `permanent-events` directory after validation.

Final validation passes the full Go parser suite and 1,273 AST tests with Cantrip enabled (three existing native-runtime tests ignored). All 33,529 corpus records remain structurally preserved and semantically complete.
