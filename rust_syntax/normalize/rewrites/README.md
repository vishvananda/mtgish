# Reviewed rewrite registry

`actions.json5` is a strict JSON subset of JSON5. Add mapping data here rather
than handwritten source match arms. `src/rewrite_spec.rs` validates the table;
the build script emits typed Rust into Cargo's `OUT_DIR`.

Registry version 2 supports shared quantities and cardinalities, player actions,
permanent effects, and continuous-effect changes alongside movement/control.
Each row declares a stable ID, source variant/arity, recipe family, operand
positions, and defaults. Every argument must be consumed exactly once, including
arguments nested in arithmetic and selection templates. Unknown fields, duplicate
IDs/variants, invalid positions, and incompatible fields fail the build. Rust
checks actual source arity and field types, including zero-field unit variants.

| Field | Meaning |
| --- | --- |
| `subjects.kind` / `argument` | Typed one/many permanent, graveyard, or exile operand and its zero-based field position |
| `amount` | `Constant`, `Source`, `Multiply`, or `Add`; source field positions become actual `GameNumber` values in generated code |
| `selection` | `AsManyAsCan`, `UpTo`, or `AnyNumber`; ordinary effect choices, never announcement targets or strict costs |
| `player_operation` | Draw, discard (with randomness), gain/lose life, mill, scry, or surveil; uses the inherited actor set |
| `object_operation` | Tap/untap, destroy (with regeneration policy), sacrifice, or regeneration shields |
| `changes` | Field containing layer effects or permanent rules; family fixes its typed interpretation |
| `duration` | Source field retained unresolved; absent means indefinite |
| `destination` | Movement destination; `BattlefieldYou` preserves the existing default controller, with explicit entry flags taking precedence |
| `entry` | Field containing entry flags; only valid for battlefield movement |

Control recipes use the inherited action actor. Layer effects capture snapshot
subjects; permanent rule effects use live subjects. Selections on ordinary object
effects emit the existing choose-then-effect program. Sacrifice retains selection
inside its operation, including each-player choices and simultaneous application.
Draw, discard, mill, sacrifice, destroy, and move remain distinct final operations.

Selectors, quantities, durations, and flags bind through the shared resolver.
Unsupported operands stay in intermediate data and produce diagnostics. Exile
returns retain exact post-exile incarnations restricted to still being in exile.
Structural migrations retain final semantics. Payment capabilities are versioned
with semantic schema 23; existing binary discriminants remain stable.

`ast pipeline-check` inventories every subtree, verifies source round trips, and
saves per-record form membership, recipe/opcode counts, final vocabulary, and
strict semantic coverage. `ast pipeline-candidates <report> <source-enum> <out>`
groups actual Rust signatures with occurrence counts, affected records, and
examples. Matching signatures suggest review batches; they do not establish
semantic equivalence or automatically create executable mappings.

`ast pipeline-fixtures` extracts representative cases from the saved report.
Selection covers each reviewed rule, operand forms at each argument position,
and enclosing actor/ability/layout forms. Handwritten tests cover semantic
policies; feature coverage alone cannot prove equivalence. Run the full corpus
at a broad batch boundary and compare prior complete final exports during
adapter migration.

The registry and intermediate artifacts are compiler inputs, not a second
executable card language or a fallback in the final semantic schema.

`numbers.json5` independently maps `GameNumber` variants into generic quantities
and typed queries. Formula trees compose constants, source subexpressions,
arithmetic, explicit rounding, object properties, player life, object counts,
aggregates, distinct-value counts, and counter queries. Private-zone queries carry
explicit players and optional card filters/exclusions; unions preserve input
order. Every nested source field is accounted for once. Both registries share a
build-time check against ID collisions, and generated Rust checks all field types.

Known subexpressions normalize recursively. Unknown leaves retain typed source
values for the binding pass; every source subtree is inventoried even beneath an
unknown parent. `Battlefield` queries restrict result/target identities to objects
still on the battlefield. `Objects` queries retain the existing current/last-known
aggregate behavior. These policies cannot be merged merely because field types
match. No query is evaluated while preparing a numeric recipe.

