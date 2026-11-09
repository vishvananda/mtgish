# Rust AST normalization

`mtgish-ast` lowers the parser's existing Rust types into generic semantic data.
It is a separate package so a change to the lowerer does not rebuild all of the
legacy schema's serialization derives. The implementation is in progress:
unsupported forms produce diagnostics, and a card with any unsupported nested
behavior is never counted as fully normalized.

Current work prioritizes complete normalization. Cantrip adapters and gameplay
scenarios are deferred. The default build has no `cantrip` feature, and `check`
does not invoke the generator. Iterate with focused Rust regressions, then run
one cached corpus check per coherent batch of related forms. Parser regeneration
is needed only when parser inputs change.

Use staged consolidation: inspect every source subtree, apply generated typed
rewrites, expand generic recipes, bind references/results, then validate and
export the final Rust semantic AST. Unresolved operands survive in intermediate
data and diagnostics; final cards never contain source escapes. Existing native
handlers provide a bridge while families migrate into the reviewed registry.

Choose broad batches from the whole-tree inventory and its per-record source
form membership. `pipeline-candidates` groups actual Rust enum signatures and
attaches occurrence/record counts and examples without parsing the corpus again.
The groups require semantic review: matching field types do not establish equal
meaning. Start with shared operands and recipe families, rather than fixing one
complete card at a time. First-failure binding diagnostics remain prioritization
estimates; they do not describe every unresolved dependency.

From the repository root:

```sh
cargo test --locked --manifest-path rust_syntax/Cargo.toml -p mtgish-ast --test normalization
cargo run --locked --manifest-path rust_syntax/Cargo.toml -p mtgish-ast --bin ast -- check data/mtgish.lines.ron /tmp/ast.coverage.json data/ast_baseline.json

# Full-tree inventory and source preservation, plus strict semantic coverage.
cargo run --locked --manifest-path rust_syntax/Cargo.toml -p mtgish-ast --bin ast -- pipeline-check data/mtgish.lines.ron /tmp/pipeline.json data/ast_baseline.json
# Reuse that report for bulk mapping preparation and representative fixtures.
cargo run --locked --manifest-path rust_syntax/Cargo.toml -p mtgish-ast --bin ast -- pipeline-candidates /tmp/pipeline.json Action /tmp/actions.json
cargo run --locked --manifest-path rust_syntax/Cargo.toml -p mtgish-ast --bin ast -- pipeline-candidates /tmp/pipeline.json GameNumber /tmp/numbers.json
cargo run --locked --manifest-path rust_syntax/Cargo.toml -p mtgish-ast --bin ast -- pipeline-fixtures data/mtgish.lines.ron /tmp/pipeline.json /tmp/pipeline.ronl
```

Numeric consolidation is independent of enclosing action support. The reviewed
`rewrites/numbers.json5` registry emits typed Rust adapters into generic formulas
and zone/player queries. Arithmetic, properties, counts, aggregates, distinct
values, and counters bind through shared resolvers. Unknown numeric leaves stay
in the intermediate tree and block final export when still unresolved.

Saved intermediate artifacts use format `mtgish.pipeline.intermediate.v8` and
carry a fingerprint map for all six registries. Report version 8 separately
counts action, numeric, selector, program, cost/condition, and reusable mechanic rewrites, with per-record source
membership and operand/scope fixture features. Remaining-form lists measure
registry migration, including some forms already supported by native handlers;
they do not enumerate semantic failures. Compiler format versions do not change
the final semantic schema version.

`pipeline-check` succeeds when all records preserve their source data and all
completed semantic cards pass invariants; native completion is a separate count.
`remaining_registry_forms` includes actions already supported by native handlers,
so it measures registry migration rather than semantic failure. Reports count
recipe operands/opcodes and the final semantic vocabulary as well as outer recipe
forms. Fixture selection covers rule/operand-position combinations and enclosing
actor/ability/layout forms. These checks supplement semantic assertions.

Use `pipeline-record <source> <line> <intermediate.json>` to save one staged
record, and `pipeline-export <intermediate.json> <final.ronl>` to validate and bind
it later. Export checks provenance/registry consistency and recomputes the final
card, ignoring the cached semantic view.

Ordinary `lower::card`/`pipeline::finalize` fuses the generated local rewrites,
recipe expansion, and binding without constructing a whole-source inspection
index. Explicit structural reports and intermediate artifacts retain full-tree
provenance and partial results. Prepared and ordinary lowering must agree.

Run affected fixtures during iteration and the accumulated Rust suites plus one
cached corpus check at a broad batch boundary. Binding-only changes can reuse the
source inventory and run native `check`; registry/indexing changes refresh the
structural `pipeline-check`. Compare existing final exports
during adapter migration. Optional Cantrip work stays deferred.

