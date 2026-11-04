# Ordered ability replacement

Implemented on `refactor/semantic-ability-changes`, based on `62b6d53`, implemented in `31f58846c5b54cc1fd956e456f7e537927ebde90`. This
completes the plural ability-replacement candidate from the user's untracked
`consolidation.md`; the input file and earlier proof directories are preserved.

## Canonical contract

`Change::OnlyAbilities` is removed. Its enclosing ordered change list now holds
`RemoveAllAbilities`, followed by `GrantKeyword` for each keyword ability and
`GrantAbility` for each other ability, in the original order. The enclosing
static ability, continuous effect, entry specification, instruction result,
duration and evaluation policy are unchanged. No extra operation or effect
boundary is introduced.

The public source reducer is now `semantics::reduce_changes`, returning a list.
All normalization callers consume that list, including static/resolving changes,
spell effects, perpetual changes and nested static terms. A private scalar
adapter accepts exactly one change in scalar-only characteristic positions; it
cannot silently drop a compound reduction. Nested ability declarations and
`Copiable`/`Continuing` lists retain their boundaries.

The static and resolving generators no longer expand a semantic wrapper. They
consume ordinary ordered changes. The existing engine calls retain their order,
which preserves ability loss before grants, layer ordering, grouping and native
timestamps. Granted ability references and printed labels retain their identities.
The `granted_refs` stage-line snapshot and native regeneration-ban handling are
unchanged.

## Typed entry handling

Both entry consumers share `entry_ability`. An `Enchant` ability installs its
typed enchant restriction; an ordinary granted ability uses its existing delayed
definition and index. Aura recognition matches the actual vocabulary family and
word, so text such as `NotAura` does not count as an Aura.

`change_lists::entering_changes` projects the final entry ability set: direct
grants before the last direct ability loss are discarded, and only one loss flag
is emitted. Nested change lists are barriers. This corrects a generic non-corpus
edge in the old entry aggregation, which could otherwise retain erased grants
or emit duplicate `loses_other_abilities` fields.

Discarded nonkeyword definitions are still visited in their printed order.
Their references are omitted from `EntersAs`, while their quotation slots and
later delayed identities remain aligned. A regression with two quoted activated
grants separated by ability loss verifies that the retained grant gets the second
quote and second definition index. Discarded `Enchant` restrictions are not
installed. Entry extraction does not reinterpret copied text or ordinary grants
as entry restrictions.

There is one additional validation correction outside the corpus equivalence
claim: the old wrapper validated abilities directly, bypassing ordinary
`GrantAbility`'s rejection of intrinsic characteristic definitions. An ordinary
replacement containing such a definition now rejects. A definition inside
`Copiable` remains valid. Positive and negative controls preserve this distinction.

## Serialization and consumers

Schema **174** replaces schema 173. Removing the enum variant changes subsequent
`Change` binary tags; `GrantAbilityIfAbsent` now has tag **42**. Old normalized
exports and binary caches must be regenerated from source. RON/JSON/bincode
round trips, typed traversal, validators and inventories use the canonical list.
There is no compatibility alias or replacement compound node.

The eleven baseline whole-record captures in
`tests/fixtures/ability_expansion_before.jsonl` come from the old producer.
Their source fixture already existed and remains unchanged. The comparison
expands only wrapper elements inside their original lists and changes schema
stamps; no other field is reconstructed or relaxed.

## Verification

The final normalizer test run passes **951 tests across 89 targets**, with two
existing ignored tests. Seven new integration regressions and four new library
controls cover ordered replacement, all reducer families, nested grouping,
ordinary/copied characteristic definitions, typed enchant restrictions, both
entry paths, repeated losses, discarded grants, and quoted ability identity.

All **33,529** normalized native records match byte-for-byte after only the
schema and declared list expansion. There are **12** expanded wrappers across
**11** records, producing **6** keyword grants and **8** other grants. Every
operand, result ID, instruction order and enclosing effect boundary otherwise
matches. The full backend check validates the records and their RON, JSON and
bincode round trips.

All record diagnostics and capability decisions are identical;
**28,511** records remain backend-capable. The complete backend report matches
apart from the schema, fingerprints of the changed implementation sources, and
these declared inventory changes:

| Inventory family | Change |
| --- | --- |
| `Change::OnlyAbilities` | 10 to 0 |
| `Change::RemoveAllAbilities` | +10 |
| `Change::GrantKeyword` | +6 |
| `Change::GrantAbility` | +5 |
| `Ability::Keyword` | -6 |

Inventory traversal does not enumerate all entry changes, so these counters are
smaller than the full native-node counts above. All other inventory fields and
operation counts match exactly.

Of the eleven affected whole-record outputs, **five** supported module bodies
are byte-identical and **two** differ only in **three verified local identifier
renames**. The remaining **four** full diagnostic lists match exactly. The
renames are resolving effect-kind locals in Dragonshift and Kitesail Larcenist:
flattening the old wrapper changes their internal suffix from `_1` to `_0`.
The proof checks unique bindings, rejects identifier capture, skips strings and
comments, and requires every other token and all whitespace to match. The
ability references, labels, engine calls and their order are unchanged.

Another **266** entry/spread control records yield **217** identical complete
modules and **49** identical full diagnostic lists. This cohort includes all
other records with serialized `Changes` entry states and explicit Necromancy
coverage. Only the two provenance comments (schema/normalized hash and generator
hash) are omitted from generated-body comparisons.

Local proof artifacts are under
`/mnt/vizier/mtgish-ast-consolidation/target/ability-expansion-proof/`:

- `compare_native.py`, full before/after native exports and backend reports,
  `native-comparison.json`, `native-comparison.log`;
- `compare_modules.py`, affected/control source cohorts and names, complete
  module outputs, `generated-comparison.json`;
- `verified-tests.log`, `final-build.log`, `export.log`, `backend.log`, and
  `validation.json`.

The machine receipt is committed alongside this report. All owned build and
proof handles are terminal. No source corpus, immutable reference/gold/scorer
data, engine, vocabulary, live pipeline, root checkout or user input was changed.

## Remaining work

Ability replacement is fully canonicalized. Movement and embedded selection
remain the final broad candidate: selection timing, independent result identity,
simultaneous/group/random choices, typed entry specifications, face-down arrival
and strict cost/target cardinality require a deliberate semantic design. The
existing movement assessment remains applicable; this pass adds no partial
movement rewrite.
