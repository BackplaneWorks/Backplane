#!/usr/bin/env python3
"""Boat state files keep supplementary Unicode as UTF-8, not surrogate escapes."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class BoatStateUnicodeTest(unittest.TestCase):
    def test_state_files_round_trip_supplementary_unicode(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            project = root / 'project'
            fake = root / 'fake'
            state = root / 'state'
            for directory in (project, fake, state):
                directory.mkdir()
            subprocess.run(['git', 'init', '-q', str(project)], check=True)
            (project / 'source.txt').write_text('fixture\n', encoding='utf-8')
            subprocess.run(['git', '-C', str(project), 'add', 'source.txt'], check=True)
            subprocess.run(['git', '-C', str(project), '-c', 'user.name=test',
                            '-c', 'user.email=test@example.invalid', 'commit', '-qm', 'fixture'], check=True)
            (state / 'enabled').write_text(str(project) + '\n', encoding='utf-8')
            (fake / 'limits.json').write_text(
                '{"activeSandboxes":ACTIVE,"maxActiveSandboxes":10,"canStart":true,'
                '"planName":"plan 🚀","subscriptionRemainingSeconds":90000,"creditBalanceSeconds":2000000,'
                '"starts":STARTS,"machineTypes":[{"type":"default","available":true}]}',
                encoding='utf-8')
            env = {**os.environ, 'BOAT_FAKE_DIR': str(fake), 'BOAT_JOB_ROOT': str(state),
                   'BACKPLANE_BOAT': str(ROOT / 'test/tools/boat-fake'), 'BOAT_JOB_SCOPED': '1'}
            marker = 'command 🚀'
            result = subprocess.run([str(ROOT / 'scripts/boat-job.sh'), '--no-setup-bend',
                                     '--no-reuse', '--idle', '0', '--wait', '0', '--',
                                     'sh', '-c', 'printf %s ' + "'" + marker + "'"],
                                    cwd=project, env=env, stdout=subprocess.DEVNULL,
                                    stderr=subprocess.DEVNULL, timeout=90)
            self.assertEqual(result.returncode, 0)
            jobs = list((state / 'jobs').iterdir())
            self.assertEqual(len(jobs), 1)
            paths = [state / 'ledger.jsonl', state / 'status.json', state / 'account.json',
                     jobs[0] / 'provenance.json']
            texts = [p.read_text(encoding='utf-8') for p in paths]
            for text in texts:
                self.assertNotIn('\\ud83d', text.lower())
                self.assertNotIn('\\ude80', text.lower())
                self.assertIn('🚀', text)
            ledger = [json.loads(line) for line in texts[0].splitlines()]
            self.assertTrue(any(marker in entry.get('cmd', '') for entry in ledger))
            self.assertTrue(any(marker in job.get('cmd', '') for job in json.loads(texts[1])['jobs']))
            self.assertEqual(json.loads(texts[2])['plan'], 'plan 🚀')
            self.assertIn('🚀', json.loads(texts[3])['command'])


if __name__ == '__main__':
    unittest.main()