`check` exits unsuccessfully while any card is incomplete. Its JSON report lists
each record, diagnostics with AST paths, unsupported forms, and operation usage.
It records `backend_checked: false`; zero backend-capable records in this mode
means unchecked. Enable the `cantrip` feature and use `check-cantrip` for an
explicit backend capability report after reconciling the backend with the
normalized schema. That compatibility work is deferred during normalization.
It also records input hashes and compares record identities and layouts to the
baseline, preserving duplicates and unnamed records. The baseline records the
original source representation; intentional grammar fixes can change a source
hash while preserving the card identity.

For a quick iteration, run `check` on a small RON fixture without the full-corpus
baseline argument. The focused test fixture contains real parser output for
basic effects, Kessig Wolf Run, Subversive Acolyte, and more difficult cards that
must remain explicitly incomplete until their semantics are implemented.

The converter commands remain available through the original `rust_syntax`
binary's features. Build the `ast` tool without those legacy output features;
in particular, `write_out_json` changes the legacy Serde input representation.

```sh
# Create an initial baseline; do not replace the baseline to hide coverage loss.
cargo run --locked --manifest-path rust_syntax/Cargo.toml -p mtgish-ast --bin ast -- baseline data/mtgish.lines.ron /tmp/baseline.json

# Export only when every record in this input lowers and passes invariants.
cargo run --locked --manifest-path rust_syntax/Cargo.toml -p mtgish-ast --bin ast -- export /tmp/subset.ronl /tmp/normalized.ronl
```

Exports are staged beside the destination and renamed only after successful
validation. A late malformed or unsupported card preserves the previous output.
The normalized schema has its own version and serialization representation.

The parser also provides a static grammar audit:

```sh
cd go_mtg_parser
go build -o /tmp/mtgish-parser
cd ..
/tmp/mtgish-parser --audit-templates --english-grammar-files grammars/english_grammar*.json5 > /tmp/grammar.audit.json
```

It detects unreferenced semantic capture occurrences and flags possible merged
instructions. An unreferenced capture can be intentionally encoded as a literal;
an instruction candidate requires rules review. The audit excludes punctuation
inside placeholders, parenthetical text, and quoted granted abilities. Each
finding includes its source template, output, line, and stable content ID.

To attach actual cards and matched capture values, add
`--audit-template-usage /tmp/grammar.usage.json` to a normal parser invocation.
That reuses the existing derivation trees during card parsing. It costs a full
parser pass, so use the static audit for routine edits and the usage pass for
triage or a coherent grammar batch. [grammar_audit.json](../../data/grammar_audit.json)
at the repository root records the initial findings and card evidence.

The last schema version 5 checkpoint fully normalized 5,389 of 33,529 records using 14 operation kinds.
Resolution-time object selection is separate from announcement targeting, with
typed selection and creation results. The coverage report inventories 38 observed semantic
families, including costs, predicates, expressions, events, changes, and keywords.
These are used forms, rather than a count of every declared enum variant.

Schema version 6 added generic attachment queries and an `Attach` operation.
Enchant retains ongoing host legality, while equip retains its keyword identity
around an ordinary targeted activation. That Rust-only checkpoint fully
normalized 5,886 records, a gain of 497 with no losses, using 15 operation kinds
and 40 observed semantic families. All 33,529 identities and layouts are
preserved; 42 focused Rust tests pass. Backend capabilities were not checked for
this checkpoint. Special equip semantics and unimplemented attack/block
restrictions remain explicit normalization gaps.

Schema version 7 adds shared object, spell, and turn event matchers, event
alternatives, and event-only conditions. Casting and copying remain distinct;
event-only conditions remain separate from intervening-if clauses. Event
bindings are derived from the matcher and intersect across alternatives.
Modal bodies inherit them; independent and granted abilities start fresh.
That checkpoint fully normalized 7,082 of 33,529 records, up 1,196 with
no coverage losses, using the same 15 operation kinds and 43 observed semantic
families. Forty-nine focused Rust tests pass, including typed round trips for
thirty-two complete event fixtures. Backend capability remains unchecked.

Schema version 8 adds a replacement ability with the ordinary conditional
program structure. `ModifyEvent` changes a pending entry; it does not tap a
permanent or add counters after entry. The active scope distinguishes a source's
own entry from replacements supplied by permanents already on the battlefield.
Entry X uses casting provenance, and entry choices use the prospective controller.

`PayCost` exposes a scoped payment decision for resolving costs. “If you do”
tests that decision, even when payment events were replaced. Counter removals
share typed amounts and recipients between costs and effects, and counter
queries retain current or last-known information. Drawing as a cost also affects
mana-ability classification. Generic discarding and player-predicate conditions
reuse the same programs and query types. Bindings from optional or conditional
payments and player predicates cannot escape their scope. Unimplemented copying,
entry choices, and counter allocation across multiple objects remain diagnostics.

