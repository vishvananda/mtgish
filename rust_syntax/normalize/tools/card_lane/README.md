# Card lane

Turns cards the mtgish generator writes whole into `Card:` commits on a local Cantrip branch. One worktree, one card at a
time, one commit per card. It replaces nothing in vizier, and it pushes and posts nothing.

Only cards with no gap come here. `ast cantrip-files` writes a card from its closed table of recipes
(`src/cantrip/card_file.rs`), or, for a card no recipe covers, from the whole generated card put in the form a person would
leave it (`src/cantrip/pr_form.rs`: shared triggers, mana, targets and effects, names and quotes from the printed text,
characteristics from Oracle). What it can't write whole is a gap, recorded in the row's `gap`. A card with no gap needs no
mechanic and no engine work, so its code and content are final, and a worker does two things: validate them against the
Oracle text, and write the scenarios. To widen the lane, teach the generator, not more to the worker's job.
`backend_capable` in `ast check-cantrip` is a different question: that scan inlines whatever a card needs, so it says the
card compiles, not that it needs nothing new.

A row's file is `ast cantrip-tracked`'s, byte for byte, for every layout, and the row says the generator's `layout` and its
`faces`, front first. The queue takes the Scryfall layouts in `LAYOUTS`: `normal`, the single-faced `saga`, `class`,
`leveler`, `prototype`, `case` and `mutate`, and the cards of several faces, `transform`, `modal_dfc`, `adventure` (an Omen
too), `split` (a Room too), `prepare` and `meld`. Each was checked by swapping the generated cards for all Cantrip has of
that layout and playing its scenarios. Not tokens, flip cards, planes or art cards. A card is queued by Cantrip's name for
it, both halves for a split card or a Room (`Claim // Fame`) and the front face otherwise (`Rona, Herald of Invasion`), and
a format's list finds it by that, by the generator's name or by any face. Each face's Oracle text is compared with
Scryfall's. `new-card.py` writes the content of most layouts; for `split`, `prepare` and `meld`, which it leaves to a
person, `card_lane.py draft NAME` writes it with `new-card.py`'s own writer of a face, as Cantrip's own cards of those
layouts have it. Every face's lines are in the report, the placeholders and the survivors, and a card's section says how
a scenario reaches its other face, with a scenario of Cantrip's that does.

The way of working is the `card-lane` skill, `.claude/skills/card-lane/SKILL.md` in this repository: plain commands and one
set of instructions, so any model, or a person, can do a card. This file covers setting the lane up and running it as an
unattended loop.

## Implementation → review/fix pipeline

`review_lane.py` is a separate consumer of implementation queues. Implementation workers still use this lane's
read-only generated code and scenario gates. Review workers use Terra with the full Oracle/rulings/CR packet,
current code and shared definitions, author validation, scenarios and validation records. They may fix card code,
scenarios and shared mechanics. Engine changes stop with a concrete case. Generator improvements are recorded
in the review report so a manual card correction does not silently become the generator's new contract.

Implementation workers use `implementation_lane.py` and atomically claim **one ready card** at a time from the
shared `implementation_queue`. Their context can span 20 cards, but each card is committed and handed to review
immediately. No implementation worker reserves the next 19 cards while another slot is idle. Terminal outcomes
and held generator gaps are never retried merely because a worker or importer restarts.

Reviewers claim **one committed revision** at a time from a SQLite queue; importing a producer queue twice does
not duplicate work. Each reviewer owns a prepared CoW worktree and keeps its build cache. Each card starts on
its own branch from the configured base plus its producer's prerequisite commits. Original producer branches and
publication stacks stay untouched. The final candidate has shared prerequisites followed by one `Card:` commit,
with the original card author and issue reference preserved. These workers never publish or merge.
Code changes receive confirmation from a separate Terra verifier; that confirmation checks the fixes, rather than
paying for another complete original review. Allowed partial fixes on a blocked card are retained in an explicit
unpublished `Review WIP:` commit on that card's own branch, freeing the slot for another card. Unexpected edits
stop the slot for inspection. Leases are never silently expired and reassigned.

The current campaign configuration is `~/card-lane/pipeline-unlimited-20261103/config.json`:

