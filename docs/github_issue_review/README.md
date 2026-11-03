# GitHub issue reconciliation

Reviewed every issue and comment returned for `i5jb/mtgish` on 2026-10-30: seven existing issues and five comments. The [original reports and comments](upstream.json) are saved together. The comments' examples were included: Fisher's Talent, Pestilent Cauldron, Nissa's Judgment, Tokka & Rahzar, Captain's Maneuver, Kiora, and Healing Grace.

The [comment-to-regression index](comments_review.json) links each follow-up to its card examples and semantic assertions. A subsequent API check returned eleven issues and six comments: the original seven reports and five comments, plus the four issues and one comment filed during this review. All original comment bodies are unchanged; no additional unreviewed comments were found.

This review updates 262 English templates and 371 corpus records beyond the starting local exports. All three exports change at exactly the same 371 record positions. Combined with earlier local repairs, 582 records differ from repository HEAD. Shared templates also repair cards beyond those explicitly listed upstream.

The [comparison](comparison.json) records 632 regression candidates, including related examples and controls: 347 receive additional changes, 198 were already updated locally, and 87 are unchanged references, controls, or cases already correct locally. Candidate extraction deliberately included broad name matches; these counts are not a count of independently confirmed upstream bugs. [Changed records](changed_records.json) lists all 371 updated corpus records. Exact before/after templates are retained in `data/grammar_audit_review.json`, under `github_repairs`.

