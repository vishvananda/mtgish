# Snowfall's optional snow replacement

The snow branch must retain the original one-blue choice. Snowfall first
permits an Island's controller to add one extra blue. For a snow Island, that
controller may replace it with two extra blue instead. Declining the optional
self-replacement leaves the original choice available: snow offers zero, one,
or two extra blue; nonsnow offers zero or one.

The existing `IfElse` and `PlayerMayAction` remain. The true branch's production
changes from `And([U, U])` to `Or([U, And([U, U])])`. Typed lowering already
supports this as `ManaProduction::ChooseOne`, inside `Instruction::Optional`.
Both decider and recipient remain the tapped Island's controller. Every extra
unit remains restricted to cumulative upkeep. No AST or wire schema changes.

This differs from closed MTGish #43, which restored the missing outer optional
action. It did not preserve the original amount when the snow replacement is
declined. The official 2008-08-01 ruling that the added mana is always blue,
regardless of the Island's own output, remains unchanged.

The rules basis is CR 614.1a ("instead"), 614.15 (self-replacement), and the
replacement choice procedure in CR 616.1. See the
[Comprehensive Rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260619.pdf).
Reports: [MTGish #289](https://github.com/i5jb/mtgish/issues/289) and
[Oracle Grammar #25](https://github.com/vishvananda/oracle-grammar/issues/25).

## Evidence and reference adoption

- The full parser consumed the same frozen 33,529-record Oracle, supplemental,
  token and dungeon input. Comparing complete records as multisets changes
  Snowfall alone. Existing token/dungeon ordering and all untouched export
  bytes are preserved in the checked-in files.
- RON, tagged JSON, and pretty RON each change only Snowfall. Pretty RON was
  produced by the existing Source serializer, not a new formatting policy.
- The fresh matched-template audit has exactly the same 182 finding IDs and
  full evidence fields as the prior reviewed report. Only the English grammar
  fingerprint changes in the source and parser-input maps. Existing historical
  findings and dispositions are retained; no exception was added.
- `go test ./...` passes, including the existing #43 regression updated to
  retain its outer-May check and assert the additional one-blue choice.
- The Source-parser regression compares the whole card. The typed regression
  validates the lowered card and independently checks optionality, both snow
  amounts, the nonsnow amount, controller identities, and spending purpose.
- Native card drafts exercise all three snow amounts, but are still drafts
  pending independent review and the combined native validation gate. This
  reference fix does not claim a finished Cantrip card or complete generation.

The published Oracle Grammar bundle/model pins and input population stay fixed.
The active Source digest advances for this one approved correction, bringing
`approved_changed_records` from six to seven. The other worker should apply an
approved reference update through `adapters/mtgish/updates.json` or adopt the
corrected source export, preserving its pinned export in place. No unrelated
Oracle Grammar working files were edited.
