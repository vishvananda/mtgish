# Lexical resolution selections

This checkpoint implements the embedded-selection portion of the movement
candidate in the user's untracked `consolidation.md`. It is based on validated
`eb831bf`, with mutable typed traversal in `fa907bb` and the implementation in
`559c924`. It does not complete the remaining `Move`/`ChangeZone` consolidation.
The input report and earlier proofs remain untouched.

## Canonical execution contract

`Operation::WithSelections { steps, action }` prepares local selections exactly
where its action executes. Each ordered `SelectionStep` has a distinct
`SelectionId`, a player or chance chooser, a pool, a typed count and constraints.
`ObjectSet::Prepared` reads that local value. The enclosing instruction keeps its
original `ResultId`, supplied by the action, rather than by a pre-move choice.
Selection IDs cannot escape, duplicate a binding in the same scope or refer
forward. Nested scopes may shadow an outer ID; metadata is frozen before the
shadow is inserted.

The typed normalizer runs at public card, unified-card and pipeline validation
boundaries. It replaces the old generator's Debug precheck, recursive JSON
search and fabricated result IDs. The extractor stops at nested operations,
instructions, ability declarations, costs, announcements, target specifications,
events and object-filter predicates. These are visited separately with their
own execution sites. Choices inside a repeated action therefore repeat, and an
action repeated zero times does not ask a question. Granted abilities and future
programs retain their own scopes.

Direct selections of a true `Together` group are prepared before any member
mutates the state. A typed lexical rename prevents the members' local IDs from
aliasing. Three existing reveal-then-discard forms were misleadingly represented
as `Together`; they now use explicit `InOrder` clauses. This makes Rag Man reveal
the hand before sampling the creature to discard. It does not turn other
simultaneous groups into sequences.

Player gathering executes each player's selections inside that player's loop,
then performs the existing single simultaneous movement. Frozen pool metadata
continues to determine ownership and entry safety. If a player's local selection
is used to compute entry or destination terms, the backend retains an explicit
gap rather than flattening those varying terms into one common batch.

Named results and transitioned objects continue to describe the action's
outcome. Implicit earlier-selected reads use a separate pre-action view, including
separate per-player rows. This preserves chosen objects even if a replacement
prevents their movement or gives them new identities. Hoisting, optional
branches, walked-player aggregation, carried exports, delayed closures and
independent-ability resets all retain those separate views. Single-object
cardinality is tracked independently of the action's result cardinality.

Cost and target choices are not moved to resolution. Exact versus unbounded
counts retain their existing typed policy; random resolution selections use
`AsManyAsCan` while preserving their numeric expression and live pool. Rich
selection constraints remain explicit, and unsupported local constraints remain
backend gaps. No native runtime capability is invented.

## Serialization and consumers

Schema **175** replaces 174. The new operation and object-set variants are
appended, preserving existing enum tags; new typed selection declarations are
serialized normally. Existing normalized exports and caches require regeneration
because the canonical records changed. RON, JSON and bincode round trips pass.
Validation, inventory, typed traversal, library-result analysis and the relevant
backend consumers understand the new control scope.

Legacy `Move`, `ChangeZone`, and draft `ObjectSet::Chosen`/`Random` constructors
remain available to source builders. All actual embedded resolution selectors
in the corpus normalize to explicit preparation scopes. The separate next phase
must replace the movement aliases and their raw entry/group terms with one
typed movement representation, preserving random selection versus random order,
placement chooser, face-down characteristics, grouped constraints and entry
timing. This checkpoint intentionally does not claim that work is complete.

## Verification

The final normalizer run passes **976 tests across 91 targets**, with two existing
ignored tests. There are **21** focused selection integration regressions and
three mutable-traversal controls. They cover dependency order, lexical barriers,
idempotence, invalid bindings, pool-zone metadata, strict cost/target separation,
repetitions, independent grants, simultaneous members, ordered reveal/discard,
pre-action versus transitioned results, player batching, entry safety, optional
exports, deferred captures, aggregate and per-player reads, and ability resets.

The full corpus has **33,529 records**. An independent migration of the captured
old native JSON produces exactly the final output after only schema 175,
**41 selections** across **39 cards**, and the **three** ordered-clause changes.
Every other operand, binding, result, layout and instruction matches. The final
JSON is also byte-identical to the earlier checkpoint's normalized JSON. The
committed 39 whole source/native record captures are old-producer output, not
hand-reconstructed expected cards.

