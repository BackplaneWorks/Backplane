const {test}=require('node:test'),assert=require('node:assert/strict');
const {pathToFileURL}=require('node:url');const path=require('node:path');
const load=async name=>import(pathToFileURL(path.resolve(__dirname,'../../src/web/'+name)).href);
test('shortcut preserves DevTools, IME, repeat, AltGraph and normal edits',async()=>{
 const {improvementsShortcut:hit,improvementsChord}=await load('improvements.js');
 const e={key:'i',code:'KeyI',ctrlKey:true,altKey:true};assert(hit(e,'Linux'));assert(!hit(e,'MacIntel'));
 assert(hit({...e,altKey:false,metaKey:true},'MacIntel'));
 for(const patch of [{isComposing:true},{keyCode:229},{repeat:true},{defaultPrevented:true},{shiftKey:true},{getModifierState:()=>true},{ctrlKey:false},{metaKey:true}])assert(!hit({...e,...patch},'Linux'));
 assert(!hit({key:'i',metaKey:true,altKey:true},'MacIntel'));assert(!hit({key:'i',metaKey:true,shiftKey:true},'MacIntel'));
 assert.equal(improvementsChord('MacIntel'),'Ctrl+Cmd+I');assert.equal(improvementsChord('Win32'),'Ctrl+Alt+I');
});
test('popup clamps every edge including a viewport moved by a phone keyboard',async()=>{
 const {clampPopup}=await load('improvements.js');
 assert.deepEqual(clampPopup(-200,3000,500,400,{x:0,y:0,width:1000,height:700}),{x:8,y:292});
 assert.deepEqual(clampPopup(900,-100,300,200,{x:0,y:250,width:390,height:400}),{x:82,y:258});
});
test('one stable creation and named turn retry survive upload/start lost replies; exact request and normal attachment',async()=>{
 const {ImprovementsFlow,improvementMessage}=await load('improvements-flow.js');const calls=[],saved=[],selected=[];
 const key='0123456789abcdef0123456789abcdef',thread='improvement-'+key,text='  Change this\n日本語 <tag> & exact trailing spaces  ';
 const draft={key,project:'p',text,image:new Blob(['png'],{type:'image/png'}),context:{app:'Backplane',project:'p',view:'Thread'}};
 let uploads=0,fail=true;
 const flow=new ImprovementsFlow({rpc:{async request(m,p){calls.push({m,p});if(m==='thread.create')return{thread};if(m==='improvements.context')return{thread,accountBinding:'named-account-generation-2'};if(fail){fail=false;throw Error('lost reply');}return{ok:true};}},
 upload:async()=>{uploads++;return'/home/test/attachments/'+thread+'/a.png';},save:async d=>saved.push({...d}),select:async t=>selected.push(t)});
 await assert.rejects(flow.send(draft),/lost reply/);assert.equal(draft.text,text);assert(draft.image);assert(!flow.busy);assert.equal(selected.length,0);
 assert(await flow.send(draft));assert.equal(uploads,1);assert.equal(selected[0],thread);
 assert.deepEqual(calls.filter(c=>c.m==='thread.create').map(c=>c.p),[{project:'p',title:'New thread',creationKey:key},{project:'p',title:'New thread',creationKey:key}]);
 const turns=calls.filter(c=>c.m==='turn.start');assert.deepEqual(turns[0].p,turns[1].p);assert(turns[0].p.text.startsWith(text+'\n\n'));assert(turns[0].p.text.endsWith('[attachment: '+draft.path+']'));
 assert.equal(turns[0].p.accountBinding,'named-account-generation-2');assert.equal(calls.filter(c=>c.m==='improvements.context').length,1);assert.equal(turns[0].p.msg,'improvement-request-'+key);assert(!JSON.stringify(calls).includes('yolo'));assert(saved.at(-1).accepted);
 assert.equal(improvementMessage(text,{...draft.context,targetProject:draft.project},draft.path),turns[0].p.text);
});
test('double click is one dispatch; failed upload retains screenshot and never starts a turn',async()=>{
 const {ImprovementsFlow}=await load('improvements-flow.js');let release,calls=0;const draft={key:'a'.repeat(32),project:'p',text:'request',image:new Blob(['png']),context:{}};
 const flow=new ImprovementsFlow({rpc:{request(m,p){calls++;if(m==='improvements.context')return Promise.resolve({thread:p.sourceThread,accountBinding:''});return new Promise(r=>release=r);}},upload:async()=>{throw Error('upload failed');},save:async()=>{},select:async()=>assert.fail('no select')});
 const first=flow.send(draft);await new Promise(r=>setImmediate(r));assert.equal(await flow.send(draft),false);release({thread:'improvement-'+draft.key});
 await assert.rejects(first,/upload failed/);assert.equal(calls,2);assert.equal(draft.text,'request');assert(draft.image);assert(!flow.busy);
});
test('RPC times out, consumes only its replies, and disconnect releases pending sends',async()=>{
 const {ImprovementsRpc}=await load('improvements-rpc.js');const sent=[],rpc=new ImprovementsRpc(o=>sent.push(o),10);
 const p=rpc.request('thread.create',{});assert(!rpc.receive({t:'reply',id:1}));assert(rpc.receive({t:'reply',id:sent[0].id,ok:true,thread:'made'}));assert.equal((await p).thread,'made');
 await assert.rejects(rpc.request('turn.start',{}),/did not answer/);const pending=rpc.request('turn.start',{});rpc.reset();await assert.rejects(pending,/Connection changed/);assert.equal(rpc.pending.size,0);
});
test('native capture only asks our own window and bounds the result',async()=>{
 const {captureAppWindow}=require('../improvements-capture.cjs');let calls=0;
 const window={isDestroyed:()=>false,capturePage:async()=>{calls++;return{isEmpty:()=>false,getSize:()=>({width:800,height:600}),toPNG:()=>Buffer.from('png')};}};
 assert.equal(await captureAppWindow(window),'data:image/png;base64,cG5n');assert.equal(calls,1);await assert.rejects(captureAppWindow({isDestroyed:()=>true}),/unavailable/);
});
test('web capture refuses other windows and tabs; every rejected or late stream is stopped',async()=>{
 const {captureApp}=await load('improvements-capture.js');
 for(const surface of ['monitor','window','browser']){
  let stopped=0,handle;const track={getSettings:()=>({displaySurface:surface}),getCaptureHandle:()=>({handle:'other-page'}),stop:()=>stopped++};
  const media={setCaptureHandleConfig(c){handle=c.handle;},getDisplayMedia:async constraints=>{assert.equal(constraints.monitorTypeSurfaces,'exclude');assert.equal(constraints.audio,false);return{getVideoTracks:()=>[track],getTracks:()=>[track]};}};
  await assert.rejects(captureApp({bridge:null,media,win:{location:{origin:'https://hub'}},doc:{}}),/Choose this Backplane tab/);assert.equal(stopped,1);assert(handle);
 }
 let release,stopped=0;const controller=new AbortController();
 const capture=captureApp({bridge:null,media:{setCaptureHandleConfig(){},getDisplayMedia:()=>new Promise(r=>release=r)},win:{location:{origin:'https://hub'}},doc:{},signal:controller.signal});
 controller.abort();release({getTracks:()=>[{stop:()=>stopped++}]});await assert.rejects(capture,/canceled/);assert.equal(stopped,1);
});
