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
and batches, and game/keyword actions. **Control** groups instruction sequences,
simultaneous actions, conditions and choices, iteration and repetition, local
selections, and results and receipts. **Find a block** searches action and
instruction names, families and Rust documentation. Action names use verbs and their objects, such as
**Draw Cards**, **Move Objects**, and **Destroy Objects**. Event references say
**Event Player** or **Event Object**; **Ability Holder** and **Granting Object**
remain distinct from the source of an effect. **Library** opens card search without discarding the workspace.

**Sequence of instructions** (`Group`) keeps a separate result for each child
instruction. **In Order** and **Together** contain action clauses under one
instruction/result boundary; one is ordered and the other simultaneous. The
repeatable-process blocks run again only when **Repeat Process** is reached;
the bounded version caps total runs. **Repeat** instead runs one action a fixed
number of times. Tooltips describe lexical scope and receipt exports. A
**Choose an option** starter includes two empty alternatives to fill in.

Empty optional fields and default modifiers are hidden. The **⋯** menu adds them
on demand, including targeting. **IR details** exposes bookkeeping/default fields
and the AST inspector. Targets, loop bindings and selections have editable names;
references use menus of names that Rust validates in that scope and type.
Unused action results stay unnamed until **⋯ → Name result** is selected. Renames
update every reference and reject collisions without changing numeric engine IDs.
The normal view combines Effect/Operation and handles
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
have separate symbolic scopes. Used results and explicitly named unused results
keep their names through code edits, imports, exports, undo/redo and workspace
saves. Compiler IDs are allocated when parsing and remain numeric in complete IR. `AnyTarget`, `Creature`, `Planeswalker`
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
* `bindings.js` attaches stable identities to typed ID ports in block state and
  derives fresh wire paths from the current block tree. It covers statement
  chains, combined actions, single-face views, enum/option wrappers and literal
  target pickers. A reference moved across scopes stays blocked until explicitly
  rebound, even when its numeric ID matches a different local declaration. Copies
  get fresh definitions and preserve external captures; cuts preserve identity.
  Names and candidate references are checked through the native Wasm document API.
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
  stay explicit. Schema 180 replaces the six remaining grammar entry lists with typed
  arrival fields. Card-code roundtrips preserve meaning after canonicalizing
  local result IDs.
* `pack.py` embeds Blockly, English messages, Rust Wasm, the generated schema,
  the search index, compressed card data and the Apache license in one HTML
  artifact. Runtime fetches are unnecessary. Vendored Blockly is 13.1.0 from
  its published npm tarball; source attribution is retained.

The UI remains an IR authoring/debugging demo. Valid IR does not prove the card
matches Oracle wording or that every operation has a Cantrip implementation.
The browser demo limits documents to 2 MiB, IR nesting to 255 levels (card-code nesting to 128 levels) and editable
lists to 2,048 elements. These interface limits do not change engine numeric
types. Workspace presentation 6 saves carry a schema, model fingerprint and binding metadata. At the same IR schema,
older layouts migrate through the typed codec with their original data backed
up locally. Saved schema-179 and schema-180 workspaces and IR JSON upgrade explicitly through
Rust to schema 181; the bundle includes the old descriptor table for decoding
those blocks. The original workspace is backed up before conversion. A failed
upgrade preserves the original draft and disables autosaving the fallback card
until the user explicitly starts or imports another card. Unsupported schema
epochs and unknown IR fields are rejected. Current card code uses typed entry
records such as `entry: {tapped: true}` and typed copy exceptions such as
`exceptions: [SetPowerToughness {power: 3, toughness: 3}]`. The Copies & tokens
menu groups creation/copy actions and draggable exception blocks. Exception
amounts accept literal shadows or expressions in the creating scope; granted
rules have fresh scopes. Schema-180 saves with names retain their stable binding
identities, including names in nested granted rules, after native migration.

For native JSON upgrades:

```sh
cargo run --manifest-path rust_syntax/Cargo.toml -p mtgish-ast --bin ast -- migrate-ir old.ir.json > current.ir.json
```

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
npm run test:migration -- /tmp/card-blocks.html
npm run test:bindings -- /tmp/card-blocks.html
npm run test:names -- /tmp/card-blocks.html
npm run test:branches -- /tmp/card-blocks.html
npm run test:copy -- /tmp/card-blocks.html
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