The schema version 8 checkpoint fully normalized 8,197 of 33,529
records, up 1,115 with no coverage losses. It uses 19 operation kinds and 47
observed semantic families. Fifty-nine focused Rust tests pass, including typed
round trips for thirty-four complete entry/cost fixtures. All record identities,
layouts, and current input fingerprints are verified. The optional Cantrip feature
compiles; its capabilities and runtime behavior remain unchecked for this batch.

Schema version 9 adds typed rule changes within existing static and continuous
effects. Prohibitions, requirements, declaration costs, proposed combat-group
counts, exceptions, block capacity, combat damage, and untap behavior share these
forms. Rule effects retain live subject queries with captured resolution
variables; characteristic changes use their snapshot policy. Mixed policies
require independently evaluated parts. Activation conditions are checked before
paying costs. Predicate candidates and declaration payers have explicit scopes;
other players remain different from opponents.

The schema version 9 corpus check fully normalized 9,253 of 33,529 records, up 1,056 with
no coverage losses, using the same 19 operation kinds and 57 observed semantic
families. Sixty-eight focused Rust tests pass, including typed round trips for
fifty-five complete rule fixtures. Record identities, layouts, and current input
fingerprints are verified. The optional Cantrip feature compiles; execution and
capability scans remain deferred. Missing casting, zone, library, targeting,
keyword, and other nested semantics still prevent complete normalization.

Schema version 10 adds typed object target domains and shared/private zone
queries. Permanents, spells (including copies), and cards retain distinct
domains while sharing characteristic filters and object references. Top/bottom
queries require one ordered library or graveyard. Exile and graveyard returns
reuse `Move`; tapped entry, counters, controller, and face-down status stay in
its operands. Exile choices precede a single batch, and unions remove duplicates.
Library placement uses the owner's ordinary ordering choice (CR 401.4).

`Counter` remains distinct from movement: its destination applies only when
countering succeeds, while subsequent instructions remain unconditional.
Transition results distinguish the actual objects before and after movement;
old target information and last-known controllers do not become the new card's
owner. Branch-local movement bindings cannot escape. Mana-ability classification
tracks library movement through queries and selections without confusing a
library observation with movement.

The schema version 10 checkpoint fully normalized **10,187 of 33,529 records**, up **934**
with no coverage losses, using **20 operation kinds** and **61 observed semantic
families**. All identities, layouts, and current input fingerprints are verified.
**Seventy-five focused Rust tests** pass, including typed round trips for
forty-three complete zone fixtures and checks on fourteen incomplete fixtures.
The optional Cantrip feature compiles; its new adapters, execution checks, and
capability scans remain deferred. Casting rules, combat-damage triggers, player
effects, and library search/inspection are the next shared blockers.

Schema version 11 adds intrinsic `Casting` abilities with generic cost,
reduction, requirement, timing, target, and mana rules. Casting proposal queries
retain the target's kind. Additional costs, alternative costs, and choices within
costs remain distinct. Reductions apply to total costs with ordinary colored
spillover and generic-only reduction rules; they do not rewrite printed mana
cost or mana value. Flash timing grants neither the flash keyword nor a new zone
permission. Legality requirements apply before payment.

Movement costs share candidate collections, strict counts, destinations, and
entry data. Top-library costs require the full number of top cards; effect
choices that merely do as much as possible cannot substitute for payment.
Filtered/random discards and cost alternatives reuse the cost vocabulary.
Mana classification follows library movement through those cost alternatives.
Mana restrictions preserve producing-source predicates independently of mana
type conversion or spending-as-though permission.

The schema version 11 checkpoint fully normalized **10,602 of 33,529 records**, up **415**
with no coverage losses, using the same **20 operation kinds** and **65 observed
semantic families**. Identities, layouts, and current input fingerprints are
verified. **Eighty-two focused Rust tests** pass, including typed round trips for
forty-seven complete casting/cost fixtures and explicit gaps for thirty more.
The optional Cantrip feature compiles; execution and capability scans remain
deferred. Combat-damage triggers, player effects, library operations, keyword
costs, casting history, and other unsupported forms remain active work.

Schema version 12 shares damage matchers across incoming/outgoing damage,
permanent/spell/other sources, combat/noncombat damage, and player/object
recipients. Four grouping forms distinguish each source/recipient pair, each
source, each recipient, and an entire simultaneous batch. Damage amounts sum
only within that occurrence. Prevention, separate instructions, and separate
combat damage events retain their ordinary boundaries.

Singular role references and participant collections are separately scoped.
Aggregate damage cannot supply a single source merely because a body requests
one. Amount thresholds remain event filters. Alternatives preserve only their
shared bindings, including Cryoshatter's common affected object across its
tapped-or-damaged trigger. Restricted damage targets retain player and object
predicates; player unions deduplicate, and multiplayer sacrifice reuses one
simultaneous operation with choices collected first.

