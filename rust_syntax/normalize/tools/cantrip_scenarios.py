"""Cantrip scenarios a card's printed words require.

Reads only the card's Oracle text and characteristics, never the AST or the
generated code. That is the point of it: the card's file comes from the grammar
and the lowering, these expectations from a second reading of the same words, so
a mistake in the first shows up as a failing scenario rather than as a test that
agrees with the bug. See `claims` for what it reads, and `scenarios` for what it
writes. A card whose text no pattern matches whole gets no scenarios at all: a
partial reading would leave a sentence unproved and still look covered.

Used by cantrip_candidates.py. Nothing here imports the Rust side.
"""

import json
import os
import re
import unicodedata
from dataclasses import dataclass, field

WORDS = {"a": 1, "an": 1, "one": 1, "two": 2, "three": 3, "four": 4, "five": 5, "six": 6, "seven": 7}
BASIC = {"W": "Plains", "U": "Island", "B": "Swamp", "R": "Mountain", "G": "Forest"}
MOX = {"W": "Mox Pearl", "U": "Mox Sapphire", "B": "Mox Jet", "R": "Mox Ruby", "G": "Mox Emerald"}
COLOR_WORD = {"W": "White", "U": "Blue", "B": "Black", "R": "Red", "G": "Green"}
KEYWORDS = {
    "flying": "Flying", "haste": "Haste", "vigilance": "Vigilance", "trample": "Trample",
    "first strike": "FirstStrike", "double strike": "DoubleStrike", "deathtouch": "Deathtouch",
    "lifelink": "Lifelink", "reach": "Reach", "defender": "Defender", "flash": "Flash",
    "hexproof": "Hexproof", "indestructible": "Indestructible", "menace": "Menace", "shroud": "Shroud",
}
OTHER_MOX = "Mox Pearl"
REMOVAL = "Bilbo's Deadly Slice"  # "Destroy target creature." for {1}{B}{B}, which Black Lotus pays


# ---- What the words say ------------------------------------------------------------------------------------------

@dataclass
class Claim:
    """One sentence's meaning: `kind` and what it names."""
    kind: str
    n: int = 0
    power: int = 0
    toughness: int = 0
    life: int = 0
    scope: str = ""


NUMBER = r"(a|an|one|two|three|four|five|six|seven|\d+)"
PT = r"([+-]\d+)/([+-]\d+)"
TARGETS = r"any target|target creature or planeswalker|target creature|target player or planeswalker|target player"
DESTROYED = r"artifact or enchantment|artifact|enchantment|creature|land"


def amount(word):
    return int(word) if word.isdigit() else WORDS[word]


def effect(text):
    """The claim of one effect, or None: the whole text must match, so nothing is half read."""
    t = text[0].upper() + text[1:]
    if m := re.fullmatch(rf"~ deals (\d+) damage to ({TARGETS})", t):
        return Claim("damage", n=int(m[1]), scope=m[2])
    if m := re.fullmatch(rf"~ deals (\d+) damage to ({TARGETS}) and you gain (\d+) life", t):
        return Claim("drain", n=int(m[1]), scope=m[2], life=int(m[3]))
    if m := re.fullmatch(rf"Draw {NUMBER} cards?", t):
        return Claim("draw", n=amount(m[1]))
    if m := re.fullmatch(r"You gain (\d+) life", t):
        return Claim("gain", n=int(m[1]))
    if m := re.fullmatch(rf"Target creature gets {PT} until end of turn", t):
        return Claim("pump", power=int(m[1]), toughness=int(m[2]))
    if m := re.fullmatch(rf"Target creature gets {PT} and gains first strike until end of turn", t):
        return Claim("pump_first_strike", power=int(m[1]), toughness=int(m[2]))
    if m := re.fullmatch(rf"Creatures you control get {PT} until end of turn", t):
        return Claim("team_pump", power=int(m[1]), toughness=int(m[2]))
    if m := re.fullmatch(rf"Destroy target ({DESTROYED})", t):
        return Claim("destroy", scope=m[1])
    return None


def split_lines(card):
    """(keyword lines' keywords, the other lines) of the printed text, reminders dropped (CR 207.2)."""
    keywords, rest = [], []
    for line in (card.get("oracleText") or "").replace(card["name"], "~").split("\n"):
        line = re.sub(r"\s*\([^)]*\)", "", line).strip()
        if not line:
            continue
        words = [w.strip().lower() for w in line.split(",")]
        if all(w in KEYWORDS for w in words):
            keywords += [KEYWORDS[w] for w in words]
        else:
            rest.append(line)
    return keywords, rest


