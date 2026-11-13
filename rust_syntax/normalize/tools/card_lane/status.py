#!/usr/bin/env python3
"""Recent card-lane work across projects. No git/network calls or hardcoded worker names."""
import argparse
import collections
import datetime as dt
import json
import os
from pathlib import Path
import shutil
import signal
import sys
import time

SKIP = {'.git', '.claude', 'target', 'targets', 'node_modules', '__pycache__', 'bin', 'sysroot', 'scryfall', 'scryfall.private', 'gen', 'generated', 'cantrip', 'auto', 'swap', 'cards', 'memo'}
STOPS = {'generator_bug', 'engine_gap', 'needs_review', 'failed_gates', 'timeout', 'infrastructure', 'error', 'abandoned'}

def read_json(path):
    try:
        return json.loads(path.read_text())
    except (OSError, ValueError):
        return None

def dispatchers(root, project=None):
    result = []
    for path in root.glob('*/dispatcher-state.json'):
        if project and project.casefold() not in path.parent.name.casefold():
            continue
        state = read_json(path)
        if isinstance(state, dict):
            result.append(dict(state, project=path.parent.name))
    return sorted(result, key=lambda item: natural_key(item['project']))

def discover(root):
    queues = []
    for folder, dirs, files in os.walk(root, followlinks=False):
        dirs[:] = [d for d in dirs if d not in SKIP and not d.startswith(('target-', 'capacity-recovery-', '.'))]
        if 'queue.json' in files:
            queues.append(Path(folder) / 'queue.json')
            dirs[:] = []  # Packets and mutation caches below a queue are never other runs.
    return queues

def pipelines(root, runs, project=None, active_only=False, process_state=None):
    process_state = process_state or {}
    result = []
    paths = set(root.glob('pipeline-*/status.json'))
    paths.update(p for p in root.glob('*/status.json') if (p.parent/'config.json').is_file())
    if (root/'config.json').is_file() and (root/'status.json').is_file():
        paths.add(root/'status.json')
    for path in sorted(paths, key=lambda p: natural_key(p.parent.name)):
        state = read_json(path)
        if not isinstance(state, dict) or not isinstance(state.get('counts'), list):
            continue
        config = read_json(path.parent / 'config.json') or {}
        lane_root = config.get('lane_root')
        if project and project.casefold() not in (path.parent.name + ' ' + str(lane_root or '')).casefold():
            continue
        active = state.get('active') if isinstance(state.get('active'), list) else []
        implementers = []
        slots = config.get('implementers') if isinstance(config.get('implementers'), dict) else {}
        for slot in sorted(slots, key=natural_key):
            live = read_json(path.parent / 'implementation' / slot / 'live.json') or {}
            held = live.get('state') != 'running' and (not config.get('enabled',True) or not config.get('implementation_enabled',True))
            reason = ('Generator refresh: new implementation claims held' if not config.get('implementation_enabled',True)
                      else 'Campaign draining: new claims held') if held else None
            implementers.append({'worker': slot, 'state': 'held' if held else live.get('state', 'unknown'), 'reason': reason,
                                 'card': live.get('card'), 'context_cards': live.get('context_cards'),
                                 'model': live.get('model'), 'context': live.get('context'),
                                 'started': timestamp(live.get('at'), 0) if live.get('state') == 'running' else None,
                                 'activity': process_state.get(str(path.parent/'implementation'/slot/'state'), {})})
        # Disabled historical campaigns retain their receipts, not an active-worker panel. A fresh
        # snapshot with unfinished work remains visible while a deliberate drain is in progress.
        fresh = time.time() - timestamp(state.get('at'),0) < 30
        if config.get('enabled') is False and not (fresh and (active or any(w['state']=='running' for w in implementers))):
            continue
        if active_only and not active and not any(w['state'] == 'running' for w in implementers):
            continue
        committed = set()
        if isinstance(lane_root, str):
            producer = Path(lane_root).resolve()
            for run in runs:
                if Path(run['state']).resolve().is_relative_to(producer):
                    committed.update(e['name'] for e in run['events'] if e['outcome'] == 'committed')
        queue = read_json(Path(config['implementation_queue'])) if isinstance(config.get('implementation_queue'), str) else None
        queue_counts = collections.Counter()
        if isinstance(queue, dict) and isinstance(queue.get('cards'), list):
            for card in queue['cards']:
                if isinstance(card, dict):
                    queue_counts[card.get('status', 'pending')] += 1
                    if card.get('status') == 'committed' and card.get('commit') and card.get('name'):
                        committed.add(card['name'])
        workers = []
        progress = {p['worker']: p.get('handled', 0) for p in state.get('worker_progress', []) if isinstance(p, dict) and p.get('worker')}
        outcomes = []
        latest_context = {}
        for event in state.get('recent') or []:
            if not isinstance(event, dict) or not isinstance(event.get('detail'), dict):
                continue
            metrics = event['detail'].get('context')
            if event.get('worker') and isinstance(metrics, dict) and isinstance(metrics.get('last_tokens'), int):
                latest_context.setdefault(event['worker'], metrics['last_tokens'])
            kind = event.get('kind')
            stage = event['detail'].get('stage', 'review')
            if kind in ('passed', 'queued', 'blocked', 'infrastructure') and event.get('name'):
                outcome = ('confirmed' if stage == 'confirm' else 'review passed') if kind == 'passed' else 'fixed → confirmation' if kind == 'queued' else f'{stage} {kind}'
                cost = ((state.get('costs') or {}).get('cards') or {}).get(event['name'], {})
                outcomes.append({'name': event['name'], 'outcome': outcome, 'detail': '', 'time': timestamp(event.get('at'), 0),
                                 'sequence': event.get('id', 0), 'run': f"{path.parent.name}/{event.get('worker') or '—'}", 'pipeline': True,
                                 'cost_usd': cost.get('usd_api_equivalent')})
        for job in active:
            if not isinstance(job, dict):
                continue
            worker = job.get('worker')
            # DB claims are authoritative; a live file can still describe the preceding job.
            live = read_json(path.parent / 'workers' / str(worker) / 'live.json') if worker and Path(str(worker)).name == worker else None
            if not isinstance(live, dict) or live.get('card') != job.get('name') or live.get('state') != 'running':
                live = {}
            assignment = read_json(path.parent/'workers'/str(worker)/'current.json') or {}
            activity = process_state.get(str(Path(assignment['folder'])/'state'), {}) if assignment.get('name') == job.get('name') and assignment.get('folder') else {}
            check = read_json(Path(assignment['folder'])/'checks-live.json') if assignment.get('folder') else None
            if not isinstance(check, dict) or check.get('card') != job.get('name') or check.get('state') != 'running' or time.time()-timestamp(check.get('at'), 0) > 120:
                check = {}
            workers.append({'worker': worker, 'state': 'running', 'card': job.get('name'), 'stage': job.get('stage'), 'activity': activity,
                            'check_progress': check,
                            'context_cards': live.get('context_cards'), 'model': live.get('model'), 'latest_context_tokens': latest_context.get(worker),
                            'handled': progress.get(worker, 0), 'started': timestamp(job.get('started'), 0)})
        present = {w['worker'] for w in workers}
        for slot, spec in (config.get('reviewers') or {}).items():
            if slot in present:
                continue
            live = read_json(path.parent / 'workers' / slot / 'live.json') or {}
            fresh = time.time() - timestamp(live.get('at'), 0) < 30
            ready = fresh and live.get('state') == 'waiting' and config.get('enabled', True)
            workers.append({'worker': slot, 'state': 'ready' if ready else 'stale', 'card': None,
                            'stage': 'confirm' if spec.get('role') == 'confirm' else 'review',
                            'context_cards': live.get('context_cards'), 'model': config.get('review_model'),
                            'latest_context_tokens': latest_context.get(slot), 'handled': progress.get(slot, 0), 'started': None})
        workers.sort(key=lambda w: natural_key(str(w['worker'] or '')))
        result.append({'name': config.get('display_name', path.parent.name), 'root': str(path.parent), 'implementation_committed': len(committed) if lane_root or queue_counts else None,
                       'implementation': state.get('implementation') or [], 'implementation_queue': dict(queue_counts),
                       'implementers': implementers, 'counts': state['counts'], 'workers': workers, 'cards_per_context': config.get('cards_per_context'),
                       'implementation_cards_per_context': config.get('implementation_cards_per_context'), 'events': outcomes,
                       'rate_per_hour': state.get('review_rate_per_hour'), 'elapsed_seconds': state.get('review_elapsed_seconds', 0),
                       'costs': state.get('costs') or {},
                       'phase_rates': state.get('phase_rates') or {},
                       'resources': read_json(path.parent/'resources'/'status.json') or {},
                       'flow': state.get('flow') if isinstance(state.get('flow'), dict) else None,
                       'workflow': config.get('workflow'),
                       'local_integration': read_json(path.parent/'integration/status.json') or {},
                       'integration_branch': (config.get('integration') or {}).get('branch'),
                       'publisher_batch_size': (config.get('publisher') or {}).get('batch_size', 20) if isinstance(config.get('publisher') or {}, dict) else 20})
    return result