The staged artifact format is `mtgish.pipeline.intermediate.v8`, with a map of
action/numeric/selector/program/operand/mechanic registry fingerprints and per-node provenance. Rebinding verifies
the complete staged data before using prepared templates in fresh lexical scopes.
Pipeline report version 8 separates action, numeric, selector, program, operand, and mechanic rule counts, remaining
registry forms, and records with each kind of rewrite. Neither remaining-form
list is a list of semantic failures: native handlers can still support those
forms. The final semantic schema remains independent of these compiler formats.

`selectors.json5` maps predicates across six typed source domains: permanents,
cards, spells, graveyard cards, hand cards, and exiled cards. A shared predicate
tree handles logical composition, vocabulary, colors, abilities, names,
control/ownership, object identity, zone scope, property comparisons, and counters.
Generated Rust checks domain-specific field types; validation accounts for every
nested field and distinguishes identical variant names in different domains.
Vocabulary constants are reviewed identifiers checked against actual Rust types.

Predicate consolidation does not replace query construction or reference binding.
Targets retain their domains; graveyard target collections and exile-transition
aliases remain explicit set references. Candidate-dependent properties and
controller operands retain the existing candidate scope. Direct zone/name
predicates retain their incoming scope. Prepared predicates contain unresolved
operands and bind anew in each ability; they cannot cache resolved target slots.

Selector fixture features cover reviewed rules and immediate argument variants.
Each descendant selector is independently inventoried, so compound All/Or rules
need not repeat every descendant combination. Numeric/action features retain
deeper operand coverage, now including enclosing selector scopes. The full
corpus check remains the source-preservation and strict native-completion gate.

Registry fingerprints are centralized and cached for the compiled tables. Saved
artifacts, reports, fixture extraction, and candidate preparation check the same
map. Changing a registry requires a rebuilt tool and fresh structural data;
changing binding alone can reuse source membership with native validation.

`programs.json5` maps source `Action` wrappers and `Actions` announcement/program
forms into shared `InstructionRecipe` data. The mappings specify actor selection,
optional decisions, conditions/negation/branches, resolving payments, ordered
sequences, targets, and mode bounds. Source field accounting includes actor,
body, condition, costs, and bounds; repeated mode counts bind one operand once.
Generated Rust checks single instructions, action lists, and announcement bodies
against their actual source types. Validation rejects incompatible domain/body
forms and a batch optional choice without an explicit policy.

Known children expand recursively into existing operation recipes. Unknown
instructions remain typed source leaves with explicit diagnostics. Modal targets
remain announcement data, never choices introduced during resolution. Modes
inherit event bindings but start fresh target/result scopes. Conditional and
optional bodies reuse the shared branch frame invalidation. An actor wrapper
uses the existing result frame and does not rebind the meaning of “you.”

Each-player recipes require an existing operation with an explicit batch policy
(draw/discard/life/mill/tokens/scry/surveil/shuffle/sacrifice). They do not serialize
players or invent a per-player decider. Unsupported body operations still fail
at binding. Sacrifice keeps its choices inside the simultaneous operation.
Resolving payments reuse `PayCost` and its result, including optional-choice
semantics under event replacement; they are not casting or activation costs.

Program recipe forms, actors, bounds, and unresolved source domains are counted
in reports. Fixture features include each reviewed structural rule, immediate
argument forms, and its enclosing scopes, with descendants inventoried by their
own rule nodes. Final semantic data uses schema 23; program compiler
formats can evolve without changing exported card data.

`operands.json5` consolidates costs and conditions independently of their enclosing
ability or action. Cost recipes represent conjunction, alternative payment
choices, mana symbols, the source's own mana cost, guarded X/tap-symbol inputs,
life/draw/mill amounts, loyalty, sacrifice, discard, movement, and counter removal.
Condition recipes represent conjunction/disjunction, payment choices, numeric
comparisons, object predicates, active players, quantified player predicates,
and source/owned-zone checks. Shared numeric and predicate recipes expand inside
these operands. Unresolved children stay typed source leaves until binding.

