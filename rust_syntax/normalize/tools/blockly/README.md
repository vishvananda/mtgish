# Card blocks demo

The generated `card-blocks.html` is one self-contained file. Open it in a modern
browser; it also works from an ordinary static web server. Search the entire
checked-in card corpus, inspect/edit the blocks, start a blank creature or draw
spell, and export validated normalized IR JSON. No server, CDN, network service,
Oracle-text reconstruction or Cantrip runtime is involved.

The **Build** view is a Scratch-style authoring workspace. Drag an ability from
Abilities into the card, then drag actions from Actions into its `do` slot. Actions
connect vertically and can be reordered by dragging. Numbers and common player
choices are editable directly inside the action. Keywords are small stackable
blocks. **Library** opens card search without discarding the workspace.

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

The default **Readable** AST dump omits null/empty entries inside characteristics
and preserves actual zero values, including 0/0. Select **Complete IR** for the
exact wire representation. Export always writes the complete Rust-validated IR,
regardless of the display format.

Save workspace preserves the block layout; Export valid IR saves the actual IR.
Import accepts either file. Edits are also saved to browser-local storage. Undo
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
  imitation. Unknown fields, unreduced forms, invalid ranges, wrong schema and
  bad reference scopes are rejected. Checks run in a Web Worker; results from
  an older edit cannot enable exporting a newer unvalidated edit.
* `pack.py` embeds Blockly, English messages, Rust Wasm, the generated schema,
  the search index, compressed card data and the Apache license in one HTML
  artifact. Runtime fetches are unnecessary. Vendored Blockly is 13.1.0 from
  its published npm tarball; source attribution is retained.

The UI remains an IR authoring/debugging demo. Valid IR does not prove the card
matches Oracle wording or that every operation has a Cantrip implementation.
The browser demo limits documents to 2 MiB, nesting to 255 levels and editable
lists to 2,048 elements. These limits do not change Rust IR numeric types or
schema 179. Workspace saves carry a model fingerprint to prevent silently
loading a layout against a different schema/validator build.

## Verification

```sh
cargo test --manifest-path rust_syntax/Cargo.toml -p mtgish-blockly-wasm --lib
cargo test --manifest-path rust_syntax/Cargo.toml -p mtgish-ast --bin blockly_catalog
# Optional browser regressions (requires Playwright and a Chromium installation):
cd rust_syntax/normalize/tools/blockly
npm ci
npx playwright install chromium
npm test -- /tmp/card-blocks.html
npm run test:catalog -- /tmp/card-blocks.html
npm run test:authoring -- /tmp/card-blocks.html
npm run test:targets -- /tmp/card-blocks.html
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
