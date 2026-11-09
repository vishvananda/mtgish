# Normalized AST consistency and readability review

Reviewed fork `main` at `01e2cd784ed51ea8b2a0e9a29cae895f0c284ac1`, schema
179. This reviews the Rust representation itself, its validator, authoring
codec, and selected generator consumers. It does not implement a schema change.
The six consolidations from the November 4 review are already complete; the
findings below are additional work.

The strongest next step is to finish typing the remaining semantic payloads
and close validation omissions before removing more operation variants. Named
bindings and clearer family names would improve readability without changing
numeric ranges or making the runtime resolve strings.

Evidence includes a type-directed inventory of all 33,529 current exported
records and nine small inputs passed to `ast read-card-code`. Those probes use
the same `human::read` and strict card validation as the browser bridge. They
test adversarial authoring inputs, not just successful source-corpus output.
No production Rust, parser, generator, or deployed application was changed for
this review.

1. **Fix incomplete validation of entry payloads first.**

   `Scope::entry` validates some nested payloads but skips others.
   `EntryState::RulesUntil` checks its duration without checking its rules;
   `FaceDownWith` checks `noted` without checking `abilities`; several other
   operand-bearing entry variants return success directly.
   `Operation::CounterWith` does not validate its entry payload or destination.
   See [entry validation](../rust_syntax/normalize/src/validate.rs#L713) and
   [CounterWith validation](../rust_syntax/normalize/src/validate.rs#L2853).

   Confirmed counterexamples: an unannounced `t99` is accepted inside
   `CounterWith.entry` and inside `EntryState::RulesUntil.rules`. The same
   reference in an ordinary `Reveal` is rejected. An arbitrary `Integer(42)`
   is also accepted as a `MoveTopCard` entry instruction. Generic operand
   validation checks references recursively when invoked, but does not establish
   that an operand is a legal entry instruction.

   This does not show that existing cards are wrong. It shows that the current
   validator does not establish the same guarantees for every authoring path.
   First check every nested semantic child in its proper scope, then replace
   generic payloads with types that rule out these malformed inputs.

2. **Give bounded counts a consistent validation contract.**

   [Count validation](../rust_syntax/normalize/src/validate.rs#L1314) rejects a
   negative `Exactly`, but `Between` only checks the upper bound for negativity.
   It checks the lower bound as an expression without checking its value or
   comparing it to the upper bound. Both `Between(-1, 1)` and `Between(3, 1)`
   are accepted on a target declaration. In contrast, the mode validator already
   rejects negative or reversed constant bounds.

   Add shared checks for literal nonnegative bounds and ordering, used where
   appropriate by counts and modes. Dynamic bounds need an explicit contract
   rather than assumed ordering. Keep `Exactly`, `AsManyAsCan`, and value-choice
   cardinality distinct: their existing targeting, cost, and resolution semantics
   differ. The corpus contains 86 `Count::Between` nodes in 84 records; the
   negative probes do not establish defects in those records.

3. **Complete the move from generic operands to typed semantic payloads.**

   The inventory finds 141 directly `Operand`-valued field positions in the
   exported descriptors for explicitly declared IR types; 85 have nonempty
   values in the current corpus. There are 2,623 `Operand::Term` occurrences
   across 1,500 records, despite zero `Operand::Form` occurrences.

   Bound descriptive terms are intentionally different from unreduced forms;
   these counts do not mean those 1,500 cards failed normalization. However,
   generic terms still leave field shape and vocabulary conventions outside
   Rust's type system.

   Entry handling is a useful bounded first cohort:

   | Operation | Nonempty generic entry payloads |
   | --- | ---: |
   | Conjure | 65 |
   | MoveTopCard | 20 |
   | Meld | 7 |
   | Manifest | 2 |
   | CounterWith | 2 |
   | Populate | 1 |

   These are payload occurrences, not disjoint card counts. Ordinary `Move`
   already uses `EntrySpec`. The generator still decodes strings such as
   `EntersTapped` for the other forms in
   [`entry_of`](../rust_syntax/normalize/src/cantrip/lane_i.rs#L3991).

   Normalize these terms before generation, sharing typed arrival state where
   meanings agree. Some entry flags change the destination controller, so the
   migration must normalize destination and entry together. Do not merge the
   underlying actions: conjuring, countering, and ordinary movement remain
   different rules actions.

   Copy adjustments are the next substantial cohort. Generic payloads occur in
   `TokenRecipe::Copy.adjustments` (280), `Change::CopyOf.effects` (96),
   `EntryState::CopyOf.effects` (69), and related copy variants. Introduce typed
   copy exceptions while preserving copiable versus continuing changes,
   quotation identity, and the scope in which each expression is evaluated.

4. **Preserve named, immutable bindings in authoring documents.**

   The compact reader accepts names such as `victim` and `chosen_cards`, but its
   output is normalized IR, so formatting subsequently prints `t0` and `r0`.
   That loses useful author intent even though it preserves semantic meaning.

   Preserve names in an authoring document or scoped symbol table alongside the
   normalized card. Resolve names to typed numeric IDs once for compilation.
   Independent abilities can reuse a name; references must resolve in the correct
   lexical scope and namespace. Target names, instruction results, loop bindings,
   and prepared selections must not share an untyped global name map.

   Renumbering and canonicalization must update or return a mapping for this
   metadata. A flat `ResultId -> String` map is insufficient because IDs repeat
   in independent scopes. Names should not participate in semantic equality.

   Improve the Rust names as well: `Slot` can become `TargetSlot`, while `Binding`
   should clearly identify a loop binding. `ObjectRef::Named` and
   `ObjectSet::Named` are complete result reads, whereas `Named` elsewhere means
   unreduced source syntax. Name those reads for their recorded-result role.
   Keep `Selected`, `Observed`, `Created`, and before/after transitions explicit;
   a readable name must not silently follow a card through zone changes.

5. **Make movement's state depend on its destination.**

   `Move` independently carries `destination`, `entry`, and `face_down`.
   `EntrySpec.states` also has several face-down variants. These represent
   different things: the corpus has 88 moves with the exile `face_down` flag and
   11 with a face-down entry state. They are not unconditional aliases.

   Nevertheless, the struct permits meaningless combinations, and its validation
   is uneven: tapped arrival and the exile flag have destination checks, but an
   arrival `FaceDown` state on an owners-hand destination passes the reader.

   A typed destination carrying its own arrival/visibility/placement terms would
   express the distinction better: battlefield arrival state, exile visibility,
   and library position/order. Introduce shared constructors and checks first;
   folding those fields into a serialized destination enum is a later schema
   migration, not a cosmetic rename.

6. **Factor selection mechanisms out of bags of optional fields.**

   `ModeSelection` has 12 fields mixing the options, cardinality, selector,
   repetition, history, price, budget, and target relationships. Current records
   include 728 ordinary configurations, 56 implied configurations, and smaller
   cohorts using the other features. Separate announced versus implied selection,
   then factor the selecting party/mechanism from the applicable selection rules.
   Do not make all existing flags mutually exclusive: several valid features
   combine.

   Coin flips are a smaller opportunity: `FlipCoin` occurs 66 times, `FlipCoins`
   five times, and `CoinFlips` nine times. The last mixes optional `times`,
   optional `each`, `until_lost`, `may_stop`, and `called`. A typed repetition
   policy and called/uncalled outcome policy would explain the combinations
   better. Unification must retain player ordering, individual flip identity,
   winning versus losing, counted results, and the surrounding result contract.

   Similarly, move parameters into the appropriate `KeywordAction` variants
   instead of permitting arbitrary combinations of `Operation::Keyword`'s
   optional `object`, `other`, `amount`, and `subtype`. This makes illegal shapes
   harder to construct and gives the editor a better block definition.

7. **Use role names consistently, while preserving semantic families.**

   | Current name or pattern | Improvement |
   | --- | --- |
   | `Search.player`, `Search.library` | `searcher`, `library_owner` |
   | `ReorderLibrary.player`, `ReorderLibrary.library` | `chooser`, `library_owner` |
   | `ChooseValue.actor` | `chooser` or `choosers`, reflecting its set type |
   | `Tap { tapped }` | `SetTapped`, already used by the authoring alias |
   | `Expr::Power` | `Exponentiate`, distinguishing arithmetic from creature power |
   | `ObjectRef::KeywordHost`, `Granter` | Carry the established `AbilityHolder` and `GrantingObject` vocabulary through the Rust API |

   This is a role-based policy, not a global replacement of `actor` with
   `player`: the person choosing can differ from the affected player, controller,
   owner, or source of an effect. Apply singular/plural names consistently with
   `PlayerRef` versus `PlayerSet`.

   `Recipients` also has broader responsibilities than its name suggests. It is
   used for damage, attachment hosts, and counter copying. Its
   `CountersTheEventPut` variant names counters from an event, not an ordinary
   collection of entities. Give that counter source a typed home in counter
   copying instead of broadening every recipient consumer to accept it.
   `DamageRecipients` is an event predicate, while `Recipients` reads entities;
   their similar names are not evidence that they should become one enum.

8. **Document and group control flow around execution and result boundaries.**

   `Instruction::Group(Program)` (3,689 occurrences) and
   `Operation::InOrder` (three) both look sequential, but the operation sits
   beneath one instruction/result boundary. `Together` (19) specifies a
   simultaneous group. They cannot simply become one generic block list.

   Likewise, `Instruction::ForEachPlayer` explicitly collects choices before
   simultaneous actions, while `EachPlayer`, `PlayersDo`, and object iteration
   have their own bodies and result behavior. Describe these differences at the
   declarations and expose sequence, simultaneous batch, and iteration as
   discoverable families. Any consolidation needs receipt and native-behavior
   comparisons, not just matching backend calls.

9. **Group the Rust sources by topic after centralizing schema discovery.**

   `ir.rs` is 6,780 lines with 233 explicit public struct/enum declarations.
   Related movement, copy, event, choice, and reference types are spread across
   it. Topic modules with re-exports can improve discoverability without changing
   public paths or serialization order.

   A file split currently has hidden dependencies: the visitor build script,
   frozen schema extractor, authoring codec, Blockly catalog, and generator
   fingerprint all read `ir.rs` directly. Centralize declaration/source discovery
   first, then split modules and prove that frozen schema, wire output, and
   generated code remain unchanged. Merely moving declarations today would make
   some consumers lose them.

10. **Keep the Scratch-style input change in the editor.**

    A literal amount and a computed amount are already one `Expr` field. A
    common player picker and a complex selection already produce `PlayerSet`.
    They should be one typed socket with an editable literal/picker when no
    expression block is connected. This removes the hidden “Use a block for…”
    conversion and permits a clear return to a simple value. No additional AST
    operation or schema change is needed for this interaction.

Additional low-priority cleanup: `UnifiedCard.layout` and layout `Field` names
are strings. A record with `layout: "Typo"` currently passes validation. Typed
known layouts or an explicit extension case would make that contract clearer.
The 18 observed layouts include metadata-only sticker sheets and other special
records, so a universal one-face/two-face arity rule would be incorrect.

Recommended implementation order: close the validation omissions and interval
checks; type the remaining entry payloads; preserve scoped names; establish the
shared naming/schema registry and topic modules; then evaluate copy, mode,
keyword-action, and coin-flip representation changes in separate cohorts.
The Scratch input work is independent UI work. None of these proposals requires
widening numeric types or implementing unrelated Cantrip engine features.

Reproduction of the two unbound-reference omissions:

```text
Card {
  name: "Unchecked counter entry",
  properties: {card_types: [Instant]},
  abilities: [Spell {do: [
    CounterWith {
      object: This, destination: OwnersHand,
      entry: [Objects(Targets(t99))]
    }
  ]}]
}
```

```text
Card {
  name: "Unchecked entry rules",
  properties: {card_types: [Instant]},
  abilities: [Spell {do: [
    MoveObjects {
      objects: One(This), destination: Exile,
      entry: {states: [RulesUntil {
        rules: [Objects(Targets(t99))], until: EndOfTurn
      }]}
    }
  ]}]
}
```

Save either as `/tmp/probe.card` and run:

```sh
cargo run --manifest-path rust_syntax/Cargo.toml -p mtgish-ast --bin ast -- read-card-code /tmp/probe.card
```

Both currently succeed. Replacing the action with
`Reveal {objects: Targets(t99)}` correctly fails. The full inventory, executable
probe inputs, and captured results from this review are retained locally under
`/mnt/vizier/ast-review-20261109/`.
