#!/usr/bin/env python3
"""Run: uv run --no-project python test/tools/allowanceio_test.py

No real CLI is used. Optional ALLOWANCEIO_READER_JSON='["bun","/abs/reader.js"]'
executes test/native/allowanceio_read.bend compiled to JS instead of extracting
its embedded script, exercising the real Bend/P.Proc.run boundary as well.
"""
import concurrent.futures
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import sys
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / 'src/server/allowanceio.bend'
FAKE = ROOT / 'test/fixtures/allowanceio/codex.py'
PRIVATE = 'private-canary-email@example.invalid/account-token-sk-never-output'


def embedded():
    source = SOURCE.read_text()
    match = re.search(r'String\.join\((\[.*?\]), "\\n"\)', source, re.S)
    assert match, 'Missing embedded script'
    return '\n'.join(json.loads(match[1]))


def supervisor():
    match = re.search(r'def AllowanceIO\.supervisor\(\) -> String:\n  (".*")', SOURCE.read_text())
    assert match, 'Missing supervisor'
    return json.loads(match[1])


class AllowanceIO(unittest.TestCase):
    def probe(self, case, missing=False):
        with tempfile.TemporaryDirectory(prefix='allowance mock ') as tmp:
            tmp = Path(tmp)
            bindir = tmp / 'bin'
            bindir.mkdir()
            log = tmp / 'wire.jsonl'
            if not missing:
                fake = bindir / 'codex'
                fake.write_text('#!/bin/sh\nexec ' + shlex.quote(sys.executable) +
                                ' -I ' + shlex.quote(str(FAKE)) + ' "$@"\n')
                fake.chmod(0o755)
            # Fail closed: only the fake CLI and Python/shell are on PATH.
            for name, target in [('python3', sys.executable), ('sh', '/bin/sh'),
                                 ('env', '/usr/bin/env'), ('perl', '/usr/bin/perl')]:
                (bindir / name).symlink_to(target)
            env = {**os.environ, 'PATH': str(bindir), 'ALLOWANCE_FIXTURE_CASE': case,
                   'ALLOWANCE_FIXTURE_LOG': str(log), 'HOME': str(tmp),
                   'CODEX_HOME': str(tmp / 'codex-home'),
                   'BACKPLANE_HOME': str(tmp / 'backplane-home')}
            env.pop('BACKPLANE_COPY', None)
            command = json.loads(env['ALLOWANCEIO_READER_JSON']) if env.get(
                'ALLOWANCEIO_READER_JSON') else ['/usr/bin/perl', '-e', supervisor(), '--', embedded()]
            command += [str(tmp / 'allowance-codex.json')]
            started = time.monotonic()
            run = subprocess.run(command, cwd=ROOT, env=env, capture_output=True,
                                 text=True, timeout=12)
            elapsed = time.monotonic() - started
            self.assertEqual(run.returncode, 0, run.stderr)
            self.assertEqual(run.stderr, '')
            self.assertNotIn(PRIVATE, run.stdout)
            self.assertLessEqual(run.stdout.count('\n'), 1)
            self.assertNotIn('used_percent', run.stdout)
            data = json.loads(run.stdout)
            self.assertEqual(set(data), {'provider', 'at', 'attempted_at', 'state', 'auth', 'windows', 'source', 'phase'})
            self.assertEqual(data['provider'], 'codex')
            self.assertEqual(data['source'], 'codex-app-server')
            self.assertEqual(data['phase'], 'complete')
            self.assertIs(type(data['at']), int)
            self.assertTrue(data['at'] == 0 or abs(time.time() - data['at']) <= 2)
            self.assertLessEqual(abs(time.time() - data['attempted_at']), 12)
            self.assertIn(data['state'], ('ok', 'error', 'unavailable'))
            self.assertIn(data['auth'], ('chatgpt', 'apikey', 'unknown'))
            self.assertLessEqual(len(data['windows']), 32)
            for row in data['windows']:
                self.assertEqual(set(row), {'name', 'used_permille', 'resets_at',
                                           'duration_minutes', 'scope', 'shared'})
                self.assertIs(type(row['used_permille']), int)
                self.assertTrue(0 <= row['used_permille'] <= 1000)
                self.assertLessEqual(len(row['name']), 138)
                self.assertTrue(row['shared'])
            wire = [json.loads(line) for line in log.read_text().splitlines()] if log.exists() else []
            calls = [row for row in wire if 'method' in row]
            self.assertEqual([row['method'] for row in calls],
                             ['initialize', 'initialized', 'account/read',
                              'account/rateLimits/read'][:len(calls)])
            self.assertFalse(any('unexpected' in row for row in wire))
            if len(calls) == 4:
                self.assertEqual(calls[2]['params'], {'refreshToken': False})
            for row in wire:
                if 'pid' in row or 'child_pid' in row:
                    self.assert_stopped(row.get('pid', row.get('child_pid')))
            return data, calls, elapsed

    def assert_stopped(self, pid):
        try:
            os.kill(pid, 0)
        except ProcessLookupError:
            return
        # A killed descendant may briefly be a zombie pending init's reaper.
        proc = subprocess.run(['/bin/ps', '-o', 'stat=', '-p', str(pid)],
                              capture_output=True, text=True)
        self.assertTrue(not proc.stdout.strip() or proc.stdout.strip().startswith('Z'),
                        f'Captured child {pid} still running')

    def test_handshake_multiple_buckets_and_sanitization(self):
        data, calls, _ = self.probe('multi')
        self.assertEqual(len(calls), 4)
        self.assertEqual(data['state'], 'ok')
        self.assertEqual(data['auth'], 'chatgpt')
        self.assertEqual(data['windows'], [
            dict(name='codex/primary', used_permille=124, resets_at=1900000000,
                 duration_minutes=300, scope='codex', shared=True),
            dict(name='codex/secondary', used_permille=1000, resets_at=None,
                 duration_minutes=None, scope='codex', shared=True),
            dict(name='Other pool/primary', used_permille=0, resets_at=0,
                 duration_minutes=0, scope='other-id', shared=True),
            dict(name='fallback-id/primary', used_permille=999, resets_at=None,
                 duration_minutes=10080, scope='fallback-id', shared=True)])

    def test_legacy_and_missing_bucket_id(self):
        for case, scope, name in [('legacy', 'codex', 'codex/primary'),
                                  ('legacy_no_id', None, 'unknown/primary')]:
            with self.subTest(case=case):
                data, _, _ = self.probe(case)
                self.assertEqual(data['state'], 'ok')
                self.assertEqual(len(data['windows']), 1)
                self.assertEqual(data['windows'][0]['scope'], scope)
                self.assertEqual(data['windows'][0]['name'], name)
                self.assertEqual(data['windows'][0]['used_permille'], 777)

    def test_null_and_empty_multi_are_unavailable(self):
        for case in ('null', 'null_windows', 'empty_multi'):
            with self.subTest(case=case):
                data, _, _ = self.probe(case)
                self.assertEqual(data['state'], 'unavailable')
                self.assertEqual(data['windows'], [])

    def test_auth_and_rpc_states(self):
        for case, state, auth in [('apikey', 'unavailable', 'apikey'),
                                  ('signed_out', 'unavailable', 'unknown'),
                                  ('rpc_error', 'error', 'chatgpt'),
                                  ('unsupported', 'unavailable', 'chatgpt'),
                                  ('account_error', 'error', 'unknown'),
                                  ('init_error', 'unavailable', 'unknown')]:
            with self.subTest(case=case):
                data, _, _ = self.probe(case)
                self.assertEqual((data['state'], data['auth']), (state, auth))
                self.assertEqual(data['windows'], [])

    def test_invalid_numbers_and_structures_reject_without_clamping(self):
        cases = ['invalid_used_' + suffix for suffix in
                 ('negative', 'large', 'bool', 'null', 'string', 'nan', 'inf')]
        cases += ['invalid_reported_reset', 'invalid_reported_duration',
                  'wrong_bucket', 'wrong_map', 'wrong_window']
        for case in cases:
            with self.subTest(case=case):
                data, _, _ = self.probe(case)
                self.assertEqual(data['state'], 'error')
                self.assertEqual(data['windows'], [])

    def test_counts_labels_and_protocol_are_bounded(self):
        for case in ('too_many', 'long_label', 'long_id', 'long_key', 'control_label',
                     'malformed', 'flood', 'host_request', 'notification_flood'):
            with self.subTest(case=case):
                data, _, _ = self.probe(case)
                self.assertEqual(data['state'], 'error')
                self.assertEqual(data['windows'], [])
        data, _, _ = self.probe('max_buckets')
        self.assertEqual(data['state'], 'ok')
        self.assertEqual(len(data['windows']), 32)
        data, _, _ = self.probe('max_label')
        self.assertEqual(data['state'], 'ok')
        self.assertEqual(data['windows'][0]['name'], 'x' * 128 + '/primary')

    def test_ceiling_keeps_all_reported_decimal_precision(self):
        data, _, _ = self.probe('decimal_precision')
        self.assertEqual(data['state'], 'ok')
        self.assertEqual(data['windows'][0]['used_permille'], 11)

    def test_missing_cli_is_unavailable(self):
        data, calls, _ = self.probe('unused', missing=True)
        self.assertEqual(data['state'], 'unavailable')
        self.assertEqual(data['windows'], [])
        self.assertEqual(calls, [])

    def test_outer_supervisor_bounds_python_startup(self):
        with tempfile.TemporaryDirectory(prefix='allowance startup ') as tmp:
            tmp = Path(tmp)
            pidfile = tmp / 'pid'
            python = tmp / 'python3'
            python.write_text('#!/bin/sh\nprintf %s "$$" > ' + shlex.quote(str(pidfile)) +
                              '\nexec /bin/sleep 30\n')
            python.chmod(0o755)
            env = {**os.environ, 'PATH': str(tmp)}
            if env.get('ALLOWANCEIO_READER_JSON'):
                for name, target in [('sh', '/bin/sh'), ('env', '/usr/bin/env'),
                                     ('perl', '/usr/bin/perl')]:
                    (tmp / name).symlink_to(target)
                command = json.loads(env['ALLOWANCEIO_READER_JSON'])
            else:
                command = ['/usr/bin/perl', '-e', supervisor(), '--', embedded()]
            command += [str(tmp / 'allowance-codex.json')]
            start = time.monotonic()
            run = subprocess.run(command, cwd=ROOT, env=env, capture_output=True,
                                 text=True, timeout=12)
            elapsed = time.monotonic() - start
            self.assertGreater(elapsed, 10)
            self.assertLess(elapsed, 12)
            self.assertEqual(run.stderr, '')
            if env.get('ALLOWANCEIO_READER_JSON'):
                self.assertEqual(run.returncode, 0)
                data = json.loads(run.stdout)
                self.assertEqual(data['state'], 'error')
                self.assertEqual(data['windows'], [])
            else:
                self.assertEqual(run.returncode, 124)
                self.assertEqual(run.stdout, '')
            self.assert_stopped(int(pidfile.read_text()))

    def test_single_total_deadline_and_cleanup_at_each_protocol_stage(self):
        cases = ('timeout_init', 'timeout_account', 'timeout_limits',
                 'timeout_total', 'descendant_timeout')
        # Independent fake sessions: cover each blocking stage in one deadline.
        with concurrent.futures.ThreadPoolExecutor(max_workers=len(cases)) as pool:
            results = list(pool.map(self.probe, cases))
        for data, _, elapsed in results:
            self.assertEqual(data['state'], 'error')
            self.assertEqual(data['windows'], [])
            self.assertGreater(elapsed, 9)
            self.assertLess(elapsed, 12)

