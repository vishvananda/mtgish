# Continuous-effect and discarded-source reference corrections

Five cards are corrected using existing schema189 representations. Four findings
came from oracle-grammar's schema189 continuous-effect packet; the separate Kogla
and Yidaro contract question exposed one additional lowering defect.

| Card | Correction |
| --- | --- |
| Emergent Sequence | Enter tapped, shuffle, then animate the actual moved land. Animation is not an arrival characteristic. Counters still use that land and the original turn-history count. |
| Spike Tiller | Add Creature, retaining Land, before the counter instruction. Adding Land previously did nothing. |
| Tatsunari, Toad Rider | Prohibit blocking by the complement of flying-or-reach creatures. The old positive filter prohibited the expressly permitted blockers. Both Tatsunari and its announced Frog are retained. |
| Sarkhan, the Dragonspeaker | Set card types to Creature and creature subtypes to Dragon. Preserve Legendary, red, base 4/4, flying, indestructible, haste and end-of-turn duration. |
| Kogla and Yidaro | Read the actual discarded card from its activation cost receipt, check the activating player's graveyard, and shuffle explicitly when that card is absent. Drawing remains after either branch. |

## Evidence and representation

[CR205.1a/b, 400.7j, 608.2c and 701.24c](https://media.wizards.com/2026/downloads/MagicCompRules%2020261106.pdf)
support replacement versus retained types, following a paid object's first public
zone incarnation, printed instruction order, and shuffling despite absent cards.
For example, Emergent Sequence must not trigger Essence Warden as its land enters.
Spike Tiller makes a land a creature; Sarkhan stops being a planeswalker; a ground
creature cannot block Tatsunari while a flying or reach creature may do so.
Explicit-retention controls Cacophony Unleashed and Mishra's Factory stay additive.

The four original complete proposed programs are independently authored in
`~/oracle-grammar/reports/analysis/reference-issues/schema189-continuous-effects/`.
Regression expectations preserve those programs; only normal canonical ordering
and unused result numbering differ from the catalog. Their native-before frames
are also checked after canonicalization. This is not an equality alias between
the old wrong semantics and the repair.

Kogla's [published text and target ruling](https://magic.wizards.com/en/news/feature/march-of-the-machine-release-notes)
explicitly name the graveyard. Choosing no artifact/enchantment target lets the
ability resolve even if the discard is replaced by exile. The card must remain
exiled while its controller still shuffles and draws. If a chosen target becomes
illegal, the entire ability fails to resolve, as before.

The old Source form `ShuffleGraveyardCardIntoLibrary(ThisGraveyardCard)` loses
both the post-cost incarnation and expected zone. `lower_zones.rs` resolves
`ThisGraveyardCard` to `Source`; the Cantrip reader emits `asker.source`, and the
movement API rejects stale objects. The local engine at `f64f55b` already shuffles
an identified owner's library even for a stale object in `put_into_library`;
that does not make the old hand object the discarded card. No implicit graveyard
restriction is carried by that normalized Move. This is a source/lowering trace,
not a newly executed gameplay test.

The corrected Kogla reference uses a `Query` over
`CostObjects(Activation, component[1], Performed, After)`. Both the condition and
move read that same receipt. A singular discarded-card reference rejects plural
or ambiguous discard costs; action-based discard references retain their existing
transition result (Aether Rift, Leonardo da Vinci and Silvan Reveler are exact
unchanged controls). An `If` with a shuffled Move in the graveyard branch and an
explicit Shuffle in the other branch preserves exactly one shuffle even with no
card. The draw is outside the condition. The existing Source condition production
is generalized to its already typed card/player arguments; no Source Rust type
or normalized schema changes are needed.

## Validation

The accompanying [proof](reference-continuous-20261111.proof.json) records the
final corpus comparison and test results. Complete-card Go tests exercise actual
Oracle parsing. Rust tests compare complete validated programs, including all
unmodified abilities, costs, announcements and characteristics, and check tagged
Source JSON against RON. Payment, discard, movement, animation and rule regressions
cover nearby behavior. Full corpus and actual-workspace browser checks use the
same Rust/Wasm validator as the deployed editor.

The normalized AST, numeric widths, validator, migrations and human editor API
are unchanged. The existing schema189 Wasm is reused: its exported operations do
not invoke Source lowering. No Cantrip engine feature is added, and this does
not claim complete backend gameplay execution. Original oracle-grammar evidence,
gold, aliases and reference pins are untouched; a sibling resolution note records
these fixes for its worker.

The Kogla generator probe explicitly reports `engine: no record of what paid a
cost`. This pre-existing backend gap remains visible. Correct reference IR does
not imply executable Cantrip support, and an incorrect Source reference must not
be used to hide the gap.

## Upstream reports

Fix commit: `1cd0b57efcf32b842557a9bb74b985df98da3daa`.

- [Emergent Sequence: animation is applied before entry instead of after shuffling](https://github.com/i5jb/mtgish/issues/262)
- [Spike Tiller omits Creature; Sarkhan retains Planeswalker and old creature types](https://github.com/i5jb/mtgish/issues/263)
- [Tatsunari reverses its flying-or-reach blocker exception](https://github.com/i5jb/mtgish/issues/264)
- [Kogla and Yidaro loses the discarded incarnation and explicit graveyard origin](https://github.com/i5jb/mtgish/issues/265)