Initial native checks passed 137 AST library tests plus the schema-export and
Wasm-bridge tests (one opt-in AST audit ignored). That first build added no IR
enum or engine behavior. Later schema changes are tracked in
[the implementation record](../../../../docs/ast-clarity-implementation.md).

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

## Native named authoring documents

Use `human::document::read` for an editable authoring document and `write` to
format it without losing names. The document holds a read-only semantic `card()`
and a scoped symbol table. The binding identity is stable within that document;
its name can change without rewriting the card's numeric IDs. `rename` and `rename_many` validate a complete rename
transaction before committing it. `bindings()` and `uses()` expose immutable
metadata; serializing the document saves it with its semantic card. Names are
excluded from card equality and engine lookup. Numeric ID types stay unchanged.
`Document::rebind_ids` transports names through a pure ID renumbering.
`canonical::mapped_unified` supplies the binding-site mapping for structural
canonicalization; `Document::transport` applies it while preserving identities
and names, or rejects an ambiguous declaration merge or split atomically.

```sh
# Reformat a .card file while retaining its names:
ast format-card-code card.card > formatted.card
# Save and restore the complete authoring document:
ast read-card-document card.card > document.json
ast format-card-document document.json > restored.card
```

The Wasm parser's `document` response carries this authoring format.
`describe_card_bindings` creates a document from exact IR without changing its
IDs; `format_card_document` validates and formats a saved document and returns
its upgraded document as well as code. The demo's
inspection API exposes these as `cardBlocks.describeBindings(irText)` and
`cardBlocks.formatDocument(documentText)`. The normal editor uses these APIs for
name-aware validation and code formatting. Import also accepts a saved native
authoring-document JSON file.

Run `npm run test:bindings -- /tmp/card-blocks.html` to test the actual Wasm
bridge, including renames, explicit unused results, scope identity, sibling
reflexive targets and target-dependent activation modifiers.

The opt-in native name audit assigns a distinct explicit name to every target,
result, loop and selection in all 33,529 normalized records, then reparses each
formatted card and compares canonical meaning:

```sh
AUTHORING_CORPUS="$PWD/__build/blockly/cards.jsonl" cargo test --manifest-path rust_syntax/Cargo.toml -p mtgish-ast --test authoring_names all_records_support_the_authoring_document -- --ignored
```

Validated naming update (2026-11-09): the browser test edits an actual name text
field and reference menu, names an unused result on demand, renames all four
binding families, checks separate player/object loop roles with equal raw IDs,
rejects a cross-scope move and explicitly repairs it, copies statement groups
from this workspace and a foreign clipboard, cuts/pastes without renaming, and
preserves names through undo/redo, reload, model upgrades and code export.
All 33,529 corpus records pass an independent comparison of every block binding
port's path, type and ID with the wire schema, in addition to block roundtrips
and native Wasm validation. UI presentation rebuilds transport metadata through
matching typed wire sites and surviving declaration identities.

Authoring document version 2 gives each `If` branch and `ChooseOne` option its
own declaration identities. A normal reference can use only a result that is
available in its lexical scope. `JoinResults` can explicitly gather alternative
receipts; its code normally lists the declaration names directly. Repeated names
can be qualified with a readable lexical path, for example
`ReceiptFrom("option_1", picked)`. The formatter chooses the shortest unique
path suffix, and ambiguous or unavailable qualifications are rejected. A shared
numeric receipt label is stored as a group expression with distinct declaration
members; rename its declarations rather than the group expression.

Version-1 documents are checked against their original grouping before an
automatic metadata migration splits independent declarations. Their semantic
card and explicit names survive. Workspace presentation 6 also migrates saved
version-5 binding metadata, retaining the original draft in
`mtgish.blockly.before-authoring-v6` until the native migration succeeds.
Neither migration changes schema 180 or numeric ranges. The branch browser
suite covers independent renames, qualified references, private scopes, complete
group copies, legacy saves and version-1/version-2 document import:

```sh
npm run test:branches -- /tmp/card-blocks.html
```

Destination validation (2026-11-09) rejects battlefield-only arrival state on
hand/library movement and exile-only state on a battlefield destination. The
same checks cover top-card movement, countering, creation and selection
constraints. Library placement on Conjure requires a library zone. Hidden exile,
exile counters and granted suspend remain valid. These checks run in native Rust
and the bundled Wasm validator; schema 181, authoring document 2 and presentation
6 are unchanged. Splitting the remaining generic entry fields and serialized
destination payloads remains part of the broader AST work.