class CacheSession:
    """Every CLI, cache, home and lock in these tests is a temporary fixture."""
    def __init__(self, folder):
        self.root = Path(folder)
        self.cache = self.root / "cache ' $() name.json"
        self.lock = Path(str(self.cache) + '.lock')
        self.log = self.root / 'wire.jsonl'
        bindir = self.root / 'bin'
        bindir.mkdir()
        fake = bindir / 'codex'
        fake.write_text('#!/bin/sh\nexec ' + shlex.quote(sys.executable) + ' -I ' +
                        shlex.quote(str(FAKE)) + ' "$@"\n')
        fake.chmod(0o755)
        for name, target in [('python3', sys.executable), ('sh', '/bin/sh'),
                             ('env', '/usr/bin/env'), ('perl', '/usr/bin/perl')]:
            (bindir / name).symlink_to(target)
        self.env = {**os.environ, 'PATH': str(bindir),
                    'ALLOWANCE_FIXTURE_LOG': str(self.log), 'HOME': str(self.root),
                    'CODEX_HOME': str(self.root / 'codex-home'),
                    'BACKPLANE_HOME': str(self.root / 'backplane-home')}
        self.env.pop('BACKPLANE_COPY', None)

    def run(self, case='multi'):
        env = {**self.env, 'ALLOWANCE_FIXTURE_CASE': case}
        command = json.loads(env['ALLOWANCEIO_READER_JSON']) if env.get(
            'ALLOWANCEIO_READER_JSON') else ['/usr/bin/perl', '-e', supervisor(), '--', embedded()]
        run = subprocess.run(command + [str(self.cache)], cwd=ROOT, env=env,
                             capture_output=True, text=True, timeout=12)
        assert run.returncode == 0 and not run.stderr, run.stderr
        assert PRIVATE not in run.stdout and 'private-workspace-' not in run.stdout
        data = json.loads(run.stdout)
        for path in (self.cache, self.lock):
            if path.exists() and not path.is_symlink():
                text = path.read_text()
                assert PRIVATE not in text and 'private-workspace-' not in text
        return data

    def wire(self):
        rows = [json.loads(line) for line in self.log.read_text().splitlines()] if self.log.exists() else []
        return [row for row in rows if 'method' in row]

    def age(self, seconds=301):
        data = json.loads(self.cache.read_text())
        data['attempted_at'] = int(time.time()) - seconds
        # at remains the actual original sample time; changing an attempt
        # never freshens a reading. Bind the fixture's two files consistently.
        raw = (json.dumps(data, separators=(',', ':'), ensure_ascii=False) + '\n').encode()
        self.cache.write_bytes(raw)
        meta = json.loads(self.lock.read_text())
        meta['cache_sha256'] = hashlib.sha256(raw).hexdigest()
        self.lock.write_text(json.dumps(meta))
        return data