def processes():
    """Match exact state paths, not 'w1' substrings from an unrelated run."""
    result = collections.defaultdict(lambda: {'driver': [], 'model': []})
    for entry in Path('/proc').iterdir():
        if not entry.name.isdigit():
            continue
        try:
            argv = (entry / 'cmdline').read_bytes().split(b'\0')
            driver = (any(x.endswith(b'card_lane.py') for x in argv) and (b'batch-run' in argv or b'run' in argv)) or any(x.endswith(b'retry_capacity.py') for x in argv)
            model = any(Path(os.fsdecode(x)).name == 'codex' for x in argv[:2]) and b'exec' in argv
            binary = Path(os.fsdecode(argv[0])).name if argv and argv[0] else ''
            tool = binary in {'cargo','rustc','corpus','llvm-cov'} or any(x.endswith((b'card_lane.py',b'review_lane.py')) for x in argv) and any(x in argv for x in (b'check',b'corpus',b'tool',b'commit')) and b'worker' not in argv
            if not (driver or model or tool):
                continue
            state = next((os.fsdecode(x.split(b'=', 1)[1]) for x in (entry / 'environ').read_bytes().split(b'\0') if x.startswith(b'CARD_LANE_STATE=')), None)
            if not state:
                continue
            if tool:
                args = [os.fsdecode(x) for x in argv if x]
                detail = ' '.join(args[1:5]) if binary in {'cargo','corpus'} else 'compiling Rust' if binary == 'rustc' else ' '.join(args[2:6])
                result[state].setdefault('tools',[]).append({'pid':int(entry.name),'detail':detail})
            if driver or model:
                result[state]['driver' if driver else 'model'].append(int(entry.name))
        except (OSError, ValueError):
            continue
    return dict(result)