The schema version 12 checkpoint fully normalized **11,113 of 33,529 records**, up **511**
with no coverage losses, using the same **20 operation kinds** and **69 observed
semantic families**. Identities, layouts, and input fingerprints are verified.
**Eighty-nine focused Rust tests** pass, with typed round trips for forty-five
complete damage fixtures and explicit gaps for forty more. The optional Cantrip
feature compiles; capability scans and engine scenarios remain deferred.
Greatbow Doyen and Vraska, Swarm's Eminence retain diagnostics for ambiguous
legacy source/recipient references. First-occurrence history, excess damage,
library operations, player effects, source-zone wrappers, cycling, and delayed
triggers remain active work.

Schema version 13 shares source-zone scoping across ordinary abilities. Scopes
intersect and flatten, retain separate intervening-if clauses, and start fresh
reference scopes. Common hand, graveyard, exile, stack, and command wrappers use
this form; all-zone scopes also include the optional ante zone. Existing stack
abilities continue independently after their source leaves its functioning zone.

Cycling uses a hand-scoped activation with its keyword identity, ordinary costs,
an exact-source discard, and drawing. Cycling/ordinary discard events share
card/player predicates and per-card/per-player grouping. Cycling-or-discarding
does not produce two occurrences. Source-only cycle triggers follow the card
across discard replacements, with a distinct post-discard reference.

Cycling X, entry X, and spell X have explicit provenance. Cycling triggers read
the causing activation; entry abilities read the spell that became the source;
stack conditions and cast triggers query the identified spell. Intrinsic counter
and copy prohibitions and stack spell keywords reuse ordinary static changes.
The two known damage-reference gaps are resolved from the existing template's
typed output and shared roles across alternatives, without card-name exceptions.

The schema version 13 checkpoint fully normalized **11,595 of 33,529 records**, up **482**
with no coverage losses, using the same **20 operation kinds** and **71 observed
semantic families**. Identities, layouts, corpus hash, and 75 input fingerprints
are verified. **Ninety-six focused Rust tests** pass across sixteen suites,
including typed round trips for fifty-seven complete scope/cycling fixtures and
explicit gaps for thirty-five more. The optional Cantrip feature compiles;
adapters, capability scans, and engine scenarios remain deferred. Player effects,
library operations, scry, delayed triggers, and further keyword/rule families
remain active work.

Schema version 14 shares library search, revelation, shuffling, and ordering.
Search choices preserve qualified fail-to-find permission versus unqualified
quantities, distinct cards across find clauses, and different-name constraints.
Consecutive clauses search once; separated unshuffled searches requiring a
shared episode remain diagnostics. Search status is independent of finding a
card, and searching does not imply revelation.

Held-aside cards remain library objects excluded from shuffling. Reordered
revealed/looked-at cards become new objects even without a zone change
(CR 701.20d-e; corrected at schema 17). Found-card identities remain separate
from new objects after movement or such reordering. Searcher, library owner, battlefield controller, and
entry flags stay explicit. Known selection zones and typed search-status results
are validated. Typecycling reuses these operations and retains its type predicate
within the cycling origin.

Scry and surveil retain native rule identities with generic players and amounts,
including private information, zero-count, ordering, and completion semantics.
Mana classification distinguishes observation/reordering from movement to or
from a library and follows selected search results into later moves.

The current checkpoint fully normalizes **12,443 of 33,529 records**, up **848**
with no coverage losses, using **26 operation kinds** and **73 observed semantic
families**. Identities, layouts, corpus hash, and 76 input fingerprints are
verified. **One hundred five focused Rust tests** pass across seventeen suites,
including typed round trips for sixty-six complete library fixtures and explicit
gaps for twenty-four more. The optional Cantrip feature compiles; adapters,
capability scans, and engine scenarios remain deferred. Player effects, general
library inspection, advanced search programs, alternate layouts, delayed
triggers, and further keyword/rule families remain active work.

The initial Cantrip generator emits static card definitions and named callbacks.
At that checkpoint its capability check covered 2,505 records; all 2,505 generated modules compiled
against the sibling checkout. Nineteen engine scenarios exercise drawing, an
activated pump with mana payment, Divine Offering's destruction and target-legality
cases, an untargeted ETB choice, suspension/replay across two prompts, and state
triggers around a collection-wide pump. The last scenario includes a control
case with separate instructions that observes an intermediate board. Compilation
alone does not establish runtime correctness for every generated card. Token
scenarios also cover defined colors and tapped entry, simultaneous entry of
different token types, Food's own activation costs, Mercenary's targeted tap
ability, and the complete exported registry.

Mana production uses four expression forms: an amount, a simultaneous bundle,
a choice of bundles, and repetition with a choice made once or for each unit.
Spending predicates remain separate from the production. Activated mana abilities
are derived from the typed cost, announcement, and program, including the current
library-movement criterion in CR 605.1a; a serialized flag cannot drift from the
body. Spell effects and ordinary activations use the stack, while eligible mana
abilities compile to Cantrip's mana callbacks. The scenarios cover fixed yields,
one chosen color, any combination, dynamic zero yields, Treasure sacrifice,
Chromatic Sphere's stack behavior, and Powerstone's restricted payments.

