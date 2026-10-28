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
for(const n of ['src/web/thin.js','src/web/thin-results.js','src/web/startup.js','src/web/mail-drafts.js','src/web/terminal-input.js','src/web/model-menu-input.js','src/web/sidebar-region.js']) {const q=path.join(stage,n);fs.mkdirSync(path.dirname(q),{recursive:true});fs.copyFileSync((process.env.BP_NATIVE_THIN_SOURCE_ROOT ? path.join(process.env.BP_NATIVE_THIN_SOURCE_ROOT,n) : path.join(packet,fs.existsSync(path.join(packet,'qualified-dependencies'))?'qualified-dependencies':'dependencies',n)),q);}
fs.mkdirSync(path.join(stage,'src/mobile'),{recursive:true});
fs.copyFileSync(process.env.BP_NATIVE_THIN_HELPER_SOURCE||path.join(root,'src/mobile/thin-native-host-307265.js'),path.join(stage,'src/mobile/thin-native-host-307265.js'));
fs.writeFileSync(path.join(stage,'package.json'),'{"type":"module"}');
const {NativeThinHost307265:Host}=await import(pathToFileURL(path.join(stage,'src/mobile/thin-native-host-307265.js')));
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
await registerBridgeChecks(check,stage,root);
const {registerAncillaryChecks}=await import('./native_thin_ancillary_307265.mjs');
await registerAncillaryChecks(check,harness,stage);
let failed=0;
try{for(const [name,fn]of tests){try{fn();console.log('PASS',name);}catch(e){failed++;console.log('FAIL',name,e.stack);}}console.log(JSON.stringify({scope:'Native source adapter + actual shared18da transport only; stub Bend renderer, no emitted/native/service/UI acceptance',checks:tests.length,failed}));}finally{fs.rmSync(stage,{recursive:true,force:true});}
if(failed)process.exitCode=1;
