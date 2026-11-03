"""Check composition against other pending publications, without changing worktrees."""
import json
from pathlib import Path
import re
import subprocess


def git(repo, *args, check=True):
    result = subprocess.run(['git', *args], cwd=repo, text=True, capture_output=True)
    if check and result.returncode:
        raise RuntimeError(result.stderr or result.stdout)
    return result


def check_heads(repo, branch, head, pending):
    """The caller supplies exact pending heads, from GitHub or a frozen repair plan."""
    results = []
    for pr in pending:
        if pr['headRefName'] == branch:
            continue
        other = pr['headRefOid']
        result = git(repo, 'merge-tree', '--write-tree', '--name-only', head, other, check=False)
        if result.returncode not in (0, 1):
            raise RuntimeError(result.stderr or result.stdout)
        lines = result.stdout.splitlines()
        if not lines or not re.fullmatch(r'[0-9a-f]{40,64}', lines[0]):
            raise RuntimeError(result.stderr or result.stdout or 'merge-tree produced no result tree')
        paths = []
        if result.returncode:
            for line in lines[1:]:
                if not line:
                    break
                paths.append(line)
        results.append(dict(pr=pr['number'], branch=pr['headRefName'], head=other,
                            ok=result.returncode == 0, conflicts=paths))
    return dict(branch=branch, head=head, ok=all(r['ok'] for r in results), checks=results)


def require_promoted(repo, base, prerequisites, pin, repairs=()):
    """A card may consume the foundation, but cannot publish its own shared fork."""
    allowed = set(pin.get('commits', [])) | {r['commit'] for r in repairs}
    missing = []
    for sha in prerequisites:
        if sha in allowed:
            continue
        # A helper already landed upstream needs no second promotion.
        if git(repo, 'merge-base', '--is-ancestor', sha, base, check=False).returncode == 0:
            continue
        equivalent = git(repo, 'cherry', base, sha, sha+'^').stdout.strip()
        if equivalent.startswith('-'):
            continue
        missing.append(sha)
    if missing:
        raise RuntimeError('Shared prerequisites need promotion to the common foundation before card publication: '
                           + ', '.join(missing))


def save(path, result):
    Path(path).write_text(json.dumps(result, indent=2) + '\n')
