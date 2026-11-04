#!/usr/bin/env python3
"""Original proof shards: plan misses, execute ONE, join authentic local passes.

No declaration slicing, caller-declared pass import or shared-cache trust.
Cold execution needs the real proof-shard guard and <=3.7GB no-swap cgroup.
The full nightly proof remains separate; this command has no force-fresh mode.
"""
import argparse
import importlib.util
import json
import math
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
SCHEMA = 1
KIND = 'backplane-original-proof-cache-plan'
CAP = 3_700_000_000


def module(name):
    spec = importlib.util.spec_from_file_location(name.replace('-', '_'), ROOT/'scripts'/name)
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


cc = module('content-cache.py')


def inventory(root, folder):
    root, folder = Path(root).resolve(), Path(folder).resolve()
    folder.relative_to(root)
    cc.reset_tool_identity()
    units, manifest = cc.shard_units(root, folder)
    return units, manifest


def make_plan(root, folder):
    root = Path(root).resolve()
    folder = (root/Path(folder)).resolve()
    units, manifest = inventory(root, folder)
    return {'schema': SCHEMA, 'kind': KIND, 'partition': folder.relative_to(root).as_posix(),
            'manifest_sha256': cc.sha(module('proof-shards.py').encoded(manifest)),
            'keys': {name: key for name, (key, _) in units.items()}}


def validate(root, planned):
    if (not isinstance(planned, dict) or set(planned) != {'schema', 'kind', 'partition', 'manifest_sha256', 'keys'}
        or type(planned['schema']) is not int or planned['schema'] != SCHEMA or planned['kind'] != KIND
        or not isinstance(planned['partition'], str) or not isinstance(planned['keys'], dict)):
        raise cc.CacheError('invalid/incomplete proof-cache plan')
    root = Path(root).resolve()
    folder = (root/planned['partition']).resolve()
    folder.relative_to(root)
    units, manifest = inventory(root, folder)
    identity = cc.sha(module('proof-shards.py').encoded(manifest))
    if planned['manifest_sha256'] != identity or planned['keys'] != {n: k for n, (k, _) in units.items()}:
        raise cc.CacheError('proof inventory, inputs or tools changed; regenerate the partition and plan')
    return folder, units, manifest


def resource_state(guard):
    directory = guard.own_cgroup()
    if not guard.verify_shard_cgroup(directory):
        raise cc.CacheError('proof cache worker needs actual <=3.7GB/no-swap cgroup')
    events = dict(line.split() for line in (directory/'memory.events').read_text().splitlines())
    state = {'cgroup': str(directory), 'memory_max_bytes': int((directory/'memory.max').read_text()),
             'memory_peak_bytes': int((directory/'memory.peak').read_text()),
             'swap_max_bytes': int((directory/'memory.swap.max').read_text()),
             'events': {name: int(value) for name, value in events.items()}}
    if (not 0 < state['memory_peak_bytes'] <= CAP or state['events'].get('oom', -1) != 0
            or state['events'].get('oom_kill', -1) != 0):
        raise cc.CacheError('proof cache worker exceeded memory/OOM contract')
    return state


def receipt_valid(receipt, shard, identity, ps, maximum):
    positive = ('seconds', 'peak_rss_kib', 'aggregate_peak_bytes')
    return (isinstance(receipt, dict) and receipt.get('shard') == shard
        and receipt.get('manifest_sha256') == identity and receipt.get('state') == 'passed'
        and receipt.get('inputs_unchanged') is True and receipt.get('actual_bend_flock_held') is True
        and type(receipt.get('memory_max_bytes')) is int and receipt['memory_max_bytes'] == maximum
        and all(isinstance(receipt.get(n), (int, float)) and not isinstance(receipt[n], bool)
                and math.isfinite(receipt[n]) and receipt[n] > 0 for n in positive)
        and receipt['peak_rss_kib']*1024 < 3*1024**3 and receipt['aggregate_peak_bytes'] <= maximum <= CAP
        and ps.no_swap_activity(receipt.get('swap_before', {}), receipt.get('swap_after', {}))
        and ps.verdict(receipt.get('exit'), receipt.get('output', '')))


