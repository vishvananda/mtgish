---
name: card-lane
description: Turn Cantrip cards the mtgish generator writes whole into card commits, in this session, one card at a time - take the next card from the queue, validate the generated code against its Oracle text, write its scenarios, pass the gates, commit, take the next. Only cards with no gap reach this lane, so it needs no mechanic or engine work. Use when asked to work through generated cards, whoever is doing the checking. It needs no separate model or driver; a driven worker follows this same file.
---

# The card lane

The mtgish generator has a closed table of recipes: a card whose every ability has one, in shared mechanics that exist, gets
a whole file in the form Cantrip accepts. Any other card has a gap and never comes here, so a card in this lane needs no
mechanic and no engine work, and its code and content are final. Your job has two parts: **validate the generated card
against its Oracle text, and write the scenarios that prove it.** A tool writes the card and placeholder scenarios, the
gates judge, and a passing card becomes one `Card:` commit on a local Cantrip branch.

You do not write code. A card that is wrong is a generator bug, and finding one is a good result: the fix goes in the
generator, where it corrects every card with that recipe.

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
| `card_lane.py abandon "Name" --outcome generator_bug --reason "..."` | Keeps the work as a patch, cleans the tree, records why |
| `card_lane.py tool cr.py check --show FILE` | Any tool in Cantrip's `tools/maintenance`, in the lane's environment |
| `card_lane.py status` | The queue, and what each card cost |

No queue yet? Run `card_lane.py setup`, then `card_lane.py queue --format pioneer`. See `README.md` beside `card_lane.py`.

## The loop

Run `next`, do the card as below, run `commit`, and run `next` again for as many cards as you were asked for. Each card
is its own commit and starts from a clean tree. A gate that fails on every card is the lane's bug: stop and look.

## Doing one card

**Done means every printed line is validated, `check --fast` passes and `report.json` says `done`.** Most of a card's cost
is thinking and context, not builds. These cards are a sentence or two over one shared function, so a card should take a
few scenarios. Spend on the card's rules, not on learning the project.

The brief names the Cantrip tree, which is your working directory, and the card's packet, which holds:

- `reference.md`: Oracle text, official rulings, CR rules, prior art, test points.
- `shared.md`: the shared code the card calls, as defined. What the code does is here, not in the card's words.
- `prior-art.md`: short scenarios of cards built on the same shared call. Copy their shape.
- `first-build.txt`: what the code and placeholders did on their first run.
- `report.json`: your report, its printed lines and test points already listed.

Read those, then, once:

- `.claude/skills/write-game-script/SKILL.md`: how scenarios play, and its gotchas.
- The section "Scenarios: what a card owes" of `.claude/skills/add-cards/SKILL.md`. Only that section.

**Read nothing else whole.** Keep what you read small, because everything you read is paid for again on every later step:

- `docs/adding-cards.md` and `scenarios/README.md` are reference: `rg -n "word" FILE | head -20`, then read those lines.
- A rule is `tool cr.py 603.6a`. Never open the rules file itself.
- Bound every search with `| head -40` and aim it at a file you can name. Do not read `crates/cantrip-script`: `corpus --next
  --brief`, `--dump` and `--diff N` tell you what a scenario can see.

A card that is wrong is worse than a card that is missing.

1. **Read the card's words first.** Work out from the Oracle text, rulings and rules what must happen, before you read the
   code. Write expectations from the rules, never from what the engine printed.
2. **Validate the card, line by line.** Now read its file, its content file and `shared.md`. For each printed line in
   `report.json`'s `validation`, say in `code` what implements it, and set `agrees`. Read `shared.md`, not the names: a
   function called `target_creature_gets` agrees only if its body does that. Check:
   - the amounts, and who gets them: you, the target, each player;
   - the targets: what is legal, how many, and that a filter in the words is in the code ("target opponent", "nonblack");
   - the trigger: the event, and whose;
   - the duration and the sequence the sentence states;
   - that a keyword on the card is in the content file's `keywords`, and that a card with no file has nothing but keywords
     and characteristics to it;
   - whether a line of the card has no code at all.

   If a line disagrees, stop here: set `agrees` to `false` and say why in `why`, then see Limits.
