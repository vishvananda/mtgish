# Published Oracle IR dependencies and cross-producer validation

MTGish consumes `oracle-ir = "=0.1.0"` and
`oracle-ir-contract = "=0.1.0"` from crates.io. `rust_syntax/Cargo.lock`
records their registry checksums. The Source frontend, normalization facade,
build dependencies and independent Blockly Wasm bridge use these same versions.
Oracle Grammar's parser is a separate producer; it is not a new runtime
dependency of the MTGish Source parser.

The original Git-provider migration and its historical proof remain in
[the ownership migration record](oracle-owned-model-consumers-20261113.md).
This update preserves schema 190, numeric domains and saved Source data.

## Validation gate

Use the original Scryfall JSON pinned by a native face release, plus its complete
GitHub release directory. Pin `manifest.json` by a separately recorded SHA-256.
Run from this repository with Rust 1.98 or newer, Go and Python 3.11 or newer:

```sh
python3 rust_syntax/normalize/tools/oracle_export.py \
  --bundle /path/to/native-face-release \
  --oracle /path/to/original-scryfall-oracle.json \
  --manifest-sha256 ACTUAL_MANIFEST_SHA256 \
  --work rust_syntax/target/oracle-export/run-001
```

The work directory must be new; existing reports are not overwritten. Full
snapshots, parser outputs and comparison artifacts remain outside tracked data.
Select an explicitly authorized population in the release manifest. Do not use
held-out texts to author grammar rules.

The gate verifies asset digests, source snapshot identity, population, every
source occurrence and coverage counts. It requires the release's model package
versions to match the locked registry dependencies. It then runs the existing
Scryfall preprocessing and MTGish Go parser afresh against that exact population,
without replacing `data/oracle.json` or `data/mtgish.lines.ron`. It preserves the
preprocessed input and parser failures, including the effects of MTGish's ignore
and find/replace policies. Parser binaries are reused only when their source,
compiler and binary checksums match.

The Rust stage lowers those fresh Source records into the shared typed AST and
validates each comparable face. Records join by their full set of face names and
the face name, while release occurrences retain provider ID and face position.
A standalone face cannot accidentally match its namesake on another record;
duplicate Source identities are an explicit failure.

`comparison.json` distinguishes exact agreement, canonical-only agreement,
semantic differences, missing or incomplete MTGish records, native refusals,
ambiguity, limits and malformed exports. Both sides must validate before an
agreement is counted. Canonical-only differences stay visible and fail the strict
gate: an equivalent alternate encoding is not silently accepted as a reason to
keep inconsistent lowerings. Ambiguity never selects a convenient reading.

Native refusals have no exported value to compare. They contribute coverage,
never agreement. `passed` means that every unique exported face compared
exactly, with at least one actual match and no ambiguous or exhausted-budget
outcomes. `complete_agreement` additionally requires every input face to match;
it is false when any native refusal remains. These are face comparisons, not a
claim that unexported record-level layout fields or token definitions agree.

Each finding retains its identity and original Oracle text. Differences include
both AST values and the first differing JSON pointer. `receipt.json` records
model versions/checksums, grammar producer, source hash, MTGish input fingerprints,
fresh Source hash, toolchain, executable hash and comparison hash. A comparison
fails if inputs change during its run. The low-level command is
`ast compare-oracle-export SOURCE.ronl REQUESTS.jsonl REPORT.json`; use the Python
wrapper for release/provenance checks.

## Updating dependencies and exports

1. Select a published grammar/face release and pin its manifest, original source
   snapshot and intended population. Keep the previous bundle for comparison.
2. Update the exact model versions in the four consumer manifests. Update only
   these locked packages with `cargo update --manifest-path rust_syntax/Cargo.toml
   -p oracle-ir -p oracle-ir-contract`. Review the lockfile diff for other changes.
3. Run the MTGish workspace tests, the Wasm build and this gate. On a model-only
   packaging update, compare the full existing Source corpus's lowered JSONL
   against its previous validated export as well. Source parse success alone
   does not establish AST agreement.
4. Investigate every difference from its original wording and operands. Group
   shared template/normalization causes, but retain a witness for each distinct
   identity, timing, scope or cardinality. Never edit the expected value merely
   to make a gate pass, or collapse an ambiguity by first match.
5. Add an independent failing regression before fixing the responsible producer.
   Re-run the affected cases together; reserve full corpus/consumer gates for the
   final batch. Retain before/after receipts and changed-face lists.
