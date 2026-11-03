#!/usr/bin/env python3
"""Deterministic card intake. Color filters apply only to set selections and test
color membership (multicolor cards match each constituent color); colorless has
no colors. Explicit names are unaffected. Set data must be locally complete.
"""
import argparse
import datetime as dt
import fcntl
import gzip
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import urllib.parse
import urllib.request

ORACLE = Path('/home/vishvananda/card-lane/scryfall/oracle-cards-20261031.jsonl.gz')
CATALOG = Path('/home/vishvananda/card-lane/unlimited-catalog-20261103')
CANTRIP = Path('/home/vishvananda/cantrip')
REPO = 'cantrip-rs/cantrip-rs'
HOLDER = 'vishvananda/unlimited-20261103'
COLORS = {'white':'W','blue':'U','black':'B','red':'R','green':'G','colorless':'C',
          'w':'W','u':'U','b':'B','r':'R','g':'G','c':'C'}


def read(path):
    return json.loads(Path(path).read_text(encoding='utf-8-sig'))


def save(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp = tempfile.mkstemp(dir=path.parent, prefix='.intake-')
    try:
        with os.fdopen(fd, 'w', encoding='utf-8') as out:
            json.dump(value, out, ensure_ascii=False, indent=2)
            out.write('\n'); out.flush(); os.fsync(out.fileno())
        os.replace(tmp, path)
    finally:
        if os.path.exists(tmp): os.unlink(tmp)


def load_oracle(path=ORACLE):
    records = {}
    opener = gzip.open if str(path).endswith('.gz') else open
    with opener(path, 'rt', encoding='utf-8') as f:
        for line_no, line in enumerate(f, 1):
            card = json.loads(line)
            records.setdefault(card['name'], []).append((card, line_no))
    return records


def files_names(paths):
    result = []
    for path in paths:
        text = Path(path).read_text(encoding='utf-8-sig')
        if text.lstrip().startswith('['):
            values = json.loads(text)
            if not isinstance(values, list) or any(not isinstance(n, str) for n in values):
                raise ValueError('list file must contain strings')
            result.extend(values)
        else:
            result.extend(line.strip() for line in text.splitlines() if line.strip())
    return result


def local_set(code, catalog=CATALOG, cache=None, fetch=False):
    catalog = Path(catalog)
    # MTGJSON per-set files can be supplied as <code>.json in this directory.
    for path in sorted(catalog.glob('*.json')):
        if path.name in {'scryfall-set.json', 'scryfall-unlimited.json'}: continue
        try:
            data = read(path)
            data = data.get('data', data) if isinstance(data, dict) else {}
            if isinstance(data, dict) and isinstance(data.get('cards'), list):
                aliases = {str(data.get('code', '')).casefold(), str(data.get('name', '')).casefold()}
                if code.casefold() in aliases: return data['cards']
        except (OSError, ValueError):
            continue
    if (catalog / 'scryfall-set.json').exists():
        meta = read(catalog / 'scryfall-set.json')
        aliases = {meta['code'].casefold(), meta['name'].casefold(), 'unlimited'}
        if code.casefold() in aliases:
            return read(catalog / 'scryfall-unlimited.json')
    return cached_set(code, cache, fetch)


def api_json(url):
    if urllib.parse.urlparse(url).netloc != 'api.scryfall.com':
        raise ValueError('unexpected Scryfall pagination host')
    request = urllib.request.Request(url, headers={'User-Agent':'mtgish-card-intake/1.0', 'Accept':'application/json'})
    with urllib.request.urlopen(request, timeout=60) as response:
        return json.load(response)


def pages(url):
    result = []
    while url:
        page = api_json(url)
        if not isinstance(page.get('data'), list): raise ValueError('invalid Scryfall response')
        result.extend(page['data'])
        url = page.get('next_page') if page.get('has_more') else None
        if page.get('has_more') and not url: raise ValueError('incomplete Scryfall pagination')
    return result


def cached_set(name, cache, fetch):
    if cache is None: raise ValueError(f'no complete local printing catalog for set {name!r}')
    cache = Path(cache); index = cache/'sets.json'
    if not index.exists():
        if not fetch: raise ValueError(f'no complete local printing catalog for set {name!r}; use --fetch-set')
        data = pages('https://api.scryfall.com/sets')
        save(index, {'source':'https://api.scryfall.com/sets','fetched_at':dt.datetime.now(dt.timezone.utc).isoformat(),'sets':data})
    entries = read(index)['sets']
    matches = [x for x in entries if name.casefold() in {x['code'].casefold(),x['name'].casefold()}]
    if len(matches) != 1: raise ValueError(f'unknown or ambiguous set {name!r}')
    meta = matches[0]; path = cache/(meta['code'].lower()+'.json')
    if not path.exists():
        if not fetch: raise ValueError(f'no complete local printing catalog for set {name!r}; use --fetch-set')
        query = urllib.parse.urlencode({'q':'set:'+meta['code'],'unique':'prints','include_extras':'true'})
        url = 'https://api.scryfall.com/cards/search?'+query
        cards = pages(url)
        save(path, {'source':url,'fetched_at':dt.datetime.now(dt.timezone.utc).isoformat(),'set':meta,'cards':cards})
    return read(path)['cards']


def exclusion(card):
    if card['name'] in {'Chaos Orb', 'Falling Star'}: return 'dexterity'
    text = '\n'.join([card.get('oracle_text', '')] + [f.get('oracle_text', '') for f in card.get('card_faces', [])])
    if re.search(r'\bante\b', text, re.I): return 'ante'
    if re.search(r'\bsubgame\b', text, re.I): return 'subgame'
    if card.get('layout') in {'token','double_faced_token','emblem','art_series','planar','scheme','vanguard','front_card'}:
        return 'not a playable Magic card'
    if card.get('games') and not set(card['games']) & {'paper','mtgo','arena'}: return 'not a playable Magic card'
    return None


def canonical_records(records):
    """Prefer real card records over a token sharing the same name."""
    result = {}
    for name, variants in records.items():
        real = [v for v in variants if exclusion(v[0]) != 'not a playable Magic card']
        candidates = real or variants
        ids = {c.get('oracle_id') or c.get('id') for c, _ in candidates}
        if len(ids) == 1: result[name] = candidates[0]
        else: result[name] = None
    return result


def select(records, names, sets, color=None, catalog=CATALOG, cache=None, fetch=False):
    if color and not sets: raise ValueError('--color requires --set')
    if color:
        color = COLORS.get(color.casefold())
        if not color: raise ValueError('unknown color')
    canonical = canonical_records(records)
    folded = {}
    for name in canonical: folded.setdefault(name.casefold(), []).append(name)
    def resolve(name):
        matches = folded.get(name.casefold(), [])
        if len(matches) != 1: raise ValueError(f'unknown or ambiguous canonical card name: {name!r}')
        result = matches[0]
        if canonical[result] is None: raise ValueError(f'ambiguous canonical card name: {name!r}')
        return result
    chosen = {resolve(name) for name in names}
    for code in sets:
        for printing in local_set(code, catalog, cache, fetch):
            # Token printings may share a name with a real card; do not turn
            # them into a set membership for that real card.
            if exclusion(printing) == 'not a playable Magic card': continue
            name = resolve(printing.get('name', ''))
            card = canonical[name][0]
            colors = card.get('colors')
            if colors is None:
                colors = sorted({c for face in card.get('card_faces', []) for c in face.get('colors', [])})
            if not color or (not colors if color == 'C' else color in colors): chosen.add(name)
    return [(name, *canonical[name]) for name in sorted(chosen, key=lambda s:(s.casefold(),s))]


def implemented(base, tree=CANTRIP):
    # Read only the configured base, not uncommitted files in the checkout.
    run = subprocess.run(['git','-C',str(tree),'grep','-h','-E','^  "name":',base,'--','content/*/cards/*.json'], text=True, capture_output=True)
    if run.returncode not in (0, 1): raise RuntimeError(run.stderr)
    names = set()
    for line in run.stdout.splitlines():
        try: names.add(json.loads('{'+line.rstrip().rstrip(',')+'}')['name'])
        except (ValueError, KeyError): pass
    return names


def rows_at(path):
    result = {}
    if Path(path).exists():
        with open(path, encoding='utf-8') as f:
            for line in f:
                row = json.loads(line); result[row['name']] = row
    return result


def plan(root, names, sets, color, oracle=ORACLE, catalog=CATALOG, tree=CANTRIP, fetch=False):
    root = Path(root); cfg = read(root/'config.json'); queue = read(cfg['implementation_queue'])
    records = load_oracle(oracle); chosen = select(records,names,sets,color,catalog,root/'set-cache',fetch)
    import shared_base
    pin = shared_base.read_work_base(cfg, tree)
    known = {c['name'] for c in queue['cards']}; done = implemented(pin['head'] if pin else cfg['base'], tree)
    rows = rows_at(Path(cfg['lane_root'])/'gen/rows.jsonl')
    result = []
    for name, card, line in chosen:
        reason = exclusion(card)
        row = rows.get(name)
        status = 'excluded' if reason else 'already_queued' if name in known else 'implemented' if name in done else 'candidate'
        readiness = 'needs_generation' if row is None else 'generator_gap' if row.get('gap') else 'pending'
        result.append({'name':name,'status':status,'reason':reason,'readiness':readiness,'layout':card.get('layout','normal'),
                       'oracle_id':card.get('oracle_id'),'oracle_source':str(oracle),'oracle_line':line})
    return result


class Claims:
    """Use Cantrip's claim event parser, including PR and branch activity."""
    def __init__(self, cards, holder=HOLDER):
        path = CANTRIP/'tools/maintenance/card-issues.py'
        spec = importlib.util.spec_from_file_location('intake_card_issues', path)
        self.ci = importlib.util.module_from_spec(spec); spec.loader.exec_module(self.ci)
        self.ci.ISSUES = self.ci.ISSUES.replace('labels:["card"],','')
        self.ci.CLOSED = self.ci.CLOSED.replace('labels:["card"],','')
        self.ci.PULLS = self.ci.PULLS.replace('closingIssuesReferences(first:5)','closingIssuesReferences(first:100)')
        self.cards = cards
        self.holder = holder
        self.since = (dt.datetime.now(dt.timezone.utc)-dt.timedelta(seconds=60)).isoformat()
        self.state = self.ci.read_state(REPO, fresh=True)
        login = self.ci.gh('api','user')['login']
        if login != holder.split('/')[0]: raise RuntimeError(f'authenticated as unexpected user {login}')

    def find(self, name):
        matches = [i for i in self.state.values() if i['title'] == self.ci.title(name)]
        if len(matches) > 1: raise RuntimeError(f'multiple card issues for {name}; resolve manually')
        return matches[0] if matches else None

    def claim(self, name):
        ci = self.ci
        # Discover newly created/retitled issues, including unlabeled ones,
        # without rereading every historical claim and PR for every new card.
        at = dt.datetime.now(dt.timezone.utc)
        query = urllib.parse.urlencode({'state':'all','since':self.since,'per_page':100})
        pages = ci.gh('api','--paginate','--slurp',f'repos/{REPO}/issues?{query}')
        for page in pages:
            for raw in page:
                if 'pull_request' in raw: continue
                number = raw['number']
                if number in self.state:
                    self.state[number].update(title=raw['title'],state=raw['state'].upper())
                else:
                    self.state[number] = ci.with_events(dict(number=number,title=raw['title'],state=raw['state'].upper(),
                        labels={'nodes':raw.get('labels',[])},comments={'nodes':[]}))
        self.since = (at-dt.timedelta(seconds=60)).isoformat()
        issue = self.find(name)
        if issue:
            # Existing claims still require fresh PR/branch activity as well as
            # fresh comments before deciding whether somebody else owns them.
            self.state = ci.read_state(REPO, fresh=True)
            issue = self.find(name)
        if issue and issue['state'] != 'OPEN': raise RuntimeError(f'{name}: closed card issue #{issue["number"]}')
        if issue:
            issue = ci.read_issue(REPO, issue)
            held = ci.holder(issue)
            if held and held[0] != self.holder: raise RuntimeError(f'{name}: held by {held[0]}')
        else:
            made = ci.gh('api','-X','POST',f'repos/{REPO}/issues','--input','-',
                         body={'title':ci.title(name),'body':ci.card_issue_body(name,self.cards[name][0],[])})
            issue = dict(made, state='OPEN', labels=set(), events=[], from_comments=0)
            self.state[issue['number']] = issue
        held = ci.holder(issue)
        if not held:
            tag = self.holder.split('/',1)[1]
            ci.gh('api','-X','POST',f'repos/{REPO}/issues/{issue["number"]}/comments','--input','-',
                  body={'body':f'{ci.CLAIM}\n{ci.CLAIMANT.format(tag)}\nClaimed by {ci.named(self.holder)}. Held 24 hours from the last push or commit on its pull request or `card/{ci.slug(name)}` branch; claim again to renew.'})
            issue = ci.read_issue(REPO,issue); held = ci.holder(issue)
        if not held or held[0] != self.holder: raise RuntimeError(f'{name}: claim lost or not confirmed')
        return {'issue':issue['number'],'holder':held[0], 'expires_at':(held[2]+ci.HOLD).isoformat()}


def add(root, candidates, claims_factory=Claims, oracle=ORACLE, reserve_gaps=False):
    root = Path(root); cfg = read(root/'config.json'); queue_path = Path(cfg['implementation_queue'])
    journal_path = root/'intake-journal.json'
    # Serialize the journal, but do not hold the shared queue lock across slow
    # reservation calls: implementation workers need it to record outcomes.
    with Path(str(journal_path)+'.lock').open('a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        journal = read(journal_path) if journal_path.exists() else {'cards':{}}
        holder = cfg.get('intake', {}).get('claimant', 'vishvananda/' + Path(cfg.get('lane_root', root)).name)
        claims = claims_factory(canonical_records(load_oracle(oracle)), holder)
        for item in candidates:
            if item['status'] != 'candidate': continue
            name = item['name']; entry = dict(journal['cards'].get(name, {}), **item)
            if item['readiness'] != 'pending' and not reserve_gaps:
                entry['phase'] = 'deferred_unclaimed'
                journal['cards'][name] = entry; save(journal_path,journal)
                continue
            entry['phase'] = 'claiming'; journal['cards'][name] = entry; save(journal_path,journal)
            try:
                entry.update(claims.claim(name)); entry['phase'] = 'claimed'; save(journal_path,journal)
                with Path(str(queue_path)+'.lock').open('a') as queue_lock:
                    fcntl.flock(queue_lock, fcntl.LOCK_EX)
                    queue = read(queue_path)
                    if name not in {c['name'] for c in queue['cards']}:
                        queue['cards'].append({'name':name,'status':item['readiness'],'issue':entry['issue'],
                            'layout':item['layout'],'attempts':0,'listed':True,
                            'oracle_id':item['oracle_id'],'oracle_source':item['oracle_source'],'oracle_line':item['oracle_line'],
                            'generator_commit':queue['generator_commit'],'source_base':cfg['base']})
                        save(queue_path,queue)
                entry['phase'] = 'enqueued'; save(journal_path,journal)
            except Exception as exc:
                entry['error'] = str(exc); save(journal_path,journal)
                raise
    return journal


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', required=True)
    parser.add_argument('action', choices=['plan','add'])
    parser.add_argument('--card', action='append', default=[])
    parser.add_argument('--file', action='append', default=[])
    parser.add_argument('--set', action='append', default=[])
    parser.add_argument('--color')
    parser.add_argument('--oracle', type=Path, default=ORACLE)
    parser.add_argument('--fetch-set', action='store_true', help='fetch and cache missing complete set data')
    parser.add_argument('--reserve-gaps', action='store_true', help='explicitly reserve held cards too; default only reserves generator-ready cards')
    parser.add_argument('--set-data', type=Path, default=CATALOG, help='directory with MTGJSON set files or Unlimited catalog')
    args = parser.parse_args()
    names = args.card + files_names(args.file)
    if not names and not args.set: parser.error('provide --card, --file, or --set')
    result = plan(args.root,names,args.set,args.color,oracle=args.oracle,catalog=args.set_data,fetch=args.fetch_set or args.action == 'add')
    if args.action == 'add': add(args.root,result,oracle=args.oracle,reserve_gaps=args.reserve_gaps)
    print(json.dumps(result,ensure_ascii=False,indent=2))

if __name__ == '__main__': main()
