# Card blocks demo

The generated `card-blocks.html` is one self-contained file. Open it in a modern
browser; it also works from an ordinary static web server. Search the entire
checked-in card corpus, inspect/edit the blocks, start a blank creature or draw
spell, and export validated normalized IR JSON or editable card code. No server, CDN, network service,
Oracle-text reconstruction or Cantrip runtime is involved.

The **Build** view is a Scratch-style authoring workspace. Drag an ability from
Abilities into the card, then drag actions from Actions into its `do` slot. Actions
connect vertically and can be reordered by dragging. Numbers and common player
choices are editable shadow blocks in typed sockets. Drop an expression onto a
number or picker to replace it; removing the expression restores the previous
simple value. There is no conversion menu. Keywords are small stackable
blocks. **All actions** groups the complete operation library by purpose: cards
and libraries, damage and life, objects and movement, counters and changes,
choices and values, mana and casting, triggers and replacements, control flow
and batches, and game/keyword actions. **Find an action** searches names, groups
and Rust documentation. Action names use verbs and their objects, such as
**Draw Cards**, **Move Objects**, and **Destroy Objects**. Event references say
**Event Player** or **Event Object**; **Ability Holder** and **Granting Object**
remain distinct from the source of an effect. **Library** opens card search without discarding the workspace.

Empty optional fields and default modifiers are hidden. The **⋯** menu adds them
on demand, including targeting. **IR details** exposes bookkeeping/default fields
and the AST inspector. Result IDs require an explicit **⋯ → result ID** selection
in IR details; ordinary effects keep that bookkeeping hidden. The normal view combines Effect/Operation and handles
Program/ability/action arrays as statement stacks, so the user does not have to
assemble serialization wrappers. New actions receive fresh result IDs; imported
result IDs remain untouched. Power/toughness use two inline fields.

Targets have named choices: **any target**, **target creature**, **target
planeswalker**, and **target player**. The editor maps these to the appropriate
Rust target domain and filter. Restricted/compound targets retain their complete
filter and appear as **custom target**. Changing the domain preserves target
counts, slot IDs, distribution, chooser and selection constraints, and updates
direct damage-recipient references within the same ability. Ordinary
damage needs no checkbox; **⋯ → can't be prevented** adds the exceptional
modifier. Its checkbox is the inverse of the wire model's `preventable` field.
Older complete workspaces migrate without changing their damage semantics.

The **Card code** button opens the editable code view without exposing internal
block fields. **IR details** also reveals those fields for debugging. It omits serialization wrappers,
empty optional fields and known defaults; **Apply code** parses and validates in
Rust/Wasm before replacing the blocks. Unapplied edits stay in the editor while
blocks change. **Export card code** saves a `.card` file. **Readable JSON** omits
empty characteristics; **Complete IR** shows the exact wire representation.
**Export valid IR** always saves complete Rust-validated IR.

```text
Card {
  name: "Lightning Bolt",
  properties: {card_types: [Instant], mana_cost: [Color(Red)]},
  abilities: [Spell {
    targets: [Target {name: victim, select: AnyTarget}],
    do: [DealDamage {from: This, to: Targets(victim), amount: 3}]
  }]
}
```

The format is an authoring view of Rust IR, not reconstructed Oracle text. Result
bindings are optional: `chosen = NoteObjects {objects: One(This)}` followed by
`NoteObjects {objects: Selected(chosen)}` names a result. Independent abilities
have separate symbolic scopes. Only used result bindings appear when rendering;
internal IDs are allocated when parsing. Formatting uses stable `r0`/`t0`
names rather than preserving the spelling of input symbols. `AnyTarget`, `Creature`, `Planeswalker`
and `Player` use Rust's typed target API. Object and player filters remain
separate on custom damage targets. Ordinary damage is preventable; write
`unpreventable: true` for the exception. State changes such as `tapped: false`
remain explicit. Omitted mana cost means no mana cost, while `mana_cost: []`
means a zero cost. Absent power/toughness differs from `[0, 0]`.

Rust APIs are `mtgish_ast::human::{read, write}` and
`mtgish_ir::targets::TargetSelector`. Inspect a card or lower a `.card` file:

