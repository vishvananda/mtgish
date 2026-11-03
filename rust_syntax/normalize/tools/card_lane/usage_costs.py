"""Standard API-equivalent estimates from per-card token deltas, never a bill.

Input includes cached tokens. Prices are USD per million tokens. Terra's rates
were checked on 2026-11-03 against the official model page below. Estimates do
not include service-tier, long-context, cache-write or regional adjustments.
"""
import json
from pathlib import Path
import re

PRICING_SOURCE = "https://developers.openai.com/api/docs/models/gpt-6.1-terra"
RATES = {"gpt-6-luna": (.10, .01, .50), "gpt-5.6-luna": (.20, .02, 1.20),
         "gpt-6-sol": (2, .20, 10), "gpt-6.1-sol": (2, .10, 10),
         "gpt-6.1-terra": (1, .05, 5), "gpt-6-astra": (10, 1, 50)}


def estimate(usage, model):
    """None means unpriced; missing usage must never look like free work."""
    if not isinstance(usage, dict):
        return None
    if model not in RATES or any(not isinstance(usage.get(k), (int, float)) or usage[k] < 0
                                 for k in ("input", "cached", "output")):
        return None
    if usage['cached'] > usage['input']:
        return None
    ordinary, cached, output = RATES[model]
    return ((usage['input'] - usage['cached']) * ordinary + usage['cached'] * cached
            + usage['output'] * output) / 1e6


def pipeline_summary(db, cfg):
    """Read authoritative results once, with events retaining all review rounds.

    Implementation results take precedence over their copies in review sources
    and producer queues. Replayed review results are deduplicated by attempt,
    not session: a retained session can process many cards and multiple rounds.
    """
    cards, seen, implemented = {}, set(), set()

    def card(name):
        return cards.setdefault(name, dict(name=name, status='queued', stages={s: 0.0 for s in
                                           ('implement', 'review', 'verify')}, attempts=0,
                                           unpriced_attempts=0, implementation_recorded=False,
                                           review_recorded=False, batch_apportioned=False))

    def add(name, stage, result, model, identity):
        entry = card(name)
        attempts = result.get('attempts')
        if not isinstance(attempts, list) or not attempts:
            attempts = [result] if isinstance(result.get('usage'), dict) else []
        for index, attempt in enumerate(attempts):
            usage = attempt.get('usage')
            if not isinstance(usage, dict):
                continue
            # Copies made while resuming deterministic gates retain these exact
            # attempt records, including elapsed seconds. Different invocations
            # in the same session are still separate charges.
            key = (identity, index, json.dumps({k: attempt.get(k) for k in
                   ('session', 'seconds', 'usage')}, sort_keys=True))
            if key in seen:
                continue
            seen.add(key)
            cost = estimate(usage, result.get('model') or model)
            entry['attempts'] += 1
            if cost is None:
                entry['unpriced_attempts'] += 1
            else:
                entry['stages'][stage] += cost
            entry['implementation_recorded' if stage == 'implement' else 'review_recorded'] = True

    if db.execute("SELECT 1 FROM sqlite_master WHERE type='table' AND name='implementation_jobs'").fetchone():
        for row in db.execute('SELECT * FROM implementation_jobs ORDER BY enqueued,rowid'):
            card(row['name'])['status'] = row['status']
            if not row['result']:
                continue
            result = json.loads(row['result'])
            source = result.get('card') or {}
            if result.get('attempts') or source.get('usage'):
                add(row['name'], 'implement', dict(result, usage=source.get('usage')),
                    source.get('worker', '').partition(':')[2] or cfg.get('producer_model'), row['id'])
                implemented.add((row['name'], source.get('commit')))

    jobs = {r['id']: dict(r) for r in db.execute('SELECT * FROM jobs ORDER BY enqueued,rowid')}
    for job in jobs.values():
        entry = card(job['name'])
        entry['status'] = job['status']
        source = json.loads(job['source'])
        key = (job['name'], source.get('commit'))
        if key not in implemented and source.get('usage'):
            add(job['name'], 'implement', source,
                source.get('worker', '').partition(':')[2] or cfg.get('producer_model'), key)
            implemented.add(key)
            entry['batch_apportioned'] |= str(source.get('producer', '')).startswith('w')

    for event in db.execute("SELECT * FROM events WHERE kind IN ('passed','queued','blocked','infrastructure') ORDER BY id"):
        job = jobs.get(event['job'])
        if not job:
            continue
        result = json.loads(event['detail'])
        stage = result.get('stage')
        if not stage:
            match = re.match(r'^(review|confirm)(?:-|$)', result.get('branch', '').rsplit('/', 1)[-1])
            stage = match[1] if match else job['stage']
        stage = 'verify' if stage == 'confirm' else 'review'
        add(job['name'], stage, result, cfg.get('review_model'), (job['id'], stage))

    # Legacy stopped implementations never entered the review queue. Their
    # producer queue contains the only available (batch-apportioned) estimate.
    queue_path = cfg.get('implementation_queue')
    review_names = {j['name'] for j in jobs.values()}
    if queue_path:
        try:
            queue = json.loads(Path(queue_path).read_text())
        except (OSError, ValueError):
            queue = {}
        for source in queue.get('cards', []):
            entry = card(source['name'])
            if not entry['implementation_recorded'] and source.get('usage'):
                add(source['name'], 'implement', source,
                    source.get('worker', '').partition(':')[2] or cfg.get('producer_model'),
                    (source['name'], source.get('commit')))
                entry['batch_apportioned'] = True
                if source['name'] not in review_names:
                    entry['status'] = source.get('status', 'queued')

    for entry in cards.values():
        entry['usd_api_equivalent'] = (round(sum(entry['stages'].values()), 6)
                                       if entry['attempts'] > entry['unpriced_attempts'] else None)
        entry['complete'] = (entry['status'] == 'passed' and entry['implementation_recorded']
                             and entry['review_recorded'] and not entry['unpriced_attempts'])
        entry['stages'] = {k: round(v, 6) for k, v in entry['stages'].items()}
    completed = [c for c in cards.values() if c['complete']]
    completed_cost = sum(c['usd_api_equivalent'] for c in completed)
    return dict(basis='standard API-equivalent estimate', pricing_source=PRICING_SOURCE,
                excludes='in-flight usage and service-tier, long-context, cache-write and regional adjustments',
                total_usd=round(sum(c['usd_api_equivalent'] or 0 for c in cards.values()), 6),
                completed_cards=len(completed), completed_usd=round(completed_cost, 6),
                average_usd_per_completed_card=round(completed_cost / len(completed), 6) if completed else None,
                unpriced_attempts=sum(c['unpriced_attempts'] for c in cards.values()),
                cards=cards)