def timestamp(value, default):
    if isinstance(value, (float, int)):
        return value
    if isinstance(value, str):
        try:
            return dt.datetime.fromisoformat(value.replace('Z', '+00:00')).timestamp()
        except ValueError:
            pass
    return default

def assigned_names(state):
    """A worker's assignment, not the generator's entire candidate queue."""
    worker = state.parent if state.name == 'state' else state
    assignment = read_json(worker / 'assignment.json')
    if isinstance(assignment, dict) and isinstance(assignment.get('cards'), list):
        names = [item.get('name') if isinstance(item, dict) else item for item in assignment['cards']]
        if names:
            return set(filter(None, names)), 'assignment.json'
    try:
        names = [line.strip() for line in (worker / 'names.txt').read_text().splitlines() if line.strip() and not line.lstrip().startswith('#')]
        if names:
            return set(names), 'names.txt'
    except OSError:
        pass
    return None, 'queue'

def natural_key(value):
    return tuple(int(part) if part.isdigit() else part.casefold() for part in __import__('re').split(r'(\d+)', value))

def stream_event_times(stream):
    """Old streams record durations, not close times. Walk backward from the
    next card's opening so file touches cannot make past outcomes new again.
    Explicit close timestamps are preferred when a newer writer supplies them.
    """
    clock = timestamp(stream.get('opened'), 0)
    result = {}
    for name, item in reversed(list((stream.get('closed') or {}).items())):
        closed = timestamp(item.get('closed_at') or item.get('finished'), clock)
        result[name] = closed
        try:
            seconds = max(0, float(item.get('seconds', 0)))
        except (ValueError, TypeError):
            seconds = 0
        clock = closed - seconds
    return result

def rate_stats(runs):
    """Successes over wall-clock time, not summed parallel worker time."""
    eligible = [r for r in runs if r.get('rate_start', 0) > 0]
    if not eligible:
        return {'validated': sum(r['committed'] + r['passed_waiting'] for r in runs), 'elapsed_seconds': 0, 'per_hour': None}
    start = min(r['rate_start'] for r in eligible)
    end = max(r['rate_end'] for r in eligible)
    elapsed = max(0, end - start)
    validated = sum(r['committed'] + r['passed_waiting'] for r in eligible)
    return {'validated': validated, 'elapsed_seconds': elapsed, 'per_hour': 3600 * validated / elapsed if elapsed else None}

def format_rate(per_hour, compact=False):
    if per_hour is None:
        return '—'
    if per_hour >= 60:
        return f'{per_hour / 60:.1f}/min' if compact else f'{per_hour / 60:.1f} cards/min'
    return f'{per_hour:.1f}/h' if compact else f'{per_hour:.1f} cards/hour'

def collect(root, live=None):
    live = processes() if live is None else live
    runs = []
    for path in discover(root):
        queue = read_json(path)
        if not isinstance(queue, dict) or not isinstance(queue.get('cards'), list):
            continue
        state = path.parent
        stream_path = state / 'batch/stream.json'
        stream = read_json(stream_path) or {}
        try:
            modified = max(path.stat().st_mtime, stream_path.stat().st_mtime if stream_path.exists() else 0)
        except OSError:
            continue
        cards = [c for c in queue['cards'] if isinstance(c, dict) and c.get('name')]
        backlog_total = len(cards)
        scope, scope_source = assigned_names(state)
        if scope is not None:
            cards = [c for c in cards if c['name'] in scope]
        by_name = {c['name']: c for c in cards}
        closed = {n: item for n, item in (stream.get('closed') or {}).items() if n in by_name}
        counts = collections.Counter(c.get('status', 'pending') for c in cards)
        committed = {c['name'] for c in cards if c.get('status') == 'committed'}
        passed = {n for n, item in closed.items() if item.get('outcome') == 'passed'} - committed
        stopped = {c['name'] for c in cards if c.get('status') in STOPS}
        stopped |= {n for n, item in closed.items() if item.get('outcome') in STOPS}
        stopped -= committed
        passed -= stopped
        running = live.get(str(state), {'driver': [], 'model': []})
        alive = bool(running['driver'] or running['model'])
        current = stream.get('current') if stream.get('current') in by_name else None
        # Untouched generator queues are backlog, not worker runs.
        if not alive and scope is None and cards and not committed and not passed and not stopped and not current and all(c.get('status', 'pending') == 'pending' for c in cards) and not any(c.get('attempts') for c in cards):
            continue
        if alive:
            phase = 'running' if running['model'] else 'checking' if current else 'preparing'
        elif cards and all(c.get('status') == 'committed' for c in cards):
            phase = 'complete'
        elif cards and len(committed | passed | stopped) == len(cards):
            phase = 'finished'
        else:
            phase = 'stopped'
        label_path = state.parent if state.name == 'state' else state
        label = str(label_path.relative_to(root)) or '.'
        events = []
        sequence = {name: i for i, name in enumerate(closed)}
        closed_times = stream_event_times(stream)
        started = max((timestamp(c.get('started') or c.get('began'), 0) for c in cards), default=0) or timestamp(queue.get('built'), 0)
        group = label.rsplit('/', 1)[0] if '/' in label else label
        starts = [timestamp(c.get('started') or c.get('began'), 0) for c in cards]
        starts = [value for value in starts if value > 0]
        rate_start = min(starts) if starts else timestamp(queue.get('built'), 0)
        finishes = [timestamp(c.get('finished'), 0) for c in cards]
        rate_end = time.time() if alive else max(finishes + [timestamp(stream.get('opened'), 0)])
        if not rate_end:
            rate_end = modified
        for index, (name, c) in enumerate(by_name.items()):
            outcome = c.get('status', 'pending')
            item = closed.get(name) or {}
            if name not in committed and outcome not in STOPS and item:
                outcome = item.get('outcome', outcome)
            if outcome not in STOPS | {'passed', 'committed'}:
                continue
            events.append({'name': name, 'outcome': outcome, 'detail': str(c.get('failure') or item.get('detail') or ''), 'commit': c.get('commit'), 'time': timestamp(c.get('finished'), started) if name in committed else closed_times.get(name, timestamp(c.get('finished'), started)), 'sequence': sequence.get(name,index), 'run': label})
        runs.append({'run': label, 'format': queue.get('format'), 'state': str(state), 'phase': phase, 'active': alive, 'pids': running, 'modified': modified, 'started': started, 'group': group, 'rate_start': rate_start, 'rate_end': rate_end, 'validated': len(committed | passed), 'rate_per_hour': 3600 * len(committed | passed) / (rate_end - rate_start) if rate_start and rate_end > rate_start else None, 'total': len(cards), 'backlog_total': backlog_total, 'assignment_source': scope_source, 'committed': len(committed), 'passed_waiting': len(passed), 'blocked': len(stopped), 'done': len(committed | passed | stopped), 'counts': dict(counts), 'current': current, 'current_since': timestamp(stream.get('opened'), modified) if current else None, 'tries': (stream.get('tries') or {}).get(current, 0), 'events': events})
    group_started = {}
    for run in runs:
        group_started[run['group']] = max(group_started.get(run['group'], 0), run['started'])
    return sorted(runs, key=lambda r: (not r['active'], -group_started[r['group']], natural_key(r['group']), natural_key(r['run'])))

