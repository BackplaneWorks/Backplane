"""Focused no-wait/fallback receipt checks; never starts a heavy gate."""
import importlib.util
import zlib
from types import SimpleNamespace
import os
import io
import contextlib
import base64
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT=Path(__file__).resolve().parents[2]
def module(name,path):
    spec=importlib.util.spec_from_file_location(name,ROOT/path)
    value=importlib.util.module_from_spec(spec);spec.loader.exec_module(value);return value
ci=module('local_ci','scripts/local-ci.py')
phase=module('local_phase','scripts/local-phase.py')


class Resources(unittest.TestCase):
    def test_unavailable_phase_moves_once_to_boat(self):
        with tempfile.TemporaryDirectory() as d, patch.object(ci,'CACHE',Path(d)), \
             patch.object(ci,'local_phase',return_value=(75,{})) as local, \
             patch.object(ci,'boat_phase',return_value=(0,{'backend':'boat'},None)) as remote:
            rc,data=ci.execute_phase('proof',['PROOF.bend'],[],['scripts/check.sh','proof'],Path(d))
            self.assertEqual(rc,0);self.assertEqual(data['backend'],'boat')
            self.assertEqual(local.call_count,1);self.assertEqual(remote.call_count,1)

    def test_real_phase_failure_does_not_start_duplicate(self):
        with tempfile.TemporaryDirectory() as d, patch.object(ci,'CACHE',Path(d)), \
             patch.object(ci,'local_phase',return_value=(1,{'seconds':1.0})), \
             patch.object(ci,'boat_phase') as remote:
            self.assertEqual(ci.execute_phase('proof',[],[],[],Path(d))[0],1)
            remote.assert_not_called()

    def test_killed_scope_without_result_moves_unfinished_phase(self):
        with tempfile.TemporaryDirectory() as d, patch.object(ci,'CACHE',Path(d)), \
             patch.object(ci,'local_phase',return_value=(-15,{})), \
             patch.object(ci,'boat_phase',return_value=(0,{'backend':'boat'},None)) as remote:
            self.assertEqual(ci.execute_phase('tests',[],[],[],Path(d))[0],0)
            self.assertEqual(remote.call_count,1)

    def test_boat_exit_zero_without_matching_receipt_is_incomplete(self):
        with tempfile.TemporaryDirectory() as d, patch.object(ci,'base_key',return_value='expected'), \
             patch.object(ci.subprocess,'run') as run:
            run.return_value.returncode=0
            rc,_,_=ci.boat_phase('proof','proof',[],[],Path(d))
            self.assertEqual(rc,3)

    def test_check_reservation_uses_measured_peak_plus_margin(self):
        gib = 1073741824
        for previous, expected in [(None,16),(8*gib,10),(20*gib,24)]:
            with self.subTest(previous=previous), tempfile.TemporaryDirectory() as d, \
                 patch.object(ci,'CACHE',Path(d)), \
                 patch.dict(ci.os.environ,{'BP_LOCAL_CI_BOAT':''}), \
                 patch.object(ci,'local_phase',return_value=(1,{'seconds':1.0})) as local, \
                 patch.object(ci,'boat_phase') as remote:
                peaks=Path(d)/'local-phase-peaks.json'
                history={'check':previous} if previous is not None else {}
                peaks.write_text(json.dumps(history))
                before=peaks.read_bytes()
                command=['scripts/check.sh','check']
                rc,_=ci.execute_phase('check',['fixture.bend'],['checker.py'],command,Path(d))
                self.assertEqual(rc,1)
                local.assert_called_once_with('check',command,Path(d),expected)
                remote.assert_not_called()
                self.assertEqual(peaks.read_bytes(),before)
                self.assertFalse((Path(d)/'local-ci-passes').exists())

    def test_measured_check_unavailable_moves_once_without_editing_history(self):
        with tempfile.TemporaryDirectory() as d, patch.object(ci,'CACHE',Path(d)), \
             patch.dict(ci.os.environ,{'BP_LOCAL_CI_BOAT':''}), \
             patch.object(ci,'local_phase',return_value=(75,{})) as local, \
             patch.object(ci,'boat_phase',return_value=(124,{'backend':'boat'},None)) as remote:
            peaks=Path(d)/'local-phase-peaks.json'
            peaks.write_text(json.dumps({'check':8*1073741824}))
            before=peaks.read_bytes()
            command=['scripts/check.sh','check']
            rc,_=ci.execute_phase('check',['fixture.bend'],['checker.py'],command,Path(d))
            self.assertEqual(rc,124)
            local.assert_called_once_with('check',command,Path(d),10)
            remote.assert_called_once_with('check','check',['fixture.bend'],['checker.py'],Path(d))
            self.assertEqual(peaks.read_bytes(),before)
            self.assertFalse((Path(d)/'local-ci-passes').exists())

    def test_noncheck_phase_keeps_smaller_history_adaptation(self):
        with tempfile.TemporaryDirectory() as d, patch.object(ci,'CACHE',Path(d)), \
             patch.dict(ci.os.environ,{'BP_LOCAL_CI_BOAT':''}), \
             patch.object(ci,'local_phase',return_value=(1,{'seconds':1.0})) as local:
            (Path(d)/'local-phase-peaks.json').write_text(json.dumps({'proof':8*1073741824}))
            ci.execute_phase('proof',[],[],['scripts/check.sh','proof'],Path(d))
            local.assert_called_once_with('proof',['scripts/check.sh','proof'],Path(d),10)

    def test_check_uses_unchanged_no_wait_admission_contract(self):
        with tempfile.TemporaryDirectory() as d, patch.object(ci.subprocess,'run') as run:
            folder=Path(d)
            admit=folder/'admit.sh'
            admit.write_text('# fixture supports --no-wait)\n')
            run.return_value.returncode=75
            with patch.dict(ci.os.environ,{'BP_ADMIT':str(admit)}):
                rc,_=ci.local_phase('check',['scripts/check.sh','check'],folder,16)
            self.assertEqual(rc,75)
            args,kwargs=run.call_args
            self.assertEqual(args[0][:9],[str(admit),'--no-wait','--priority','--mem','16','--cores','2','--',ci.sys.executable])
            self.assertEqual(args[0][-2:],['scripts/check.sh','check'])
            self.assertEqual(kwargs['env']['BACKPLANE_MEM_GB'],'16')
            self.assertEqual(run.call_count,1)

    def test_measurement_labels_process_peak_and_exits(self):
        with tempfile.TemporaryDirectory() as d:
            receipt=Path(d)/'phase.json'
            self.assertEqual(phase.measure(['/bin/true'],receipt),0)
            data=json.loads(receipt.read_text())
            self.assertGreater(data['peak_rss_kib'],0)
            self.assertNotIn('aggregate_peak_bytes',data)
            self.assertIn('not aggregate',data['rss_measurement'])


