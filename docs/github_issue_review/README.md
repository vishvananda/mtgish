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