```sh
cargo run --manifest-path rust_syntax/Cargo.toml -p mtgish-ast --bin ast -- card-code data/mtgish.lines.ron 'Lightning Bolt'
cargo run --manifest-path rust_syntax/Cargo.toml -p mtgish-ast --bin ast -- read-card-code /tmp/example.card
```

Save workspace preserves the block layout; Export valid IR saves the actual IR.
Import accepts `.card` files, IR JSON and saved workspaces. Edits are also saved to browser-local storage. Undo
and redo use complete workspace snapshots so structural edits include their
nested blocks. Connected v1 workspaces are migrated through the lossless IR
codec, with the original workspace backed up locally; incomplete drafts remain
available in their original representation.

## Build

Requirements: this repository's Rust toolchain, Python 3, the
`wasm32-unknown-unknown` target, and `wasm-bindgen` CLI 0.2.127.

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.127 --locked
rust_syntax/normalize/tools/blockly/build.sh /tmp/card-blocks.html
```

The default output is `__build/card-blocks.html`; intermediate output stays in
`__build/blockly`. Set `MTGISH_BLOCKLY_BUILD` to use a different build directory.
A release Wasm build can take several minutes the first time. Subsequent builds
reuse Cargo's target cache. The generated HTML embeds compressed corpus shards,
so a selected card is decompressed on demand rather than lowering 33,529 cards
in the browser. Rebuild after changing the IR, validator or Source corpus.

## Design

* `blockly_catalog.rs` reads the actual Rust IR/type declarations, including the
  counter vocabulary, imported Source vocabularies, Serde variant renames and
  the compiled macro-generated Mechanic registry. It exports the current full
  normalized corpus. There is no manually maintained second IR schema.
* `app.js` maps that schema to editable typed Blockly blocks and back to the
  exact JSON wire shape, including tuples, options, lists and both enum layouts.
  Primitive, vocabulary and scalar newtype fields are inline; compound list items use statement sockets. Rust documentation appears when
  selecting a constructor. Explicit integer text fields and lossless JSON
  handling preserve the full i64/u64 ranges.
* `blockly-wasm` depends on `mtgish-ast` with default features disabled. It uses
  the existing `validate::unified` and strict `validate::card`, not a JavaScript
  imitation. Card-code parsing and formatting also run in Rust/Wasm. Unknown fields, unreduced forms, invalid ranges, wrong schema and
  bad reference scopes are rejected. Checks run in a Web Worker; results from
  an older edit cannot enable exporting a newer unvalidated edit.
* `human` provides the reversible authoring codec and shared semantic defaults,
  action names, categories and role names. Both the card-code renderer and the
  block editor use that Rust metadata. Meaningful timing, batching and provenance
  stay explicit. The existing schema-179 wire shape stays unchanged; card-code
  roundtrips preserve meaning after canonicalizing local result IDs.
* `pack.py` embeds Blockly, English messages, Rust Wasm, the generated schema,
  the search index, compressed card data and the Apache license in one HTML
  artifact. Runtime fetches are unnecessary. Vendored Blockly is 13.1.0 from
  its published npm tarball; source attribution is retained.

The UI remains an IR authoring/debugging demo. Valid IR does not prove the card
matches Oracle wording or that every operation has a Cantrip implementation.
The browser demo limits documents to 2 MiB, IR nesting to 255 levels (card-code nesting to 128 levels) and editable
lists to 2,048 elements. These limits do not change Rust IR numeric types or
schema 179. Workspace saves carry a schema and model fingerprint. At the same IR schema,
older layouts migrate through the typed codec with their original data backed
up locally. A different IR schema requires explicit conversion.

## Verification

```sh
cargo test --manifest-path rust_syntax/Cargo.toml -p mtgish-blockly-wasm --lib
cargo test --manifest-path rust_syntax/Cargo.toml -p mtgish-ir --lib
cargo test --manifest-path rust_syntax/Cargo.toml -p mtgish-ast --test human_code
# Full semantic card-code roundtrip after building the demo:
HUMAN_CODE_CORPUS="$PWD/__build/blockly/cards.jsonl" cargo test --manifest-path rust_syntax/Cargo.toml -p mtgish-ast --test human_code -- --include-ignored
cargo test --manifest-path rust_syntax/Cargo.toml -p mtgish-ast --bin blockly_catalog
# Optional browser regressions (requires Playwright and a Chromium installation):
cd rust_syntax/normalize/tools/blockly
npm ci
npx playwright install chromium
npm test -- /tmp/card-blocks.html
npm run test:catalog -- /tmp/card-blocks.html
npm run test:authoring -- /tmp/card-blocks.html
npm run test:targets -- /tmp/card-blocks.html
npm run test:code -- /tmp/card-blocks.html
npm run test:sockets -- /tmp/card-blocks.html
```

Primary Blockly documentation: [connection checks](https://docs.blockly.com/guides/create-custom-blocks/inputs/connection-checks/),
[serialization](https://docs.blockly.com/guides/configure/serialization/) and
[mutator state](https://docs.blockly.com/guides/create-custom-blocks/mutators/).
Wasm is packaged using [wasm-bindgen's no-bundler output](https://wasm-bindgen.github.io/wasm-bindgen/examples/without-a-bundler.html).

The earlier `~/mtgish_blockly_demo.tgz` demonstrated useful typed connections,
list controls and workspace persistence, but targeted the original parser's
Source enums. This implementation targets the normalized IR and derives the
block definitions from that model. It does not execute generated JavaScript.

Validated initial build (2026-11-08): all 33,529 corpus records round-trip through
the generated block schema and pass the real Wasm validator, with zero failures.
Rendered Blockly/workspace roundtrips cover 14 representative cards including
split and transforming layouts, modal/targeted spells, delayed triggers,
planeswalkers and exact-duration effects. Browser scenarios also cover search,
new cards, field edits, grouped list undo/redo, incompatible socket rejection,
JSON download, invalid result scope, workspace restoration and the maximum i64.
No HTTP requests are made when opening the standalone file from disk.

Native checks: 137 AST library tests plus the schema-export and Wasm-bridge tests
pass (one existing opt-in AST audit ignored). The demo adds no IR enum, numeric
range, schema version or engine behavior.

## Deployment

The public demo is served at `https://tetrarcum.com/blocks` (and `/blocks/`).
Caddy serves the standalone HTML from `/srv/mtgish-blocks/current/index.html`.
The `current` symlink points to a versioned directory beneath
`/srv/mtgish-blocks/releases`; replacing that symlink publishes or rolls back one
complete bundle. No backend service or extra open port is needed.

