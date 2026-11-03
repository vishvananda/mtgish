"""Validated, immutable helper pins, independent of individual card batches."""
import json
from pathlib import Path
import re
import subprocess


def git(repo, *args):
    return subprocess.check_output(['git', *args], cwd=repo, text=True).strip()


def read_pin(cfg, repo):
    path = cfg.get('shared_base_manifest')
    if not path:
        return None
    pin = json.loads(Path(path).read_text())
    if not re.fullmatch(r'[0-9a-f]{40}', pin.get('head', '')):
        raise ValueError('shared base must pin a full commit hash')
    validation = pin.get('validation', {})
    if validation.get('ok') is not True or validation.get('head') != pin['head']:
        raise ValueError('shared base has no matching successful validation')
    git(repo, 'merge-base', '--is-ancestor', pin['base'], pin['head'])
    actual = git(repo, 'rev-list', '--reverse', '--first-parent', f"{pin['base']}..{pin['head']}").splitlines()
    commits = pin.get('commits', [])
    if actual != commits or not commits:
        raise ValueError('shared base manifest does not cover exactly its helper commits')
    for commit in commits:
        subject = git(repo, 'show', '-s', '--format=%s', commit)
        if not subject.startswith(('Mechanics: ', 'Engine: ')):
            raise ValueError('shared base may contain only helper prerequisite commits')
        if len(git(repo, 'rev-list', '--parents', '-n', '1', commit).split()) != 2:
            raise ValueError('shared base must have linear prerequisite history')
    if not re.fullmatch(r'https://github\.com/[^/]+/[^/]+/pull/\d+', pin.get('pr_url', '')):
        raise ValueError('shared base must link its prerequisite PR')
    return pin


def metadata(pin):
    if pin.get('kind') == 'local_integration':
        return dict(publication_base=pin['head'], local_integration_base=pin['head'], shared_dependencies=[])
    return dict(publication_base=pin['head'], shared_dependencies=[
        dict(commit=commit, pr_url=pin['pr_url']) for commit in pin['commits']])


def read_work_base(cfg, repo):
    """A worker takes a validated immutable snapshot, never the moving integration tip."""
    path = cfg.get('local_base_manifest')
    if not path:
        return read_pin(cfg, repo)
    pin = json.loads(Path(path).read_text())
    validation = pin.get('validation', {})
    if pin.get('kind') != 'local_integration' or not re.fullmatch(r'[0-9a-f]{40}', pin.get('head', '')):
        raise ValueError('local integration base must pin an immutable commit')
    if validation.get('ok') is not True or validation.get('head') != pin['head']:
        raise ValueError('local integration base has no matching successful validation')
    git(repo, 'merge-base', '--is-ancestor', pin['base'], pin['head'])
    return pin