| Upstream report | Additional repairs here |
|---|---|
| [#1: capture indices](https://github.com/i5jb/mtgish/issues/1) | Restore morph and other dropped ability lines; distinguish creature/land/color filters, station thresholds, modal actions, and token subtypes; fix Phabine's nonland-scaled bonus. Correct the swapped Cauldron face costs from the comment. Earlier Class costs and many amounts were already fixed. |
| [#2: dropped captures](https://github.com/i5jb/mtgish/issues/2) | Restore the affected cohorts for Agatha's Soul Cauldron, Hazel's Brewmaster, and Steward; correct controls-no-Snake, source restrictions, Rogue attack history, Liliana's discard cost, Favored Enemy, and Idris. Healing Grace gains its fixed three life immediately, independently of future prevention. |
| [#3: target counts](https://github.com/i5jb/mtgish/issues/3) | Preserve optional targets, including Nissa's Judgment from the comment; require at least one target for “one or two”; announce Terastodon's targets before its optional destruction. |
| [#4: target kinds and other fixes](https://github.com/i5jb/mtgish/issues/4) | Remove invented targets from untapping, returns, endure, and triggered damage; correct Earthbend and other target filters; restore Spacecraft plane subtypes; repair Captain's Maneuver and Fisher's Talent from comments. |
| [#5: color-specific hexproof](https://github.com/i5jb/mtgish/issues/5) | Use the existing `HexproofFrom(FromColor(TheChosenColor))` for both cards. Plain hexproof still permits its controller to target the permanent; the report's statement otherwise is incorrect, but its diagnosis of the lost color restriction is valid. |
| [#6: wrong actions/zones](https://github.com/i5jb/mtgish/issues/6) | Correct return destinations, discard/exile, life gain/loss, tap/untap, cost reduction, delayed control changes, and trigger events. Sylvan Library selects distinct drawn cards before making a separate payment decision for each. |
| [#7: missing instructions](https://github.com/i5jb/mtgish/issues/7) | Restore play permissions, draws, untaps, exile/return instructions, type changes, and delayed triggers. Preserve Bronzehide Lion's returned Aura characteristics and abilities; repair the Tale of Tamiyo repeat loop. Previously repaired cases such as Sudden Storm are retained as regressions. |

Issue #4 also mentions approximately 45 missing-target cases without naming them. The named morph, layout, ETB, and modal cases were checked, but the unnamed set cannot be reconciled card by card from the information published. Do not treat the candidate list as confirmation of that unspecified count.

## Findings filed upstream

Confirmed findings from our earlier audit that were not already listed were filed with concrete before/after examples:

- [#8](https://github.com/i5jb/mtgish/issues/8): Incremental Growth and Incremental Blight put the third counter placement on target two.
- [#9](https://github.com/i5jb/mtgish/issues/9): Four abilities count resolutions instead of activations.
- [#10](https://github.com/i5jb/mtgish/issues/10): Eladamri and Vassal's Duty lose their damage amount limit.
- [#11](https://github.com/i5jb/mtgish/issues/11): Crumb and Get It gifts a draw instead of Food and omits the target's controller restriction.
- [#12](https://github.com/i5jb/mtgish/issues/12): Eighteen double-kicker cards encode two independently optional costs as one exclusive alternative.
- [Additional examples on #2](https://github.com/i5jb/mtgish/issues/2#issuecomment-6514251308): Brilliance Unleashed loses flying; Menacing Ogre uses the chosen number instead of two counters.

A later comparison of normalized cards against Oracle text found wrong template literals, filed on 2026-10-30 from the upstream corpus:

- [#13](https://github.com/i5jb/mtgish/issues/13): Fifteen cards end "until end of turn" effects at end of combat.
- [#14](https://github.com/i5jb/mtgish/issues/14): Fifteen cards end "until your next turn" effects at end of turn.
- [#15](https://github.com/i5jb/mtgish/issues/15): Eleven cards lose "until end of turn," leaving the effect permanent.
- [#16](https://github.com/i5jb/mtgish/issues/16): Twenty-eight Domain cards count only basic lands.
- [#17](https://github.com/i5jb/mtgish/issues/17): Rank Officer's optional discard is encoded as random.

Word-token mining over every template, then a rule table, found more wrong literals; these were filed the same day and repaired locally with the earlier five:

- [#18](https://github.com/i5jb/mtgish/issues/18): Twelve cards have the wrong or a missing duration.
- [#19](https://github.com/i5jb/mtgish/issues/19): Four cards encode the wrong player.
- [#20](https://github.com/i5jb/mtgish/issues/20): Eleven cards encode the wrong action or zone.
- [#21](https://github.com/i5jb/mtgish/issues/21): Ten cards have the wrong tapped state.
- [#22](https://github.com/i5jb/mtgish/issues/22): Six searches miss zones named in the text.
- [#23](https://github.com/i5jb/mtgish/issues/23): Seventeen cards confuse power, toughness and mana value.
- [#24](https://github.com/i5jb/mtgish/issues/24): Eight cards encode upkeep as end step.
- [#25](https://github.com/i5jb/mtgish/issues/25): Twenty-six cards have another wrong literal.

Requiring one output token per English occurrence, for phrases that must survive into the encoding, then found clauses dropped from single templates. These were filed the same day and repaired locally, except Awesome Presence, whose per-blocker cost has no source form yet:

- [#26](https://github.com/i5jb/mtgish/issues/26): Thirteen cards lose "tapped".
- [#27](https://github.com/i5jb/mtgish/issues/27): Five cards encode "in a random order" as any order.
- [#28](https://github.com/i5jb/mtgish/issues/28): Twenty-six cards end "until the end of your next turn" too early.
- [#29](https://github.com/i5jb/mtgish/issues/29): Eleven cards have twice, half or rounding wrong.
- [#30](https://github.com/i5jb/mtgish/issues/30): Five cards lose "without paying its mana cost".
- [#31](https://github.com/i5jb/mtgish/issues/31): Three cards lose "face down".
- [#32](https://github.com/i5jb/mtgish/issues/32): Four cards encode "under its owner's control" as under your control.
- [#33](https://github.com/i5jb/mtgish/issues/33): Nine cards have another clause dropped or wrong.
- [#34](https://github.com/i5jb/mtgish/issues/34): Five cards encode "your next end step" as the next end step.

The primitive audit, then a scan of every "you own" template, found two more, filed and repaired the same way:

- [#35](https://github.com/i5jb/mtgish/issues/35): Two cards encode "you control" as your team.
- [#36](https://github.com/i5jb/mtgish/issues/36): Five cards lose the owner from "cards you own exiled with".

Mining word pairs between rendered cards and their Oracle text, reading what it flagged, then scanning every template for dropped captures and every "another target" for a lost "other", found the largest batch yet. These were filed the same day and repaired locally:

- [#37](https://github.com/i5jb/mtgish/issues/37): Twelve cards encode "your opponents control" as yours or anyone's.
- [#38](https://github.com/i5jb/mtgish/issues/38): Seven cards have the wrong comparison.
- [#39](https://github.com/i5jb/mtgish/issues/39): Thirteen cards lose "may" or "up to one".
- [#40](https://github.com/i5jb/mtgish/issues/40): Fifteen cards lose "other" from a target.
- [#41](https://github.com/i5jb/mtgish/issues/41): Three cards encode owner as controller.
- [#42](https://github.com/i5jb/mtgish/issues/42): Sixteen cards have another clause dropped or wrong.
- [#43](https://github.com/i5jb/mtgish/issues/43): Eighty-five cards lose "may".
- [#44](https://github.com/i5jb/mtgish/issues/44): Twenty-one triggers lose "another".
- [#45](https://github.com/i5jb/mtgish/issues/45): Seven cards lose "noncombat".
- [#46](https://github.com/i5jb/mtgish/issues/46): Three cards lose "combat".
- [#47](https://github.com/i5jb/mtgish/issues/47): Thirteen cards apply a modifier to a whole list.
- [#48](https://github.com/i5jb/mtgish/issues/48): Twelve cards have a wrong comparison.
- [#49](https://github.com/i5jb/mtgish/issues/49): Eight cards have a wrong opponent restriction.
- [#50](https://github.com/i5jb/mtgish/issues/50): Ten cards have a wrong word or value.
- [#51](https://github.com/i5jb/mtgish/issues/51): Three cards reuse one reference for two targets.

A third pass over the complete cards added since, reading each named row's Oracle text beside its template, filed these:

- [#52](https://github.com/i5jb/mtgish/issues/52): Six cards swap "if you do" and "if you don't".
- [#53](https://github.com/i5jb/mtgish/issues/53): Four Aura triggers aim at the Aura instead of its host.
- [#54](https://github.com/i5jb/mtgish/issues/54): Sixteen mandatory actions are encoded as optional.
- [#55](https://github.com/i5jb/mtgish/issues/55): Twenty-seven cards widen a strict comparison.
- [#56](https://github.com/i5jb/mtgish/issues/56): Six cards have the wrong comparison.
- [#57](https://github.com/i5jb/mtgish/issues/57): Nine cards lose "other" or "another".
- [#58](https://github.com/i5jb/mtgish/issues/58): Four cards lose a filter in a later clause.
- [#59](https://github.com/i5jb/mtgish/issues/59): Eight cards have owner and controller wrong.
- [#60](https://github.com/i5jb/mtgish/issues/60): Two cards name the wrong object.
- [#61](https://github.com/i5jb/mtgish/issues/61): Two cards encode "play" as "cast".
- [#62](https://github.com/i5jb/mtgish/issues/62): Two cards drop a clause.
- [#63](https://github.com/i5jb/mtgish/issues/63): Two cards repeat one time too few.
- [#64](https://github.com/i5jb/mtgish/issues/64): Five cards encode "mana of any type" as any color.
- [#65](https://github.com/i5jb/mtgish/issues/65): Two cards refer to an exile that never happened.
- [#66](https://github.com/i5jb/mtgish/issues/66): Breena's "that opponent" names the attacker.
- [#68](https://github.com/i5jb/mtgish/issues/68): "that player controls" dropped from choices (3 cards).
- [#69](https://github.com/i5jb/mtgish/issues/69): Benthic Anomaly token is a Zombie.
- [#70](https://github.com/i5jb/mtgish/issues/70): Peer into the Abyss: caster loses the life instead of the target.
- [#71](https://github.com/i5jb/mtgish/issues/71): Reference to a target that doesn't exist (Whip of Erebos, The Last Ronin, Etched Slith).
- [#72](https://github.com/i5jb/mtgish/issues/72): Chosen object encoded as a target (Eunuchs' Intrigues, Goblin War Cry, Scrounge).
- [#73](https://github.com/i5jb/mtgish/issues/73): Rise from the Wreck: reference to a fifth target.
- [#74](https://github.com/i5jb/mtgish/issues/74): "unless" encoded as "if" (25 cards).
- [#75](https://github.com/i5jb/mtgish/issues/75): Target restriction lost (Planeswalker's Mischief, Waking the Trolls, Alacrian Armory).
- [#76](https://github.com/i5jb/mtgish/issues/76): "combat" lost from damage (Decorated Griffin, Soltari Guerrillas).
- [#77](https://github.com/i5jb/mtgish/issues/77): Wrong player (Shadow of the Second Sun, Davriel, Soul Broker).
- [#78](https://github.com/i5jb/mtgish/issues/78): Wrong color, condition, optional or type (Esper, Gaea's Liege, Life and Limb, Chain Lightning, Chain of Plasma, Old-Growth Troll, Harold and Bob, First Numens).
- [#79](https://github.com/i5jb/mtgish/issues/79): Perennial Gravewarden: "entered" encoded as "explored".
- [#80](https://github.com/i5jb/mtgish/issues/80): Blocked by a creature: "that creature" read as the attacker (Kolaghan Aspirant, Somberwald Vigilante, Grasping Giant, Barrow-Blade, Wooden Stake).
- [#81](https://github.com/i5jb/mtgish/issues/81): Wrong reference in a body (Shifty Doppelganger, Wall of Caltrops, Infinite Authority, Siege Dragon, Mythos of Snapdax, Mirror Shield).
- [#82](https://github.com/i5jb/mtgish/issues/82): Reference with nothing to bind to (Bronze Bombshell, Corrupted Resolve, Domri's Ambush, For Each of You, a Gift, Rotted Ones, Lay Siege, Tenza, Godo's Maul).
- [#83](https://github.com/i5jb/mtgish/issues/83): Announced target never used (Altar of Dementia, Increasing Confusion, Trenchpost, Veteran Ice Climber, Helm of Obedience, Phantasmal Sphere, Auditore Ambush, Tunnel Vision, Rakdos, the Muscle, How to Start a Riot, Selfcraft Mechan, When We Were Young, Legolas, Master Archer).
- [#84](https://github.com/i5jb/mtgish/issues/84): Wrong target number (Cracked Earth Technique, Crooked Scales, Coastal Discovery).
- [#85](https://github.com/i5jb/mtgish/issues/85): Enduring cycle stays a creature (Enduring Courage, Enduring Curiosity, Enduring Friendship, Enduring Innocence, Enduring Tenacity, Enduring Vitality).
- [#86](https://github.com/i5jb/mtgish/issues/86): Valley Floodcaller: pump reads no group.
- [#87](https://github.com/i5jb/mtgish/issues/87): "becomes" a colored creature adds the color, 64 cards.
- [#88](https://github.com/i5jb/mtgish/issues/88): Ward of Bones applies to every player.
- [#89](https://github.com/i5jb/mtgish/issues/89): "same name as another" matches the object itself, six cards.
- [#94](https://github.com/i5jb/mtgish/issues/94): Anya, Merciless Angel compares life totals to half the current total, not half the starting total.
- [#95](https://github.com/i5jb/mtgish/issues/95): Deadly Complication unsuspects a trigger's permanent, not its target.
- [#96](https://github.com/i5jb/mtgish/issues/96): Coiling Rebirth reads a target permanent, not the card it returned.
- [#97](https://github.com/i5jb/mtgish/issues/97): Close Encounter deals damage equal to the target's power, not the chosen creature's or card's.
- [#98](https://github.com/i5jb/mtgish/issues/98): Luxior, Giada's Gift equips only creature planeswalkers.
- [#99](https://github.com/i5jb/mtgish/issues/99): Anya, Merciless Angel counts you as well as opponents.
- [#100](https://github.com/i5jb/mtgish/issues/100): Back from the Brink exiles from any graveyard.
- [#101](https://github.com/i5jb/mtgish/issues/101): Dinosaur Headdress's choice isn't limited to creature cards.
- [#102](https://github.com/i5jb/mtgish/issues/102): Goblin Game makes each player hide at least two items.
- [#103](https://github.com/i5jb/mtgish/issues/103): Judgment of Alexander deals damage to the recipient, not the creature.
- [#104](https://github.com/i5jb/mtgish/issues/104): Kaya, Spirits' Justice's -2 can exile any creature.
- [#105](https://github.com/i5jb/mtgish/issues/105): Krenko's Buzzcrusher forces the search and finds any land.
- [#106](https://github.com/i5jb/mtgish/issues/106): Laquatus's Champion and Soul Scourge gain the life lost, not the printed amount.
- [#107](https://github.com/i5jb/mtgish/issues/107): Nihiloor's control outlasts your control of it.
- [#108](https://github.com/i5jb/mtgish/issues/108): Rally the Horde's repeat condition is inverted.
- [#109](https://github.com/i5jb/mtgish/issues/109): Spikeshell Harrier reads and reduces the wrong player's speed.
- [#110](https://github.com/i5jb/mtgish/issues/110): Springheart Nantuko makes no Insect unless it's attached.
- [#111](https://github.com/i5jb/mtgish/issues/111): The Madcap Jester triggers on opponents' spells and abilities.
- [#112](https://github.com/i5jb/mtgish/issues/112): Winnowing can sacrifice a chosen creature with no type.
- [#113](https://github.com/i5jb/mtgish/issues/113): Benthic Explorers' mana reads the wrong permanent.
- [#114](https://github.com/i5jb/mtgish/issues/114): Signal the Clans pays out with fewer than three names.
- [#115](https://github.com/i5jb/mtgish/issues/115): Stronghold Gambit's choices aren't hidden.
- [#116](https://github.com/i5jb/mtgish/issues/116): The Toymaker's Trap's life loss reads the secret number.
- [#117](https://github.com/i5jb/mtgish/issues/117): Uldaros Theorix's limit is per spell, not a total.
- [#118](https://github.com/i5jb/mtgish/issues/118): Zevlor's copies keep the original target's kind.
- [#119](https://github.com/i5jb/mtgish/issues/119): Baldur's Gate Wilderness's Reithwin Tollhouse rolls one die.
- [#120](https://github.com/i5jb/mtgish/issues/120): Spikeshell Harrier's speed can drop below 1.
- [#121](https://github.com/i5jb/mtgish/issues/121): Galea, Kindler of Hope needs Aura or Equipment.
- [#122](https://github.com/i5jb/mtgish/issues/122): Alaundo the Seer casts the card with the ability.
- [#123](https://github.com/i5jb/mtgish/issues/123): Deification keeps "you control". Its cancelled damage stays open.

The Cantrip generator held these cards back on wrong source data. They were filed upstream and are repaired locally:

- [#2](https://github.com/i5jb/mtgish/issues/2): Quicksilver Dragon changes the spell's target to another creature, not to any other permanent.
- [#146](https://github.com/i5jb/mtgish/issues/146): Nissa, Who Shakes the World and Tendril of the Mycotyrant target a noncreature land you control.
- [#156](https://github.com/i5jb/mtgish/issues/156): Arcbond deals damage to each other creature, not to each other permanent.
- [#161](https://github.com/i5jb/mtgish/issues/161): Redeem prevents damage dealt to its targets, not damage they deal.
- [#162](https://github.com/i5jb/mtgish/issues/162): Fire Giant's Fury triggers on combat damage to a player.
- [#163](https://github.com/i5jb/mtgish/issues/163): Acidic Dagger's leaves-the-battlefield trigger is made by the activation itself.
- [#165](https://github.com/i5jb/mtgish/issues/165): Fable of the Mirror-Breaker discards up to two cards.
- [#166](https://github.com/i5jb/mtgish/issues/166): Karona, False God pumps the chosen creature type.
- [#167](https://github.com/i5jb/mtgish/issues/167), [#168](https://github.com/i5jb/mtgish/issues/168): Trait Doctoring, Whim of Volrath and Crystal Spray end the color word change at end of turn; Crystal Spray draws after any choice.
- [#169](https://github.com/i5jb/mtgish/issues/169): Bison Whistle offers the graveyard only for a card that is neither a Bison nor a creature card.
- [#173](https://github.com/i5jb/mtgish/issues/173): All Hallow's Eve has each player return their own creature cards.
- [#174](https://github.com/i5jb/mtgish/issues/174): Thought Dissector's controller sacrifices it.
- [#175](https://github.com/i5jb/mtgish/issues/175): Korvold, Gleeful Glutton counts only permanents you sacrificed. The count takes a player now: `TheNumberOfCardTypesAmongThePermanentsSacrificedByPlayerThisTurn(You)`.
- [#176](https://github.com/i5jb/mtgish/issues/176): Ravenous Vampire taps when you don't sacrifice.
- [#177](https://github.com/i5jb/mtgish/issues/177): Ajani, Nacatl Pariah, Great Fierce Bee, Vengeful Townsfolk and Soul Shredder trigger once for a batch of deaths.
- [#178](https://github.com/i5jb/mtgish/issues/178): Sink into Stupor and Press the Enemy return only a nonland permanent an opponent controls.

[#67](https://github.com/i5jb/mtgish/issues/67) was filed in error and closed: Shifting Loyalties keeps its restriction in the old encoding.

The issue payloads and comment text are saved here. No commits, pushes, issue closures, or upstream code changes were made. Disputed continuous-effect instruction grouping remains a rules-review question and was not filed as a confirmed bug. Changes that merely replace an equivalent scalar expression are not reported as new card mistakes.

Issues #8–#12 and our additional-examples comment on #2 have been shortened to describe only the specific problems and examples. Rust port, local repair, and validation details were removed from the upstream reports. [The latest upstream read](latest_upstream.json) verifies all five edited issue bodies and the comment; saved issue payloads match them. Future reports should follow the same concise format.

## Validation

- Full parser pass: 33,528 successful records. It exposed one overlapping syntax production for Racketeer Boss; removing the redundant production fixed it. Seven complete records, including Racketeer Boss and the final filter/index repairs, then passed targeted parsing. Their successful outputs were merged by identity into the staged corpus.
- Strict Rust deserialization and baseline identity/layout verification cover all 33,529 final records. RON-to-binary and binary-to-JSON conversion also cover all records; pretty RON is synchronized. There are no skipped records.
- The Go suite passes in about four seconds: 34 earlier card cases plus 632 GitHub regression candidates. Of the latter, 146 have explicit semantic assertions. The others provide syntax coverage for related examples and controls.
- Twenty-five focused Rust tests pass. New typed assertions cover Sylvan Library's selection/payment scopes, Captain's Maneuver's restricted targets, chosen-color hexproof, untargeted untapping, and buyback reduction. Nineteen representative source records round-trip through RON and binary serialization.
- One focused test compares every union arm in the eleven affected Rust/TypeScript declarations. It passes without compiling the giant TypeScript corpus.
- Four formerly reviewed capture findings are now repaired. The remaining 188 finding IDs and their complete matched evidence are unchanged: 151 scalar equivalences and 37 guarded context reviews. No card-name exceptions were added. These checks do not make the detector a complete semantic verifier; several GitHub bug families require the explicit card regressions.

At the repair checkpoint, the source Rust schema represented the full corpus, while generic normalization covered 4,496 of 33,529 records using 12 operation kinds. Corrected source semantics exposed five previously hidden capability gaps (the entering-event alias, untargeted optional object selection, and the modified-permanent filter); Venom's corrected dies trigger became supported, giving a net reduction of four complete records. Subsequent AST implementation has added these generic capabilities; current progress is recorded in [AST_PLAN.md](../../AST_PLAN.md).


## Reference findings, November 3, 2026

The Oracle-grammar report REF-001 traced named artifact-creature animations back to nine English templates emitting `AddCreatureType` where CR 205.1b requires `SetCreatureType`. [Upstream #184](https://github.com/i5jb/mtgish/issues/184) covers the original 15 cards (17 nodes) and four more found by complete regeneration: Dire Mimic, Mimic, Sanguine Statuette and Xanthic Statue. The repaired corpus changes 21 creature-type modes across 19 cards. Bare animations and explicit retention wording retain their existing behavior.

REF-002 is the previously reported [Restless Cottage #134](https://github.com/i5jb/mtgish/issues/134). Its attack trigger now accepts a card in any graveyard, retaining its card domain, graveyard zone, optional single target, Food creation and exile instruction. Its separate land animation is unchanged.

The complete 33,529-record parse changes exactly these 20 cards. Whole-record Go regressions cover all 20 plus four controls; Rust regressions compare complete semantic trees while allowing only the specified 22 field changes. [Fresh export evidence](reference-mistakes-20261103.json) records their paths. The pinned Oracle-grammar reference cache and scorer were not edited; consumers must regenerate from the corrected corpus before updating their pins.

Validation: the Go suite and 33 focused Rust tests pass; the template audit applies its input review and retains one pre-existing Cauldron Dance unused-capture finding; the literal audit has zero unreviewed findings. The existing `ast_baseline.json` identity check fails on both the unchanged parent corpus and the repaired corpus. Direct before/after comparisons verify identical record counts/order and no changes outside the intended operands; the stale baseline is not refreshed as part of these repairs.
