#!/usr/bin/env python3
"""Type-check the code planned holes hold against stubs of the Cantrip API it proposes.

    proposal_check.py CANTRIP [--cards STEM,...] [--source STEM=FILE ...] [--work DIR] [--jobs N] [--report OUT]

CANTRIP is a Cantrip tree whose crates/cantrip-generated cantrip_tree.py wrote (it has the `proposals` feature and
planned.txt). The planned cards are copied into WORK/cards, rewritten only where Rust leaves no other way, and compiled
in parallel shards with `planned!(id => code)` as `code`. Every name rustc cannot find is stubbed in WORK/stubs.rs with
the constraints of where it would live in Cantrip, and the cards compiled again until no new name turns up. What still
fails is a proposal Cantrip could not take in that shape, or a stub's fault, which the report keeps apart.

The constraints come from how Cantrip stores and dispatches, never from the proposal:
- `Game` and `Query` are used as `&mut dyn Game` and `&dyn Query`, so a method on them is concrete: a closure can't reach
  one generically. A stubbed method's arguments are generic, bounded by what it may take.
- A `Game` method that makes or keeps something (an argument is a `Duration` or `EffectKind`, or its name says add,
  create, grant, set, schedule, delay, remember or note) keeps it in the game state, which is `Copy + Eq + Hash` data:
  each argument is `Stored`. Any other `Game` method, and every `Query` method, takes data (`Debug`), read as it runs.
- A new variant of a Cantrip enum is a constructor of that enum whose payload has what the enum derives.
- A new field of a Cantrip struct holds what the struct derives; of a table (`CardDef`, `Cost`, ...: not `Hash`), what a
  static holds, `Copy + Sync + 'static`, so anything the engine calls later is a `fn`.
- A free function (`cantrip_mechanics`, a `CardDef` helper) is called statically, so it may be generic; one that makes
  or keeps something takes `Stored` data besides the game itself.
- A struct of the proposal's own holds what a table holds: a function its literal names is a `fn` pointer of that
  function's signature, a string a `&'static str`, a variant its enum; a field nothing settles stays generic.
- A function the card defines, given by name to a method that takes data, is its `fn` pointer, as Cantrip's method
  would take it (`AbilityDef::condition`); never to one that keeps it in the game state, which holds no functions.
- A bare `None` given to a stub's generic argument, or to a field nothing else settles, is the stub's to type.

All 5,289 planned cards take about 90 s (four passes of 48 shards); a handful, about 2 s.
"""
import argparse
import collections
import json
import os
import re
import shlex
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

CRATE = "crates/cantrip-generated"
SOURCES = ["crates/cantrip-api/src", "crates/cantrip-state/src", "crates/cantrip-mechanics/src"]
STORING = {"add", "create", "grant", "set", "schedule", "delay", "remember", "note"}
BOOLISH = re.compile(r"^(is|has|can|was|were|did|does|would|must|may|should)_")
# What a free function takes as the game, not as data (Cantrip's mechanics take `g: &mut dyn Game` or `q: &dyn Query`).
HANDLES = {"g", "q", "game", "*g", "&*g", "&mut *g", "&**g", "&mut **g"}
DERIVABLE = ["Clone", "Copy", "Debug", "PartialEq", "Eq", "Hash", "PartialOrd", "Ord"]
STATE = ["Clone", "Copy", "Debug", "PartialEq", "Eq", "Hash"]
PANIC = "proposal stub: check only"
PANIC_RS = json.dumps(PANIC)
ROOT = "crate::proposals"


def log(*parts):
    print(*parts, file=sys.stderr, flush=True)


# ---------------------------------------------------------------------------------------------------------------------
# Rust text


def skip_string(text, i):
    """The index after the string or char literal at `i`, or `i` if there is none."""
    c = text[i]
    if c == '"':
        j = i + 1
        while j < len(text) and text[j] != '"':
            j += 2 if text[j] == "\\" else 1
        return j + 1
    if c == "r" and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] == "_")):
        m = re.compile(r'r(#*)"').match(text, i)
        if m:
            end = text.find('"' + m.group(1), m.end())
            return len(text) if end < 0 else end + 1 + len(m.group(1))
    if c == "'":
        m = re.compile(r"'(\\.|\\u\{[0-9a-fA-F]+\}|[^\\'])'").match(text, i)
        if m:
            return m.end()
    return i


# Text no bracket, string or char literal starts in (an `r` may start a raw string, but only after a non-word char).
BORING = re.compile(r"(?:[^\"'()\[\]{}r]|(?<=\w)r)*")
SPLIT_BORING = re.compile(r"(?:[^\"'()\[\]{}r:<>|,]|(?<=\w)r)*")


def balanced(text, i):
    """The index after the bracket group that opens at `i`."""
    pairs = {"(": ")", "[": "]", "{": "}"}
    stack = []
    j = i
    while j < len(text):
        j = BORING.match(text, j).end()
        if j >= len(text):
            break
        k = skip_string(text, j)
        if k != j:
            j = k
            continue
        c = text[j]
        if c in pairs:
            stack.append(pairs[c])
        elif c in ")]}":
            if not stack or stack.pop() != c or not stack:
                return j + 1
        j += 1
    return j


def split_args(inner):
    """The top-level comma-separated parts of `inner`, the text between a call's parentheses or a literal's braces."""
    # `ret`: in a closure's return type (`-> Result<(), Suspend>`), whose `<` opens generics too.
    parts, angle, start, j, ret = [], 0, 0, 0, False
    while j < len(inner):
        j = SPLIT_BORING.match(inner, j).end()
        if j >= len(inner):
            break
        k = skip_string(inner, j)
        if k != j:
            j = k
            continue
        c = inner[j]
        if c in "([{":
            if c == "{":
                ret = False
            j = balanced(inner, j)
            continue
        if inner.startswith("::<", j):
            angle += 1
            j += 3
            continue
        if c == ">" and inner[j - 1] == "-":
            ret = True
        elif (angle or ret) and c == "<":
            angle += 1
        elif angle and c == ">":
            angle -= 1
        elif c == "|" and inner.startswith("||", j):
            j += 2
            continue
        elif c == "|" and re.search(r"(^|[^:]:|=|\bmove|\()\s*$", inner[start:j]):
            # A closure's parameters, as an argument or a field's value: their commas are not the call's.
            end = inner.find("|", j + 1)
            j = len(inner) if end < 0 else end + 1
            continue
        elif c == "," and not angle:
            parts.append(inner[start:j])
            start = j + 1
        j += 1
    if inner[start:].strip():
        parts.append(inner[start:])
    return [p.strip() for p in parts]


def after(text, at):
    """How the path ending at `at` is used: ("call", args), ("struct", fields), ("path", segment, end) or ("bare",)."""
    m = re.compile(r"\s*(::<)?").match(text, at)
    j = m.end()
    if m.group(1):
        depth = 1
        while depth and j < len(text):
            depth += {"<": 1, ">": -1}.get(text[j], 0)
            j += 1
        j = re.compile(r"\s*").match(text, j).end()
    if j < len(text) and text[j] == "(":
        close = balanced(text, j)
        # Followed by `?`: it returns a `Result`, as Cantrip's functions that may ask a player do.
        tried = bool(re.compile(r"\s*\?").match(text, close))
        return ("call", split_args(text[j + 1 : close - 1]), tried)
    if j < len(text) and text[j] == "{":
        body = text[j + 1 : balanced(text, j) - 1]
        # A literal's or pattern's fields, not a block after a condition.
        if not body.strip() or re.match(r"\s*(\.\.|(ref\s+|mut\s+)*\w+\s*(:(?!:)|,|$))", body):
            return ("struct", fields_of(body))
    m = re.compile(r"::(\w+)").match(text, at)
    if m:
        return ("path", m.group(1), m.end())
    return ("bare",)


def fields_of(body):
    """The field names a struct literal or pattern body names."""
    names = []
    for part in split_args(body):
        m = re.match(r"(?:ref\s+)?(?:mut\s+)?(\w+)\s*(?::|$)", part)
        if m and not part.startswith(".."):
            names.append(m.group(1))
    return names


def statement_after(text, at):
    """Whether the call whose name ends at `at` is a whole statement: nothing reads its value."""
    use = re.compile(r"\s*(?:::<[^(]*>)?\s*\(").match(text, at)
    if not use:
        return False
    end = balanced(text, use.end() - 1)
    return bool(re.compile(r"\s*(;|\}|$)").match(text, end))


def camel(name):
    return bool(re.match(r"[A-Z][a-z0-9]", name)) or (len(name) == 1 and name.isupper())


def caps(name):
    return bool(re.match(r"[A-Z][A-Z0-9_]*$", name)) and len(name) > 1


# ---------------------------------------------------------------------------------------------------------------------
# What Cantrip has