Token creation uses one operation with typed definitions and entry conditions.
The build script extracts the existing predefined-token records from the canonical
parser export; the lowerer deserializes those recipes as Rust source types and
lowers every nested ability. A missing mana option or token ability is an explicit
failure. Predefined token text and text from the creating effect remain distinct,
so text-changing effects can be passed correctly to Cantrip. Conditional creations
do not leak result bindings into later instructions.

```sh
# Generate Rust from a fully normalized input subset.
cargo run --locked --manifest-path rust_syntax/Cargo.toml --features cantrip -p mtgish-ast --bin ast -- cantrip /tmp/normalized.ronl /tmp/cards.rs

# Explicitly check backend capabilities; this work is deferred during normalization.
just ast_cantrip_check

# Compile representative generated modules and execute core-engine scenarios.
just ast_cantrip_test

# Compile every module advertised by a fresh check-cantrip report.
just ast_cantrip_corpus_test /tmp/ast.cantrip.coverage.json
```

The generated file exposes `definitions()`: pass its result to
`cantrip_api::CardTable::new` to register both the cards and every token definition
they need. Individual modules expose `CARD` and `EXTRA`. Definitions with equal
names but different characteristics remain distinct.

Backend capability diagnostics are independent of normalization diagnostics.
Code generation rejects unsupported semantics and publishes atomically. Generated
modules identify the normalized schema, card content, generator, and snapshotted
Cantrip API and shared mechanics. Integration checks reject a stale API snapshot;
review and regenerate it with `tools/update_cantrip_vocabulary.py ../cantrip` when
the sibling inputs change. Self-ETB triggers and choices consumed by tapping or
untapping are supported. Broader trigger matchers and groups with multiple
mutating operations still produce explicit diagnostics. Dynamic token-defining
power/toughness, entry counters, and mixed token-text origins in one creation batch
also require backend adapters; they are retained in normalized data. Token copies,
transforming tokens, and attacking or attached entry still require lowering work.
Standalone token-definition records and other non-card layouts remain explicitly
incomplete. Multi-change continuous effects remain unsupported until instruction
boundaries are verified. Full normalization and broader executable semantics
remain active work under [AST_PLAN.md](../../AST_PLAN.md). The
[GitHub reconciliation](../../docs/github_issue_review/README.md) records the
source repairs and issue-comment coverage.

Milling as an activation cost is represented and makes the activation use the
stack, but currently has no Cantrip cost adapter. Compound mana callbacks, choices
involving colorless or different bundles, and unsupported spending predicates
also retain precise backend diagnostics. Generator hashes cover the generator,
derived semantics, validation, schema, and API vocabulary inputs.

Schema version 15 generalizes static and resolving continuous effects to typed
object/player subjects, keeping **26 operation kinds**. Player rules cover
restrictions, limits, hand size, land allowances, spell costs/timing, visibility,
ordinary zone permissions, skipped steps, and other shared rules. Explicit origin
zones and scoped affected-player references preserve whose hand/graveyard is
meant; exact exiled-card incarnations remain distinct from live zone queries.
Permissions preserve ordinary costs, timing, and land allowances. Spell-cost
floors and colored reductions that cannot spill into generic mana stay distinct.

The cached corpus check normalizes **12,900 / 33,529** records, up **457** with
no losses, and inventories **83 semantic families**. All identities/layouts,
the corpus hash, and **77 input hashes** are verified. **112 focused Rust tests**
pass, including forty-five complete player fixture round trips and six explicit
negative fixtures. Cantrip compatibility and execution remain deferred.

Schema version 16 adds typed protection, qualified hexproof, landwalk, and ward
operands to the existing keyword form, plus native regeneration-shield creation.
Separate protection qualities retain their separate keyword instances and source
domains. Ward retains a later triggered payment, with fresh payer/keyword-host
scope and cost alternatives distinct from joint costs. Granting ward does not
snapshot its future payment recipe. Regeneration creates a next-destruction
shield, without immediately tapping or clearing damage.

Coverage is **13,489 / 33,529**, up **589** with no losses, using **27 operation
kinds** and **85 observed semantic families**. All identities/layouts, the corpus
hash, and **78 input hashes** are verified. **120 focused Rust tests** pass,
including thirty-five complete keyword fixture round trips and twelve explicit
negative fixtures. Cantrip work remains deferred.

Schema version 17 adds generic `LookAt` observations and captured cohort queries.
Inspection choices, revelation, movement, and ordering share existing operations;
typed set subtraction represents the rest. Placement constraints retain entry
legality and group property budgets. Several type clauses choose distinct cards
before one simultaneous movement. Actual movement results drive remaining-card
queries and conditions about cards put into hand. Branch-local results remain
inside their conditional continuation.

Reordering revealed/looked-at cards creates new identities under CR 701.20d-e,
even without a zone change. Ordering now exposes before/after placement results;
old observation references do not follow the new object. This corrects the earlier
identity-preservation statement. Looking and reordering alone still do not move
cards to or from a library for mana-ability classification.