def shorten(text, width):
    text = ' '.join(str(text).split())
    return text if len(text) <= width else text[:max(0, width - 1)] + '…'

ANSI = __import__('re').compile(r'\x1b\[[0-?]*[ -/]*[@-~]')
PALETTE = {'dim': '2;37', 'title': '1;97', 'cyan': '1;36', 'green': '1;32', 'yellow': '1;33', 'red': '1;31', 'blue': '34'}

def render(runs, root, interval=None, event_limit=8, width=120, height=None, color=False, dispatch=None, pipeline=None):
    width = max(60, min(width, 180))
    inner = width - 4
    def paint(text, tone):
        return f'\033[{PALETTE[tone]}m{text}\033[0m' if color else text
    def fit(text, size):
        plain = ANSI.sub('', text)
        if len(plain) > size:
            # Clip styled text without cutting an escape sequence in half.
            parts = __import__('re').split(r'(\x1b\[[0-?]*[ -/]*[@-~])', text)
            result, used = '', 0
            for part in parts:
                if ANSI.fullmatch(part):
                    result += part
                elif used + len(part) <= size - 1:
                    result += part; used += len(part)
                else:
                    result += part[:max(0, size - 1 - used)] + '…'
                    return result + ('\033[0m' if color else '')
            return result
        return text + ' ' * (size - len(plain))
    def row(text=''):
        return paint('│ ', 'blue') + fit(text, inner) + paint(' │', 'blue')
    def rule(title=''):
        title = (' ' + title + ' ') if title else ''
        return paint('├' + title + '─' * max(0, width - 2 - len(title)) + '┤', 'blue')
    def tone(run):
        return 'cyan' if run['active'] else 'green' if run['phase'] == 'complete' else 'yellow' if run['blocked'] else 'dim'
    def progress(run, size=10):
        total = max(1, run['total'])
        result, previous, count = '', 0, 0
        for number, shade in [(run['committed'],'green'),(run['passed_waiting'],'cyan'),(run['blocked'],'red')]:
            count += number; boundary = min(size, round(size * count / total))
            result += paint('━' * (boundary - previous),shade); previous=boundary
        return result + paint('─' * (size - previous),'dim')
    def current_text(run):
        current = run['current'] or ('Preparing cards / compiling' if run['active'] else '—')
        if run['current_since']:
            current += f" · {max(0, int((time.time() - run['current_since']) / 60))}m · {run['tries']} retries"
        return current
    now = dt.datetime.now().astimezone().strftime('%H:%M:%S %Z')
    active = [r for r in runs if r['active'] and not any(p.get('root') and Path(r['state']).is_relative_to(Path(p['root'])) for p in pipeline or [])]
    recent = [r for r in runs if not r['active']]
    active_groups = {r['group'] for r in active}
    summary_runs = [r for r in runs if r['group'] in active_groups] if active_groups else runs
    batch_rate = rate_stats(summary_runs)
    summary = '   '.join([paint(f'{len(active)} LIVE','green'),paint(f"{sum(r['total'] for r in summary_runs)} ASSIGNED",'title'),paint(f"{sum(r['committed'] for r in summary_runs)} COMMITTED",'green'),paint(f"{sum(r['passed_waiting'] for r in summary_runs)} PASSED*",'cyan'),paint(f"{sum(r['blocked'] for r in summary_runs)} BLOCKED",'red')])
    if pipeline:
        totals = collections.Counter()
        for pipe in pipeline:
            for item in pipe['counts']:
                totals[item.get('status')] += item.get('cards', 0)
        live_count = len(active) + sum(sum(w.get('state', 'running') == 'running' for w in p['workers']) + sum(w['state']=='running' for w in p['implementers']) for p in pipeline)
        flows = [p['flow'] for p in pipeline if p.get('flow')]
        if len(flows) == len(pipeline):
            ready = sum(f.get('ready', {}).get('cards', 0) for f in flows)
            blocked = sum(f.get('blocked', {}).get('cards', 0) for f in flows)
            summary = f"{live_count} LIVE   {sum(p['implementation_committed'] or 0 for p in pipeline)} IMPLEMENTED   {ready} PR READY   " + paint(f'{blocked} BLOCKED','red')
            if all(p.get('workflow') == 'consolidate_then_publish' for p in pipeline):
                staged = sum((p.get('local_integration', {}).get('verified') or {}).get('collected', 0) for p in pipeline)
                summary = f"{live_count} LIVE   {staged} VERIFIED LOCALLY   " + paint(f'{blocked} BLOCKED','red') + '   PUBLISH AT END'
        else:
            summary = f"{live_count} LIVE   {sum(p['implementation_committed'] or 0 for p in pipeline)} IMPLEMENTED   {totals['passed']} REVIEW READY   " + paint(f"{totals['blocked']} BLOCKED",'red') + f"   {totals['queued']} QUEUED"
    lines = [paint('╭' + '─' * (width - 2) + '╮', 'blue'), row(paint('CARD LANE', 'title') + paint('  /  recent card work', 'dim') + ' ' + paint(now, 'cyan')), row(summary)]
    elapsed = int(batch_rate['elapsed_seconds'])
    if pipeline:
        rates = [p['rate_per_hour'] for p in pipeline if p.get('rate_per_hour') is not None]
        priced = [p['costs'] for p in pipeline if p.get('costs')]
        completed = sum(c.get('completed_cards', 0) for c in priced)
        avg = sum(c.get('completed_usd', 0) for c in priced) / completed if completed else None
        cost_text = f"AVG ${avg:.3f}/card · {completed} completed" if avg is not None else 'AVG —/card'
        total = sum(c.get('total_usd', 0) for c in priced)
        cost_text += f" · TOTAL ${total:.2f} · API est" if priced else ' · awaiting usage'
        lines.append(row(paint(f"READY RATE  {format_rate(sum(rates) if rates else None)}  ·  {cost_text}", 'cyan')))
        phase_pipes = [p['phase_rates'] for p in pipeline if p.get('phase_rates')]
        if phase_pipes:
            periods = {p.get('window_seconds', 1800) for p in phase_pipes}
            window = f"{next(iter(periods))/60:g}m" if len(periods) == 1 else 'rolling'
            parts = []
            for name, label in [('implementation','IMPL'), ('review','REVIEW'), ('fix','FIX'), ('verify','VERIFY')]:
                rate = sum(p.get('phases', {}).get(name, {}).get('advanced_per_hour', 0) for p in phase_pipes)
                parts.append(f'{label} {format_rate(rate, True)}')
            line = f'PHASE RATES ({window})  '
            for part in parts:
                extra = (' · ' if not line.endswith('  ') else '') + part
                if len(line + extra) > inner:
                    lines.append(row(paint(line, 'cyan')))
                    line = '  ' + part
                else:
                    line += extra
            lines.append(row(paint(line, 'cyan')))
        for pipe in pipeline:
            resource = pipe.get('resources') or {}
            if resource and time.time()-timestamp(resource.get('at'),0) < 30:
                roles = resource.get('roles', {})
                cpu = ' · '.join(f"{label} {roles.get(role,{}).get('cpu_cores',{}).get('mean',0):.1f}" for role,label in [('implement','IMPL'),('review','REVIEW'),('verify','VERIFY')])
                host = (resource.get('host_cpu_cores') or {}).get('mean')
                if host is not None:
                    lines.append(row(paint(f"CPU avg ({resource.get('sampled_seconds',0)/60:.1f}m)  {cpu} · HOST {host:.1f}/{resource.get('logical_cpus','?')} logical CPUs", 'cyan')))
    else:
        lines.append(row(paint(f"VALIDATED RATE  {format_rate(batch_rate['per_hour'])}  ·  {batch_rate['validated']} passed/committed  ·  {elapsed // 3600}h {(elapsed % 3600) // 60:02d}m elapsed", 'cyan')))
    for controller in dispatch or []:
        age = time.time() - timestamp(controller.get('checked_at'), 0)
        state = 'STALE' if age > 30 else 'ON' if controller.get('enabled') else 'PAUSED'
        info = f"AUTO {state}  {controller['project']} · {controller.get('active', 0)}/{controller.get('max_workers', 0)} slots · next {controller.get('batch_size', 0)} · {controller.get('eligible_waiting', 0)} waiting · {controller.get('generator_gaps', 0)} gaps"
        lines.append(row(paint(info, 'yellow' if state != 'ON' else 'cyan')))
    if pipeline:
        for pipe in pipeline:
            implementers = pipe['implementers']
            if implementers:
                jobs = collections.Counter({item.get('status'): item.get('cards', 0) for item in pipe['implementation'] if isinstance(item, dict) and isinstance(item.get('cards'), int)})
                queue = pipe['implementation_queue']
                held = ' · '.join(f"{queue[key]} {label}" for key, label in [('generator_gap', 'generator gaps'), ('deferred_blocked', 'deferred')] if queue.get(key))
                info = f"implement {sum(w['state'] == 'running' for w in implementers)}/{len(implementers)} active · {jobs['queued']} queued · {jobs['committed']} new committed"
                if not pipe.get('flow'):
                    lines.append(row(paint(info + (' · ' + held if held else ''), 'cyan')))
            counts = collections.Counter()
            for item in pipe['counts']:
                if isinstance(item, dict) and isinstance(item.get('cards'), int):
                    counts[(item.get('stage'), item.get('status'))] += item['cards']
            stages = []
            for stage in sorted({stage for stage, _ in counts if stage}, key=natural_key):
                values = ', '.join(f'{status} {counts[(stage, status)]}' for status in ('queued', 'running', 'passed', 'blocked', 'infrastructure') if counts[(stage, status)])
                others = sorted((s for st, s in counts if st == stage and s not in ('queued', 'running', 'passed', 'blocked', 'infrastructure') and s), key=natural_key)
                values += ''.join(f", {s} {counts[(stage, s)]}" for s in others)
                stages.append(f'{stage} {values.lstrip(", ")}')
            committed = pipe['implementation_committed']
            ready = sum(n for (stage, state), n in counts.items() if state == 'passed')
            total = sum(counts.values())
            if not pipe.get('flow'):
                lines.append(row(paint(f"{pipe['name']}  {ready}/{total} review ready · impl committed {committed if committed is not None else '—'}", 'cyan')))
                for stage in stages:
                    lines.append(row(paint('  ' + stage, 'cyan')))
            lines.append(rule('ACTIVE WORKERS · ' + pipe['name']))
            lines.append(row(paint('  WORKER  ROLE       PHASE        ELAPSED     COST  CARD / CURRENT CHECK', 'dim')))
            unified = [(w, 'implement') for w in implementers] + [(w, 'verify' if w.get('stage') == 'confirm' else 'review') for w in pipe['workers']]
            for worker, role in unified:
                state = worker.get('state', 'running')
                activity = worker.get('activity', {})
                tools = activity.get('tools', [])
                if state == 'running':
                    check = worker.get('check_progress') or {}
                    phase = 'REGRESSION' if check.get('phase') == 'caller regressions' else 'CHECKING' if tools or check else 'WORKING'
                    age = max(0, int(time.time() - worker.get('started', time.time()))) if worker.get('started') else 0
                    elapsed = f'{age//60}m{age%60:02d}s'
                    detail = ' · ' + tools[0]['detail'] if tools else ''
                    if check:
                        detail = f" · {check.get('card_index')}/{check.get('total_cards')} {check.get('gate','')}"
                    card = worker.get('card') or 'Preparing card'
                else:
                    phase = 'HELD' if state == 'held' else 'READY' if state in ('ready', 'waiting') else 'STALE'
                    elapsed, detail = '—', ''
                    card = worker.get('reason') or ('waiting for confirmation' if role == 'verify' else 'waiting for a card')
                    if phase == 'STALE':card = 'no recent heartbeat'
                amount = ((pipe.get('costs') or {}).get('cards') or {}).get(worker.get('card'), {}).get('usd_api_equivalent')
                cost = f'${amount:.3f}' if amount is not None else '—'
                identity = f"  {worker['worker']:<6}  {role:<9}  {phase:<11}  {elapsed:>6}  {cost:>7}  {card}{detail}"
                lines.append(row(identity))
    if active:
        lines.append(rule('ACTIVE WORKERS'))
        for r in active:
            label_width = 22 if width < 100 else 27
            info = f"{shorten(r['run'],label_width):<{label_width}} {r['phase'].upper():<9} {r['done']:>2}/{r['total']:<3} "
            stats = '  '+paint(f"C:{r['committed']}",'green')+'  '+paint(f"P:{r['passed_waiting']}",'cyan')+'  '+paint(f"B:{r['blocked']}",'red' if r['blocked'] else 'dim')
            lines.append(row(paint(info, tone(r)) + progress(r,8 if width < 100 else 10) + stats + '  '+paint(format_rate(r.get('rate_per_hour'),True),'cyan') + ('  '+current_text(r) if width >= 130 else '')))
            if width < 130:
                lines.append(row(paint('  ↳ ' + current_text(r), 'dim')))
    elif not runs and not pipeline:
        lines.append(row('No matching card-lane runs found.'))
    events = sorted([e for r in runs for e in r['events']] + [e for p in pipeline or [] for e in p.get('events',[])], key=lambda e: (e['time'],e.get('sequence',0)), reverse=True)
    # Prioritize all active workers; fit history/activity within the live terminal.
    reserved = min(event_limit, len(events)) + 1 if events and event_limit else 0
    budget = max(0, height - len(lines) - 5 - reserved) if height else 10000
    if pipeline:
        lines.append(rule('CARD FLOW'))
        for pipe in pipeline:
            flow = pipe.get('flow') or {}
            def node(name):
                value = flow.get(name)
                return value if isinstance(value, dict) else {}
            def cards(name):
                return node(name).get('cards', 0)
            def rounds(name):
                values = node(name).get('rounds')
                if not isinstance(values, list) or len(values) < 4:
                    return 'rounds unavailable'
                return ' '.join(f'R{i}:{values[i]}' for i in range(4))
            def workers(name):
                value = node(name)
                return f"{value.get('active_workers', 0)}/{value.get('workers', 0)}"
            if flow:
                review, fix = node('review'), node('fix')
                pool = max(review.get('workers', 0), fix.get('workers', 0))
                busy = min(pool, review.get('active_workers', 0) + fix.get('active_workers', 0))
                ready = cards('ready')
                batch = pipe.get('publisher_batch_size', 20)
                gather = f'gather {ready}/{batch}' if ready <= batch else f'{ready} ready · next {batch}'
                intake = node('intake')
                verify_rounds = node('verify').get('rounds')
                if isinstance(verify_rounds, list) and len(verify_rounds) >= 4:
                    verify_hist = [f'R0:{verify_rounds[0]} R1:{verify_rounds[1]}', f'R2:{verify_rounds[2]} R3:{verify_rounds[3]}']
                else:
                    verify_hist = ['rounds unavailable']
                phase_data = (pipe.get('phase_rates') or {}).get('phases', {})
                def rate_lines(name):
                    values = phase_data.get(name, {})
                    if not values:
                        return []
                    mean = values.get('mean_seconds')
                    return [format_rate(values.get('advanced_per_hour'), True) + ' (30m)',
                            f'{mean/60:.1f}m/card' if mean is not None else 'no finishes']
                blocked_rounds = node('blocked').get('rounds')
                blocked_hist = [f'R{i}: {v}' for i,v in enumerate(blocked_rounds[:4])] if isinstance(blocked_rounds, list) else []
                boxes = [
                    (12, ['INTAKE', f"Queued {intake.get('queued', cards('intake'))}", f"Held {intake.get('held', 0)}"]),
                    (16, ['IMPLEMENT', f"{workers('implementation')} active", f"{cards('implementation')} cards", *rate_lines('implementation')]),
                    (26, ['REVIEW / FIX', f'{busy}/{pool} active (shared)', f"Review {cards('review')} cards", rounds('review'), f"Fix {cards('fix')} cards", rounds('fix')]),
                    (14, ['VERIFY', f"{workers('verify')} active", f"{cards('verify')} cards", *verify_hist, *rate_lines('verify')[:1]]),
                    (20, ['PR', gather, f"publishing {cards('publishing')}", f"published {cards('published')}"]),
                    (12, ['BLOCKED', f"{cards('blocked')} cards", *blocked_hist]),
                ]
                if pipe.get('workflow') == 'consolidate_then_publish':
                    local = (pipe.get('local_integration') or {}).get('counts') or {}
                    verified = (pipe.get('local_integration') or {}).get('verified') or {}
                    boxes[4] = (20, ['LOCAL', f"verified {verified.get('collected', 0)}",
                                    f"to collect {verified.get('to_collect', 0)}", f"upstream {verified.get('upstream', 0)}",
                                    f"held {local.get('needs_resolution', 0)}", 'publish at end'])
                def box_rows(group):
                    def cell(box, index):
                        size, contents = box
                        if index == 0:
                            return '┌' + '─' * (size - 2) + '┐'
                        if index == 7:
                            return '└' + '─' * (size - 2) + '┘'
                        content = contents[index - 1] if index <= len(contents) else ''
                        return '│' + shorten(content, size - 2).ljust(size - 2) + '│'
                    for index in range(8):
                        cells = []
                        for position, box in enumerate(group):
                            if position:
                                connector = ' → ' if index == 3 and box[1][0] != 'BLOCKED' else '   '
                                if index == 4 and group[position-1][1][0] == 'REVIEW / FIX' and box[1][0] == 'VERIFY':
                                    connector = ' ← '
                                cells.append(connector)
                            tone = 'red' if box[1][0] == 'BLOCKED' else 'yellow' if box[1][0] == 'LOCAL' else 'green' if box[1][0] == 'PR' else 'cyan'
                            cells.append(paint(cell(box, index), tone))
                        lines.append(row(''.join(cells)))
                if width >= 119:
                    box_rows(boxes)
                else:
                    box_rows(boxes[:2])
                    lines.append(row(paint('                  ↓', 'cyan')))
                    box_rows(boxes[2:4])
                    lines.append(row(paint('                  ↓', 'cyan')))
                    box_rows(boxes[4:])
            else:
                lines.append(row(paint('IMPLEMENT → REVIEW / FIX → VERIFY → PR', 'cyan')))
                lines.append(row('  Round breakdown unavailable in this snapshot'))
    elif recent and budget >= 3:
        lines.append(rule('RECENT RUNS'))
        lines.append(row(paint(f"{'RUN / WORKER':<30} {'STATE':<10} {'DONE':>7} {'COMMIT':>7} {'PASSED*':>8} {'BLOCKED':>8} {'RATE':>10}", 'dim')))
        count = min(len(recent), max(1, budget - 4))
        for r in recent[:count]:
            text = f"{r['run']:<30} {r['phase']:<10} {r['done']:>3}/{r['total']:<3} {r['committed']:>7} {r['passed_waiting']:>8} {r['blocked']:>8} {format_rate(r.get('rate_per_hour'),True):>10}"
            lines.append(row(paint(text, tone(r))))
        if count < len(recent):
            lines.append(row(paint(f'  + {len(recent)-count} older runs · use --once or a taller terminal', 'dim')))
    budget = max(0, height - len(lines) - 4) if height else event_limit + 1
    if events and event_limit and budget >= 2:
        lines.append(rule('RECENT OUTCOMES'))
        for e in events[:min(event_limit, budget - 1)]:
            successful = e['outcome'] in ('committed', 'confirmed', 'review passed')
            pending = e['outcome'] in ('passed', 'fixed → confirmation')
            mark = '✓' if successful else '○' if pending else '!'
            detail = (' ' + e['commit'][:10]) if e.get('commit') else (': ' + e['detail'] if e['detail'] else '')
            clock = dt.datetime.fromtimestamp(e['time']).astimezone().strftime('%H:%M:%S') if e['time'] else 'unknown'
            label = e['run'].removeprefix('pipeline-') if e.get('pipeline') else e['run']
            cost = f" · ${e['cost_usd']:.3f}" if e.get('cost_usd') is not None else ''
            text = f"{clock} {mark} {label} · {e['name']}{cost} · {e['outcome']}{detail}"
            lines.append(row(paint(shorten(text, inner), 'green' if successful else 'cyan' if pending else 'yellow')))
    lines.append(rule())
    refresh = f'every {interval:g}s · Ctrl-C to exit' if interval else '--watch 1 for live view'
    lines.append(row(paint(f'{refresh}   |   * passed ≠ committed   |   C commit · P passed · B blocked', 'dim')))
    lines.append(paint('╰' + '─' * (width - 2) + '╯', 'blue'))
    return '\n'.join(lines)