def claims(card):
    """(frame, keywords, claim) for a card read whole, or None. `frame` is how the effect comes about."""
    keywords, rest = split_lines(card)
    kinds = card["type"].split(" — ")[0].split()
    spell = "Instant" in kinds or "Sorcery" in kinds
    if not rest:
        return ("keywords", keywords, None) if "Creature" in kinds else None
    if len(rest) != 1 or not rest[0].endswith("."):
        return None
    sentence = rest[0][:-1]
    if spell:
        found = effect(sentence)
        return ("spell", keywords, found) if found else None
    if "Creature" not in kinds:
        return None
    for frame, pattern in (("enters", r"When this creature enters, (.+)"), ("dies", r"When this creature dies, (.+)")):
        if m := re.fullmatch(pattern, sentence):
            found = effect(m[1].replace("this creature", "~"))
            if found and found.kind in ("gain", "draw", "team_pump", "pump"):
                return (frame, keywords, found)
    return None


# ---- The board --------------------------------------------------------------------------------------------------

def mana_value(cost):
    return sum(int(s) if s.isdigit() else 1 for s in re.findall(r"\{([^}]+)\}", cost or ""))


def free_mana(cost):
    """Zero-cost sources making exactly this cost on one turn, as (card, activation argument), or None. new-card.py's
    rule: Black Lotus for three of one color, a Mox for one of its color, Gleemox and Lotus Petal for one of any."""
    colored, generic = {}, 0
    for symbol in re.findall(r"\{([^}]+)\}", cost or ""):
        if "P" in symbol or symbol[0].isdigit() and "/" in symbol:
            return None  # Phyrexian and {2/R} ask how they are paid, which is a choice no line here answers
        symbol = symbol.split("/")[0]  # either half of a hybrid symbol pays it (CR 107.4e)
        if symbol.isdigit():
            generic += int(symbol)
        elif symbol in MOX:
            colored[symbol] = colored.get(symbol, 0) + 1
        else:
            return None
    sources, left, need = [], generic, dict(colored)
    if need:
        c = max(need, key=need.get)
        if need[c] + left >= 3 and need[c] >= 2:
            sources.append(("Black Lotus", f"Color: {COLOR_WORD[c]}"))
            used = min(3, need[c])
            need[c] -= used
            left -= 3 - used
    elif left >= 3:
        sources.append(("Black Lotus", "Color: White"))
        left -= 3
    any_color = ["Gleemox", "Lotus Petal"]
    for c, n in need.items():
        for i in range(n):
            if i == 0 and MOX[c] not in [s for s, _ in sources]:
                sources.append((MOX[c], ""))
            elif any_color:
                sources.append((any_color.pop(0), f"Color: {COLOR_WORD[c]}"))
            else:
                return None
    named = set(re.findall(r"[WUBRG]", cost or ""))  # spare mana of a color the cost names would have to be assigned
    spare = [m for c, m in MOX.items() if m not in [s for s, _ in sources] and c not in named]
    spare += [m for c, m in MOX.items() if m not in [s for s, _ in sources] and c in named] + any_color
    for _ in range(left):
        if not spare:
            return None
        card = spare.pop(0)
        sources.append((card, "Color: White" if card in ("Gleemox", "Lotus Petal") else ""))
    return sources if len(sources) <= 6 else None


class Pieces:
    """The plainest implemented cards to build a board from, read from Cantrip's content."""

    def __init__(self, content):
        self.vanilla = {}
        self.names = set()
        for file in os.listdir(content):
            card = json.load(open(os.path.join(content, file), encoding="utf-8-sig"))
            self.names.add(card["name"])
            text = (card.get("oracleText") or "").strip()
            if "Creature" in card.get("type", "") and not text and not card.get("keywords"):
                t = card.get("toughness")
                if isinstance(t, int) and free_mana(card.get("manaCost", "")) is not None:
                    self.vanilla.setdefault(t, []).append(card)
        for t in self.vanilla:
            self.vanilla[t].sort(key=lambda c: (mana_value(c.get("manaCost")), c["name"]))

    def creature(self, toughness):
        """A vanilla creature with exactly this toughness, cheapest first, or None."""
        found = self.vanilla.get(toughness)
        return found[0] if found else None


