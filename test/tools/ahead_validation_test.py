"""Cheap negative controls for source inventory and receipt reuse. No Bend jobs."""
import copy
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT=Path(__file__).resolve().parents[2]


def load(name,path):
    spec=importlib.util.spec_from_file_location(name,ROOT/path)
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);return module


C=load('coverage','scripts/ahead-validation/coverage.py')
G=load('ui_validate','scripts/ui-validate.py')


class Coverage(unittest.TestCase):
    def test_real_binding_removal(self):
        src=C.Source()
        class Deleted(C.Source):
            def read(self,path):
                text=src.read(path)
                return text.replace('"select"','"REMOVED_SELECT_HANDLER"') if path=='src/core/client.bend' else text
        out=C.inventory(Deleted())
        self.assertTrue(any(a['action']=='select' for a in out['missing']))

    def test_copy_gesture_cannot_inherit_generic_copy_handler(self):
        src=C.Source()
        class Deleted(C.Source):
            def read(self,path):
                text=src.read(path)
                return text.replace('action === "copy"','action === "REMOVED_COPY_GESTURE"') if path=='src/web/host.js' else text
        self.assertTrue(C.inventory(src)['copy_local_gesture'])
        self.assertFalse(C.inventory(Deleted())['copy_local_gesture'])

    def test_local_search_owner_must_exist(self):
        src=C.Source()
        class Deleted(C.Source):
            def read(self,path):
                text=src.read(path)
                return text.replace('action === "thin-search-open"','action === "REMOVED_SEARCH_HANDLER"') if path=='src/web/host.js' else text
        self.assertFalse(any(a['action']=='thin-search-open' for a in C.inventory(src)['missing']))
        self.assertTrue(any(a['action']=='thin-search-open' for a in C.inventory(Deleted())['missing']))

    def test_indexed_handler_becomes_noop(self):
        src=C.Source()
        class Noop(C.Source):
            def read(self,path):
                text=src.read(path)
                return text.replace('case 0n:\n      Ui.select(u, value)','case 0n:\n      Act{u, Nil{}}') if path=='src/core/client.bend' else text
        out=C.inventory(Noop());self.assertIn('select',out['syntactically_inert'])
        self.assertTrue(any(a['action']=='select' for a in out['missing']))

    def test_remote_route_and_url_are_independent(self):
        src=C.Source()
        class LocalOnly(C.Source):
            def read(self,path):
                text=src.read(path)
                return text.replace('"/img?path="','"file://"') if path=='src/core/images.bend' else text
        out=C.inventory(LocalOnly());self.assertTrue(out['remote_image_route_present']);self.assertFalse(out['relative_image_url'])

    def test_comments_do_not_create_handlers(self):
        self.assertEqual(C.code('"#data" # "action"\n'),'"#data" \n')
        self.assertEqual(C.arguments('"a,b", f(x, y), z)',0),['"a,b"','f(x, y)','z'])