Coverage is **13,701 / 33,529**, up **212** with no losses, using **28 operation
kinds** and **85 observed semantic families**. All identities/layouts, the corpus
hash, and **79 input hashes** are verified. **128 focused Rust tests** pass,
including thirty-eight complete inspection fixture round trips and fifteen
explicit negative fixtures. Cantrip work remains deferred.

Schema version 18 adds intrinsic characteristic-definition metadata to the
existing static form. Definitions retain all-zone/outside-game scope, layer
precedence, live formulas, defining keyword identity, and color-identity
contribution. Ordinary color/type effects reuse the same changes with set/add
modes. Numeric expressions share aggregates, distinct-value counts, and
mana-symbol counts. Token defining abilities retain their live recipes.

The parser now emits a joint source rule for a single printed power/toughness
ability, retaining separately printed definitions as separate abilities. Maro
and Yavimaya Kavu exercise that distinction. The source rule is appended to the
legacy Rust enum to preserve existing bincode variant indices. All three
checked-in source exports contain **33,529 records**; the only source changes
are **160 records / 164 joint definitions**. Parser, audit, and TypeScript
checks verify that repair; the audit has zero active findings.

Coverage is **14,194 / 33,529**, up **493** with no losses, using **28 operation
kinds** and **90 observed semantic families**. All identities/layouts, the corpus
hash, and **80 input hashes** are verified. **137 focused Rust tests** pass,
including forty-seven complete characteristic fixture round trips and seven
explicit negative fixtures. The large legacy bincode derive is tested on a
dedicated larger-stack thread; normalized AST tests use the default stack.
Cantrip work remains deferred.

Schema version 19 adds `CreateDelayedTrigger` for once-only turn schedules,
reusing event matchers and programs with eligibility gates and optional
expiration/intervening-if checks. Its program is a lexical closure over the
creator's reference frame. Targets, results, original event bindings, source,
and controller remain available; queries/properties are still evaluated when
the deferred body resolves. Future turn events do not overwrite those captured
bindings. Body-local results/searches cannot escape into the creating program.
Delayed targets/modes, new future-event bindings, and repeated temporary
triggers require further scope support and remain diagnostics.

Single/multiple exile returns share `Move`, restricted to exact original
post-exile incarnations still in exile. Owner/controller destinations and entry
counters stay explicit. Conditions about remaining exiled cards reuse queries.
Creating a delayed trigger does not execute its mana or library work immediately.

Coverage is **14,389 / 33,529**, up **195** with no losses, using **29 operation
kinds** and **91 observed semantic families**. All identities/layouts, source
hashes, the corpus hash, and **80 input hashes** are verified. **146 focused Rust
tests** pass across twenty-two suites, including twenty complete delayed-family
fixture round trips and thirty-three fixtures retaining unsupported semantics.
Only the two suites with newly complete fixtures were rerun after promoting
their old negative expectations. No parser/export regeneration or Cantrip work
was needed. Normalization remains incomplete.

Schema version 20 adds cast/entry provenance predicates, a recorded-caster
reference, reusable conditions inside object filters, and actual-spent-mana
expressions. Casting origin, entry origin, and current controller remain
distinct. Spent-unit counts, distinct colors, same-color maxima, and snow
queries do not use mana value or recolor payments through an “as though” rule.
Copied cost choices do not fabricate an actual casting event or mana payment.

Flashback reuses casting abilities with a generic permission, alternative cost,
required resulting spell types, and stack-exit replacement. Its exile rule
applies to countering/bouncing as well as resolution. Graveyard grants reuse
snapshot continuous effects and fresh ability definitions. `ManaCostOf` retains
the receiving card's cost symbols, including X and unpayable absent costs;
it is not that card's mana value. Granted casting abilities continue to the
resulting spell under CR 400.7g with their original expiration.

Coverage is **14,638 / 33,529**, up **249** with no losses, still using **29
operation kinds**, with **93 observed semantic families**. Identities/layouts,
source hashes, the corpus hash, and **81 input hashes** are verified. **155
focused Rust tests** pass across twenty-three suites. Forty-two cast-history
fixtures round-trip through RON, JSON, and bincode; twenty-seven retain missing
semantics. Newly supported existing fixtures were promoted and their affected
suites rerun. No parser/source export regeneration or Cantrip work was needed.
Kicker linkage, further history, conditional entry flags, and the other
remaining families still require normalization.

Before kicker normalization, the source grammar's double-kicker production
was repaired to emit two independently optional `Kicker` rules instead of one
`Kicker(Or(...))`. All 33,529 records pass parser and Rust/binary/JSON/pretty-RON
conversion; exactly eighteen records change and all other source values remain
identical. Fresh audit evidence retains zero active findings. The cached corpus
still normalizes **14,638 / 33,529**, with no losses and all **81 input hashes**
current. The accumulated Rust checkpoint is **156 tests**; only the two changed
suites needed rerunning. The source schema is unchanged. Definition-scoped
casting-choice bindings and conditional kicker announcements remain next work.