def activate(player, source, argument):
    return f"{player}.Battlefield.{source}.Activate" + (f"({argument})" if argument else "")


def casts(player, sources):
    lines = []
    for source, _ in sources:
        lines += [f"{player}.Hand.{source}.Cast", f"{player}.Pass"]
    return lines


@dataclass
class Script:
    slug: str
    why: list
    bot1: list = field(default_factory=list)
    bot2: list = field(default_factory=list)
    actions: list = field(default_factory=list)
    expect: list = field(default_factory=list)

    def text(self):
        out = [f"# {line}" for line in self.why]
        out += ["", "[Bot1 Deck]", *self.bot1, "Island x20", "", "[Bot2 Deck]", *self.bot2, "Island x20", "", "[Actions]"]
        out += [*self.actions, "", "[Expect]", *self.expect, ""]
        return "\n".join(out)


class Unwritable(Exception):
    """The board this claim needs can't be built from what Cantrip has."""


def sacrificed(sources):
    return [s for s, _ in sources if s in ("Black Lotus", "Lotus Petal")]


def graveyard_keys(player, names, sources):
    """A graveyard exactly: `names` went in last, on top of what paying sacrificed."""
    gone = sacrificed(sources)
    listed = list(names) + gone
    return [f"{player}.Graveyard = " + "; ".join(listed)] if len(gone) < 2 else [f"{player}.Graveyard.Count = {len(listed)}"]


def spell_setup(card, name):
    sources = free_mana(card.get("manaCost", ""))
    if sources is None:
        raise Unwritable(f"no free mana makes {card.get('manaCost')}")
    return sources


def slug_of(name):
    folded = unicodedata.normalize("NFKD", name)
    folded = "".join(c for c in folded if not unicodedata.combining(c))
    return re.sub(r"[^a-z0-9]+", "-", folded.replace("'", "").lower()).strip("-")


def victim_lines(pieces, toughness, player="Bot2"):
    """Lines that put a vanilla creature of this toughness on `player`'s side on their first turn, and the cards for it."""
    creature = pieces.creature(toughness)
    if creature is None:
        raise Unwritable(f"no vanilla creature with toughness {toughness}")
    sources = free_mana(creature.get("manaCost", ""))
    lines = casts(player, sources) + [activate(player, s, a) for s, a in sources]
    lines += [f"{player}.Hand.{creature['name']}.Cast", f"{player}.Pass"]
    return creature, [s for s, _ in sources] + [creature["name"]], lines, sources


def board_turn(card, sources, opponent_cards, opponent_lines, target, mana_check=True):
    """Bot1 readies its mana, Bot2 builds its side, and on turn 3 Bot1 casts the card at `target`."""
    name = card["name"]
    actions = casts("Bot1", sources) + ["[T2 Bot2.Main1]", *opponent_lines, "[T3 Bot1.Main1]"]
    actions += [activate("Bot1", s, a) for s, a in sources]
    actions.append(f"Bot1.Hand.{name}.Cast({target})")
    if mana_check:
        actions.append("? Bot1.Mana =")
    actions += ["Bot1.Pass", "Bot2.Pass"]
    return actions


def refused(name, *targets):
    return [f"! Bot1.Hand.{name}.Cast({t})" for t in targets]


def rule_of_resolving(card):
    return "304.2" if "Instant" in card["type"] else "307.2"


