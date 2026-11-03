#!/usr/bin/env python3
"""Sample worker cgroups, including short-lived build/test descendants."""
import argparse
from collections import deque
import json
import os
from pathlib import Path
import subprocess
import time

import review_lane

CGROUP = Path('/sys/fs/cgroup')
GIB = 1024**3


def counters(path):
    try:
        return {parts[0]: int(parts[1]) for line in path.read_text().splitlines()
                if len(parts := line.split()) >= 2 and parts[1].isdigit()}
    except OSError:
        return {}


def integer(path):
    try:
        return int(path.read_text().strip())
    except (OSError, ValueError):
        return None


def discover(root, prefix):
    cfg = review_lane.config(root)
    roles = {f'{prefix}-implement-{slot}.service': 'implement' for slot in cfg.get('implementers', {})}
    roles.update({f'{prefix}-review-{slot}.service': 'verify' if spec.get('role') == 'confirm' else 'review'
                  for slot, spec in cfg.get('reviewers', {}).items()})
    if not roles:
        return {}
    result = subprocess.run(['systemctl', '--user', 'show', *roles, '-p', 'Id', '-p', 'ControlGroup'],
                            text=True, capture_output=True)
    if result.returncode and not result.stdout.strip():
        result.check_returncode()
    groups = {unit:dict(role=role,path=None) for unit,role in roles.items()}
    for block in result.stdout.strip().split('\n\n'):
        fields = dict(line.split('=', 1) for line in block.splitlines() if '=' in line)
        unit, path = fields.get('Id'), fields.get('ControlGroup')
        if unit in roles:
            groups[unit] = dict(role=roles[unit], path=path or None)
    return groups


def snapshot(groups):
    workers = {}
    for unit, spec in groups.items():
        path = CGROUP / spec['path'].lstrip('/') if spec['path'] else None
        cpu = counters(path/'cpu.stat') if path else {}
        memory = counters(path/'memory.stat') if path else {}
        workers[unit] = dict(role=spec['role'], cgroup=spec['path'], cpu_usec=cpu.get('usage_usec'),
                             throttled_usec=cpu.get('throttled_usec'),
                             memory_bytes=integer(path/'memory.current') if path else None,
                             anon_bytes=memory.get('anon'), shmem_bytes=memory.get('shmem'))
    fields = Path('/proc/stat').read_text().splitlines()[0].split()[1:]
    ticks = list(map(int, fields[:8]))  # guest counters already included in user/nice
    memory = counters(Path('/proc/meminfo'))
    pressure = {}
    for name in ('cpu', 'memory', 'io'):
        try:
            for line in (Path('/proc/pressure')/name).read_text().splitlines():
                fields = line.split()
                pressure[f'{name}_{fields[0]}'] = int(dict(v.split('=') for v in fields[1:])['total'])
        except OSError:
            pass
    vm = counters(Path('/proc/vmstat'))
    return dict(at=review_lane.now(), monotonic=time.monotonic(), workers=workers,
                host=dict(cpu_total=sum(ticks), cpu_busy=sum(ticks)-ticks[3]-ticks[4], cpu_iowait=ticks[4],
                          cpus=os.cpu_count(), affinity_cpus=len(os.sched_getaffinity(0)),
                          memory_total_bytes=memory.get('MemTotal:', 0)*1024,
                          memory_available_bytes=memory.get('MemAvailable:', 0)*1024,
                          swap_in_pages=vm.get('pswpin', 0), swap_out_pages=vm.get('pswpout', 0),
                          pressure_usec=pressure, load=os.getloadavg()))


def interval(previous, current):
    elapsed = current['monotonic'] - previous['monotonic']
    if elapsed <= 0:
        raise ValueError('nonpositive sampling interval')
    result = dict(at=current['at'], seconds=elapsed, workers={}, roles={})
    for unit, worker in current['workers'].items():
        before = previous['workers'].get(unit, {})
        old, new = before.get('cpu_usec'), worker.get('cpu_usec')
        valid = (old is not None and new is not None and new >= old
                 and before.get('cgroup') == worker.get('cgroup'))
        value = dict(worker, cpu_cores=(new-old)/1e6/elapsed if valid else None)
        old_throttle, new_throttle = before.get('throttled_usec'), worker.get('throttled_usec')
        value['throttled_seconds'] = ((new_throttle-old_throttle)/1e6 if old_throttle is not None
                                      and new_throttle is not None and new_throttle >= old_throttle else None)
        result['workers'][unit] = value
        role = result['roles'].setdefault(worker['role'], dict(workers=0, measured_workers=0, cpu_cores=0,
                           memory_bytes=0, anon_bytes=0, shmem_bytes=0, throttled_seconds=0))
        role['workers'] += 1
        role['measured_workers'] += valid
        role['cpu_cores'] += value['cpu_cores'] or 0
        role['throttled_seconds'] += value['throttled_seconds'] or 0
        for key in ('memory_bytes', 'anon_bytes', 'shmem_bytes'):
            role[key] += value.get(key) or 0
    old, new = previous['host'], current['host']
    total = new['cpu_total'] - old['cpu_total']
    host = result['host'] = dict(new)
    host['cpu_cores'] = (new['cpu_busy']-old['cpu_busy'])/total*new['cpus'] if total > 0 else None
    host['iowait_cores'] = (new['cpu_iowait']-old['cpu_iowait'])/total*new['cpus'] if total > 0 else None
    host['pressure_pct'] = {k:max(0, v-old['pressure_usec'].get(k, v))/1e6/elapsed*100
                            for k,v in new['pressure_usec'].items()}
    for key in ('swap_in_pages', 'swap_out_pages'):
        host[key.replace('_pages', '_mib_per_second')] = max(0, new[key]-old[key])*os.sysconf('SC_PAGE_SIZE')/1024**2/elapsed
    return result


