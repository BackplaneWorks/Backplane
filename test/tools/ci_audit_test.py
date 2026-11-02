import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('audit', Path(__file__).resolve().parents[2]/'scripts/ci-audit.py')
audit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(audit)


class AuditTest(unittest.TestCase):
    def test_queue_excludes_dependencies_and_skips(self):
        run = dict(id=1, name='CI', event='pull_request', head_sha='a', created_at='2026-11-02T00:00:00Z')
        job = dict(id=2, name='native', created_at='2026-11-02T00:02:00Z',
                   started_at='2026-11-02T00:02:08Z', completed_at='2026-11-02T00:02:30Z', steps=[
                       dict(name='Run actions/checkout@v4', status='completed', conclusion='success',
                            started_at='2026-11-02T00:02:08Z', completed_at='2026-11-02T00:02:10Z'),
                       dict(name='Build', status='completed', conclusion='success',
                            started_at='2026-11-02T00:02:10Z', completed_at='2026-11-02T00:02:30Z'),
                       dict(name='Skipped proof', status='completed', conclusion='skipped')])
        row = audit.job_row(run, job)
        self.assertEqual((row['dependency_seconds'], row['queue_seconds'], row['setup_seconds'], row['running_seconds']), (120, 8, 2, 20))

    def test_missing_queue_is_unknown(self):
        self.assertIsNone(audit.seconds(None, '2026-11-02T00:00:00Z'))


if __name__ == '__main__':
    unittest.main()