The staged registry checkpoint retains semantic schema version 20 and fully
normalizes 14,796 of 33,529 records. Seventy-four generated action mappings cover
23,104 occurrences (37.1%); all 14,796 prior native exports are byte-identical.
Source preservation covers every record. Whole-tree membership supports bulk
review of action and operand signatures, including nodes beneath unsupported
parents. Candidate grouping takes about five seconds using a saved full report.

The generated fixture set contains 2,448 records covering 6,554 observed
rule/operand-position/scope features. The accumulated checkpoint is 173 passing
Rust tests across 24 suites. Broad legacy source-fixture processing uses an 8 MiB
worker to match the CLI's stack; final normalized-card codecs remain checked on
the default test stack. Run the full generated fixture test at broad boundaries;
use focused semantic assertions while editing individual shared templates.

The numeric checkpoint adds 64 generated mappings covering 19,526 of 23,418
numeric occurrences (including 4,184 of 8,076 nonconstant occurrences). Semantic
coverage is 14,809 / 33,529, up 13, with no losses and unchanged final schema 20.
Prepared and ordinary lowering have matching completeness and exact diagnostics
across all records; all 14,809 complete exports match byte-for-byte. The measured
native corpus check takes 40.99 seconds and complete export takes 5.44 seconds.

The expanded generated fixture set contains 3,410 records covering all 138
reviewed rules and 10,203 observed features. Its 701 complete native cards pass
codec round trips on the default stack. The accumulated checkpoint is 183 passing
Rust tests across 24 suites; the final 27-test pipeline suite takes 20.36 seconds.
Cantrip remains unchecked and deferred.

The selector checkpoint adds 211 generated mappings across six source domains,
covering 93,400 / 98,373 selector occurrences (94.9%). Complete coverage is
14,874 / 33,529, up 65, with no losses. All 14,809 prior complete exports remain
byte-identical; prepared/native completeness and exact diagnostics match across
every source record. All 96 input fingerprints and source identities are current.

Generic predicates keep typed operands, candidate scopes, target domains, and
explicit query/result references. Centralized fingerprint maps serve all three
registries and are hashed once per compiled tool. The final Rust schema remains
20; staged artifact format 3 and report format 4 require refreshed compiler data.

The selected fixture set now contains 5,482 records covering all 349 reviewed
rules and 21,307 observed features, including 1,101 complete cards passing native
codec round trips. The accumulated checkpoint is 192 passing Rust tests across
24 suites. The final 36-test pipeline suite takes 17.45 seconds, and exporting
all 14,874 complete records takes 5.59 seconds. Cantrip remains deferred.

The program checkpoint adds 25 generated structural mappings covering 64,166
occurrences across 28,514 records, including 53,402 / 53,714 `Actions` occurrences
(99.4%). Generic recipes retain actors, optional choices, conditions, resolving
payments, sequences, targets, and mode bounds. Explicit batch policy, fresh modal
announcement frames, and branch-local result invalidation remain enforced.

Coverage remains 14,874 / 33,529 with no losses or changed prior complete exports.
Prepared/native completeness and exact diagnostics match across all records, and
both reports have 100 current input fingerprints. The regenerated fixture set
has 6,157 records covering all 374 reviewed rules and 25,247 observed features,
including 1,106 complete cards with native codec round trips. The accumulated
checkpoint is 200 passing Rust tests across 24 suites; the final 44-test pipeline
suite takes 22.95 seconds. Cantrip remains deferred.

The cost/condition checkpoint adds 50 generated mappings covering 25,335 / 26,500
cost occurrences (95.6%) and 6,121 / 7,741 condition occurrences (79.1%). Templates
retain strict payment choices, explicit payers, mana symbols, quantified-player
bindings, and unresolved source operands. Program payments/conditions now embed
these shared recipes. Final semantic schema 20 and its serialization are unchanged.

Complete coverage remains 14,874 / 33,529, with identical prior exports and matching
prepared/native completeness and exact diagnostics on every record. Both reports
have 104 current input fingerprints. The generated set now has 6,647 fixtures
covering all 424 reviewed rules and 28,686 features, including native round trips
for 1,158 complete cards. The checkpoint is 209 passing Rust tests across 24 suites.
The full 53-test pipeline suite takes 42.24 seconds; its 52 focused checks take
3.18 seconds with the generated-fixture test excluded. Cantrip remains deferred.

Semantic schema 21 adds one generic tap-state payment form, preserving object
selection, required state change, and the tap/untap-symbol distinction. Object
tapping, untapping, and one-permanent return costs use shared strict selections.
Eight more mappings bring the operand registry to 58 rules and cost occurrence
coverage to 25,659 / 26,500 (96.8%). Compound group constraints and paid-object
history references remain explicit gaps rather than being discarded.

