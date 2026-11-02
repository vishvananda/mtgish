# Writing scenarios

This is all you need of the harness. Do not open `scenarios/README.md`, `docs/adding-cards.md`, the skills, the rules file or
`crates/cantrip-script`, and do not search `scenarios/`: it holds 14,000 files. Each card's section shows how its keywords are said.

## The file

A scenario is a real game: two ordered decklists, a sparse list of actions, and the state it must end in. Libraries are not
shuffled. The first seven cards of a deck are its opening hand and the next ones are its draws in order. Give each deck 7 cards
plus one per draw step reached: pad with `Island x20`. Lands go first when they must be in the opening hand.

```
# Lightning Strike deals exactly 3 to the player, and costs {1}{R} (CR 120.3, 601.2f).

[Bot1 Deck]
Mountain
Mountain
Lightning Strike
Island x20

[Bot2 Deck]
Island x20

[Actions]
Bot1.Hand.Mountain.Play
[T3 Bot1.Main1]
Bot1.Hand.Mountain.Play
Bot1.Battlefield.Mountain#1.Activate
Bot1.Battlefield.Mountain#2.Activate
Bot1.Hand.Lightning Strike.Cast(Bot2)
? Bot1.Mana =
Bot1.Pass

[Expect]
Bot2.Life = 17
Bot1.Graveyard = Lightning Strike
```

Free mana on turn 1, to skip the land drops: put `Mox Ruby`, `Mox Pearl`, `Mox Sapphire`, `Mox Jet`, `Mox Emerald` or
`Black Lotus` in the opening hand, cast them for nothing, and each is on the battlefield once both players pass. Then
`Bot1.Battlefield.Mox Ruby.Activate` makes one mana; `Bot1.Battlefield.Black Lotus.Activate(Color: Blue)` makes three.
Tap for the whole cost right before the `.Cast`, in the same step: mana empties between steps.

Board filler: the plainest implemented card. `Grizzly Bears` 2/2, `Hill Giant` 3/3, `Serra Angel` 4/4 flying vigilance,
`Ornithopter` 0/2 flying artifact for {0}, `Sol Ring`, `Wind Drake` 2/2 flying, `Shock`, `Lightning Bolt`, `Counterspell`,
`Giant Growth`, `Go for the Throat`, `Disenchant`, `Healing Salve`, `Llanowar Elves`. `tool find-cards.py --type creature --toughness 4` (also `--power --mv --color --keyword --text
--vanilla`) finds others. Use only what is already in the tree, never another card of this batch.

## How lines play

There is one cursor for both bots. When a player is asked something, the line under the cursor is tried. If it is that player's
and the question offers it, it is taken and the cursor moves on. Otherwise the player gives the default: pass, no attackers, no
blockers, decline, first eligible card, find nothing, discard newest. So a line happens **as soon as it can**.

- Bot1 plays odd turns and Bot2 even ones. A wait for Bot1 is `[T3 Bot1.Main1]`, never `[T2 Bot1.Main1]`. One land per turn.
- A wait that is already true is consumed on the spot: write the turn. Steps: `Upkeep`, `Draw`, `Main1`, `BeginCombat`,
  `DeclareAttackers`, `DeclareBlockers`, `CombatDamage`, `EndCombat`, `Main2`, `End`, `Cleanup`.
- A mana ability is offered at every priority, so a tap with nothing before it fires in upkeep. Put it after a land play or a wait.
- A spell waits on the stack while its caster holds priority, permanents too. `Bot1.Pass` and then `Bot2.Pass` resolve it
  (or any later line that is not offered yet gets there by default passes). Put `BotN.Pass` after a cast that must have resolved
  before a line that is offered at once: another instant, a `?`, a `!`, a `Target`.
- A `? Key = Value` is checked at the next question after the line above it. Right after a cast the spell is still on the stack.
- A trigger's target is asked as the trigger goes on the stack: `Bot1.Target(Bot2.Battlefield.Island)`.
- A cast waits until every named target is legal. A spell with no legal target is not offered.
- A line that cannot happen within 4 turns fails. `Outstanding: ... asked PayCosts` means the mana came up short.
- A script stops when every line has fired and the stack is empty. It does not reach combat or the next turn unless a trailing
  wait (`[T3 Bot1.Main2]`) says so.
