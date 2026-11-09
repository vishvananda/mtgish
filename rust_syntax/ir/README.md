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

The type definitions remain the serialization contract. Moving a declaration
does not change its fields, variant order or public re-export. Changing those
requires reviewing the frozen schema and rebuilding affected artifacts.

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

Preserve these boundaries when normalizing. A shared backend call does not prove
that two forms have the same selection timing, receipt identity or replacement
behavior. Likewise, original selected identities, observations, created objects,
and before/after movement views have distinct meanings.
