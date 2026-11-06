'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),vm=require('node:vm');
async function fixture(supported=false, legacyProbe=false, legacyPaint=false){
 const {ConnectionLog}=await import('../../src/web/connection-log.js');
 const {BubMoveObserver}=await import('../../src/web/bub-move.js');
 const {ViewerRpc}=await import('../../src/web/viewer-rpc.js'),{ReviewPosition}=await import('../../src/web/review-position.js');
 const {ImprovementsRpc}=await import('../../src/web/improvements-rpc.js');
 const {SubsFeedback}=await import('../../src/web/subs-feedback.js');
 const {SecretSaveHost}=await import('../../src/web/secret-save.js');
 const {phoneRequest}=await import('../../src/web/phone-shell.js');
 const {EntryHistory}=await import('../../src/web/entry-history.js');
 const {ThinHost,presentationTarget,preservePresentationRoute,refusedFallback}=await import('../../src/web/thin-host.js'),{Backoff,Retry,directReconnect,DIRECT_TRIES,probeHub,QUIET_MS}=await import('../../src/web/reconnect.js'),{ThinViews,viewCapability,VIEW_LIMITS}=await import('../../src/web/thin.js'),{discoverViews}=await import('../../src/web/thin-discovery.js');
 let source=fs.readFileSync(path.resolve(__dirname,'../../src/web/host.js'),'utf8');
 if(legacyProbe)source=source.replaceAll(' || attempt !== thinAttempt','');
 if(legacyPaint)source=source.replace(' && !detail?.presenting','').replace('state.painted && !becameReady','state.paged && !becameReady');
 const a=source.indexOf('function showViewStatus('),b=source.indexOf('// when a frame (a heartbeat included)',a);assert(a>0&&b>a);
 // Earlier pagehide listeners are unrelated to startup; bound the end after boot.
 const bootStart=source.indexOf('pairingReady.then('),bootEnd=source.indexOf('window.addEventListener("pagehide",',bootStart);assert(bootStart>0&&bootEnd>bootStart);
 const boot=source.slice(bootStart,bootEnd);
 let clock=0,next=0;const timers=new Map(),sockets=[],requests=[],calls=[],status=[],encoder=new TextEncoder(),decoder=new TextDecoder(),store=new Map();
 const setTimer=(fn,ms)=>{const id=++next;timers.set(id,{fn,at:clock+ms});return id;};
 const clearTimer=id=>timers.delete(id);
 class FixtureRetry extends Retry {constructor(){super({setTimer,clearTimer,now:()=>clock});}}
 class Socket{constructor(url){this.url=url;this.readyState=0;sockets.push(this);}open(){this.readyState=1;this.onopen?.();}send(b){requests.push(JSON.parse(decoder.decode(b)));}frame(v){this.onmessage?.({data:encoder.encode(JSON.stringify(v)).buffer});assert(!JSON.parse(store.get('bp.connection.log.v1')||'[]').some(r=>r.cause==='decode-error'),'host callback must not hide a missing fixture dependency as a decode error');}close(){this.readyState=3;this.onclose?.();}}
 const ctx={entryHistory:new EntryHistory(),ConnectionLog,bubMoves:new BubMoveObserver(()=>null),terminalOutput:{clear(){calls.push('terminal-retired');},rebind(){}},refocusKeep(){},bodyWindows:{disconnect(){},bind(){}},sealedReply:()=>false,CONNECTION_BUNDLE:'chunk-fixture.js',ThinHost,ThinViews,viewCapability,discoverViews,WebSocket:Socket,location:{origin:'https://fixture',host:'fixture',protocol:'https:'},
  thin:null,thinSocket:null,thinAttempt:0,thinExpiry:0,thinGreetingTimer:0,thinDraftTimer:0,thinFieldTimer:0,socket:null,viewMode:'discovering',remoteReady:false,remoteEpoch:'',canonicalBoot:false,pairingAccepted:false,
  thinSession:null,frontendNode:null,frontend:null,cid:'fixture',ui:{},heard:0n,thinMailDraft:{clear(){},bind(){},matches:()=>false,edit(){},scope:()=>'',needsSubmit:()=>false},mailReplyField:()=>undefined,focusedView:{invalidate(){},remote(){calls.push('cache-retired');}},desktopBrowser:{hide(){calls.push('hide');}},Solid:{reset(){},isPlot:()=>false},Plot2d:{reset(){}},
  localStorage:{getItem:k=>store.get(k)??null,setItem:(k,v)=>store.set(k,v)},performance:{now:()=>clock},pnow:()=>clock,enterImprovements(){calls.push('improvements-entry');},Date,Number,JSON,String,Math,Uint8Array,Error,BigInt,
  setTimeout:(fn,ms)=>{const id=++next;timers.set(id,{fn,at:clock+ms});return id;},clearTimeout:id=>timers.delete(id),setInterval:()=>++next,clearInterval(){},
  window:{fetch:async(url)=>{calls.push(new URL(url,'https://fixture').pathname);return {status:supported?200:404,headers:new Headers({'content-type':'application/json'}),body:new ReadableStream({start(c){c.enqueue(encoder.encode(JSON.stringify({version:1,views:{version:1,websocketQuery:'view=1',renderers:['web','mobile'],compatibility:'explicit-only'}})));c.close();}})};}},
  checkpointFocused(){calls.push('checkpoint');},pageLang(){},later(){},render(){},trace(){},KEYS:[],WORDS:[],restoreFocused:()=>{calls.push('focused-read');return new Promise(()=>{});},refreshFocusedIdentity:()=>{calls.push('identity');return new Promise(()=>{});},
  currentSession:()=>ctx.thinSession||{sel:'A',drafts:{A:'unsent 日本語'},rid:1},choosePresentation:(s,a,v)=>a==='select'?{...s,sel:v}:{...s,drafts:{...s.drafts,[s.sel]:v}},localPresentation(){},
  presentationJson:x=>x,fromList:x=>x,presentationCommand:c=>calls.push(c),anchorOf:()=>null,
  History:{project:(_before,fresh)=>fresh,project_block:(_before,fresh)=>fresh},historyNode:null,timelineList:()=>null,
  App:{wire_out:t=>encoder.encode(t),wire_out_json:v=>encoder.encode(JSON.stringify(v)),viewer_input:(_u,_info,s)=>s,desk_reset:u=>u,presentation_node:n=>n,presentation_prepare:(s,input)=>({...input,rid:s.rid}),presentation_reply:s=>s,presentation_select:(u,id)=>({...u,sel:id}),presentation_handoff:(u,s)=>({...u,sel:s.sel}),unsync:u=>u,lang:()=> 'en'},
  Wire:{cbor:b=>JSON.parse(decoder.decode(b)),plain:x=>x,json:x=>x},document:{getElementById:()=>null,querySelector:()=>null,querySelectorAll:()=>[],documentElement:{classList:{toggle(){},contains:()=>false},style:{setProperty(){},getPropertyValue:()=>''}}},pairingReady:Promise.resolve(true),pendingInvitation:null,pairingCause:'',pairingUI:{show(){calls.push('pairing-screen');}},
  fallbackIntent:null,wantFrontend:true,Backoff,Retry:FixtureRetry,directReconnect,probeHub,QUIET_MS,navigator:{onLine:true},thinContinuations:{clear(){},accept:()=>false,bind(){},project:x=>x},presentationTarget,preservePresentationRoute,checkDeploy:()=>{},outbox:new Map(),OUTBOX:'outbox',OUTBOX_DAY:86400000,DRAFT:'draft:',
  fabLook(){},restoreTransition(){},transitionNode:null,transitionFor:'',shownFor:'',heldAt:0,transitionScope:()=>({}),transitionPosition:null,previewViews:{enqueueFolds(){},enqueueViewers(){},stop(){}},PHONE:false,VIEWER_OPENS:[],warmDestinations(){},transitionViews:{put(){},position:()=>null,warm:()=>Promise.resolve(false),destination:()=>null},
  thinReading:null,resumedConversation:null,readingIdentity:()=> 'fixture-account',historyDirection:'',
  historyFillMore(){calls.push('history-fill-cleared');},historyPageState(){},historyClear(){},thinSearchAnswer(){},thinSearchFailed(){},showThinMessage(){},keepViewer(){},held:null,
  modelFeedback:{clear(){}},subsFeedback:new SubsFeedback(()=>{}),pressFeedback:{drop(){},seen(){},showing:false,project:p=>p,choice(){}},foldFeedback:{unseen:()=>[],drop(){},seen(){},showing:false,project:p=>p,choice(){}},viewerFeedback:{choice(){}},
  // The extracted socket adapter shares these services with the full host.
  // Record peripheral lifecycle calls; wire admission/ownership still uses real ThinHost/ThinViews.
  RR:null,COARSE_POINTER:false,PHONE:false,phoneRequest,ViewerTools:{reset(){calls.push('viewer-tools-retired');}},viewerRpc:new ViewerRpc(()=>{throw Error('unexpected viewer RPC');}),
  secretSaves:new SecretSaveHost({build:()=>{throw Error('unexpected secret save');},scope:()=>'',binding:()=>'',send(){throw Error('unexpected secret send');},dispatch(){},note(){}}),
  improvementsRpc:new ImprovementsRpc(()=>{throw Error('unexpected Improvements RPC');}),voiceMicHeard(){calls.push('voice-envelope');},
  reviewPosition:new ReviewPosition({getItem:k=>store.get(k)??null,setItem:(k,v)=>store.set(k,v)}),
  composerKeeper:{selecting(){},navigationFailed(){}},keepDesk:s=>s,deskCheck(){},settingsRecovery:null,settingsRecovering:null,
  historyLetGo(){},historyBack(){},historyGone:{older:new Set(),newer:new Set()},historyRegion:()=> 'page',laterRegions(){},
  attachmentIdentityRetry:0,attachmentIdentity:'',attachmentIdentityPending:true,attachmentIdentitySince:0,attachmentIdentityGeneration:0,
  attachmentController:null,attachmentView:null,attachmentSelection:null,
  viewNote:'',viewNoteTimer:0,shownTarget:null,refusedViews:new Set(),refusedFallback,searchJump:null,
  targetKey:t=>t?t.kind+':'+t.id:'',holdPicture(){if(ctx.frontendNode){ctx.transitionNode=ctx.frontendNode;ctx.frontendNode=null;}},
  stopThin:undefined};
 vm.createContext(ctx);
 const ownerStart=source.indexOf('function historyFillOwner()'),ownerEnd=source.indexOf('function historyHasMessage(tl)',ownerStart);
 assert(ownerStart>0&&ownerEnd>ownerStart);vm.runInContext(source.slice(ownerStart,ownerEnd),ctx);
 for(const [start,end] of [['function keepThinReading(','// (the re-render harness'],['function keepSettingsRecovery(','async function recoverSettings('],['function restoreDraftAttachments(','function uploadScope('],['function identityUnverified(','async function refreshFocusedIdentityFor(']]){
  const from=source.indexOf(start),to=source.indexOf(end,from);assert(from>0&&to>from);vm.runInContext(source.slice(from,to),ctx);
 }
 // Top-level names of host.js that the slice below reads, taken from host.js itself.
 const phoneFrom=source.indexOf('const PHONE_TITLES ='),phoneTo=source.indexOf('\n',phoneFrom);assert(phoneFrom>0&&phoneTo>phoneFrom);vm.runInContext(source.slice(phoneFrom,phoneTo),ctx);
 const watchFrom=source.indexOf('let wakeProbe = 0, plotHeard = 0;'),watchTo=source.indexOf('const onWake = why =>',watchFrom);assert(watchFrom>0&&watchTo>watchFrom);vm.runInContext(source.slice(watchFrom,watchTo),ctx);
 vm.runInContext(source.slice(a,b),ctx);ctx.showViewStatus=s=>status.push(s);
 return{ctx,calls,status,requests,sockets,VIEW_LIMITS,DIRECT_TRIES,boot,source,runBoot:async()=>{let started;const start=ctx.startThin;ctx.startThin=()=>started=start();vm.runInContext(boot,ctx);await Promise.resolve();await started;},tick:ms=>{clock+=ms;for(const[id,t]of[...timers])if(t.at<=clock&&timers.has(id)){timers.delete(id);t.fn();}}};
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
 assert(f.calls.includes('viewer-tools-retired'));
 // Drive both the host deadline and reconnect backoff with the fixture clock.
 f.tick(300);await new Promise(setImmediate);
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
 const f=await fixture(true);f.ctx.pairingReady=Promise.resolve(false);await f.runBoot();await new Promise(setImmediate);assert.equal(f.sockets.length,0);assert(!f.calls.includes('focused-read'));assert(!f.calls.includes('/client/capabilities'));assert(!f.calls.includes('FULL-CACHE'));assert.equal(f.ctx.pairingAccepted,false);assert(f.calls.includes('pairing-screen'));
});
test('actual current Mailroom callback binds displayed text and preserves a newer deliberately empty field',async()=>{
 const f=await fixture(true),bound=[];const {mailReplyField}=await import('../../src/web/mail-drafts.js');f.ctx.mailReplyField=mailReplyField;f.ctx.thinMailDraft={clear(){},matches:()=>true,bind:(...args)=>bound.push(args),edit(){},scope:()=>'',needsSubmit:()=>false};
 await f.runBoot();const s=f.sockets[0];s.open();s.frame({t:'view',version:1,renderer:'web',generation:'7',epoch:'actual',limits:{...f.VIEW_LIMITS}});const request=f.requests[0];
 const shown={type:'element',tag:'textarea',key:'mail-reply',attrs:[{name:'data-bfield',value:'mrreply'},{name:'value',value:'captured older reply'}],kids:[]},context={kind:'mail-draft',owner:'bot-A',origin:'actual',conv:'conv-A',recipient:'Room'};
 const response={t:'reply',id:request.id,ok:true,result:{version:1,hub:'https://fixture',generation:'7',epoch:'actual',requestRevision:request.p.requestRevision,target:request.p.target,viewId:'real-view',viewRevision:'1',remoteReady:true,renderer:'web',shown,session:request.p.session,cmds:[],mailDraftContext:context,window:{ids:[],older:'',hasMore:false,anchor:{id:'',offset:0},partial:[]}}};
 s.frame(response);assert.equal(bound.length,1);assert.equal(bound[0][2],'captured older reply');f.ctx.thin.update();f.tick(16);const update=f.requests.at(-1);assert.equal(update.m,'view.update');f.ctx.thin.request('bfield','mrreply\x1f');s.frame({...response,id:update.id,result:{...response.result,viewRevision:'2',requestRevision:update.p.requestRevision}});assert.equal(bound.at(-1)[2],'');
});

