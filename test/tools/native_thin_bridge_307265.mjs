// Source-adapter controls only. No Bend emission, app build, service or UI proof.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
// Hermetic by default: without the frozen review packet (build/phone-cache), check the repo's own sources.
if(!process.env.BP_NATIVE_THIN_SOURCE_ROOT&&!fs.existsSync(path.join(process.cwd(),'build/phone-cache/native-thin-bridge-proposal-01')))process.env.BP_NATIVE_THIN_SOURCE_ROOT=process.cwd();
const root=process.cwd(), packet=path.join(root,'build/phone-cache/native-thin-bridge-proposal-01');
const stage=fs.mkdtempSync(path.join(os.tmpdir(),'bp-native-thin-js-307265-'));
for(const n of ['src/web/bub-move.js','src/web/thin.js','src/web/thin-results.js','src/web/startup.js','src/web/mail-drafts.js','src/web/terminal-input.js','src/web/model-menu-input.js','src/web/sidebar-region.js']) {const q=path.join(stage,n);fs.mkdirSync(path.dirname(q),{recursive:true});fs.copyFileSync((process.env.BP_NATIVE_THIN_SOURCE_ROOT ? path.join(process.env.BP_NATIVE_THIN_SOURCE_ROOT,n) : path.join(packet,fs.existsSync(path.join(packet,'qualified-dependencies'))?'qualified-dependencies':'dependencies',n)),q);}
fs.mkdirSync(path.join(stage,'src/mobile'),{recursive:true});
fs.copyFileSync(process.env.BP_NATIVE_THIN_HELPER_SOURCE||path.join(root,'src/mobile/thin-native-host-307265.js'),path.join(stage,'src/mobile/thin-native-host-307265.js'));
fs.writeFileSync(path.join(stage,'package.json'),'{"type":"module"}');
const {NativeThinHost307265:Host,validateNativeSidebar307265:validateSidebar,validateNativeSidebarAction307265:validateSidebarAction}=await import(pathToFileURL(path.join(stage,'src/mobile/thin-native-host-307265.js')));
const {VIEW_LIMITS,VIEW_ID_BASE}=await import(pathToFileURL(path.join(stage,'src/web/thin.js')));
const clone=x=>JSON.parse(JSON.stringify(x));
const tests=[],check=(name,fn)=>tests.push([name,fn]);
function harness(){
 let now=1000;const calls={command:0,search:[],message:0};
 const base=()=>({sel:'a',drafts:{},open:{},adding:false,panel:false,info:{},rid:1});
 const target=s=>s.route?{kind:'bot',id:s.route.bot}:s.sel?{kind:'thread',id:s.sel}:{kind:'recent',id:''};
 const api={now:()=>now,cid:()=> 'install',renderer:()=> 'ios-v1',session:base,target,
  intent:(hub,s,t,a,v)=>{
   if(a==='select'){s.sel=v.startsWith(hub+'|')?v.slice(hub.length+1):v;delete s.route;return{kind:'select',session:s,target:target(s)};}
   if(a==='draft'){s.drafts={...s.drafts,[s.sel]:v};return{kind:'draft',session:s};}
   if(a==='earlier')return{kind:'page'};
   if(a==='thin-search')return{kind:'search',args:JSON.parse(v)};
   if(a==='thin-message-more')return{kind:'message',args:JSON.parse(v)};
   if(a==='thin-cancel')return{kind:'cancel',args:JSON.parse(v)};
   if(a==='bot-tab')s.route={kind:'bot-tab',bot:t.id,tab:v};
   return{kind:'action',session:s};
  },
  prepare:(s,input)=>({...clone(input),rid:s.rid}),
  ack:(s,e)=>e.clear?{...s,drafts:{...s.drafts,[e.clear]:''}}:s,
  reconcile:(s,w)=>({...s,sel:w.sel,open:w.open,info:w.info,...(w.route?{route:w.route}:{})}),
  send:(hub,request)=>({type:'send',hub,request}),
  command:(hub,c)=>{calls.command++;return{...c,...(c.type==='keep'?{thread:hub+'|'+c.thread}:{})};},
  materialize:(r,s)=>({shown:{...clone(r.shown),draft:s.drafts[s.sel]||''},overlay:clone(r.overlay)}),
  message:(view,m)=>{calls.message++;if(view.shown.thread.id!=='h|'+m.thread||view.shown.thread.entries.find(e=>e.id===m.id)?.partial?.version!==m.version)throw Error('stale');return{...view,shown:{...view.shown,chunk:m}};},
  search:(h,s)=>calls.search.push([h,s]),
 };
 const h=new Host(api,VIEW_ID_BASE+100);
 function greeting(hub='h',epoch='1'){h.receive(hub,{t:'view',version:1,renderer:'mobile',generation:'1',epoch:'origin',limits:VIEW_LIMITS},epoch);return h.drain();}
 function screen(hub='h',sel='a'){return{hub,sel:hub+'|'+sel,online:true,version:'test',error:'',note:'',empty:'',hubs:[],found:[],projects:[],bots:[],rooms:[],thread:{id:sel,entries:[{id:'e',kind:'assistant',text:'current',blocks:[]}]}};}
 function answer(id,overrides={},hub='h'){
  const call=h.client.calls.get(id);assert(call,'test owns actual request');const p=call.input;
  const s=clone(p.session||h.states.get(hub).session),shown=screen(hub,s.sel);
  const result={version:1,hub,generation:'1',epoch:'origin',requestRevision:p.requestRevision,target:p.target,viewId:p.viewId||'v'+id,viewRevision:'1',renderer:'mobile',remoteReady:true,session:s,shown,overlay:{key:hub,name:hub,j:clone(shown)},window:{ids:['e'],older:'older',hasMore:true,anchor:{id:'e',offset:0},partial:[]},cmds:[],...overrides};
  if(result.nextTarget){result.remoteReady=false;delete result.shown;delete result.overlay;delete result.window;}
  h.receive(hub,{t:'reply',id,ok:true,result},h.states.get(hub).epoch);return h.drain();
 }
 function open(hub='h'){h.hubs([...new Set([...h.paired,hub])]);h.connection(hub,'1','view-v1');const out=greeting(hub);const id=out.cmds[0].request.id;answer(id,{},hub);return out;}
 return{h,api,calls,greeting,answer,open,screen,advance:n=>now+=n};
}
check('native sidebar support is opt-in and recorded for the exact offered request',()=>{
 const x=harness();x.h.hubs(['h']);x.h.connection('h','1','view-v1');
 let req=x.greeting().cmds[0].request;
 assert(!Object.hasOwn(req.p,'nativeSidebar'));
 const before=x.h.client.calls.get(req.id).input;
 x.h.nativeSidebar=true;
 assert(!x.h.sidebarOffers.has(before));
 x.answer(req.id);x.h.update(x.h.states.get('h'));
 req=x.h.drain().cmds[0].request;
 assert.equal(req.p.nativeSidebar,1);
 assert(x.h.sidebarOffers.has(x.h.client.calls.get(req.id).input));
 assert(!x.h.sidebarOffers.has(before));
 // A hub that ignores the offer still answers the original native schema.
 const out=x.answer(req.id);assert.equal(out.sync.at(-1).phase,'presented');
});
check('navigation admission reflects pre-greeting, ready, deferred and blocked cases',()=>{
 const x=harness();x.h.hubs(['h']);x.h.connection('h','1','view-v1');
 assert.equal(x.h.navigation('h','uber','bot'),false);assert.deepEqual(x.h.drain().cmds,[]);
 assert.equal(x.h.navigation('h','select','h|a'),true);const root=x.greeting().cmds[0].request;x.answer(root.id);
 assert.equal(x.h.navigation('h','uber','bot'),true);let issued=x.h.drain().cmds[0].request;assert.equal(issued.p.action,'uber');x.answer(issued.id);
 const s=x.h.states.get('h');x.h.update(s);const update=x.h.drain().cmds[0].request;
 assert.equal(x.h.navigation('h','uber','bot'),true);assert.equal(x.h.navigation('h','room','room'),false);
 issued=x.answer(update.id).cmds[0].request;assert.equal(issued.p.action,'uber');
 assert.equal(x.h.navigation('h','send','bad'),false);assert.equal(x.h.navigation('other','select','x'),false);
});
check('navigation admission rejects queue saturation and blocked intent',()=>{
 const x=harness();x.open();const s=x.h.states.get('h');s.queue=Array.from({length:7},()=>({}));
 assert.equal(x.h.navigation('h','uber','bot'),false);s.queue=[];
 x.api.intent=()=>({kind:'blocked'});assert.equal(x.h.navigation('h','uber','bot'),false);
});
check('admitted held navigation is retired by selection, connection or forget',()=>{
 for(const change of ['select','connection','forget']) {
  const x=harness();x.open();const s=x.h.states.get('h');x.h.update(s);const call=x.h.drain().cmds[0].request;
  assert.equal(x.h.navigation('h','uber','bot'),true);
  if(change==='select') { assert.equal(x.h.navigation('h','select','h|other'),true);x.h.drain(); }
  else { if(change==='forget') x.h.forgetHub('h'); else x.h.connection('h','2','view-v1');assert.equal(s.deferredNavigation,null);assert.deepEqual(x.h.drain().cmds,[]);continue; }
  const out=x.answer(call.id);assert(!out.cmds.some(c=>c.request?.p?.action==='uber'));
 }
});
check('busy project query retains the latest full value including empty clear',()=>{
 const x=harness();x.open();const s=x.h.states.get('h');x.h.update(s);const update=x.h.drain().cmds[0].request;
 x.h.request('h','proj-find-q','older');x.h.request('h','proj-find-q','');assert.deepEqual(x.h.drain().cmds,[]);
 const out=x.answer(update.id);assert.equal(out.cmds.length,1);assert.equal(out.cmds[0].request.p.action,'proj-find-q');assert.equal(out.cmds[0].request.p.value,'');
});
function projectPicture(x, hub, query, ids) {
 const shown = x.screen(hub, x.h.states.get(hub).session.sel);
 shown.search = {open:true, query, hint:'Search'};
 shown.projects = ids.map(id => ({id}));
 return {shown, overlay:{key:hub,name:hub,j:clone(shown)}};
}
check('project Go waits exact query and strips only the owning hub prefix',()=>{
 const x=harness();x.open();x.h.projectSearchQuery(1,1,'PoE');const query=x.h.drain().cmds[0].request;
 assert(x.h.projectSearchGo(1,1,'PoE','h|p'));assert.deepEqual(x.h.drain().cmds,[]);
 const out=x.answer(query.id,projectPicture(x,'h','PoE',['p']));
 assert.equal(out.cmds.length,1);assert.equal(out.cmds[0].request.p.action,'proj-go');assert.equal(out.cmds[0].request.p.value,'p');
});
check('old prefix project cannot invoke Find.go fallback or create a thread',()=>{
 const x=harness();x.open();x.h.projectSearchQuery(1,1,'new');const query=x.h.drain().cmds[0].request;
 x.h.projectSearchGo(1,1,'new','h|old');const out=x.answer(query.id,projectPicture(x,'h','new',['new']));
 assert.deepEqual(out.cmds,[]);
});
check('new typing back to acknowledged prefix remains a full edit',()=>{
 const x=harness();x.open();x.h.projectSearchQuery(1,1,'a');let call=x.h.drain().cmds[0].request;
 x.answer(call.id,projectPicture(x,'h','a',['p']));
 x.h.projectSearchQuery(1,2,'ab');call=x.h.drain().cmds[0].request;
 x.h.projectSearchQuery(1,3,'a');const out=x.answer(call.id,projectPicture(x,'h','ab',['other']));
 assert.equal(out.cmds[0].request.p.value,'a');
});
check('project query fans out without stealing focus and Go waits every hub',()=>{
 const x=harness();x.open('a');x.open('b');const focus=x.h.current;
 x.h.projectSearchQuery(1,1,'match');const calls=x.h.drain().cmds;assert.equal(calls.length,2);assert.equal(x.h.current,focus);
 x.h.projectSearchGo(1,1,'match','a|p');assert.deepEqual(x.h.drain().cmds,[]);
 const ac=calls.find(c=>c.hub==='a').request,bc=calls.find(c=>c.hub==='b').request;
 assert.deepEqual(x.answer(ac.id,projectPicture(x,'a','match',['p']),'a').cmds,[]);
 const done=x.answer(bc.id,projectPicture(x,'b','match',['q']),'b');
 assert(done.cmds.some(c=>c.hub==='a'&&c.request.p.action==='proj-go'));
 assert(done.cmds.some(c=>c.hub==='b'&&c.request.p.action==='proj-find'&&c.request.p.value==='off'));
});
check('end tombstone and later scope defeat delayed old queries and go',()=>{
 const x=harness();x.open();x.h.projectSearchEnd(2);
 assert.equal(x.h.projectSearchQuery(1,1,'old'),false);assert.equal(x.h.projectSearchQuery(2,1,'ended'),false);
 x.h.projectSearchQuery(3,1,'new');const call=x.h.drain().cmds[0].request;x.h.projectSearchEnd(2);
 x.h.projectSearchGo(3,1,'new','h|p');x.h.projectSearchEnd(3);
 assert.deepEqual(x.answer(call.id,projectPicture(x,'h','new',['p'])).cmds,[]);
});
check('query survives generic queue saturation and selection retires pending go',()=>{
 const x=harness();x.open();const state=x.h.states.get('h');x.h.update(state);const call=x.h.drain().cmds[0].request;
 state.queue=Array.from({length:7},()=>({action:'view',value:'board',input:clone(state.session),revision:state.revision}));
 x.h.projectSearchQuery(1,1,'latest');const out=x.answer(call.id);assert.equal(out.cmds[0].request.p.action,'proj-find-q');
 x.h.projectSearchGo(1,1,'latest','h|p');x.h.request('h','select','h|other');x.h.drain();
 assert(x.h.projectSearch.ended);
});
check('newer end tombstone retires older queued work before refresh completes',()=>{
 const x=harness();x.open();x.h.update(x.h.states.get('h'));const call=x.h.drain().cmds[0].request;
 x.h.projectSearchQuery(1,1,'old');x.h.projectSearchEnd(2);
 assert.deepEqual(x.answer(call.id).cmds,[]);
});
check('a paired hub entering thin mode cancels an incomplete participant scope',()=>{
 const x=harness();x.open('a');x.h.hubs(['a','b']);
 x.h.projectSearchQuery(1,1,'match');const call=x.h.drain().cmds[0].request;
 x.h.connection('b','1','view-v1');x.greeting('b');
 assert(x.h.projectSearch.ended);assert.equal(x.h.projectSearchGo(1,1,'match',''),false);
 const out=x.answer(call.id,projectPicture(x,'a','match',['p']),'a');assert(!out.cmds.some(c=>c.request.p?.action==='proj-go'));
});
check('Return resolves the current merged first project only after every query',()=>{
 const x=harness();x.open('a');x.open('b');x.api.firstProject=()=> 'b|new-first';
 x.h.projectSearchGo(1,1,'match','');const calls=x.h.drain().cmds;
 const a=calls.find(c=>c.hub==='a').request,b=calls.find(c=>c.hub==='b').request;
 x.answer(a.id,projectPicture(x,'a','match',['old-first']),'a');
 const out=x.answer(b.id,projectPicture(x,'b','match',['new-first']),'b');
 assert(out.cmds.some(c=>c.hub==='b'&&c.request.p.action==='proj-go'&&c.request.p.value==='new-first'));
});
check('viewer receipts identify the exact accepted explicit action',()=>{
 const x=harness();x.open();assert(x.h.viewerAction('h','h|a',41,'board'));
 const call=x.h.drain().cmds[0].request;assert.equal(call.p.action,'view');
 assert(!x.h.drain().sync.some(s=>s.viewerIntent===41));
 const result=x.answer(call.id);assert(result.sync.some(s=>s.phase==='presented'&&s.viewerIntent===41));
});
check('deferred latest viewer token survives refresh without acknowledging old open',()=>{
 const x=harness();x.open();x.h.update(x.h.states.get('h'));const update=x.h.drain().cmds[0].request;
 x.h.viewerAction('h','h|a',1,'board');x.h.viewerAction('h','h|a',2,'3d');
 const next=x.answer(update.id);assert(!next.sync.some(s=>s.viewerIntent!==undefined));
 const call=next.cmds[0].request;assert.equal(call.p.value,'3d');
 const accepted=x.answer(call.id);assert(accepted.sync.some(s=>s.viewerIntent===2));assert(!accepted.sync.some(s=>s.viewerIntent===1));
});
check('viewer action cannot target a stale owner and close needs actual reply',()=>{
 const x=harness();x.open();assert.equal(x.h.viewerAction('h','h|other',1,''),false);assert.deepEqual(x.h.drain().cmds,[]);
 x.h.viewerAction('h','h|a',2,'');const issued=x.h.drain();assert(!issued.sync.some(s=>s.phase==='presented'));
 const done=x.answer(issued.cmds[0].request.id);assert(done.sync.some(s=>s.viewerIntent===2&&s.phase==='presented'));
});
check('full generic queue cannot discard accepted upload completion',()=>{
 const x=harness();x.open();const s=x.h.states.get('h');x.h.update(s);const update=x.h.drain().cmds[0].request;
 s.queue=Array.from({length:7},(_,i)=>({action:'view',value:'board'+i,input:clone(s.session),revision:s.revision}));
 assert(x.h.attachmentEnd('h','a','upload-one'));let out=x.answer(update.id),ends=[];
 for(let i=0;i<10&&out.cmds.length;i++){const call=out.cmds[0].request;if(call.p.action==='attach-end')ends.push(call.p.value);out=x.answer(call.id);}
 assert.deepEqual(ends,['upload-one']);assert.equal(s.attachmentEnds.length,0);
});
check('upload duplicate coalesces while its completion is behind another queued action',()=>{
 const x=harness();x.open();const s=x.h.states.get('h');x.h.update(s);const update=x.h.drain().cmds[0].request;
 s.queue.push({action:'view',value:'board',input:clone(s.session),revision:s.revision});
 assert(x.h.attachmentEnd('h','a','upload-one'));let out=x.answer(update.id);const prior=out.cmds[0].request;assert.equal(prior.p.action,'view');
 assert(x.h.attachmentEnd('h','a','upload-one'));assert.deepEqual(x.h.drain().cmds,[]);
 out=x.answer(prior.id);const completion=out.cmds[0].request;assert.equal(completion.p.action,'attach-end');assert.equal(completion.p.value,'upload-one');
 out=x.answer(completion.id);assert(!out.cmds.some(c=>c.request?.p?.action==='attach-end'));
});
check('pre-greeting select issues zero commands and no readiness',()=>{const x=harness();x.h.hubs(['h']);x.h.connection('h','1','view-v1');x.h.request('h','select','h|a');assert.deepEqual(x.h.drain().cmds,[]);assert.equal(x.h.states.get('h').ready,false);});
check('restored bot target and Browser route survive bootstrap select',()=>{const x=harness();const saved={version:1,current:'h',hubs:[{hub:'h',target:{kind:'bot',id:'bot'},session:{sel:'bot-thread',open:{p:true},adding:false,panel:true,info:{'fold:e':'on'},route:{kind:'bot-tab',bot:'bot',tab:'browser'}},anchor:{id:'e',offset:0}}]};assert(x.h.loadPresentation(JSON.stringify(saved)));x.h.hubs(['h']);x.h.connection('h','1','view-v1');x.h.request('h','select','h|bot-thread');const out=x.greeting();assert.equal(out.cmds[0].request.p.target.kind,'bot');assert.equal(out.cmds[0].request.p.session.route.tab,'browser');assert.equal(out.cmds[0].request.p.session.open.p,true);});
check('greeting emits presenting before one bounded open, never history',()=>{const x=harness();const out=x.open();assert.equal(out.sync[0].phase,'presenting');assert.equal(out.sync[0].remoteReady,false);assert.equal(out.cmds.length,1);assert.equal(out.cmds[0].request.m,'view.open');assert.equal(out.cmds[0].request.p.nativeRenderer,'ios-v1');});
check('only exact reply readies current native view',()=>{const x=harness();x.open();assert(x.h.states.get('h').ready);assert(x.h.views());});
check('selected A completes while B greeting remains pending',()=>{const x=harness();x.open('a');x.h.hubs(['a','b']);x.h.connection('b','1','view-v1');x.greeting('b');assert(x.h.states.get('a').ready);assert(x.h.views());});
check('normal thin frames never fall back to canonical replay',()=>{const x=harness();x.open();x.h.receive('h',{t:'log',items:[1,2,3]},'1');const out=x.h.drain();assert.equal(out.sync[0].phase,'failed');assert.deepEqual(out.cmds,[]);});
check('old native epoch frame cannot affect current view',()=>{const x=harness();x.open();assert.equal(x.h.receive('h',{t:'view.changed'},'old'),false);assert(x.h.states.get('h').ready);assert.equal(x.h.drain().sync.length,0);});
check('compact save contains choices and no drafts body or request authority',()=>{const x=harness();x.open();x.h.request('h','draft','secret draft');x.h.drain();const t=x.h.savePresentation();assert(t);assert(!t.includes('secret draft'));for(const key of ['viewId','requestRevision','remoteReady','body','outbox'])assert(!t.includes('"'+key+'"'));});
check('bad compact owner binding cannot restore',()=>{const x=harness();assert.equal(x.h.loadPresentation(JSON.stringify({version:1,current:'h',hubs:[{hub:'h',target:{kind:'thread',id:'a'},session:{sel:'a',open:{},adding:false,panel:false,info:{},route:{kind:'bot-tab',bot:'other',tab:'space'}},anchor:{id:'',offset:0}}]})),false);assert.deepEqual(x.h.drain().cmds,[]);});
check('compact load never rewinds a running connection',()=>{const x=harness();x.open();assert.equal(x.h.loadPresentation(x.h.savePresentation()),false);assert(x.h.states.get('h').ready);});
check('same bot tab action advances presentation and retires ready',()=>{const x=harness();x.open();const s=x.h.states.get('h');s.target={kind:'bot',id:'bot'};x.h.client.links.get('h').target=clone(s.target);x.h.request('h','bot-tab','space');const o=x.h.drain();assert.equal(o.sync[0].phase,'presenting');assert(o.sync[0].requestRevision>1);assert.equal(s.ready,false);assert.equal(o.cmds[0].request.m,'view.action');assert.equal(o.cmds[0].request.p.session.route.tab,'space');});
check('coalesced update issues no command or presenting transition',()=>{const x=harness();x.open();const s=x.h.states.get('h');x.h.update(s);const first=x.h.drain();const rev=first.sync[0].requestRevision;x.h.update(s);const second=x.h.drain();assert.deepEqual(second.cmds,[]);assert.deepEqual(second.sync,[]);assert.equal(x.h.client.links.get('h').revision,rev);});
check('latest viewer intent waits for current presentation and issues once',()=>{const x=harness();x.open();const s=x.h.states.get('h');x.h.update(s);const update=x.h.drain().cmds[0].request;x.h.request('h','view','board');x.h.request('h','view','3d');assert.deepEqual(x.h.drain().cmds,[]);const done=x.answer(update.id);assert.equal(done.cmds.length,1);assert.equal(done.cmds[0].request.m,'view.action');assert.equal(done.cmds[0].request.p.action,'view');assert.equal(done.cmds[0].request.p.value,'3d');assert.deepEqual(x.h.drain().cmds,[]);});
check('held Bub open runs before a coalesced background refresh',()=>{
 const x=harness();x.api.session=()=>({sel:'',drafts:{},open:{},adding:false,panel:false,info:{},rid:1});
 x.h.publicationAck=true;
 x.h.hubs(['h']);x.h.connection('h','1','view-v1');const first=x.greeting().cmds[0].request;
 const home=x.screen('h','');home.thread=null;home.sel='';x.answer(first.id,{shown:home,overlay:{key:'h',name:'h',j:clone(home)}});
 const s=x.h.states.get('h'),link=x.h.client.links.get('h');x.h.update(s);const refresh=x.h.drain().cmds[0].request;
 x.h.request('h','uber','h|bub');assert.deepEqual(x.h.drain().cmds,[]);
 x.h.receive('h',{t:'view.changed',version:1,generation:'1',epoch:'origin',viewId:link.viewId,target:clone(link.target),viewRevision:'2'},'1');
 assert.equal(link.invalidated,true);assert.deepEqual(x.h.drain().cmds,[]);
 const out=x.answer(refresh.id,{shown:home,overlay:{key:'h',name:'h',j:clone(home)},viewRevision:'1'});
 console.log('TRACE priority sync='+out.sync.map(v=>v.phase+':'+v.requestRevision).join(',')+' methods='+out.cmds.map(c=>c.request?.m||c.type).join(','));
 assert.equal(out.cmds.length,1);assert.equal(out.cmds[0].request.m,'view.action');assert.equal(out.cmds[0].request.p.action,'uber');
 const action=out.cmds[0].request;
 x.h.receive('h',{t:'view.changed',version:1,generation:'1',epoch:'origin',viewId:link.viewId,target:clone(link.target),viewRevision:'3'},'1');
 assert.deepEqual(x.h.drain().cmds,[],'a new background change cannot overtake an active person action');
 const after=x.answer(action.id,{shown:home,overlay:{key:'h',name:'h',j:clone(home)},viewRevision:'1'});
 assert.equal(after.cmds.length,0);assert(after.sync.some(s=>s.phase==='presented'));assert.equal(x.h.deadline(),null);
 x.h.published();x.h.expire(true);const follow=x.h.drain();assert.equal(follow.cmds.length,1);assert.equal(follow.cmds[0].request.m,'view.update');
 const final=x.answer(follow.cmds[0].request.id,{shown:home,overlay:{key:'h',name:'h',j:clone(home)},viewRevision:'3'});
 assert.equal(final.cmds.length,0);assert.equal(link.invalidated,false,'background change eventually covered');
});
check('unpublished follow-up cannot cross selection or epoch',()=>{
 for(const change of ['select','epoch','forget']){const x=harness();x.h.publicationAck=true;x.open();const s=x.h.states.get('h'),link=x.h.client.links.get('h');x.h.update(s);const old=x.h.drain().cmds[0].request;
  x.h.receive('h',{t:'view.changed',version:1,generation:'1',epoch:'origin',viewId:link.viewId,target:clone(link.target),viewRevision:'2'},'1');x.h.drain();const shown=x.answer(old.id);assert(shown.sync.some(v=>v.phase==='presented'));assert.equal(shown.cmds.length,0);
  if(change==='select'){x.h.request('h','select','h|other');const next=x.h.drain().cmds[0].request;x.answer(next.id);}
  else {if(change==='forget'){x.h.forgetHub('h');x.h.hubs(['h']);}x.h.connection('h','2','view-v1');const next=x.greeting('h','2').cmds[0].request;x.answer(next.id);}
  x.h.published();x.h.expire(true);assert.equal(x.h.drain().cmds.length,0);
 }
});
check('legacy engines continue immediate refresh without publication opt-in',()=>{
 const x=harness();x.open();const s=x.h.states.get('h'),link=x.h.client.links.get('h');x.h.update(s);const old=x.h.drain().cmds[0].request;
 x.h.receive('h',{t:'view.changed',version:1,generation:'1',epoch:'origin',viewId:link.viewId,target:clone(link.target),viewRevision:'2'},'1');x.h.drain();
 const out=x.answer(old.id);assert.equal(out.cmds.length,1);assert.equal(out.cmds[0].request.m,'view.update');
});
check('publication acknowledgment never consumes other hub commands',()=>{
 const x=harness();x.h.publicationAck=true;x.open('a');x.open('b');const a=x.h.states.get('a'),link=x.h.client.links.get('a');x.h.update(a);const old=x.h.drain().cmds[0].request;
 x.h.receive('a',{t:'view.changed',version:1,generation:'1',epoch:'origin',viewId:link.viewId,target:clone(link.target),viewRevision:'2'},'1');x.h.drain();x.answer(old.id,{},'a');
 x.h.enqueue('b',{type:'send',hub:'b',data:'fixture'});x.h.published();const out=x.h.drain();assert.deepEqual(out.cmds,[{type:'send',hub:'b',data:'fixture'}]);assert.deepEqual(x.h.drain().cmds,[]);
});
check('busy recent Home retains latest Bub open once through nextTarget',()=>{
 const x=harness();x.api.session=()=>({sel:'',drafts:{},open:{},adding:false,panel:false,info:{},rid:1});
 x.h.hubs(['h']);x.h.connection('h','1','view-v1');const first=x.greeting().cmds[0].request;
 const home=x.screen('h','');home.thread=null;home.sel='';x.answer(first.id,{shown:home,overlay:{key:'h',name:'h',j:clone(home)}});
 const s=x.h.states.get('h');assert.equal(s.target.kind,'recent');x.h.update(s);const refresh=x.h.drain().cmds[0].request;
 x.h.request('h','bot','h|other');x.h.request('h','uber','h|bub');x.h.request('h','send','');assert.deepEqual(x.h.drain().cmds,[]);
 const resumed=x.answer(refresh.id,{shown:home,overlay:{key:'h',name:'h',j:clone(home)}});assert.equal(resumed.cmds.length,1);
 const action=resumed.cmds[0].request;assert.equal(action.m,'view.action');assert.equal(action.p.action,'uber');assert.equal(action.p.value,'h|bub');
 const session={...clone(s.session),sel:'bub-thread'};const next=x.answer(action.id,{session,nextTarget:{kind:'thread',id:'bub-thread'}});
 assert.equal(next.cmds.length,1);assert.equal(next.cmds[0].request.m,'view.open');assert.equal(next.cmds[0].request.p.target.id,'bub-thread');
 const presented=x.answer(next.cmds[0].request.id);assert(x.h.states.get('h').ready);assert.equal(x.h.states.get('h').session.sel,'bub-thread');assert.equal(presented.cmds.length,0);
});
check('deferred Bub open is cancelled by selection or connection replacement',()=>{
 for(const change of ['select','disconnect','forget']){const x=harness();x.open();const s=x.h.states.get('h');x.h.update(s);const old=x.h.drain().cmds[0].request;x.h.request('h','uber','h|bub');
  if(change==='select'){x.h.request('h','select','h|other');const next=x.h.drain().cmds[0].request;assert(!x.answer(next.id).cmds.some(c=>c.request?.p?.action==='uber'));assert(!x.answer(old.id).cmds.some(c=>c.request?.p?.action==='uber'));}
  else {if(change==='forget'){x.h.forgetHub('h');x.h.hubs(['h']);}x.h.connection('h','2','view-v1');const next=x.greeting('h','2').cmds[0].request;assert(!x.answer(next.id).cmds.some(c=>c.request?.p?.action==='uber'));}}
});
check('only explicit bot navigation opens are deferred',()=>{
 for(const action of ['bot','uber','uber-bot']){const x=harness();x.open();const s=x.h.states.get('h');x.h.update(s);const refresh=x.h.drain().cmds[0].request;x.h.request('h',action,'h|fixture');const out=x.answer(refresh.id);assert.equal(out.cmds.length,1);assert.equal(out.cmds[0].request.p.action,action);}
});
check('newer open on another hub cancels held Bub intent',()=>{
 const x=harness();x.open('a');x.open('b');x.h.current='a';const a=x.h.states.get('a');x.h.update(a);const old=x.h.drain().cmds[0].request;
 x.h.request('a','uber','a|bub');x.h.request('b','bot','b|other');x.h.drain();const stale=x.answer(old.id,{},'a');assert(!stale.cmds.some(c=>c.request?.p?.action==='uber'));
});
check('room or remote navigation cancels an older held Bub open',()=>{
 for(const action of ['room','remote']){const x=harness();x.open();const s=x.h.states.get('h');x.h.update(s);const refresh=x.h.drain().cmds[0].request;x.h.request('h','uber','h|bub');x.h.request('h',action,'h|other');assert(!x.answer(refresh.id).cmds.some(c=>c.request?.p?.action==='uber'));}
});
check('bot tab waits for presentation and does not admit arbitrary action',()=>{const x=harness();x.open();const s=x.h.states.get('h');s.target={kind:'bot',id:'bot'};s.shownTarget={kind:'bot',id:'bot'};x.h.client.links.get('h').target=clone(s.target);x.h.update(s);const update=x.h.drain().cmds[0].request;x.h.request('h','bot-tab','space');x.h.request('h','send','');assert.deepEqual(x.h.drain().cmds,[]);const out=x.answer(update.id);assert.equal(out.cmds.length,1);assert.equal(out.cmds[0].request.p.action,'bot-tab');assert.equal(out.cmds[0].request.p.value,'space');});
check('selection change cancels held viewer intent and stale reply cannot run it',()=>{const x=harness();x.open();const s=x.h.states.get('h');x.h.update(s);const old=x.h.drain().cmds[0].request;x.h.request('h','view','board');x.h.request('h','select','h|b');const selection=x.h.drain().cmds[0].request;const fresh=x.answer(selection.id);assert(!fresh.cmds.some(c=>c.request?.p?.action==='view'));const late=x.answer(old.id);assert(!late.cmds.some(c=>c.request?.p?.action==='view'));});
check('connection replacement and forget discard held viewer intent',()=>{for(const forget of [false,true]){const x=harness();x.open();const s=x.h.states.get('h');x.h.update(s);x.h.drain();x.h.request('h','view','board');if(forget){x.h.forgetHub('h');x.h.hubs(['h']);}x.h.connection('h','2','view-v1');const greeting=x.greeting('h','2');const fresh=x.answer(greeting.cmds[0].request.id);assert(!fresh.cmds.some(c=>c.request?.p?.action==='view'));}});
check('older page is only an explicit gesture request',()=>{const x=harness();x.open();assert.equal(x.h.client.calls.size,0);x.h.request('h','earlier','');const o=x.h.drain();assert.equal(o.cmds[0].request.m,'view.page');assert.equal(o.cmds[0].request.p.cursor,'older');assert.equal(o.sync[0].phase,'presenting');});
check('hub search result does not change readiness or auto page',()=>{const x=harness();x.open();x.h.request('h','thin-search',JSON.stringify({query:'needle',cursor:''}));const o=x.h.drain();assert.deepEqual(o.sync,[]);assert(x.h.states.get('h').ready);x.answer(o.cmds[0].request.id,{search:{query:'needle',rows:[],cursor:'next',hasMore:true,visited:1}});assert.equal(x.calls.search.length,1);assert.equal(x.h.client.calls.size,0);assert(x.h.states.get('h').ready);});
check('original action commands settle once after newer selection',()=>{const x=harness();x.open();x.h.request('h','send','');const action=x.h.drain().cmds[0].request.id;x.h.request('h','select','h|b');const selected=x.h.drain().cmds[0].request.id;x.answer(selected);const command={type:'send',text:JSON.stringify({id:42,m:'turn.start',p:{thread:'a'}})};const out=x.answer(action,{cmds:[command]});assert.equal(out.cmds.length,1);assert.equal(x.h.states.get('h').session.sel,'b');assert.equal(x.h.states.get('h').shown.thread.id,'b');x.h.receive('h',{t:'reply',id:action,ok:true,result:{}},'1');assert.deepEqual(x.h.drain().cmds,[]);assert.equal(x.calls.command,1);});
check('old Keep cannot clear newer local draft',()=>{const x=harness();x.open();x.h.request('h','draft','first');x.h.drain();x.h.request('h','send','');const id=x.h.drain().cmds[0].request.id;x.h.request('h','draft','newer');x.h.drain();const out=x.answer(id,{cmds:[{type:'keep',thread:'h|a',text:''}]});assert.equal(x.h.states.get('h').session.drafts.a,'newer');assert(!out.cmds.some(c=>c.type==='keep'&&c.text===''));});
check('all command validation precedes first descriptor conversion',()=>{const x=harness();x.open();x.h.request('h','send','');const id=x.h.drain().cmds[0].request.id;const out=x.answer(id,{cmds:[{type:'send',text:JSON.stringify({id:42,m:'turn.start',p:{}})},{type:'send',text:'invalid'}]});assert.equal(x.calls.command,0);assert.deepEqual(out.cmds,[]);});
check('wrong native overlay owner rejects valid-looking commands atomically',()=>{const x=harness();x.open();x.h.request('h','send','');const id=x.h.drain().cmds[0].request.id;const out=x.answer(id,{overlay:{key:'foreign',name:'x',j:x.screen()},cmds:[{type:'send',text:JSON.stringify({id:42,m:'turn.start',p:{}})}]});assert.equal(x.calls.command,0);assert.deepEqual(out.cmds,[]);});
check('timeout never resends unknown action or downloads history',()=>{const x=harness();x.open();x.h.request('h','send','');x.h.drain();x.advance(5001);x.h.expire();const out=x.h.drain();assert.deepEqual(out.cmds,[]);assert.equal(out.sync[0].phase,'failed');x.h.expire();assert.deepEqual(x.h.drain().cmds,[]);});
check('timed-out initial presentation stays failed despite heartbeats until session replacement',()=>{
 const x=harness();x.h.hubs(['h']);x.h.connection('h','1','view-v1');const first=x.greeting().cmds[0].request;
 x.advance(5001);x.h.expire();const failed=x.h.drain();assert.equal(failed.sync[0].phase,'failed');assert.equal(failed.cmds.length,0);
 for(let i=0;i<6;i++){x.advance(15000);x.h.receive('h',{t:'hb'},'1');x.h.expire();assert.deepEqual(x.h.drain(),{cmds:[],sync:[],deadline:null});}
 assert.equal(x.h.states.get('h').ready,false);
 x.h.connection('h','2','view-v1');const retry=x.greeting('h','2');assert.equal(retry.cmds.length,1);assert.equal(retry.cmds[0].request.m,'view.open');
 assert.notEqual(retry.cmds[0].request.id,first.id);x.answer(retry.cmds[0].request.id);assert.equal(x.h.states.get('h').ready,true);
});
check('replacement after ambiguous action timeout reopens read view without replaying actions',()=>{
 const x=harness();x.open();x.h.request('h','pin','');const issued=x.h.drain().cmds[0].request;assert.equal(issued.m,'view.action');
 x.h.request('h','stop','');x.advance(5001);x.h.expire();assert.equal(x.h.drain().sync[0].phase,'failed');
 x.h.connection('h','2','view-v1');const retry=x.greeting('h','2');assert.equal(retry.cmds.length,1);assert.equal(retry.cmds[0].request.m,'view.open');
 x.answer(retry.cmds[0].request.id);assert.deepEqual(x.h.drain().cmds,[]);assert.equal(x.h.states.get('h').ready,true);
 assert.equal(x.h.receive('h',{t:'reply',id:issued.id,ok:true,result:{}},'1'),false);
 assert.equal(x.calls.command,0);
});
check('disconnect drops unstarted intentions and never resends',()=>{const x=harness();x.open();x.h.request('h','send','');x.h.drain();x.h.connection('h','2','view-v1');assert.deepEqual(x.h.drain().cmds,[]);const out=x.greeting('h','2');assert.equal(out.cmds.length,1);assert.equal(out.cmds[0].request.m,'view.open');});
check('partial message chunks require explicit gesture and assemble from the beginning',()=>{const x=harness();x.h.hubs(['h']);x.h.connection('h','1','view-v1');const initial=x.greeting();const id=initial.cmds[0].request.id;const shown=x.screen();shown.thread.id='h|a';shown.thread.entries[0].partial={version:'1',cursor:'c1',hasMore:true,complete:false};x.answer(id,{shown,overlay:{key:'h',name:'h',j:clone(shown)},window:{ids:['e'],older:'',hasMore:false,anchor:{id:'e',offset:0},partial:[{id:'e',version:'1',cursor:'c1',reason:'byte-limit',hasMore:true,complete:false}]}});assert.equal(x.h.client.calls.size,0);x.h.request('h','thin-message-more',JSON.stringify({entryId:'e',cursor:'c1'}));const out=x.h.drain();assert.deepEqual(out.sync,[]);x.answer(out.cmds[0].request.id,{message:{id:'e',thread:'a',version:'1',text:'🚀 suffix',cursor:'',hasMore:false,complete:true}});assert.equal(x.calls.message,1);assert.equal(x.h.states.get('h').shown.chunk.complete,true);assert.equal(x.h.client.calls.size,0);});
check('request counters are scoped by hub for ordinary ACK',()=>{const x=harness();x.open('a');x.open('b');for(const hub of ['a','b']){x.h.request(hub,'send','');const id=x.h.drain().cmds[0].request.id;x.answer(id,{cmds:[{type:'send',text:JSON.stringify({id:42,m:'turn.start',p:{}})}]},hub);}assert.equal(x.h.ordinary.size,2);x.h.receive('a',{t:'reply',id:42,ok:true,clear:'a'},'1');assert.equal(x.h.ordinary.size,1);assert.equal([...x.h.ordinary.values()][0].hub,'b');});

