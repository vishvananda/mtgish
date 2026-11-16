# MTGish report actionability review — 2026-11-15

## Subsequent feedback — 2026-11-16

MTGish #290 is withdrawn: the maintainer clarified the inspection-list contract;
Lamp needs a consumer fix, not a new Source constructor. #289 is now a disputed
interpretation: a judge precedent for Emeria Shepherd needs reconciliation with
the maintainer’s Snowfall reading. The dated inventory below is historical. See
[the follow-up](upstream-feedback-289-290-20261116.md).

## Finding

[#202](https://github.com/i5jb/mtgish/issues/202) was a downstream generator bug.
The upstream parser already emits
`AttackedDuringLastPlayersTurn(ItsController)` for Halls of Mist. `ItsController`
belongs to the current candidate; our generated Rust shadowed that candidate.
The issue's title and body now withdraw the upstream request and a reply
acknowledges the maintainer's explanation.

The review inventoried all **290 reports authored by vishvananda**, read the
available issue comments, rechecked all **96 initially open reports** against
current upstream, and corrected previously closed reports whose opening claims
still contradicted maintainer feedback. Seven reports by other authors were
outside the edit scope. Accepted historical closed reports were not reopened or
represented as newly proved gameplay defects.

## Open-report dispositions

| Disposition | Reports | Reason |
|---|---|---|
| Close: consumer bug or no established producer defect | [203](https://github.com/i5jb/mtgish/issues/203), [221](https://github.com/i5jb/mtgish/issues/221), [240](https://github.com/i5jb/mtgish/issues/240), [265](https://github.com/i5jb/mtgish/issues/265), [284](https://github.com/i5jb/mtgish/issues/284) | Source already preserves the necessary information, or the report assumed a Source contract without establishing it. |
| Close: optional representation proposals consolidated into #275 | [258](https://github.com/i5jb/mtgish/issues/258), [296](https://github.com/i5jb/mtgish/issues/296) | The earlier trigger-count and absent-copy gameplay claims were not established. |
| Close: already fixed upstream | [214](https://github.com/i5jb/mtgish/issues/214), [252](https://github.com/i5jb/mtgish/issues/252), [271](https://github.com/i5jb/mtgish/issues/271) | Fresh upstream parsing already uses the requested replacing creature-type constructors. |
| Retain: parser, script or data correction | 79 | Each body names the upstream location, current expansion/data and requested edit. |
| Retain: explicit clarification, classification, generic contract or normalization proposal | 7 | These are explicitly distinguished from demonstrated runtime bugs. |

The expected author-specific open count is **86**, down from 96. It is an issue
count, not a card count or a count of independently proved gameplay defects.
The [machine-readable inventory](mtgish-report-actionability-20261115.json) gives
every report's disposition and every retained report's exact requested change
and producing-rule locations.

The seven qualified proposals are
[#213](https://github.com/i5jb/mtgish/issues/213),
[#260](https://github.com/i5jb/mtgish/issues/260),
[#273](https://github.com/i5jb/mtgish/issues/273),
[#275](https://github.com/i5jb/mtgish/issues/275),
[#282](https://github.com/i5jb/mtgish/issues/282),
[#287](https://github.com/i5jb/mtgish/issues/287), and
[#297](https://github.com/i5jb/mtgish/issues/297). In particular, a helper's name
alone does not prove successful-destruction gating or a creature-only linked
exile domain. Those reports request explicit references/domains and acknowledge
that a correct existing contract would instead require a consumer fix.

## Important scope corrections

- **Winds of Change (#203):** separate each-player shuffle and draw phases are
  already present. Preserving each player's result belongs to the consumer.
- **Rakdos (#221):** `PlayerAction(Trigger_ThatPlayer, ...)` already identifies
  whose permanents are sacrificed. Our population scoping was not proof that
  Source needed a redundant controller filter.
- **Villainous choices (#240; part of #205):** the opponent chooses an
  alternative; that does not transfer every unqualified action to the opponent.
  The Master finding is withdrawn from #205; nine other cards retain explicit
  incorrect-recipient/payer/source requests. The misleading four-card follow-up
  on #240 is corrected as well.
- **Kogla and Yidaro (#265):** Source already declares the discard activation
  and graveyard return origin. Post-cost identity and an absent return target
  must be handled correctly downstream.
- **Frankenstein's Monster (#284):** our all-or-nothing interpretation of
  `MustCost` inside `AsPermanentEnters` does not establish an upstream contract.
  The bug claim is withdrawn pending such evidence; no new rules ruling is
  claimed by this audit.
- **Nyla (#241):** the concrete request is to remove an unprinted optional
  resolution-time decline. An ordinary effect is not the only possible correct
  encoding; a required resolving-cost representation may be appropriate.
- **Codsworth (#276):** only its mana restriction still uses the Aura/Equipment
  intersection. Danitha, Golden-Tail Trainer and Sigarda's Aid are already-correct
  controls and are removed from the outstanding fix list.
- **Fixed-player untap (#281):** the requested representation qualifies both the
  prohibition and its expiry with the printed player. Merely changing the
  deadline while retaining a controller-relative rule would be incomplete.
- **Linked exile (#297):** our index-zero behavior was a lowering mistake. The
  surviving proposal explicitly distinguishes printed card/creature-card
  domains; it does not infer the existing helper's runtime contract from its
  name or demand a particular backend receipt implementation.

Fifteen older closed reports also receive corrected opening bodies/titles:
[#33](https://github.com/i5jb/mtgish/issues/33),
[#81](https://github.com/i5jb/mtgish/issues/81),
[#82](https://github.com/i5jb/mtgish/issues/82),
[#89](https://github.com/i5jb/mtgish/issues/89),
[#106](https://github.com/i5jb/mtgish/issues/106),
[#120](https://github.com/i5jb/mtgish/issues/120),
[#125](https://github.com/i5jb/mtgish/issues/125),
[#143](https://github.com/i5jb/mtgish/issues/143),
[#154](https://github.com/i5jb/mtgish/issues/154),
[#169](https://github.com/i5jb/mtgish/issues/169),
[#170](https://github.com/i5jb/mtgish/issues/170),
[#172](https://github.com/i5jb/mtgish/issues/172),
[#188](https://github.com/i5jb/mtgish/issues/188),
[#190](https://github.com/i5jb/mtgish/issues/190), and
[#191](https://github.com/i5jb/mtgish/issues/191).
These remain closed; genuine independent findings in mixed reports remain
identified. #67 already stated that it was filed in error and needs no new edit.

## Evidence and limits

The upstream revision checked is
[`9c59b9b4cbf09c2ff99003b46f3c397b0d88566a`](https://github.com/i5jb/mtgish/commit/9c59b9b4cbf09c2ff99003b46f3c397b0d88566a).
Its parser and grammar were downloaded to scratch storage and built separately;
no upstream changes were merged into this fork. The unmodified executable ran
against our existing frozen local inputs, not a newly downloaded Scryfall
population. The full output includes parse failures and is not a claim that all
upstream cards parse successfully.

A second executable records matched English templates through a side channel.
Every one of its 439 output records is byte-identical to a record in the
unmodified parser's full output. Producing-rule links are pinned to the upstream
commit. The unsupported-layout stdout failure (#277) was reproduced directly
with upstream preprocessing. The incorrect Octopus token (#291) was inspected
in upstream token data separately, since our local token record is already
correct.

Original issue bodies/comments, pristine output, producing-rule traces,
before/after API responses and edit payloads are retained at
`/mnt/vizier/mtgish-report-actionability-20261115/`. The committed inventory
records content hashes and portable upstream locations; the bulky scratch
archive is not a runtime dependency.

This review changes reports and documentation, not parser/lowering code or
published exports. No Rust rebuild is needed. Verification covers report scope,
Source evidence, remote body/state agreement and inventory consistency. Future
reports should follow [upstream-reporting.md](upstream-reporting.md).
