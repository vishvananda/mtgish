# Normalized semantic IR

`src/ir.rs` is the public entry point. It re-exports the topic modules, so
existing paths such as `mtgish_ir::ir::ObjectSet` remain valid. The normalized
model uses portable data, with lexical bindings and instruction receipts rather
than runtime object IDs or function pointers.

| Module | Contents |
| --- | --- |
| `cards` | Records, faces, characteristics, layout fields and extension operands |
| `abilities` | Ability declarations, functioning scope, limits and deck rules |
| `program` | Instruction streams, actions and execution boundaries |
| `selection` | Announcements, targets, modes, counts and prepared selections |
| `movement` | Destinations, arrival state, ordering, placement and visibility |
| `objects` | Object references, queries, filters and characteristic reads |
| `players` | Player references, sets, roles, history and rules |
| `values` | Numeric expressions, measurements, history windows and comparisons |
| `events` | Trigger matchers, replacement matchers and pending-event changes |
| `changes` | Continuous changes, durations, affected subjects and combat rules |
| `costs` | Casting rules, cost components, payment history and play permissions |
| `mana` | Mana symbols, production, choice timing and restrictions |
| `damage` | Damage instructions' payloads, recipient reads and event predicates |
| `keywords` | Keywords, mechanics and keyword-action vocabulary |
| `actions` | Supporting payloads for choices, counters, exchange and other actions |
| `conditions` | Conditions and coin outcomes |
| `tokens` | Token definitions and copy recipes |
| `copying` | Typed copy exceptions, colors and ability patterns |

The type definitions remain the serialization contract. Moving a declaration
does not change its fields, variant order or public re-export. Changing those
requires reviewing the frozen schema and rebuilding affected artifacts.
The readable contract in `schema/<epoch>.txt` is accompanied by machine-readable
typed descriptors in `schema/<epoch>.json`. Migrations use the archived input
epoch to identify binding sites; they must not infer an old card's types from
the current model. Both current snapshots are checked against compiled source.

`src/declarations.rs` registers model sources once. The frozen schema extractor,
typed visitor generator, authoring codec, Blockly catalog and generator
fingerprint consume this registry. Semantic, vocabulary and authoring sources
have explicit roles; helper APIs such as `TargetSelector` do not silently expand
the wire contract. A regression checks that every external semantic module is
registered.

Execution boundaries carry meaning:

| Form | Boundary |
| --- | --- |
| `Program` | Ordered instructions |
| `Instruction::Group` | A nested instruction stream in the current receipt scope |
| `Instruction::Effect` | One action and its explicit result ID |
| `Operation::InOrder` | Ordered action clauses under an enclosing instruction/result boundary |
| `Operation::Together` | Simultaneous action clauses under an enclosing batch boundary |
| `Instruction::ForEachPlayer` | Player choices in APNAP order before simultaneous actions; receipts retain player identity |
| `Instruction::ForEachObject` | A body with an explicit object binding |
| `Operation::WithSelections` | Lexical preparation followed by the action that supplies the result |
| `Instruction::If` / `ChooseOne` | Independent alternative scopes; an explicit `JoinResults` can gather compatible receipts |
| `Instruction::Optional` | Optional program with stated selection/creation/transition exports; other local results stay private |
| `Instruction::Repeatable` | Repeat only when `RepeatProcess` is reached, gathering immediate body receipts |
| `Instruction::Times` | A repeatable process with a total-run cap and private body receipts |
| `Operation::Repeat` | A fixed number of executions of one action under one receipt boundary |
| `Operation::EachPlayer` | One player-relative action under the enclosing receipt; `Acting` reads that player |
| `Operation::PlayersDo` | A rules-defined player-action policy, including optional/any-one/otherwise choices |

Preserve these boundaries when normalizing. A shared backend call does not prove
that two forms have the same selection timing, receipt identity or replacement
behavior. Likewise, original selected identities, observations, created objects,
and before/after movement views have distinct meanings.

A receipt identifies what happened at an instruction, not a mutable variable.
An observation retains the observed incarnation and order. A selection denotes
the chosen current objects; a movement receipt exposes distinct before/after
views, and created objects have their own result kind. Neither a named binding
nor a `Group` follows an old object through an unrelated zone change. A joined
receipt must come from a compatible choice group; it cannot make every private
branch result available to ordinary reads. See the individual declarations in
`program.rs` for lexical export and execution contracts.

The editor discovers instructions and action families from
`human::editor_metadata`, backed by the same naming policy as card code. Its
Control menu groups sequences/batches, conditions/choices, iteration/repetition,
local selections, and results/receipts. Search includes control instructions as
well as actions; the shared vocabulary does not collapse their distinct types.

Schema 181 gives token recipes, stack copies, copy arrivals and continuing copy
changes the same `Vec<CopyException>` payload. Copy-time exceptions belong to
copiable values; later continuous effects and extra arrival instructions retain
their own typed homes. Numeric inputs use the creating effect's scope, while
quoted granted abilities define fresh scopes. `AbilityPattern` matches abilities
rather than overloading object filters. Numeric engine ranges are unchanged.

Frozen schema-180 descriptors support explicit JSON migration and typed ID-site
origins. A shared PTX read may map to both power and toughness; the mapper records
that expansion instead of inferring identity from repeated numeric labels.