check('auxiliary errors and expiry never retire current ready presentation',()=>{const x=harness();x.open();x.h.request('h','thin-search',JSON.stringify({query:'x',cursor:''}));let req=x.h.drain().cmds[0].request;x.h.receive('h',{t:'reply',id:req.id,ok:false,error:{code:'STALE'}},'1');assert(x.h.states.get('h').ready);assert.equal(x.h.drain().sync.length,0);x.h.request('h','thin-search',JSON.stringify({query:'y',cursor:''}));x.h.drain();x.advance(5001);x.h.expire();assert(x.h.states.get('h').ready);assert.equal(x.h.drain().sync.length,0);});
check('stale action failure releases queued current action without resending old action',()=>{const x=harness();x.open();x.h.request('h','send','');const old=x.h.drain().cmds[0].request;x.h.request('h','select','h|b');const next=x.h.drain().cmds[0].request;x.answer(next.id);x.h.request('h','send','');assert.equal(x.h.drain().cmds.length,0);x.h.receive('h',{t:'reply',id:old.id,ok:false,error:{code:'UNAVAILABLE'}},'1');const out=x.h.drain();assert.equal(out.cmds.length,1);assert.equal(out.cmds[0].request.p.target.id,'b');assert.notEqual(out.cmds[0].request.id,old.id);});
check('owned update can be explicitly cancelled while presentation is pending',()=>{const x=harness();x.open();x.h.update(x.h.states.get('h'));const pending=x.h.drain().cmds[0].request;x.h.request('h','thin-cancel',JSON.stringify({requestId:pending.id}));const out=x.h.drain();assert.equal(out.cmds[0].request.m,'view.cancel');assert.equal(out.sync.length,0);});
check('live partial change and removal retire old message cursor with no auto read',()=>{const x=harness();x.open();const s=x.h.states.get('h');s.window.partial=[{id:'e',version:'1'}];s.messages.accept({id:'e',thread:'a',version:'1',text:'old',cursor:'old-next',hasMore:true,complete:false},'');x.h.update(s);let id=x.h.drain().cmds[0].request.id;x.answer(id);assert.equal(s.messages.next('e'),undefined);assert.equal(x.h.client.calls.size,0);s.window.partial=[{id:'e',version:'1'}];s.messages.accept({id:'e',thread:'a',version:'1',text:'old',cursor:'old-next',hasMore:true,complete:false},'');x.h.update(s);id=x.h.drain().cmds[0].request.id;const shown=x.screen();shown.thread.entries[0].partial={version:'2',cursor:'new',hasMore:true,complete:false};x.answer(id,{shown,overlay:{key:'h',name:'h',j:clone(shown)},window:{ids:['e'],older:'',hasMore:false,anchor:{id:'e',offset:0},partial:[{id:'e',version:'2',cursor:'new',reason:'byte-limit',hasMore:true,complete:false}]}});assert.equal(s.messages.next('e'),undefined);assert.equal(x.h.client.calls.size,0);});
check('32 historical entries plus one bounded synthetic live entry fit native contract',()=>{const x=harness();x.h.hubs(['h']);x.h.connection('h','1','view-v1');const req=x.greeting().cmds[0].request;const shown=x.screen();shown.thread.id='h|a';shown.thread.entries=Array.from({length:32},(_,i)=>({id:'e'+i,kind:'assistant',text:'history',blocks:[]}));shown.thread.entries.push({id:'@live:a',kind:'assistant',text:'live',blocks:[]});const partial=shown.thread.entries.map(e=>({id:e.id,version:'1',cursor:e.id,reason:'byte-limit',hasMore:true,complete:false}));const out=x.answer(req.id,{shown,overlay:{key:'h',name:'h',j:clone(shown)},window:{ids:shown.thread.entries.slice(0,32).map(e=>e.id),older:'',hasMore:false,anchor:{id:'e0',offset:0},partial}});assert.equal(out.sync.at(-1).phase,'presented');assert.equal(x.h.states.get('h').shown.thread.entries.length,33);});
check('malformed native partial rejects complete effect batch',()=>{const x=harness();x.open();x.h.request('h','send','');const req=x.h.drain().cmds[0].request;const shown=x.screen();shown.thread.entries[0].partial={version:1,cursor:'c',hasMore:true,complete:false};const out=x.answer(req.id,{shown,cmds:[{type:'keep',thread:'h|a',text:''}]});assert.equal(out.cmds.length,0);assert.equal(x.calls.command,0);});
check('materialization failure cannot queue any original action effect',()=>{const x=harness();x.open();x.api.materialize=()=>{throw Error('decode');};x.h.request('h','send','');const req=x.h.drain().cmds[0].request;const out=x.answer(req.id,{cmds:[{type:'send',text:JSON.stringify({id:42,m:'turn.start',p:{}})}]});assert.equal(out.cmds.length,0);assert.equal(x.calls.command,0);});