```sh
R=rust_syntax/normalize/tools/card_lane/review_lane.py
python3 "$R" --root ~/card-lane/pipeline-unlimited-20261103 sync
python3 "$R" --root ~/card-lane/pipeline-unlimited-20261103 worker r1
python3 "$R" --root ~/card-lane/pipeline-unlimited-20261103 watch
python3 "$R" --root ~/card-lane/pipeline-unlimited-20261103 status
~/card-lane/status.sh
python3 rust_syntax/normalize/tools/card_lane/implementation_lane.py --root ~/card-lane/pipeline-unlimited-20261103 worker i1
```

The watcher continuously imports committed producer cards, including future workers in the manifest. Worker slots
are independent services. `enabled: false` drains current work and prevents the next claim. `reviewers` maps slot
names to prepared worktrees; `producer_manifests` lists the existing implementation manifests. `review_model` is
explicitly `gpt-6.1-terra`; `cards_per_context` defaults to **20**, independently of implementation batch size.
`implementation_cards_per_context` also defaults to 20. `implementers` maps implementation slot names to
prepared trees. The previous batch dispatcher is disabled once this consumer owns the shared queue.

`reviewers.<slot>.role` selects `review`, `confirm`, or `all`. The current pool has four shared review/fix
workers and one verifier. Verification findings return the card to fix work with immutable per-round artifacts.
`max_verifier_rejections` defaults to **3**: the third rejection leaves the card blocked. There is no fourth
automatic cycle. Cards without code changes skip confirmation. Full machine gates run after the review worker
returns its report; only failures resume the model. Source must remain unchanged throughout those checks.

After a specific blocker has changed, the coordinator can explicitly return preserved review work to the pool:
`review_lane.py --root ROOT retry JOB_ID --reason "What changed"`. This keeps the WIP commit and original report,
uses a new artifact round, and records the retry reason. It cannot take an active lease, retry confirmation work,
or bypass the verifier rejection cap. Artifact rounds and actual verifier rejection counts are independent.
Existing client prompts, actions and state follow `add-cards/SKILL.md` step 8: caption corrections using supported
choice kinds need engine metadata and presenter inspection, not a new browser test solely because wording changed.
Actual new client capabilities still require browser evidence.

### Deterministic intake and publication

`intake_lane.py` selects and reserves canonical cards before enqueueing them. It accepts repeated `--card`,
a newline/JSON list with `--file`, a complete `--set`, and `--set ... --color blue` (or W/U/B/R/G/C).
Local Oracle and set data are preferred; `add` fetches and caches complete printings for an uncached set, while
`plan` requires `--fetch-set` for that read. Ante, dexterity and non-card objects are excluded; flavor bans are
not exclusions. Existing queues and other workers' live claims are preserved. Claims and enqueueing have a journal.
Only generator-ready candidates are reserved by default. Gaps and missing generation are catalogued in the journal
as `deferred_unclaimed`; `--reserve-gaps` is an explicit opt-in. Slow reservation calls do not hold the shared queue
lock, and implementation outcomes are mirrored back into intake, including recovery after interrupted writes.

```sh
P=~/card-lane/pipeline-unlimited-20261103
T=rust_syntax/normalize/tools/card_lane
python3 "$T/intake_lane.py" --root "$P" plan --set 2ED --color blue
python3 "$T/intake_lane.py" --root "$P" add --card "Card Name"
python3 "$T/intake_lane.py" --root "$P" add --file cards.txt
python3 "$T/publish_lane.py" --root "$P" plan
python3 "$T/publish_lane.py" --root "$P" watch
```

Publication is explicitly enabled by `publisher.enabled`. The deterministic publisher freezes groups of **20
approved cards**, ordered by issue/name, with prerequisites before card commits and duplicate prerequisites
removed. It validates the integrated branch before pushing to the configured fork and opening one upstream PR.
Membership is reserved atomically and PR creation is idempotent. Conflicts or failed checks retain the batch for
inspection. `flush_final` allows the final smaller batch once all work is terminal. Nothing merges upstream.

`shared_base_manifest` optionally names an immutable validated helper pin: upstream `base`, full `head`,
the exact linear list of `Engine:`/`Mechanics:` commits, its helper-only `pr_url`, and matching successful
`validation` evidence. Promote it only after the helper branch passes integration checks. Card commits cannot
enter this pin. New implementation jobs branch from it and pass that frozen base to review. Legacy candidates
keep their existing review base; `review_use_shared_base` explicitly opts them into migration after prerequisites
are reconciled. The review base remains separate from producer history. Already-running jobs, retries and confirmations retain their original revision. Publication
freezes each card's base and helper dependencies, includes the prerequisite commits while upstream is pending,
and links the helper PRs. Thus independent card batches can share helpers without depending on earlier card
batches. Overlapping helper changes still stop for a semantic resolution rather than being deduplicated by name.