- Bots with nothing to do draw and discard each turn, which changes `Hand.Count` and `Graveyard`.

## Lines

| Line | Answers |
| --- | --- |
| `Bot1.Hand.Island.Play` | play a land |
| `Bot1.Hand.Lightning Bolt.Cast(Bot2)` | cast; arguments are targets, split by `;` |
| `Bot1.Hand.Mind Twist.Cast(Bot2; X: 3)` · `Cast(Kicked: 1)` | X, kicker |
| `Bot1.Hand.Daze.Cast(Duress; Ability: Return)` | alternative cost, by part of its text |
| `Bot1.Battlefield.Island.Activate` | an ability, mana included |
| `Bot1.Battlefield.Tundra.Activate(Ability: {U})` | one of several abilities, by part of its text; `Ability: #2` the Nth |
| `Bot1.Battlefield.Cathar Commando.Activate(Bot2.Battlefield.Sol Ring)` | arguments are targets |
| `Bot1.Battlefield.Black Lotus.Activate(Color: Blue)` | mana of any one color |
| `Bot1.Battlefield.Smuggler's Copter.Activate(Ability: Crew)` then `Bot1.Choose(Grizzly Bears)` | crew, and the creatures that tap for it |
| `Bot1.Pass` · `Bot1.Accept` · `Bot1.Decline` · `Bot1.PayCost` | priority, "you may", unless-costs (default: decline) |
| `Bot1.Target(Sol Ring)` · `Bot1.Target(Island; Mountain)` · `Bot1.Target()` | a trigger's or mode's targets; none for "up to" |
| `Bot1.Choose(Island; Forest)` | choosing cards as something resolves |
| `Bot1.Discard(Island)` · `Bot1.Search(Black Lotus)` · `Bot1.Search()` | discard, search (nothing found) |
| `Bot1.Arrange(Top1; Top2 \| Bottom1)` | scry (top \| bottom), surveil (top \| graveyard) |
| `Bot1.Attack(Grizzly Bears; Hill Giant > Bot2)` · `Bot2.Block(Serra Angel > Grizzly Bears)` | attackers, blockers |
| `Bot1.Hand.Appeal to Eirdu.Cast(Convoke: Ornithopter, Ornithopter)` | tap creatures for a convoke cost |
| `Bot1.AssignDamage(Ox > Lurker: 1; Ox > Raider: 3)` | how combat damage divides |
| `Bot1.Divide(Grizzly Bears: 2; Hill Giant: 1)` | a divided slot's targets and shares |
| `Bot1.Modes(1; 4)` · `Bot1.Modes(Destroy target artifact)` | a modal spell's modes |
| `Bot1.Modes(Elf)` · `Bot1.Modes(Mountain)` | a creature type, land type or other subtype chosen as asked (`ChooseSubtype`) |
| `Bot1.Colors(Red)` · `Bot1.Name(Lightning Bolt)` · `Bot1.Number(7)` · `Bot1.ChoosePlayer(Bot2)` | chosen as asked |

Three players: add a `[Bot3 Deck]`; turns go Bot1, Bot2, Bot3, so a wait for Bot1 after turn 1 is `[T4 Bot1.Main1]`. A search that
shuffles shows in `Bot1.Shuffles = 1`. A generated card's tokens and shared effects are in its code already: read that, never search.

Names: `Stack.Duress` for a spell or for an ability named for its source, `Bot2.Battlefield.Grizzly Bears` where both players
have one, `Island#2` for a second copy, top first. A token is its rules name: the name its text gives it, else subtypes then
`Token`, as `Spirit Token`, `Treasure Token`. Same-named cards in `[Expect]` need an index: `Bot1.Battlefield.Island#1.Tapped`.

## Refusals

`! Bot1.Hand.Card.Cast` fails if that player's next priority offers it. `! Bot2.Hand.Lightning Bolt.Cast(Bot1.Battlefield.X)` refuses
one target. `! Bot2.Block(Blocker > Attacker)` and `! Bot1.Attack(Attacker > Bot2)` refuse a pair at the next declaration. `!
Bot1.Search(Plains)` refuses a find. A bare `! Bot1.Choose` (also `Accept`, `Target`, `Modes`) says the question is not asked; it
needs something since the line above that could have asked it, so `Bot1.Pass` first.

