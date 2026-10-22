'use strict';
// Offline metadata and asset fixtures only; no Bend, browser or payload executes.
const{test}=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),os=require('node:os'),zlib=require('node:zlib'),{execFileSync}=require('node:child_process');
const p=require('../produce-release.cjs'),webProducer=require('../produce-web-release.cjs');
const write=(file,value)=>{fs.mkdirSync(path.dirname(file),{recursive:true});fs.writeFileSync(file,typeof value==='object'?JSON.stringify(value,null,2)+'\n':value);return file;};
const ref=file=>({path:file,sha256:p.digest(file),bytes:fs.statSync(file).size});
function fixture(t){
 const root=fs.mkdtempSync(path.join(os.tmpdir(),'bp-web-offline-'));t.after(()=>fs.rmSync(root,{recursive:true,force:true}));
 const release={commit:'a'.repeat(40),tree:'b'.repeat(40),version:'0.12.1',tag:'v0.12.1',channel:'release'},assets=path.join(root,'web');
 write(path.join(assets,'index.html'),'<main>日本語🙂 synthetic only</main>');write(path.join(assets,'app.js'),'// synthetic data\n');write(path.join(assets,'LICENSE.Backplane'),'SYNTHETIC test license');
 for(const name of ['index.html','app.js'])fs.writeFileSync(path.join(assets,name+'.gz'),zlib.gzipSync(fs.readFileSync(path.join(assets,name))));
 const members=Object.fromEntries(Object.entries(p.inventory(assets,'linux-x64')).map(([n,v])=>[n,{sha256:v.sha256,bytes:v.bytes}]));
 const manifest={schema:1,kind:'matching-generated-web',sourceCommit:release.commit,sourceTree:release.tree,entrypoint:'src/web/index.html',members};
 const manifestRef=ref(write(path.join(root,'WEB.json'),manifest)),log=ref(write(path.join(root,'check.log'),'PASS synthetic only\n'));
 const source={schema:1,kind:'backplane-web-source-qualification',status:'PASS',commit:release.commit,tree:release.tree,version:'0.12.1',checks:[log]};
 const emission={schema:1,kind:'backplane-web-emission',status:'VERIFIED',target:'web',sourceCommit:release.commit,sourceTree:release.tree,version:'0.12.1',channel:'release',entrypoint:'src/web/index.html',webManifestSha256:manifestRef.sha256,compiler:{version:'synthetic',sha256:'c'.repeat(64)},sourceQualification:ref(write(path.join(root,'source.json'),source)),packages:[{name:'Backplane',version:'0.12.1',sourceDisposition:'Synthetic test only',licenses:[{path:'LICENSE.Backplane',sha256:members['LICENSE.Backplane'].sha256}]}]};
 const emissionRef=ref(write(path.join(root,'emission.json'),emission)),freeze={schema:1,kind:'backplane-release-freeze',status:'FROZEN',...release,sourceDateEpoch:1700000000,components:{web:{webManifestSha256:manifestRef.sha256,emissionReceiptSha256:emissionRef.sha256}}};
 const request={schema:1,kind:'backplane-web-release-inputs',target:'web',release:{...release,freezeReceipt:ref(write(path.join(root,'freeze.json'),freeze))},web:{receipt:manifestRef,payload:{directory:assets}},emissionReceipt:emissionRef};
 const requestFile=write(path.join(root,'request.json'),request);
 return{root,request,requestFile,emission,source,freeze,assets,refresh(){request.emissionReceipt=ref(write(path.join(root,'emission.json'),emission));freeze.components.web.emissionReceiptSha256=request.emissionReceipt.sha256;request.release.freezeReceipt=ref(write(path.join(root,'freeze.json'),freeze));write(requestFile,request);}};
}
test('callable standalone web adapter creates deterministic exact archive and public receipt',t=>{
 const f=fixture(t),cli=path.resolve(__dirname,'../produce-web-release.cjs');const described=JSON.parse(execFileSync(process.execPath,[cli,'describe']));assert.equal(described.target,'web');
 const a=JSON.parse(execFileSync(process.execPath,[cli,'archive','--request',f.requestFile,'--output',path.join(f.root,'dist')]));const b=webProducer.archive(f.requestFile,path.join(f.root,'again'));assert.equal(a.sha256,b.sha256);assert.equal(a.published,false);
 const receipt=JSON.parse(fs.readFileSync(a.receipt));assert.equal(receipt.web.root,'backplane-web/web');assert.equal(receipt.releaseQualified,false);const extracted=path.join(f.root,'extracted');p.runPython(['extract',a.archive,extracted]);
 const actual=Object.fromEntries(Object.entries(p.inventory(extracted,'linux-x64')).map(([n,v])=>[n,{sha256:v.sha256,bytes:v.bytes}]));assert.deepEqual(receipt.members,actual);assert(fs.readFileSync(path.join(extracted,'backplane-web/WEB.json')).equals(fs.readFileSync(f.request.web.receipt.path)));
});
test('dev versions, missing qualification, compiler identity and mismatching freeze cannot package',t=>{
 for(const change of [f=>f.request.release.version='0.12.1-dev.fixture',f=>delete f.emission.compiler,f=>f.emission.sourceTree='c'.repeat(40),f=>f.emission.status='PENDING',f=>f.freeze.components.web.webManifestSha256='0'.repeat(64)]){
  const f=fixture(t);change(f);f.refresh();const out=path.join(f.root,'dist');assert.throws(()=>webProducer.archive(f.requestFile,out));assert(!fs.existsSync(out));
 }
});
test('wrong bytes, omitted gzip, extra files and missing notice cannot be packaged',t=>{
 for(const change of [f=>write(path.join(f.assets,'app.js'),'changed'),f=>fs.unlinkSync(path.join(f.assets,'index.html.gz')),f=>write(path.join(f.assets,'unexpected'),'extra'),f=>f.emission.packages=[]]){
  const f=fixture(t);change(f);f.refresh();assert.throws(()=>webProducer.archive(f.requestFile,path.join(f.root,'dist')));
 }
});
test('web input references remain portable from their containing JSON without payload discovery',t=>{
 const f=fixture(t);f.emission.sourceQualification.path='source.json';f.source.checks[0].path='check.log';f.emission.sourceQualification=ref(write(path.join(f.root,'source.json'),f.source));f.emission.sourceQualification.path='source.json';f.refresh();
 for(const r of [f.request.emissionReceipt,f.request.web.receipt,f.request.release.freezeReceipt])r.path=path.basename(r.path);f.request.web.payload.directory='web';write(f.requestFile,f.request);
 const moved=fs.mkdtempSync(path.join(os.tmpdir(),'bp-web-relocated-'));t.after(()=>fs.rmSync(moved,{recursive:true,force:true}));fs.cpSync(f.root,moved,{recursive:true});assert.equal(webProducer.archive(path.join(moved,'request.json'),path.join(moved,'dist')).status,'PACKAGED_FROM_QUALIFIED_WEB_ARTIFACT');
});
