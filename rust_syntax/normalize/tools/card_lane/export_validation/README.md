# Local export validation runner

Use with frozen `export_lane.py plan` and `build` receipts. The source, publication base, foundation and sibling heads must be fixed before validation. This command creates no publication refs and performs no fetch, push, PR or merge. It records actual results separately from the synthetic preflight tests.

`validate.py` verifies the plan digest, blocker-free status, explicit full-SHA publication base, export receipt binding, exact foundation parent and file blobs, every card commit's single parent, every card's assigned file blobs, group membership (1–20), and each sibling's common foundation. It then creates one **new**, isolated detached worktree and a **new**, private Cargo target directory. It runs the complete checked native workspace test suite, offline and locked, whenever a foundation step changes native inputs, then separately on every sibling head. A foundation step changing only managed files or documentation runs the complete native all-target compilation and explicitly reuses a prior full test result only when every remaining tracked file object is identical. The receipt records the input fingerprint and original test-log hash; shard preflight and aggregation verify that equivalence and reject stale or tampered evidence. Managed and browser validation remains separate. The full script corpus and every integration/unit target run on each actual tree, so missing card, oracle and test dependencies in unrelated existing callers fail the gate. Existing assertions are not filtered. `CANTRIP_CARDS_ONLY` is removed from the child environment so the run cannot silently use a partial card registry.

The private target directory is reused only serially in this same checkout; Cargo's normal tracked-input fingerprints and the build scripts' watched source/scenario paths determine rebuilds. This avoids a cold build for every card group while preventing overlap with an author or live root target. A new worktree, target and results path are mandatory; no existing path is reset or removed. Every terminal command writes its own complete log, SHA-256, exit status, exact head before/after, and dirty-state record; the summary is updated after each passing head and stops on the first failure. The worktree and cache remain available for inspection. If interrupted, preserve the existing output and start a fresh run in new paths; do not interpret partial receipts as success.

Example only (substitute frozen local inputs and approved locations):

```sh
python3 rust_syntax/normalize/tools/card_lane/export_validation/validate.py \
  --repo /path/to/native-repository \
  --plan /path/to/final-plan.json --export /path/to/export-build-receipt.json \
  --expected-base FULL_FROZEN_PUBLICATION_BASE_SHA \
  --worktree /path/to/new-validation-worktree \
  --target-dir /path/to/new-private-target \
  --results /path/to/new-results-directory
```

The `--expected-base` equality is intentional: comparing to an old merge base does **not** prove that the resulting foundation merges into a newer upstream tip. Root must choose the frozen actual publication base after reconciliation. This native gate does not substitute for independently required managed fast tests, browser tests, model/API compatibility checks, or the quiet benchmark; those remain separate final-foundation gates.

The complete per-sibling suite is the conservative minimum for detecting runtime dependencies from unchanged tests and scenarios; a group-only or manifest-only run cannot detect those. It can be expensive. Prewarm the external dependency cache before the approved run: `--offline` deliberately fails instead of fetching anything. The validator uses the final manifest's actual group count.

### Optional parallel validation

Use `--foundation-only` with the ordinary arguments to produce one foundation summary and log. After it passes, run three independent invocations with `--shard 0/3`, `1/3`, and `2/3`, each with `--foundation-receipt /absolute/path/to/foundation-results/summary.json` and **distinct new** worktree, target and result directories. Each invocation repeats the inexpensive complete plan/tree-union preflight, validates the foundation receipt and log, and tests only the groups assigned by their authoritative 1-based manifest index. It does not call its shard a full pass.

After all three succeed, run `python3 aggregate.py --repo REPO --plan PLAN --export EXPORT --expected-base BASE --foundation-receipt FOUNDATION/summary.json --shard-receipts SHARD0/summary.json SHARD1/summary.json SHARD2/summary.json`, saving stdout as the aggregate receipt. The aggregator repeats plan/tree checks and verifies every expected head, log hash, exit status, exact foundation receipt, validator hash, and complete disjoint coverage. Any plan, export, foundation, or validator change invalidates old receipts. Do not run three workers against one live Cargo target or checkout; copy a stopped private cache if desired, preserving Cargo fingerprint checks. These commands run full workspace tests and require explicit scheduling.

Every gate sets `CANTRIP_VERIFY=1`, `RUST_MIN_STACK=33554432`, `CARGO_BUILD_JOBS=4` and `--jobs 4`, and records those values; `CANTRIP_CARDS_ONLY` is unset. Optionally pass `--seed-checked /absolute/path/to/stopped/target/checked`. The validator rejects overlapping paths, symlinks, special files, an unavailable Cargo profile lock and observed open seed files, holds that lock throughout the `cp -a --reflink=auto` copy into the new private `target/checked`, and records the seed path and qualification. The caller must ensure the seed build has terminated and stays stopped during copying; process inspection cannot protect against noncooperating writers. Cargo still checks its own fingerprints. Never share a writable target among workers.

The native-input comparison excludes only `dotnet/`, `docs/`, and `scenarios/README.md`; current native build scripts and tests do not load those paths. All card data, decks, native tests, scenarios, workspace metadata, build scripts and other tracked files remain part of the comparison. No sibling test run uses this shortcut. Synthetic preflight covers documentation-only equivalence, changed-native rejection, and tampered-log rejection; it is not evidence that a real exported head passed.

An initial foundation step changing only excluded managed/documentation paths requires native all-target compilation but introduces no native gameplay change. Its compile-only record binds the exact single parent and proves native input equality; it does not claim a full native test run. A later step that changes native inputs always runs the full suite.

Run `python3 export_validation/selftest.py` from the card-lane directory for the synthetic preflight checks. Each parallel invocation needs its own checkout, target and receipt directory. Serialize the stopped-seed copies: wait for one shard summary to record `cache_seed` before starting the next shard, then let their private builds run concurrently.