A refusal lands on the next question, so put it where the action would otherwise be legal. An action the player can't pay for
is not offered, and the runner judges a refused cast or activation again with mana to spare, failing it as "proves nothing":
give the player the mana, or write `! Bot1.Hand.Card.Cast(Mana: short)` when the cost is the point. Pair a refusal with the
same action allowed where the rule does not apply.

## Expectations

`? Key = Value` among the actions, `Key = Value` under `[Expect]`. Lists use `;`. Graveyard, library and stack are top first and
in order, so the spell that resolved last comes first; hand, battlefield and exile compare as sets. When order does not matter use
`.Count`. `(absent)` asserts a key is gone: `Bot2.Battlefield.Bat.Keywords = (absent)`, `Bot1.Battlefield.Ornithopter = (absent)`.
A counter kind at zero is absent: `Bat.Counters.MinusOneMinusOne = (absent)`.

- Game: `Turn`, `Step`, `ActivePlayer`, `Result`, `Stack` and `Stack.Count`, `Exile`, `Combat.Attackers`, `Combat.Blocks`, `Delayed`
- Player: `BotN.Life`, `.Poison`, `.Energy`, `.Mana` (`{U}{U}`, empty after `=` for none), `.LandsPlayed`, `.Shuffles`, `.Revealed`
- Zones: `BotN.Hand`, `.Library`, `.Graveyard`, `.Battlefield`, `.Emblems`, each with `.Count`. Never `<Name>.Count`. Exile is one
  list for everyone: `Exile`.
- Permanent, `BotN.Battlefield.<Name>.`: `Tapped`, `Types`, `Subtypes`, `Supertypes`, `Colors`, `Power`, `Toughness`, `Damage`,
  `SummoningSick`, `Token`, `ManaValue`, `AttachedTo`, `Keywords` (engine names: `FirstStrike`, `Flying`, sorted),
  `Counters.PlusOnePlusOne` (any `CounterKind`), `ProtectionFrom`, `CantBeBlocked`, `MustAttack`
- Card in a hand, graveyard or library: `BotN.Hand.<Name>.Power`, `.Toughness`, `.Subtypes`, `.Colors`

`corpus --file PATH --dump` prints every key a scenario ended with, `--diff N` the keys that moved since line N, and `--next --brief`
stops at the next question and lists its options as script lines. Use them on a line you cannot get right from this page.
Write expected values from the rules, never by copying what the engine printed.

## What a card's scenarios owe

A few short scenarios that together run every line and take every branch of the card's own code, each executing that code.

- Open each file with a `#` comment saying what it shows and the rule (`CR 603.6a`). It claims only what an expectation checks.
- Give each test point and ruling an expectation that fails if the card is wrong: the exact amount, both players' objects for
  "each", "target" or "you control", an object exactly at a numeric limit and one just past it, an earlier turn for "this turn".
- A spell or creature with a mana cost gets `? Bot1.Mana =`, nothing after the `=`, right after its cast, paid exactly, so a cost
  too low fails. A `!` with a positive control proves a limit the card's own code enforces.
- A keyword gets a scenario that fails without it: the flier that cannot be blocked, the first striker that takes no damage back. The gates take each keyword off the card and play the scenarios, so a card that only checks its power and toughness does not pass.
- **Rulings are specification.** Each gets an expectation unless the card's own code decides nothing there.
- **Order.** If swapping two of the card's steps leaves the end state the same, add a `?` between them. Where nothing can be asked
  between them, as a search and the shuffle after it, say so in the report instead.
- **Last-known information.** If the code reads `old`, use something whose answer differs on the battlefield and in the graveyard.
- A spell that fizzles runs no line of the card, and each scenario must run one. Show the fizzle, then resolve a second copy.
- Never weaken an expectation to pass. A scenario never says `Known bug`. A filename contains no `todo`.
- A citation must say what its comment claims. `tool cr.py 603.6a` prints a rule, `tool cr.py check --show FILE` every rule a file
  cites. Gaining life is 119.3, not 119.1; a triggered ability on the stack is 603.3; a keyword's rule is its 702.x.