class Model:
    """Cantrip's types with their derives, each enum's unit variants and struct-variant field types, the fields of its
    structs, its traits and its mechanics modules."""

    def __init__(self, root):
        self.derives, self.kinds, self.units, self.lifetimes, self.homes = {}, {}, {}, {}, {}
        self.fields = collections.defaultdict(dict)
        self.variant_fields = collections.defaultdict(lambda: collections.defaultdict(collections.Counter))
        self.variants = collections.defaultdict(dict)  # enum -> struct variant -> field -> type
        self.any_field = collections.defaultdict(collections.Counter)
        self.traits, self.modules = set(), set()
        used = collections.defaultdict(collections.Counter)
        self_consts = {}
        item = re.compile(r"((?:#\[[^\]]*\]\s*)*)pub (enum|struct) (\w+)(<[^>{]*>)?\s*(\{|\(|;)")
        for base in SOURCES:
            for path in sorted((Path(root) / base).rglob("*.rs")):
                text = path.read_text(encoding="utf-8")
                code = re.sub(r"//[^\n]*", "", text)
                # A mechanics type is named by its module: cantrip_api re-exports only its own and the state's.
                rel = path.relative_to(Path(root) / base).with_suffix("").parts
                home = ("cantrip_mechanics::" + "::".join(p for p in rel if p not in ("lib", "mod"))).rstrip(":")
                for m in item.finditer(code):
                    kind, name = m.group(2), m.group(3)
                    if base.startswith("crates/cantrip-mechanics"):
                        self.homes.setdefault(name, home)
                    derive = re.search(r"#\[derive\(([^)]*)\)\]", m.group(1))
                    self.derives.setdefault(name, [d.strip() for d in derive.group(1).split(",")] if derive else [])
                    self.kinds.setdefault(name, kind)
                    self.lifetimes.setdefault(name, len(re.findall(r"'\w+", m.group(4) or "")))
                    if m.group(5) != "{":
                        continue
                    body = code[m.end() : balanced(code, m.end() - 1) - 1]
                    if kind == "struct":
                        for f in re.finditer(r"^\s*pub (\w+): (.+?),?\s*$", body, re.M):
                            self.fields[name].setdefault(f.group(1), f.group(2))
                        continue
                    body = re.sub(r"#\[[^\]]*\]", "", body)
                    for entry in split_args(body):
                        v = re.match(r"(\w+)\s*(\{|\(|=|$)", entry)
                        if not v:
                            continue
                        if v.group(2) in ("", "="):
                            self.units.setdefault(name, v.group(1))
                        elif v.group(2) == "{":
                            inner = entry[v.end() : entry.rfind("}")]
                            for f in split_args(inner):
                                t = re.match(r"(\w+)\s*:\s*(.+)$", f, re.S)
                                if t:
                                    self.variant_fields[name][t.group(1)][t.group(2).strip()] += 1
                                    self.any_field[t.group(1)][t.group(2).strip()] += 1
                                    self.variants[name].setdefault(v.group(1), {})[t.group(1)] = t.group(2).strip()
                for m in re.finditer(r"^pub trait (\w+)", code, re.M):
                    self.traits.add(m.group(1))
                for m in re.finditer(r"\b([A-Z]\w*)::([A-Z][a-z]\w*)\b(?!\s*[({:])", code):
                    used[m.group(1)][m.group(2)] += 1
                for m in re.finditer(r"^impl (\w+) \{", code, re.M):
                    block = code[m.end() - 1 : balanced(code, m.end() - 1)]
                    for c in re.finditer(r"pub const (\w+): (?:Self|" + m.group(1) + r")\b", block):
                        self_consts.setdefault(m.group(1), c.group(1))
        lib = Path(root) / "crates/cantrip-mechanics/src/lib.rs"
        self.modules = set(re.findall(r"^pub mod (\w+)", lib.read_text(encoding="utf-8"), re.M))
        self.struct_fields = {f for fields in self.fields.values() for f in fields}
        # A value of the type, for a stub that must give one: an enum's first unit variant (a macro's enum: the variant
        # Cantrip names most), a struct's first constant of its own type.
        self.value = dict(self.units)
        for name, counter in used.items():
            if name not in self.value and self.kinds.get(name) == "enum" and counter:
                self.value[name] = counter.most_common(1)[0][0]
        for name, const in self_consts.items():
            self.value.setdefault(name, const)
        # Types whose associated constants are of the type itself (`EventMask::SPELL_CAST`, `ManaCost::ZERO`).
        self.own_consts = set(self_consts)

    def known(self, name):
        return name in self.kinds

    def in_card_def(self, name):
        """Whether a field of `CardDef` holds `name` (`abilities: &'static [AbilityDef]`)."""
        return any(re.search(rf"\b{name}\b", ty) for ty in self.fields.get("CardDef", {}).values())

    def storage(self, name):
        """What a value inside `name` must have: its derives, or a table's `Copy + Sync + 'static` if it isn't `Hash`."""
        derives = [d for d in self.derives.get(name, []) if d in DERIVABLE]
        return derives if "Hash" in derives else None

    def field_type(self, enum, field, fields=()):
        """The type of `field` in a new struct variant of `enum` with `fields`: as the variant of `enum` sharing the most
        of them types it (`CountersRemoved { object, kind, amount }`: `CountersPut`'s `kind`, not `DamageDealt`'s), or
        else as most of `enum`'s variants, or any enum, type a field of that name."""
        alike = [(len(set(fields) & set(have)), have) for have in self.variants.get(enum, {}).values() if field in have]
        if alike and max(alike, key=lambda a: a[0])[0] > 1:
            return max(alike, key=lambda a: a[0])[1][field]
        for counter in (self.variant_fields.get(enum, {}).get(field), self.any_field.get(field)):
            if counter:
                return counter.most_common(1)[0][0]
        return None


# ---------------------------------------------------------------------------------------------------------------------
# What the cards propose


def base_of(ty):
    """("dyn", Trait) or ("type", Name) for a receiver type rustc names, else None."""
    ty = re.sub(r"^(&(mut )?)+", "", ty.strip())
    m = re.match(r"\(?dyn ([\w:]+)", ty)
    if m:
        return ("dyn", m.group(1).split("::")[-1])
    m = re.match(r"([\w:]+)", ty)
    return ("type", m.group(1).split("::")[-1]) if m else None


def new_use():
    return {"consts": set(), "ctors": {}, "structs": {}, "fns": {}, "literal": set(), "bare": False, "module": "",
            "typed": collections.defaultdict(collections.Counter)}


def fn_pointer(text, name):
    """The `fn` pointer type of the function `name` the file defines, or None."""
    m = re.search(rf"\bfn {re.escape(name)}\s*\(", text)
    if not m:
        return None
    close = balanced(text, m.end() - 1)
    params = []
    for part in split_args(text[m.end() : close - 1]):
        p = re.match(r"(?:mut\s+)?\w+\s*:\s*(.+)$", part, re.S)
        if not p:
            return None
        params.append(re.sub(r"\s+", " ", p.group(1).strip()))
    ret = re.compile(r"\s*->\s*([^{]+?)\s*(?:where\b|\{)").match(text, close)
    return f"fn({', '.join(params)})" + (f" -> {ret.group(1)}" if ret else "")


def value_type(text, value):
    """The type a value of a table entry has as Cantrip's tables hold it: a function named is a `fn` pointer (what the
    engine calls later), a string a `&'static str`, a variant its enum, a literal number nothing (left generic)."""
    value = value.strip()
    if re.fullmatch(r'"(?:[^"\\]|\\.)*"', value):
        return "&'static str"
    if value in ("true", "false"):
        return "bool"
    m = re.fullmatch(r"(?:[\w:]*::)?([A-Z]\w*)::[A-Z]\w*(?:\s*\(.*\))?", value, re.S)
    if m:
        return m.group(1)
    if re.fullmatch(r"[a-z_]\w*", value):
        return fn_pointer(text, value)
    return None