All **33,529 backend record objects**, including diagnostics and capability
decisions, match the baseline exactly. **28,511** remain backend-capable. The
only report differences are schema, implementation fingerprints, and declared
inventory changes: 24 `Chosen` plus 17 `Random` become 41 `Prepared` reads and
41 `WithSelections` scopes; random selections add 17 `AsManyAsCan` counts; three
`Together` nodes become `InOrder`. Top-level per-record operation counters change
where a preparation scope now encloses the original action. No underlying
operation is removed.

Complete generated-output comparison covers **82 cohort records** (51 complete
modules and 31 complete diagnostic lists) and **266 controls** (217 complete
modules and 49 complete diagnostic lists). Excluding the two provenance comments:

| Cohort | Exact | Verified binder/whitespace changes | Structural changes |
| --- | ---: | ---: | ---: |
| Affected and scratch-counter controls | 32 | 47 | 3 |
| Entry and spread controls | 264 | 2 | 0 |

The comparison checks consistent injective declared-binder renames, rejects
capture, and preserves every other token and all literals/comments. Structural
changes are Demonic Hordes (a guaranteed chooser local replaces a fabricated
player binding), Descent into Madness (explicit preparation inside gathering,
retaining aggregate/per-player views before one exile batch), and Rag Man
(reveal before random sampling). All full diagnostic lists match. The final
consumer audit changes only Descent's preparation bookkeeping relative to the
previous post-change module capture.

## Native proof and explicit prerequisites

The independent native crate pins clean checkpoint
`def1558de5a5ebd33e4e85b51d3d6abfb22406ea` in
`/mnt/vizier/card-lane/engine-mana-counter-costs`. The donor is unchanged. Final
proof compiles **265 exact generated card modules** (49 affected, 216 controls)
plus ten old/new probe modules, **275 modules total**. Six runtime tests pass:

1. Repeated choices ask against the updated live pool: old one choice/tap,
   corrected two choices/taps.
2. Zero repetitions ask no choice: old one question, corrected none.
3. Repeated random destruction samples twice: old one destruction/pool size 4,
   corrected two destructions/pool sizes 4 then 3.
4. Actual Rag Man reveals before the recorded random choice, discarding one
   creature in both versions. The real BBB activation is paid with three actual
   Mox Jets and uses an explicit player target.
5. Paired aggregate batch control: both players choose while both permanents
   are still on the battlefield, then both are exiled and the caster draws two.
6. Per-player control: each player later draws one from their own pre-action
   selection row after the batch.

Three exact complete modules have pre-existing native prerequisites and are
explicitly excluded from that successful compile: Descent into Madness needs
`CounterKind::Despair`; Haphazard Bombardment needs `CounterKind::Aim`; the Ian
Malcolm control needs an `ExileCast.instead` initializer. Paired old/new contract
checks fail with exactly the same counts (Despair 2, Aim 3, missing initializer
1). No named counter is aliased, no emitted semantics are edited, and no native
placeholder is installed. Root fixed the initializer separately in `8141689`;
integration inherits that fix without rewriting this established proof baseline.
The generic native batch control proves the affected batching path while the
actual Descent module's prerequisite remains explicit.

## Local receipts

Artifacts are under
`/mnt/vizier/mtgish-ast-consolidation/target/movement-proof/`:

- `selection-final-consumers-full-tests.log` and
  `selection-final-consumers-proof-build.log`.
- `selection.final-consumers.before.jsonl`,
  `selection.final-consumers.after.jsonl`,
  `selection-final-consumers-native-comparison.json`.
- `before-backend-report.json` and
  `selection-final-consumers-backend-report.json`.
- `selection.{modules,controls}.before.rs`,
  `selection.{modules,controls}.final-consumers.rs`,
  `selection-final-generated-comparison.json` and the three complete module diffs.
- `native-final/`, `native-final-runtime.log`, paired `native-contract-gaps/`,
  `native-contract-gap-evidence.json`, `native-gaps-before.log` and
  `native-gaps-after.log`.
- Independent comparison helpers and captured temporary probe sources remain
  with those receipts. Temporary Cargo examples were removed after all handles
  became terminal; generated modules are not committed.

The companion proof JSON records exact hashes. No live worker, Cantrip tree,
vocabulary, Oracle/reference artifact, source corpus or user input was changed.