def distribution(values):
    """Time-weighted mean and nearest-rank p95 of interval measurements."""
    values = sorted((v, seconds) for v,seconds in values if v is not None and seconds > 0)
    if not values:
        return dict(mean=None, p95=None, maximum=None, minimum=None)
    elapsed = sum(s for _,s in values)
    running = 0
    p95 = values[-1][0]
    for value, seconds in values:
        running += seconds
        if running >= elapsed*.95:
            p95 = value
            break
    return dict(mean=round(sum(v*s for v,s in values)/elapsed, 3), p95=round(p95, 3),
                maximum=round(values[-1][0], 3), minimum=round(values[0][0], 3))


def summarize(samples):
    samples = list(samples)
    if not samples:
        return {}
    def dist(getter):
        return distribution([(getter(s), s['seconds']) for s in samples])
    roles = {}
    for name in ('implement', 'review', 'verify'):
        roles[name] = dict(cpu_cores=dist(lambda s:s['roles'].get(name, {}).get('cpu_cores')),
                           memory_gib=dist(lambda s:s['roles'].get(name, {}).get('memory_bytes', 0)/GIB),
                           anon_gib=dist(lambda s:s['roles'].get(name, {}).get('anon_bytes', 0)/GIB),
                           workers=samples[-1]['roles'].get(name, {}).get('workers', 0),
                           measured_workers=samples[-1]['roles'].get(name, {}).get('measured_workers', 0),
                           throttled_seconds=sum(s['roles'].get(name, {}).get('throttled_seconds', 0) for s in samples))
    def projection(s):
        role = s['roles'].get('review', {})
        if not role.get('workers') or role['measured_workers'] != role['workers'] or s['host']['cpu_cores'] is None:
            return None
        return s['host']['cpu_cores'] + 3*role['cpu_cores']/role['workers']
    def thirteen(s):
        value = s['host']['cpu_cores']
        if value is None:
            return None
        for role, count in [('implement',5),('review',7),('verify',1)]:
            measured=s['roles'].get(role,{})
            if not measured.get('workers') or measured['measured_workers'] != measured['workers']:
                return None
            value += (count-measured['workers'])*measured['cpu_cores']/measured['workers']
        return value
    return dict(at=samples[-1]['at'], sampled_seconds=round(sum(s['seconds'] for s in samples), 1), samples=len(samples),
                logical_cpus=samples[-1]['host']['cpus'], affinity_cpus=samples[-1]['host']['affinity_cpus'],
                roles=roles, host_cpu_cores=dist(lambda s:s['host']['cpu_cores']),
                worker_cpu_cores=dist(lambda s:sum(r['cpu_cores'] for r in s['roles'].values())),
                available_memory_gib=dist(lambda s:s['host']['memory_available_bytes']/GIB),
                iowait_cores=dist(lambda s:s['host']['iowait_cores']),
                pressure_pct={name:dist(lambda s:s['host']['pressure_pct'].get(name)) for name in samples[-1]['host']['pressure_pct']},
                swap_in_mib_per_second=dist(lambda s:s['host']['swap_in_mib_per_second']),
                swap_out_mib_per_second=dist(lambda s:s['host']['swap_out_mib_per_second']),
                projection_plus_three_reviewers=dict(host_cpu_cores=dist(projection),
                    basis='current host load plus three times the measured mean CPU per reviewer; excludes cold-cache bursts and future competing load'),
                projection_13_workers=dict(host_cpu_cores=dist(thirteen), implementers=5,reviewers=7,verifiers=1,
                    basis='scale each role to 5/7/1 using its observed CPU per worker, holding other host load fixed'))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, required=True)
    parser.add_argument('--unit-prefix', default='mtgish-unlimited')
    parser.add_argument('--interval', type=float, default=2)
    parser.add_argument('--window', type=float, default=600, help='rolling summary window in seconds')
    parser.add_argument('command', choices=['watch', 'report'])
    args = parser.parse_args()
    folder = args.root/'resources'
    if args.command == 'report':
        print((folder/'status.json').read_text())
        return
    if args.interval <= 0 or args.window <= 0:
        parser.error('interval and window must be positive')
    folder.mkdir(parents=True, exist_ok=True)
    groups = discover(args.root, args.unit_prefix)
    previous = snapshot(groups)
    samples = deque()
    refreshed = time.monotonic()
    while True:
        time.sleep(args.interval)
        if time.monotonic() - refreshed >= 30:
            groups = discover(args.root, args.unit_prefix)
            refreshed = time.monotonic()
        current = snapshot(groups)
        sample = interval(previous, current)
        previous = current
        samples.append(sample)
        elapsed = sum(s['seconds'] for s in samples)
        while len(samples) > 1 and elapsed - samples[0]['seconds'] >= args.window:
            elapsed -= samples.popleft()['seconds']
        date = current['at'][:10]
        with (folder/f'samples-{date}.jsonl').open('a') as file:
            file.write(json.dumps(sample)+'\n')
        review_lane.save(folder/'status.json', summarize(samples))


if __name__ == '__main__':
    main()
