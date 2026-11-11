# Card-lane recovery and retirement — 2026-11-11

The campaign is paused. All six implementer, five reviewer and two verifier
worktrees were clean and had no running assignments. Their 13 prepared worktrees
and two retired reviewer worktrees were removed after preserving every head under
`refs/card-lane-retired/20261111/`. Original branches, queue rows, review packets,
reports, transcripts, integration/publication branches and source worktrees remain.
Local Cargo configuration was saved separately. The integration watcher was stopped.

## Failure findings

Several reviewers and the queue watcher exited while opening SQLite, at
`PRAGMA journal_mode=WAL`, with `sqlite3.OperationalError: disk I/O error`.
Systemd records independently show implementer i6 and verifier v2 failing this way
on November 6 at 15:54 UTC. The launchers gave only integration automatic restart;
other workers therefore remained dead after the storage fault cleared. The current
database passes `PRAGMA quick_check`; no running or queued review assignments remain.
The logs do not establish the underlying filesystem fault's exact cause. An older
reviewer r7 log separately records disk quota exhaustion in `/tmp`.

An older r3 crash is a missing producer `packet` field. Current code catches packet
preparation as infrastructure failure, but it still required that field even when
complete candidate-bound author evidence was already cached. Missing optional
campaign/producer metadata could also prevent preparing the packet.

## Repair

- Queue initialization retries transient SQLite open/WAL/schema-initialization
  failures three times, with one- and two-second backoff. Failed connections close.
  Only this idempotent initialization is retried; card transactions and commits are
  never automatically replayed. Corruption and unrelated SQL errors are not retried.
- Every maintained systemd service uses `Restart=on-failure` and `RestartSec=5`.
  The legacy runtime launcher now delegates to the maintained launcher and retains
  its existing `mtgish-unlimited` prefix.
- Before touching the checkout, a restarted implementation/review slot checks for
  its existing running assignment. If one exists, it parks for explicit recovery,
  preserving both the lease and worktree. It cannot silently claim the next card
  after an ambiguous failed commit. A disabled slot does not open the queue or
  recreate a retired source worktree.
- Packet preparation can use its existing job/round-bound author evidence without
  the deleted producer packet path. Oracle/reference and author-report evidence
  remain mandatory. Missing evidence is an explicit infrastructure error before
  invoking a model, with restoration instructions; it is never fabricated or
  treated as a review pass. Optional prior-findings metadata stays optional.

All 401 card-lane Python tests pass, including injected storage failures,
connection cleanup, transaction non-replay, retained assignments, paused/removed
trees, cached/missing evidence and service restart settings. No Rust or Wasm
rebuild is needed for these lane-only changes.

## Local records and resuming

The runtime campaign is `~/card-lane/pipeline-unlimited-20261103`. Maintenance
records are under `~/card-lane/maintenance-20261111/`:

- `queue-backup.sqlite`, `database-before.json`, and `campaign-before.json`;
- `retired-workers.json`, with exact paths, original branches and preserved refs;
- `worker-configs/`, `services-before.txt`, service journals and test logs;
- `cleanup-summary.json` and the maintained launcher's `restart-plan.json`.

Queue status/stage counts are unchanged after retirement, and all preserved refs
were verified. Report disk savings cautiously: these were CoW snapshots sharing
most source extents, and other workloads change filesystem free space concurrently.

Keep the campaign disabled until explicitly resumed. Recreate the configured
worktrees from the manifest's saved branches/heads using `cow-git worktree add`;
verify each head against its archive ref, restore its local Cargo configuration,
and prepare private caches using [SETUP.md](../rust_syntax/normalize/tools/card_lane/SETUP.md).
Do not restart a slot onto an unrelated current main or discard a running lease.
The configuration records the retirement manifest and the user-requested pause.
No workers were restarted during this maintenance.