The domain's Caddy site adds only this exact-path handler:

```caddyfile
@card_blocks path /blocks /blocks/
handle @card_blocks {
    root * /srv/mtgish-blocks/current
    rewrite * /index.html
    header Cache-Control no-cache
    file_server
}
```

Rebuild the HTML, copy it into a new release directory as `index.html`, and move
`current` atomically. Each deployed release includes a SHA-256 manifest. The
initial Caddy configuration was validated before reload and its previous file
was retained as `/etc/caddy/Caddyfile.before-blocks-20261108`.

Authoring update (2026-11-08): all 33,529 records still round-trip and validate
unchanged through the projected presentation. A browser test performs real mouse
drags from the category flyout, attaches a spell ability, stacks Draw and Gain
Life, edits an inline amount, adds targeting on demand, toggles detail mode and
downloads complete valid IR. Readable-dump tests verify omitted empty fields,
preserved 0/0, and the full wire representation. The updated toolbox/menu sprite
is embedded so the standalone file still has no runtime network dependency.

Workspace migration preserves Blockly's serialized checkbox booleans as well as
the live `TRUE`/`FALSE` field representation. The full-corpus codec test exercises
this serialized form, preventing a saved `preventable: true` (or other boolean)
from silently changing during an authoring-view upgrade.

Validated authoring update (2026-11-09): all 33,529 records round-trip through
card code with equal canonical semantics; the normalized export is unchanged.
All 33,529 also pass the block codec roundtrip and real Wasm IR validation.
Browser checks cover all 134 non-draft action starters, action search, shared
labels, code edits, rejected typos, draft preservation, target-domain inference,
`.card` export, 14 varied Wasm card-code roundtrips and workspace migration.