class Spec:
    """Every name the cards use that Cantrip doesn't have, with how they use it: what the stubs are written from."""

    def __init__(self, model):
        self.model = model
        self.methods = {}  # name -> {"bases", "arity", "stores", "query"}
        self.fns = {}  # (module, name) -> {"arity", "handles"}
        self.types = {}  # name -> use, for a type Cantrip doesn't have (module "" or "mechanics::m")
        self.on = {}  # name -> use, for new items of a type Cantrip has
        self.new_fields = collections.defaultdict(set)  # struct -> fields a literal gives that it doesn't have
        self.read_fields = collections.defaultdict(set)  # struct -> fields read that it doesn't have
        self.variant_fields = collections.defaultdict(set)  # (enum, variant) -> fields a pattern binds it doesn't have
        self.missing_modules, self.module_items = set(), set()
        self.rets = {}  # stub key -> the return type chosen where no call site says it
        self.unknown_rets = set()

    # Learning from rustc

    def learn(self, diags, texts):
        """Add what `diags` say is missing; True if anything is new."""
        before = self.fingerprint()
        self._rules = None
        self._data_call = False
        for d in diags:
            span = primary(d)
            if not span or span["file_name"].endswith("stubs.rs"):
                continue
            text = texts(span["file_name"])
            if text is None:
                continue
            self.learn_one(d, (d.get("code") or {}).get("code"), d["message"], text, span["byte_start"], span["byte_end"])
        return self.fingerprint() != before

    def size(self):
        return (f"{len(self.methods)} methods, {len(self.fns)} functions, {len(self.types)} types, {len(self.on)} "
                f"extended, {sum(map(len, self.new_fields.values()))} new fields, "
                f"{sum(map(len, self.read_fields.values()))} read fields, "
                f"{sum(map(len, self.variant_fields.values()))} variant fields, {len(self.rets)} returns chosen")

    def fingerprint(self):
        """What the stubs and rewrites would be: a pass that changes neither learnt nothing (counts of uses may grow)."""
        return self.write() + repr(self.rules())

    def learn_one(self, d, code, msg, text, start, end):
        m = re.match(r"no method named `(\w+)` found for .*?`(.+)` in the current scope", msg)
        if code == "E0599" and m:
            base = base_of(m.group(2))
            if base and (base[1] in self.model.traits or self.model.known(base[1]) or base[1] in self.types):
                use = after(text, end)
                args = use[1] if use[0] == "call" else []
                entry = self.methods.setdefault(m.group(1), {"bases": set(), "arity": collections.Counter(),
                                                             "stores": False, "tried": collections.Counter()})
                entry["bases"].add(base)
                entry["arity"][len(args)] += 1
                entry["tried"][use[0] == "call" and use[2]] += 1
                entry["stores"] |= any(re.search(r"\b(Duration|EffectKind)::", a) for a in args)
            return
        m = re.match(r"no (?:variant|associated item|function|[\w ,]+?) named `(\w+)` found for (?:enum|struct) `([^`]+)`", msg)
        if code == "E0599" and m:
            path = re.sub(r"<.*", "", m.group(2))
            ty = path.split("::")[-1]
            if self.model.known(ty):
                use = self.on.setdefault(ty, new_use())
                if path.startswith("cantrip_mechanics::") or not use["module"]:
                    use["module"] = path if path.startswith("cantrip_mechanics::") else (
                        f"{self.model.homes[ty]}::{ty}" if ty in self.model.homes else ty)
                self.use_item(use, m.group(1), after(text, end))
            elif ty in self.types:
                self.use_item(self.types[ty], m.group(1), after(text, end))
            return
        m = re.match(r"(?:cannot find type|cannot find struct, variant or union type|failed to resolve: use of undeclared "
                     r"type) `(\w+)` in this scope|failed to resolve: use of undeclared type `(\w+)`", msg)
        if code in ("E0412", "E0422", "E0433") and m:
            name = m.group(1) or m.group(2)
            self.use_type(self.types.setdefault(name, new_use()), text, end)
            return
        m = re.match(r"cannot find function `(\w+)` in this scope", msg)
        if code == "E0425" and m:
            self.use_fn("", m.group(1), text, end)
            return
        m = re.match(r"(?:failed to resolve: )?(?:could not find|cannot find) (?:[\w ,]+ )?`(\w+)` in (?:module )?`([\w:]+)`", msg)
        if code in ("E0433", "E0425", "E0412", "E0422", "E0423", "E0531", "E0532") and m:
            self.use_path(text, start, end)
            return
        m = re.match(r"struct `([^`]+)` has no field named `(\w+)`", msg)
        if code == "E0560" and m:
            ty = m.group(1).split("::")[-1]
            if self.model.known(ty):
                self.new_fields[ty].add(m.group(2))
            elif ty in self.types:
                self.types[ty]["literal"].add(m.group(2))
            return
        m = re.match(r"variant `(?:[\w:]*::)?(\w+)` does not have a field named `(\w+)`", msg)
        if code == "E0026" and m:
            # The enum is the one the pattern names before the variant.
            path = list(re.finditer(rf"(\w+)::{m.group(1)}\s*\{{", text[max(0, start - 2000) : start]))
            if path and self.model.known(path[-1].group(1)):
                self.variant_fields[(path[-1].group(1), m.group(1))].add(m.group(2))
            return
        m = re.match(r"no field `(\w+)` on type `([^`]+)`", msg)
        if code == "E0609" and m:
            base = base_of(m.group(2))
            if base and self.model.known(base[1]) and m.group(1) not in self.model.struct_fields:
                self.read_fields[base[1]].add(m.group(1))
            return
        if code in ("E0282", "E0283", "E0284", "E0790"):
            self.learn_return(text, start, end)

    def use_item(self, use, name, how):
        if how[0] == "call":
            use["fns" if not camel(name) else "ctors"].setdefault(name, collections.Counter())[len(how[1])] += 1
            if how[2] and not camel(name):
                use.setdefault("tried", set()).add(name)
        elif how[0] == "struct":
            use["structs"].setdefault(name, set()).update(how[1])
        elif camel(name):
            use["consts"].add(name)
        else:
            # A constant whose type nothing says (`AbilityRef::GRANTED`) or a function named without a call: a
            # function of no arguments, its value inferred where it stands, and the path called.
            use.setdefault("values", set()).add(name)

    def use_type(self, use, text, end):
        how = after(text, end)
        if how[0] == "path":
            self.use_item(use, how[1], after(text, how[2]))
        elif how[0] == "struct":
            use["literal"].update(how[1])
            self.type_fields(use, text, end)
        else:
            use["bare"] = True

    def type_fields(self, use, text, end):
        """What each field of a proposed struct's literal holds, as `value_type` reads it."""
        at = re.compile(r"\s*\{").match(text, end)
        if not at:
            return
        body = text[at.end() : balanced(text, at.end() - 1) - 1]
        for part in split_args(body):
            f = re.match(r"(\w+)\s*:\s*(.+)$", part, re.S)
            if f:
                use["typed"][f.group(1)][value_type(text, f.group(2))] += 1

    def field_types(self, use):
        """Each field of a proposed struct and its type, None where its values leave it generic."""
        out = {}
        for f in sorted(use["literal"]):
            # A path's type counts once Cantrip or the proposal has it.
            typed = [(n, ty) for ty, n in use["typed"].get(f, {}).items()
                     if ty and (not re.fullmatch(r"[A-Z]\w*", ty) or self.model.known(ty) or ty in self.types)]
            out[f] = max(typed)[1] if typed else None
        return out

    def use_fn(self, module, name, text, end):
        how = after(text, end)
        args = how[1] if how[0] == "call" else []
        entry = self.fns.setdefault((module, name), {"arity": collections.Counter(), "handles": {},
                                                     "tried": collections.Counter(), "value": False})
        if how[0] != "call":
            entry["value"] = True
            return
        entry["arity"][len(args)] += 1
        entry["tried"][how[2]] += 1
        for i, a in enumerate(args):
            if a in HANDLES:
                kind = "mut" if a in ("g", "game", "&mut *g", "&mut **g") else "ref"
                entry["handles"].setdefault(i, collections.Counter())[kind] += 1

    def use_path(self, text, start, end):
        """A path into `cantrip_mechanics` that names a module or item it doesn't have."""
        head = re.search(r"([\w:]*)$", text[:start]).group(1)
        ident = re.compile(r"\w+(?:::\w+)*").match(text, start)
        if not ident:
            return
        end = ident.end()
        tail = re.compile(r"(?:::\w+)*").match(text, end)
        segments = (head + text[start:end] + tail.group(0)).split("::")
        for root in (["cantrip_mechanics"], ["crate", "proposals", "mechanics"]):
            if segments[: len(root)] == root:
                rest = segments[len(root):]
                break
        else:
            return
        if not rest:
            return
        if rest[0] not in self.model.modules:
            self.missing_modules.add(rest[0])
        elif len(rest) > 1:
            self.module_items.add((rest[0], rest[1]))
        modules = []
        for i, seg in enumerate(rest):
            if camel(seg) or caps(seg) or i == len(rest) - 1:
                break
            modules.append(seg)
        module = "mechanics::" + "::".join(modules) if modules else "mechanics"
        item = rest[len(modules)] if len(modules) < len(rest) else None
        if item is None:
            return
        # Where the item's name ends in the text.
        item_end = start - len(head) + len("::".join(segments[: len(root) + len(modules) + 1]))
        if camel(item):
            use = self.types.setdefault(item, new_use())
            use["module"] = module
            self.use_type(use, text, item_end)
        else:
            self.use_fn(module, item, text, item_end)

    def learn_return(self, text, start, end):
        """A stub whose return type the call site doesn't settle: `bool` for a question, `()` for a statement."""
        for m in re.finditer(r"(\w+)\s*(?:::<[^(]*>)?\s*\(", text[start : max(end, start + 200)]):
            name = m.group(1)
            keys = [k for k in self.stub_keys() if k.split("::")[-1] == name]
            if not keys:
                continue
            at = start + m.start(1) + len(name)
            for key in keys:
                if key in self.rets:
                    continue
                if BOOLISH.match(name):
                    self.rets[key] = "bool"
                elif statement_after(text, at):
                    self.rets[key] = "()"
                else:
                    self.unknown_rets.add(key)
            return

    def stub_keys(self):
        keys = [f"m::{n}" for n in self.methods]
        keys += [f"f::{m}::{n}" for m, n in self.fns]
        for owner, uses in (("on", self.on), ("t", self.types)):
            for ty, use in uses.items():
                keys += [f"{owner}::{ty}::{n}" for n in list(use["fns"]) + list(use["ctors"])]
        return keys

    # Writing the stubs

    def derives_trait(self, ty, derives, out):
        name = f"Derives_{ty}"
        bounds = " + ".join(f"std::fmt::{d}" if d == "Debug" else f"std::hash::{d}" if d == "Hash" else d
                            for d in derives) or "Sized"
        out.append(f"/// What `{ty}` derives, so what a value inside it must have.\n"
                   f"pub trait {name}: {bounds} + 'static {{}}\nimpl<T: {bounds} + 'static> {name} for T {{}}")
        return name

    def payload(self, ty, out, seen):
        """What a value inside `ty` must have. A Cantrip type: what it derives (a table's: `TableEntry`). A type of
        the proposal's own: nothing says where it will live, so the least of Cantrip's two homes, `TableEntry`."""
        if ty not in seen:
            storage = self.model.storage(ty) if self.model.known(ty) else None
            seen[ty] = self.derives_trait(ty, storage, out) if storage else "TableEntry"
        return f"{ROOT}::{seen[ty]}"

    def ret(self, key, tried=None):
        """The return type of a stub: generic `R` unless a call site left it unsettled; a `Result` of it when most of
        its calls are followed by `?`."""
        base = self.rets.get(key)
        if tried and tried[True] > tried[False]:
            return f"Result<{base or 'R'}, Suspend>", base is None
        return base or "R", base is None

    def write(self):
        out, seen = [PRELUDE], {}
        # Methods, one trait for each type they are called on.
        by_base = collections.defaultdict(list)
        for name, entry in sorted(self.methods.items()):
            for base in entry["bases"]:
                by_base[base].append(name)
        for base, names in sorted(by_base.items()):
            kind, ty = base
            lines = []
            for name in names:
                entry = self.methods[name]
                query = any(k == "dyn" and t != "Game" for k, t in entry["bases"])
                stores = self.method_stores(name, ty)
                bound = "Stored" if stores else "Data"
                # A method of a value a CardDef field holds (`ability(..).divide(..)`) builds that table entry: what
                # it is given is held in the table, as a field of it would be.
                if kind != "dyn" and self.model.in_card_def(ty):
                    bound = self.payload(ty, out, seen)[len(ROOT) + 2 :]
                receiver = "&mut self" if ty == "Game" and not query else "&self"
                arity = entry["arity"].most_common(1)[0][0]
                ret, generic = self.ret(f"m::{name}", entry.get("tried"))
                params = [f"A{i}: {ROOT}::{bound}" for i in range(arity)] + (["R"] if generic else [])
                args = "".join(f", a{i}: A{i}" for i in range(arity))
                what = ("keeps what it is given in the game state" if stores else
                        "reads what it is given as it runs")
                lines.append(f"    /// Proposed: {what}.\n    fn {name}<{', '.join(params)}>({receiver}{args}) -> "
                             f"{ret} {{ panic!({PANIC_RS}) }}")
            trait = f"Proposed_{ty}"
            target = f"dyn {ty} + '_" if kind == "dyn" else self.type_path(ty)
            out.append(f"pub trait {trait} {{\n" + "\n".join(lines) + f"\n}}\nimpl{self.generics(ty)} {trait} for {target} {{}}")
        # Free functions, in the crate root (`cantrip_api`'s helpers) or under `mechanics`.
        modules = collections.defaultdict(list)
        for (module, name), entry in sorted(self.fns.items()):
            modules[module].append(self.free_fn(module, name, entry))
        for name, use in sorted(self.types.items()):
            modules[use["module"]].append(self.new_type(name, use, out, seen))
        for module in sorted(set(f"mechanics::{m}" for m in self.missing_modules)
                             | set(f"mechanics::{m}" for m, _ in self.module_items)):
            modules.setdefault(module, [])
        root = modules.pop("", [])
        out += root
        tree = {}
        for module, items in modules.items():
            node = tree
            for seg in module.split("::"):
                node = node.setdefault(seg, {})
            node.setdefault("", []).extend(items)
        out.append(self.module_tree(tree))
        # New items of Cantrip's own types, under `on::Type`.
        ons = []
        for ty, use in sorted(self.on.items()):
            ons.append(self.on_type(ty, use, out, seen))
        for ty, use in sorted(self.types.items()):
            if use["structs"]:
                ons.append(self.on_type(ty, use, out, seen, new=True))
        out.append("pub mod on {\n" + "\n".join(ons) + "\n}")
        # New fields.
        fields = []
        for ty, names in sorted(self.new_fields.items()):
            bound = self.payload(ty, out, seen)
            fns = "\n".join(f"    /// Proposed: a field of `{ty}`.\n    pub const fn {f}<V: {bound}>(v: V) {{ "
                            f"core::mem::forget(v) }}" for f in sorted(names))
            fields.append(f"pub mod {ty} {{\n{fns}\n}}")
        out.append("pub mod field {\n" + "\n".join(fields) + "\n}")
        for ty, names in sorted(self.read_fields.items()):
            bound = self.payload(ty, out, seen)
            fns = "\n".join(f"    /// Proposed: a field of `{ty}`.\n    fn {f}<R: {bound}>(&self) -> R {{ panic!({PANIC_RS}) }}\n"
                            f"    fn {f}__set<V: {bound}>(&mut self, v: V) {{}}" for f in sorted(names))
            out.append(f"pub trait ProposedField_{ty} {{\n{fns}\n}}\nimpl ProposedField_{ty} for {ty} {{}}")
        # New fields of an existing enum's variant, as a pattern binds them: read off the value matched, as the
        # field would be held.
        by_enum = collections.defaultdict(list)
        for (ty, variant), names in sorted(self.variant_fields.items()):
            bound = self.payload(ty, out, seen)
            fns = "\n".join(f"        /// Proposed: a field of `{ty}::{variant}`.\n        pub fn {f}<R: {bound}, S>(s: "
                            f"S) -> &'static R {{ panic!({PANIC_RS}) }}" for f in sorted(names))
            by_enum[ty].append(f"    pub mod {variant} {{\n{fns}\n    }}")
        out.append("pub mod variant_field {\n" + "\n".join(f"pub mod {ty} {{\n" + "\n".join(mods) + "\n}"
                                                       for ty, mods in sorted(by_enum.items())) + "\n}")
        return "\n\n".join(out) + "\n"

    def method_stores(self, name, ty):
        """Whether the stubbed method `name`, called on `ty`, keeps its arguments in the game state."""
        entry = self.methods[name]
        query = any(k == "dyn" and t != "Game" for k, t in entry["bases"])
        return not query and ty == "Game" and (entry["stores"] or bool(STORING & set(name.split("_"))))

    def data_method_call(self):
        """A call of a method `data_methods` names, or None."""
        if getattr(self, "_data_call", False) is False:
            methods = self.data_methods()
            self._data_call = re.compile(rf"\.({'|'.join(sorted(map(re.escape, methods)))})\s*(?:::<[^(]*>)?\s*\(") \
                if methods else None
        return self._data_call

    def data_methods(self):
        """The stubbed methods that take data read as they run, on no receiver that keeps it."""
        return {name for name, entry in self.methods.items()
                if not any(self.method_stores(name, ty) for _, ty in entry["bases"])}

    def type_path(self, ty):
        if ty in self.types:
            module = self.types[ty]["module"]
            return f"{ROOT}::{module + '::' if module else ''}{ty}{self.generics(ty, bare=True)}"
        lifetimes = self.model.lifetimes.get(ty, 0)
        return f"{ty}<{', '.join(["'_"] * lifetimes)}>" if lifetimes else ty

    def generics(self, ty, bare=False):
        use = self.types.get(ty)
        if not use or not self.is_struct(use):
            return ""
        n = sum(ty is None for ty in self.field_types(use).values())
        params = ", ".join(f"T{i}" for i in range(n))
        return f"<{params}>" if params else ""

    @staticmethod
    def is_struct(use):
        return bool(use["literal"]) and not (use["consts"] or use["ctors"] or use["structs"] or use["fns"])

    def free_fn(self, module, name, entry):
        if not entry["arity"]:
            return (f"/// Proposed: a constant, its type what it stands for.\npub const fn {name}<R>() -> R "
                    f"{{ panic!({PANIC_RS}) }}")
        arity = entry["arity"].most_common(1)[0][0]
        calls = sum(entry["arity"].values())
        stores = bool(STORING & set(name.split("_")))
        # The game passed on (`g`, `q`): borrowed, as Cantrip's mechanics take `&mut dyn Game` or `&dyn Query`.
        handles = {i: kinds.most_common(1)[0][0] for i, kinds in entry["handles"].items()
                   if sum(kinds.values()) * 2 >= calls}
        ret, generic = self.ret(f"f::{module}::{name}", entry.get("tried"))
        params, args, forget = [], [], ""
        for i in range(arity):
            if i in handles:
                params.append(f"G{i}: ?Sized")
                args.append(f"a{i}: &{'mut ' if handles[i] == 'mut' else ''}G{i}")
            else:
                params.append(f"A{i}" + (f": {ROOT}::Stored" if stores else ""))
                args.append(f"a{i}: A{i}")
                forget += f"core::mem::forget(a{i}); "
        params += ["R"] if generic else []
        what = "keeps what it is given besides the game" if stores else "called statically: takes anything"
        return (f"/// Proposed: {what}.\npub const fn {name}<{', '.join(params)}>({', '.join(args)}) -> {ret} "
                f"{{ {forget}panic!({PANIC_RS}) }}")

    def module_tree(self, tree, depth=0):
        parts = []
        for name, node in sorted((k, v) for k, v in tree.items() if k):
            inner = "\n".join(node.get("", []))
            prelude = "use cantrip_api::*;\n#[allow(unused_imports)] use crate::proposals::*;\n"
            parts.append(f"pub mod {name} {{\n{prelude}{inner}\n{self.module_tree(node, depth + 1)}\n}}")
        return "\n".join(parts)

    def new_type(self, name, use, out, seen):
        derive = "#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]"
        if self.is_struct(use):
            # A field holds what its values are as a table holds them (`value_type`); one they don't settle is generic.
            # It derives what its fields all have: a generic field's derive asks it of what fills it.
            params, body, has = [], [], set(STATE)
            for f, ty in self.field_types(use).items():
                if ty is None:
                    ty = f"T{len(params)}"
                    params.append(ty)
                elif self.model.known(ty):
                    has &= set(self.model.derives.get(ty, []))
                body.append(f"pub {f}: {ty}")
            gen = f"<{', '.join(params)}>" if params else ""
            derive = f"#[derive({', '.join(d for d in STATE if d in has)})]"
            return f"/// Proposed: a struct.\n{derive}\npub struct {name}{gen} {{ {', '.join(body)} }}"
        if not (use["consts"] or use["ctors"] or use["structs"] or use["fns"] or use.get("values")):
            return f"/// Proposed: a type.\n{derive}\npub struct {name};"
        bound = self.payload(name, out, seen)
        units = sorted(use["consts"])
        items = [self.value_fn(v) for v in sorted(use.get("values", set()) - set(use["fns"]))]
        for ctor, arity in sorted(use["ctors"].items()):
            n = arity.most_common(1)[0][0]
            params = ", ".join(f"A{i}: {bound}" for i in range(n))
            args = ", ".join(f"a{i}: A{i}" for i in range(n))
            forget = "".join(f"core::mem::forget(a{i}); " for i in range(n))
            items.append(f"    pub const fn {ctor}<{params}>({args}) -> Self {{ {forget}Self::__Proposed }}")
        for fn, arity in sorted(use["fns"].items()):
            items.append("    " + self.assoc_fn(f"t::{name}::{fn}", fn, arity, name in use.get("tried", ())))
        variants = ", ".join(["__Proposed"] + units)
        return f"/// Proposed: an enum.\n{derive}\npub enum {name} {{ {variants} }}\nimpl {name} {{\n" + "\n".join(items) + "\n}"

    @staticmethod
    def value_fn(name):
        return f"    /// Proposed: a constant, its type what it stands for.\n    pub const fn {name}<R>() -> R {{ panic!({PANIC_RS}) }}"

    def assoc_fn(self, key, name, arity, tried=False):
        n = arity.most_common(1)[0][0]
        ret, generic = self.ret(key, collections.Counter({True: 1}) if tried else None)
        params = [f"A{i}" for i in range(n)] + (["R"] if generic else [])
        args = ", ".join(f"a{i}: A{i}" for i in range(n))
        forget = "".join(f"core::mem::forget(a{i}); " for i in range(n))
        return f"pub const fn {name}<{', '.join(params)}>({args}) -> {ret} {{ {forget}panic!({PANIC_RS}) }}"

    def value_of(self, ty, path, new):
        if new:
            return f"{path}::__Proposed"
        value = self.model.value.get(ty)
        return f"{path}::{value}" if value else f"panic!({PANIC_RS})"

    def on_type(self, ty, use, out, seen, new=False):
        """`on::Type`: new variants and associated items of `ty`, as constructors and constants of it."""
        path = self.type_path(ty) if new else (use["module"] or ty)
        bound = self.payload(ty, out, seen)
        value = self.value_of(ty, path, new)
        items = []
        if not new:
            for c in sorted(use["consts"] - set(use["fns"]) - set(use["ctors"])):
                items.append(f"    pub const {c}: {path} = {value};")
            for v in sorted(self.values(ty, use)):
                items.append(self.value_fn(v))
            for c in sorted(self.own_consts(ty, use)):
                items.append(f"    /// Proposed: a constant of `{ty}`, as its others are.\n    pub const {c}: {path} = {value};")
            for ctor, arity in sorted(use["ctors"].items()):
                n = arity.most_common(1)[0][0]
                params = ", ".join(f"A{i}: {bound}" for i in range(n))
                args = ", ".join(f"a{i}: A{i}" for i in range(n))
                forget = "".join(f"core::mem::forget(a{i}); " for i in range(n))
                items.append(f"    /// Proposed: a variant of `{ty}`.\n    pub const fn {ctor}<{params}>({args}) -> {path} "
                             f"{{ {forget}{value} }}")
            for fn, arity in sorted(use["fns"].items()):
                items.append("    " + self.assoc_fn(f"on::{ty}::{fn}", fn, arity, fn in use.get("tried", ())))
        for variant, fields in sorted(use["structs"].items()):
            fields = sorted(fields)
            typed, params = [], []
            for f in fields:
                known = self.model.field_type(ty, f, fields)
                if known:
                    typed.append(f"pub {f}: {known}")
                else:
                    typed.append(f"pub {f}: F{len(params)}")
                    params.append(f"F{len(params)}")
            gen = f"<{', '.join(params)}>" if params else ""
            all_params = ", ".join(f"F{i}" for i in range(len(fields)))
            bounded = ", ".join(f"F{i}: {bound}" for i in range(len(fields)))
            new_fields = ", ".join(f"pub {f}: F{i}" for i, f in enumerate(fields))
            items.append(
                f"    /// Proposed: the variant `{ty}::{variant}`, as a pattern reads it (its fields typed as `{ty}`'s "
                f"other variants type them).\n"
                f"    pub struct {variant}{gen} {{ {', '.join(typed)} }}\n"
                f"    impl{gen} {variant}{gen} {{ pub fn of<S>(_: S) -> &'static Self {{ panic!({PANIC_RS}) }} }}\n"
                f"    /// The variant as an expression makes it: its fields must have what `{ty}` derives.\n"
                f"    pub struct {variant}_new<{all_params}> {{ {new_fields} }}\n"
                f"    impl<{bounded}> {variant}_new<{all_params}> {{ pub const fn make(self) -> {path} "
                f"{{ core::mem::forget(self); {value} }} }}"
            )
        prelude = "    use cantrip_api::*;\n"
        return f"pub mod {ty} {{\n{prelude}" + "\n".join(items) + "\n}"

    # Rewriting the check copy

    def rules(self):
        """Regex rewrites of the paths Rust gives no way to stub where they stand: (text a file must hold for the rule
        to apply, pattern, replacement)."""
        rules = []
        called = r"(?!\s*(?:\(|::|<))"
        for ty, use in sorted(self.on.items()):
            values = self.values(ty, use)
            names = sorted(use["consts"] | set(use["ctors"]) | set(use["fns"]) | set(use["structs"]) | values
                           | self.own_consts(ty, use))
            if names:
                rules.append((f"{ty}::", rf"(?<![\w:])(?:cantrip_api::|cantrip_mechanics::(?:\w+::)*)?{ty}::"
                              rf"({'|'.join(names)})\b", rf"{ROOT}::on::{ty}::\1"))
            if values:
                rules.append((f"{ty}::", rf"({re.escape(ROOT)}::on::{ty}::(?:{'|'.join(sorted(values))}))\b{called}",
                              r"\1()"))
        for ty, use in sorted(self.types.items()):
            if use["structs"]:
                rules.append((f"{ty}::", rf"(?<![\w])(?:[\w:]*::)?{ty}::({'|'.join(sorted(use['structs']))})\b"
                              rf"(?=\s*\{{)", rf"{ROOT}::on::{ty}::\1"))
            values = use.get("values", set()) - set(use["fns"])
            if values:
                rules.append((f"{ty}::", rf"(\b{ty}::(?:{'|'.join(sorted(values))}))\b{called}", r"\1()"))
        for module in sorted(self.missing_modules):
            rules.append((f"::{module}", rf"\bcantrip_mechanics::{module}\b", rf"{ROOT}::mechanics::{module}"))
        for module, item in sorted(self.module_items):
            rules.append((f"::{item}", rf"\bcantrip_mechanics::{module}::{item}\b",
                          rf"{ROOT}::mechanics::{module}::{item}"))
        values = sorted(name for (module, name), entry in self.fns.items() if module and not entry["arity"])
        if values:
            rules.append(("::mechanics::", rf"({re.escape(ROOT)}::mechanics::(?:\w+::)*(?:{'|'.join(values)}))\b"
                          rf"{called}", r"\1()"))
        fields = sorted({f for names in self.read_fields.values() for f in names})
        if fields:
            alt = "|".join(fields)
            rules.append((".", rf"\.({alt})\s*=(?!=)\s*([^;]*);", r".\1__set(\2);"))
            rules.append((".", rf"\.({alt})\b(?!\s*[(:])", r".\1()"))
        return rules

    def own_consts(self, ty, use):
        """The SCREAMING_CASE items of a Cantrip type whose constants are of the type itself: constants of it."""
        if ty not in self.model.own_consts:
            return set()
        return {v for v in use.get("values", set()) - set(use["fns"]) if caps(v)}

    def values(self, ty, use):
        """Associated items named without a call whose type nothing says: functions of no arguments, called."""
        return use.get("values", set()) - set(use["fns"]) - self.own_consts(ty, use)

    def compiled_rules(self):
        if getattr(self, "_rules", None) is None:
            self._rules = []
            for k, p, r in self.rules():
                try:
                    self._rules.append((k, re.compile(p), r))
                except re.error as e:
                    sys.exit(f"bad rule {p!r}: {e}")
        return self._rules