def damage_scripts(card, claim, pieces):
    name, slug, n, scope = card["name"], slug_of(card["name"]), claim.n, claim.scope
    sources = spell_setup(card, name)
    bot1 = [s for s, _ in sources] + [name]
    out = []
    creature_ok = scope in ("any target", "target creature", "target creature or planeswalker")
    player_ok = scope in ("any target", "target player", "target player or planeswalker")
    sentence = f"\"{name} deals {n} damage to {scope}.\""
    life_rider, rider = [], ""
    if claim.kind == "drain":
        life_rider = [f"Bot1.Life = {20 + claim.life}"]
        sentence = f"\"{name} deals {n} damage to {scope} and you gain {claim.life} life.\""
        rider = f" Bot1 gains exactly {claim.life} (CR 119.3)."
    if creature_ok:
        victim, cards, lines, spent = victim_lines(pieces, n)
        limits = []
        # A permanent that is no legal target, there and offered otherwise, beside the legal cast below.
        lines = lines + casts("Bot2", [(OTHER_MOX, "")])
        cards = cards + [OTHER_MOX]
        if not player_ok:
            limits.append("Bot2")
        limits.append(f"Bot2.Battlefield.{OTHER_MOX}")
        actions = casts("Bot1", sources) + ["[T2 Bot2.Main1]", *lines, "[T3 Bot1.Main1]"]
        actions += refused(name, *limits)
        actions += [activate("Bot1", s, a) for s, a in sources]
        actions += [f"Bot1.Hand.{name}.Cast(Bot2.Battlefield.{victim['name']})", "? Bot1.Mana =", "Bot1.Pass", "Bot2.Pass"]
        out.append(Script(
            f"{slug}-kills-a-creature-with-toughness-{n}-and-can-only-target-what-it-says",
            [f"{sentence} A {victim['name']} with toughness exactly {n} is dealt lethal damage and dies (CR 120.3e, 704.5g).{rider}",
             "A Mox is refused: it is no legal target, and the refusals sit before the legal cast so they cannot pass for want of a target.",
             "The pool is checked right after the cast, so the cost is exact."],
            bot1, cards, actions,
            [f"Bot2.Battlefield.{victim['name']} = (absent)", *graveyard_keys("Bot2", [victim["name"]], spent),
             *graveyard_keys("Bot1", [name], sources), *life_rider]))
        big, cards, lines, spent = victim_lines(pieces, n + 1)
        out.append(Script(
            f"{slug}-deals-exactly-{n}-to-a-creature-that-survives-it",
            [f"{sentence} A {big['name']} with toughness {n + 1} survives with exactly {n} damage marked (CR 120.3e): one more would have killed it."],
            bot1, cards, board_turn(card, sources, cards, lines, f"Bot2.Battlefield.{big['name']}"),
            [f"Bot2.Battlefield.{big['name']}.Damage = {n}", *graveyard_keys("Bot1", [name], sources), *life_rider]))
        own = pieces.creature(1)
        actions = casts("Bot1", [(own["name"], "")]) + casts("Bot1", sources) + ["[T3 Bot1.Main1]"]
        actions += [activate("Bot1", s, a) for s, a in sources]
        actions += [f"Bot1.Hand.{name}.Cast(Bot1.Battlefield.{own['name']})", "Bot1.Pass", "Bot2.Pass"]
        out.append(Script(
            f"{slug}-can-target-your-own-creature",
            [f"{sentence} \"Target\" reads either player's creature: Bot1's own {own['name']} is a legal target and dies (CR 115.1, 704.5g).",
             f"The spell goes to the graveyard as the last step of resolving and the creature dies after, at the next check, so the creature is on top (CR {rule_of_resolving(card)}, 704.3)."],
            [own["name"], *bot1], [], actions,
            [f"Bot1.Battlefield.{own['name']} = (absent)", *graveyard_keys("Bot1", [own["name"], name], sources), *life_rider]))
    if player_ok:
        actions = casts("Bot1", sources) + [activate("Bot1", s, a) for s, a in sources]
        actions += [f"Bot1.Hand.{name}.Cast(Bot2)", "? Bot1.Mana =", "Bot1.Pass", "Bot2.Pass"]
        out.append(Script(
            f"{slug}-hits-a-player-for-{n}",
            [f"{sentence} Bot2 loses exactly {n} life (CR 120.3a).{rider}"],
            bot1, [], actions, [f"Bot2.Life = {20 - n}", *graveyard_keys("Bot1", [name], sources), *life_rider]))
    return out