def render_costs(pipeline):
    lines = ['Standard API-equivalent estimates · recorded attempts including retries',
             'Average uses completed cards; older batch implementation costs are apportioned.',
             f"{'CARD':<42} {'IMPLEMENT':>10} {'REVIEW/FIX':>10} {'VERIFY':>10} {'TOTAL':>10}  STATUS"]
    for pipe in pipeline:
        lines.append(pipe['name'])
        for name, cost in sorted((pipe.get('costs') or {}).get('cards', {}).items()):
            if cost.get('usd_api_equivalent') is None:
                continue
            amounts = [cost['stages'].get(s, 0) for s in ('implement', 'review', 'verify')]
            amounts.append(cost['usd_api_equivalent'])
            money = ' '.join(f'${v:>9.4f}' for v in amounts)
            label = cost['status'] + (' · partial' if cost.get('unpriced_attempts') else '')
            lines.append(f'{name:<42} {money}  {label}')
    return '\n'.join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(os.environ.get('CARD_LANE_HOME', Path.home() / 'card-lane')))
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument('--once', action='store_true', help='one snapshot, then exit')
    mode.add_argument('--watch', type=float, nargs='?', const=1.0, metavar='SECONDS', help='refresh continuously (default 1 second)')
    parser.add_argument('--interval', type=float, default=1.0, help='refresh period when run in a terminal')
    parser.add_argument('--project', help='show only matching project/run names')
    parser.add_argument('--active', action='store_true', help='only live worker processes')
    parser.add_argument('--limit', type=int, default=12, help='most recent runs, active first; 0 for all')
    parser.add_argument('--events', type=int, default=8, help='recent outcomes to show')
    parser.add_argument('--color', choices=['auto','always','never'], default='auto', help='ANSI colors (default: terminal only)')
    parser.add_argument('--json', action='store_true', help='one machine-readable snapshot')
    parser.add_argument('--costs', action='store_true', help='per-card implementation, review/fix and verifier cost estimates')
    args = parser.parse_args()
    interval = args.watch if args.watch is not None else args.interval
    if interval <= 0 or args.limit < 0 or args.events < 0:
        parser.error('interval must be positive; limits cannot be negative')
    watch = not (args.once or args.json or args.costs) and (args.watch is not None or sys.stdout.isatty())
    root = args.root.expanduser().resolve()
    tty = sys.stdout.isatty()
    source = Path(__file__).resolve()
    source_stamp = source.stat().st_mtime_ns
    signal.signal(signal.SIGTERM, lambda *_: (_ for _ in ()).throw(KeyboardInterrupt()))
    try:
        if watch and tty:
            print('\033[?1049h\033[?25l', end='', flush=True)
        while True:
            if watch and source.stat().st_mtime_ns != source_stamp:
                try:
                    compile(source.read_text(), str(source), 'exec')
                except (OSError, SyntaxError):
                    pass  # Wait for an editor to finish writing a valid update.
                else:
                    if tty:
                        print('\033[0m\033[?25h\033[?1049l', end='', flush=True)
                    os.execv(sys.executable, [sys.executable, str(source), *sys.argv[1:]])
            start = time.monotonic()
            live_processes = processes()
            runs = collect(root, live_processes)
            controllers = dispatchers(root, args.project)
            pipeline = pipelines(root, runs, args.project, args.active, live_processes)
            if args.project:
                runs = [r for r in runs if args.project.casefold() in r['run'].casefold()]
            if args.active:
                runs = [r for r in runs if r['active']]
            if args.limit:
                runs = runs[:args.limit]
            if args.json:
                print(json.dumps({'root': str(root), 'runs': runs, 'dispatchers': controllers, 'pipelines': pipeline}, indent=2))
            elif args.costs:
                print(render_costs(pipeline))
            else:
                if watch and tty:
                    print('\033[H\033[2J', end='')
                size = shutil.get_terminal_size((120, 40))
                colored = args.color == 'always' or args.color == 'auto' and tty and 'NO_COLOR' not in os.environ
                print(render(runs, root, interval if watch else None, args.events, size.columns, size.lines if watch and tty else None, colored, controllers, pipeline), flush=True)
            if not watch:
                break
            time.sleep(max(0.01, interval - (time.monotonic() - start)))
    except KeyboardInterrupt:
        pass
    finally:
        if watch and tty:
            print('\033[0m\033[?25h\033[?1049l', end='', flush=True)
    return 0

if __name__ == '__main__':
    raise SystemExit(main())
