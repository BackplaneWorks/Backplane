import pathlib,tempfile,json,hashlib,importlib.util,unittest,copy,sys
from unittest.mock import patch
w=pathlib.Path(__file__).resolve().parents[2];spec=importlib.util.spec_from_file_location('release',w/'scripts/service-release.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
class Gates(unittest.TestCase):
 def setUp(self):
  self.tmp=tempfile.TemporaryDirectory();self.addCleanup(self.tmp.cleanup);self.root=pathlib.Path(self.tmp.name);self.product=self.root/'product';self.product.mkdir();(self.product/'source.txt').write_text('real source bytes');m.ROOT=self.product;self.e=self.root/'emission';(self.e/'logs').mkdir(parents=True);(self.e/'web').mkdir();(self.e/'server.js').write_text('raw emitted JS');(self.e/'web/chunk-test.js').write_text('raw emitted web');(self.e/'logs/entries.log').write_text('EXACT_SERVER_AND_WEB_ENTRY_CHECKS_PASS\n')
  self.release={'commit':'a'*40,'tree':'b'*40};self.compiler={'sha256':'c'*64,'version':'2.0.36'}
  self.source={'commit':'a'*40,'tree':'b'*40,'members':{'source.txt':m.digest(self.product/'source.txt')}};(self.e/'SOURCE.json').write_text(json.dumps(self.source));(self.e/'PHASES.json').write_text(json.dumps([{'phase':p,'exitCode':0} for p in ['entries','server','web']]))
  self.d={'schema':1,'kind':'backplane-secret-free-js-emission','status':'VERIFIED','sourceCommit':'a'*40,'sourceTree':'b'*40,'version':'0.14.0','channel':'release','compiler':self.compiler,'serverEmitKey':'d'*40,'sourceManifest':'SOURCE.json','server':'server.js','web':'web','entryCheckLog':'logs/entries.log'}
 def seal(self):
  self.d['files']={p.relative_to(self.e).as_posix():{'sha256':m.digest(p),'bytes':p.stat().st_size} for p in self.e.rglob('*') if p.is_file() and p.name!='EMISSION.json'};(self.e/'EMISSION.json').write_text(json.dumps(self.d));self.ref=m.reference(self.e/'EMISSION.json')
 def verify(self):
  with patch.object(m,'git',return_value='source.txt'),patch.object(m.subprocess,'check_output',return_value='d'*40):return m.verified_preemission(self.ref,self.root,self.release,self.compiler)
 def reject(self):
  with self.assertRaises((ValueError,OSError)):self.verify()
 def test_valid_attested_fixture(self):self.seal();self.assertEqual(self.verify()[0],self.e)
 def test_source_identity(self):self.d['sourceCommit']='f'*40;self.seal();self.reject()
 def test_compiler_identity(self):self.d['compiler']={'sha256':'e'*64,'version':'2.0.36'};self.seal();self.reject()
 def test_unattested_byte_tamper(self):self.seal();(self.e/'server.js').write_text('tampered');self.reject()
 def test_extra_member(self):self.seal();(self.e/'extra.js').write_text('extra');self.reject()
 def test_symlink(self):self.seal();(self.e/'link').symlink_to('server.js');self.reject()
 def test_missing_entry_verdict(self):(self.e/'logs/entries.log').write_text('not executed');self.seal();self.reject()
 def test_failed_phase(self):(self.e/'PHASES.json').write_text(json.dumps([{'phase':p,'exitCode':1} for p in ['entries','server','web']]));self.seal();self.reject()
 def test_missing_phase(self):(self.e/'PHASES.json').write_text('[]');self.seal();self.reject()
 def test_wrong_source_bytes(self):self.source['members']['source.txt']='0'*64;(self.e/'SOURCE.json').write_text(json.dumps(self.source));self.seal();self.reject()
 def test_wrong_closure_key(self):self.d['serverEmitKey']='f'*40;self.seal();self.reject()
 def test_absent_web(self):(self.e/'web/chunk-test.js').unlink();self.seal();self.reject()
 def test_receipt_tamper(self):self.seal();(self.e/'EMISSION.json').write_text('{}');self.reject()
if __name__=='__main__':unittest.main(verbosity=2)