def pump_scripts(card, claim, pieces):
    """A pump on a creature of each side: the amounts while it lasts, gone again after the turn (CR 611.2a, 514.2)."""
    name, slug = card["name"], slug_of(card["name"])
    p, t = claim.power, claim.toughness
    sources = spell_setup(card, name)
    bot1 = [s for s, _ in sources] + [name]
    sign = lambda v: f"{v:+d}"
    keyed = claim.kind == "pump_first_strike"
    base = max(1, 1 - t)
    victim, cards, lines, _ = victim_lines(pieces, base)
    v = victim["name"]
    cards = cards + [OTHER_MOX]
    lines = lines + casts("Bot2", [(OTHER_MOX, "")])
    actions = casts("Bot1", sources) + ["[T2 Bot2.Main1]", *lines, "[T3 Bot1.Main1]"]
    actions += refused(name, "Bot2", f"Bot2.Battlefield.{OTHER_MOX}")
    actions += [activate("Bot1", s, a) for s, a in sources]
    actions += [f"Bot1.Hand.{name}.Cast(Bot2.Battlefield.{v})", "? Bot1.Mana =", "Bot1.Pass", "Bot2.Pass"]
    actions += [f"? Bot2.Battlefield.{v}.Power = {victim['power'] + p}", f"? Bot2.Battlefield.{v}.Toughness = {victim['toughness'] + t}"]
    if keyed:
        actions.append(f"? Bot2.Battlefield.{v}.Keywords = FirstStrike")
    actions.append("[T4 Bot2.Main1]")
    text = f"Target creature gets {sign(p)}/{sign(t)}" + (" and gains first strike" if keyed else "") + " until end of turn."
    expect = [f"Bot2.Battlefield.{v}.Power = {victim['power']}", f"Bot2.Battlefield.{v}.Toughness = {victim['toughness']}",
              *graveyard_keys("Bot1", [name], sources)]
    if keyed:
        expect.append(f"Bot2.Battlefield.{v}.Keywords = (absent)")
    out = [Script(
        f"{slug}-gives-exactly-{'minus-' if p < 0 else 'plus-'}{abs(p)}-{'minus-' if t < 0 else 'plus-'}{abs(t)}-until-end-of-turn",
        [f"\"{text}\" Bot2's {v} ({victim['power']}/{victim['toughness']}) has exactly that on turn 3 and is back as printed on",
         "turn 4 (CR 611.2a, 514.2). A player and a Mox are refused as targets: \"target creature\" reads neither."],
        bot1, cards, actions, expect)]
    own = pieces.creature(1)
    actions = casts("Bot1", [(own["name"], "")]) + casts("Bot1", sources) + ["[T3 Bot1.Main1]"]
    actions += [activate("Bot1", s, a) for s, a in sources]
    actions += [f"Bot1.Hand.{name}.Cast(Bot1.Battlefield.{own['name']})", "Bot1.Pass", "Bot2.Pass"]
    if own["toughness"] + t > 0:
        expect = [f"Bot1.Battlefield.{own['name']}.Power = {own['power'] + p}", f"Bot1.Battlefield.{own['name']}.Toughness = {own['toughness'] + t}"]
        says = f"it is {own['power'] + p}/{own['toughness'] + t}"
    else:
        expect = [f"Bot1.Battlefield.{own['name']} = (absent)"]
        says = f"toughness {own['toughness'] + t} puts it into the graveyard (CR 704.5f)"
    out.append(Script(
        f"{slug}-can-target-your-own-creature",
        [f"\"{text}\" \"Target\" reads either player's creature: on Bot1's own {own['name']}, {says}."],
        [own["name"], *bot1], [], actions, expect))
    return out


def team_pump_actions(p, t):
    return f"Creatures you control get {p:+d}/{t:+d} until end of turn."


def team_scripts(card, claim, pieces, frame):
    """Your creatures get it, the opponent's do not, and it is over next turn."""
    name, slug = card["name"], slug_of(card["name"])
    p, t = claim.power, claim.toughness
    sources = spell_setup(card, name)
    own = pieces.creature(max(1, 1 - t))
    if own is None or own.get("manaCost") != "{0}":
        raise Unwritable("no free creature survives the pump")
    other = pieces.creature(1)
    o, x = own["name"], other["name"]
    bot1 = [o] + [s for s, _ in sources] + [name]
    actions = casts("Bot1", [(o, "")]) + casts("Bot1", sources)
    actions += ["[T2 Bot2.Main1]", f"Bot2.Hand.{x}.Cast", "Bot2.Pass", "[T3 Bot1.Main1]"]
    actions += [activate("Bot1", s, a) for s, a in sources]
    actions += [f"Bot1.Hand.{name}.Cast", "? Bot1.Mana =", "Bot1.Pass", "Bot2.Pass"]
    if frame != "spell":
        actions += ["Bot1.Pass", "Bot2.Pass"]
    actions += [f"? Bot1.Battlefield.{o}.Power = {own['power'] + p}", f"? Bot1.Battlefield.{o}.Toughness = {own['toughness'] + t}",
                f"? Bot2.Battlefield.{x}.Power = {other['power']}", f"? Bot2.Battlefield.{x}.Toughness = {other['toughness']}",
                "[T4 Bot2.Main1]"]
    how = "it resolves" if frame == "spell" else "it enters"
    return [Script(
        f"{slug}-pumps-your-creatures-and-not-the-opponents",
        [f"\"{team_pump_actions(p, t)}\" As {how}, Bot1's {o} has exactly that and Bot2's {x} does not (CR 611.2a, 611.2c), and on",
         "turn 4 it is over (CR 514.2)."],
        bot1, [x], actions,
        [f"Bot1.Battlefield.{o}.Power = {own['power']}", f"Bot1.Battlefield.{o}.Toughness = {own['toughness']}"])]


