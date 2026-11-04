# Canonical shared-battlefield queries

`ObjectSet::Battlefield(filter)` is removed from the normalized semantic IR.
Its single representation is now:

```rust
ObjectSet::Zone {
    scope: ZoneScope::Shared(EventZone::Battlefield),
    kind: ObjectKind::Permanent,
    filter,
}
```

`Permanent` includes tokens. Card-only battlefield queries, private/player-scoped
queries, other zones and result/reference wrappers remain distinct.
`ObjectSet::battlefield` constructs this form; `battlefield_filter` and the shared
`battlefield_query!` pattern centralize its exact scope/kind match. The pattern
also works in deeply nested cost/recipient matches without duplicating a long
Zone pattern throughout the backend.

The preparatory commit recognizes both spellings before the alias is removed.
Specialized consumers cover static subjects, counter/tap/sacrifice/movement
costs, keyword and landwalk recipes, player permissions, object filters,
conditional counts and historical attachment queries. Generic Zone handling
must not classify permanent battlefield effects as `.everywhere()` card statics
or open sets of playable cards. Those boundaries are preserved explicitly.

The shared reader uses the general zone reader's candidate-name hygiene. When
an outer candidate is named `id`, the inner battlefield filter uses
`card_in_zone`; ownership reads can still access the outer candidate. Ordinary
battlefield enumeration still includes tokens. Card enumeration retains its
explicit token/card-copy exclusions. Saved-object `InZone` guards still reject
stale identities, and historical attached-Aura counts still read the departure
history rather than recounting the current battlefield. The typed IR visitor
adapts automatically to the remaining Zone node.

## Validation

**923 distinct tests across 85 Cargo targets pass.** The full normalizer run
covered library and integration tests, including backend generation/compilation,
reference corrections, payment histories, costs, targets, layers, scopes, choices,
multiface cards and the existing Wurm blocker clamps. The only initial failure
was the old ObjectSet bincode-tag assertion. Deleting the alias deliberately
moves Created from tag 13 to 12 and CostObjects from 14 to 13; the codec-contract
test now documents schema 172, and its final 16-test rerun passes. That rerun
supersedes the earlier 15-pass/1-fail component in the total.

The new focused tests cover exact scope/kind matching, token versus card queries,
invalid player-scoped battlefields, independent result views, stale-object
guards, historical attached Auras, outer-candidate ownership, permanent versus
everywhere card statics and complete before/after trees for 30 source records.

The intermediate dual-form probe validated and compared complete modules or
exact gaps for **all 8,321 affected records**, with zero representation
mismatches. The final full export contains **33,529** normalized records, all
validated and round-tripped through JSON, RON and bincode. Its entire serialized
native tree is byte-identical after only schema stamps and the **10,546**
declared query substitutions. Identities, layouts, completeness, diagnostics,
operation inventory and backend capability are unchanged. The isolated base
ed476d2 retains **28,497** backend-capable records; this is a generator capability
metric, not a claim of new runtime-supported cards.

All **8,315 named records** in the affected cohort were compared against the
before generator. After removing provenance comments, 8,176 module/gap bodies
are byte-identical. The other 139 differ only in bound battlefield-filter
parameter names. The comparison changes only the corresponding filter
parameter/references, rejects free old identifiers or rewritten literals and
leaves existing general-zone variable names intact. Six unnamed data records
are covered by the full serialization and dual-form probes.

Captured reference fixture bytes remain unchanged. The result-membership and
reflexive-X comparator code permits only this explicit additional query alias
migration while retaining every other operand. No English grammar, source
corpus, immutable Oracle reference/scorer, engine tree, queue/cache or live worker
configuration was changed.

## Migration and evidence

Schema advances from 171 to **172** because enum deletion changes binary tags.
Rebuild semantic JSON/RON/bincode artifacts from source with matching generator
revisions; do not feed old AST payloads to the new reader. Existing generated
Cantrip Rust remains valid. No Cantrip API/vocabulary change is part of this
branch. The root's later regeneration feature is a separate change to retain
when integrating these commits.

Evidence is under this worktree's `target/battlefield-query-proof/`:

- `validation.json`: aggregate receipt, source/proof hashes and inventory deltas.
- `final-tests.log`, `final-paid-tests.log`: complete run and final codec rerun.
- `before.unified.ronl`, `after.unified.ronl`: complete native exports.
- `before-backend-report.json`, `after-backend-report.json`: complete codec and
  backend-capability reports.
- `representation-probe.json`: all 8,321 dual-form comparisons.
- `final-alpha-comparison.json`, before/after named modules and
  `alpha_compare.py`: generated-output comparison.
- `compare_native.py`: quote-aware, balanced-parenthesis substitution proof;
  schema and query substitutions are the only permitted changes.

Movement and value-choice consolidation, the remaining plural ability wrapper
and the separate NORM-002 canonical-contract history are outside this pass.
