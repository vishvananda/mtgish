# Reproducing the card pipeline

The lane code, dashboard, service launcher, accounting and cache cleanup are in this directory.
Campaign configuration and state live outside the checkout. Each worker gets a separate prepared Cantrip
worktree and Cargo target. Implementation, review/fix and verification pull one card at a time; a context
lasts up to 20 cards and expires after an hour idle. The example pool has six implementers, five reviewers
and two verifiers. The counts are configurable.

## Prerequisites and revisions

Use Python 3.11 or later, Git, the Rust/LLVM tools required by Cantrip, `cargo-llvm-cov`, an authenticated
`gh`, and the configured worker backend on `PATH`. The service launcher and CPU measurements use Linux
systemd user services and cgroup v2. Without systemd, run the individual worker/watch commands below in
separate terminals. Client work also needs Cantrip's .NET, WebAssembly and browser-test tools.

Keep the generator and native API compatible. [tested-profile.json](examples/tested-profile.json) records
the pair used for this setup. Until the linked foundation is merged upstream, fetch its branch into your
Cantrip clone and use that commit as `BASE`. The generator vocabulary fingerprint is embedded in generated
files; changing a Git label alone does not make incompatible APIs compatible.

```sh
T="$PWD/rust_syntax/normalize/tools/card_lane"   # from the mtgish checkout
CANTRIP="$HOME/src/cantrip-rs"                 # your Cantrip clone
LANE="$HOME/card-lane/lane-demo"               # generator, Oracle cache and LLVM shim
P="$HOME/card-lane/pipeline-demo"               # queue, reports and service logs
POOL="/path/on/reflink-filesystem/card-workers" # worker sources and private caches

git -C "$CANTRIP" fetch https://github.com/vishvananda/cantrip-rs.git \
    card/early-sets-20261106-v4/foundation
BASE=27690f3fb867737b346c0df275c6ddf34121ff17
git -C "$CANTRIP" worktree add -b lane-demo/seed "$LANE/cantrip" "$BASE"
export CARD_LANE="$LANE" CARD_LANE_CANTRIP="$LANE/cantrip"
export CARD_LANE_BRANCH=lane-demo/seed CARD_LANE_TARGET="$LANE/cantrip/target"
cargo install cargo-llvm-cov --locked --root "$LANE"
python3 "$T/card_lane.py" setup --checkout "$CANTRIP"
python3 "$T/card_lane.py" rows
```

The seed is created first so `setup` retains the selected compatible revision. An absent seed is normally
created from `origin/main` by `setup`. The setup command builds the generator and corpus, fetches Scryfall
when needed, and prepares the coverage shim. The checked-in `data/mtgish.lines.ron` is its parsed input.

## Configure and prepare slots

```sh
python3 "$T/control.py" --root "$P" init \
    --lane-root "$LANE" --cantrip "$CANTRIP" --worktrees "$POOL" --base "$BASE" \
    --implementers 6 --reviewers 5 --verifiers 2
```

This writes absolute local paths, an empty implementation queue, and a campaign-specific service prefix.
It preserves existing state by refusing to overwrite a campaign. It does not start workers or reserve cards.
`cargo_build_jobs` defaults to 8 and `rust_min_stack` to 16777216 bytes; set them in the campaign configuration
before starting to control compiler parallelism and stack size for workers and integration checkpoints.
Set `publisher.fork_owner` to your GitHub account before later publication. Keep `publisher.enabled: false`
for local consolidation and final export.

Create the configured worktrees. The standard Git route works without a custom worktree tool:

```sh
python3 - "$P" "$CANTRIP" <<'PY'
import json, pathlib, subprocess, sys
root = pathlib.Path(sys.argv[1]); repo = sys.argv[2]
cfg = json.loads((root / 'config.json').read_text())
slots = {**cfg['implementers'], **cfg['reviewers']}
for slot, spec in slots.items():
    tree = pathlib.Path(spec['tree']); tree.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(['git', '-C', repo, 'worktree', 'add', '-b',
                    cfg['unit_prefix'] + '/' + slot, str(tree), cfg['base']], check=True)
subprocess.run(['git', '-C', repo, 'worktree', 'add', '-b',
                cfg['integration']['branch'], cfg['integration']['tree'], cfg['base']], check=True)
PY
```

For warm builds, keep the seed idle after its build finishes and reflink its `target/` into each new worker's
`target/` on the same filesystem. For example, repeat this for each configured worker:

```sh
mkdir -p "$POOL/i1/target"
cp -a --reflink=always "$LANE/cantrip/target/." "$POOL/i1/target/"
```

`--reflink=always` fails if the filesystem cannot share extents, instead of silently making a full cache copy.
Cargo still checks fingerprints and may rebuild paths or changed card registries. Each worker writes its own
cache. A Btrfs `cow-git` installation can instead snapshot the warm seed while creating worktrees; the optional
[configuration template](examples/cow-cantrip.conf) lists the settings. That tool is external to mtgish.
Compiler writes consume private space even with CoW; the supervisor's six-hour cache cleanup remains necessary.

## Inspect, intake and run

```sh
python3 "$T/control.py" --root "$P" start --dry-run
python3 "$T/intake_lane.py" --root "$P" plan --set ARN --fetch-set
python3 "$T/intake_lane.py" --root "$P" add --set ARN
# Alternatives: --card "Name", --file cards.txt, or --set LEG --color blue
python3 "$T/control.py" --root "$P" start
"$T/status.sh" --root "$HOME/card-lane"
```

Intake reserves only eligible generator-ready cards by default. Ante and dexterity cards are excluded; flavor
bans are not. The launcher starts workers, queue supervision, local integration and CPU sampling. It skips
already running services and draining slots. Service names include the campaign prefix; logs are in the
campaign's role directories. The units are transient: rerun `start` after a reboot.
Failed services restart after five seconds. SQLite initialization retries transient storage
errors before failing; card transactions are never replayed. A restarted slot with an unfinished
running assignment parks for explicit recovery, preserving its lease and checkout.

After explicitly retiring prepared worktrees during a pause, recreate their saved branches/heads
and private caches before `start`. Disabled workers do not recreate removed worktrees. Missing
producer packets can use existing candidate-bound cached author evidence; absent Oracle/reference
or author-report evidence requires restoration before review.

```sh
python3 "$T/control.py" --root "$P" drain    # finish active cards, then wait
python3 "$T/control.py" --root "$P" status
"$T/status.sh" --root "$HOME/card-lane" --once
"$T/status.sh" --root "$HOME/card-lane" --json
"$T/status.sh" --root "$HOME/card-lane" --costs
```

In a terminal the dashboard stays open and refreshes every second; `--watch 0.5` changes the interval.
It discovers campaigns, keeps natural worker ordering, shows the intake/review/fix/verify pipeline, retry
rounds, recent outcomes, per-phase rates, API-equivalent cost estimates, and measured CPU usage. Colors
honor `NO_COLOR`, and Ctrl-C restores the terminal. `CARD_LANE_HOME` sets its default discovery root.

For foreground supervision, run `implementation_lane.py --root "$P" worker i1`,
`review_lane.py --root "$P" worker r1`, and `review_lane.py --root "$P" worker v1` in separate terminals,
alongside `review_lane.py --root "$P" watch` and `integration_lane.py --root "$P" watch`.
The service command plan printed by `start --dry-run` includes all slots and the CPU-monitor prefix.

## Consolidation, validation and publication

The assembled branch can contain unresolved shared proposals. Promote only a validated checkpoint with
`integration_lane.py --root "$P" checkpoint`; workers keep using their immutable pin while consolidation
continues. See [README.md](README.md#consolidate-locally-publish-when-finished) for the reconciliation process.

Use `export_lane.py` to freeze the final file objects and reconstruct one shared foundation with independent
card groups of at most 20. Literal Rust embeds and scenario deck references are audited. Held publications
and deferred implementation work remain explicit. A prepared foundation series can be retained with
`build --foundation-head COMMIT`.

[export_validation/README.md](export_validation/README.md) provides the exact-head native gates, private
cache seeding, bounded shards and receipt aggregation. Run separate required managed/browser checks.
Freeze a stopped seed during copies; start shard cache copies serially so their exclusive seed locks do not
collide. Retain reports and source worktrees, then retire successful temporary caches with `cache_cleanup.py`.

Publish the foundation first. Dependent card PRs contain the identical prerequisite commits and a card-only
comparison, targeting upstream `main` when only the fork is writable. They stay draft until the foundation
lands. Before using grouped PRs to renew claims, ensure the upstream claims reader includes every closing
issue; [the grouped-claims fix](https://github.com/cantrip-rs/cantrip-rs/pull/2593) raises its query limit from 5 to 100.

Only code, templates and instructions belong in the checkout. Keep campaign databases, worker transcripts,
Oracle bulk caches, generated Cargo targets, credentials and current reservation lists in your runtime directories.