Every field in nested formulas, selections, object queries, and player operands
is accounted for once. Generated Rust checks actual source field types. A rule
cannot use a cost recipe in the condition domain, or vice versa. All six
registries share ID-collision checks, fingerprints, and provenance verification.

Costs have strict `Exactly`/`AnyNumber` selections, distinct from effects that do
as much as possible. Conjunction retains source order; a choice retains every
alternative and cannot discard an unsupported option. `ItsManaCost` retains the
actual symbols and missing-cost semantics, including the receiving host when
used in a granted keyword. Mana X only accepts the existing announced-X source;
a tap symbol only accepts the source permanent. General tapping costs retain
explicit object selections and the distinction from symbol costs.

Resolving payments preserve the explicit payer during nested counter choices.
Payment conditions read the scoped payment choice, not whether replacement events
occurred. Branch-local payment results cannot escape into an outer condition.
Quantified conditions retain bound players and nesting depth; source zone checks
remain separate from current casting/entry history checks. Context-dependent
history conditions continue through the existing native handlers.

Operand registry version 2 adds object tap/untap payments and exact one-object
returns. `CostTapPermanent` preserves the source grammar's `{T}`/`{Q}` marker for
`ThisPermanent`; a named attached/target object is an ordinary object payment.
`CostTapObjects` reuses shared strict `CostSelection` and typed subjects. Group
constraints use the shared qualification binder described below.

Semantic schema 21 appends `CostPart::ChangeTapState { selections, tapped, symbol }`.
It represents payment of one component, rather than resolving a tap effect.
Selected objects must start in the opposite state and the entire strict count
must be payable. A symbol payment uses only the implicit source/keyword host,
without an object choice, and retains the creature activation restriction of
CR 107.5-6 / 302.6 / 602.5a. Ordinary tapping has no implicit control or summoning
sickness constraint beyond its query and the rules governing payment. Existing
`TapSymbol` remains the compact `{T}` form with its old binary tag.

Validation requires battlefield inputs, nonempty selection descriptions, strict
counts, and the implicit-source constraint for symbol payments. The AST describes
state eligibility and full-payment requirements; actual state-dependent payment
execution remains backend work. Activation mana classification treats tap-state
payment independently from a library-moving cost. Grouped movement costs and
sacrifice share the same strict selection validation and binding helper.

Returning a permanent reuses `Move` with an exact selection and the owner's hand
destination. Selected payment objects do not become targets or automatically bind
"the permanent tapped this way" in the resolving body. Such histories still need
explicit shared binding support. Intermediate format 6 records the changed operand
recipes and fingerprints; the report's fields remain format 6.


Operand registry version 3 adds six grouped tap, sacrifice, and discard mappings.
The optional `group` field consumes its own source argument exactly once. The
unbound recipe retains that typed `GroupFilter`; binding uses the same constraint
resolver as library searches and inspected-card selections. Unknown qualifications
remain diagnostics instead of losing the qualification. `DifferentProperty`
compares scalar values pairwise; `CommonProperty` requires a value common to the
selected objects. `LandTypes` includes nonbasic land subtypes and is distinct from
the five `BasicLandTypes` used for domain. Total power/mana value retains the
comparison and numeric expression.

Semantic schema 22 appends `CostPart::Constrained`, wrapping one selected-object
payment component. It qualifies the chosen inputs together before payment, not
the entire eligible pool or post-movement identities. The child retains strict
counts, eligibility, and payment semantics. Validation rejects empty constraints,
symbol/mana/alternative/nested components, multiple selection descriptions, and
resolution-only movement constraints. The generic wrapper reuses tap, sacrifice,
discard, and movement costs; it adds no new operation kind. Existing cost binary
tags remain unchanged.

