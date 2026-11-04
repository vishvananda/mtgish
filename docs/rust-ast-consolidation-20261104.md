# Semantic AST consolidation review and first implementation

The six suggestions in `consolidation.md` are useful, but only two are simple
aliases in the current producer. This change removes those aliases and also
implements the bounded source-normalization part of NORM-001. Parser syntax,
source corpus artifacts, Oracle references and the live Cantrip queue are
unchanged.

## Implemented

- `Change::OnlyAbility` is removed. Its producer emits a singleton
  `OnlyAbilities` list, sharing the existing ordered ability-loss/grant handling.
  The plural wrapper remains one effect. In particular, entry-as-Aura lowering
  still extracts its Enchant ability and preserves the abilities the object
  keeps; it must not become several unrelated continuous effects.
- `Expr::PlayerLife` is removed. Life reads use
  `PlayerNumber { player, number: Life }`. `Expr::player_life` is a construction
  helper, not another serialized node. The general single-player reader now
  keeps the signed i32 bounds of life. Counts remain nonnegative. The former
  general reader incorrectly treated life as a `0..u32::MAX` count, which could
  omit the floor required when a negative life total becomes an amount and
  could change bound-driven arithmetic generation. This does not change
  `AmongPlayers` aggregation semantics.
- `change_lists::coalesce_creature_type_additions` joins strictly adjacent
  `CreatureTypes { mode: Add, types: Listed(...) }` changes, retaining first-seen
  order and removing repeated vocabulary words. Set, All, removals and every
  intervening change are barriers. Copiable and Continuing lists normalize
  independently. Layer and static source-list lowering call the helper, so
  Inkmoth Nexus and Nantuko Monastery publish combined additions. This is a
  producer normalization; no scorer or frozen reference was changed. Arbitrary
  handcrafted IR lists are not implicitly rewritten by serialization.

Semantic schema advances from 168 to 170 over two alias-removal commits. Enum
removal changes binary tags. Rebuild semantic RON/JSON/bincode artifacts from
source using the matching generator revision; do not mix old serialized ASTs
with this reader. Previously emitted Cantrip Rust remains valid. A new generator
revision should be enabled at a normal queue boundary, without rewriting frozen
jobs or assuming schema-only comment changes require card reimplementation.

## Validation

- 284 tests pass across the ability alias, change canonicalization, life reads,
  pipeline, entry costs, keyword scope, unified and artifact-animation suites.
- Three focused backend tests verify signed life bounds, negative-amount
  flooring, safe arithmetic and unchanged nonnegative count bounds.
- 183 more tests pass across backend reads/events, multiface generation,
  choices, event lowering, scope validation and final-card reductions.
- All 33,529 source records validate and round-trip through JSON, RON and bincode
  before and after. Every record retains its exact identity, layout,
  completeness and diagnostics. All remain fully normalized and primitive-free;
  the operation inventory is unchanged. The change inventory loses nine
  singular ability nodes and 34 redundant creature-type addition nodes, while
  life reads move to the general player-number inventory.
- All 11 ability-replacement records and nine representative life-reading cards
  generate identical executable Rust and gap diagnostics after removing only
  schema/content/generator fingerprint comments.
- The complete 29-card cohort with adjacent source creature-type additions also
  retains generated code after renaming internal effect temporaries. Spider-Suit
  additionally joins two statements onto one line. No effect, rule, destination
  or callback operand changes. The Inkmoth/Nantuko tests also compare the entire
  previous producer tree, permitting only schema stamps and the specified
  adjacent additions to change.

Detailed logs, old/new modules, full corpus reports and comparisons are retained
in this worktree's `target/consolidation-proof/`. They are scratch evidence, not
published card work. No Cantrip tree, job, cache row, configuration or remote was
modified.

## Remaining candidates and bounded follow-ups

The subsequent result-membership pass is documented in
[`rust-ast-result-membership-20261104.md`](rust-ast-result-membership-20261104.md).
The battlefield-query pass follows in
[`rust-ast-battlefield-queries-20261104.md`](rust-ast-battlefield-queries-20261104.md).

| Candidate | Assessment and next implementation boundary |
| --- | --- |
| Movement/embedded selection | Worth doing after a shared result allocator and resolution-context normalization exist. Start with deterministic, unconstrained movement; lower an embedded selection to `ChooseObjects` and preserve the move's result separately. Then use one typed entry specification. The existing `chosen_then_moved` transformation only handles one narrow bound-player case and changes `Exactly` to `AsManyAsCan` specifically for resolution. Random library order, group constraints, face-down arrival and simultaneous choices need explicit representations and separate tests. Do not apply that count conversion to costs or target declarations. |
| Full ability replacement expansion | The singular alias is gone. Expanding the plural node requires moving entry-as-Aura Enchant extraction into a shared typed entry path, then expanding ordered changes within the original effect in both static and resolving contexts. Preserve effect grouping and ability-layer ordering. |
| Value choices | Replace the two operation forms with one typed choice node only after representing default-one versus any-number explicitly. `ChooseValue.count == 0` is unbounded; `ChoiceTerms.count == None` means one. Audit as-enters discovery in `chosen.rs`, richer-term fallback in `lane_l.rs`, ability subsets, secret/random/order/exclusion flags and result bookkeeping before deleting either form. Equivalent backend calls alone do not prove these forms equivalent in context. |
| Result membership | First introduce typed continuation/result-read traversal. Besides `reads_graveyard_part` and `read_as_objects` Debug checks, update pile complements in `read.rs`, per-object damage prevention in `lane_n.rs` and validation/inventory. Surveil membership reads must continue to request the graveyard result; plain reads used for kept-on-top bookkeeping have a different role. Only then replace `InResult(r)` with `InSet(Named(r))`. Add surveil, exiled/not-chosen-pile and prevention regressions before changing producers. |
| Battlefield queries | Add a typed `battlefield_filter` accessor recognizing the general Shared/Battlefield/Permanent query and migrate the roughly 92 specialized matches first. Then eliminate `Battlefield` at producers and schema consumers. Keep `Permanent`, including tokens; a `Card` query would change semantics. Test count/history special cases such as previously attached Auras, controlled/token permanents and stale-object reads. |
| Life expressions | The direct alias is removed with signed bounds preserved. Separate signed aggregate-life bound improvements, if desired, should have their own arithmetic and empty-player-set tests. |

## NORM-002 is on a separate canonical-contract history

The report's `canonical/events.rs::subject`/`merge_triggers` code is absent from
this fork's b7bbba6 base. A related canonicalizer exists on the separate local
remote-tracking branch `fork/oracle-grammar-canonical-contract` (1caf8a6), which is
not an ancestor of this work. The newer reported runtime revision add8429 is not
present locally. Importing that stack would be an unrelated integration change.
The narrow helper here is therefore named `change_lists`, avoiding a collision
with its `canonical.rs` module.

A bounded fix on that canonical-contract history is to make its existing
source-event predicate recursive: unwrap Filtered; classify a nonempty Either as
source-only iff every alternative recursively satisfies the existing source-only
leaf predicate; classify empty and mixed Either as other. Preserve SourceEntering
handling separately. Test Efreet Weaponmaster's joined versus separate printing,
nested/Filtered Either, mixed general/other-subtype alternatives, empty Either,
target slots, intervening conditions, result references and idempotence. Do not
change immutable gold or the scorer to hide the distinction.