def primary(d):
    spans = [s for s in d.get("spans", []) if s.get("is_primary")] or d.get("spans", [])
    if not spans:
        return None
    span = spans[0]
    while span.get("expansion") and span["file_name"].startswith("<"):
        span = span["expansion"]["span"]
    return span


# ---------------------------------------------------------------------------------------------------------------------
# The check copy

USE = "use cantrip_api::*;"
PLACEHOLDER_MATCHER = (r"^(?:#\[allow\(unused_variables\)\] )?fn {name}\(q: &dyn Query, asker: &Asker, event: &Event\) "
                       r"-> Option<Fired> \{{ None \}}\n")


def base_rewrite(text, model):
    """The stubs in scope, what a hole replaces taken out, and each entry a hole proposes for a `CardDef` list marked
    with what the list is: a list, another field, or one `CardDef` doesn't have."""
    text = text.replace(USE, USE + " #[allow(unused_imports)] use crate::proposals::*;")
    # A trigger's matcher stands in a hole beside the placeholder that keeps the file compiling: the hole's is the card's.
    for name in re.findall(r'planned_item!\("[^"]*" => \w+: (?:#\[[^\]]*\] )*fn (\w+)\(', text):
        text = re.sub(PLACEHOLDER_MATCHER.format(name=re.escape(name)), "", text, count=1, flags=re.M)

    def mark(m):
        field = m.group(2)
        if m.group(3) in ("#[", "fn "):
            return m.group(0)
        if field not in model.fields["CardDef"]:
            return f"{m.group(1)}@new {field}: {m.group(3)}"
        listed = model.fields["CardDef"][field].startswith("&'static [")
        return f"{m.group(1)}{'' if listed else '@value '}{field}: {m.group(3)}"

    return re.sub(r'(planned_item!\("[^"]*" => )(\w+): (#\[|fn |\S)', mark, text)


