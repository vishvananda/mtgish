# Ordered library and graveyard queries — 2026-11-13

Fresh Rust IR now uses one positional set form for a single player's library or
graveyard: `Take { objects: Zone { … }, count, from: Top | Bottom }`. A scalar
read of its top card uses `Query { objects: Zone { … }, index: 0 }`. This removes
alternate spellings of the same selector without changing the portable schema
(190), runtime AST layout, integer widths, saved IR, or comparison rules.

## Scope and export exclusions

The local wording catalog expands beyond “top/bottom N cards” to qualified
cards, conditional tests, owner phrases, library/graveyard destinations, and
manifest/cloak/manifest-dread controls. It records each clause, action words,
source dataset, cached IR record indices, operations, and selector paths.

| Catalog measure | Count |
| --- | ---: |
| Face occurrences discovered | 2,494 |
| Distinct face names | 2,491 |
| Included face occurrences with cached IR | 2,394 |
| Distinct included face names exercised by the regression | 2,391 |
| Explicitly excluded occurrences | 100 |
| Unexpected missing included occurrences | **0** |
| Occurrences with a conditional follow-up wording match | 229 |

The machine-readable collection is
[ordered-zone-catalog-20261113.jsonl](ordered-zone-catalog-20261113.jsonl).
The regex is discovery assistance, not a proof that all possible English
paraphrases were found. Manifest-related typed IR controls broaden the search
beyond literal positional wording. Destination matches are deliberately kept
and must not be mistaken for source selectors.

**The 97 diagnostic English parse failures were not normal export failures.**
The diagnostic probe bypassed the active ignore list. Every one of those names
is explicitly excluded by `grammars/ignore.json5`; another three excluded names
parse and lower successfully. Those three are Chimney Goyf, Oteclán, and The
Powerful Dragon. They remain excluded. No input or Source cache was added,
removed, or restored during this work.

The ordinary export filters exclusions before parsing
(`go_mtg_parser/main.go`, preprocessing in `main`; `justfile` supplies
`grammars/ignore.json5`). The counts reconcile:

```
35,018 oracle + 24 tokens + 5 dungeons + 0 additional = 35,047 input records
35,047 input − 1,518 matching excluded records = 33,529 cached records
```

The ignore list comments group these cases into playtest/event products,
Un/acorn products, challenge decks, and special promotional products. These are
scope decisions, not evidence that each card is technically impossible to
parse. The README's grammar-checking section describes this policy, although
its `data/ignore.json5` path is stale; the active path is under `grammars/`.
The exact category, line, and diagnostic outcome for all 100 matching excluded
records are in
[ordered-zone-exclusions-20261113.json](ordered-zone-exclusions-20261113.json).

Dungeon matches are included and valid: Baldur's Gate Wilderness, Dungeon of
the Mad Mage, and Undercity. None of the 100 excluded matches is a dungeon.
The other two dungeon inputs also remain in the normal validated export.

## Normal forms and preserved differences

- A single player's `LibraryTops(One(player))` becomes `Take(Zone, 1, Top)`.
  A positional `One(Query(…))` used as a set becomes the same `Take` form.
- A scalar `Query(Take(Zone, 1, Top), 0)` becomes `Query(Zone, 0)`. Proven
  singleton ordered observations can use scalar type/property reads, sharing
  the existing cardinality proof rather than adding per-card exceptions.
- “Top creature card” filters **before** taking the top card. “Top card, if it
  is a creature” takes the unfiltered top card and tests it **afterward**.
  Corpse Dance and Guiding Spirit explicitly test that distinction.
- Bottom selection retains `Bottom`; Phyrexian Furnace exercises it. An outer
  filter never moves inside a positional selector.
- `LibraryTops(All)` remains one card **per library**. It is not equivalent to
  taking one card from a combined population. Multi-player loops retain their
  own selection scope. Existing IR expresses these differences, so no new
  zone-slice enum is needed for this normalization.
- `LookAt`, `Reveal`, movement to exile, `Manifest`, cloak, manifest dread,
  `Scry`, `Surveil`, and `RevealUntil` retain their separate operations and
  receipts. A live top-card visibility ability is not a one-time reveal.
- Wand of Denial now tests and moves its prior `Observed` look receipt when
  the player binding matches. The paid-life guard remains in place. This is
  an explicit identity/receipt correction; no mana-window exploit is claimed.
  If there is no matching definite look, the live positional selector remains.
- Requested mill-one and token-one still do not prove a singleton result:
  replacements can increase the actual population. Unrelated choices and
  receipts do not receive the ordered-zone scalar rewrite.

## Cantrip generator correction and efficiency

Native libraries store their top first, while native graveyards store their
newest/top card last. Previously, scalar graveyard queries reversed the order
but `Take(Top)` did not. The generator now gives all graveyard zone reads the
same top-first logical order and removes the scalar-only reversal. Bottom
slices keep the relative order of the selected suffix.

For unfiltered cards in one library/graveyard, bounded reads use an iterator
prefix instead of first allocating the entire zone. General zone iteration
also avoids the intermediate per-zone `to_vec`. Tokens and card copies remain
excluded from physical-card queries. Cast-test reads retain their source
exclusion. This is an allocation/iteration improvement, not a benchmark claim.

A regression compiles and runs the actual generated reader expressions
against a typed adapter with native zone ordering. It checks library/graveyard
top and bottom, scalar reads, the cast-test path, transient non-card entries,
and one-per-library versus one-total selection. It does not claim full native
engine integration testing or new backend support for every cataloged card.

## Verification and reproduction

All **33,529** cached Source records lower without diagnostics, validate, and
round-trip through the binary representation. **40** fresh IR records change;
**33,489** remain identical. Identities, layouts, characteristics, operation and
result-ID sequences, and movement destinations remain unchanged. All Manifest
(44), Scry (461), Surveil (233), and RevealUntil (166) payloads are unchanged.
The checked-in proof records the exact changed paths and values.

The dedicated tests cover the 40-record witness packet and every one of the
2,391 discovered included face names. Historical saved IR stays unchanged;
test-only before/after expectations document four reviewed snapshot changes.
The full Rust workspace passes **2,103 tests / 24 ignored / 204 targets**.
The catalog also regenerates byte for byte. Exact changes, hashes, and scope
counts are in [the companion proof](ordered-zone-audit-20261113.json).
Evidence logs and full before/after exports are retained locally at
`/mnt/vizier/ordered-zones-20261113`.

To regenerate the catalog from the current Source producer:

```sh
RUST_MIN_STACK=16777216 cargo run --manifest-path rust_syntax/Cargo.toml \
  -p mtgish-ast --features cantrip,serde_json/preserve_order \
  --example export_unified_jsonl -- data/mtgish.lines.ron /tmp/current-ir.jsonl
python3 rust_syntax/tools/audit_ordered_zones.py \
  --ir /tmp/current-ir.jsonl --output /tmp/ordered-zones.jsonl
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=8 cargo test \
  --manifest-path rust_syntax/Cargo.toml --workspace --features cantrip --no-fail-fast
```

This batch changes Rust producer/generator behavior, not English grammar or
Source. No new English parser defect is inferred from the excluded-card probe.
The canonicalization policy, receipt correction, generator fix, and export-scope
clarification are reported in the existing
[upstream consistency discussion](https://github.com/i5jb/mtgish/issues/275#issuecomment-6841131391).
Implementation **54abf45e9992e52a49f7569f1d038b393a0fa9b1** is pushed to `fork/main`.
Foundation work and card workers stay
paused; this batch does not pull upstream updates or publish the Blockly site.
