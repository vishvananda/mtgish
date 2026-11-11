# AST review after the human editor cleanup

Implementation: the proposed migration is now completed in [schema189](ast-human-189-20261111.md). The original review below records the pre-migration analysis.

The editor exposes two meaningful representation problems: match alternatives
inhabit the scalar expression type, and countering has duplicate shapes with
different destination types. Most other simplifications belong in authoring
metadata and UI projections. This review proposes a follow-up migration; it does
not change schema188 or require new Cantrip functionality in this reference batch.

## Recommended order

1. Centralize authoring roles and defaults without changing the wire schema.
   The editor currently recognizes binary operators, compact operands, optional
   defaults and labels in JavaScript. Extend shared authoring metadata alongside
   `human/defaults.rs` so the editor and text authoring agree. Keep an advanced
   view for nondefault operands. A compact single-line Add already maps naturally
   to `Expr::Add(left, right)`; there is no need for an extra scalar wrapper or a
   universal operation node merely to draw it that way.
2. Separate numeric match tests from scalar expressions. `Expr::AnyOf(Vec<Expr>)`
   describes alternatives rather than one number, yet an Expr socket/validator
   permits it in arithmetic and action amounts. `Aggregation::AnyOf` has the same
   issue, so changing only the literal list would leave the problem. A reusable
   `NumberTest`/`NumericPredicate` should express comparisons and their boolean
   combinations and be used by numeric property tests, conditions and nth-event
   tests. Scalar `Expr` should yield one number. Preserve the current comparison
   interpretation explicitly: expanding alternatives under `!=` by OR is not the
   same as a “not in this set” test. Define side restrictions and empty-list
   behavior before migrating; do not silently reinterpret old cards.
3. Consolidate `Operation::Counter` and `CounterWith`. The first takes an ObjectSet
   and legacy `Destination`; the second an ObjectRef and typed MovementDestination
   with an optional destination filter. Prefer a set-valued counter action (One
   handles a single object) with typed destination terms. In `CounterWith`, `when`
   chooses the replacement destination; failing it still counters to the owner's
   graveyard. It does not condition whether countering occurs. Preserve this
   fallback, successful-counter requirement, arrival terms and stack spell/ability
   distinctions. The only other Operation use of legacy Destination is the hidden,
   invalid-in-validated-cards `DraftChangeZone` producer form; handle that boundary
   explicitly rather than letting it force a second public counter representation.
4. Improve names in one planned migration if the semantic changes justify a schema
   bump. `Instruction::Times` should read as a repeatable process with a total-run
   cap, not fixed-count iteration. `RepeatableWithLimit` is clearer. It currently
   exports no receipts, unlike Repeatable: adding an optional limit to Repeatable
   would require preserving that difference. Standardize All/Either/AnyOf labels
   across predicates, filters and events. `ObjectFilter::Any` is an unrestricted
   filter, not an OR list, so a mechanical Either-to-Any rename would be confusing.

## Corpus evidence

A typed schema walk of 33,529 schema188 cards counted occurrences, not text matches:

| Family | Occurrences / context |
| --- | --- |
| Literal Expr AnyOf | 8: 5 Condition Compare right operands, 2 property values, 1 nth-event value |
| Aggregation AnyOf | 3, inside Expr Aggregate |
| Counter / CounterWith | 540 / 2; one CounterWith has a destination filter, one does not |
| Repeatable / Times / fixed Operation Repeat | 39 / 11 / 5 |
| Instruction Group / Operation Together / InOrder | 3,689 / 19 / 3 |
| Add / Subtract / Multiply / Divide | 276 / 112 / 735 / 126 |

The walk and complete counts are in `/mnt/vizier/reference-seven-20261111/ast-ui-usage.json`.
Counts are a migration-size estimate, not proof that all affected cards have the
same semantics. Numeric predicate work must inspect the aggregate alternatives too.

## Distinctions to retain

Keep ObjectRef versus ObjectSet and PlayerRef versus PlayerSet; the editor can
project One wrappers away while retaining cardinality in valid IR. Keep arithmetic
variants unless a borrowed operator/operand view reduces duplicated visitors;
infix layout alone does not justify changing every number expression.

Keep Program instructions with separate receipts distinct from Together/InOrder
operations under one result. Keep loop receipt scope and simultaneous player-choice
semantics explicit. Keep announced modes/targets separate from choices made during
resolution. Keep Optional and PayCost separate, and preserve performed-object
receipts rather than replacing them with source-linked global queries. The delayed
scope investigation illustrates why registration location must remain observable.

Keep event-captured participants distinct from live object relationships, as the
Curse fix demonstrates. Keep characteristic-defining and ordinary statics distinct,
including all-zone applicability, as the six CDA corrections demonstrate. Keep
created triggers with their own announcements distinct from delayed triggers that
capture the creating resolution's bindings.

Named variables already belong in the authoring document, where names can change
without changing canonical slot identities. Display names should not replace
numeric binding IDs in execution IR. Bounded engine number representations also
remain intact; UI convenience gives no reason to widen them.

Any future schema migration should batch the semantic changes, update parser,
lowering, validators, generator interface and human metadata together, and compare
all exported cards plus binding paths. No browser workspace migration is needed
for this unused prototype. Backend implementation is needed only to validate a
representation or correct existing lowering, not merely to match a UI label.
