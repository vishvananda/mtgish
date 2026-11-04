# Canonical result membership and typed continuation reads

This follows the result-membership candidate in `consolidation.md`. The portable
IR now uses `ObjectFilter::InSet(ObjectSet::Named(result))` for membership in a
result's objects. `InResult` is removed from the enum, producers, validator,
inventory and backend. Schema advances from 170 to 171 because deleting the enum
variant changes binary tags.

## Preserve bookkeeping before removing the alias

The first commit adds typed traversal generated deterministically from the local
IR declarations. `visit_spec.rs` emits traversal into Cargo's build directory;
generated Rust is not committed. Hooks can stop beneath a node, so result queries
can respect independent ability namespaces without maintaining a partial manual
walk or interpreting Debug/serialized strings.

`result_reads::ResultReads` distinguishes four uses:

- Object lists and individual named objects, including joined, affected and
  transition views.
- Membership in the objects named by a result.
- Counts explicitly measuring objects put in a graveyard.
- Counts of the cards a surveil kept on top.

Membership and graveyard counts make surveil retain its graveyard result. Plain
named-object reads do not imply that graveyard view. Kept-on-top bookkeeping is
independent, so both views can be needed by one continuation. The former Debug
checks for these uses are removed.

An inline `PayCost` can read the current resolution's results. Created delayed,
reflexive, triggered and replacement Programs are lexical closures and can also
capture them. An embedded Ability declaration, including its cost/body, has an
independent result namespace and is skipped. A regression validates an outer
program and a granted ability whose procedural cost and body each define result
3 locally: those local membership/kept reads must not retain the outer result 3.
Text resembling a Debug reference is also ignored.

`ObjectFilter::named_result` is the shared typed accessor for this membership.
Pile complements use the actual unchosen pile rather than the complement of the
chosen pile across every object. The per-object prevention path still makes one
shield for each named permanent. Selected, Observed and Created remain distinct
views; availability validation still rejects an unknown result and does not
reinterpret a generic Named result as Selected.

## Validation

**471 distinct tests pass across 11 integration suites and the library unit
suite.** This includes 12 result-read/migration tests, the direct backend pile
complement regression, reference corrections, pipeline/codec tests, scopes,
choices, events, multiface generation and backend reads. The final 12-test
result-read run supersedes its earlier 11-test component in the broad run; the
count does not double-count that rerun.

Full before/after checks cover **33,529 source records**, all normalized,
validated and round-tripped through JSON, RON and bincode. The entire native RON
export differs only by schema stamps and the **25 alias nodes in 19 records**.
Every other serialized field is byte-identical after those exact substitutions.
The operation inventory and per-record identity, layout, completeness,
diagnostics and backend capability are unchanged. All **28,497** previously
backend-capable records retain that capability; this is a generator capability
check, not a claim of new Cantrip runtime/card completion.

All 19 affected records and 234 continuation-control records produce identical
executable Rust and gap diagnostics after removing only provenance comments.
Controls cover every source record containing a Surveil, SurveilKeptOnTop or
JoinResults node. Dedicated regressions preserve Enlightened Confidant's
`surveil_into_graveyard` path and Mutational Advantage's per-object shields.

The frozen producer JSON fixtures are test evidence from the before export.
Comparisons permit only the alias and schema migration. No Oracle reference,
scorer, source corpus artifact or live card packet was edited.

Evidence is retained under this worktree's `target/result-membership-proof/`:

- `validation.json`: aggregate receipt, inventory delta and proof hashes.
- `final-suites.log`, `final-result-tests.log`, `final-unit-tests.log`.
- `before.unified.ronl`, `after.unified.ronl`: full native exports.
- `before-backend-report.json`, `after-backend-report.json`: full codec and
  backend checks.
- `generated-comparison.json` and the before/after affected/control modules.
- `typed-backend-report.json`: the intermediate typed-traversal commit preserves
  the exact full baseline report before alias removal.

Rebuild semantic artifacts with schema 171 and keep generator revisions matched
to their frozen inputs. Old binary/JSON/RON AST payloads containing InResult need
regeneration from source. Existing emitted Cantrip Rust remains valid. Neither
commit changes Cantrip, live worker configuration, queue state or cache rows.

The separate NORM-002 canonical-contract branch and the other consolidation
candidates are outside this pass.