class Receipts(unittest.TestCase):
    def fixture(self,root):
        # Deliberately synthetic unit data for validating the cache validator,
        # never written to a real CI/pre-push cache and never called evidence.
        binding={'unit-test-only':True};key=G.sha(G.canonical(binding));module=root/(key+'.mjs');module.write_text('unit-test module')
        value={'pass':True,'approach':'simulator','seeds':[{'seed':s,'steps':161}for s in G.SPEC['simulator']['seeds']],
               'parserCases':104,'properties':{'negativeControls':3}}
        transport={'pass':True};trace={'pass':True}
        receipt={'key':key,'binding':binding,'spec':G.SPEC,'result':value,'result_sha256':G.sha(G.canonical(value)),
                 'module_sha256':G.digest(module),'transport':transport,'trace':trace,
                 'lanes_sha256':G.sha(G.canonical([transport,trace]))}
        receipt['receipt_sha256']=G.sha(G.canonical(receipt))
        path=root/(key+'.json');path.write_text(json.dumps(receipt));return key,path,module,receipt

    def test_missing_is_incomplete(self):
        with tempfile.TemporaryDirectory() as d:
            with self.assertRaises(G.Incomplete):G.reuse(Path(d),'missing')

    def test_corrupt_result_module_spec_and_auxiliary_lanes(self):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d);key,path,module,receipt=self.fixture(root)
            self.assertEqual(G.reuse(root,key)['status'],'pass')
            for modify in [lambda r:r['result'].update(pass_=False),lambda r:r['spec'].update(version=999),
                           lambda r:r['trace'].update(extra='corruption'),lambda r:r.update(key='other')]:
                r=copy.deepcopy(receipt);modify(r);path.write_text(json.dumps(r))
                with self.assertRaises(G.Incomplete):G.reuse(root,key)

            path.write_text(json.dumps(receipt));module.write_text('corrupt artifact')
            with self.assertRaises(G.Incomplete):G.reuse(root,key)

    def test_source_closure_includes_import_helpers_and_spec(self):
        paths=G.files_under(ROOT)
        for path in ['src/core/client.bend','src/core/json.bend','src/web/thin.js',
                     'src/web/view-delta.js','src/web/dom.bend','src/server/ui-replay-trace.js',
                     'scripts/ahead-validation/transport.mjs','scripts/ui-validate.py']:
            self.assertIn(path,paths)
        self.assertFalse(any('node_modules' in p for p in paths))

    def test_malformed_receipt_is_incomplete(self):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d);key,path,module,receipt=self.fixture(root)
            for body in [b'null',b'[]',b'3',b'"text"',b'{',b'\xff',b' '*1048577]:
                path.write_bytes(body)
                with self.assertRaises(G.Incomplete):G.reuse(root,key)


class ExportContract(unittest.TestCase):
    def test_git_free_immutable_export_and_changed_endpoints(self):
        with tempfile.TemporaryDirectory() as folder:
            work=Path(folder);repo=work/'repo';repo.mkdir()
            for name in G.files_under(ROOT):
                target=repo/name;target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(ROOT/name,target)
            def git(*args):
                return subprocess.check_output(['git','-c','core.hooksPath=/dev/null','-c','user.name=i2cjak',
                    '-c','user.email=build@amemb.com',*args],cwd=repo,text=True).strip()
            git('init','-q');(repo/'gone.txt').write_text('public fixture');(repo/'from.txt').write_text('public fixture')
            git('add','-A');git('commit','-qm','Synthetic contract base');base=git('rev-parse','HEAD')
            (repo/'gone.txt').unlink();git('mv','from.txt','to.txt');git('add','-A');git('commit','-qm','Synthetic endpoints');head=git('rev-parse','HEAD')
            snapshot=work/'export';shutil.copytree(repo,snapshot,ignore=shutil.ignore_patterns('.git'))
            paths=work/'paths0';paths.write_bytes(subprocess.check_output(['git','diff','--no-renames','--name-only','-z',base,head],cwd=repo))
            self.assertEqual(set(paths.read_bytes().split(b'\0')),{b'gone.txt',b'from.txt',b'to.txt',b''})
            before={p.relative_to(snapshot).as_posix():G.digest(p) for p in snapshot.rglob('*') if p.is_file()}
            def invoke(label,budget=3000):
                report=work/(label+'.json')
                r=subprocess.run([sys.executable,str(snapshot/'scripts/ui-validate.py'),'--snapshot',str(snapshot),
                    '--base',base,'--head',head,'--changed-paths0',str(paths),'--report',str(report),'--budget-ms',str(budget)],
                    cwd=repo,env={**os.environ,'UI_VALIDATE_CACHE':str(work/'empty-cache')},capture_output=True,text=True)
                return r.returncode,json.loads(report.read_text())
            rc,r=invoke('cold');self.assertEqual(rc,3);self.assertIn('cache',r['reason']);self.assertEqual(set(r['approaches']),{'static','simulator','layout','trace'})
            self.assertEqual(r['approaches']['static']['literal_contracts'],'pass')
            self.assertEqual(invoke('deadline',1)[0],3)
            paths.write_bytes(b'gone.txt\0to.txt\0');rc,r=invoke('missing-endpoint');self.assertEqual(rc,3);self.assertIn('path set',r['reason'])
            after={p.relative_to(snapshot).as_posix():G.digest(p) for p in snapshot.rglob('*') if p.is_file()}
            self.assertEqual(before,after,'validator writes nothing into the export')
            self.assertFalse((snapshot/'.git').exists())


if __name__=='__main__':unittest.main()