Complete coverage is 15,089 / 33,529, up 215, with no losses. Every prior complete
export is identical after changing only the top-level schema version from 20 to
21. Existing binary cost tags stay stable and new tags are appended. Validation
rejects prior semantic versions. Prepared/native coverage and exact diagnostics
agree on all records; both reports have 104 current input fingerprints.

The generated set has 6,663 fixtures covering all 432 reviewed rules and 28,788
features, including native codecs for 1,188 complete cards. A focused 24-card
payment fixture adds representative complete and explicitly unresolved cases.
The checkpoint is 217 passing Rust tests across 24 suites, with the final 61-test
pipeline suite taking 23.21 seconds. Exporting all complete cards takes 5.68
seconds. Cantrip execution remains deferred.


Semantic schema 22 adds a generic constrained-payment wrapper and common/distinct
selection properties, shared with library and inspection choices. Six grouped
cost aliases bring the operand registry to 64 rules. A sixth registry,
`mechanics.json5`, expands Crew and CrewOnceEachTurn into shared costs and effects
while preserving Crew identity, participant eligibility, and the distinction
between payment and resolution. Intermediate/report formats are 7. Paid-object
history bindings, alternate crew payments, crew modifiers, and crew event queries
remain explicit gaps.

Complete coverage is **15,186 / 33,529**, up **97**, with no losses. All 15,089
prior complete exports are byte-identical after the top-level schema migration
21 → 22; existing binary tags stay stable. Prepared/native completeness and exact
diagnostics agree on every record. Source bytes and identities are unchanged,
and 109 current input fingerprints are verified. Vocabulary remains 29 operation
kinds / 93 semantic families.

The six registries contain 440 reviewed rules. Crew recipes cover 199 occurrences
across 198 records; costs cover 25,666 occurrences. The refreshed 6,665 generated
fixtures cover all 28,848 observed features and include 1,197 native-complete
cards. A twelve-card group/Crew fixture retains both complete and independently
unsupported cases. The structural pass takes 159.65 seconds, the concurrent
native pass 53.29 seconds, and complete export 6.50 seconds. Cantrip stays deferred.


The checkpoint passes **227 Rust tests across 25 suites**. The full 61-test pipeline
suite takes 25.30 seconds, including generated native codec checks and preserved
partial artifacts. `/tmp/mtgish-group-validation.json` records input/test hashes,
source preservation, full feature coverage, staged/native agreement, and prior
export equivalence after schema migration.


Semantic schema 23 adds explicit payment-object histories. Activation, individual
additional casting costs, and resolving-payment results have distinct namespaces
and exact component paths. Shared input/performed views preserve object identity,
LKI, and actual-action counts without conflating them with payment choice. Planned
inputs can participate in target constraints; recorded references remain stable
for resolution and copies. Singular/plural checks and lexical scopes prevent
ambiguous components, sibling bindings, and later ordinary actions from silently
reusing another payment. Ordinary event histories and linked/repeated/conditional
casting-cost declarations remain separate binding work.

Eleven numeric aliases reuse generic properties/counts/aggregates. Numeric registry
2 has 75 rules covering 19,714 occurrences across 14,070 records. Intermediate and
report formats are 8. All six registries contain 451 reviewed rules.

Complete coverage is **15,273 / 33,529**, up **87**, with no losses and unchanged
source bytes/identities. Prior 15,186 complete exports differ only in top-level
schema version 22 → 23. Existing binary tags stay stable. Prepared/native coverage
and exact diagnostics match on every record; 111 input fingerprints are verified.
The vocabulary uses 29 operation kinds and 95 semantic families, including the
explicit payment namespaces and object views.

The refreshed 6,692 fixtures cover 29,061 observed features and include 1,238
native-complete cards. A 23-card payment-history fixture retains 16 complete and
seven independently unsupported cards. The structural checkpoint takes 131.17
seconds, concurrent native check 43.94 seconds, and full complete-card export 5.49
seconds. Cantrip execution remains deferred.


The checkpoint passes **243 Rust tests across 26 suites**. The full 61-test pipeline
suite takes 24.99 seconds, and 16 payment-history checks cover native references,
codecs, scopes, proposal constraints, and ambiguity. The validation receipt is
`/tmp/mtgish-paid-validation.json`; it includes current source/test fingerprints,
source/feature preservation, exact staged/native diagnostics, and schema-aware
prior export comparison.

## Card code and blocks

The [card blocks demo](tools/blockly/README.md) provides a searchable operation
library and an editable Rust/Wasm-validated card-code view. The public
`human::{read, write}` functions convert between that compact authoring format
and normalized `UnifiedCard`. Shared defaults and vocabulary live in
`human::{defaults, names}`; `mtgish_ir::targets::TargetSelector` groups domains
with their applicable filters. These APIs retain schema 179's wire format and
the compiler's explicit result IDs.