def drop_new_fields(text, new_fields):
    """Each struct literal that gives a field its struct doesn't have, as a block that checks the field's value as the
    struct would hold it and then makes the struct without it."""
    for ty, names in new_fields.items():
        if ty not in text or not re.search(rf"\b(?:{'|'.join(sorted(names))})\b\s*[:,}}]", text):
            continue
        starts = [m for m in re.finditer(rf"(?<![\w:])(?:cantrip_api::)?{ty}\s*\{{", text)]
        for m in reversed(starts):
            if re.search(r"(struct|impl|enum)\s+$", text[max(0, m.start() - 10) : m.start()]):
                continue
            open_at = m.end() - 1
            close = balanced(text, open_at)
            parts = split_args(text[open_at + 1 : close - 1])
            kept, checks = [], []
            for part in parts:
                f = re.match(r"(\w+)\s*(?::\s*(.*))?$", part, re.S)
                if f and f.group(1) in names:
                    checks.append(f"{ROOT}::field::{ty}::{f.group(1)}({(f.group(2) or f.group(1)).strip()});")
                else:
                    kept.append(part)
            if checks:
                literal = text[m.start() : open_at] + "{ " + ", ".join(kept) + " }"
                text = text[: m.start()] + "{ " + " ".join(checks) + " " + literal + " }" + text[close:]
    return text


