# Canonical value choices

Implemented on `refactor/semantic-value-choices`, starting at `c1d164e`, implemented in `7613a4622303020b7391d12357ddd8788a6870fb`. This is
the value-choice candidate in the user's untracked `consolidation.md`; that input
is preserved and is not part of these commits.

## Contract

There is one semantic operation, `ChooseValue { actor, kind, terms }`.
`ChoiceTerms` retains its options, exclusions, secret/random/ordered flags and
not-previously-chosen constraint. Its cardinality is explicit:

| Previous representation | Canonical cardinality |
| --- | --- |
| Ordinary positive `count: n` | `ChoiceCount::Exactly(Constant(n))` |
| Ordinary sentinel `count: 0` | `ChoiceCount::AnyNumber` |
| Rich `count: None` | `ChoiceCount::Exactly(Constant(1))` |
| Rich `Some(n)`, without `up_to` | `ChoiceCount::Exactly(n)` |
| Rich `Some(n)`, with `up_to` | `ChoiceCount::UpTo(n)` |

There is no separate default-one alias. Exactly zero remains exactly zero and
cannot reach the scalar generator's private zero-as-any convention. Values have
a separate cardinality type from object selection, targeting and cost payment;
`Count::Exactly` and `Count::AsManyAsCan` remain distinct.

All normalizer producers, validation, operation inventory, generated typed
traversal, entering-choice discovery, restricted card-type menus, ability subset
menus, mana-color recognition and result bookkeeping consume the unified node.
The ordinary-choice accessor checks flags and exclusions before selecting the
scalar backend. Restricted multi-choices retain the rich backend path rather
than dropping their options. Secret, random, ordered and other unsupported terms
remain explicit gaps. Existing native menu errors remain precise; their wording
and stable requirement IDs are retained, including the legacy
`operation ChooseValueWith` gap label.

Activation-cost choice discovery now traverses typed operations and stops at
independent ability declarations. Printed strings containing AST-like text do
not impersonate choices. Programs executed by the cost are traversed.

Two small contract corrections accompany the alias removal:

- Planned helper hints spell a default choice of one as `Some(1)` and unbounded
  cardinality as `None`, instead of making the hints ambiguous. Zero and up-to
  bounds remain explicit. These are still held code; no new engine support is
  claimed.
- A scalar numeric choice records its actual generated local under its own
  result ID. Consecutive choices remain separate, and restoring a lexical scope
  restores its earlier result bindings. This makes the formerly ordinary and
  explicit-one rich representations obey one result contract. Secret object
  choices retain their selection result IDs.

Schema **173** replaces schema 172. Deleting `ChooseValueWith` shifts subsequent
`Operation` binary tags; `RevealChoices` now has tag 92. Cardinality tags are
`Exactly=0`, `UpTo=1`, `AnyNumber=2`. Regenerate older normalized/binary caches
from source; this change does not add an old-payload decoder. Existing captured
reference fixtures remain unchanged. Their comparators perform only the
declared choice representation migration in addition to earlier migrations.

## Evidence

The final Rust run passes **940 tests across 88 targets**, with two existing
ignored tests. It includes seven new integration regressions and four library
controls. The broader suites retain cost/target-count, native-generation,
reference-cost, delayed-label and result-provenance coverage.

A quote-aware balanced RON conversion compares the complete before/after
normalized exports. All **33,529** records match byte-for-byte after only the
schema and declared choice migration: **648** operations in **595** records
(previously 598 ordinary and 50 rich nodes). Every other operand, result ID,
ability boundary and instruction order is unchanged. The full backend check
also validates each record and exercises RON, JSON and bincode round trips.

The complete backend report matches after only its schema stamp, the two
operation-inventory key folds and the fingerprints of the twelve changed
implementation sources. All record identities, source diagnostics, backend
first diagnostics and capability decisions are identical. **28,511** records
remain backend-capable.

Generated comparison uses every affected whole record, including multiface and
nonstandard layouts: **433** complete modules have identical bodies and **162**
records have identical complete diagnostic lists. An additional **196** evenly
spread unaffected records produce **168** identical modules and **28** identical
diagnostic lists. Only the schema/normalized-hash and generator-hash provenance
comments are omitted from body comparison; no code renaming or whitespace
normalization is allowed.

Twenty-nine complete baseline native Card captures exercise ordinary, any-number,
entering pairs, ability subsets, secret choices, exclusions, ordered choices,
up-to counts, activation-cost reads and created-trigger reads. The fixtures are
captured from the baseline producer rather than reconstructed expectations.
Tests additionally cover exact zero, random terms, bound-count lexical scopes,
independent nested ability declarations, fake Debug strings, separate numeric
result IDs and secret-object selection identity.

The reproducible local proof is at
`/mnt/vizier/mtgish-ast-consolidation/target/value-choice-proof/`:

- `compare_native.py`, `before.unified.ronl`, `after.unified.ronl`,
  `native-comparison.json` and both full backend reports;
- `compare_modules.py`, affected/control source cohorts, name lists and complete
  before/after module outputs;
- `verified-tests.log`, `final-build.log`, `export.log`, `backend.log` and
  `validation.json`.

No source corpus, immutable reference/gold/scorer data, live pipeline, engine,
Cantrip vocabulary, or root worktree files were changed. Build processes used
this worktree's own target directory and are terminal.

## Remaining consolidation

This finishes the full value-choice candidate. The broad movement/embedded
selection candidate remains a semantic redesign, rather than an alias removal:
its implementation must preserve selection timing, simultaneous/group and random
choices, entry ordering, face-down characteristics and distinct cost behavior.
The concrete staged design in the earlier consolidation assessment remains the
next recommendation. No movement subset or unrelated canonicalizer is included
here.
