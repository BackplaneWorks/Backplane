'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
const source=fs.readFileSync(require.resolve('../../src/web/host.js'),'utf8');
test('a home Send keeps its draft visible while an ordinary thread may show a sending bubble',async()=>{
  const {presentationTarget}=await import('../../src/web/thin-host.js');
  const callback=source.match(/previewSend: (text => [^\n]+),\n/)[1];
  let session={sel:'project-home-p'},calls=0;
  const env={presentationTarget,currentSession:()=>session,App:{presentation_send_preview:()=>{calls++;return true;}}};
  vm.createContext(env);const preview=vm.runInContext('('+callback+')',env);
  assert.equal(preview('Question'),false);assert.equal(calls,0);
  session={sel:'thread-p'};assert.equal(preview('Question'),true);assert.equal(calls,1);
});
function fixture(saved=new Map()) {
  const code=source.slice(source.indexOf('function outboxSave()'),source.indexOf('function waiting('));
  const sent=[],status=[],drafts=new Map(),outbox=new Map(JSON.parse(saved.get('outbox')||'[]').map(o=>[o.msg,o]));
  const env={outbox,OUTBOX:'outbox',OUTBOX_DAY:86400000,DRAFT:'draft:',viewMode:'thin',Date,JSON,
    socket:{readyState:1,send:v=>sent.push(v)},App:{wire_out:v=>v},fromList:v=>v,
    thin:null,currentSession:()=>({sel:'project-home-p',drafts:{}}),showViewStatus:v=>status.push(v),
    localStorage:{setItem:(k,v)=>saved.set(k,v),getItem:k=>saved.get(k)||null,removeItem:k=>saved.delete(k)}};
  vm.createContext(env);vm.runInContext(code,env);return {env,saved,sent,status,outbox,drafts};
}
test('a home Send survives reload and reconnect with the same idempotency key and exact message',()=>{
  const f=fixture(),text='Keep the entire question 日本語.\n'.repeat(2400);
  const request=JSON.stringify({id:42,m:'project.start',p:{project:'p',msg:'stable-message',text}});
  f.env.outboxKeep(request);
  assert.equal(f.outbox.get('stable-message').thread,'project-home-p');
  const reloaded=fixture(f.saved);reloaded.env.outboxFlush();reloaded.env.outboxFlush();
  assert.deepEqual(reloaded.sent,[request,request]);
  assert.equal(JSON.parse(reloaded.sent[0]).p.text,text);
  reloaded.env.outboxReply({id:42,ok:true});
  assert.equal(reloaded.outbox.size,0);assert.equal(reloaded.saved.has('outbox'),false);
});
test('a rejected home Send restores its owner draft while a newer draft remains intact',()=>{
  const f=fixture(),request=JSON.stringify({id:7,m:'project.start',p:{project:'p',msg:'rejected',text:'Original question'}});
  f.env.outboxKeep(request);f.env.outboxReply({id:7,ok:false,error:'unavailable'});
  assert.equal(f.saved.get('draft:project-home-p'),'Original question');
  f.saved.set('draft:project-home-p','New typing');f.env.outboxKeep(request);f.env.outboxReply({id:7,ok:false,error:'unavailable'});
  assert.equal(f.saved.get('draft:project-home-p'),'New typing');
});
test('ordinary thread messages retain their existing outbox behavior and read RPCs are excluded',()=>{
  const f=fixture();f.env.outboxKeep(JSON.stringify({id:1,m:'turn.start',p:{thread:'t',msg:'ordinary',text:'Hello'}}));
  f.env.outboxKeep(JSON.stringify({id:2,m:'project.home',p:{project:'p',msg:'not-a-message'}}));
  assert.equal(f.outbox.size,1);assert.equal(f.outbox.get('ordinary').thread,'t');
});