`mechanics.json5` starts a reusable keyword-definition stage. Generated typed Rust
adapters preserve the numeric argument and activation limit; the binder expands
Crew and CrewOnceEachTurn into the shared constrained payment and continuous
effect forms. `ActivationOrigin::Crew` retains the identity needed by crew-specific
modifiers and events. Creatures crew as the cost is paid; the source becomes
crewed when the ability resolves. Costs select any number of other untapped
creatures controlled by the ability's controller with sufficient total power.
Resolution adds artifact and creature until end of turn without setting power or
toughness, announcing targets, or adding a sorcery restriction. The validator
checks that only this reviewed definition carries Crew identity. These rules use
CR 702.122 in the [September 25, 2026 rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.pdf).

Intermediate/report formats 7 add mechanic provenance, keys, recipes, rule counts,
per-record features, and the sixth registry fingerprint. Candidate preparation
marks reviewed mechanic rows. Saved artifacts verify the complete staged data
before binding. These compiler recipes are absent from final exported cards.
Paid-object histories, alternate crew payments, crew-as-though modifiers, and
crew-specific event/query bindings remain explicit future work. Cantrip lowering
is deferred.


Numeric registry version 2 adds eleven history aliases without new numeric
operations. `NumberObject::Payment` and `NumberQuery::Payment` feed the existing
property, count, and aggregate recipes. Payment object adapters consume no source
argument; ordinary object adapters still require exactly one. The compiler checks
both cases and rejects extraneous fields or missing operands. Numeric recipes
expand even beneath unsupported parents, retaining unresolved scope diagnostics.

Semantic schema 23 appends `ObjectSet::CostObjects` with a `PaymentRef`, structural
component path, object view, and before/after phase. Payment namespaces distinguish
the current activation, a specific top-level casting-cost declaration, and an
earlier resolving `PayCost` result. Component paths begin with a cost-part index;
choices add an alternative index followed by that alternative's component path.
`Constrained` wrappers are transparent. There is no global latest-paid-object
slot. Repeated, conditional, alternative, or linked casting declarations remain
outside the current direct additional-cost binder.

`Inputs` retains designated original identities for object properties and LKI;
`Performed` describes participants in the corresponding action after replacement
processing, for quantities such as “sacrificed this way.” An unpaid component or
unchosen alternative has an empty history. Payment choice is still the separate
`PaymentChosen` result; it cannot imply an action occurred. A later ordinary
action of the same kind shadows compiler shorthand; until ordinary event-history
binding is added, it diagnoses the missing binding rather than reusing an older
payment. The distinction follows CR 118.11-12 and 608.2h-k in the
[September 25, 2026 rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.pdf).

A singular object uses the existing indexed-query form only when the addressed
input component is statically at most one object; plural unordered histories cannot pick
an arbitrary first entry. Source shorthand with multiple matching components or
casting declarations remains ambiguous. Explicit final references can name an
individual component of an alternative; an unchosen alternative yields nothing.
Native validation checks declarations, component paths, object-history-producing
cost kinds, input phases, cardinality, and lexical visibility. Mode resolution
inherits announcement payments while keeping fresh target/result frames. Granted
abilities and other card abilities start independent payment frames. Delayed
closures retain their creator's payment references.

During announcement, `Inputs` names the proposal's planned casting/activation
inputs under CR 601.5, including objects chosen for costs paid later. The same
reference uses the recorded identities after payment, including when targets are
rechecked. It cannot turn into a fresh selector if an Aura changes hosts. This
supports Veteran's Voice and chosen-input target constraints with the same form.
`Performed` histories remain unavailable at announcement: they describe actual
actions after replacement processing, not just planned choices. A copied spell
or ability refers to the original's cost objects under 707.10; copying does not
make another payment. These are required runtime reference contracts, while
backend implementation remains deferred.

Intermediate/report formats 8 record the expanded numeric recipes and current
registry fingerprints. Final cards contain exact typed references and generic
queries, with no source-history recipe or unresolved source escape. Existing
binary tags remain stable; schema version validation rejects older native cards.
Execution of these history contracts remains deferred with Cantrip.