check('a field typed while its first key is answered keeps every key, as one edit',()=>{const x=harness();x.open();x.h.request('h','bfield','kname\u001fE');const first=x.h.drain().cmds;assert.equal(first.length,1);assert.equal(first[0].request.p.value,'kname\u001fE');assert.equal(x.h.states.get('h').ready,false);
 for(const t of ['E2','E2E','E2E_','E2E_K','E2E_KE','E2E_KEY','E2E_KEY1','E2E_KEY12'])x.h.request('h','bfield','kname\u001f'+t);assert.deepEqual(x.h.drain().cmds,[]);assert.equal(x.h.states.get('h').queue.length,1);
 x.h.request('h','bfield','kval\u001fs');x.h.request('h','key-add','');assert.equal(x.h.states.get('h').queue.length,3);
 let out=x.answer(first[0].request.id);assert.equal(out.cmds.length,1);assert.equal(out.cmds[0].request.p.value,'kname\u001fE2E_KEY12');
 out=x.answer(out.cmds[0].request.id);assert.equal(out.cmds[0].request.p.value,'kval\u001fs');out=x.answer(out.cmds[0].request.id);assert.equal(out.cmds[0].request.p.action,'key-add');});
check('a view still opening takes no action meanwhile',()=>{const x=harness();x.open();x.h.request('h','select','h|b');x.h.drain();x.h.request('h','bfield','kname\u001fE');assert.deepEqual(x.h.drain().cmds,[]);assert.equal(x.h.states.get('h').queue.length,0);});
check('a burst of search keys keeps one search on its way, then the newest',()=>{const x=harness();x.open();const q=s=>x.h.request('h','thin-search',JSON.stringify({query:s,cursor:''}));q('z');const first=x.h.drain().cmds;assert.equal(first.length,1);
 for(const s of ['ze','zeb','zebr','zebra','zebraf','zebrafi','zebrafin','zebrafinc','zebrafinch','zebrafinch3','zebrafinch37'])q(s);assert.deepEqual(x.h.drain().cmds,[]);assert(x.h.states.get('h').ready);
 const out=x.answer(first[0].request.id,{search:{query:'z',rows:[],cursor:'',hasMore:false,visited:1}});assert.equal(x.calls.search.length,0);assert.equal(out.cmds.length,1);assert.equal(out.cmds[0].request.p.query,'zebrafinch37');
 x.answer(out.cmds[0].request.id,{search:{query:'zebrafinch37',rows:[],cursor:'',hasMore:false,visited:1}});assert.equal(x.calls.search.length,1);assert.equal(x.calls.search[0][1].query,'zebrafinch37');assert(x.h.states.get('h').ready);});