def destroy_scripts(card, claim, pieces):
    name, slug, scope = card["name"], slug_of(card["name"]), claim.scope
    sources = spell_setup(card, name)
    bot1 = [s for s, _ in sources] + [name]
    if scope == "land":
        theirs, lines, cards, wrong = "Island", ["Bot2.Hand.Island.Play"], [], OTHER_MOX
        lines = lines + casts("Bot2", [(OTHER_MOX, "")])
        cards = [OTHER_MOX]
    elif scope == "creature":
        creature = pieces.creature(1)
        theirs, wrong = creature["name"], OTHER_MOX
        lines = casts("Bot2", [(theirs, ""), (OTHER_MOX, "")])
        cards = [theirs, OTHER_MOX]
    elif scope in ("artifact", "artifact or enchantment"):
        theirs, wrong = OTHER_MOX, "Island"
        lines, cards = ["Bot2.Hand.Island.Play"] + casts("Bot2", [(OTHER_MOX, "")]), [OTHER_MOX]
    else:
        raise Unwritable(f"no plain {scope} to destroy")
    actions = casts("Bot1", sources) + ["[T2 Bot2.Main1]", *lines, "[T3 Bot1.Main1]"]
    actions += refused(name, f"Bot2.Battlefield.{wrong}", "Bot2")
    actions += [activate("Bot1", s, a) for s, a in sources]
    actions += [f"Bot1.Hand.{name}.Cast(Bot2.Battlefield.{theirs})", "? Bot1.Mana =", "Bot1.Pass", "Bot2.Pass"]
    gone = f"Bot2.Battlefield.{theirs} = (absent)" if theirs != "Island" else "Bot2.Battlefield.Island = (absent)"
    out = [Script(
        f"{slug}-destroys-the-{slug_of(scope)}-it-targets",
        [f"\"Destroy target {scope}.\" Bot2's {theirs} goes to its graveyard (CR 701.8a); a {wrong} and a player are refused as",
         "targets. The pool is checked right after the cast, so the cost is exact."],
        bot1, cards, actions,
        [gone, f"Bot2.Graveyard = {theirs}", *graveyard_keys("Bot1", [name], sources)])]
    return out


def life_and_cards_expect(claim, listed, cast):
    """`listed` cards were named ahead of the deck's twenty Islands, seven were drawn, and `cast` have left the hand."""
    if claim.kind == "gain":
        return [f"Bot1.Life = {20 + claim.n}", "Bot2.Life = 20"]
    return [f"Bot1.Hand.Count = {7 - cast + claim.n}", f"Bot1.Library.Count = {listed + 20 - 7 - claim.n}", "Bot2.Hand.Count = 7"]


def plain_scripts(card, claim):
    """"Draw N cards" and "You gain N life": the caster, and exactly N."""
    name, slug = card["name"], slug_of(card["name"])
    sources = spell_setup(card, name)
    bot1 = [s for s, _ in sources] + [name]
    actions = casts("Bot1", sources) + [activate("Bot1", s, a) for s, a in sources]
    actions += [f"Bot1.Hand.{name}.Cast", "? Bot1.Mana =", "Bot1.Pass", "Bot2.Pass"]
    what = f"gains exactly {claim.n} life (CR 119.3)" if claim.kind == "gain" else f"draws exactly {claim.n} (CR 121.1)"
    verb = f"gains-{claim.n}-life" if claim.kind == "gain" else f"draws-exactly-{claim.n}"
    return [Script(
        f"{slug}-{verb}", [f"{name}: the caster {what}, and the opponent nothing. The pool is checked right after the cast, so the cost is exact."],
        bot1, [], actions,
        [*life_and_cards_expect(claim, len(bot1), len(bot1)), *graveyard_keys("Bot1", [name], sources)])]


