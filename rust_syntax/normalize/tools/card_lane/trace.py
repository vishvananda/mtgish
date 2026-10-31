#!/usr/bin/env python3
"""Where a Codex worker's time and tokens went on one card, from its session rollout.

    trace.py "Card Name"            one card: model and tool time, context growth, the largest reads, failed commands
    trace.py "Card Name" --steps    and every step with its time
    trace.py --all                  one line per card that has a Codex session

Read this before changing the skill or the packet. It reads only the rollout and the card's worker.jsonl, and needs no
build. It knows Codex's format; another backend needs its own reader.
"""
import collections
import datetime
import glob
import json
import os
import re
import sys
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
import card_lane as lane  # noqa: E402

CALLS = ("function_call", "custom_tool_call", "local_shell_call")
RESULTS = ("function_call_output", "custom_tool_call_output", "local_shell_call_output")


def records(path):
    for line in open(path, errors="replace"):
        try:
            yield json.loads(line)
        except ValueError:
            pass


def rollout(stem):
    """The Codex session file of a card's last worker, and the commands its stream reported."""
    thread, commands = None, []
    log = lane.STATE / "cards" / stem / "worker.jsonl"
    if not log.exists():
        return None, []
    for event in records(log):
        if event.get("type") == "thread.started":
            thread = event["thread_id"]
        item = event.get("item") or {}
        if event.get("type") == "item.completed" and item.get("type") == "command_execution":
            commands.append((item["command"].removeprefix("/bin/bash -lc "), item.get("exit_code"), item.get("aggregated_output") or ""))
    found = glob.glob(os.path.expanduser(f"~/.codex/sessions/**/*{thread}*.jsonl"), recursive=True) if thread else []
    return (found[0] if found else None), commands


def seconds(event):
    return datetime.datetime.fromisoformat(event["timestamp"].replace("Z", "+00:00")).timestamp()


def steps(path):
    """(kind, what, seconds, output chars) in order: `model` is the time before a call, `tool` the call's own."""
    rows, calls, last = [], {}, None
    for event in records(path):
        payload = event.get("payload") or {}
        when = seconds(event)
        last = last or when
        kind = payload.get("type")
        if kind in CALLS:
            what = str(payload.get("arguments") or payload.get("input") or "")
            found = re.search(r'cmd:\s*"((?:[^"\\]|\\.)*)"', what)
            what = found[1] if found else what
            rows.append(("model", what, when - last, 0))
            calls[payload.get("call_id")] = (when, what)
            last = when
        elif kind in RESULTS:
            begun, what = calls.pop(payload.get("call_id"), (when, "?"))
            rows.append(("tool", what, when - begun, len(str(payload.get("output")))))
            last = when
    return rows


def bucket(command):
    if "card_lane.py" in command or "corpus" in command:
        return "gates and scenarios"
    if "patch" in command[:20] or "Begin Patch" in command:
        return "edits"
    if re.search(r"\b(cat|sed|rg|grep|nl|head|tail|ls|find)\b", command):
        return "reading"
    return "other"


def tokens(path):
    calls = []
    for event in records(path):
        payload = event.get("payload") or {}
        if payload.get("type") == "token_count" and payload.get("info"):
            last = payload["info"].get("last_token_usage") or {}
            calls.append((last.get("input_tokens", 0), last.get("output_tokens", 0), last.get("reasoning_output_tokens", 0)))
    return calls


def summary(stem):
    path, commands = rollout(stem)
    if path is None:
        return None
    rows, calls = steps(path), tokens(path)
    model = sum(r[2] for r in rows if r[0] == "model")
    tool = sum(r[2] for r in rows if r[0] == "tool")
    return {"path": path, "commands": commands, "rows": rows, "calls": calls, "model": model, "tool": tool,
            "read": sum(len(o) for c, _, o in commands), "output": sum(c[1] for c in calls), "reasoning": sum(c[2] for c in calls),
            "peak": max((c[0] for c in calls), default=0)}


def show(stem, every):
    found = summary(stem)
    if found is None:
        sys.exit(f"no Codex session for {stem}")
    rows, calls = found["rows"], found["calls"]
    print(f"{stem}: model {found['model']:.0f}s, tools {found['tool']:.0f}s, {sum(r[0] == 'tool' for r in rows)} calls")
    print(f"output {found['output']} tokens, {found['reasoning']} of them reasoning; context {calls[0][0] if calls else 0} to {found['peak']} over {len(calls)} API calls")
    time_in, count, chars = collections.Counter(), collections.Counter(), collections.Counter()
    for kind, what, spent, size in rows:
        if kind == "tool":
            time_in[bucket(what)] += spent
            count[bucket(what)] += 1
            chars[bucket(what)] += size
    for name, spent in time_in.most_common():
        print(f"  {name:<20} {spent:6.0f}s {count[name]:4} calls {chars[name]:8} chars")
    print("largest outputs:")
    for size, command in sorted(((len(o), c[:100].replace("\n", " ")) for c, _, o in found["commands"]), reverse=True)[:8]:
        print(f"  {size:8} {command}")
    print("failed commands:")
    for command, code, output in found["commands"]:
        if code not in (0, None):
            print(f"  {command[:110]}".replace("\n", " "))
            for line in [l for l in output.splitlines() if re.search(r"FAIL|error|is not a key|went by|never", l)][:3]:
                print(f"      {line.strip()[:140]}")
    if every:
        clock = 0
        for kind, what, spent, size in rows:
            clock += spent
            print(f"{clock:7.0f} {kind:<5} {spent:6.0f}s {size:8} {what[:110]}".replace("\n", " "))


def main():
    if "--all" in sys.argv:
        print(f"{'card':<32} {'model':>6} {'tools':>6} {'output':>7} {'reason':>7} {'peak':>7} {'read':>8}")
        for card in lane.load_queue()["cards"]:
            stem = lane.names_of(card["name"])[0]
            found = summary(stem) if card.get("seconds") else None
            if found:
                print(f"{card['name'][:32]:<32} {found['model']:6.0f} {found['tool']:6.0f} {found['output']:7} {found['reasoning']:7} {found['peak']:7} {found['read']:8}")
        return
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    show(lane.names_of(sys.argv[1])[0], "--steps" in sys.argv)


if __name__ == "__main__":
    main()