### Context and measurements

Both stages measure latest-request context from Codex rollouts, separately from cumulative token usage. They
restart at a card boundary at 300,000 measured tokens or 20 cards, whichever comes first. Implementation streams
accept `--context-budget` and `--context-cards`; reviewers use `context_tokens` and `cards_per_context` in config.
Zero removes the card cap; implementations retain a 20-card fallback when context measurement is unavailable.

A narrowly trusted `PreCompact` hook returns `continue: false` before automatic compaction. A stopped worker
restarts in fresh context on the **same unfinished card**, keeping its files and packet. Three repeated stops on
one card become an infrastructure stop, rather than an endless loop or a false card failure. This path was tested
with a live low-threshold Codex run, not just an assumed configuration setting. See the
[official hook contract](https://learn.chatgpt.com/docs/hooks#precompact).

Per-card artifacts live in `jobs/<revision-id>/<review-or-confirm>/`: full packet, worker stream, review report,
gate logs, exact fix diff and result. Queue events retain each stage's elapsed time, token usage, context size and
rotations; `status` reports review/confirmation counts, timing and cost estimates. Its `costs.cards` map
sums implementation, review/fix and verifier usage, including rejected rounds. Original batch implementation
costs are apportioned; newer one-card jobs use per-card token deltas. Results copied into sources or replayed
during deterministic gate recovery count once. The top average uses cards that finished review/verification
and have priced usage for both implementation and review; total cost includes blocked and unfinished cards'
recorded attempts. In-flight usage is added when its result arrives. These are standard API-equivalent
estimates, not subscription charges, and exclude tier, long-context, cache-write and regional adjustments.
The dashboard shows the average and per-card costs; `~/card-lane/status.sh --costs` lists every recorded card
with its stage breakdown, and `--json` includes the underlying accounting. Pricing lives in `usage_costs.py`.
`status.phase_rates` measures implementation, first review, repeat fix, verification and the combined
review/fix pool over the same rolling 30-minute wall-clock window. Throughput counts unique cards that
advanced successfully in that phase; blocked outcomes count toward attempts and time, not throughput.
The combined pool deduplicates cards that appear in both review and fix. Historical branch names preserve
the original phase after a job moves on. Replayed deterministic-gate results count once. Mean elapsed
time, queued cards and occupied worker time are included in JSON; occupied time includes in-progress
work and unions overlapping intervals per slot. These rates use the new one-card implementation ledger,
so the old batch of imported implementations does not inflate current throughput. The top `READY RATE`
retains the lifetime completed-card rate; `PHASE RATES (30m)` shows recent throughput for staffing decisions.
`resource_monitor.py --root ROOT watch` samples systemd worker cgroup CPU counters every two seconds,
including compiler/test descendants that can disappear between process-list snapshots. It records daily
`resources/samples-YYYY-MM-DD.jsonl` and a rolling ten-minute `resources/status.json`: time-weighted mean,
p95 and peak CPU by role, host load, memory, pressure, swap activity and throttling. CPU units are logical
CPUs occupied, not percentages or requested Cargo jobs. `--unit-prefix` selects the service prefix and
`report` reads the current summary. The dashboard displays role and host averages. A 5 implementer /
7 reviewer / 1 verifier projection scales observed per-worker CPU, holding unrelated host work fixed;
it is an estimate, and the samples after expansion measure actual load separately.

Review validation runs edited cards before unchanged shared callers. A failing edited-card gate returns
for repair before the wide regression sweep or that card's mutation run. Scenario-only caller repairs
still receive corpus, coverage and citation checks; full mutations apply to the current card and other
cards whose Rust code changed. `checks-live.json` exposes the current gate and card count, so the
dashboard marks a long shared-caller sweep as REGRESSION instead of model review.
Reviewers may add Rust card or mechanics integration tests and existing local workspace dev-dependencies;
runtime dependencies and external dependency changes remain outside card review scope. The driver runs
new integration tests (requiring a nonzero pass count) and edited cards' inline Rust tests. Integration
tests do not automatically satisfy the scenario mutation gate; unresolved mutants still block the card.
An infrastructure stop retains completed
review evidence and usage so an explicit resume can reuse an unchanged report and source.
Drivers reload changed pipeline code at a clean card boundary, after finishing their lease, so later jobs
receive scope and helper-pin updates without interrupting active cards or reusing an old resume argument.
An explicit `retry JOB --reason ...` can also restart a stopped confirmation that produced no report.
It retains the reviewed candidate and review evidence, writes a separate confirmation-attempt directory,
and preserves the rejection count. A real confirmation report must follow the normal finding/fix path;
neither retry form bypasses the rejection cap.
Focused builds resolve an existing legacy module filename when its non-ASCII spelling differs from
the current filename convention. Publisher counter-only prerequisite conflicts can merge plain appended
vocabulary entries, retaining existing IDs and assigning new contiguous IDs; other conflicts remain blocked.
First-card cache warmup is part of elapsed time;
worker wall time includes its tool waits and must not be described as pure inference time. Machine validation
time is recorded separately and can overlap that wall time. No pricing estimate is treated as an actual bill.

Focused checks always build the current source and run affected cards' corpus, Oracle comparison, lint, citations,
coverage, mutation checks and the layer contract when needed. Unresolved survivors block. Equivalence claims and
manual meaning breaks stay visible to the reviewer/confirmation context. Confirmation cannot edit source or
replace missing machine evidence with a self-reported pass.

The current card and edited card files/scenario owners receive the full gate set. Existing callers affected only
by a shared helper change still run their scenarios and coverage; their unchanged card decisions are not mutated
again, and their unchanged Oracle/lint/citation checks are not repeated. Changed shared files receive citation
checks, and the review/verifier must inspect the helper change and its specific meaning probes. This prevents a
small helper correction from turning into mutation review of hundreds of unrelated card implementations.

Both stages enable `CARD_LANE_FOCUSED=1`: builds include all scenario deck dependencies, transitive card-file
dependencies and providers of shared extra definitions. The selector is refreshed after scenario changes. The
layer contract uses the full registry, and an incomplete focused baseline cannot count as a caught mutation.
In the first fresh-tree probe, corpus and coverage fell from 64/68 seconds to 31/30 seconds; subsequent runs
reuse each worker's cache. Timings are recorded rather than assumed to improve every card by the same amount.

## Set up

```sh
cd <mtgish checkout>
L=rust_syntax/normalize/tools/card_lane/card_lane.py
python3 $L setup --checkout ~/cantrip   # worktree, ast, corpus, coverage shim, Scryfall; about a minute warm
python3 $L queue --format pioneer       # every card with no gap, that format's list first
```

`cargo-llvm-cov` must be installed (`cargo install cargo-llvm-cov --locked --root ~/card-lane`). `llvm-cov` and
`llvm-profdata` come from `PATH`; `setup` writes a small `rustc` shim so an unmodified `card-coverage.py` finds them.

Everything lives under `~/card-lane`. Move it with `CARD_LANE`; `CARD_LANE_BRANCH` names the Cantrip branch.

| Path | Holds |
| --- | --- |
| `cantrip/` | The Cantrip worktree, on a new local branch |
| `targets/` | Cargo target directories. Nothing goes to `/tmp` |
| `gen/` | `rows.jsonl`, the generator's card rows, each with its file or its gap |
| `scryfall/` | Bulk files and index. `tool` points `scryfall.py` here, so the main checkout's `target/` stays untouched |
| `state/queue.json` | The queue. Written atomically after every step |
| `state/cards/<stem>/` | The packet: `reference.md`, `shared.md`, `prior-art.md`, `content.json`, `prompt.md`, `report.json`, `check.json`, `result.json`, `worker.jsonl`, `work.patch` |

## Doing cards in a session

Any agent that can run a shell, or a person, can follow the skill directly:

```sh
python3 $L next                                # opens the next card and prints its brief
python3 $L check "Bishop of Wings" --fast      # as often as you like
python3 $L commit "Bishop of Wings"            # every gate, then one commit
```

In Claude Code the skill is found in this repository. A subagent needs one line: "Follow the card-lane skill for the next
card", with `model: sonnet` or whichever you want. Another tool can be pointed at the path of `SKILL.md`.

## Running a loop

`run` starts one worker per card with no human in the loop. It spawns a model through a backend, then runs the same
`next`, `check` and `commit` steps.

```sh
python3 $L run --backend codex --model gpt-6.1-terra --effort medium --limit 10
python3 $L status
```

| Backend | Options | Sandbox |
| --- | --- | --- |
| `codex` | `--model`, `--effort` | Yes. No network; writes only to the tree, the target directory and the packet |
| `claude` | `--model`, `--effort` | No. Writes are limited by its tool list |
| `command` | `--command "my-agent --stdin"` | Yours. Reads the prompt on stdin; `{session}` is replaced on a repair round |

To add a backend, write one class in `card_lane.py` with `command(card, session)` and `read(line, usage)`, then add its
prices to `RATES` if it reports none.

| Option | Default | Meaning |
| --- | --- | --- |
| `--limit` | 10 | Cards in this run |
| `--timeout` | 1800 | Seconds a card may take, worker and repairs together |
| `--repairs` | 2 | Rounds a worker is sent back with failed gates, in the same session |
| `--attempts` | 2 | Runs a card may take across restarts |
| `--stop-after` | 3 | Stop if this many first cards all fail |
| `--only` | | Only these cards |

Two infrastructure failures in a row always stop a run. A killed run restarts from the queue: a card left `working` keeps
its work as `work.patch` and goes back to pending.

Outcomes are `committed`, `generator_bug`, `failed_gates`, `engine_gap`, `needs_review`, `timeout`, `infrastructure` and
`error`. A card that did not commit keeps its work in `work.patch`, and the tree is cleaned for the next card.

A model-capacity error pauses for 30, 60, then 60 seconds and resumes the same session and worktree, with at most three
retries inside the original timeout. These pauses do not consume repair rounds. If capacity remains unavailable, a stream's
unfinished cards are recorded as `infrastructure`; existing card verdicts and completed cards keep their own results.

`generator_bug` is the worker's finding that the card is wrong, or the driver's that the generated file cannot stand, which
it decides before starting a worker: style problems, a function that repeats a shared effect, or a file that does not
compile. The fix goes in the generator; `status --json` has each card's reason.

Cost is token counts as the backend reports them, and for Codex that is the session's total, which a repair round grows
rather than restarts. Where a backend reports no price, `usd_api_equivalent` applies the list rates in
`RATES`. That is an API-equivalent figure, not what a ChatGPT login is billed.

## Running a batch

`batch-run` gives one worker several cards in one session. The prompt holds what the worker needs, so it starts by writing,
not reading: `handbook.md`, the Oracle text, each card's code and test points, and how scenarios say its keywords. A driver
then plays everything at once and sends back what failed, for every card together. A card that passes is committed alone
and leaves the batch.

```sh
python3 $L batch-run --size 10 --limit 20 --way work --backend codex --model gpt-6.1-terra --effort medium
python3 $L batch-check --fast
```

Which worker: the same 20 cards, two batches of ten, `--way work`, on 2 November 2026.

| Worker | Landed | Minutes | API-equivalent $ | $ a card landed |
| --- | --- | --- | --- | --- |
| `gpt-6.1-terra`, medium | 19 | 42 | 1.88 | 0.10 |
| `gpt-6-luna`, max | 15 | 139 | 1.27 | 0.08 |

Terra lands more and takes a third of the time for about the same price a card, so it is the default. Luna finished each batch
in one round and set aside what it could not settle; Terra used its repair rounds. The card Terra left (Goblin Shrine) is a
closing brace no game runs, a generator matter.

| Option | Default | Meaning |
| --- | --- | --- |
| `--size` | 10 | Cards in one session |
| `--way` | `work` | `draft`: the worker writes everything blind, the driver plays it and reports, and each repair is run until green with the tools open. `work`: the worker has the tools from the start and runs until the whole `batch-check` passes |
| `--repairs` | 4 | Rounds a batch is sent back with failed gates |
| `--timeout` | 3600 | Seconds a batch may take, every round together. At maximum effort ten cards can use all of it |
| `--only-file` | | Cards to do, one name per line |

Every step of a worker resends the session, so cost follows steps, not bytes read. One build serves the whole batch, and every
card's coverage comes from one instrumented run. Mutation runs for every card its scenarios cover, in the same pass as the report
gate: when it waited for a clean report, a card spent its rounds on a test point and heard about its survivors with one round left.

Mutation would be the dearest gate. In the tree one mutant costs a rebuild of the whole card crate, 38 seconds with thousands
of cards in it, and the tree must be built back after. So it runs in a shadow under `$TARGET/mutants`: links to the real tree,
with a `cantrip-cards` of only the cards these scenarios play, copied in. A build takes about 3 seconds, the first at a path
about a minute, and the tree is never touched. A card whose scenarios do not all run and pass in the shadow goes to the tree
instead, so a smaller card table cannot turn into a pass. On seven cards and 73 mutants the two gave the same verdict every time.

A keyword is taken off in the card's own file rather than its content, since content would rebuild the oracle crate and
everything above it. A card with no file gets one of that single field for the change.

Verdicts are kept in the card's `mutants.json`, by the mutant's text. A change caught stays caught while the scenario that
caught it is unchanged, a survivor is played again when any of the card's scenarios changes, and a change that does not compile
is not tried again.

A repair prompt must tell the worker to keep going until the cards pass. A worker told to fix what it can in one action does
exactly that, whatever the failures, and a lower effort follows such wording more literally. Its finish line is the whole
`batch-check`: told `--fast`, a worker stops with mutants it has never seen.

Test points come from `new-card.py`, by rule of thumb on the card's words, and some ask what no game can show, as the order of a
search and the shuffle after it. A point or a ruling may be answered with a sentence saying why, at least 20 characters. A mutant
no game can reach or see goes in `equivalent_mutants` the same way: reading the code is evidence enough.

Each batch appends a record to `$STATE/batches.jsonl`: cards, outcome by card, rounds, seconds by phase, tokens and price.

Parallel runs each need a tree. Make them as copy-on-write snapshots of a warm base on the btrfs volume rather than as
worktrees with a copied target: a snapshot takes about a second and about 300 MB after a rebuild. Point the lane at one with
`CARD_LANE_CANTRIP`, `CARD_LANE_TARGET`, `CARD_LANE_STATE` and `CARD_LANE_BRANCH`.

```sh
COW_WORKTREE_CONFIG=/mnt/vizier/cow-cantrip.conf ~/.local/bin/cow-git -C /mnt/vizier/base/cantrip \
    worktree add -q -b card-lane/NAME /mnt/vizier/card-lane/NAME card-lane/pioneer-pilot
```

## Counters a card brings

A counter that is only a name needs one line in `counter_kinds!` (CR 122.1). The generator writes such a card as it will stand, and `prepare` adds the line to the worktree so it builds. `commit_card` then makes a commit of its own for the line, `Engine: the spore counter (CR 122.1), for <Card>`, just before the first card that uses it, as Cantrip's own are made. The number comes from `HEAD`, since a card set aside leaves its line in the worktree and the numbers index an array. Counters on players and keyword counters need more than a line, so the generator holds them back (`MORE_THAN_A_NAME` in `planned.rs`).

## The gates

`check` runs Cantrip's own tools unmodified, with `CANTRIP_VERIFY=1`, cheapest first. `check --fast` leaves out `mutation`
and `layers`, which take minutes. A commit needs all of them.

| Gate | Fails when |
| --- | --- |
| `scope` | Anything changed but the card's content file, its card file, new scenarios and the lines of counters it brings |
| `generated` | The card's file is not the generator's, or its content file is not what `new-card.py` wrote |
| `oracle` | `new-card.py` says the content differs from Scryfall's current card |
| `scenarios` | A placeholder or TODO remains, a scenario says `Known bug`, has no opening comment or no `[Expect]`, or a card with a cost has no `.Mana =` expectation |
| `style` | Ability text not printed on the card, generator scaffolding left in, or a `fn` without a doc comment quoting the Oracle and citing the CR |
| `lint` | `card-lint.py` reports a NEVER |
| `citations` | `cr.py check` finds a rule that does not exist |
| `corpus` | One of the card's scenarios fails |
| `warnings` | The build warns about the card's file |
| `coverage` | `card-coverage.py` finds a line never run |
| `liveness` | One scenario, played alone, runs none of the card's lines |
| `mutation` | `card-mutate.py` could not finish, the card has more than 40 possible changes, or a keyword taken off the card's content is not noticed |
| `layers` | With a static ability, `layer_pass_cases` fails |
| `report` | A printed line has no validation, a test point or ruling is unpaired, or a LOOK, a surviving mutant or a function copied from another card is unanswered |

`liveness` exists because `card-coverage.py` judges a card's scenarios together, so one that plays nothing of the card hides
among them. A card whose file names only a shared function has no line of its own, so `liveness` asks only that a scenario
plays it, and `mutation` shows the scenarios reach it. A card with no file is tested by its keywords alone: `mutation` takes each off in turn.

No gate reads the rules. The skill's last section says what that leaves to a review.

## Tests

```sh
python3 rust_syntax/normalize/tools/card_lane/test_card_lane.py
```

No Cantrip tree is needed.
