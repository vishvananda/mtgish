# Early-set generator repairs — 11 November 2026

This pass fixes generator gaps before adding engine features. It targets Cantrip
`6814db9c8c564e8c51c6f698c1f487189baf48f7`, API fingerprint
`31d0e482b00c413d94f371d6cde3b468b35221d47f0ca6d5cf90c64ee2612bff`.
The user's older Cantrip main is unchanged, the card campaign stays paused, and
no engine implementation or card PR is part of this change. The AST and schema
remain at version 190. Game quantities, bot interfaces, Wasm and the site do not
change.

## Result

Relative to generator `03bb80b`, 123 cards lose all generator gaps across the
33,529-card corpus, with no newly gapped cards. These include 28 cards missing
from Cantrip in the through–Ice Age scope, plus three already implemented cards:
Clockwork Avian, Clockwork Beast and Consecrate Land. These are generation
readiness counts, not new cards installed in Cantrip.

The scope has 1,320 eligible unique cards after excluding ante, dexterity and
Shahrazad; flavor-banned cards remain included. Current Cantrip implements 1,034
(78.3%), compared with 193 at the original campaign base and 519 at the validated
November 3 snapshot. Of the 286 currently missing cards, 132 now have no generator
gaps and 154 still have gaps. Across all eligible cards, 1,151 have no generator
gaps; some implemented native cards still have generator gaps.

The machine-readable [receipt](cantrip-early-set-generator-20261111.proof.json)
contains the exact names and validation results. The 154 outstanding cards must
still be classified by source, lowering, generator and native requirement. They
are not 154 engine features. The earlier preliminary 53-capability inventory is
not a final minimum: native behavioral tests disproved several proposed
"generator-only" adapters, as documented below.

## Generator changes

| Existing native support | Generator repair |
| --- | --- |
| Resolved land-type effects | Use `EffectKind::SetLandType`, preserving CR 305.7 rather than merely adding a subtype. Resolve Vision Charm's second choice from its local binding. |
| Untap and control-change events | Use native event shapes and preserve the exact watched creature when control is lost. |
| Event-delayed object lists | Retain captured objects for Glyph of Doom, Phantasmal Mount, Venomous Breath and War Barge. |
| Counter-removal events | Match the shared counter event mask and the event's resulting total. Named removing players remain unsupported. |
| Counter-placement ceilings | Offer the full X choice, convert only the chosen value at the existing native quantity boundary, and let the engine apply the ceiling after replacements. |
| Entry choices | Store two distinct basic land types, pay/store Nameless Race's chosen life amount, and choose each Frankenstein's Monster counter independently. |
| Exact movement costs | A zero-card exile cost succeeds; a positive exact cost requires the entire selected payment to reach exile. |
| Payment history | Read typed, activation-specific and spell-specific object receipts for Necropolis, Soul Exchange and other simple flat costs. Retain explicit gaps for ambiguous compositions. |
| Resolution counter payments | Tetravus reads the counters actually paid for that resolution, rather than casting X. |
| Shared mechanics | Use exile-on-leave for Dreams of the Dead, atomic life exchange for Mirror Universe, and compound mana/life costs for cumulative upkeep. |
| Attachment and attack rules | Use existing no-other-Auras rules and an open attacker restriction for Island Sanctuary, including newly entering creatures and next-turn expiry. |
| Ordinary resolution locals | Preserve Forgotten Lore's previous choices and last successful choice; keep Game of Chaos's stake across repeated flips. |
| Existing mana replacements | Express Chaos Moon's temporary colorless-mana rule through the existing replacement system. |
| Granted abilities | Basic landwalk choices use the five proved basic types. Gabriel Angelfire can choose/grant rampage; its missing upkeep duration remains a gap. |
| Ordered graveyards | Calculate Ashen Ghoul's position directly from the public ordered zone. |

Capability detection is updated for existing interfaces. Profiles without the
interfaces retain planned holes. A completed module never silently drops an
unsupported instruction.

## What did not prove to be a generator-only repair