class AllowanceCache(unittest.TestCase):
    def test_private_atomic_cache_and_ttl_no_force_bypass(self):
        with tempfile.TemporaryDirectory(prefix='allowance cache ') as tmp:
            session = CacheSession(tmp)
            first = session.run()
            self.assertEqual(first['state'], 'ok')
            self.assertEqual(first, json.loads(session.cache.read_text()))
            self.assertEqual(session.cache.stat().st_mode & 0o777, 0o600)
            self.assertEqual(session.lock.stat().st_mode & 0o777, 0o600)
            inode = session.lock.stat().st_ino
            second = session.run('account_switch')
            self.assertEqual(first, second)
            self.assertEqual(len(session.wire()), 4)
            cached = session.age(299)
            self.assertEqual(session.run('account_switch'), cached)
            self.assertEqual(len(session.wire()), 4)
            session.age(300)
            refreshed = session.run('account_switch')
            self.assertEqual(refreshed['state'], 'ok')
            self.assertEqual(len(session.wire()), 8)
            self.assertEqual(session.lock.stat().st_ino, inode)
            self.assertEqual(list(Path(tmp).glob('.*.tmp')), [])

    def test_unicode_maximum_snapshot_is_cacheable_without_extra_probe(self):
        with tempfile.TemporaryDirectory(prefix='allowance unicode ') as tmp:
            session = CacheSession(tmp)
            good = session.run('unicode_max')
            self.assertEqual(good['state'], 'ok')
            self.assertEqual(len(good['windows']), 32)
            self.assertGreater(session.cache.stat().st_size, 32768)
            self.assertLess(session.cache.stat().st_size, 131072)
            self.assertEqual(session.run('unicode_max'), good)
            self.assertEqual(len(session.wire()), 4)

    def test_unavailable_attempt_is_cached_even_after_cli_returns(self):
        with tempfile.TemporaryDirectory(prefix='allowance unavailable ') as tmp:
            session = CacheSession(tmp)
            fake = Path(tmp) / 'bin/codex'
            absent = fake.with_name('codex-disabled')
            fake.rename(absent)
            unavailable = session.run()
            self.assertEqual(unavailable['state'], 'unavailable')
            self.assertEqual(unavailable['at'], 0)
            self.assertGreater(unavailable['attempted_at'], 0)
            absent.rename(fake)
            self.assertEqual(session.run(), unavailable)
            self.assertEqual(session.wire(), [])
            session.age()
            self.assertEqual(session.run()['state'], 'ok')
            self.assertEqual(len(session.wire()), 4)

    def test_concurrent_details_and_timer_deduplicate_one_probe(self):
        with tempfile.TemporaryDirectory(prefix='allowance concurrent ') as tmp:
            session = CacheSession(tmp)
            with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
                answers = list(pool.map(lambda _: session.run('slow_multi'), range(8)))
            self.assertEqual(len(session.wire()), 4)
            self.assertTrue(any(data['state'] == 'ok' for data in answers))
            final = session.run()
            self.assertEqual(final['state'], 'ok')
            self.assertEqual(len(session.wire()), 4)

    def test_inflight_returns_cached_even_when_expired(self):
        with tempfile.TemporaryDirectory(prefix='allowance locked ') as tmp:
            session = CacheSession(tmp)
            session.run()
            cached = session.age(1000)
            with session.lock.open('r+') as lock:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                start = time.monotonic()
                self.assertEqual(session.run('account_switch'), cached)
                self.assertLess(time.monotonic() - start, 1)
            self.assertEqual(len(session.wire()), 4)

    def test_pending_cache_is_explicit_and_does_not_issue_another_read(self):
        with tempfile.TemporaryDirectory(prefix='allowance pending ') as tmp:
            session = CacheSession(tmp)
            good = session.run()
            self.assertEqual(good['phase'], 'complete')
            pending = {**good, 'phase': 'pending', 'state': 'error', 'windows': []}
            session.cache.write_text(json.dumps(pending))
            observed = session.run()
            self.assertEqual(observed['phase'], 'pending')
            self.assertEqual(observed['windows'], [])
            self.assertEqual(len(session.wire()), 4)

    def test_same_account_error_preserves_sample_at_and_windows(self):
        with tempfile.TemporaryDirectory(prefix='allowance same account ') as tmp:
            session = CacheSession(tmp)
            good = session.run()
            session.age()
            failed = session.run('same_account_invalid')
            self.assertEqual(failed['state'], 'error')
            self.assertEqual(failed['auth'], 'chatgpt')
            self.assertEqual(failed['windows'], good['windows'])
            self.assertEqual(failed['at'], good['at'])
            self.assertGreater(failed['attempted_at'], 0)
            self.assertEqual(session.run(), failed)
            self.assertEqual(len(session.wire()), 8)

    def test_account_switch_or_missing_identity_cannot_retain_old_windows(self):
        for case in ('other_account_invalid', 'no_identity_invalid', 'rpc_error', 'account_error'):
            with self.subTest(case=case), tempfile.TemporaryDirectory(prefix='allowance identity ') as tmp:
                session = CacheSession(tmp)
                good = session.run()
                session.age()
                failed = session.run(case)
                self.assertEqual(failed['state'], 'error')
                self.assertEqual(failed['windows'], [])
                self.assertEqual(failed['at'], 0 if case == 'other_account_invalid' else good['at'])
                self.assertEqual(session.run(), failed)
                self.assertEqual(len(session.wire()), 8)

    def test_no_auth_and_api_key_success_clear_previous_account(self):
        for case, auth in [('signed_out', 'unknown'), ('apikey', 'apikey'), ('null', 'chatgpt')]:
            with self.subTest(case=case), tempfile.TemporaryDirectory(prefix='allowance auth ') as tmp:
                session = CacheSession(tmp)
                session.run()
                session.age()
                current = session.run(case)
                self.assertEqual(current['state'], 'unavailable')
                self.assertEqual(current['auth'], auth)
                self.assertEqual(current['windows'], [])
                self.assertEqual(current, json.loads(session.cache.read_text()))
                self.assertEqual(session.run(), current)

    def test_identity_metadata_must_match_exact_snapshot(self):
        with tempfile.TemporaryDirectory(prefix='allowance interrupted save ') as tmp:
            session = CacheSession(tmp)
            session.run()
            session.age()
            meta = json.loads(session.lock.read_text())
            meta['cache_sha256'] = '0' * 64
            session.lock.write_text(json.dumps(meta))
            failed = session.run('same_account_invalid')
            self.assertEqual(failed['state'], 'error')
            self.assertEqual(failed['windows'], [])

    def test_cache_hit_is_resanitized_and_corruption_is_replaced(self):
        with tempfile.TemporaryDirectory(prefix='allowance dirty cache ') as tmp:
            session = CacheSession(tmp)
            good = session.run()
            dirty = {**good, 'email': PRIVATE, 'accountId': PRIVATE}
            dirty['windows'] = [{**row, 'token': PRIVATE} for row in good['windows']]
            session.cache.write_text(json.dumps(dirty))
            # Output is reconstructed from allowed fields, never echoed raw.
            command = json.loads(session.env['ALLOWANCEIO_READER_JSON']) if session.env.get(
                'ALLOWANCEIO_READER_JSON') else ['/usr/bin/perl', '-e', supervisor(), '--', embedded()]
            run = subprocess.run(command + [str(session.cache)], cwd=ROOT,
                                 env={**session.env, 'ALLOWANCE_FIXTURE_CASE': 'multi'},
                                 capture_output=True, text=True, timeout=12)
            self.assertNotIn(PRIVATE, run.stdout)
            self.assertEqual(json.loads(run.stdout), good)
            self.assertEqual(len(session.wire()), 4)
            session.cache.write_text('{corrupt')
            self.assertEqual(session.run()['state'], 'ok')
            self.assertEqual(len(session.wire()), 8)

    def test_symlinks_and_hardlinks_are_not_followed_or_modified(self):
        for target_kind in ('cache_symlink', 'cache_hardlink', 'lock_symlink'):
            with self.subTest(kind=target_kind), tempfile.TemporaryDirectory(prefix='allowance safe files ') as tmp:
                session = CacheSession(tmp)
                target = Path(tmp) / 'untouched'
                target.write_text('untouched private target')
                target.chmod(0o640)
                leaf = session.lock if target_kind == 'lock_symlink' else session.cache
                if target_kind == 'cache_hardlink':
                    os.link(target, leaf)
                else:
                    leaf.symlink_to(target)
                current = session.run()
                self.assertEqual(current['state'], 'error')
                self.assertEqual(current['windows'], [])
                self.assertEqual(target.read_text(), 'untouched private target')
                self.assertEqual(target.stat().st_mode & 0o777, 0o640)
                self.assertEqual(session.wire(), [])


if __name__ == '__main__':
    unittest.main()
