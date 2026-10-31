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
| `card_lane.py check "Name" --fast` | The gates that take seconds. Prints what failed |
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

**Done means `check --fast` passes and `report.json` says `done`, with the card exact to its text.** Most of a card's cost
is thinking and context, not builds, so spend them on the card's rules and not on learning the project. A card is usually a
few sentences over mechanics that already exist.

The brief names the Cantrip tree, which is your working directory, and the card's packet, which holds:

- `reference.md`: Oracle text, official rulings, CR rules, prior art, test points.
- `prior-art.md`: the nearest cards' files and their shortest scenarios. Copy their shape.
- `draft.rs`: the generator's output, unedited. It is one reading of the card's text, not the answer.
- `first-build.txt`: what the code and placeholders did on their first run.
- `report.json`: your report, its test points already listed.

Read those, then these two, once. They are the project's rules, not suggestions:

- `.claude/skills/add-cards/SKILL.md`: the non-negotiables, the never list, and "Scenarios: what a card owes".
- `.claude/skills/write-game-script/SKILL.md`: how scenarios play, and its gotchas.

**Read nothing else whole.** Keep what you read small, because everything you read is paid for again on every later step:

- `docs/adding-cards.md` and `scenarios/README.md` are reference: `rg -n "word" FILE | head -20`, then read those lines.
- A rule is `tool cr.py 603.6a`. Never open the rules file itself.
- Bound every search with `| head -40` and aim it at a file you can name. Do not read `crates/cantrip-script`: `corpus --next
  --brief`, `--dump` and `--diff N` tell you what a scenario can see.

The card must be exact to its Oracle text, rulings and the Comprehensive Rules. A card that is wrong is worse than a card
that is missing.

1. **Read the card's words first.** Work out from the Oracle text, rulings and rules what must happen, before you read
   the draft. Write expectations from the rules, never from what the engine printed.
2. **Rewrite the card's file in the project's style.** The file compiles but no reviewer would pass it: the generator
   inlines what `cantrip-mechanics` and `cantrip-api` already do. Replace it:
   - use the shared recipe wherever one exists, starting from `prior-art.md`;
   - name functions for what they do, and give each a doc comment that quotes the Oracle sentence it implements and
     cites the CR (`/// "Whenever ... you gain 4 life." (CR 603.2).`);
   - use the printed Oracle sentence as each ability's text;
   - no `#[allow]`, no `.expect`, no clamp, no `ability_N` names, no token defined by hand when `tokens::` has it.

   If the draft and your reading of the rules disagree, the rules win. Note the disagreement in your report: it may be a
   generator bug.
3. **Write the scenarios.** Write one, play it, and read `corpus --file PATH --dump` for its keys before you write the rest.
   Replace every placeholder with scenarios named for what they prove, each opening with a `#` comment saying what it shows
   and the rule. Together they run every line and branch of the card, and each one must execute the card's own code. Give
   every test point an expectation that fails if the card gets it wrong: the exact amount, both players' objects for
   "each", "target" or "you control", an earlier turn for "this turn".

   The gate breaks numbers and comparisons, not meaning, so ask the card these yourself. An independent review blocked
   pilot cards on each of them:
   - **Every ruling is specification.** Each gets an expectation, unless the card's own code decides nothing there. A ruling
     with two halves gets both.
   - **Order.** If swapping two of your statements leaves the end state the same, add a `?` at the point between them,
     usually at the question the second one asks. A sequence the text states must be able to fail.
   - **Last-known information.** If you read `old` rather than `new`, or a value that can change, use something whose answer
     differs on the battlefield and in the graveyard. A card that is an Angel in both proves nothing: an animated land does.
   - **Read through words.** A scenario does not have to show a text change, but say so in `notes`.

   Mistakes that cost earlier cards a run:
   - Bot1 plays odd turns and Bot2 even ones: a wait for Bot1 is `[T3 Bot1.Main1]`, never `[T2 Bot1.Main1]`.
   - Counts belong to a zone: `Bot1.Battlefield.Count`. There is no `<Name>.Count`. Same-named objects are `Name#1`.
   - Mana empties between steps, and a `?` reads at the next question, so a spell is usually still on the stack.
4. **Check.** Run `check --fast` until it passes. Then run `tool cr.py check --show <card file> <its scenarios>` and read
   each citation against the rule printed beside it: a rule that does not say what your comment claims is fixed. Do this
   for scenario comments as well. A number copied from a shared function's comment or another card is how a wrong one gets
   in (CR 119.1 is starting life totals; gaining life is 119.3). Do not run the whole corpus.
5. **Fill in `report.json`.** Set `decision` to `done`. Give each test point its `scenario` and `line`, the 1-based line of
   the expectation that catches it: a `?`, a `!` or a line under `[Expect]`. Do the same for each ruling, or say in
   `no_decision` why the card's own code decides nothing there. Each entry of `look_answers` names the LOOK's
   line as `line 12`, or the other card's file for a repeated function. `equivalent_mutants` stays empty unless the full gate
   reports a mutant that means the same thing: then add `{"mutant": "line 6: 4 to 5", "why": "..."}`, and only if it truly
   changes nothing the rules can see.
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
- If a function body in your file repeats another card's, or a trigger is a shared matcher plus one condition another card
  also adds, the project wants it shared, and a review will send the card back. This lane cannot share it. Say so in
  `look_answers`, naming the other file. Do not edit it.
- No `git commit`, no `git add`, no `git push`, no `gh`, and no network. `commit` makes the commit. Do not write
  `Co-Authored-By` anywhere.
- Never `--release`. One command at a time, in the foreground. `check` and `corpus` share one build, so running them
  together only makes each wait for the other. Wait for a command to return: no polling, no `pgrep`.

## What the gates do not do

`check` runs Cantrip's lint, citation, coverage and mutation tools and checks the shape of the work. No gate reads the
rules. Nothing here compares the Oracle text to the code, checks that a citation's rule says what its comment claims,
decides whether a mechanic should be shared, or looks at the client. A passing card has earned a review. It has not passed
one, and Cantrip's own review (`review.md`) still has to run before it is proposed. Commits stay on the lane's local branch.
Proposing them upstream is a person's decision.