- **Sword of the Ages:** the native API declares `VariableCost::SacrificeX`, but
  regular activation announcement handles mana X, loyalty X, removal X and tap X
  without announcing sacrifice X. A native test paid only the source and dealt
  zero damage. Do not enable this by detecting the enum variant alone. The card
  retains its existing activation-cost gap.
- **Divine Intervention / Psychic Purge:** matching needs the actual removing
  player or discard cause. `CountersRemoved` has no actor. The trigger matcher
  sees history ending before the current event batch, so looking through history
  cannot recover the pending event's cause. Current Oracle really says **you**
  remove the last intervention counter; do not erase that requirement or report
  it as a reference error.
- **Barbarian Guides / Illusionary Presence:** Oracle permits an unrestricted land
  type, not just a basic type. The native finite landwalk keyword set does not
  implement every chosen subtype, including the snow-qualified case. Giant
  Slug's basic-type choice is supported separately.
- **Runesword:** applying a regeneration prohibition from an ordinary damage
  trigger is too late for lethal-damage state-based actions. It needs a rule
  effective for the damage before those actions.
- **Balduvian Shaman / Rapid Fire:** querying a keyword family needs reliable
  ability metadata. Do not infer it from display labels or function addresses.
- **Goblin Flotilla / Melee:** a delayed trigger at end of this combat needs the
  correct combat-bound lifetime. An effect's end-of-combat duration does not
  supply that delayed-trigger contract.
- **Errant Minion / Power Leak:** subtracting prevention from damage would skip
  prevention rules. A general next-N shield can leak unused capacity into later
  damage, while next-time prevention prevents the whole event. The generated
  partly-prevented-damage contract remains explicit pending exact native support.
- **Cursed Rack:** the current printed maximum-hand-size play rule applies to its
  controller; a rule applying to opponents needs a supported player selector.
- **City in a Bottle:** a source-wide boolean state trigger is insufficient for
  the separate permanent identities/groups that its state trigger must retain.

Additional reference investigations remain separate from this generator pass:
Heroism appears to place its payments at damage time rather than resolution;
Glyph of Delusion's granted effects appear incorrectly limited to end of turn;
Infernal Denizen's opponent action needs unambiguous original-controller/doer
bindings; Sacred Boon's prevention capture/delayed aggregation needs review.
These notes are investigation handoff, not claims of newly filed upstream issues.

## Validation and reproduction

All 1,566 generator tests pass (21 optional tests ignored). All 33,529 generated
modules type-check against the pinned API, in 32 shards with eight compiler
processes (74.49 seconds using cached native dependencies). The 21 focused native
behavior scenarios also pass with verification enabled.

Run generator checks once for the batch:

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=8 cargo test \
  --manifest-path rust_syntax/Cargo.toml -p mtgish-ast --features cantrip
```

The focused regression corpus contains 51 cards, including deliberately blocked
controls. Its generated modules are also type-checked against the pinned native
API. The runtime fixture executes 21 scenarios with `CANTRIP_VERIFY=1`, covering
payments, replay through suspensions, land choices, control loss, delayed lists,
Aura legality, counter replacement order, and attack-rule expiry.

The optional native integration entry point is checked in with its fixture:

```sh
MTGISH_CANTRIP_DIR=/path/to/pinned/cantrip \
MTGISH_CANTRIP_TARGET=/path/to/separate/native/target \
CARGO_BUILD_JOBS=8 RUST_MIN_STACK=16777216 cargo test \
  --manifest-path rust_syntax/Cargo.toml -p mtgish-ast --features cantrip \
  --test cantrip_early_sets early_set_generated_effects_execute_in_cantrip \
  -- --ignored --nocapture
```

This session executes the same generated modules and checked-in fixture directly
with `rustc --test`, reusing the prior pristine native build to avoid rebuilding
Cantrip. Full-corpus type-checking uses the previous validation target's
names-only counter overlay: it appends counter names without altering existing
IDs or implementing rules. Planned holes remain gated during compilation.
Compilation proves interface compatibility; it does not establish gameplay
correctness for every card or turn a card with holes into an implementation.

Local evidence is under `/mnt/vizier/early-set-coverage-20261111`: final rows,
corpus delta, compiler diagnostics, native scenario output and generator logs.