6. Publish new immutable artifacts through the publishing worker, update pins,
   then rerun the gate against those actual published values. Do not replace an
   existing release asset or bypass the registry with a local dependency patch.

## Triage and upstream reporting

- If MTGish's Source parser encodes the wrong Oracle meaning or chooses needless
  alternate Source constructors for the same text, fix the parser/template here
  and verify the discrepancy against upstream's parser in an isolated checkout
  before filing at `i5jb/mtgish`. Do not pull upstream into an active worktree as
  part of triage.
- If Source is correct and our Rust lowering is wrong, fix this fork's lowering
  and its tests. That alone is not an upstream MTGish reference defect.
- If the native grammar, source adapter or exporter is wrong, record a focused
  Oracle Grammar regression and fix that producer. If the shared AST cannot
  represent the distinction correctly, address the model contract first, with
  an explicit schema/compatibility decision.
- An input snapshot, preprocessing or exclusion difference is an input-policy
  finding until equivalent original inputs have been demonstrated. Missing
  records and two refusals are not evidence of semantic agreement or a parser bug.

A confirmed upstream issue includes original Oracle text, snapshot hash and face
identity, both producer revisions, observed Source and AST values, the expected
AST distinction with rules evidence, an exact differing path, and the smallest
independent regression/control pair. Report consistent-encoding problems too,
even when their meanings happen to be equivalent.

Oracle Grammar's independently pinned reference is not silently rewritten.
Pending corrections/normalizations belong in its
`adapters/mtgish/updates.json`, using that repository's input-checked from/to
records, evidence and issue link. When upstream adopts a fix, remove the adopted
update, advance the reference artifact/checksum and rerun comparisons. A stale
or already-adopted update must fail rather than be applied twice.

## Caller-stack safety

A default-stack regression exposed legacy `Action` serialization through the
Source inspector on a 2 MB test thread. Corpus workers already had a protected
stack, but direct `inspect::value` and `Identity::of` callers could bypass it.
The inspector now enters that boundary once at its public entry point and uses
an internal recursive visitor. Inputs must be `Sync` to permit a borrowed tree
to be inspected safely on a scoped worker. Normal corpus workers stay inline;
thread construction is kept in a separate cold function. AST fields, numeric
widths, schema and serialization are unchanged.

The full workspace passes with `RUST_MIN_STACK` absent. Dedicated regressions
exercise a 2 MB caller stack and verify that an existing protected worker is
reused. Full-corpus inspection manifests remain identical. Seven interleaved
before/after measurements in the development profile (Source unoptimized,
dependencies at opt-level 2, two corpus threads) are retained in the proof.
These development timings are not an optimized-release performance claim.

## Verification

- Published dependencies resolve to one registry instance each at `0.1.0`;
  unrelated locked packages were retained.
- Full workspace: **2,003 passed, 20 ignored**, without `RUST_MIN_STACK`.
- Five provenance/population tests and seven Rust comparison tests pass.
- The complete **33,529-record** Source corpus lowers, validates and binary
  roundtrips to byte-identical JSONL against the previous provider export.
- The actual wasm32 bridge builds; its normal dependency graph contains no
  MTGish Source parser or AST/compiler facade.
- A synthetic pinned bundle exercises fresh Go parsing through registry-backed
  Rust lowering: matching values pass and an intentionally changed P/T operand
  fails with its differing path. A native refusal is recorded, not counted as a
  match. These are independent test fixtures, not a published face-data audit.

Run the gate against the publishing worker's actual face bundle and its pinned
original source snapshot when those data artifacts are available. Publishing
crates alone does not provide independent per-card expected values. Evidence and
artifact hashes are recorded in
[the registry-consumer proof](oracle-registry-consumers-20261113.proof.json).

The producer now pins its original Scryfall snapshot and publishes matching face
bundles after code releases. Bundles include the original source bytes. Use the
included `oracle-source.jsonl.gz` as `--oracle` when the manifest declares
`encoding: "jsonl-gzip"`; older JSON array sources remain supported. The source
checksum always covers the original compressed bytes, not a rewritten array.

The first real published-data run is complete; see
[face adoption and audit](oracle-face-adoption-20261113.md). Its strict structural
comparison remains failed and is retained as triage evidence. The approved five
history thresholds are adopted without rewriting the published expected values.