def variant_new_fields(text, variant_fields):
    """`if let Enum::Variant { a, new: p, .. } = s {`, where `new` is a field the variant doesn't have, as
    `if let (p, Enum::Variant { a, .. }) = (variant_field::Enum::Variant::new(&s), s) {`: the field read off the value
    matched, its pattern binding as the field's would."""
    for (ty, variant), names in variant_fields.items():
        if variant not in text:
            continue
        pat = re.compile(rf"\bif\s+let\s+((?:cantrip_api::)?{ty}::{variant}\s*)\{{")
        at = 0
        while m := pat.search(text, at):
            open_at = m.end() - 1
            close = balanced(text, open_at)
            eq = re.compile(r"\s*=(?!=)\s*").match(text, close)
            if not eq:
                at = close
                continue
            # The scrutinee runs to the body's brace.
            depth, end = 0, eq.end()
            while end < len(text) and not (text[end] == "{" and depth == 0):
                depth += 1 if text[end] in "([" else -1 if text[end] in ")]" else 0
                end += 1
            scrutinee = text[eq.end() : end].strip()
            kept, reads = [], []
            for part in split_args(text[open_at + 1 : close - 1]):
                f = re.match(r"(\w+)\s*(?::\s*(.*))?$", part.strip(), re.S)
                if f and f.group(1) in names:
                    reads.append(((f.group(2) or f.group(1)).strip(),
                                  f"{ROOT}::variant_field::{ty}::{variant}::{f.group(1)}(&{scrutinee})"))
                elif part.strip():
                    kept.append(part.strip())
            if not reads:
                at = close
                continue
            if ".." not in kept:
                kept.append("..")
            patterns = ", ".join([p for p, _ in reads] + [f"{m.group(1)}{{ {', '.join(kept)} }}"])
            values = ", ".join([r for _, r in reads] + [scrutinee])
            new = f"if let ({patterns}) = ({values}) "
            text = text[: m.start()] + new + text[end:]
            at = m.start() + len(new)
    return text


def struct_variants(text, spec):
    """A struct variant Cantrip doesn't have, `on::Type::Variant { .. }`: as a pattern, matched against the shim of the
    variant its scrutinee is read as; as an expression, made by `make`, which asks what the type derives of its fields."""
    names = sorted(f"{ty}::{v}" for uses in (spec.on, spec.types) for ty, use in uses.items() for v in use["structs"])
    if not names or f"{ROOT}::on::" not in text:
        return text
    pat = re.compile(rf"{re.escape(ROOT)}::on::(?:{'|'.join(names)})()\s*\{{")
    at = 0
    while True:
        m = pat.search(text, at)
        if not m:
            return text
        close = balanced(text, m.end() - 1)
        before = text[max(0, m.start() - 300) : m.start()]
        is_pattern = re.search(r"\blet\s+$", before) or re.search(r"matches!\([^;]*,\s*$", before) or re.match(
            r"\s*(=>|\|[^|])", text[close:])
        if is_pattern:
            eq = re.compile(r"\s*=(?!=)\s*").match(text, close)
            if eq and re.search(r"\blet\s+$", before):
                j = eq.end()
                while j < len(text) and text[j] != "{":
                    j = balanced(text, j) if text[j] in "([" else j + 1
                scrutinee = text[eq.end() : j].rstrip()
                path = text[m.start() : m.end() - 1].rstrip()
                wrapped = f"{path}::of({scrutinee})"
                text = text[: eq.end()] + wrapped + " " + text[j:]
            at = close
        else:
            name_end = m.start(1)
            text = text[:name_end] + "_new" + text[name_end:close] + ".make()" + text[close:]
            # Inside its braces next: a field may hold another new variant, made the same way.
            at = m.end() + len("_new")


def fn_items_as_pointers(text, pat):
    """Each function the file defines, given by name to a stubbed method that takes data, as its `fn` pointer: what
    Cantrip's method would take (`AbilityDef::condition` takes `fn(&dyn Query, &Asker) -> bool`), so the item coerces
    there, and a pointer is data (`Debug`) where a function item is not. A closure stays as it is."""
    if pat is None:
        return text
    out, at = [], 0
    for m in pat.finditer(text):
        if m.start() < at:
            continue
        close = balanced(text, m.end() - 1)
        args = split_args(text[m.end() : close - 1])
        cast = []
        for a in args:
            pointer = fn_pointer(text, a) if re.fullmatch(r"[a-z_]\w*", a) else None
            cast.append(f"({a} as {pointer})" if pointer else a)
        if cast != args:
            out.append(text[at : m.end()] + ", ".join(cast) + ")")
            at = close
    return "".join(out) + text[at:]


