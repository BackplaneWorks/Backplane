'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),vm=require('node:vm');
async function fixture(supported=false){
 const {ThinHost,presentationTarget,preservePresentationRoute}=await import('../../src/web/thin-host.js'),{Backoff,Retry,probeHub,QUIET_MS}=await import('../../src/web/reconnect.js'),{ThinViews,viewCapability,VIEW_LIMITS}=await import('../../src/web/thin.js'),{discoverViews}=await import('../../src/web/thin-discovery.js');
 const source=fs.readFileSync(path.resolve(__dirname,'../../src/web/host.js'),'utf8');
 const a=source.indexOf('function showViewStatus('),b=source.indexOf('// when a frame (a heartbeat included)',a);assert(a>0&&b>a);
 const boot=source.slice(source.indexOf('pairingReady.then('),source.indexOf('window.addEventListener("pagehide", checkpointFocused)'));
 let clock=0,next=0;const timers=new Map(),sockets=[],requests=[],calls=[],status=[],encoder=new TextEncoder(),decoder=new TextDecoder(),store=new Map();
 class Socket{constructor(url){this.url=url;this.readyState=0;sockets.push(this);}open(){this.readyState=1;this.onopen?.();}send(b){requests.push(JSON.parse(decoder.decode(b)));}frame(v){this.onmessage?.({data:encoder.encode(JSON.stringify(v)).buffer});}close(){this.readyState=3;this.onclose?.();}}
 const ctx={ThinHost,ThinViews,viewCapability,discoverViews,WebSocket:Socket,location:{origin:'https://fixture',host:'fixture',protocol:'https:'},
  thin:null,thinSocket:null,thinAttempt:0,thinExpiry:0,thinGreetingTimer:0,thinDraftTimer:0,thinFieldTimer:0,socket:null,viewMode:'discovering',remoteReady:false,remoteEpoch:'',canonicalBoot:false,pairingAccepted:false,
  thinSession:null,frontendNode:null,frontend:null,cid:'fixture',ui:{},heard:0n,thinMailDraft:{clear(){},bind(){},matches:()=>false,edit(){},scope:()=>'',needsSubmit:()=>false},mailReplyField:()=>undefined,focusedView:{invalidate(){},remote(){calls.push('cache-retired');}},desktopBrowser:{hide(){calls.push('hide');}},Solid:{reset(){},isPlot:()=>false},Plot2d:{reset(){}},
  localStorage:{getItem:k=>store.get(k)??null,setItem:(k,v)=>store.set(k,v)},performance:{now:()=>clock},Date,Number,JSON,String,Math,Uint8Array,Error,BigInt,
  setTimeout:(fn,ms)=>{const id=++next;timers.set(id,{fn,at:clock+ms});return id;},clearTimeout:id=>timers.delete(id),setInterval:()=>++next,clearInterval(){},
  window:{fetch:async(url)=>{calls.push(new URL(url,'https://fixture').pathname);return {status:supported?200:404,headers:new Headers({'content-type':'application/json'}),body:new ReadableStream({start(c){c.enqueue(encoder.encode(JSON.stringify({version:1,views:{version:1,websocketQuery:'view=1',renderers:['web','mobile'],compatibility:'explicit-only'}})));c.close();}})};}},
  checkpointFocused(){calls.push('checkpoint');},later(){},render(){},trace(){},KEYS:[],WORDS:[],restoreFocused:()=>{calls.push('focused-read');return new Promise(()=>{});},refreshFocusedIdentity:()=>{calls.push('identity');return new Promise(()=>{});},
  currentSession:()=>ctx.thinSession||{sel:'A',drafts:{A:'unsent 日本語'},rid:1},choosePresentation:(s,a,v)=>a==='select'?{...s,sel:v}:{...s,drafts:{...s.drafts,[s.sel]:v}},localPresentation(){},
  presentationJson:x=>x,fromList:x=>x,presentationCommand:c=>calls.push(c),anchorOf:()=>null,
  App:{wire_out:t=>encoder.encode(t),presentation_node:n=>n,presentation_prepare:(s,input)=>({...input,rid:s.rid}),presentation_reply:s=>s,presentation_select:(u,id)=>({...u,sel:id}),presentation_handoff:(u,s)=>({...u,sel:s.sel}),unsync:u=>u,lang:()=> 'en'},
  Wire:{cbor:b=>JSON.parse(decoder.decode(b)),plain:x=>x},document:{getElementById:()=>null,documentElement:{style:{setProperty(){}}}},pairingReady:Promise.resolve(true),
  fallbackIntent:null,wantFrontend:true,Backoff,Retry,probeHub,QUIET_MS,navigator:{onLine:true},thinContinuations:{clear(){},accept:()=>false,bind(){},project:x=>x},presentationTarget,preservePresentationRoute,checkDeploy:()=>{},outbox:new Map(),OUTBOX:'outbox',OUTBOX_DAY:86400000,DRAFT:'draft:',
  stopThin:undefined};
 vm.createContext(ctx);vm.runInContext(source.slice(a,b),ctx);ctx.showViewStatus=s=>status.push(s);
 return{ctx,calls,status,requests,sockets,VIEW_LIMITS,boot,runBoot:async()=>{let started;const start=ctx.startThin;ctx.startThin=()=>started=start();vm.runInContext(boot,ctx);await Promise.resolve();await started;},tick:ms=>{clock+=ms;for(const[id,t]of[...timers])if(t.at<=clock&&timers.has(id)){timers.delete(id);t.fn();}}};
}
test('actual startup begins capability discovery despite a permanently pending focused IDB lookup',async()=>{
 const f=await fixture(false);await f.runBoot();assert(f.calls.includes('focused-read'));assert(f.calls.includes('/client/capabilities'));assert.equal(f.ctx.viewMode,'read-only');
 assert(!f.calls.includes('FULL-CACHE'));assert(!f.calls.includes('LEGACY-SOCKET'));assert.equal(f.sockets.length,0);assert.equal(f.ctx.canonicalBoot,false);
});
test('actual supported startup opens only view=1 and a real greeting requests only the selected bounded view',async()=>{
 const f=await fixture(true);await f.runBoot();assert.equal(f.sockets.length,1);const s=f.sockets[0],url=new URL(s.url);assert.equal(url.searchParams.get('view'),'1');assert.equal(url.searchParams.get('since'),'0');assert.equal(url.searchParams.get('origin'),'');assert(!url.searchParams.has('token'));
 s.open();s.frame({t:'view',version:1,renderer:'web',generation:'7',epoch:'actual',limits:{...f.VIEW_LIMITS}});assert.equal(f.requests.length,1);assert.equal(f.requests[0].m,'view.open');assert.equal(f.ctx.remoteReady,false);assert.equal(f.ctx.canonicalBoot,false);
 s.frame({t:'log',seq:999,complete:true});assert.equal(f.ctx.canonicalBoot,false);assert.equal(f.requests.length,1);assert(!f.calls.includes('FULL-CACHE'));
});
test('actual missing greeting closes boundedly without canonical hydration or history fallback',async()=>{
 const f=await fixture(true);await f.runBoot();f.sockets[0].open();f.tick(4000);assert.equal(f.sockets[0].readyState,3);assert.equal(f.ctx.remoteReady,false);assert.equal(f.ctx.canonicalBoot,false);assert.equal(f.requests.length,0);assert(!f.calls.includes('FULL-CACHE'));assert(!f.calls.includes('LEGACY-SOCKET'));
});
test('actual stalled WebSocket connection is bounded even before its open event',async()=>{
 const f=await fixture(true);await f.runBoot();f.tick(4000);assert.equal(f.sockets[0].readyState,3);assert.equal(f.requests.length,0);assert.equal(f.ctx.remoteReady,false);assert(!f.calls.includes('FULL-CACHE'));assert(!f.calls.includes('LEGACY-SOCKET'));
});
test('a missing or failed focused read does not change discovery failure into full replay',async()=>{
 for(const result of [null,{error:'quota or corrupt view'}]){
  const f=await fixture(false);f.ctx.restoreFocused=()=>Promise.resolve(result);await f.runBoot();assert.equal(f.sockets.length,0);assert.equal(f.ctx.canonicalBoot,false);assert(!f.calls.includes('FULL-CACHE'));assert(!f.calls.includes('LEGACY-SOCKET'));
 }
});
test('a dropped view reconnects by itself, bounded, with no whole-log path',async()=>{
 const f=await fixture(true);await f.runBoot();const s=f.sockets[0];s.open();s.close();assert.equal(f.ctx.remoteReady,false);
 // the retry waits out reconnect.js's real-time backoff (125-250 ms first)
 for(let i=0;i<20&&f.sockets.length<2;i++){f.tick(300);await new Promise(r=>setTimeout(r,100));}
 assert.equal(f.sockets.length,2,'a second view socket after the backoff');const url=new URL(f.sockets[1].url);assert.equal(url.searchParams.get('view'),'1');assert.equal(url.searchParams.get('since'),'0');
 assert(!f.calls.includes('FULL-CACHE'));assert(!f.calls.includes('LEGACY-SOCKET'));
});
test('actual matching bounded view alone sets remoteReady while canonical readiness remains false',async()=>{
 const f=await fixture(true);await f.runBoot();const s=f.sockets[0];s.open();s.frame({t:'view',version:1,renderer:'web',generation:'7',epoch:'actual',limits:{...f.VIEW_LIMITS}});const request=f.requests[0];
 s.frame({t:'reply',id:request.id,ok:true,result:{version:1,hub:'https://fixture',generation:'7',epoch:'actual',requestRevision:request.p.requestRevision,target:request.p.target,
  viewId:'real-view',viewRevision:'1',remoteReady:true,renderer:'web',shown:{type:'text',text:'ACTUAL CURRENT CONTENT'},session:request.p.session,cmds:[],window:{ids:['row'],older:'',hasMore:false,anchor:{id:'row',offset:0},partial:[]}}});
 assert.equal(f.ctx.remoteReady,true);assert.equal(f.ctx.frontendNode.text,'ACTUAL CURRENT CONTENT');assert.equal(f.ctx.canonicalBoot,false);assert(!f.calls.includes('FULL-CACHE'));assert(!f.requests.some(r=>r.m.includes('history')));
});
test('wrong-owner view result cannot ready the host or cause canonical fallback',async()=>{
 const f=await fixture(true);await f.runBoot();const s=f.sockets[0];s.open();s.frame({t:'view',version:1,renderer:'web',generation:'7',epoch:'actual',limits:{...f.VIEW_LIMITS}});const request=f.requests[0];
 s.frame({t:'reply',id:request.id,ok:true,result:{version:1,hub:'https://FOREIGN',generation:'7',epoch:'actual',requestRevision:request.p.requestRevision,target:request.p.target,viewId:'v',viewRevision:'1',remoteReady:true,renderer:'web',cmds:[]}});
 assert.equal(f.ctx.remoteReady,false);assert.equal(f.ctx.canonicalBoot,false);assert.equal(f.ctx.frontendNode,null);assert(!f.calls.includes('FULL-CACHE'));assert(!f.requests.some(r=>r.m.includes('history')));
});
test('failed explicit pairing cannot restore a previous profile or open any legacy socket',async()=>{
 const f=await fixture(true);f.ctx.pairingReady=Promise.resolve(false);await f.runBoot();assert.equal(f.sockets.length,0);assert(!f.calls.includes('focused-read'));assert(!f.calls.includes('/client/capabilities'));assert(!f.calls.includes('FULL-CACHE'));assert.equal(f.ctx.pairingAccepted,false);
});
test('actual current Mailroom callback binds displayed text and preserves a newer deliberately empty field',async()=>{
 const f=await fixture(true),bound=[];const {mailReplyField}=await import('../../src/web/mail-drafts.js');f.ctx.mailReplyField=mailReplyField;f.ctx.thinMailDraft={clear(){},matches:()=>true,bind:(...args)=>bound.push(args),edit(){},scope:()=>'',needsSubmit:()=>false};
 await f.runBoot();const s=f.sockets[0];s.open();s.frame({t:'view',version:1,renderer:'web',generation:'7',epoch:'actual',limits:{...f.VIEW_LIMITS}});const request=f.requests[0];
 const shown={type:'element',tag:'textarea',key:'mail-reply',attrs:[{name:'data-bfield',value:'mrreply'},{name:'value',value:'captured older reply'}],kids:[]},context={kind:'mail-draft',owner:'bot-A',origin:'actual',conv:'conv-A',recipient:'Room'};
 const response={t:'reply',id:request.id,ok:true,result:{version:1,hub:'https://fixture',generation:'7',epoch:'actual',requestRevision:request.p.requestRevision,target:request.p.target,viewId:'real-view',viewRevision:'1',remoteReady:true,renderer:'web',shown,session:request.p.session,cmds:[],mailDraftContext:context,window:{ids:[],older:'',hasMore:false,anchor:{id:'',offset:0},partial:[]}}};
 s.frame(response);assert.equal(bound.length,1);assert.equal(bound[0][2],'captured older reply');f.ctx.thin.request('bfield','mrreply\x1f');f.ctx.thin.update();const update=f.requests.at(-1);s.frame({...response,id:update.id,result:{...response.result,requestRevision:update.p.requestRevision}});assert.equal(bound.at(-1)[2],'');
});
