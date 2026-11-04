import importlib.util
import json
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('repair', Path(__file__).resolve().parents[2]/'scripts/repair-coordinator-parents.py')
repair = importlib.util.module_from_spec(spec); spec.loader.exec_module(repair)

class RepairTests(unittest.TestCase):
    def fixture(self, supported=True, moved=False):
        states = {thread: {'id': thread, 'project': 'p', 'parent': ''} for parent, children in repair.LANES.items() for thread in (parent, *children)}
        first = next(iter(repair.LANES.values()))[0]
        if moved: states[first]['parent'] = 'user-choice'
        writes = []
        def request(caller, method, params):
            if method == 'tools/list':
                return {'tools': [{'name': 'thread_update', 'inputSchema': {'properties': {'parent': {}} if supported else {}}}]}
            name, args = params['name'], params['arguments']
            if name == 'thread_read': value = {'status': dict(states[args['threadId']])}
            elif name == 'thread_update':
                writes.append((args['threadId'], args['parent']))
                states[args['threadId']]['parent'] = args['parent']
                value = {'status': dict(states[args['threadId']])}
            else: self.fail(name)
            return {'content': [{'type': 'text', 'text': json.dumps(value)}]}
        return request, writes

    def test_six_supported_moves_and_idempotent_second_run(self):
        request, writes = self.fixture()
        self.assertEqual(len(repair.repair(request)), 6)
        self.assertEqual(writes, [(child, parent) for parent, children in repair.LANES.items() for child in children])
        self.assertTrue(all(r['result'] == 'already-linked' for r in repair.repair(request)))
        self.assertEqual(len(writes), 6)

    def test_old_harness_refused_before_mutation(self):
        request, writes = self.fixture(supported=False)
        with self.assertRaisesRegex(ValueError, 'install the fix first'): repair.repair(request)
        self.assertEqual(writes, [])

    def test_manual_move_is_kept(self):
        request, writes = self.fixture(moved=True)
        self.assertEqual(repair.repair(request)[0]['result'], 'kept-manual-move')
        self.assertEqual(len(writes), 5)

if __name__ == '__main__': unittest.main()