// A late probe was able to close a socket opened by an intervening explicit Retry.
test('late retry probe cannot replace a newer connection; previous host path reproduces the extra close',async()=>{
 for(const legacy of [true,false]){
  const f=await fixture(true,legacy);await f.runBoot();f.sockets[0].open();
  const original=f.ctx.window.fetch;let release;
  f.ctx.window.fetch=url=>url==='/hello'?new Promise(resolve=>{release=resolve;}):original(url);
  // A page past its direct tries asks /hello first (directReconnect); that probe is what can come back late.
  vm.runInContext('directFails='+f.DIRECT_TRIES,f.ctx);
  f.sockets[0].close();vm.runInContext('thinRetry.wake()',f.ctx);assert.equal(typeof release,'function');
  await f.ctx.startThin(false,'retry');const newer=f.sockets[1];newer.open();
  release({status:200});await new Promise(setImmediate);
  assert.equal(f.sockets.length,legacy?3:2);assert.equal(newer.readyState,legacy?3:1);
 }
});

// Exercise the socket adapter and the actual region scheduler together. The
// pure Bend landing planner has its own laws/tests; its output is explicit at
// this effect boundary, and we assert the paths the host gives it. No hub,
// compiler, generated bundle, display, or real browser profile is involved.
function regionPainter(f) {
 const c=f.ctx,frames=[],patches=[],landings=[],effects=[],list=xs=>xs.reduceRight((tail,head)=>({$: 'Con',head,tail}),{$:'Nil'});
 const node=n=>n.type==='text'?{$:'Txt',text:n.text}:{$:'El',tag:n.tag,key:n.key,attrs:list(n.attrs.map(a=>({$:a.name?'At':'On',...a}))),kids:list(n.kids.map(node))};
 const is=(v,k)=>v.$===k,each=function*(xs){for(;xs?.$==='Con';xs=xs.tail)yield xs.head;};
 const side={__v:node(regionPage().kids[0])},timeline={__v:node(regionPage().kids[1].kids[1])},top={__v:node(regionPage().kids[1].kids[0])};
 c.document.querySelector=s=>s==='aside.sidebar'?side:s==='header.topbar'?top:null;
 c.document.getElementById=id=>id==='timeline'?timeline:null;
 const scroller={refresh:()=>effects.push('timeline.refresh'),destroy:()=>effects.push('timeline.destroy')};
 Object.assign(c,{is,each,queued:false,requestAnimationFrame:fn=>frames.push(fn),landingPlan:'sidebar ',searchJump:null,jump:null,scroll:false,
  feedbackSession:()=>c.currentSession(),historyGaps:n=>n,pendingPresentation:n=>n,
  same:(a,b)=>a.$===b.$&&a.tag===b.tag&&a.key===b.key,
  patch:(el,v)=>{el.__v=v;patches.push(v.key);},expand:n=>n,root:{},
  bubMoveSync:()=>effects.push('sidebar.moves'),timelineList:()=>scroller,activeTimeline:scroller,
  timelineChanged:l=>{assert.equal(l,scroller);effects.push('timeline.reconcile');},
  historyStatus:null,searchLight:()=>effects.push('timeline.search'),earlierAsked:false,laterAsked:false,
  historyCheckSoon:()=>effects.push('timeline.history'),retireAllowance(){},saveFocusedSoon(){},
  later(){if(c.queued)return;c.queued=true;frames.push(()=>{c.queued=false;effects.push('paint.page');});}});
 c.App.presentation_node=node;c.App.presentation_overlay=n=>{effects.push('overlay:'+n.key);return n;};
 c.bodyWindows.project=n=>{effects.push('body:'+n.key);return n;};c.bodyWindows.observe=()=>effects.push('timeline.observe');
 c.History.landing=(regions,paths)=>{landings.push({regions,paths});return c.landingPlan;};
 const from=f.source.indexOf('const jsonClass ='),to=f.source.indexOf('\nfunction later() {',from);assert(from>0&&to>from);
 vm.runInContext(f.source.slice(from,to),c);
 const drain=()=>{let budget=10;while(frames.length){assert(budget-->0,'paint must settle without a continuous frame loop');frames.shift()();}};
 return{side,timeline,top,frames,patches,effects,landings,drain,reset(){patches.length=effects.length=landings.length=0;}};
}
function regionPage(rows=2){
 const el=(tag,key,attrs,kids=[])=>({type:'element',tag,key,attrs:Object.entries(attrs).map(([name,value])=>({name,value})),kids});
 return el('div','app',{},[
  el('aside','sidebar',{class:'sidebar'},Array.from({length:15},(_,i)=>el('button','thread-'+i,{},[{type:'text',text:'Working '+i}]))),
  el('main','main',{},[el('header','topbar',{class:'topbar'}),el('div','timeline',{id:'timeline'},Array.from({length:rows},(_,i)=>el('div','row-'+i,{},[{type:'text',text:'Message '+i}])))]),
 ]);
}
async function liveRegionFixture(legacy=false,rows=2){
 const f=await fixture(true,false,legacy),paint=regionPainter(f);await f.runBoot();const socket=f.sockets[0];socket.open();
 socket.frame({t:'view',version:1,renderer:'web',generation:'7',epoch:'actual',limits:{...f.VIEW_LIMITS}});
 const req=f.requests.at(-1);
 socket.frame({t:'reply',id:req.id,ok:true,result:{version:1,hub:'https://fixture',generation:'7',epoch:'actual',requestRevision:req.p.requestRevision,target:req.p.target,viewId:'live',viewRevision:'1',renderer:'web',remoteReady:true,
  session:{...req.p.session,rid:1},cmds:[],shown:regionPage(rows),window:{ids:[],older:'',hasMore:false,anchor:{id:'',offset:0},partial:[]},viewDelta:{version:1,base:'',token:String(req.p.requestRevision)}}});
 assert(f.ctx.remoteReady);paint.drain();paint.reset();
 const update=(ops,plan='sidebar ')=>{
  f.ctx.landingPlan=plan;const revision=String(BigInt(f.ctx.thin.client.links.get('https://fixture').viewRevision)+1n);
  socket.frame({t:'view.changed',version:1,generation:'7',epoch:'actual',viewId:'live',target:{kind:'thread',id:'A'},viewRevision:revision});
  f.tick(16);const request=f.requests.at(-1);assert.equal(request.m,'view.update');
  socket.frame({t:'reply',id:request.id,ok:true,result:{version:1,hub:'https://fixture',generation:'7',epoch:'actual',requestRevision:request.p.requestRevision,target:request.p.target,viewId:'live',viewRevision:revision,renderer:'web',remoteReady:true,cmds:[],
   viewDelta:{version:1,base:request.p.viewDelta.token,token:String(request.p.requestRevision),ops}}});
 };
 return{...f,paint,update};
}
test('15 active-thread sidebar updates no longer amplify into full-page or timeline paints',async t=>{
 for(const rows of [2,600]){
  const measured=[];
  for(const legacy of [true,false]){
   const f=await liveRegionFixture(legacy,rows);
   for(let i=0;i<15;i++){
    f.update([['s',['shown','kids',0,'kids',i,'kids',0,'text'],'Tool call '+i]]);
    f.paint.drain();
   }
   const pages=f.paint.effects.filter(x=>x==='paint.page').length;
   measured.push(pages);
   if(!legacy){
    assert.deepEqual(f.paint.patches,Array(15).fill('sidebar'));
    assert(!f.paint.effects.some(x=>x.includes('timeline')),'sidebar paint must not project, refresh, observe or search the timeline');
    assert.equal(f.paint.effects.filter(x=>x==='sidebar.moves').length,15,'sidebar lease display still updates');
    assert(f.paint.landings.every(x=>x.regions.includes('sidebar\tshown\tkids\t0')&&x.paths.startsWith('shown\tkids\t0\t')));
    assert.equal(f.ctx.thin.session.drafts.A,'unsent 日本語','background activity keeps the local draft');
   }
  }
  assert.deepEqual(measured,[15,0]);t.diagnostic(`${rows} selected rows: full-page paint requests 15 -> 0; sidebar patches 15; timeline effects 0`);
 }
});
test('live region paints coalesce against the latest accepted picture and retain timeline lifecycle work',async()=>{
 const f=await liveRegionFixture();
 f.update([['s',['shown','kids',0,'kids',0,'kids',0,'text'],'background change']]);
 f.update([['s',['shown','kids',1,'kids',1,'kids',0,'kids',0,'text'],'new selected message']], 'timeline ');
 assert.equal(f.paint.frames.length,1,'one two-frame region paint for both replies');f.paint.drain();
 assert.deepEqual(f.paint.patches,['sidebar','timeline']);assert(!f.paint.effects.includes('paint.page'));
 assert(f.paint.effects.includes('timeline.refresh'));assert(f.paint.effects.includes('timeline.observe'));assert(f.paint.effects.includes('timeline.history'));
 assert.equal(f.paint.timeline.__v.kids.head.kids.head.text,'new selected message');
});
test('whole replies, session changes and missing region DOM retain the full-page fallback',async()=>{
 const f=await liveRegionFixture();
 assert.equal(f.ctx.historyRegion({shown:regionPage(),changed:null},'update'),'page');
 assert.equal(f.ctx.historyRegion({shown:regionPage(),changed:[[]]},'update'),'page','a root replacement must paint even though its tab-separated path is empty');
 assert.equal(f.ctx.historyRegion({shown:regionPage(),changed:[]},'reset'),'page');
 assert.equal(f.ctx.historyRegion({shown:regionPage(),changed:[],historyReset:true},'update'),'page');
 assert.equal(f.ctx.historyRegion({shown:regionPage(),changed:[],position:{atEnd:true}},'update'),'page');
 f.ctx.searchJump={id:'search-result'};assert.equal(f.ctx.historyRegion({shown:regionPage(),changed:[]},'update'),'page');f.ctx.searchJump=null;
 f.ctx.scroll=true;assert.equal(f.ctx.historyRegion({shown:regionPage(),changed:[]},'update'),'page','ScrollEnd commands keep the full render that consumes them');f.ctx.scroll=false;
 f.update([['s',['session','open'],{settings:true}]],'page');f.paint.drain();assert(f.paint.effects.includes('paint.page'));
 assert.equal(f.paint.landings.at(-1).paths,'session\topen');f.paint.reset();
 f.paint.side.__v=null;f.update([['s',['shown','kids',0,'kids',0,'kids',0,'text'],'changed']]);f.paint.drain();
 assert(f.paint.effects.includes('paint.page'));assert.equal(f.paint.patches.length,0,'incompatible DOM cannot get a partial patch');
});
test('a pill-only timeline region reconciles unread even when rows and geometry do not change',async()=>{
 const f=await liveRegionFixture();
 // refresh deliberately emits no plan/scroll callback. Only the accepted
 // timeline picture changes; virtual rows and scroller geometry stay equal.
 f.update([['s',['shown','kids',1,'kids',1,'attrs'],[{name:'id',value:'timeline'},{name:'data-unread-pill',value:'1'}]]],'timeline ');
 f.paint.drain();
 assert.deepEqual(f.paint.patches,['timeline']);
 assert.equal(f.paint.effects.filter(x=>x==='timeline.refresh').length,1);
 assert.equal(f.paint.effects.filter(x=>x==='timeline.reconcile').length,1,'accepted region must run the same bottom reconciliation as a full paint');
 assert(!f.paint.effects.includes('paint.page'));
 assert(!f.paint.effects.includes('sidebar.moves'));
});
test('a queued full paint covers a live region update without an additional paint',async()=>{
 const f=await liveRegionFixture();f.ctx.later();
 f.update([['s',['shown','kids',0,'kids',0,'kids',0,'text'],'covered']]);f.paint.drain();
 assert.deepEqual(f.paint.effects,['paint.page']);assert.equal(f.paint.patches.length,0);
});