def permanent_keys(card, name, keywords):
    keys = [f"Bot1.Battlefield.{name}.Power = {card['power']}", f"Bot1.Battlefield.{name}.Toughness = {card['toughness']}"]
    keys.append(f"Bot1.Battlefield.{name}.Keywords = " + ("; ".join(sorted(keywords)) if keywords else "(absent)"))
    return keys


def trigger_scripts(card, frame, keywords, claim, pieces):
    """A creature's enters or dies trigger, and that another creature's does not set it off."""
    name, slug = card["name"], slug_of(card["name"])
    sources = spell_setup(card, name)
    if not isinstance(card.get("power"), int) or not isinstance(card.get("toughness"), int):
        raise Unwritable("a creature with a number in its P/T only the rules can read")
    other = pieces.creature(1)
    x = other["name"]
    if claim.kind == "pump":
        return trigger_pump_scripts(card, frame, keywords, claim, pieces)
    if claim.kind not in ("gain", "draw"):
        if claim.kind == "team_pump" and frame == "enters":
            return team_scripts(card, claim, pieces, frame)
        raise Unwritable(f"{frame} {claim.kind}")
    bot1 = [s for s, _ in sources] + [name, x]
    if frame == "enters":
        actions = casts("Bot1", sources) + [activate("Bot1", s, a) for s, a in sources]
        actions += [f"Bot1.Hand.{name}.Cast", "? Bot1.Mana =", "Bot1.Pass", "Bot2.Pass", "Bot1.Pass", "Bot2.Pass"]
        actions += [f"Bot1.Hand.{x}.Cast", "Bot1.Pass", "Bot2.Pass"]
        expect = [*life_and_cards_expect(claim, len(bot1), len(bot1)), *permanent_keys(card, name, keywords)]
        what = f"you gain {claim.n} life" if claim.kind == "gain" else f"draw {claim.n}"
        return [Script(
            f"{slug}-entering-triggers-once",
            [f"\"When this creature enters, {what}.\" It triggers when it enters and resolves once (CR 603.2); the {x} cast after it is",
             "another creature entering, which does not trigger it. Its printed P/T and keywords are checked too."],
            bot1, [], actions, expect)]
    # It dies: a creature dying is the one way the trigger is reached, so Bot2 kills it.
    if REMOVAL not in pieces.names or "Black Lotus" not in pieces.names:
        raise Unwritable("no plain removal")
    actions = casts("Bot1", sources) + [activate("Bot1", s, a) for s, a in sources]
    actions += [f"Bot1.Hand.{name}.Cast", "? Bot1.Mana =", "Bot1.Pass", "Bot2.Pass"]
    before = "? Bot1.Life = 20" if claim.kind == "gain" else f"? Bot1.Hand.Count = {7 - (len(bot1) - 1)}"
    actions += [before, "[T2 Bot2.Main1]", *kill_lines(name), "Bot2.Pass", "Bot1.Pass", "[T2 Bot2.Main2]"]
    expect = [f"Bot1.Battlefield.{name} = (absent)", *life_and_cards_expect(claim, len(bot1) - 1, len(bot1) - 1)]
    expect = [e for e in expect if not e.startswith("Bot2.Hand")]
    what = f"you gain {claim.n} life" if claim.kind == "gain" else f"draw {claim.n}"
    return [Script(
        f"{slug}-dying-triggers-for-its-controller",
        [f"\"When this creature dies, {what}.\" Bot2's {REMOVAL} kills it: its controller, Bot1, gets the effect once (CR 603.10a, 700.4).",
         f"Nothing happens while it merely sits on the battlefield: {'life' if claim.kind == 'gain' else 'the hand'} is checked before, so entering is not what did it."],
        bot1[:-1], ["Black Lotus", REMOVAL], actions, expect)]


def kill_lines(name):
    """Bot2 kills Bot1's creature, the stack left with whatever that trigger put there."""
    return ["Bot2.Hand.Black Lotus.Cast", "Bot2.Pass", "Bot2.Battlefield.Black Lotus.Activate(Color: Black)",
            f"Bot2.Hand.{REMOVAL}.Cast(Bot1.Battlefield.{name})", "Bot2.Pass", "Bot1.Pass"]


