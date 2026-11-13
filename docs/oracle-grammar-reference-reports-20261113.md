# Oracle Grammar reference reports from early-set runtime work

Confirmed reference defects now have issues in `vishvananda/oracle-grammar`.
The repository's MTGish adapter still pins `d4b172343b06f8887f700b425ebf69a67acf8efa`,
schema 190. I inspected the seven representative faces in the pinned native
reference as well as the original Source; the reports are not deductions from
Cantrip output alone. Family populations come from exact before/after frozen
Source diffs. The Oracle input is still MTGish `20261001.1`.

| Confirmed reference group | Oracle Grammar | Related MTGish parser issue | MTGish fork fix |
|---|---|---|---|
| Prevention source/recipient (nine Source records) | [#4](https://github.com/vishvananda/oracle-grammar/issues/4) | [#278](https://github.com/i5jb/mtgish/issues/278) | [91a2fe43](https://github.com/vishvananda/mtgish/commit/91a2fe43f26c2cd46e9120f58809c64182664921) |
| Gauntlets of Chaos Aura restriction | [#5](https://github.com/vishvananda/oracle-grammar/issues/5) | [#201](https://github.com/i5jb/mtgish/issues/201) | [855deccc](https://github.com/vishvananda/mtgish/commit/855deccca5ba5b32312b3e01835094af5d71b2ed) |
| Mercenaries damage source | [#6](https://github.com/vishvananda/oracle-grammar/issues/6) | [#279](https://github.com/i5jb/mtgish/issues/279) | [4ba5c00f](https://github.com/vishvananda/mtgish/commit/4ba5c00f6e5a1b5f32abf8e017a10ccf74bd9fff) |
| Juxtapose artifact exchange | [#7](https://github.com/vishvananda/oracle-grammar/issues/7) | [#280](https://github.com/i5jb/mtgish/issues/280) | [4ba5c00f](https://github.com/vishvananda/mtgish/commit/4ba5c00f6e5a1b5f32abf8e017a10ccf74bd9fff) |
| Named next-untap player (17 Source records) | [#8](https://github.com/vishvananda/oracle-grammar/issues/8) | [#281](https://github.com/i5jb/mtgish/issues/281) | [7fed1a80](https://github.com/vishvananda/mtgish/commit/7fed1a80affa9e6ed5067210333243e3c3ada4b5) |
| Thermokarst / Extinguish the Light target after destroy | [#9](https://github.com/vishvananda/oracle-grammar/issues/9) | [#282](https://github.com/i5jb/mtgish/issues/282) | [06de9c78](https://github.com/vishvananda/mtgish/commit/06de9c786d700414985cb9cf960402169039b830) |
| Vexing Arcanix reveal existence | [#10](https://github.com/vishvananda/oracle-grammar/issues/10) | [#283](https://github.com/i5jb/mtgish/issues/283) | [06de9c78](https://github.com/vishvananda/mtgish/commit/06de9c786d700414985cb9cf960402169039b830) |

Each issue states the incorrect reference meaning, corrected semantics and
runtime witness, with controls where tested. The repair commits contain the
Source/parser changes and scoped regression fixtures. The complete fresh parse
matches all 33,529 checked-in frozen Source records after the fixes. These
corrections use existing schema forms.

## Follow-up workflow

- File confirmed gold/reference meaning defects on Oracle Grammar as they are
  discovered. Include the pinned reference revision, named face/path, before and
  expected values, Oracle wording, and a distinguishing witness or rules argument.
- Link a separate MTGish report when its parser or Source translation needs a fix.
  Keep the two trackers cross-referenced so either worker can see adoption status.
- The grammar worker can apply checked corrections through
  `adapters/mtgish/updates.json`, or adopt a verified corrected reference pin.
  Preserve its immutable reference artifacts and keep model releases separate.
- Generator-only defects remain MTGish backend work. Necropotence's suppressed
  empty-capture delayed trigger and Urza's Miter's native look-back history reads
  are examples; their reference meanings were already sufficient.
- Orcish Farmer already retains its dynamic controller in IR. Its missing native
  until-next-untap deadline is a Cantrip interface gap, distinct from the wrong
  named-player references reported in #8.

I did not edit the other worker's Oracle Grammar checkout or advance its pins.
The issues provide the handoff for its reference-update process.
