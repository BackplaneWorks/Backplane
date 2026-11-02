#!/usr/bin/env python3
"""Capture Actions job/step timings, without confusing dependencies with queue time.

Requires authenticated gh. Raw API replies are retained for reproducibility.
Jobs API created_at -> started_at is queue time; run creation -> job creation
is dependency/scheduling time. Missing timestamps remain unknown, never zero.
"""
import argparse
import concurrent.futures
import datetime as dt
import json
from pathlib import Path
import re
import subprocess


SETUP = re.compile(r'^(Set up job|Set up runner|Initialize containers|Prepare|Checkout|Run actions/checkout|Run actions/setup-|Run oven-sh/setup-bun|Run \./\.github/actions/setup-bend|Run actions/cache|Cache|Install|Setup|Set up (?:Bun|Bend|Node|Python))', re.I)
CLEANUP = re.compile(r'^(Post |Complete job|Stop containers)', re.I)
TRANSFER = re.compile(r'^Run actions/(?:upload|download)-artifact', re.I)


def seconds(a, b):
    if not a or not b:
        return None
    return max(0, (dt.datetime.fromisoformat(b.replace('Z', '+00:00')) -
                   dt.datetime.fromisoformat(a.replace('Z', '+00:00'))).total_seconds())


def job_row(run, job):
    steps = []
    for s in job.get('steps', []):
        if s.get('status') != 'completed' or s.get('conclusion') == 'skipped':
            continue
        kind = ('setup' if SETUP.search(s['name']) else 'cleanup' if CLEANUP.search(s['name'])
                else 'transfer' if TRANSFER.search(s['name']) else 'running')
        steps.append({'name': s['name'], 'kind': kind,
                      'seconds': seconds(s.get('started_at'), s.get('completed_at')),
                      'conclusion': s.get('conclusion')})
    return {'run': run['id'], 'attempt': job.get('run_attempt', run.get('run_attempt')),
            'workflow': run['name'], 'event': run['event'], 'sha': run['head_sha'],
            'job': job['name'], 'id': job['id'], 'conclusion': job.get('conclusion'),
            'runner': job.get('runner_name'), 'labels': job.get('labels', []),
            'dependency_seconds': seconds(run['created_at'], job.get('created_at')),
            'queue_seconds': seconds(job.get('created_at'), job.get('started_at')),
            'wall_seconds': seconds(job.get('started_at'), job.get('completed_at')),
            **{kind + '_seconds': sum(s['seconds'] or 0 for s in steps if s['kind'] == kind)
               for kind in ['setup', 'running', 'cleanup', 'transfer']}, 'steps': steps}


def api(route):
    return json.loads(subprocess.check_output(['gh', 'api', route], text=True))


def capture(repo, limit, out, workers):
    raw = out / 'raw'
    raw.mkdir(parents=True, exist_ok=True)
    runs = []
    for page in range(1, (limit + 99) // 100 + 1):
        data = api(f'repos/{repo}/actions/runs?per_page=100&page={page}')
        (raw / f'runs-{page}.json').write_text(json.dumps(data, indent=2) + '\n')
        runs.extend(data['workflow_runs'])
    runs = runs[:limit]

    def fetch(run):
        jobs, page = [], 1
        # filter=all preserves reruns rather than silently losing failed attempts.
        while True:
            data = api(f'repos/{repo}/actions/runs/{run["id"]}/jobs?filter=all&per_page=100&page={page}')
            (raw / f'{run["id"]}-jobs-{page}.json').write_text(json.dumps(data, indent=2) + '\n')
            jobs.extend(data['jobs'])
            if len(jobs) >= data['total_count'] or not data['jobs']:
                break
            page += 1
        return [job_row(run, job) for job in jobs]

    with concurrent.futures.ThreadPoolExecutor(max_workers=workers) as pool:
        rows = [row for group in pool.map(fetch, runs) for row in group]
    report = {'repo': repo, 'captured_at': dt.datetime.now(dt.timezone.utc).isoformat(),
              'runs': len(runs), 'jobs': rows}
    (out / 'actions.json').write_text(json.dumps(report, indent=2) + '\n')
    print(f'{len(runs)} runs, {len(rows)} jobs -> {out}/actions.json', flush=True)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--repo', default='BackplaneWorks/Backplane')
    p.add_argument('--limit', type=int, default=150)
    p.add_argument('--out', type=Path, default=Path('build/ci-speed/audit'))
    p.add_argument('--workers', type=int, default=8)
    a = p.parse_args()
    if not 1 <= a.limit <= 1000 or not 1 <= a.workers <= 16:
        p.error('limit must be 1..1000 and workers 1..16')
    capture(a.repo, a.limit, a.out, a.workers)


if __name__ == '__main__':
    main()