def run_one(root, planned, shard, result, cache=None):
    guard = module('heavy-guard.py')
    if guard.inherited_lock(guard.lock_path()) is None:
        raise cc.CacheError('proof cache worker needs actual inherited native bend flock')
    before = resource_state(guard)
    folder, units, manifest = validate(root, planned)
    if shard not in units:
        raise cc.CacheError('unknown proof shard')
    key, body = units[shard]
    if cc.lookup(key, cache):
        _, current, current_manifest = validate(root, planned)
        if current != units or current_manifest != manifest:
            raise cc.CacheError('proof snapshot changed during cache lookup')
        resource_state(guard)
        return {'state': 'cached', 'shard': shard, 'key': key, 'proof_executions': 0}, 0
    if not guard.admit('proof-shard'):
        raise cc.CacheError('proof cache worker memory admission refused')
    ps = module('proof-shards.py')
    identity = cc.sha(ps.encoded(manifest))
    result = (Path(root).resolve()/Path(result)).resolve()
    # The original worker owns the execution and its attempt marker. It refuses
    # duplicate attempts; the caller cannot import a pass file into this route.
    status = ps.execute(Path(root).resolve(), folder, shard, identity, Path(result))
    after = resource_state(guard)
    if before['cgroup'] != after['cgroup'] or before['memory_max_bytes'] != after['memory_max_bytes']:
        raise cc.CacheError('proof worker cgroup changed during execution')
    _, current, _ = validate(root, planned)
    if current[shard] != (key, body):
        raise cc.CacheError('proof inputs changed during execution')
    receipt = json.loads(Path(result).read_text())
    accepted = (status == 0 and receipt.get('executor_pid') == os.getpid()
                and receipt_valid(receipt, shard, identity, ps, after['memory_max_bytes']))
    if not accepted:
        return {'state': 'failed', 'shard': shard, 'key': key, 'proof_executions': 1,
                'resources': after, 'receipt': str(result)}, 1
    cc._record(key, body, cache)
    return {'state': 'passed', 'shard': shard, 'key': key, 'proof_executions': 1,
            'resources': after, 'receipt': str(result)}, 0


def join(root, planned, cache=None):
    _, units, manifest = validate(root, planned)
    hit, miss = cc.plan(units, True, cache)
    # Fence the lookups with snapshot validation. Do not perform a new lookup
    # after the final validation: it would fall outside this source/tool fence.
    _, current, current_manifest = validate(root, planned)
    if current != units or current_manifest != manifest:
        raise cc.CacheError('proof snapshot changed during cache lookup')
    return {'state': 'incomplete' if miss else 'passed', 'kind': 'original-proof-cache-join',
            'manifest_sha256': planned['manifest_sha256'], 'law_count': len(manifest['laws']),
            'shards': len(current), 'hit': hit, 'miss': miss,
            'trust': 'same-user local MAC; not shared signatures or a fresh nightly proof'}, 1 if miss else 0


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=('plan', 'run', 'join'))
    parser.add_argument('--root', type=Path, default=ROOT)
    parser.add_argument('--partition', type=Path)
    parser.add_argument('--plan', type=Path, required=True)
    parser.add_argument('--cache', type=Path)
    parser.add_argument('--shard')
    parser.add_argument('--result', type=Path)
    args = parser.parse_args(argv)
    if args.action == 'plan':
        if args.partition is None:
            raise cc.CacheError('plan needs --partition')
        planned = make_plan(args.root, args.partition)
        cc._write(args.plan, planned)
        _, units, manifest = validate(args.root, planned)
        hit, miss = cc.plan(units, True, args.cache)
        result, status = {'law_count': len(manifest['laws']), 'shards': len(units), 'hit': hit, 'miss': miss}, 0
    else:
        planned = json.loads(args.plan.read_text())
        if args.action == 'run':
            if args.shard is None or args.result is None:
                raise cc.CacheError('run needs --shard and --result')
            result, status = run_one(args.root, planned, args.shard, args.result, args.cache)
        else:
            result, status = join(args.root, planned, args.cache)
    print(json.dumps(result, sort_keys=True))
    return status


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (cc.CacheError, OSError, ValueError, subprocess.CalledProcessError) as error:
        print('proof-cache: '+str(error), file=sys.stderr)
        raise SystemExit(2)