def rewrite(text, model, spec=None):
    text = base_rewrite(text, model)
    if spec is None:
        return text
    for key, old, new in spec.compiled_rules():
        if key in text:
            text = old.sub(new, text)
    text = drop_new_fields(text, spec.new_fields)
    text = variant_new_fields(text, spec.variant_fields)
    text = fn_items_as_pointers(text, spec.data_method_call())
    return struct_variants(text, spec)


# ---------------------------------------------------------------------------------------------------------------------
# Stubs

PRELUDE = f'''//! Stubs of the Cantrip API the planned cards propose, written by mtgish's tools/proposal_check.py: each with the
//! constraints of where it would live in Cantrip, none taken from the proposal. Check only: nothing here runs.
#![allow(non_snake_case, non_upper_case_globals, non_camel_case_types, unused_variables, dead_code, unreachable_code)]
#![allow(clippy::all, unused_imports, unconditional_panic)]
use cantrip_api::*;

pub const PANIC: &str = {PANIC_RS};

/// What Cantrip's game state can keep: `ContinuousEffect`, `EffectKind`, `Duration` and the rest are `Copy + Eq + Hash`
/// data. No closure is (none is `Eq` or `Hash`), nor a `Vec`. Not `'static`: a reference, as Cantrip's own `Game` methods
/// take `item: &StackItem` or `&[ObjectId]`, is read as the call runs; what it points to must be data all the same.
pub trait Stored: Copy + Eq + std::hash::Hash {{}}
impl<T: Copy + Eq + std::hash::Hash> Stored for T {{}}

/// What a method dispatched through `&dyn Query`, or a `Game` method that keeps nothing, can take: data, read as it runs.
/// `Debug` stands for data: every type of Cantrip's state derives it and no closure has it.
pub trait Data: std::fmt::Debug {{}}
impl<T: std::fmt::Debug + ?Sized> Data for T {{}}

/// What a `CardDef` table can hold: a static is `Sync + 'static`, and Cantrip's tables are `Copy` data and `fn` pointers.
pub trait TableEntry: Copy + Sync + 'static {{}}
impl<T: Copy + Sync + 'static> TableEntry for T {{}}

/// An entry of a list `CardDef` does not have yet, as the card's static would hold it.
pub const fn new_card_entry<T: TableEntry>(_: T) {{}}
'''


# ---------------------------------------------------------------------------------------------------------------------
# Compiling in shards


class Compiler:
    """rustc on the generated crate, as cargo runs it with the `proposals` feature, over shards of the cards at once: one
    crate of five thousand modules spends most of its time looking for names to suggest."""

    def __init__(self, cantrip, target, work, jobs):
        self.cantrip, self.target, self.work, self.jobs = Path(cantrip), Path(target), Path(work), jobs
        self.command = self.rustc_command()

    def rustc_command(self):
        probe = self.work / "probe"
        (probe / "cards").mkdir(parents=True, exist_ok=True)
        (probe / "cards/probe.rs").write_text("")
        (probe / "stubs.rs").write_text("")
        # Written each time, so the build script runs again and cargo says how it runs rustc.
        (probe / "list.txt").write_text("probe\n")
        env = dict(os.environ, CANTRIP_PROPOSALS=str(probe), CARGO_TARGET_DIR=str(self.target))
        env.update(CARGO_INCREMENTAL="0", CARGO_PROFILE_DEV_DEBUG="0")
        run = subprocess.run(
            ["cargo", "check", "-v", "-p", "cantrip-generated", "--features", "proposals"],
            cwd=self.cantrip, env=env, capture_output=True, text=True,
        )
        cache = self.work / "rustc.json"
        for line in run.stderr.splitlines():
            m = re.search(r"Running `(.*--crate-name cantrip_generated .*)`$", line.strip())
            if m:
                words = shlex.split(m.group(1))
                cache.write_text(json.dumps(words))
                break
        else:
            if not cache.exists():
                sys.exit(f"no rustc command for cantrip-generated in:\n{run.stderr[-3000:]}")
            words = json.loads(cache.read_text())
        keep, skip = [], False
        for word in words:
            if skip:
                skip = False
                continue
            if word == "--out-dir":
                skip = True
                continue
            if re.match(r"--(emit|json|error-format)=", word) or re.match(r"(metadata|extra-filename|incremental)=", word):
                if keep and keep[-1] == "-C":
                    keep.pop()
                continue
            keep.append(word)
        return keep + ["--emit=metadata", "--error-format=json", "-A", "warnings"]

    def compile(self, cards, stems, shards):
        """The errors of `stems` (in `cards`), compiled in `shards` crates at once, and the seconds it took."""
        start = time.time()
        order = sorted(stems, key=lambda s: -(cards / f"{s}.rs").stat().st_size)
        groups = [order[i::shards] for i in range(shards)]
        stubs = self.work / "stubs.rs"

        def one(i):
            out = self.work / "shards" / str(i)
            out.mkdir(parents=True, exist_ok=True)
            lines = [f"#[path = {json.dumps(str(stubs))}]\nmod proposals;"]
            lines += [f"#[path = {json.dumps(str(cards / (s + '.rs')))}]\nmod card_{j};" for j, s in enumerate(groups[i])]
            (out / "modules.rs").write_text("\n".join(lines) + "\n")
            run = subprocess.run(
                self.command + ["--out-dir", str(out), "-C", f"extra-filename=-shard{i}"],
                cwd=self.cantrip, env=dict(os.environ, OUT_DIR=str(out)), capture_output=True, text=True,
            )
            found = []
            for line in run.stderr.splitlines():
                try:
                    message = json.loads(line)
                except ValueError:
                    continue
                if message.get("level") == "error" and message.get("spans"):
                    found.append(message)
            return found

        with ThreadPoolExecutor(self.jobs) as pool:
            found = [d for part in pool.map(one, [i for i in range(shards) if groups[i]]) for d in part]
        return found, time.time() - start


# ---------------------------------------------------------------------------------------------------------------------
# What the errors say


def bound_in(d):
    """The stub bound an `E0277` comes from, and the stub that asks for it."""
    texts = [d["message"]] + [c.get("message", "") for c in d.get("children", [])]
    trait = where = None
    for t in texts:
        m = re.search(r"\b(Stored|Data|TableEntry|Derives_\w+)\b", t)
        if m and not trait:
            trait = m.group(1)
        m = re.search(r"required by a bound in `([^`]+)`", t)
        if m and not where:
            where = re.sub(r"^(crate::)?proposals::", "", m.group(1))
    return trait, where


def enclosing(text, at):
    """The bracket that holds `text[at]` and the path just before it: ("(", "crate::proposals::x::f") for an argument,
    ("{", "ManaRule") for a field of a literal; None at the top."""
    depth, j = 0, at - 1
    while j >= 0:
        c = text[j]
        if c in ")]}":
            depth += 1
        elif c in "([{":
            if depth == 0:
                m = re.search(r"((?:[\w]+::)*\.?\w+)\s*(?:::<[^()]*>)?\s*$", text[max(0, j - 300) : j])
                return (c, m.group(1) if m else "")
            depth -= 1
        j -= 1
    return None


def given_to_stub(text, at, spec):
    """Whether the value at `text[at]` is an argument of a stub or a field of a struct the proposal has, whose type
    no stub can give: a stub's parameter is generic, so nothing settles what a bare `None` or a closure's
    parameters are, as Cantrip's own signature would."""
    held = enclosing(text, at) if text else None
    if not held:
        return False
    bracket, path = held
    name = path.lstrip(".").split("::")[-1]
    if "proposals::" in path:
        return True
    if bracket == "(":
        return name in {k.split("::")[-1] for k in spec.stub_keys()}
    return bracket == "{" and (name in spec.types or name in spec.new_fields)


