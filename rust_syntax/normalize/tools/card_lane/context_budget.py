"""Measure Codex context from its local rollout, not cumulative billed usage."""
import json
import os
from pathlib import Path
import time


IDLE_CONTEXT_SECONDS = 60 * 60


def idle_restart(last_finished, *, now=None):
    """Expire retained context between cards, never during work on the current card."""
    if last_finished is None:
        return False
    current = time.monotonic() if now is None else now
    return current - last_finished > IDLE_CONTEXT_SECONDS


def session_context(session, codex_home=None):
    result = {"last_tokens": None, "peak_tokens": None, "compactions": 0,
              "path": None, "context_window": None}
    if not session or not all(c.isalnum() or c == '-' for c in session):
        return result
    home = Path(codex_home or os.environ.get("CODEX_HOME", Path.home() / ".codex"))
    paths = list((home / "sessions").rglob(f"*{session}.jsonl"))
    if len(paths) != 1:
        return result
    result["path"] = str(paths[0])
    try:
        with paths[0].open(encoding="utf-8") as source:
            for line in source:
                try:
                    item = json.loads(line)
                except (ValueError, UnicodeError):
                    continue
                payload = item.get("payload") or {}
                if item.get("type") == "compacted" or (item.get("type") == "event_msg" and payload.get("type") == "context_compacted"):
                    result["compactions"] += 1
                if item.get("type") != "event_msg" or payload.get("type") != "token_count":
                    continue
                info = payload.get("info") or {}
                usage = info.get("last_token_usage") or {}
                input_tokens, output_tokens = usage.get("input_tokens"), usage.get("output_tokens")
                if isinstance(input_tokens, int) and isinstance(output_tokens, int) and input_tokens >= 0 and output_tokens >= 0:
                    value = input_tokens + output_tokens
                    result["last_tokens"] = value
                    result["peak_tokens"] = max(result["peak_tokens"] or 0, value)
                if isinstance(info.get("model_context_window"), int):
                    result["context_window"] = info["model_context_window"]
    except OSError:
        pass
    return result


def budget_restart(metrics, token_limit):
    return bool(token_limit and metrics.get("last_tokens") is not None and metrics["last_tokens"] >= token_limit)


def hook_config(marker):
    """CLI flags for exactly this trusted PreCompact hook, with no global trust bypass."""
    import hashlib
    import shlex
    command = f"python3 {shlex.quote(str(Path(__file__).resolve()))} hook {shlex.quote(str(marker))}"
    handler = {"type": "command", "command": command, "timeout": 10, "async": False}
    identity = {"event_name": "pre_compact", "matcher": "auto", "hooks": [handler]}
    encoded = json.dumps(identity, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()
    trusted = 'sha256:' + hashlib.sha256(encoded).hexdigest()
    group = '{matcher="auto",hooks=[{type="command",command=' + json.dumps(command) + ',timeout=10}]}'
    key = '/<session-flags>/config.toml:pre_compact:0:0'
    return ['--config', 'features.hooks=true', '--config', 'hooks.PreCompact=[' + group + ']',
            '--config', 'hooks.state={' + json.dumps(key) + '={trusted_hash=' + json.dumps(trusted) + '}}']


def precompact_hook(marker):
    """Record why the worker stopped before denying automatic compaction."""
    import sys
    payload = json.load(sys.stdin)
    Path(marker).write_text(json.dumps({'session_id': payload.get('session_id'), 'trigger': payload.get('trigger')}), encoding='utf-8')
    print(json.dumps({'continue': False, 'stopReason': 'Context checkpoint: restart the worker in a fresh session.'}))


def precompact_stopped(marker, session):
    try:
        data = json.loads(Path(marker).read_text(encoding='utf-8'))
        return bool(session and data.get('session_id') == session and data.get('trigger') == 'auto')
    except (OSError, ValueError, AttributeError):
        return False


if __name__ == '__main__':
    import sys
    if len(sys.argv) == 3 and sys.argv[1] == 'hook':
        precompact_hook(sys.argv[2])
