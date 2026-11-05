"""Keep exact check receipts and logs after later checks replace live files."""
import hashlib
import json
from pathlib import Path


def freeze(folder, data, suffix):
    folder = Path(folder)
    folder.mkdir(parents=True, exist_ok=True)
    path = folder / (hashlib.sha256(data).hexdigest() + suffix)
    try:
        with path.open('xb') as output:
            output.write(data)
    except FileExistsError:
        if path.read_bytes() != data:
            raise ValueError(f'corrupt immutable evidence: {path}')
    return path.resolve()


def _logs(value, folder, *, rewrite):
    found = []
    if isinstance(value, dict):
        log = value.get('log')
        if isinstance(log, str):
            original = Path(log)
            if not original.is_absolute():
                original = Path(folder) / original
            if original.is_file():
                raw = original.read_bytes()
                saved = freeze(Path(folder) / 'evidence/logs', raw, '.log')
                found.append(dict(original=str(original), path=str(saved),
                                  sha256=hashlib.sha256(raw).hexdigest()))
                if rewrite:
                    value['log'] = str(saved)
        for child in value.values():
            found.extend(_logs(child, folder, rewrite=rewrite))
    elif isinstance(value, list):
        for child in value:
            found.extend(_logs(child, folder, rewrite=rewrite))
    return found


def archive_previous_checks(folder):
    """Archive the exact old bytes before any gate overwrites their live logs."""
    folder = Path(folder)
    path = folder / 'checks.json'
    if not path.is_file():
        return
    raw = path.read_bytes()
    saved = freeze(folder / 'evidence/checks', raw, '.json')
    mapping = dict(checks=str(saved), checks_sha256=hashlib.sha256(raw).hexdigest(),
                   logs=_logs(json.loads(raw), folder, rewrite=False))
    freeze(folder / 'evidence/maps', (json.dumps(mapping, indent=2) + '\n').encode(), '.json')


def freeze_checks(folder, result):
    """Return a receipt whose log references survive every subsequent rerun."""
    result = json.loads(json.dumps(result))
    _logs(result, folder, rewrite=True)
    raw = (json.dumps(result, indent=2) + '\n').encode()
    path = freeze(Path(folder) / 'evidence/checks', raw, '.json')
    return result, path