def classify(d, text, spec):
    """(class, stub_fault) for one error."""
    code = (d.get("code") or {}).get("code")
    msg = d["message"]
    rendered = d.get("rendered") or ""
    span = primary(d)
    if span["file_name"].endswith("stubs.rs"):
        return "stub layer: error inside the stubs", True
    snippet = text[span["byte_start"] : span["byte_end"]] if text else ""
    stubbed = "stubs.rs" in rendered or ROOT in snippet or "proposals::" in msg
    if code == "E0080" and (PANIC in msg or "proposal stub" in msg):
        return "stub layer: a stub's value read in a static", True
    if code == "E0015" and re.search(r"Proposed|proposals", msg):
        return "stub layer: a stubbed method called in a static (a trait method can't be const)", True
    if code in ("E0282", "E0283", "E0284", "E0790"):
        names = {k.split("::")[-1] for k in spec.stub_keys()}
        declared = re.search(r"declared on the (?:method|function|associated function) `(\w+)`", rendered)
        if (declared and declared.group(1) in names) or re.search(r"type parameter `(R|A\d+|F\d+|T\d+)`", rendered):
            return "stub layer: a stub's return or argument type the call doesn't settle", True
        if "closure parameter" in rendered and (stubbed or any(re.search(rf"\b{re.escape(n)}\s*\(", snippet) for n in names)):
            return "stub layer: a closure's parameter types, which a stub's generic argument doesn't give", True
        if snippet == "None" and "declared on the enum `Option`" in rendered and given_to_stub(text, span["byte_start"], spec):
            return "stub layer: a bare None given to a stub's generic argument or a proposed struct's field", True
        return "type annotations needed", False
    if "`!`" in msg:
        return "stub layer: a stub's value its call doesn't type", True
    if code == "E0107":
        return "stub layer: generic arguments given to a stub (a turbofish or a proposed struct named bare)", True
    if code == "E0747":
        return "stub layer: a constant generic argument given to a stub", True
    if code == "E0532" and stubbed:
        return "stub layer: a new tuple variant matched in a pattern", True
    if code == "E0308" and stubbed and "fn item" in rendered:
        return "stub layer: a stubbed function used as a value (a fn pointer)", True
    if code == "E0277":
        trait, where = bound_in(d)
        if trait == "Stored":
            return f"kept in the game state but not Copy + Eq + Hash ({where})", False
        if trait == "Data":
            return f"passed through dyn but not data ({where})", False
        if trait == "TableEntry":
            return f"in a CardDef table but not Copy + Sync + 'static ({where})", False
        if trait and trait.startswith("Derives_"):
            return f"inside {trait[8:]} but without what it derives ({where})", False
        return "trait bound not met", False
    m = re.match(r"the method `make` exists for struct `(\w+)_new", msg)
    if code == "E0599" and m:
        trait, _ = bound_in(d)
        owner = trait[8:] if trait and trait.startswith("Derives_") else "its type"
        return f"inside {owner} but without what it derives (variant {m.group(1)})", False
    if code == "E0425" and re.match(r"cannot find value `[a-z_]\w*`", msg):
        return "a local the hole names isn't in scope", False
    if code is None:
        return "syntax error", False
    if code == "E0061":
        if stubbed:
            return "the same proposed function called with different numbers of arguments", False
        return "arity: called with other arguments than Cantrip's function takes", False
    if code == "E0026":
        return "a field on an existing variant (not stubbed)", False
    if code == "E0063":
        if stubbed or "proposals::" in msg or "_new<" in msg:
            return "the same proposed struct or variant written with different fields", False
        return "a Cantrip struct literal missing fields", False
    if code == "E0599" and "`()`" in msg:
        return "stub layer: a value of a stub defaulted to ()", True
    return f"{code}: {re.sub(r'`[^`]*`', '`_`', msg)[:90]}", False


def report(diags, stems, spec, texts, cards):
    by_class = collections.defaultdict(list)
    per_card = collections.defaultdict(lambda: [0, 0])
    seen_stub = set()
    for d in diags:
        span = primary(d)
        stem = Path(span["file_name"]).stem
        # rustc says once for each bound a value fails, and the stubs' own errors come once from each shard.
        key = (d["message"] if not d["message"].startswith("the trait bound") else "", span["file_name"],
               span["byte_start"], span["byte_end"])
        if key in seen_stub:
            continue
        seen_stub.add(key)
        text = texts(span["file_name"])
        cls, fault = classify(d, text, spec)
        if span["file_name"].endswith("stubs.rs"):
            stem = "(stubs)"
        line = text.count("\n", 0, span["byte_start"]) + 1 if text else 0
        snippet = (text or "")[max(0, span["byte_start"] - 60) : span["byte_end"] + 60].replace("\n", " ")
        why = re.findall(r"(?:within `[^`]*`, )?the trait `[\w:]+` is not implemented for [^\n]*?`[^`]*`",
                         d.get("rendered") or "")
        message = d["message"] + (f" ({why[0]})" if why else "")
        by_class[(cls, fault)].append({"card": stem, "line": line, "message": message, "near": snippet})
        if stem in stems:
            per_card[stem][1 if fault else 0] += 1
    clean = [s for s in stems if s not in per_card]
    stub_only = [s for s, (real, fault) in per_card.items() if not real]
    classes = []
    for (cls, fault), items in sorted(by_class.items(), key=lambda kv: -len(kv[1])):
        cards_of = sorted({i["card"] for i in items})
        classes.append({"class": cls, "stub_fault": fault, "errors": len(items), "cards": len(cards_of),
                        "examples": [next(i for i in items if i["card"] == c) for c in cards_of[:3]]})
    return {"cards": len(stems), "compile": len(clean), "compile_but_for_stubs": len(stub_only),
            "fail": len(stems) - len(clean) - len(stub_only), "classes": classes,
            "defaulted_returns": spec.rets, "unknown_returns": sorted(spec.unknown_rets),
            "failing": {s: v for s, v in sorted(per_card.items())}}


def print_report(rep, out=sys.stdout):
    print(f"{rep['cards']} planned cards: {rep['compile']} compile, {rep['compile_but_for_stubs']} fail only on the "
          f"stub layer, {rep['fail']} fail on what they propose", file=out)
    for c in rep["classes"]:
        tag = "STUB " if c["stub_fault"] else ""
        print(f"\n{tag}{c['errors']:5} errors {c['cards']:4} cards  {c['class']}", file=out)
        for e in c["examples"]:
            print(f"      {e['card']}:{e['line']}: {e['message'][:140]}", file=out)
    if rep["defaulted_returns"]:
        print("\nreturn types chosen where no call site settles one:", file=out)
        for k, v in sorted(rep["defaulted_returns"].items()):
            print(f"      {k} -> {v}", file=out)


# ---------------------------------------------------------------------------------------------------------------------


def run(cantrip, stems, work, target, jobs, sources=None, passes=8, quiet=False):
    """Compile `stems` against stubs learnt from their own errors until no new name turns up; the report."""
    t0 = time.time()
    model = Model(cantrip)
    if not quiet:
        log(f"model {time.time() - t0:.1f}s: {len(model.kinds)} types, {len(model.fields['CardDef'])} CardDef fields")
    spec = Spec(model)
    cards = Path(work) / "cards"
    cards.mkdir(parents=True, exist_ok=True)
    generated = Path(cantrip) / CRATE / "cards"
    originals = {s: Path((sources or {}).get(s) or generated / f"{s}.rs").read_text(encoding="utf-8") for s in stems}
    compiler = Compiler(cantrip, target, work, jobs)
    shards = max(1, min(jobs, len(stems) // 30 or 1))
    cache = {}

    def texts(path):
        if path not in cache:
            try:
                # rustc's spans are byte offsets: one character a byte, so an index is one (an Oracle text's dash
                # is three, in a comment).
                cache[path] = Path(path).read_bytes().decode("latin-1")
            except OSError:
                cache[path] = None
        return cache[path]

    for n in range(passes):
        t = time.time()
        for stem, original in originals.items():
            text = rewrite(original, model, spec)
            target_path = cards / f"{stem}.rs"
            if not target_path.exists() or target_path.read_text(encoding="utf-8") != text:
                target_path.write_text(text, encoding="utf-8")
        (Path(work) / "stubs.rs").write_text(spec.write())
        prep = time.time() - t
        cache.clear()
        diags, seconds = compiler.compile(cards, stems, shards)
        t = time.time()
        changed = spec.learn(diags, texts)
        if not quiet:
            log(f"pass {n + 1}: rewrite {prep:.1f}s, compile {seconds:.1f}s in {shards} shards, {len(diags)} errors, "
                f"learn {time.time() - t:.1f}s: {spec.size()}")
        if not changed:
            break
    rep = report(diags, set(stems), spec, texts, cards)
    rep["seconds"] = round(time.time() - t0, 1)
    rep["passes"] = n + 1
    return rep, diags


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("cantrip")
    parser.add_argument("--cards", help="comma-separated stems; default planned.txt")
    parser.add_argument("--source", action="append", default=[], help="STEM=FILE: check FILE as card STEM")
    parser.add_argument("--work", help="the check copy and stubs; default CANTRIP/target/proposals")
    parser.add_argument("--target", help="cargo's target dir; default CANTRIP/target")
    parser.add_argument("--jobs", type=int, default=min(48, os.cpu_count() or 4))
    parser.add_argument("--passes", type=int, default=8)
    parser.add_argument("--report", help="write the report as JSON here")
    parser.add_argument("--dump", help="write the final errors as JSON lines here")
    args = parser.parse_args()
    cantrip = Path(args.cantrip).resolve()
    work = Path(args.work or cantrip / "target/proposals").resolve()
    target = Path(args.target or cantrip / "target").resolve()
    sources = dict(s.split("=", 1) for s in args.source)
    if args.cards and args.cards.startswith("@"):
        stems = Path(args.cards[1:]).read_text().replace(",", "\n").split()
    else:
        stems = args.cards.split(",") if args.cards else (cantrip / CRATE / "planned.txt").read_text().split()
    stems += [s for s in sources if s not in stems]
    rep, diags = run(cantrip, stems, work, target, args.jobs, sources, args.passes)
    print_report(rep)
    log(f"{rep['seconds']}s, {rep['passes']} passes")
    if args.report:
        Path(args.report).write_text(json.dumps(rep, indent=1, default=sorted))
    if args.dump:
        with open(args.dump, "w") as out:
            for d in diags:
                out.write(json.dumps(d) + "\n")
    return 0 if rep["fail"] == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