def encoded(spec):
    return base64.urlsafe_b64encode(zlib.compress(json.dumps(spec).encode())).decode()


class Layout(unittest.TestCase):
    def test_selected_layout_and_bounded_decimal_validation(self):
        for value in ['1', '4', '8', '16']:
            with self.subTest(value=value), patch.dict(os.environ, {'CHECK_SHARDS': value}):
                self.assertEqual(ci.check_layout(), {'jobs': 1, 'shards': int(value)})
        for value in ['', '0', '-1', '17', '04', '+4', '4.0', ' 4', '4\n', 'x']:
            with self.subTest(value=value), patch.dict(os.environ, {'CHECK_SHARDS': value}):
                with self.assertRaises(ValueError):
                    ci.check_layout()
        with patch.dict(os.environ, {}, clear=True):
            self.assertEqual(ci.check_layout(), {'jobs': 1, 'shards': 1})

    def test_main_honors_shards_but_forces_serial_jobs(self):
        with tempfile.TemporaryDirectory() as d, patch.object(ci, 'CACHE', Path(d)), \
                patch.dict(os.environ, {'CHECK_SHARDS': '8', 'CHECK_JOBS': '99'}), \
                patch.object(ci.sys, 'argv', ['local-ci.py', '--only', 'check']), \
                patch.object(ci, 'execute') as execute:
            def inspect(args):
                self.assertEqual(args.only, 'check')
                self.assertEqual(os.environ['CHECK_SHARDS'], '8')
                self.assertEqual(os.environ['CHECK_JOBS'], '1')
                self.assertEqual(os.environ['CHECK_FALLBACK'], '0')
                return 0
            execute.side_effect = inspect
            self.assertEqual(ci.main(), 0)
            execute.assert_called_once()

    def test_invalid_layout_fails_before_controller_or_cache(self):
        with tempfile.TemporaryDirectory() as d, patch.object(ci, 'CACHE', Path(d) / 'cache'), \
                patch.dict(os.environ, {'CHECK_SHARDS': '0'}), \
                patch.object(ci.sys, 'argv', ['local-ci.py', '--only', 'check']), \
                patch.object(ci, 'execute') as execute, contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaises(SystemExit) as error:
                ci.main()
            self.assertEqual(error.exception.code, 2)
            execute.assert_not_called()
            self.assertFalse(ci.CACHE.exists())

    def test_check_source_key_and_controller_key_bind_shards(self):
        with patch.dict(os.environ, {'CHECK_SHARDS': '8'}), patch.object(ci, 'output', return_value='source') as output:
            ci.base_key('check', ['a.bend'], ['driver'])
            self.assertEqual(output.call_args.args[0][1], 'local-ci-v2:check:serial-shards=8')
        keys = []
        with patch.object(ci, 'base_key', return_value='same-source'), patch.object(ci, 'output', return_value='same-tool'):
            for value in ['1', '4', '8']:
                with patch.dict(os.environ, {'CHECK_SHARDS': value, 'CHECK_JOBS': '1'}):
                    keys.append(ci.key('check', [], []))
        self.assertEqual(len(set(keys)), 3)

    def test_boat_spec_and_receipt_require_exact_selected_layout(self):
        for received in [{'jobs': 1, 'shards': 8}, None, {'jobs': 1, 'shards': 1},
                         {'jobs': True, 'shards': 8}, {'jobs': 1, 'shards': 8.0}]:
            with self.subTest(received=received), tempfile.TemporaryDirectory() as d, \
                    patch.dict(os.environ, {'CHECK_SHARDS': '8'}), \
                    patch.object(ci, 'base_key', return_value='bound-source'), \
                    patch.object(ci.subprocess, 'run') as run:
                folder = Path(d)
                remote = folder / 'remote'
                artifact = remote / 'artifacts/local-remote/check'
                artifact.mkdir(parents=True)
                receipt = {'base': 'bound-source', 'rc': 0}
                if received is not None:
                    receipt['check_layout'] = received
                (artifact / 'phase.json').write_text(json.dumps(receipt))
                def fake(argv, **kwargs):
                    spec = json.loads(zlib.decompress(base64.urlsafe_b64decode(argv[-1])))
                    self.assertEqual(spec['check_layout'], {'jobs': 1, 'shards': 8})
                    self.assertEqual(spec['check_shards'], '8')
                    self.assertEqual(spec['roots'], ['a.bend'])
                    self.assertEqual(spec['base'], 'bound-source')
                    kwargs['stdout'].write('logs and artifacts in ' + str(remote) + '\n')
                    return SimpleNamespace(returncode=0)
                run.side_effect = fake
                rc, _, _ = ci.boat_phase('check', 'check', ['a.bend'], ['driver'], folder)
                exact = received is not None and type(received['jobs']) is int and type(received['shards']) is int and received['shards'] == 8
                self.assertEqual(rc, 0 if exact else 3)
                run.assert_called_once()

    def test_remote_layout_is_strict_and_required(self):
        bad = [None, {}, {'jobs': 2, 'shards': 8}, {'jobs': True, 'shards': 8},
               {'jobs': 1, 'shards': 0}, {'jobs': 1, 'shards': 17},
               {'jobs': 1, 'shards': '8'}, {'jobs': 1, 'shards': 8.0}]
        for layout in bad:
            with self.subTest(layout=layout):
                with self.assertRaises(SystemExit):
                    phase.check_layout({'name': 'check', 'gate': 'check', 'check_layout': layout})
        for count in [1, 4, 8, 16]:
            spec = {'name': 'check', 'gate': 'check', 'check_layout': {'jobs': 1, 'shards': count}}
            self.assertEqual(phase.check_layout(spec), spec['check_layout'])

    def test_legacy_shard_field_cannot_conflict_with_bound_check_layout(self):
        spec = {'name': 'check', 'gate': 'check', 'check_layout': {'jobs': 1, 'shards': 8}}
        self.assertEqual(phase.check_layout({**spec, 'check_shards': '8'}), spec['check_layout'])
        for value in ['1', 8, True, '08', '8.0']:
            with self.subTest(value=value), self.assertRaisesRegex(SystemExit, 'Conflicting'):
                phase.check_layout({**spec, 'check_shards': value})

    def test_noncheck_remote_preserves_current_shard_field_without_check_layout(self):
        with tempfile.TemporaryDirectory() as d:
            folder = Path(d)
            compiler = folder / 'fake-bend'
            compiler.write_bytes(b'not executed')
            spec = {'name': 'proof', 'gate': 'proof', 'roots': ['PROOF.bend'],
                    'files': ['driver'], 'base': 'bound-source', 'check_shards': '4'}
            def measured(command, record):
                self.assertEqual(command, ['scripts/check.sh', 'proof'])
                self.assertEqual(os.environ['CHECK_SHARDS'], '4')
                self.assertEqual(os.environ['CHECK_JOBS'], '1')
                Path(record).write_text(json.dumps({'rc': 0}))
                return 0
            with patch.object(phase, 'ROOT', folder), patch.object(phase.os, 'chdir'), \
                    patch.dict(os.environ, {}), patch.object(phase, 'measure', side_effect=measured), \
                    patch.object(phase.subprocess, 'check_output', return_value='bound-source') as inputs, \
                    patch('shutil.which', return_value=str(compiler)):
                self.assertEqual(phase.boat(encoded(spec)), 0)
                self.assertNotIn('check_layout', json.loads((folder/'build/local-remote/proof/phase.json').read_text()))
                for call in inputs.call_args_list:
                    self.assertEqual(call.args[0][1], 'local-ci-v2:proof')

    def remote_run(self, folder, change_layout=False, source='bound-source'):
        spec = {'name': 'check', 'gate': 'check', 'roots': ['a.bend'], 'files': ['driver'],
                'base': 'bound-source', 'check_layout': {'jobs': 1, 'shards': 8}}
        compiler = folder / 'fake-bend'
        compiler.write_bytes(b'not executed')
        def measure(command, record):
            self.assertEqual(command, ['scripts/check.sh', 'check'])
            self.assertEqual(os.environ['CHECK_SHARDS'], '8')
            self.assertEqual(os.environ['CHECK_JOBS'], '1')
            Path(record).write_text(json.dumps({'rc': 0}))
            if change_layout:
                os.environ['CHECK_SHARDS'] = '1'
            return 0
        with patch.object(phase, 'ROOT', folder), patch.object(phase.os, 'chdir'), \
                patch.dict(os.environ, {'CHECK_SHARDS': '1', 'CHECK_JOBS': '99'}), \
                patch.object(phase, 'measure', side_effect=measure) as measured, \
                patch.object(phase.subprocess, 'check_output', return_value=source) as inputs, \
                patch('shutil.which', return_value=str(compiler)):
            rc = phase.boat(encoded(spec))
            measured.assert_called_once()
            for call in inputs.call_args_list:
                self.assertEqual(call.args[0][1], 'local-ci-v2:check:serial-shards=8')
            return rc, json.loads((folder / 'build/local-remote/check/phase.json').read_text())

    def test_remote_runs_selected_layout_and_records_it(self):
        with tempfile.TemporaryDirectory() as d:
            rc, receipt = self.remote_run(Path(d))
            self.assertEqual(rc, 0)
            self.assertEqual(receipt['check_layout'], {'jobs': 1, 'shards': 8})
            self.assertEqual(receipt['base'], 'bound-source')

    def test_remote_layout_drift_refuses_receipt(self):
        with tempfile.TemporaryDirectory() as d:
            with self.assertRaisesRegex(SystemExit, 'layout changed'):
                self.remote_run(Path(d), change_layout=True)

    def test_remote_source_mismatch_never_runs_check(self):
        with tempfile.TemporaryDirectory() as d, patch.object(phase, 'ROOT', Path(d)), \
                patch.object(phase.os, 'chdir'), patch.dict(os.environ, {}), \
                patch.object(phase, 'measure') as measure, \
                patch.object(phase.subprocess, 'check_output', return_value='wrong-source'):
            spec = {'name': 'check', 'gate': 'check', 'roots': [], 'files': [],
                    'base': 'expected', 'check_layout': {'jobs': 1, 'shards': 8}}
            with self.assertRaisesRegex(SystemExit, 'input mismatch'):
                phase.boat(encoded(spec))
            measure.assert_not_called()

if __name__=='__main__':unittest.main()