check('the thread list view chosen on the device reaches the hub with every view request',()=>{const x=harness();x.open();const s=x.h.states.get('h');s.session={...s.session,drafts:{...s.session.drafts,'@local.view':'active','@f.kname':'secret-ish'}};x.h.update(s);const req=x.h.drain().cmds[0].request;assert.equal(req.m,'view.update');assert.equal(req.p.session.drafts['@local.view'],'active');assert.equal(req.p.session.drafts['@f.kname'],undefined);});
check('registration counter exhaustion is refused before calling Bend builder',()=>{const x=harness();x.open();x.h.states.get('h').session.rid=2**47-1;let invoked=0;x.api.registration=()=>{invoked++;return null;};x.h.register('h',['ios','token','alert','','env','bundle']);assert.equal(invoked,0);assert.equal(x.h.drain().cmds.length,0);});
const {registerBridgeChecks}=await import('./native_thin_bridge_binding_307265.mjs');
const {registerNativeHistoryChecks}=await import('./native_history_adoption_test.mjs');
registerNativeHistoryChecks(check,harness);
await registerBridgeChecks(check,stage,root);
const {registerAncillaryChecks}=await import('./native_thin_ancillary_307265.mjs');
await registerAncillaryChecks(check,harness,stage);
const {registerSidebarChecks}=await import('./native_thin_sidebar_307265.mjs');
registerSidebarChecks(check,harness,validateSidebar,validateSidebarAction);
let failed=0;
try{for(const [name,fn]of tests){try{fn();console.log('PASS',name);}catch(e){failed++;console.log('FAIL',name,e.stack);}}console.log(JSON.stringify({scope:'Native source adapter + actual shared18da transport only; stub Bend renderer, no emitted/native/service/UI acceptance',checks:tests.length,failed}));}finally{fs.rmSync(stage,{recursive:true,force:true});}
if(failed)process.exitCode=1;
