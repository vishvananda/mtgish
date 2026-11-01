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

The way of working is the `card-lane` skill, `.claude/skills/card-lane/SKILL.md` in this repository: plain commands and one
set of instructions, so any model, or a person, can do a card. This file covers setting the lane up and running it as an
unattended loop.

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
python3 $L run --backend codex --model gpt-6-luna --effort max --limit 10
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

`generator_bug` is the worker's finding that the card is wrong, or the driver's that the generated file cannot stand, which
it decides before starting a worker: style problems, a function that repeats a shared effect, or a file that does not
compile. The fix goes in the generator; `status --json` has each card's reason.

Cost is token counts as the backend reports them, and for Codex that is the session's total, which a repair round grows
rather than restarts. Where a backend reports no price, `usd_api_equivalent` applies the list rates in
`RATES`. That is an API-equivalent figure, not what a ChatGPT login is billed.

## The gates

`check` runs Cantrip's own tools unmodified, with `CANTRIP_VERIFY=1`, cheapest first. `check --fast` leaves out `mutation`
and `layers`, which take minutes. A commit needs all of them.

| Gate | Fails when |
| --- | --- |
| `scope` | Anything changed but the card's content file, its card file and new scenarios |
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
| `mutation` | `card-mutate.py` could not finish, or the card has more than 40 possible changes. A card with no file has none |
| `layers` | With a static ability, `layer_pass_cases` fails |
| `report` | A printed line has no validation, a test point or ruling is unpaired, or a LOOK, a surviving mutant or a function copied from another card is unanswered |

`liveness` exists because `card-coverage.py` judges a card's scenarios together, so one that plays nothing of the card hides
among them. A card whose file names only a shared function has no line of its own, so `liveness` asks only that a scenario
plays it, and `mutation` shows the scenarios reach it. A card with no file has neither, so its scenarios stand on their own.

No gate reads the rules. The skill's last section says what that leaves to a review.

## Tests

```sh
python3 rust_syntax/normalize/tools/card_lane/test_card_lane.py
```

No Cantrip tree is needed.
