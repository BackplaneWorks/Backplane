'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict');
const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),vm=require('node:vm');
test('only the trusted main frame can persist a bounded draft, including a clear',()=>{
  const dir=fs.mkdtempSync(path.join(os.tmpdir(),'bp-home-drafts-'));
  const source=fs.readFileSync(path.join(__dirname,'../main.cjs'),'utf8');
  const block=source.slice(source.indexOf('const draftsFile = '),source.indexOf('// The theme the hub'));
  const start=()=>{let handle;vm.runInNewContext(block,{path,fs,app:{getPath:()=>dir},trusted:e=>e.allowed===true,ipcMain:{on:(name,fn)=>{assert.equal(name,'desktop:drafts');handle=fn;}}});return (value,allowed=true)=>{let result,calls=0;const e={allowed,set returnValue(v){result=v;calls++;}};handle(e,value);assert.equal(calls,1,'a synchronous IPC reply is sent on the first assignment');return result;};};
  try {
    let call=start();assert.equal(call({op:'read'},false),null);
    for(const [key,text]of [['token','secret'],['backplane-draft:x',{}],['backplane-draft:x','a'.repeat(4*1024*1024+1)]])assert.equal(call({op:'write',key,text}),null);
    const key='backplane-draft:thread';assert.equal(call({op:'write',key,text:'unsent'}),true);
    assert.equal(call({op:'write',key,text:'forged'},false),null);
    call=start();assert.equal(JSON.stringify(call({op:'read'})),JSON.stringify([[key,'unsent']]));
    assert.equal(call({op:'write',key,text:null}),true);call=start();assert.equal(JSON.stringify(call({op:'read'})),JSON.stringify([[key,null]]));
    assert.deepEqual(fs.readdirSync(dir),['drafts.json']);assert.equal(fs.statSync(path.join(dir,'drafts.json')).mode&0o777,0o600);
  } finally {fs.rmSync(dir,{recursive:true,force:true});}
});
function storage(entries=[]) {
  const map=new Map(entries);
  return {get length(){return map.size;},key:i=>[...map.keys()][i]??null,getItem:k=>map.get(k)??null,setItem:(k,v)=>map.set(k,v),removeItem:k=>map.delete(k)};
}
test('drafts migrate from the current port and follow the home through two other ports',async()=>{
  const {homeDraftStorage}=await import('../../src/web/home-drafts.js');
  const disk=new Map(),home={read:()=>[...disk],write:(k,v)=>disk.set(k,v)};
  const key='backplane-draft:thread',a=storage([[key,'old Ελληνικά'],['token','private']]);
  const first=homeDraftStorage(a,home);assert.equal(first.getItem(key),'old Ελληνικά');
  const second=homeDraftStorage(storage(),home);assert.equal(second.getItem(key),'old Ελληνικά');assert.equal(second.getItem('token'),null);
  second.setItem(key,'new draft');const third=homeDraftStorage(storage(),home);assert.equal(third.getItem(key),'new draft');
  assert.equal(homeDraftStorage(a,home).getItem(key),'new draft');assert.equal(first.getItem('token'),'private');
});
test('cleared and sent drafts cannot return from an older origin',async()=>{
  const {homeDraftStorage}=await import('../../src/web/home-drafts.js');
  const disk=new Map(),home={read:()=>[...disk],write:(k,v)=>disk.set(k,v)},key='backplane-draft:thread';
  const old=storage([[key,'old text']]),a=homeDraftStorage(old,home);a.setItem(key,'new text');
  homeDraftStorage(storage(),home).removeItem(key);
  const again=homeDraftStorage(old,home);assert.equal(again.getItem(key),null);assert.equal(again.length,0);assert.equal(again.key(0),null);
});
test('ordinary browsers retain their original storage',async()=>{
  const {homeDraftStorage}=await import('../../src/web/home-drafts.js');const local=storage();assert.equal(homeDraftStorage(local,null),local);
});