3. **Write the scenarios.** Write one, play it, and read `corpus --file PATH --dump` for its keys before you write the rest.
   The tool may have written real scenarios from the card's words: keep one that proves what it says, and fix or replace the
   rest. Replace every `TODO` placeholder. Each scenario is named for what it proves and opens with a `#` comment saying what
   it shows and the rule. Together they run every line and branch of the card, and each one must execute the card's own code
   or, for a card with none, play the card itself. A keyword gets a scenario that fails without it: the flier that cannot be blocked,
   the first striker that takes no damage back. Give every test point an expectation that fails if the card gets it wrong:
   the exact amount, both players' objects for "each", "target" or "you control", an earlier turn for "this turn".

   The gate breaks numbers and comparisons, not meaning, so ask the card these yourself. An independent review blocked
   pilot cards on each of them:
   - **Every ruling is specification.** Each gets an expectation, unless the card's own code decides nothing there. A ruling
     with two halves gets both.
   - **Order.** If swapping two of the card's steps leaves the end state the same, add a `?` at the point between them,
     usually at the question the second one asks. A sequence the text states must be able to fail.
   - **Last-known information.** If the code reads `old` rather than `new`, or a value that can change, use something whose
     answer differs on the battlefield and in the graveyard.
   - **Read through words.** A scenario does not have to show a text change, but say so in `notes`.

   Mistakes that cost earlier cards a run:
   - Bot1 plays odd turns and Bot2 even ones: a wait for Bot1 is `[T3 Bot1.Main1]`, never `[T2 Bot1.Main1]`.
   - Counts belong to a zone: `Bot1.Battlefield.Count`. There is no `<Name>.Count`. Same-named objects are `Name#1`.
   - Mana empties between steps, and a `?` reads at the next question, so a spell is usually still on the stack.
4. **Check.** Run `check --fast` until it passes. Then run `tool cr.py check --show <its scenarios>` and read each citation
   against the rule printed beside it. A number copied from a shared function's comment or another card is how a wrong one
   gets in (CR 119.1 is starting life totals; gaining life is 119.3). Do the same for the card file's citations: a wrong one
   there is a generator bug. Do not run the whole corpus.
5. **Fill in `report.json`.** Set `decision` to `done`. Give each test point its `scenario` and `line`, the 1-based line of
   the expectation that catches it: a `?`, a `!` or a line under `[Expect]`. Do the same for each ruling, or say in
   `no_decision` why the card's own code decides nothing there. Each entry of `look_answers` names the LOOK's
   line as `line 12`, or the other card's file for a repeated function. `equivalent_mutants` stays empty unless the full gate
   reports a mutant that means the same thing: then add `{"mutant": "line 6: 4 to 5", "why": "..."}`, and only if it truly
   changes nothing the rules can see.
6. **Finish.** Run `commit`. If it fails, fix what it names and run it again. If the brief says a driver runs the gates,
   stop after the report instead: it runs them and sends back what fails, in this same session.

## Limits

- Write only new files under `scenarios/` and the packet's `report.json`. The card's file and content file are read-only,
  and the `generated` gate holds them to what the tools wrote. Edit nothing else: not `cantrip-mechanics`, not the core, not
  the client.
- If the card is wrong, or a scenario shows it does something other than the text says, do not work around it. Set
  `"decision": "generator_bug"` with the line, what the code does and what the text requires in `reason`. If the card is
  right but the engine plays the rules wrongly, the decision is `engine_gap`. If a tool is broken or the tree is not what the
  brief describes, it is `needs_review`. Write the report, then run `abandon` with that outcome. Do this early. These outcomes
  are normal and cost the project little; a false pass costs it a lot.
- Never weaken a scenario to pass. Never assert what you know is wrong. No `# Known bug` scenarios in this lane.
- No `git commit`, no `git add`, no `git push`, no `gh`, and no network. `commit` makes the commit. Do not write
  `Co-Authored-By` anywhere.
- Never `--release`. One command at a time, in the foreground. `check` and `corpus` share one build, so running them
  together only makes each wait for the other. Wait for a command to return: no polling, no `pgrep`.

## What the gates do not do

`check` runs Cantrip's lint, citation, coverage and mutation tools, holds the code to what the generator wrote, and checks
the shape of the work. No gate reads the rules. The comparison with the Oracle text is the validation above, and a gate only
checks that one was recorded, not that it is right. Nothing checks that a citation's rule says what its comment claims, or
looks at the client.
A passing card has earned a review. It has not passed
one, and Cantrip's own review (`review.md`) still has to run before it is proposed. Commits stay on the lane's local branch.
Proposing them upstream is a person's decision.
