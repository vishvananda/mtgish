---
name: card-lane
description: Turn generated Cantrip cards into card commits, in this session, one card at a time - take the next card from the queue, make the generator's draft exact and in the project's style, write real scenarios, pass the gates, commit, take the next. Use when asked to work through cards the mtgish generator can emit, whoever is doing the writing. It needs no separate model or driver; a driven worker follows this same file.
---

# The card lane

The mtgish generator writes most Magic cards as Cantrip code that compiles and plays. Cantrip will not take that code as
it stands: a card is exact to its Oracle text, built from the shared mechanics, and proved by scenarios that fail when it
is wrong. This lane closes that gap one card at a time. A tool writes the card's code and placeholder scenarios, you make
them right, the gates judge, and a passing card becomes one `Card:` commit on a local Cantrip branch.

Everything you use is a plain command, so this works the same from any agent or by hand. `card_lane.py` means
`python3 rust_syntax/normalize/tools/card_lane/card_lane.py` from the mtgish root, or the absolute path a brief prints.
Each command sets up its own build environment. Run them from anywhere.

| Command | What it does |
| --- | --- |
| `card_lane.py next` | Takes the next card, writes its code, placeholders and packet, and prints its brief. Prints the same brief again if a card is already open |
| `card_lane.py corpus --card "Name" --brief` | Plays the card's scenarios. Also `--next`, `--dump`, `--diff N`, `--file PATH` |
| `card_lane.py check "Name" --fast` | The gates that take seconds. Run it as often as you like |
| `card_lane.py check "Name"` | Every gate, including breaking the card. Takes minutes, so run it when the fast gates pass |
| `card_lane.py commit "Name"` | Every gate, then exactly one commit, or no commit |
| `card_lane.py abandon "Name" --outcome engine_gap --reason "..."` | Keeps the work as a patch, cleans the tree, records why |
| `card_lane.py tool cr.py check --show FILE` | Any tool in Cantrip's `tools/maintenance`, in the lane's environment |
| `card_lane.py status` | The queue, and what each card cost |

No queue yet? Run `card_lane.py setup`, then `card_lane.py queue --format pioneer`. See `README.md` beside `card_lane.py`.

## The loop

Run `next`, do the card as below, run `commit`, and run `next` again for as many cards as you were asked for. Each card
is its own commit and starts from a clean tree. A gate that fails on every card is the lane's bug: stop and look.

## Doing one card

The brief names the Cantrip tree, which is your working directory, and the card's packet, which holds:

- `reference.md`: Oracle text, official rulings, CR rules, prior art, test points. Read this first.
- `draft.rs`: the generator's output, unedited. It is one reading of the card's text, not the answer.
- `first-build.txt`: what the code and placeholders did on their first run.

Read these once, before you write anything, and follow them. They are the project's rules, not suggestions.

- `.claude/skills/add-cards/SKILL.md`: "The non-negotiables", "Where code goes" with its never list, the loop's steps
  4 to 7, and "Scenarios: what a card owes".
- `.claude/skills/write-game-script/SKILL.md`, and `scenarios/README.md` for syntax and dump keys when you need them.
- `docs/adding-cards.md` for a card file's fields.

The card must be exact to its Oracle text, rulings and the Comprehensive Rules. A card that is wrong is worse than a card
that is missing.

1. **Read the card's words first.** Work out from the Oracle text, rulings and rules what must happen, before you read
   the draft. Write expectations from the rules, never from what the engine printed.
2. **Rewrite the card's file in the project's style.** The file compiles but no reviewer would pass it: the generator
   inlines what `cantrip-mechanics` and `cantrip-api` already do. Replace it:
   - use the shared recipe wherever one exists, starting from the prior art in `reference.md`, whose card files you
     should read;
   - name functions for what they do, and give each a doc comment that quotes the Oracle sentence it implements and
     cites the CR (`/// "Whenever ... you gain 4 life." (CR 603.2).`);
   - use the printed Oracle sentence as each ability's text;
   - no `#[allow]`, no `.expect`, no clamp, no `ability_N` names, no token defined by hand when `tokens::` has it.

   If the draft and your reading of the rules disagree, the rules win. Note the disagreement in your report: it may be a
   generator bug.
3. **Write the scenarios.** Replace every placeholder with scenarios named for what they prove, each opening with a `#`
   comment saying what it shows and the rule. Together they run every line and branch of the card, and each one must
   execute the card's own code. Give every test point an expectation that fails if the card gets it wrong: the exact
   amount, both players' objects for "each", "target" or "you control", an earlier turn for "this turn".
4. **Check.** Run `check --fast` until it passes, then read each citation that `tool cr.py check --show <card file>`
   prints against the rule beside it: a rule that does not say what your comment claims is fixed. Do not run the whole
   corpus.
5. **Write your report** to `report.json` in the packet. `check` writes `test-points.json` there, listing the test points
   by id.

   ```json
   {
     "decision": "done",
     "reason": "",
     "test_points": [{"id": 1, "scenario": "file-name.cantrip", "line": 31}],
     "look_answers": ["line 12: why this LOOK from card-lint is right as written"],
     "equivalent_mutants": [{"mutant": "line 6: 4 to 5", "why": "only if it truly changes nothing the rules can see"}],
     "generator_disagreements": [],
     "notes": "what cost you time"
   }
   ```

   `line` is the 1-based line of the expectation in that scenario: a `?`, a `!` or a line under `[Expect]`. Each entry of
   `look_answers` names the LOOK's line as `line 12`, or the other card's file for a repeated function.
   `equivalent_mutants` starts empty. Fill it only for a mutant the full gate reports that means the same thing.
6. **Finish.** Run `commit`. If it fails, fix what it names and run it again. If the brief says a driver runs the gates,
   stop after the report instead: it runs them and sends back what fails, in this same session.

## Limits

- Change only the card's file, its content file and new files under `scenarios/`. Add a core keyword to the content file
  only if the rules text requires it. Edit nothing else: not `cantrip-mechanics`, not the core, not the client.
- If the card cannot be exact without a change anywhere else, stop. A faked value, a stand-in that only behaves alike or a
  flag the rules should decide are not options. Set `"decision": "engine_gap"` with the rule and what is missing in
  `reason`. Use `"decision": "needs_review"` if a tool is broken or the tree is not what the brief describes. Write the
  report, then run `abandon` with that outcome. Do this early. These outcomes are normal and cost the project little; a
  false pass costs it a lot.
- Never weaken a scenario to pass. Never assert what you know is wrong. No `# Known bug` scenarios in this lane: report an
  engine bug as `needs_review`.
- If a function body in your file repeats another card's, the project wants it shared. Say so in `look_answers`, naming
  the other file. Do not edit it.
- No `git commit`, no `git add`, no `git push`, no `gh`, and no network. `commit` makes the commit. Do not write
  `Co-Authored-By` anywhere.
- Never `--release`. Work in the foreground, one command at a time. Waiting costs context, so wait for a command to
  return rather than polling it, and never start a second copy of a running build.

## What the gates do not do

`check` runs Cantrip's lint, citation, coverage and mutation tools and checks the shape of the work. No gate reads the
rules. Nothing here compares the Oracle text to the code, checks that a citation's rule says what its comment claims,
decides whether a mechanic should be shared, or looks at the client. A passing card has earned a review. It has not passed
one, and Cantrip's own review (`review.md`) still has to run before it is proposed. Commits stay on the lane's local branch.
Proposing them upstream is a person's decision.
