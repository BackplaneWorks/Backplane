'use strict';
// src/web/app-shell.js and mobile/shell/backplane-app.js: the phone apps' bridge (docs/phone-app-shell.md)
const {test}=require('node:test'),assert=require('node:assert/strict');
const fs=require('node:fs'),path=require('node:path'),vm=require('node:vm');
const load=()=>import('../../src/web/app-shell.js');
const SHIM=fs.readFileSync(path.join(__dirname,'../../mobile/shell/backplane-app.js'),'utf8');

// the shim in a page whose app is Android (BackplaneNative) or iOS (webkit.messageHandlers)
function page(kind,info){
  const posted=[];
  const win={location:{href:'http://hub:3787/?phone=1'},setTimeout:(f)=>f(),JSON,Promise,Object,Error,Array,String};
  if (kind==='android') win.BackplaneNative={postMessage:t=>posted.push(JSON.parse(t)),onmessage:null};
  else win.webkit={messageHandlers:{backplane:{postMessage:t=>posted.push(JSON.parse(t))}}};
  win.window=win;
  vm.runInNewContext(SHIM.replace('/*@info*/null',JSON.stringify(info)),win);
  const answer=(m)=>kind==='android'?win.BackplaneNative.onmessage({data:JSON.stringify(m)}):win.backplaneApp._receive(m);
  return {win,posted,answer};
}

test('the shim says hello and carries the app\'s facts, the same on both platforms',()=>{
  for (const kind of ['android','ios']){
    const {win,posted}=page(kind,{platform:kind,version:'1.2.3',features:['camera','share'],hub:{key:'hub:3787',name:'desk'}});
    const app=win.backplaneApp;
    assert.equal(app.bridge,1);assert.equal(app.platform,kind);assert.equal(app.version,'1.2.3');
    assert.deepEqual([...app.features],['camera','share']);assert.equal(app.hub.key,'hub:3787');
    assert.equal(posted[0].op,'hello');
    assert.throws(()=>{'use strict';win.backplaneApp=null;});
  }
});

test('a question is answered by its id; an event before its listener is kept for it',async()=>{
  for (const kind of ['android','ios']){
    const {win,posted,answer}=page(kind,{});
    const app=win.backplaneApp;
    const asked=app.requestCamera();
    const q=posted.find(m=>m.op==='requestCamera');
    assert.ok(q.id>0);
    answer({id:q.id,ok:true,value:true});
    assert.equal(await asked,true);
    const shared=app.share({text:'hi'});
    answer({id:posted.at(-1).id,ok:false,error:'no'});
    assert.equal(await shared,false,'a refused share answers false');
    answer({event:'open-thread',data:{thread:'t1'}});
    const got=[];app.on('open-thread',d=>got.push(d.thread));
    answer({event:'open-thread',data:{thread:'t2'}});
    assert.deepEqual(got,['t1','t2']);
    app.setState({thread:'t2'});
    assert.deepEqual(posted.at(-1),{op:'setState',args:{thread:'t2'}});
  }
});

test('no app, no bridge: the page runs as in a browser',async()=>{
  const {appBridge,installAppShell}=await load();
  assert.equal(appBridge({}),null);
  const s=installAppShell({win:{},doc:{},dispatch:()=>{},selected:()=>''});
  assert.equal(s.app,null);s.rendered();
});

test('the state told: the thread only while a thread or its viewer is on screen',async()=>{
  const {shellState}=await load();
  const doc=(screen,light,badge)=>({
    querySelector:(sel)=>sel==='.app.phone'?{getAttribute:()=>screen}:sel.includes('ph-badge')?(badge==null?null:{textContent:badge}):null,
    documentElement:{classList:{contains:c=>c==='light'&&light}},
    defaultView:{getComputedStyle:()=>({getPropertyValue:()=>' #0d0f12 '})},
  });
  assert.deepEqual(shellState(doc('thread',false,'2'),'t1'),{thread:'t1',needs:2,theme:'dark',bg:'#0d0f12'});
  assert.equal(shellState(doc('viewer',false,null),'t1').thread,'t1');
  assert.equal(shellState(doc('now',true,null),'t1').thread,'');
  assert.equal(shellState(doc('now',true,null),'t1').theme,'light');
  assert.equal('needs' in shellState(doc('thread',false,null),'t1'),false,'no tab bar on screen: the count is not known');
});

test('a tapped notification opens its thread; the state goes to the app once per change',async()=>{
  const {installAppShell}=await load();
  const told=[],listeners={},dispatched=[];
  const app={bridge:1,features:['settings'],on:(t,f)=>{listeners[t]=f;},setState:s=>told.push(s),openAppSettings:()=>told.push('settings'),pairingFailed:()=>{}};
  const cls=new Set();
  const doc={documentElement:{classList:{add:c=>cls.add(c),contains:()=>false}},addEventListener:()=>{},
    querySelector:(sel)=>sel==='.app.phone'?{getAttribute:()=> 'thread'}:null,defaultView:{getComputedStyle:()=>({getPropertyValue:()=>''})}};
  const s=installAppShell({win:{backplaneApp:app},doc,dispatch:(a,v)=>dispatched.push([a,v]),selected:()=>'t9'});
  assert.ok(cls.has('ph-in-app'));
  listeners['open-thread']({thread:'t3'});
  assert.deepEqual(dispatched,[['select','t3']]);
  s.rendered();s.rendered();
  assert.deepEqual(told,[{thread:'t9',theme:'dark'}]);
});