def trigger_pump_scripts(card, frame, keywords, claim, pieces):
    """A target chosen as the trigger goes on the stack (CR 603.3d), a creature only, of either player's."""
    name, slug = card["name"], slug_of(card["name"])
    p, t = claim.power, claim.toughness
    sources = spell_setup(card, name)
    text = f"target creature gets {p:+d}/{t:+d} until end of turn."
    victim, cards, lines, _ = victim_lines(pieces, max(1, 1 - t))
    v = victim["name"]
    cards = cards + [OTHER_MOX]
    lines = lines + casts("Bot2", [(OTHER_MOX, "")])
    bot1 = [s for s, _ in sources] + [name]
    own = pieces.creature(1)
    start = casts("Bot1", sources) + ["[T2 Bot2.Main1]", *lines]
    if frame != "enters":
        bot1 = [own["name"], *bot1]
        start = casts("Bot1", [(own["name"], "")]) + start
    cast = ["[T3 Bot1.Main1]", *[activate("Bot1", s, a) for s, a in sources], f"Bot1.Hand.{name}.Cast", "? Bot1.Mana =", "Bot1.Pass", "Bot2.Pass"]
    choose = [f"! Bot1.Target(Bot2.Battlefield.{OTHER_MOX})", "! Bot1.Target(Bot2)", f"Bot1.Target(Bot2.Battlefield.{v})"]
    during = [f"? Bot2.Battlefield.{v}.Power = {victim['power'] + p}", f"? Bot2.Battlefield.{v}.Toughness = {victim['toughness'] + t}"]
    if frame == "enters":
        actions = start + cast + choose + ["Bot1.Pass", "Bot2.Pass", *during, "[T4 Bot2.Main1]"]
        keys = permanent_keys(card, name, keywords)
    else:
        if REMOVAL not in pieces.names or "Black Lotus" not in pieces.names:
            raise Unwritable("no plain removal")
        cards = cards + ["Black Lotus", REMOVAL]
        actions = start + cast + ["[T4 Bot2.Main1]", *kill_lines(name), *choose, "Bot2.Pass", "Bot1.Pass", *during, "[T5 Bot1.Main1]"]
        keys = [f"Bot1.Battlefield.{name} = (absent)"]
    return [Script(
        f"{slug}-{'entering' if frame == 'enters' else 'dying'}-pumps-the-creature-it-targets",
        [f"\"When this creature {'enters' if frame == 'enters' else 'dies'}, {text}\" The target is chosen as the trigger goes on the stack (CR 603.3d):",
         f"a Mox and a player are refused, Bot2's {v} ({victim['power']}/{victim['toughness']}) is taken over the other creature, and has exactly that until the turn ends (CR 611.2a, 514.2)."],
        bot1, cards, actions,
        [f"Bot2.Battlefield.{v}.Power = {victim['power']}", f"Bot2.Battlefield.{v}.Toughness = {victim['toughness']}", *keys])]


def keyword_scripts(card, keywords):
    name, slug = card["name"], slug_of(card["name"])
    sources = spell_setup(card, name)
    if not isinstance(card.get("power"), int) or not isinstance(card.get("toughness"), int):
        raise Unwritable("a creature with a number in its P/T only the rules can read")
    actions = casts("Bot1", sources) + [activate("Bot1", s, a) for s, a in sources]
    actions += [f"Bot1.Hand.{name}.Cast", "? Bot1.Mana =", "Bot1.Pass", "Bot2.Pass"]
    what = ", ".join(keywords) if keywords else "no abilities"
    return [Script(
        f"{slug}-has-its-printed-characteristics",
        [f"{name} has no text but {what}: it is cast for its printed cost and has its printed power, toughness and keywords (CR 702)."],
        [s for s, _ in sources] + [name], [], actions, permanent_keys(card, name, keywords))]


def scenarios(card, pieces):
    """Every scenario this card's words call for, or raises Unwritable, or None for a text it cannot read whole."""
    read = claims(card)
    if read is None:
        return None
    frame, keywords, claim = read
    if frame == "keywords":
        return keyword_scripts(card, keywords)
    if frame != "spell":
        return trigger_scripts(card, frame, keywords, claim, pieces)
    if claim.kind in ("damage", "drain"):
        return damage_scripts(card, claim, pieces)
    if claim.kind in ("pump", "pump_first_strike"):
        return pump_scripts(card, claim, pieces)
    if claim.kind == "team_pump":
        return team_scripts(card, claim, pieces, frame)
    if claim.kind == "destroy":
        return destroy_scripts(card, claim, pieces)
    return plain_scripts(card, claim)
