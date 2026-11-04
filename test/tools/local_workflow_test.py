"""Hermetic deployment guards; never accesses the installed home or service."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('local_workflow', ROOT / 'scripts/local.py')
local = importlib.util.module_from_spec(spec)
spec.loader.exec_module(local)


class LocalWorkflow(unittest.TestCase):
    def test_transferred_emit_requires_exact_inputs_and_bytes_before_caching(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp);prepared=root/'prepared';prepared.mkdir()
            (prepared/'emit.js').write_text('verified compiler output')
            key='a'*40
            meta={'emit_key':key,'emit_sha256':local.digest(prepared/'emit.js')}
            env={'BACKPLANE_EMIT_CACHE':str(root/'cache')}
            with patch.object(local.subprocess,'check_output',return_value=key):
                local.import_emit(prepared,meta,env)
                cached=(root/'cache'/(key+'.js')).read_text()
                (prepared/'emit.js').write_text('corrupted transfer')
                with self.assertRaisesRegex(SystemExit,'input/hash mismatch'):
                    local.import_emit(prepared,meta,env)
                self.assertEqual((root/'cache'/(key+'.js')).read_text(),cached)
                with self.assertRaisesRegex(SystemExit,'input/hash mismatch'):
                    local.import_emit(prepared,{**meta,'emit_key':'different'},env)

    def test_apply_requires_smoke_and_never_requests_deploy_on_refusal(self):
        with tempfile.TemporaryDirectory() as temp:
            stage = Path(temp)
            (stage/'LOCAL.json').write_text(json.dumps({'smoked':False}))
            with patch.object(local, 'request_deploy') as deploy:
                with self.assertRaisesRegex(SystemExit, 'no passing'):
                    local.apply(stage)
                deploy.assert_not_called()

    def test_changed_artifact_cannot_be_applied(self):
        with tempfile.TemporaryDirectory() as temp:
            stage = Path(temp);(stage/'payload').mkdir()
            (stage/'payload/backplane-serve').write_text('changed')
            (stage/'LOCAL.json').write_text(json.dumps({'smoked':True,'runtime_passed':True,'service_sha256':'wrong'}))
            with patch.object(local, 'request_deploy') as deploy:
                with self.assertRaisesRegex(SystemExit, 'changed'): local.apply(stage)
                deploy.assert_not_called()

    def test_apply_requires_shared_runtime_gate(self):
        with tempfile.TemporaryDirectory() as temp:
            stage = Path(temp)
            (stage/'LOCAL.json').write_text(json.dumps({'smoked':True}))
            with patch.object(local,'request_deploy') as deploy:
                with self.assertRaisesRegex(SystemExit,'runtime gate'): local.apply(stage)
                deploy.assert_not_called()

    def test_desktop_activation_requires_the_exact_ready_service(self):
        with tempfile.TemporaryDirectory() as temp:
            store = Path(temp);stage=store/'review';meta=store/'review/stage';meta.mkdir(parents=True)
            (meta/'LOCAL.json').write_text(json.dumps({'version':'new','commit':'expected'}))
            with patch.object(local, 'STORE', store), patch.object(local,'greeting',return_value={'backplane':'0.12.1','commit':'old'}):
                with self.assertRaisesRegex(SystemExit,'not ready'): local.activate(stage)


if __name__ == '__main__': unittest.main()
