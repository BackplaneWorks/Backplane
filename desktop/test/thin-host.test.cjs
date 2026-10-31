'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict');
async function fixture(overrides={}){
 const {ThinHost}=await import(process.env.THIN_HOST_MODULE||'../../src/web/thin-host.js'),{ThinViews,VIEW_LIMITS}=await import('../../src/web/thin.js');
 let at=0,session={sel:'A',drafts:{A:'original 日本語'},rid:1},persisted=0,fieldRefresh;
 const sent=[],commands=[],views=[],states=[],warnings=[],searches=[],messages=[],disk=new Map();
 const choice=(s,a,v)=>a==='select'?{...s,sel:v}:a==='draft'?{...s,drafts:{...s.drafts,[s.sel]:v}}:s;
 const api={hub:'https://fixture',cid:'fixture',now:()=>at,session:()=>session,
  target:s=>s.sel?{kind:'thread',id:s.sel}:{kind:'recent',id:''},choice,
  prepare:(s,input)=>({...input,rid:s.rid}),counter:n=>persisted=n,
  send:o=>{assert.equal(persisted,o.id);sent.push(structuredClone(o));},
  local:(a,v,s)=>{if(a==='draft')disk.set(s.sel,v);},sessionChanged:s=>session=s,
  ack:(s,e)=>({...s,ack:e.id}),decode:n=>n,
  command:c=>{commands.push(c);if(c.type==='keep')disk.set(c.thread,c.text);},
  state:s=>states.push(s),view:(n,detail)=>views.push({n,detail}),search:s=>searches.push(s),message:m=>messages.push(m),anchor:()=>({id:'row',offset:0}),fieldRefresh:fn=>fieldRefresh=fn,blocked(){},warning:s=>warnings.push(s)};
 Object.assign(api,overrides);
 const host=new ThinHost(api,ThinViews,2**47);
 const greeting={t:'view',version:1,renderer:api.renderer||'web',generation:'7',epoch:'actual',limits:{...VIEW_LIMITS}};
 const reply=(request=sent.at(-1),extra={})=>({t:'reply',id:request.id,ok:true,result:{version:1,hub:api.hub,generation:'7',epoch:'actual',requestRevision:request.p.requestRevision,
  target:request.p.target,viewId:request.p.viewId||'view-'+request.p.target.id,viewRevision:'1',remoteReady:true,renderer:greeting.renderer,shown:{type:'text',text:'VIEW '+request.p.target.id},
  session:structuredClone(request.p.session||session),cmds:[],window:{ids:['row'],older:'older-bound-to-A',hasMore:true,anchor:{id:'row',offset:0},partial:[]},...extra}});
 const open=()=>{host.greeting(greeting);const r=reply();host.receive(r);return r;};
 return{host,api,sent,commands,views,states,warnings,searches,messages,disk,reply,open,greeting,flushFields:()=>fieldRefresh?.(),getSession:()=>session,tick:ms=>{at+=ms;host.expire();}};
}
test('model menu compact: cached menu settles without replacing picture or readiness',async()=>{
 const f=await fixture({modelMenuReady:()=>true});f.open();const count=f.views.length;f.host.request('flag','models');const req=f.sent.at(-1);assert.equal(req.p.modelMenuInput,true);
 const reply=f.reply(req,{modelMenuInput:true,session:{...req.p.session,open:{models:true}}});delete reply.result.shown;delete reply.result.window;
 assert.equal(f.host.receive(reply),true);assert.equal(f.host.ready,true);assert.equal(f.views.length,count);assert.equal(f.getSession().open.models,true);assert.equal(f.host.receive(reply),false);
 f.host.request('model','server-choice');assert.equal(f.sent.at(-1).p.modelMenuInput,undefined);
});
test('model menu compact: invalid ACK executes no commands and cannot establish first readiness',async()=>{
 const f=await fixture({modelMenuReady:()=>true});f.open();f.host.request('flag','models');const req=f.sent.at(-1),reply=f.reply(req,{modelMenuInput:true,session:{...req.p.session,open:{models:true}},cmds:[{type:'send',text:JSON.stringify({id:2,m:'thread.modes',p:{}})}]});delete reply.result.shown;delete reply.result.window;
 f.host.receive(reply);assert.equal(f.commands.length,0);assert.equal(f.host.ready,false);
 const fresh=await fixture();fresh.host.greeting(fresh.greeting);const forged=fresh.reply(undefined,{modelMenuInput:true});delete forged.result.shown;delete forged.result.window;fresh.host.receive(forged);assert.equal(fresh.host.ready,false);
});
test('model menu compact: deferred output refresh follows ACK and queued Settings retains priority',async()=>{
 const f=await fixture({modelMenuReady:()=>true});f.open();f.host.request('flag','models');const req=f.sent.at(-1);f.host.update();f.host.request('flag','settings');
 const reply=f.reply(req,{modelMenuInput:true,session:{...req.p.session,open:{models:true}}});delete reply.result.shown;delete reply.result.window;f.host.receive(reply);
 assert.equal(f.sent.at(-1).m,'view.action');assert.equal(f.sent.at(-1).p.value,'settings');assert.equal(f.sent.filter(x=>x.m==='view.update').length,0);
 f.host.receive(f.reply());assert.equal(f.sent.at(-1).m,'view.update');
});
test('refresh coverage host: one Settings action picture satisfies its earlier invalidation',async()=>{
 const f=await fixture();f.open();f.host.request('flag','settings');const req=f.sent.at(-1);f.host.receive({t:'view.changed',version:1,generation:'7',epoch:'actual',viewId:'view-A',target:{kind:'thread',id:'A'},viewRevision:'2'});
 f.host.receive(f.reply(req,{viewRevision:'2'}));assert.equal(f.sent.length,2);f.host.request('settings-page','appearance');assert.equal(f.sent.at(-1).m,'view.action');assert.equal(f.sent.at(-1).p.action,'settings-page');
});
test('refresh coverage host: draft refresh remains explicit across a covered action picture',async()=>{
 const f=await fixture({draftRefresh:fn=>fn()});f.open();f.host.request('flag','settings');const req=f.sent.at(-1);f.host.receive({t:'view.changed',version:1,generation:'7',epoch:'actual',viewId:'view-A',target:{kind:'thread',id:'A'},viewRevision:'2'});f.host.request('draft','new local draft');f.host.receive(f.reply(req,{viewRevision:'2'}));assert.equal(f.sent.at(-1).m,'view.update');assert.equal(f.sent.at(-1).p.session.drafts.A,'new local draft');assert.equal(f.getSession().drafts.A,'new local draft');
});
test('greeting alone is not ready; matching current result renders without any canonical history request',async()=>{
 const f=await fixture();f.host.greeting(f.greeting);assert.equal(f.host.ready,false);assert.equal(f.sent.length,1);assert.equal(f.sent[0].m,'view.open');
 f.host.receive(f.reply());assert.equal(f.host.ready,true);assert.equal(f.views.length,1);assert(f.sent.every(x=>x.m.startsWith('view.')));
});
test('a draft typed while first view travels remains local and overtakes its old response',async()=>{
 const f=await fixture();f.host.greeting(f.greeting);const old=f.reply();f.host.request('draft','NEW 日本語 👩‍💻');f.host.receive(old);
 assert.equal(f.getSession().drafts.A,'NEW 日本語 👩‍💻');assert.equal(f.disk.get('A'),'NEW 日本語 👩‍💻');assert.equal(f.sent.length,1);
});
test('scrolling pages: one page request at a time, and a failed one puts the trail back',async()=>{
 const f=await fixture();f.open();assert.equal(f.host.page(),true);const req=f.sent.at(-1);assert.equal(f.host.page(),false,'a second older page waits for the first');assert.equal(f.host.newer(),false,'and so does a newer one');
 f.host.receive({t:'reply',id:req.id,ok:false,error:{code:'STALE_CURSOR',message:'stale'}});assert.equal(f.host.atNewest(),true,'the failed page leaves the trail as it was');assert.equal(f.host.paging,null,'and no page is waited for');
});
test('history recovery: failed page keeps the validated picture, draft and actions usable until explicit retry',async()=>{
 const states=[],f=await fixture({pageState:s=>states.push(s)});f.open();const before=f.views.length;
 f.host.page();const req=f.sent.at(-1);assert.deepEqual(req.p.anchor,{id:'row',offset:0});
 f.host.receive({t:'reply',id:req.id,ok:false,error:{code:'UNAVAILABLE'}});
 assert.equal(f.host.ready,true);assert.equal(f.views.length,before);assert.equal(f.getSession().drafts.A,'original 日本語');assert.equal(states.at(-1).phase,'error');
 f.host.request('draft','new unsent');f.host.request('stop','');assert.equal(f.sent.at(-1).p.action,'stop');f.host.receive(f.reply());
 assert.equal(f.host.retryPage(),true);assert.equal(f.sent.at(-1).p.cursor,req.p.cursor);f.host.receive(f.reply());
 assert.equal(f.host.pageError,null);assert.equal(f.getSession().drafts.A,'new unsent');
});
test('history recovery: timeout restores the trail; late timed-out reply cannot move the page',async()=>{
 const f=await fixture();f.open();f.host.page();const req=f.sent.at(-1),before=f.views.length;f.tick(5001);
 assert.equal(f.host.ready,true);assert.equal(f.host.atNewest(),true);assert.equal(f.host.paging,null);assert.equal(f.host.pageError.direction,'older');
 assert.equal(f.host.receive(f.reply(req)),false);assert.equal(f.views.length,before);assert.equal(f.host.retryPage(),true);
});
test('history ordering: notifications and draft refresh wait behind one page, then use the accepted page',async()=>{
 const f=await fixture();f.open();f.host.page();const req=f.sent.at(-1),count=f.sent.length;
 f.host.update();f.host.receive({t:'view.changed',version:1,generation:'7',epoch:'actual',viewId:'view-A',target:{kind:'thread',id:'A'},viewRevision:'2'});
 assert.equal(f.sent.length,count);f.host.request('draft','typing while paging');f.host.receive(f.reply(req));
 assert.equal(f.sent.at(-1).m,'view.update');assert.equal(f.sent.at(-1).p.session.drafts.A,'typing while paging');
});
test('history ordering: Stop can overtake a page without losing its accepted cursor or the stop',async()=>{
 const f=await fixture();f.open();f.host.page();const req=f.sent.at(-1);f.host.request('stop','');const stop=f.sent.at(-1);
 f.host.receive(f.reply(req));assert.equal(f.host.atNewest(),false);assert.equal(f.host.cursor,req.p.cursor);
 f.host.receive(f.reply(stop));assert.equal(f.host.ready,true);assert(f.sent.some(r=>r.p.action==='stop'));
});
test('history progress: identical next cursor never triggers an overlap request loop',async()=>{
 const f=await fixture();f.open();f.host.page();f.host.receive(f.reply());const count=f.sent.length;
 assert.equal(f.host.page(),false);assert.equal(f.sent.length,count);
});
test('history identity: returning to a conversation restores its issued page before presenting, with its own draft and anchor',async()=>{
 const position={top:1100,atEnd:false,anchor:'row',offset:-37};
 const f=await fixture({pagePosition:()=>({position})});f.open();f.host.page();const page=f.sent.at(-1);f.host.receive(f.reply(page));
 f.host.request('select','B');f.host.receive(f.reply());f.host.request('draft','B only');const count=f.views.length;
 f.host.request('select','A');f.host.receive(f.reply());assert.equal(f.sent.at(-1).m,'view.page');assert.equal(f.sent.at(-1).p.cursor,page.p.cursor);
 assert.equal(f.views.length,count,'newest page is never flashed');f.host.receive(f.reply());
 assert.equal(f.host.atNewest(),false);assert.deepEqual(f.views.at(-1).detail.position,position);assert.equal(f.getSession().drafts.A,'original 日本語');assert.equal(f.getSession().drafts.B,'B only');
});
test('history identity: a reader in the outgoing newer bridge restores its adjacent page',async()=>{
 let adjacent='';const f=await fixture({pagePosition:()=>({position:{anchor:'row',atEnd:false,offset:0},adjacent})});f.open();
 f.host.page();f.host.receive(f.reply(undefined,{window:{ids:['row'],older:'next',hasMore:true,anchor:{id:'row',offset:0},partial:[]}}));f.host.page();f.host.receive(f.reply());
 adjacent='newer';f.host.request('select','B');f.host.receive(f.reply());adjacent='';f.host.request('select','A');f.host.receive(f.reply());
 assert.equal(f.sent.at(-1).p.cursor,'older-bound-to-A');f.host.receive(f.reply());assert.equal(f.host.back.length,1);
});
test('history identity: bookmarks are limited to eight conversations and 64KiB',async()=>{
 const f=await fixture({pagePosition:()=>({position:{anchor:'row',atEnd:false,offset:0}})});f.open();
 for(let i=0;i<20;i++){f.host.request('select','thread-'+i);f.host.receive(f.reply());}
 assert.equal(f.host.pagePositions.size,8);assert(f.host.pagePositionBytes<=65536);
});
test('history identity: a failed restore retries its exact saved page, never the newest page cursor',async()=>{
 const f=await fixture({pagePosition:()=>({position:{anchor:'row',atEnd:false,offset:-5}})});f.open();f.host.page();f.host.receive(f.reply());
 f.host.request('select','B');f.host.receive(f.reply());f.host.request('select','A');f.host.receive(f.reply());const restore=f.sent.at(-1);
 f.host.receive({t:'reply',id:restore.id,ok:false,error:{code:'UNAVAILABLE'}});assert.equal(f.host.pageError.direction,'restore');assert.equal(f.host.retryPage(),true);
 assert.equal(f.sent.at(-1).p.cursor,restore.p.cursor);f.host.receive(f.reply());assert.equal(f.views.at(-1).detail.position.offset,-5);
});
test('history identity: changed resolved conversation rejects the bookmark after fresh selection',async()=>{
 const f=await fixture({pagePosition:()=>({position:{anchor:'row',atEnd:false,offset:-5}})});f.open();f.host.page();f.host.receive(f.reply());
 f.host.request('select','B');f.host.receive(f.reply());f.host.request('select','A');const open=f.sent.at(-1);
 f.host.receive(f.reply(open,{session:{...open.p.session,sel:'resolved-other'}}));assert.equal(f.sent.at(-1).m,'view.open');assert.equal(f.host.atNewest(),true);assert.equal(f.host.ready,true);assert.equal(f.views.at(-1).detail.position,undefined);
});
test('history identity: stale saved cursor is discarded and selects fresh without disabling the conversation',async()=>{
 const f=await fixture({pagePosition:()=>({position:{anchor:'row',atEnd:false,offset:-5}})});f.open();f.host.page();f.host.receive(f.reply());
 f.host.request('select','B');f.host.receive(f.reply());f.host.request('select','A');f.host.receive(f.reply());const restore=f.sent.at(-1);
 f.host.receive({t:'reply',id:restore.id,ok:false,error:{code:'STALE_CURSOR'}});assert.equal(f.sent.at(-1).m,'view.open');f.host.receive(f.reply());
 assert.equal(f.host.ready,true);assert.equal(f.host.atNewest(),true);assert.equal(f.host.pageError,null);assert(!f.states.some(s=>s.phase==='error'));
});
test('history identity: oversized trails are never retained in conversation bookmarks',async()=>{
 const f=await fixture({pagePosition:()=>({position:{anchor:'row',atEnd:false,offset:0}})});f.open();f.host.back=Array(513).fill('issued-cursor');f.host.rememberPage();assert.equal(f.host.pagePositions.size,0);
});
test('scrolling pages: older pages are retraced back down to the newest, and a send returns there',async()=>{
 const f=await fixture();f.open();assert.equal(f.host.atNewest(),true);assert.equal(f.host.newer(),false);
 f.host.page();assert.equal(f.sent.at(-1).p.cursor,'older-bound-to-A');assert.equal(f.host.atNewest(),false);
 f.host.receive(f.reply(f.sent.at(-1),{window:{ids:['r2'],older:'older-2',hasMore:true,anchor:{id:'r2',offset:0},partial:[]}}));
 f.host.page();assert.equal(f.sent.at(-1).p.cursor,'older-2');f.host.receive(f.reply(f.sent.at(-1)));
 f.host.newer();assert.equal(f.sent.at(-1).m,'view.page');assert.equal(f.sent.at(-1).p.cursor,'older-bound-to-A');f.host.receive(f.reply(f.sent.at(-1)));
 f.host.newer();assert.equal(f.sent.at(-1).p.cursor,'');assert.equal(f.host.atNewest(),true);f.host.receive(f.reply(f.sent.at(-1)));
 f.host.page();f.host.receive(f.reply(f.sent.at(-1)));assert.equal(f.host.atNewest(),false);f.host.request('send','');assert.equal(f.host.atNewest(),true);
});
test('a send drops the older page still on its way: its late reply never puts the reader back on it',async()=>{
 const f=await fixture();f.open();f.host.page();const pg=f.sent.at(-1);f.host.request('send','');const count=f.views.length;
 f.host.receive(f.reply(pg));assert.equal(f.views.length,count,'the old page is not shown');assert.equal(f.host.atNewest(),true);assert.equal(f.host.paging,null);
 assert(f.sent.some(x=>x.m==='view.action'),'the send still goes');
});
test('page and search are explicit requests; ordinary open and selected updates never fetch older pages',async()=>{
 const f=await fixture();f.open();assert.equal(f.sent.length,1);f.host.page();assert.equal(f.sent.at(-1).m,'view.page');assert.equal(f.sent.at(-1).p.cursor,'older-bound-to-A');
 f.host.search('日本語 query');assert.equal(f.sent.at(-1).m,'view.search');assert.equal(f.sent.at(-1).p.query,'日本語 query');
});
test('selected invalidations coalesce; they only request update, not page/history',async()=>{
 const f=await fixture();f.open();const event={t:'view.changed',version:1,generation:'7',epoch:'actual',viewId:'view-A',target:{kind:'thread',id:'A'},viewRevision:'2'};
 f.host.receive(event);f.host.receive(event);assert.equal(f.sent.length,2);assert.equal(f.sent.at(-1).m,'view.update');
});
test('an old action settles once without overwriting a newer selection or its draft',async()=>{
 const f=await fixture();f.open();f.host.request('send','');const action=f.sent.at(-1);f.host.request('select','B');const next=f.sent.at(-1);f.host.request('draft','B newer');f.host.receive(f.reply(next));
 const count=f.views.length;const old=f.reply(action,{session:{sel:'A',drafts:{A:''},rid:20},cmds:[{type:'send',text:JSON.stringify({id:20,m:'message.send',p:{thread:'A'}})},{type:'keep',thread:'A',text:''}]});
 f.host.receive(old);assert.equal(f.host.receive(old),false);assert.equal(f.commands.filter(c=>c.type==='send').length,1);assert.equal(f.views.length,count);
 assert.equal(f.getSession().sel,'B');assert.equal(f.getSession().drafts.B,'B newer');assert.equal(f.disk.get('B'),'B newer');
});
test('an old Keep cannot clear a newer draft in the same conversation',async()=>{
 const f=await fixture();f.open();f.host.request('send','');const action=f.sent.at(-1);f.host.request('draft','new unsent');
 f.host.receive(f.reply(action,{session:{sel:'A',drafts:{A:''},rid:20},cmds:[{type:'keep',thread:'A',text:''}]}));assert.equal(f.getSession().drafts.A,'new unsent');assert.equal(f.disk.get('A'),'new unsent');
});
test('an unstarted old-view action never acquires the newly selected target',async()=>{
 const f=await fixture();f.open();f.host.request('send','first');const action=f.sent.at(-1);f.host.request('send','second');assert.equal(f.sent.filter(x=>x.m==='view.action').length,1);
 f.host.request('select','B');f.host.receive(f.reply());f.host.receive(f.reply(action));assert.equal(f.sent.filter(x=>x.m==='view.action').length,1);assert.equal(f.getSession().sel,'B');
});
test('old target error cannot revoke a newer current view or replace its body',async()=>{
 const f=await fixture();f.open();f.host.request('send','');const action=f.sent.at(-1);f.host.request('select','B');f.host.receive(f.reply());const count=f.views.length;
 f.host.receive({t:'reply',id:action.id,ok:false,error:{code:'STALE_VIEW'}});assert.equal(f.host.ready,true);assert.equal(f.views.length,count);assert.equal(f.getSession().sel,'B');
});
test('timeout/disconnect never resends an action or starts global history',async()=>{
 const f=await fixture();f.open();f.host.request('send','');const before=f.sent.length;f.tick(5001);f.host.disconnect();assert.equal(f.sent.length,before);assert.equal(f.host.ready,false);
 assert.equal(f.getSession().drafts.A,'original 日本語');assert(!f.sent.some(r=>r.m.includes('history')));
});
test('a bad later command prevents every command from dispatching',async()=>{
 const f=await fixture();f.open();f.host.request('send','');f.host.receive(f.reply(undefined,{cmds:[{type:'send',text:JSON.stringify({id:2,m:'message.send',p:{}})},{type:'INVALID'}]}));assert.equal(f.commands.length,0);assert.equal(f.host.ready,false);
});
test('ordinary ACK arriving during a projection survives its later session response',async()=>{
 const f=await fixture();f.open();f.host.request('send','');f.host.receive(f.reply(undefined,{cmds:[{type:'send',text:JSON.stringify({id:33,m:'message.send',p:{thread:'A'}})}]}));
 f.host.update();const response=f.reply();f.host.ack({id:33});f.host.receive(response);assert.equal(f.getSession().ack,33);
});
test('control IDs are disjoint from the ordinary command counter',async()=>{
 const f=await fixture();f.open();f.host.request('send','');assert(f.sent.every(r=>r.id>2**47));assert(f.getSession().rid<2**47);
});
test('search result cannot replace shown, displayed revision, session or readiness',async()=>{
 const f=await fixture();f.open();f.host.search('query');const result=f.reply(undefined,{viewRevision:'999',search:{query:'query',rows:[{id:'r',thread:'B',ordinal:'7',kind:'message',text:'日本語',partial:false}],cursor:'',hasMore:false,visited:1}});
 delete result.result.session;delete result.result.window;result.result.shown={type:'text',text:'MUST NOT DISPLAY'};
 const count=f.views.length;f.host.receive(result);assert.equal(f.searches.length,1);assert.equal(f.views.length,count);assert.equal(f.host.client.links.get(f.api.hub).viewRevision,'1');assert.equal(f.getSession().sel,'A');assert.equal(f.host.ready,true);
});
test('new navigation rejects a late search result without showing a stale result panel',async()=>{
 const f=await fixture();f.open();f.host.search('old');const query=f.sent.at(-1);f.host.request('select','B');f.host.receive(f.reply());
 f.host.receive(f.reply(query,{search:{query:'old',rows:[],cursor:'',hasMore:false,visited:0}}));assert.equal(f.searches.length,0);assert.equal(f.host.ready,true);assert.equal(f.getSession().sel,'B');
});
test('a search answered after an update of the same view still reaches the search dialog',async()=>{
 const f=await fixture();f.open();const change={t:'view.changed',version:1,generation:'7',epoch:'actual',viewId:'view-A',target:{kind:'thread',id:'A'},viewRevision:'2'};
 f.host.search('query');const query=f.sent.at(-1);f.host.receive(change);const update=f.sent.at(-1);assert.equal(update.m,'view.update');
 f.host.receive(f.reply(query,{search:{query:'query',rows:[],cursor:'',hasMore:false,visited:0}}));assert.equal(f.searches.length,1);assert.equal(f.searches[0].query,'query');
});
test('a search the hub refuses is told to the search, and the view stays',async()=>{
 const f=await fixture();const failed=[];f.api.searchFailed=code=>failed.push(code);f.open();f.host.search('???');const query=f.sent.at(-1);
 f.host.receive({t:'reply',id:query.id,ok:false,error:{code:'LIMIT'}});assert.deepEqual(failed,['LIMIT']);assert.equal(f.host.ready,true);assert.equal(f.getSession().sel,'A');
});
test('auxiliary query cannot strand the selected update or consume its displayed revision',async()=>{
 const f=await fixture();f.open();const change={t:'view.changed',version:1,generation:'7',epoch:'actual',viewId:'view-A',target:{kind:'thread',id:'A'},viewRevision:'2'};
 f.host.receive(change);const update=f.sent.at(-1);f.host.search('query');const query=f.sent.at(-1);f.host.receive(f.reply(update,{viewRevision:'2'}));
 assert.equal(f.sent.filter(x=>x.m==='view.update').length,2);assert.equal(f.views.length,1);
 f.host.receive(f.reply(query,{viewRevision:'777',search:{query:'query',rows:[],cursor:'',hasMore:false,visited:0}}));assert.equal(f.host.client.links.get(f.api.hub).viewRevision,'1');
});
test('current normal-action nextTarget settles commands then opens a fresh bot view without fabricated readiness',async()=>{
 const f=await fixture();f.open();f.host.request('bot-open','bot-B');const action=f.sent.at(-1),response=f.reply(action,{nextTarget:{kind:'bot',id:'bot-B'},remoteReady:false});delete response.result.shown;
 const count=f.views.length;f.host.receive(response);assert.equal(f.views.length,count);assert.equal(f.host.ready,false);assert.equal(f.sent.at(-1).m,'view.open');assert.deepEqual(f.sent.at(-1).p.target,{kind:'bot',id:'bot-B'});
 f.host.receive(f.reply());assert.equal(f.host.ready,true);assert.equal(f.views.length,count+1);
});
test('creation ACK update opens the child without repeating creation or fabricating readiness',async()=>{
 const f=await fixture();f.open();f.host.request('thread-child','A');
 f.host.receive(f.reply(undefined,{session:{sel:'A',drafts:{A:'original 日本語'},rid:34},cmds:[{type:'send',text:JSON.stringify({id:33,m:'thread.create',p:{project:'p',parent:'A'}})}]}));
 f.host.ack({t:'reply',id:33,ok:true,thread:'child'});f.host.update();const update=f.sent.at(-1);assert.equal(update.m,'view.update');
 const response=f.reply(update,{nextTarget:{kind:'thread',id:'child'},remoteReady:false,session:{sel:'child',drafts:{A:'original 日本語'},rid:34}});delete response.result.shown;
 const count=f.views.length;f.host.receive(response);assert.equal(f.commands.filter(c=>c.type==='send').length,1);assert.equal(f.views.length,count);assert.equal(f.host.ready,false);
 assert.equal(f.sent.at(-1).m,'view.open');assert.deepEqual(f.sent.at(-1).p.target,{kind:'thread',id:'child'});
 f.host.receive(f.reply());assert.equal(f.host.ready,true);assert.equal(f.getSession().sel,'child');
});
test('creation ACK update cannot redirect a newer explicit selection',async()=>{
 const f=await fixture();f.open();f.host.update();const update=f.sent.at(-1);
 f.host.request('select','B');f.host.receive(f.reply());
 const response=f.reply(update,{nextTarget:{kind:'thread',id:'child'},remoteReady:false,session:{sel:'child',drafts:{},rid:34}});delete response.result.shown;
 const sent=f.sent.length,shown=f.views.length;f.host.receive(response);assert.equal(f.sent.length,sent);assert.equal(f.views.length,shown);assert.equal(f.getSession().sel,'B');assert.equal(f.host.ready,true);
});
test('an obsolete action nextTarget cannot navigate away from the newer selected thread',async()=>{
 const f=await fixture();f.open();f.host.request('bot-open','bot-C');const action=f.sent.at(-1);f.host.request('select','B');f.host.receive(f.reply());
 const response=f.reply(action,{nextTarget:{kind:'bot',id:'bot-C'},remoteReady:false});delete response.result.shown;const count=f.sent.length;f.host.receive(response);assert.equal(f.sent.length,count);assert.equal(f.getSession().sel,'B');assert.equal(f.host.ready,true);
});
// A project's Intent… opens the project's thread and asks the hub for its
// intent at once (fab.look); the ask goes now, and the Focus on the dialog and
// a Jump into the thread are for the view it opens, not the picture shown
// while it comes.
const moving=(f,action,cmds)=>{const r=f.reply(action,{nextTarget:{kind:'thread',id:'B'},remoteReady:false,cmds});delete r.result.shown;return r;};
const intentCmds=[{type:'send',text:JSON.stringify({id:5,m:'fab.look',p:{thread:'B'}})},{type:'focus',id:'intent'},{type:'jump',id:'entry-9'}];
test('a moving action asks at once and keeps its Focus and Jump for the view it opens',async()=>{
 const f=await fixture();f.open();f.host.request('proj-intent','q');f.host.receive(moving(f,f.sent.at(-1),intentCmds));
 assert.deepEqual(f.commands.map(c=>c.type),['send'],'the hub is asked at once; Focus and Jump wait');
 assert.equal(f.sent.at(-1).m,'view.open');assert.deepEqual(f.sent.at(-1).p.target,{kind:'thread',id:'B'});
 f.host.receive(f.reply());assert.equal(f.host.ready,true);
 assert.deepEqual(f.commands.map(c=>c.type+':'+(c.id||'')),['send:','focus:intent','jump:entry-9'],'given once B is on show');
 f.host.update();f.host.receive(f.reply());assert.equal(f.commands.length,3,'and only once');
});
test('a moving action\'s Focus and Jump are dropped when the person goes elsewhere first',async()=>{
 const f=await fixture();f.open();f.host.request('proj-intent','q');f.host.receive(moving(f,f.sent.at(-1),intentCmds));
 f.host.request('select','C');f.host.receive(f.reply());assert.deepEqual(f.getSession().sel,'C');
 assert.deepEqual(f.commands.map(c=>c.type),['send'],'not on C');
 f.host.request('select','B');f.host.receive(f.reply());assert.deepEqual(f.commands.map(c=>c.type),['send'],'nor on B later');
});
test('a failed move drops its Focus and Jump',async()=>{
 const f=await fixture();f.open();f.host.request('proj-intent','q');f.host.receive(moving(f,f.sent.at(-1),intentCmds));
 const open=f.sent.at(-1);f.host.receive({t:'reply',id:open.id,ok:false,error:{code:'UNAVAILABLE'}});assert.equal(f.host.forView,null);
});
// A click right after picking a thread (the top bar's gear while its view is
// on its way) was refused as "not ready" and lost; it waits for that view.
test('a press while a thread\'s view is on its way goes once the view is on show',async()=>{
 const f=await fixture();f.open();f.host.request('select','B');const open=f.sent.at(-1);assert.equal(open.m,'view.open');
 let settled;f.host.request('flag','settings',ok=>settled=ok);assert.equal(f.sent.length,2,'nothing sent while B is on its way');assert.equal(settled,undefined,'and not refused');
 f.host.receive(f.reply(open));const action=f.sent.at(-1);assert.equal(action.m,'view.action');assert.equal(action.p.action,'flag');assert.equal(action.p.value,'settings');
 assert.deepEqual(action.p.target,{kind:'thread',id:'B'},'for B');f.host.receive(f.reply(action));assert.equal(settled,true);
});
test('a press waiting for a view is dropped when the person picks another thread first',async()=>{
 const f=await fixture();f.open();f.host.request('select','B');let settled;f.host.request('flag','settings',ok=>settled=ok);
 f.host.request('select','C');assert.equal(settled,false);f.host.receive(f.reply());assert.equal(f.sent.filter(x=>x.m==='view.action').length,0);
});
test('a press with no view on its way is still refused, and a terminal\'s keys always wait for theirs',async()=>{
 const f=await fixture();f.host.request('flag','settings');assert.equal(f.sent.length,0,'before the greeting');
 f.open();f.host.request('select','B');const open=f.sent.at(-1);f.host.receive({t:'reply',id:open.id,ok:false,error:{code:'UNAVAILABLE'}});
 let settled;f.host.request('flag','settings',ok=>settled=ok);assert.equal(settled,false,'after a failed open');
 const g=await fixture();g.open();g.host.request('select','B');let keys;g.host.request('term-key','a',ok=>keys=ok);assert.equal(keys,false);
});
test('a background update cannot discard a newer navigation action',async()=>{
 const f=await fixture();f.open();f.host.request('uber','bub');const action=f.sent.at(-1),count=f.sent.length;
 assert.equal(f.host.update(),true);assert.equal(f.sent.length,count,'refresh waits for the navigation result');assert.equal(f.host.active,action.id);
 const response=f.reply(action,{nextTarget:{kind:'bot',id:'bub'},remoteReady:false});delete response.result.shown;
 f.host.receive(response);assert.equal(f.sent.at(-1).m,'view.open');assert.deepEqual(f.sent.at(-1).p.target,{kind:'bot',id:'bub'});
 const open=f.sent.at(-1);f.host.receive(f.reply(open));assert.equal(f.host.ready,true);assert.deepEqual(f.host.target,{kind:'bot',id:'bub'});assert.equal(f.host.receive(response),false);
});
test('a changed notification cannot discard a travelling navigation action',async()=>{
 const f=await fixture();f.open();f.host.request('uber','bub');const action=f.sent.at(-1),count=f.sent.length;
 f.host.receive({t:'view.changed',version:1,generation:'7',epoch:'actual',viewId:'view-A',target:{kind:'thread',id:'A'},viewRevision:'2'});
 assert.equal(f.sent.length,count,'notification defers its refresh behind navigation');assert.equal(f.host.active,action.id);
 const response=f.reply(action,{nextTarget:{kind:'bot',id:'bub'},remoteReady:false});delete response.result.shown;
 f.host.receive(response);assert.equal(f.sent.at(-1).m,'view.open');assert.deepEqual(f.sent.at(-1).p.target,{kind:'bot',id:'bub'});
 f.host.receive(f.reply());assert.equal(f.host.ready,true);assert.deepEqual(f.host.target,{kind:'bot',id:'bub'});assert.equal(f.host.receive(response),false);
});
test('message chunks assemble only their exact owner/version/cursor and never confer view readiness',async()=>{
 const f=await fixture();f.host.greeting(f.greeting);f.host.receive(f.reply(undefined,{window:{ids:['row'],older:'',hasMore:false,anchor:{id:'row',offset:0},partial:[{id:'row',cursor:'after-preview',reason:'byte-limit'}]}}));
 f.host.messageFromStart('row');const first=f.reply(undefined,{message:{id:'row',thread:'A',version:'7',text:'日本語 ',cursor:'chunk-2',hasMore:true,complete:false}});delete first.result.shown;delete first.result.session;delete first.result.window;
 const count=f.views.length;f.host.receive(first);assert.equal(f.messages.length,1);assert.equal(f.messages[0].complete,false);assert.equal(f.views.length,count);assert.equal(f.host.ready,true);
 assert.equal(f.host.message('row','wrong-cursor'),false);f.host.message('row','chunk-2');const last=f.reply(undefined,{message:{id:'row',thread:'A',version:'7',text:'👩‍💻 é',cursor:'',hasMore:false,complete:true}});delete last.result.shown;delete last.result.session;delete last.result.window;
 f.host.receive(last);assert.equal(f.messages.at(-1).text,'日本語 👩‍💻 é');assert.equal(f.messages.at(-1).complete,true);assert.equal(f.host.receive(last),false);
});
test('a continuation starting beyond the initial preview is not called the complete body',async()=>{
 const f=await fixture();f.host.greeting(f.greeting);f.host.receive(f.reply(undefined,{window:{ids:['row'],older:'',hasMore:false,anchor:{id:'row',offset:0},partial:[{id:'row',cursor:'after-preview',reason:'byte-limit'}]}}));
 f.host.message('row','after-preview');f.host.receive(f.reply(undefined,{message:{id:'row',thread:'A',version:'7',text:'tail',cursor:'',hasMore:false,complete:true}}));assert.equal(f.messages.at(-1).complete,false);assert.equal(f.messages.at(-1).fromStart,false);
});
test('explicit query cancellation never cancels an ordinary action',async()=>{
 const f=await fixture();f.open();f.host.search('query');const query=f.sent.at(-1);assert.equal(f.host.cancel(query.id),true);assert.equal(f.sent.at(-1).m,'view.cancel');
 f.host.request('send','');const action=f.sent.at(-1);assert.equal(f.host.cancel(action.id),false);
});
test('double tap cannot queue a duplicate of the unchanged in-flight draft',async()=>{
 const f=await fixture();f.open();f.host.request('send','');f.host.request('send','');const action=f.sent.at(-1);
 f.host.receive(f.reply(action,{session:{sel:'A',drafts:{A:''},rid:20},cmds:[{type:'send',text:JSON.stringify({id:20,m:'message.send',p:{thread:'A'}})},{type:'keep',thread:'A',text:''}]}));
 assert.equal(f.sent.filter(r=>r.m==='view.action').length,1);assert.equal(f.commands.filter(c=>c.type==='send').length,1);
});
test('a late failed-send ACK cannot restore over a newer deliberately emptied draft',async()=>{
 const f=await fixture();f.open();f.host.request('send','');f.host.receive(f.reply(undefined,{session:{sel:'A',drafts:{A:''},rid:20},cmds:[{type:'send',text:JSON.stringify({id:20,m:'message.send',p:{thread:'A'}})},{type:'keep',thread:'A',text:''}]}));
 f.host.request('draft','new');f.host.request('draft','');f.api.ack=s=>({...s,drafts:{...s.drafts,A:'old failed send'}});f.host.ack({id:20,ok:false});assert.equal(f.getSession().drafts.A,'');assert.equal(f.disk.get('A'),'');
});
test('a backend-restored failed-send draft is persisted when no newer edit overtook it',async()=>{
 const f=await fixture();f.open();f.host.request('send','');f.host.receive(f.reply(undefined,{session:{sel:'A',drafts:{A:''},rid:20},cmds:[{type:'send',text:JSON.stringify({id:20,m:'message.send',p:{thread:'A'}})},{type:'keep',thread:'A',text:''}]}));
 f.api.ack=s=>({...s,drafts:{...s.drafts,A:'restored 日本語'}});f.host.ack({id:20,ok:false});assert.equal(f.disk.get('A'),'restored 日本語');assert.equal(f.getSession().drafts.A,'restored 日本語');
});
test('awaited attachment admission requires its matching current authoritative action response',async()=>{
 const f=await fixture();f.open();const admitted=f.host.perform('attach-up','metadata');f.host.receive(f.reply());assert.equal(await admitted,true);
 const stale=f.host.perform('attach-up','other');const action=f.sent.at(-1);f.host.request('select','B');f.host.receive(f.reply());f.host.receive(f.reply(action));assert.equal(await stale,false);
});
test('disconnect settles pending attachment admission without granting upload or retrying',async()=>{
 const f=await fixture();f.open();const admitted=f.host.perform('attach-up','metadata');const count=f.sent.length;f.host.disconnect();assert.equal(await admitted,false);assert.equal(f.sent.length,count);
});
test('form ordering: long Unicode typing coalesces before one exact field then Send',async()=>{
 const f=await fixture();f.open();const text='日本語 👩‍💻 é\n'.repeat(300);
 for(let i=1;i<=text.length;i++)f.host.request('bfield','mrreply\x1f'+text.slice(0,i));
 assert.equal(f.sent.length,1);assert.equal(f.host.fieldValue('mrreply','stale'),text);
 f.host.request('mail-send','');assert.equal(f.sent.length,2);assert.equal(f.sent.at(-1).p.action,'bfield');assert.equal(f.sent.at(-1).p.value,'mrreply\x1f'+text);
 f.host.receive(f.reply());assert.equal(f.sent.length,3);assert.equal(f.sent.at(-1).p.action,'mail-send');
 f.host.receive(f.reply());assert.equal(f.host.receive(f.reply()),false);assert.equal(f.sent.filter(r=>r.p.action==='mail-send').length,1);
});
test('form ordering: new text after Send cannot alter captured field or be overwritten by its reply',async()=>{
 const f=await fixture();f.open();f.host.request('bfield','mrreply\x1ffirst');f.host.request('mail-send','');const field=f.sent.at(-1);
 f.host.request('bfield','mrreply\x1fnew 日本語');f.host.receive(f.reply(field));const send=f.sent.at(-1);assert.equal(send.p.action,'mail-send');
 assert.equal(f.host.fieldValue('mrreply','server first'),'new 日本語');f.host.receive(f.reply(send));assert.equal(f.sent.at(-1).p.action,'bfield');assert.equal(f.sent.at(-1).p.value,'mrreply\x1fnew 日本語');
 assert.equal(f.host.fieldValue('mrreply','server cleared'),'new 日本語');f.host.receive(f.reply());assert.equal(f.host.fieldValue('mrreply','server latest'),'server latest');
});
test('form ordering: a refresh cannot erase an unsent local field, including deliberate empty',async()=>{
 const f=await fixture();f.open();f.host.request('bfield','mrreply\x1f');f.host.update();f.host.receive(f.reply());
 assert.equal(f.host.fieldValue('mrreply','old body'),'');f.flushFields();assert.equal(f.sent.at(-1).p.value,'mrreply\x1f');
});
test('form ordering: changing owner discards unstarted field actions and local overlays',async()=>{
 const f=await fixture();f.open();f.host.request('bfield','secret\x1fprivate');f.host.request('select','B');f.flushFields();f.host.receive(f.reply());
 assert.equal(f.host.fieldValue('secret','B value'),'B value');assert(!f.sent.some(r=>r.p.action==='bfield'));
});
test('form ordering: pending field failure retains local text and never sends the queued mail',async()=>{
 const f=await fixture();f.open();f.host.request('bfield','mrreply\x1fkeep');f.host.request('mail-send','');const request=f.sent.at(-1);
 f.host.receive({t:'reply',id:request.id,ok:false,error:{code:'UNAVAILABLE'}});assert.equal(f.host.ready,false);assert.equal(f.host.fieldValue('mrreply',''),'keep');assert(!f.sent.some(r=>r.p.action==='mail-send'));
});
test('draft reconciliation: pre-Keep projection cannot resurrect successfully sent text',async()=>{
 const f=await fixture();f.open();f.host.request('send','');const send=f.sent.at(-1);f.host.update();const update=f.sent.at(-1),old=f.reply(update);
 f.host.receive(f.reply(send,{session:{sel:'A',drafts:{A:''},rid:20},cmds:[{type:'keep',thread:'A',text:''}]}));assert.equal(f.getSession().drafts.A,'');
 f.host.receive(old);assert.equal(f.getSession().drafts.A,'');assert.equal(f.disk.get('A'),'');
});
test('draft reconciliation: failure restoration survives a projection captured after Keep',async()=>{
 const f=await fixture();f.open();f.host.request('send','');f.host.receive(f.reply(undefined,{session:{sel:'A',drafts:{A:''},rid:20},cmds:[{type:'send',text:JSON.stringify({id:20,m:'message.send',p:{thread:'A'}})},{type:'keep',thread:'A',text:''}]}));
 f.host.update();const response=f.reply();f.api.ack=s=>({...s,drafts:{...s.drafts,A:'restored 日本語'}});f.host.ack({id:20,ok:false});
 // A delayed projection of the old cleared draft must not undo the domain ACK.
 f.api.ack=s=>s;f.host.receive(response);assert.equal(f.getSession().drafts.A,'restored 日本語');assert.equal(f.disk.get('A'),'restored 日本語');
});
test('form ordering: refresh waits for a field; failure blocks dependent Send and rejects delayed success',async()=>{
 const f=await fixture();f.open();f.host.request('bfield','mrreply\x1fkeep');f.host.request('mail-send','');const field=f.sent.at(-1);
 const delayed=f.reply(field),count=f.sent.length;f.host.update();assert.equal(f.sent.length,count,'refresh cannot overtake the pending field');
 f.host.receive({t:'reply',id:field.id,ok:false,error:{code:'UNAVAILABLE'}});assert.equal(f.host.receive(delayed),false);
 assert.equal(f.host.ready,false);assert.equal(f.host.actions.length,0);assert(!f.sent.some(r=>r.p.action==='mail-send'));assert.equal(f.host.fieldValue('mrreply',''),'keep');
});
test('form ordering: double tap sends the captured Mailroom reply once',async()=>{
 const f=await fixture();f.open();f.host.request('bfield','mrreply\x1freply');f.host.request('mail-send','');f.host.request('mail-send','');
 f.host.receive(f.reply());f.host.receive(f.reply());assert.equal(f.sent.filter(r=>r.p.action==='mail-send').length,1);
});
test('native adapter: mobile renderer uses supplied normal overlay/result validators and exact decode context',async()=>{
 let validations=0,results=0,decoded;
 const f=await fixture({renderer:'mobile',validateResult:(r,input)=>{results++;return r.shown?.type==='native-overlay'&&input.target.id==='A';},validatePresentation:()=>{validations++;},decode:(shown,result,input)=>{decoded={shown,result,input};return shown;}});
 f.host.greeting(f.greeting);const response=f.reply(undefined,{shown:{type:'native-overlay',text:'日本語'}});f.host.receive(response);
 assert.equal(f.host.ready,true);assert.equal(results,1);assert.equal(validations,1);assert.equal(decoded.result,response.result);assert.equal(decoded.input.target.id,'A');assert.equal(decoded.shown.text,'日本語');
 f.host.adoptLocalSession(f.getSession());assert.equal(validations,2);
 const state=f.states.at(-1);assert.equal(state.requestRevision,response.result.requestRevision);assert.equal(state.viewRevision,'1');assert.equal(state.viewId,'view-A');
});
test('native adapter: presenting fires before each actual presentation send, never query or coalesced update',async()=>{
 const order=[];const f=await fixture({presentationRequest:r=>order.push(['presenting',r.id,r.m])});const send=f.api.send;
 f.api.send=r=>{order.push(['send',r.id,r.m]);send(r);};f.open();f.host.update();const update=f.sent.at(-1);f.host.update();f.host.search('query');
 assert.deepEqual(order.slice(0,4).map(x=>x[0]),['presenting','send','presenting','send']);assert.equal(order.filter(x=>x[0]==='presenting').length,2);assert.equal(order.at(-1)[2],'view.search');
 f.host.receive(f.reply(update));assert.equal(order.filter(x=>x[0]==='presenting').length,3);
});
test('native adapter: failed presenting callback prevents the request from leaving the adapter',async()=>{
 const f=await fixture({presentationRequest:()=>{throw Error('native unavailable');}});f.host.greeting(f.greeting);assert.equal(f.sent.length,0);assert.equal(f.host.ready,false);
});
test('native adapter: qualified thread Keep checks newer local draft but forwards original command identity',async()=>{
 const f=await fixture({threadKey:t=>'h/'+t});f.open();f.host.adoptLocalSession({...f.getSession(),sel:'h/A',drafts:{'h/A':'old'}});
 f.host.request('send','');const send=f.sent.at(-1);f.host.request('draft','new native draft');f.host.receive(f.reply(send,{cmds:[{type:'keep',thread:'A',text:''}]}));
 assert.equal(f.getSession().drafts['h/A'],'new native draft');assert.equal(f.commands.length,0);
 f.host.request('send','');f.host.receive(f.reply(undefined,{cmds:[{type:'keep',thread:'A',text:''}]}));assert.equal(f.getSession().drafts['h/A'],'');assert.equal(f.commands.at(-1).thread,'A');assert(!Object.hasOwn(f.getSession().drafts,'A'));
});
test('native adapter: JSON-safe local clone works without structuredClone in a native JS engine',async()=>{
 const saved=globalThis.structuredClone;globalThis.structuredClone=undefined;
 try{const f=await fixture({send:()=>{}});assert.equal(f.host.session.drafts.A,'original 日本語');f.host.request('draft','native 日本語');assert.equal(f.getSession().drafts.A,'native 日本語');}
 finally{globalThis.structuredClone=saved;}
});
test('mail context: only a validated current presentation reaches the local draft callback',async()=>{
 const f=await fixture();f.open();const seen=[];f.api.view=(_node,detail)=>seen.push(detail);const context={kind:'mail-draft',owner:'bot',origin:'actual',conv:'conv',recipient:'Room'};
 f.host.request('mail-open','conv');const old=f.sent.at(-1);f.host.request('select','B');f.host.receive(f.reply());assert.equal(seen.length,1);
 f.host.receive(f.reply(old,{mailDraftContext:context}));assert.equal(seen.length,1);f.host.update();f.host.receive(f.reply(undefined,{mailDraftContext:{...context,conv:'current-B'}}));assert.equal(seen.length,2);assert.equal(seen[1].mailDraftContext.conv,'current-B');assert.equal(seen[1].epoch,'actual');
 const count=seen.length;f.host.search('query');f.host.receive(f.reply(undefined,{mailDraftContext:context,search:{query:'query',rows:[],cursor:'',hasMore:false,visited:0}}));assert.equal(seen.length,count);
});
test('mail context: wrong epoch fails the entire reply before any ordinary command or draft binding',async()=>{
 const f=await fixture();f.open();f.host.request('mail-send','');const count=f.views.length;f.host.receive(f.reply(undefined,{mailDraftContext:{kind:'mail-draft',owner:'bot',origin:'foreign',conv:'conv',recipient:'Room'},cmds:[{type:'send',text:JSON.stringify({id:22,m:'mail.send',p:{}})}]}));assert.equal(f.commands.length,0);assert.equal(f.views.length,count);assert.equal(f.host.ready,false);
});
test('mail context: queued old-conversation field and Send never acquire the newly presented conversation',async()=>{
 let scope='A';const f=await fixture({operationScope:(a)=>a==='bfield'||a==='mail-send'?scope:undefined});f.open();f.host.request('mail-open','B');const open=f.sent.at(-1);f.host.request('bfield','mrreply\x1fprivate A');f.host.request('mail-send','');scope='B';f.host.receive(f.reply(open));
 assert.equal(f.host.fieldValue('mrreply','B field'),'B field');assert.equal(f.host.actions.length,0);assert(!f.sent.some(r=>r.p.action==='mail-send'||r.p.action==='bfield'));
});
test('mail context: a travelling original Send still settles once after the displayed conversation changes',async()=>{
 let scope='A';const f=await fixture({operationScope:(a)=>a==='mail-send'?scope:undefined});f.open();f.host.request('mail-send','');const send=f.sent.at(-1);scope='B';f.host.request('select','B');const open=f.sent.at(-1);assert.equal(open.m,'view.open');f.host.receive(f.reply(open));assert.equal(f.getSession().sel,'B');
 const shown=f.views.length,response=f.reply(send,{cmds:[{type:'send',text:JSON.stringify({id:24,m:'mail.send',p:{conv:'A'}})}]});f.host.receive(response);assert.equal(f.commands.length,1);assert.equal(JSON.parse(f.commands[0].text).p.conv,'A');assert.equal(f.getSession().sel,'B');assert.equal(f.views.length,shown,'the old Send cannot replace B');assert.equal(f.host.receive(response),false);assert.equal(f.commands.length,1);
});
async function partialFixture(){
 const f=await fixture(),row=(id,version)=>({id,version,cursor:'display-'+id+'-'+version,reason:'byte-limit'}),window=partial=>({ids:['row'],older:'',hasMore:false,anchor:{id:'row',offset:0},partial});
 f.host.greeting(f.greeting);f.host.receive(f.reply(undefined,{window:window([row('@live:A','7'),row('row','5')])}));
 async function read(id,version){f.host.messageFromStart(id);f.host.receive(f.reply(undefined,{message:{id,thread:'A',version,text:'first '+id,cursor:'continued-'+id,hasMore:true,complete:false}}));}
 await read('@live:A','7');await read('row','5');return{...f,row,window};
}
test('live parts: current version change retires only that assembly; next explicit More starts fresh',async()=>{
 const f=await partialFixture();f.host.update();const request=f.sent.at(-1),count=f.sent.length;f.host.receive(f.reply(request,{window:f.window([f.row('@live:A','8'),f.row('row','5')])}));
 assert.equal(f.sent.length,count);assert.equal(f.host.messages.next('@live:A'),undefined);assert.equal(f.host.messages.next('row'),'continued-row');
 assert.equal(f.host.message('@live:A',''),true);assert.equal(f.sent.at(-1).p.cursor,'');f.host.receive(f.reply(undefined,{message:{id:'@live:A',thread:'A',version:'8',text:'CURRENT 日本語',cursor:'',hasMore:false,complete:true}}));
 assert.equal(f.messages.at(-1).text,'CURRENT 日本語');assert.equal(f.messages.at(-1).complete,true);assert.equal(f.host.ready,true);
});
test('live parts: removal retires only the removed id and never triggers a body request',async()=>{
 const f=await partialFixture();f.host.update();const count=f.sent.length;f.host.receive(f.reply(undefined,{window:f.window([f.row('row','5')])}));
 assert.equal(f.sent.length,count);assert.equal(f.host.messages.next('@live:A'),undefined);assert.equal(f.host.messages.next('row'),'continued-row');assert.equal(f.host.message('@live:A',''),false);
});
test('live parts: unchanged current versions preserve their exact continuation cursors',async()=>{
 const f=await partialFixture();f.host.update();const count=f.sent.length;f.host.receive(f.reply(undefined,{window:f.window([f.row('@live:A','7'),f.row('row','5')])}));
 assert.equal(f.sent.length,count);assert.equal(f.host.messages.next('@live:A'),'continued-@live:A');assert.equal(f.host.messages.next('row'),'continued-row');
});
test('live parts: stale action and auxiliary metadata cannot retire a current assembly',async()=>{
 const f=await partialFixture();f.host.request('fold','row');const action=f.sent.at(-1);f.host.update();f.host.receive(f.reply(undefined,{window:f.window([f.row('@live:A','7'),f.row('row','5')])}));
 const reads=f.sent.filter(r=>r.m==='view.message').length;f.host.receive(f.reply(action,{window:f.window([f.row('@live:A','99')])}));assert.equal(f.host.messages.next('@live:A'),'continued-@live:A');
 f.host.search('query');f.host.receive(f.reply(undefined,{window:f.window([]),search:{query:'query',rows:[],cursor:'',hasMore:false,visited:0}}));assert.equal(f.host.messages.next('@live:A'),'continued-@live:A');assert.equal(f.sent.filter(r=>r.m==='view.message').length,reads);
});
test('request privacy: views and actions send only the selected draft, language and bounded choices',async()=>{
 const privateSession={sel:'A',rid:1,drafts:{A:'initial A',B:'PRIVATE_UNSELECTED_'.repeat(20000),'@lang':'ja','@lang.os':'ja-JP','@lang.tag':'ja','@lang.preview':'1','@f.password':'PRIVATE_FORM','@mr.read':'PRIVATE_MAIL_CACHE','@wm:pending':'PRIVATE_MAIL_STATE','@img:body':'PRIVATE_DERIVED_CACHE'},outbox:{'22':'PRIVATE_PENDING_SEND'},info:{'fold:row':'1','subs:A':'0','@f.secret':'PRIVATE_INFO'},open:{settings:true},adding:false,panel:true,error:'PRIVATE_ERROR',note:'PRIVATE_NOTE'};
 const original=JSON.stringify(privateSession),f=await fixture({session:()=>privateSession});f.open();assert.equal(f.host.ready,true);assert.equal(f.sent[0].m,'view.open');assert(Buffer.byteLength(JSON.stringify(f.sent[0]))<4096,'filter before shared request-size validation');
 const language={'@lang':'ja','@lang.os':'ja-JP','@lang.tag':'ja','@lang.preview':'1'};
 function readOnly(request){assert.deepEqual(request.p.session.drafts,{...language,A:f.getSession().drafts.A});assert.deepEqual(request.p.session.outbox,{});assert.deepEqual(request.p.session.info,{'fold:row':'1','subs:A':'0'});assert.deepEqual(request.p.session.open,{settings:true});assert.equal(request.p.session.panel,true);assert(!JSON.stringify(request).includes('PRIVATE_'));}
 readOnly(f.sent[0]);f.host.page();readOnly(f.sent.at(-1));f.host.receive(f.reply());
 f.host.request('draft','new captured A');f.host.update();readOnly(f.sent.at(-1));const stale={...f.sent.at(-1).p.session,drafts:{...language,A:'stale actor draft'}};f.host.receive(f.reply(undefined,{session:stale}));assert.equal(f.getSession().drafts.A,'new captured A');
 f.host.receive({t:'view.changed',version:1,generation:'7',epoch:'actual',viewId:'view-A',target:{kind:'thread',id:'A'},viewRevision:'2'});readOnly(f.sent.at(-1));f.host.receive(f.reply(undefined,{viewRevision:'2'}));
 assert.equal(f.getSession().drafts.B,privateSession.drafts.B);assert.deepEqual(f.getSession().outbox,privateSession.outbox);assert.equal(f.getSession().info['@f.secret'],'PRIVATE_INFO');
 f.host.request('send','');const action=f.sent.at(-1);assert.equal(action.m,'view.action');assert.deepEqual(action.p.session.drafts,{...language,A:'new captured A'});assert.deepEqual(action.p.session.outbox,{});assert(!JSON.stringify(action).includes('PRIVATE_'));assert.equal(JSON.stringify(privateSession),original);
 f.host.receive(f.reply(action,{session:{...action.p.session,drafts:{...language,A:'domain result',B:'UNRELATED STALE ACTOR DRAFT'}},cmds:[{type:'keep',thread:'A',text:''}]}));assert.equal(f.getSession().drafts.A,'');assert.equal(f.getSession().drafts.B,privateSession.drafts.B);assert.deepEqual(f.getSession().outbox,privateSession.outbox);
 for (const saved of ['initial restored 日本語','']) {
   const g=await fixture({session:()=>({sel:'A',rid:1,drafts:{A:saved,B:'other local'},outbox:{'44':'local pending'}})});g.open();assert.equal(g.getSession().drafts.A,saved,'initial saved/empty survives first read');
   g.host.page();g.host.receive(g.reply(undefined,{session:{...g.sent.at(-1).p.session,drafts:{A:'stale actor'},outbox:{}}}));assert.equal(g.getSession().drafts.A,saved,'read cannot replace initial saved/empty');
   g.host.request('send','');const sent=g.sent.at(-1);assert.deepEqual(sent.p.session.drafts,{A:saved});assert.deepEqual(sent.p.session.outbox,{});
   g.host.receive(g.reply(sent,{session:{...sent.p.session,drafts:{A:'selected domain result',B:'foreign replacement'}}}));assert.equal(g.getSession().drafts.A,'selected domain result');assert.equal(g.getSession().drafts.B,'other local');assert.deepEqual(g.getSession().outbox,{'44':'local pending'});
 }
});

test('settings navigation keeps its first click when background refresh races opening Settings',async()=>{
 const f=await fixture({prepare:s=>s});f.open();
 let accepted;
 f.host.request('flag','settings',yes=>accepted=yes);
 const action=f.sent.at(-1),link=f.host.client.links.get(f.api.hub);
 f.host.update();
 f.host.receive({t:'view.changed',version:1,generation:'7',epoch:'actual',viewId:link.viewId,target:link.target,viewRevision:'2'});
 assert.equal(f.sent.length,2,'refresh waits for Settings open result instead of retiring it');
 f.host.receive(f.reply(action,{session:{...f.getSession(),open:{settings:true}},shown:{type:'text',text:'Settings categories'}}));
 assert.equal(accepted,true,'the first click is accepted');
 assert.equal(f.views.at(-1).n.text,'Settings categories');
 assert.equal(f.sent.at(-1).m,'view.update','one deferred refresh follows the accepted result');
 assert.equal(f.sent.at(-1).p.session.open.settings,true,'refresh uses opened Settings state');
});

test('settings navigation keeps submenu on first click during background invalidation',async()=>{
 const f=await fixture({session:()=>({sel:'A',rid:1,drafts:{A:''},open:{settings:true}}),prepare:s=>s});f.open();
 let accepted;
 f.host.request('settings-page','appearance',yes=>accepted=yes);
 const action=f.sent.at(-1),link=f.host.client.links.get(f.api.hub);
 f.host.receive({t:'view.changed',version:1,generation:'7',epoch:'actual',viewId:link.viewId,target:link.target,viewRevision:'2'});
 assert.equal(f.sent.length,2,'background invalidation cannot overtake submenu navigation');
 f.host.receive(f.reply(action,{session:{...f.getSession(),drafts:{...f.getSession().drafts,'@settings.page':'appearance'}},shown:{type:'text',text:'Appearance controls'}}));
 assert.equal(accepted,true);
 assert.equal(f.views.at(-1).n.text,'Appearance controls');
 assert.equal(f.sent.filter(r=>r.m==='view.action'&&r.p.action==='settings-page').length,1,'one click sends one navigation');
 assert.equal(f.sent.at(-1).m,'view.update');
});


test('workspace actions adopt their choices before shell/background invalidation refreshes',async()=>{
 for(const [action,value,open] of [['term-toggle','100x24',{term:true}],['ws-key','term',{term:true}],['ws-palette-open','',{palette:true}],['bfield','wspalette\x1fterminal',{palette:true}]]) {
  const f=await fixture({prepare:s=>s});f.open();let accepted;
  f.host.request(action,value,yes=>accepted=yes);
  if(action==='bfield')f.flushFields();
  const request=f.sent.at(-1),link=f.host.client.links.get(f.api.hub),count=f.sent.length;
  f.host.update();
  f.host.receive({t:'view.changed',version:1,generation:'7',epoch:'actual',viewId:link.viewId,target:link.target,viewRevision:'2'});
  assert.equal(f.sent.length,count,'no refresh can retire the travelling '+action);
  f.host.receive(f.reply(request,{session:{...f.getSession(),open,drafts:{...f.getSession().drafts,'@terminal.owner':'A'}},shown:{type:'text',text:'Workspace accepted'}}));
  assert.equal(accepted,true);assert.equal(f.views.at(-1).n.text,'Workspace accepted');
  const refresh=f.sent.at(-1);assert.equal(refresh.m,'view.update');assert.deepEqual(refresh.p.session.open,open);
  assert(!Object.hasOwn(refresh.p.session.drafts,'@terminal.owner'),'terminal owner remains actor-private on requests');
  f.host.receive(f.reply(refresh));assert.deepEqual(f.getSession().open,open);
 }
});

for (const queued of [false, true]) test('panel close survives background refresh while '+(queued?'queued':'active'),async()=>{
 const f=await fixture({session:()=>({sel:'A',rid:1,drafts:{A:''},panel:true}),prepare:s=>s});f.open();
 if(queued)f.host.request('noop','');
 let accepted;f.host.request('panel','',yes=>accepted=yes);const count=f.sent.length;
 f.host.update();
 f.host.receive({t:'view.changed',version:1,generation:'7',epoch:'actual',viewId:'view-A',target:{kind:'thread',id:'A'},viewRevision:'2'});
 assert.equal(f.sent.length,count,'a refresh cannot retire the close intention');
 if(queued){f.host.receive(f.reply());assert.equal(f.sent.at(-1).p.action,'panel');}
 f.host.receive(f.reply(undefined,{session:{...f.getSession(),panel:false},shown:{type:'text',text:'Closed viewer'}}));
 assert.equal(accepted,true);assert.equal(f.getSession().panel,false);assert.equal(f.views.at(-1).n.text,'Closed viewer');
 assert.equal(f.sent.at(-1).m,'view.update');assert.equal(f.sent.at(-1).p.session.panel,false);
 f.host.receive(f.reply());assert.equal(f.getSession().panel,false);
 assert.equal(f.sent.filter(r=>r.m==='view.action'&&r.p.action==='panel').length,1,'one click closes once');
});

test('panel close and reopen remain two ordered toggles',async()=>{
 const f=await fixture({session:()=>({sel:'A',rid:1,drafts:{A:''},panel:true}),prepare:s=>s});f.open();
 f.host.request('noop','');f.host.request('panel','');f.host.request('panel','');
 assert.equal(f.host.actions.length,2,'panel toggles must not coalesce as destinations');
 f.host.receive(f.reply());assert.equal(f.sent.at(-1).p.action,'panel');
 f.host.receive(f.reply(undefined,{session:{...f.getSession(),panel:false}}));
 assert.equal(f.sent.at(-1).p.action,'panel');assert.equal(f.sent.at(-1).p.session.panel,false);
 f.host.receive(f.reply(undefined,{session:{...f.getSession(),panel:true}}));
 assert.equal(f.getSession().panel,true);assert.equal(f.sent.filter(r=>r.m==='view.action'&&r.p.action==='panel').length,2);
});

for (const queued of [false, true]) test('legacy subthread picker defers refresh while '+(queued?'queued':'active'),async()=>{
 for (const mode of ['update','changed']) {
  const f=await fixture();f.open();if(queued)f.host.request('noop','');
  f.host.request('thread-child','A');const sent=f.sent.length;
  if(mode==='update')f.host.update();
  else f.host.receive({t:'view.changed',version:1,generation:'7',epoch:'actual',viewId:'view-A',target:{kind:'thread',id:'A'},viewRevision:'2'});
  assert.equal(f.sent.length,sent,'old closed-picker session must not overtake the route action');
  if(queued){f.host.receive(f.reply());assert.equal(f.sent.at(-1).m,'view.action');assert.equal(f.sent.at(-1).p.action,'thread-child');}
  f.host.receive(f.reply(undefined,{session:{sel:'A',drafts:{A:'original 日本語'},rid:20,open:{proj:true}}}));
  assert.equal(f.getSession().open.proj,true);assert.equal(f.sent.at(-1).m,'view.update');assert.equal(f.sent.at(-1).p.session.open.proj,true);
  f.host.receive(f.reply());assert.equal(f.getSession().open.proj,true);assert.equal(f.host.ready,true);assert.equal(f.commands.length,0);
 }
});

// Changed-path autocomplete transport and action ordering, without providers.
test('composer references: debounced edits refresh with only the current draft, never an action',async()=>{
 let refresh;const f=await fixture({draftRefresh:fn=>refresh=fn});f.open();
 for(const sigil of ['@','>','%','#']){f.host.request('draft','prefix '+sigil);refresh();const request=f.sent.at(-1);assert.equal(request.m,'view.update');assert.deepEqual(request.p.session.drafts,{A:'prefix '+sigil});f.host.receive(f.reply(request));}
 assert.equal(f.commands.length,0);assert(!f.sent.some(r=>r.m==='view.action'));
 f.host.request('select','B');assert(!Object.hasOwn(f.sent.at(-1).p.session.drafts,'A'));
});
test('composer references: refresh waits for active and queued menu intentions',async()=>{
 for(const queued of [false,true])for(const action of ['skill','skill-move','skill-close']){
  const f=await fixture();f.open();if(queued)f.host.request('fold','row');
  f.host.request(action,action==='skill-move'?'down':'');const before=f.sent.length;
  f.host.update();f.host.receive({t:'view.changed',version:1,generation:'7',epoch:'actual',viewId:'view-A',target:{kind:'thread',id:'A'},viewRevision:'2'});
  assert.equal(f.sent.length,before,'no stale read may retire '+action);
  if(queued){f.host.receive(f.reply());assert.equal(f.sent.at(-1).p.action,action);}
  f.host.receive(f.reply());assert.equal(f.sent.at(-1).m,'view.update');
 }
});
test('composer references: late completion Keep cannot replace newer typing or another thread',async()=>{
 const f=await fixture();f.open();f.host.request('draft','ask @');f.host.request('skill','@Miso');const old=f.sent.at(-1);
 f.host.request('draft','newer 日本語');f.host.receive(f.reply(old,{session:{...old.p.session,drafts:{A:'ask @Miso '}},cmds:[{type:'keep',thread:'A',text:'ask @Miso '}]}));
 assert.equal(f.getSession().drafts.A,'newer 日本語');assert.equal(f.disk.get('A'),'newer 日本語');
 f.host.request('skill','@Miso');const other=f.sent.at(-1);f.host.request('select','B');f.host.request('draft','B own text');f.host.receive(f.reply());f.host.receive(f.reply(other,{cmds:[{type:'keep',thread:'A',text:'old completion'}]}));
 assert.equal(f.getSession().sel,'B');assert.equal(f.getSession().drafts.B,'B own text');
});

// A UI intention must settle before any background picture can restore its old flags.
test('local UI intentions: active and queued model and Settings clicks outrank refresh',async()=>{
 for(const queued of [false,true])for(const [action,value] of [['flag','models'],['flag','settings'],['settings-page','appearance'],['bfield','mq\u001fquery']]){
  const f=await fixture({prepare:s=>s});f.open();if(queued)f.host.request('fold','row');
  f.host.request(action,value);if(action==='bfield'&&!queued)f.flushFields();
  const before=f.sent.length,link=f.host.client.links.get(f.api.hub);
  f.host.update();f.host.receive({t:'view.changed',version:1,generation:'7',epoch:'actual',viewId:link.viewId,target:link.target,viewRevision:'2'});
  assert.equal(f.sent.length,before,'background refresh may not overtake '+(queued?'queued ':'active ')+action+':'+value);
 }
});
test('local UI intentions: earlier in-flight read cannot trigger a stale read over model opening',async()=>{
 const f=await fixture({prepare:s=>s});f.open();f.host.update();const old=f.sent.at(-1);
 let accepted;f.host.request('flag','models',yes=>accepted=yes);const action=f.sent.at(-1),before=f.sent.length;
 f.host.receive(f.reply(old));assert.equal(f.sent.length,before,'retired read must defer its follow-up until action adoption');
 f.host.receive(f.reply(action,{session:{...f.getSession(),open:{models:true}},shown:{type:'text',text:'server model choices'}}));
 assert.equal(accepted,true);assert.equal(f.views.at(-1).n.text,'server model choices');
 assert.equal(f.sent.at(-1).m,'view.update');assert.equal(f.sent.at(-1).p.session.open.models,true);
 assert.equal(f.sent.filter(r=>r.m==='view.action').length,1,'no action retries');
});

test('refresh launch: notifications coalesce into one bounded scheduled read',async()=>{
 const scheduled=[],f=await fixture({deferRefresh:fn=>scheduled.push(fn)});f.open();
 const e={t:'view.changed',version:1,generation:'7',epoch:'actual',viewId:'view-A',target:{kind:'thread',id:'A'},viewRevision:'2'};
 f.host.receive(e);f.host.receive({...e,viewRevision:'3'});assert.equal(scheduled.length,1);assert.equal(f.sent.length,1);
 scheduled.shift()();assert.equal(f.sent.length,2);assert.equal(f.sent.at(-1).m,'view.update');
 f.host.receive(f.reply(undefined,{viewRevision:'3'}));assert.equal(f.sent.length,2);
});
test('refresh launch: immediate click outranks unsent notification read and settles once',async()=>{
 const scheduled=[],f=await fixture({deferRefresh:fn=>scheduled.push(fn)});f.open();
 f.host.receive({t:'view.changed',version:1,generation:'7',epoch:'actual',viewId:'view-A',target:{kind:'thread',id:'A'},viewRevision:'2'});
 f.host.request('flag','settings');const action=f.sent.at(-1);assert.equal(action.m,'view.action');assert.equal(f.sent.length,2);scheduled.shift()();assert.equal(f.sent.length,2);
 const r=f.reply(action,{viewRevision:'2',session:{...action.p.session,open:{settings:true}}});f.host.receive(r);assert.equal(f.host.receive(r),false);assert.equal(f.sent.length,2);assert.equal(f.getSession().open.settings,true);
});
test('refresh launch: uncovered revision still refreshes accepted action session and latest draft',async()=>{
 const scheduled=[],f=await fixture({deferRefresh:fn=>scheduled.push(fn)});f.open();f.host.request('flag','settings');const action=f.sent.at(-1);
 f.host.receive({t:'view.changed',version:1,generation:'7',epoch:'actual',viewId:'view-A',target:{kind:'thread',id:'A'},viewRevision:'3'});
 f.host.request('draft','new local 日本語');f.host.receive(f.reply(action,{viewRevision:'2',session:{...action.p.session,open:{settings:true}}}));assert.equal(f.sent.length,2);
 scheduled.shift()();const update=f.sent.at(-1);assert.equal(update.m,'view.update');assert.equal(update.p.session.open.settings,true);assert.equal(update.p.session.drafts.A,'new local 日本語');
 f.host.receive(f.reply(update,{viewRevision:'3'}));assert.equal(f.sent.length,3);
});
test('refresh launch: selection and disconnect retire scheduled callbacks without request replay',async()=>{
 for(const change of ['select','disconnect']){const scheduled=[],f=await fixture({deferRefresh:fn=>scheduled.push(fn)});f.open();f.host.receive({t:'view.changed',version:1,generation:'7',epoch:'actual',viewId:'view-A',target:{kind:'thread',id:'A'},viewRevision:'2'});
 if(change==='select'){f.host.request('select','B');f.host.receive(f.reply());}else f.host.disconnect();
 const count=f.sent.length;scheduled.shift()();assert.equal(f.sent.length,count);}
});
test('refresh launch: malformed foreign and covered notifications schedule nothing',async()=>{
 const scheduled=[],f=await fixture({deferRefresh:fn=>scheduled.push(fn)});f.open();const e={t:'view.changed',version:1,generation:'7',epoch:'actual',viewId:'view-A',target:{kind:'thread',id:'A'},viewRevision:'1'};
 for(const value of [e,{...e,epoch:'foreign',viewRevision:'2'},{...e,viewId:'old',viewRevision:'2'},{...e,viewRevision:'bad'}])f.host.receive(value);
 assert.equal(scheduled.length,0);assert.equal(f.sent.length,1);assert.equal(f.host.ready,true);
});
test('refresh launch: compact input does not consume scheduled output invalidation',async()=>{
 const scheduled=[],f=await fixture({deferRefresh:fn=>scheduled.push(fn),modelMenuReady:()=>true});f.open();f.host.receive({t:'view.changed',version:1,generation:'7',epoch:'actual',viewId:'view-A',target:{kind:'thread',id:'A'},viewRevision:'2'});
 f.host.request('flag','models');const action=f.sent.at(-1),r=f.reply(action,{modelMenuInput:true,viewRevision:'2',session:{...action.p.session,open:{models:true}}});delete r.result.shown;delete r.result.window;f.host.receive(r);
 scheduled.shift()();assert.equal(f.sent.at(-1).m,'view.update');assert.equal(f.sent.at(-1).p.session.open.models,true);assert.equal(f.host.ready,true);
});
test('history ordering: an action waits for a page on its way, so the page is still shown as one; Stop does not wait',async()=>{
 const f=await fixture();f.open();f.host.page();const req=f.sent.at(-1),count=f.sent.length;
 f.host.request('fab-look','');assert.equal(f.sent.length,count,'the action waits for the page');
 f.host.receive(f.reply(req));assert.equal(f.views.at(-1).detail.page.direction,'older','the page is drawn as a page, its bridge kept');
 assert.equal(f.sent.at(-1).p.action,'fab-look','then the action goes');
 f.host.receive(f.reply());f.host.page();f.host.request('stop','');assert.equal(f.sent.at(-1).p.action,'stop','Stop overtakes a page');
});
test('history independence: older, newer and newest page while an automatic ordering lookup is pending',async()=>{
 for(const direction of ['older','newer','newest'])for(const actionFirst of [true,false]){
  const f=await fixture();f.open();
  if(direction!=='older'){f.host.page();f.host.receive(f.reply());}
  const savedDraft=f.getSession().drafts.A;
  f.host.request('fab-look','');const card=f.sent.at(-1),count=f.sent.length;
  assert.equal(card.m,'view.action');assert.equal(f.host.active,card.id);
  assert.equal(direction==='older'?f.host.page():f.host[direction](),true,'paging does not wait for the unrelated lookup');
  const page=f.sent.at(-1);assert.equal(page.m,'view.page');assert.equal(f.sent.length,count+1);
  assert.equal(f.host.page(),false,'only one page travels at a time');
  const before=f.views.length,cardReply=f.reply(card,{shown:{type:'text',text:'OLD CARD PICTURE'},session:{...card.p.session,rid:100,drafts:{A:'old actor draft'}},cmds:[{type:'send',text:JSON.stringify({id:90,m:'fab.look',p:{thread:'A'}})}]});
  if(actionFirst){f.host.receive(cardReply);assert.equal(f.views.length,before);f.host.receive(f.reply(page));}
  else{f.host.receive(f.reply(page));f.host.receive(cardReply);}
  assert.equal(f.views.length,before+1);assert.equal(f.views.at(-1).detail.page.direction,direction);
  assert.equal(f.views.at(-1).n.text,'VIEW A');assert.equal(f.getSession().drafts.A,savedDraft);
  assert.equal(f.commands.filter(c=>c.type==='send').length,1);assert.equal(f.host.receive(cardReply),false);
  assert.equal(f.host.active,null);assert.equal(f.host.ready,true);
 }
});
test('history independence: failed or expired ordering lookup cannot revoke a page or its draft',async()=>{
 for(const failure of ['reply','timeout']){
  const f=await fixture();f.open();f.host.request('fab-look','');const card=f.sent.at(-1);
  if(failure==='timeout')f.tick(4000);
  assert.equal(f.host.page(),true);const page=f.sent.at(-1);
  f.host.request('draft','new 日本語 draft');
  if(failure==='reply')f.host.receive({t:'reply',id:card.id,ok:false,error:{code:'UNAVAILABLE'}});
  else f.tick(1001);
  assert.equal(f.host.ready,true);assert.equal(f.host.paging.id,page.id);
  f.host.receive(f.reply(page));assert.equal(f.host.ready,true);assert.equal(f.getSession().drafts.A,'new 日本語 draft');
  assert.equal(f.views.at(-1).detail.page.direction,'older');
 }
});
test('history independence: failed page restores its cursor while the unrelated lookup remains pending',async()=>{
 const f=await fixture();f.open();f.host.request('fab-look','');const card=f.sent.at(-1);
 assert.equal(f.host.page(),true);const page=f.sent.at(-1);
 f.host.receive({t:'reply',id:page.id,ok:false,error:{code:'STALE_CURSOR'}});
 assert.equal(f.host.ready,true);assert.equal(f.host.atNewest(),true);assert.equal(f.host.pageError.direction,'older');
 assert.equal(f.host.active,card.id);assert.equal(f.host.retryPage(),true);
 f.host.receive(f.reply());f.host.receive(f.reply(card));assert.equal(f.host.ready,true);assert.equal(f.host.atNewest(),false);
});
test('history independence: presentation actions, queued actions, native and nonempty lookups retain their fence',async()=>{
 for(const [action,value]of [['flag','settings'],['fold','row'],['send',''],['ws-go','files'],['fab-look','nonempty']]){
  const f=await fixture();f.open();f.host.request(action,value);const count=f.sent.length;
  assert.equal(f.host.page(),false);assert.equal(f.sent.length,count);
 }
 const queued=await fixture();queued.open();queued.host.request('fab-look','');queued.host.request('flag','settings');
 const count=queued.sent.length;assert.equal(queued.host.page(),false);assert.equal(queued.sent.length,count);
 const native=await fixture({renderer:'mobile'});native.open();native.host.request('fab-look','');assert.equal(native.host.page(),false);
});
test('history independence: queued lookup still waits for the accepted page and canonical refresh keeps action ownership',async()=>{
 const f=await fixture();f.open();f.host.request('fab-look','');const first=f.sent.at(-1);
 f.host.request('fab-look','');assert.equal(f.host.page(),true);const page=f.sent.at(-1),count=f.sent.length;
 f.host.update();assert.equal(f.sent.length,count,'background refresh still waits for the action');
 f.host.receive(f.reply(first));assert.equal(f.sent.length,count,'queued lookup cannot retire a page in flight');
 f.host.receive(f.reply(page));assert.equal(f.views.at(-1).detail.page.direction,'older');
 assert.equal(f.sent.at(-1).m,'view.action');assert.equal(f.sent.at(-1).p.action,'fab-look');
});
test('the away mark rides on every view request for the shown thread; a switch lets it go',async()=>{
 const f=await fixture();f.open();assert.equal(f.sent.at(-1).p.away,'');
 f.host.away('A','m7');assert.equal(f.sent.at(-1).m,'view.update');assert.equal(f.sent.at(-1).p.away,'m7');f.host.receive(f.reply());
 const n=f.sent.length;f.host.away('A','m7');assert.equal(f.sent.length,n,'the same mark asks for nothing');
 f.host.away('A','');assert.equal(f.sent.at(-1).p.away,'','back at the end');f.host.receive(f.reply());
 f.host.away('A','m9');f.host.receive(f.reply());f.host.request('select','B');assert.equal(f.sent.at(-1).m,'view.open');assert.equal(f.sent.at(-1).p.away,'');assert.equal(f.host.awayMark,null);
});
test('the new-messages pill on an earlier page goes straight to the newest page',async()=>{
 const f=await fixture();f.open();f.host.page();f.host.receive(f.reply());f.host.page();f.host.receive(f.reply());assert.equal(f.host.atNewest(),false);
 assert.equal(f.host.newest(),true);const req=f.sent.at(-1);assert.equal(req.m,'view.page');assert.equal(req.p.cursor,'');assert.equal(f.host.atNewest(),true);
 f.host.receive(f.reply(req));assert.equal(f.views.at(-1).detail.page.direction,'newest');assert.equal(f.host.newest(),false,'at the newest there is nowhere to go');
});
// A search result once named a subagent's log ("@sub:<thread>"), which the
// hub has no view of: the page stopped on "Conversation unavailable", and
// every reload asked for it again. A refused selection now goes where the
// page can show (api.refused), and only a view too big still stops.
test('a refused selection goes back to a view the page can show, and is not retried',async()=>{
 const refused=[];let f;f=await fixture({refused:(target,reason)=>{refused.push([target,reason]);return f.host.request('select','A');}});
 f.open();f.host.request('select','@sub:A');const open=f.sent.at(-1);assert.equal(open.m,'view.open');assert.deepEqual(open.p.target,{kind:'thread',id:'@sub:A'});
 f.host.receive({t:'reply',id:open.id,ok:false,error:{code:'STALE_VIEW',retryable:false}});
 assert.deepEqual(refused,[[{kind:'thread',id:'@sub:A'},'STALE_VIEW']]);
 assert.equal(f.states.some(s=>s.phase==='error'),false,'no stuck "Conversation unavailable"');
 const back=f.sent.at(-1);assert.equal(back.m,'view.open');assert.deepEqual(back.p.target,{kind:'thread',id:'A'});
 f.host.receive(f.reply(back));assert.equal(f.host.ready,true);assert.equal(f.getSession().sel,'A');
 assert.equal(f.sent.filter(x=>x.p.target?.id==='@sub:A').length,1,'the refused view is asked for once');
});
test('a refused first view (a reload of a saved selection) falls back too',async()=>{
 const refused=[];let f;f=await fixture({refused:(target,reason)=>{refused.push(target.id);return f.host.request('select','');}});
 f.host.greeting(f.greeting);const open=f.sent.at(-1);f.host.receive({t:'reply',id:open.id,ok:false,error:{code:'STALE_VIEW'}});
 assert.deepEqual(refused,['A']);assert.deepEqual(f.sent.at(-1).p.target,{kind:'recent',id:''});assert.equal(f.states.some(s=>s.phase==='error'),false);
});
test('a view too big, or one refused with no way back, still says so',async()=>{
 const f=await fixture({refused:()=>{throw Error('not asked');}});f.host.greeting(f.greeting);
 f.host.receive({t:'reply',id:f.sent.at(-1).id,ok:false,error:{code:'LIMIT'}});assert.equal(f.states.at(-1).phase,'error');assert.equal(f.states.at(-1).reason,'LIMIT');
 const g=await fixture({refused:()=>false});g.host.greeting(g.greeting);
 g.host.receive({t:'reply',id:g.sent.at(-1).id,ok:false,error:{code:'STALE_VIEW'}});assert.equal(g.states.at(-1).phase,'error');assert.equal(g.states.at(-1).reason,'STALE_VIEW');
});
test('an action refused on a view on show is not a refused selection',async()=>{
 const refused=[],f=await fixture({refused:t=>{refused.push(t);return true;}});f.open();f.host.request('flag','settings');const req=f.sent.at(-1);
 f.host.receive({t:'reply',id:req.id,ok:false,error:{code:'STALE_VIEW'}});assert.deepEqual(refused,[]);assert.equal(f.states.at(-1).phase,'error');
});
test('refusedFallback: the thread on show before, else the project list, never a refused one',async()=>{
 const {refusedFallback}=await import(process.env.THIN_HOST_MODULE||'../../src/web/thin-host.js');
 const thread=id=>({kind:'thread',id});
 assert.equal(refusedFallback(thread('@sub:a'),thread('a'),new Set(['thread:@sub:a'])),'a');
 assert.equal(refusedFallback(thread('@sub:a'),null,new Set(['thread:@sub:a'])),'');
 assert.equal(refusedFallback(thread('b'),thread('b'),new Set(['thread:b'])),'');
 assert.equal(refusedFallback(thread('c'),thread('b'),new Set(['thread:b','thread:c'])),'');
 assert.equal(refusedFallback(thread('c'),{kind:'bot',id:'x'},new Set(['thread:c'])),'');
 assert.equal(refusedFallback({kind:'recent',id:''},thread('b'),new Set(['recent:'])),null);
 assert.equal(refusedFallback(thread('c'),null,new Set(['recent:','thread:c'])),null);
});

test('send Keep consumes only its captured prefix when fast typing overtakes it',async()=>{
 for(const action of ['send','send-alt']){
  const f=await fixture();f.open();const original=f.getSession().drafts.A;
  f.host.request(action,'');const sent=f.sent.at(-1);
  f.host.request('draft',original+'next 日本語');
  f.host.receive(f.reply(sent,{session:{sel:'A',drafts:{A:''},rid:20},cmds:[{type:'keep',thread:'A',text:''}]}));
  assert.equal(f.getSession().drafts.A,'next 日本語');assert.equal(f.disk.get('A'),'next 日本語');
  f.host.request('send','');assert.equal(f.sent.at(-1).p.session.drafts.A,'next 日本語');
 }
});
test('two fast Enters capture separate messages even before the first Keep',async()=>{
 const f=await fixture();f.open();const original=f.getSession().drafts.A;
 f.host.request('send','');const first=f.sent.at(-1);
 f.host.request('draft',original+'second');f.host.request('send','');
 f.host.request('draft',original+'second third');
 f.host.receive(f.reply(first,{session:{sel:'A',drafts:{A:''},rid:20},cmds:[{type:'keep',thread:'A',text:''}]}));
 const second=f.sent.at(-1);assert.equal(second.p.session.drafts.A,'second');
 f.host.receive(f.reply(second,{session:{sel:'A',drafts:{A:''},rid:21},cmds:[{type:'keep',thread:'A',text:''}]}));
 assert.equal(f.getSession().drafts.A,' third');assert.equal(f.disk.get('A'),' third');
});
test('send prefix reconciliation stays with its thread and never trims a completion',async()=>{
 const f=await fixture();f.open();const original=f.getSession().drafts.A;
 f.host.request('send','');const first=f.sent.at(-1);f.host.request('draft',original+'tail');f.host.request('select','B');f.host.receive(f.reply());
 f.host.request('draft','B draft');f.host.receive(f.reply(first,{session:{sel:'A',drafts:{A:''},rid:20},cmds:[{type:'keep',thread:'A',text:''}]}));
 assert.equal(f.getSession().sel,'B');assert.equal(f.getSession().drafts.B,'B draft');assert.equal(f.getSession().drafts.A,'tail');
 const g=await fixture();g.open();g.host.request('skill','');const completion=g.sent.at(-1);g.host.request('draft',original+'tail');
 g.host.receive(g.reply(completion,{cmds:[{type:'keep',thread:'A',text:''}]}));assert.equal(g.getSession().drafts.A,original+'tail');
});
test('an accepted send never takes text the person replaced it with, however it starts',async()=>{
 const f=await fixture();f.open();const original=f.getSession().drafts.A;
 f.host.request('send','');const sent=f.sent.at(-1);
 // select all, then paste a new message that happens to start with the sent one
 f.host.touched(0);f.host.request('draft',original+'story is useful');
 f.host.receive(f.reply(sent,{session:{sel:'A',drafts:{A:''},rid:20},cmds:[{type:'keep',thread:'A',text:''}]}));
 assert.equal(f.getSession().drafts.A,original+'story is useful');assert.equal(f.disk.get('A'),original+'story is useful');
 // select all and type it a letter at a time: the first letter already stops it
 const g=await fixture();g.open();g.host.request('send','');const again=g.sent.at(-1);
 g.host.touched(0);g.host.request('draft','o');g.host.request('draft',original);g.host.request('draft',original+' more');
 g.host.receive(g.reply(again,{session:{sel:'A',drafts:{A:''},rid:20},cmds:[{type:'keep',thread:'A',text:''}]}));
 assert.equal(g.getSession().drafts.A,original+' more');
});
test('a Backspace into the sent text leaves the field as the person made it; typing after it does not',async()=>{
 const f=await fixture();f.open();const original=f.getSession().drafts.A;
 f.host.request('send','');const sent=f.sent.at(-1);
 f.host.touched(original.length);f.host.request('draft',original+'x');f.host.touched(original.length);f.host.request('draft',original);
 f.host.touched(original.length-1);f.host.request('draft',original.slice(0,-1)+'語!');
 f.host.receive(f.reply(sent,{session:{sel:'A',drafts:{A:''},rid:20},cmds:[{type:'keep',thread:'A',text:''}]}));
 assert.equal(f.getSession().drafts.A,original.slice(0,-1)+'語!');
});
test('during an IME composition the sent text comes off only when it ends; queued sends lose it at once',async()=>{
 let composing=true;const f=await fixture({composing:()=>composing});f.open();const original=f.getSession().drafts.A;
 f.host.request('send','');const first=f.sent.at(-1);
 f.host.touched(original.length);f.host.request('draft',original+' 次');f.host.request('send','');
 f.host.touched(original.length+2);f.host.request('draft',original+' 次 に');
 f.host.receive(f.reply(first,{session:{sel:'A',drafts:{A:''},rid:20},cmds:[{type:'keep',thread:'A',text:''}]}));
 assert.equal(f.getSession().drafts.A,original+' 次 に','the field is not written while composing');
 const second=f.sent.at(-1);assert.equal(second.p.session.drafts.A,' 次','the queued send carries only what followed');
 f.host.request('draft',original+' 次 にほん');
 composing=false;f.host.composed();
 assert.equal(f.getSession().drafts.A,' 次 にほん');assert.equal(f.disk.get('A'),' 次 にほん');
 f.host.receive(f.reply(second,{session:{sel:'A',drafts:{A:''},rid:21},cmds:[{type:'keep',thread:'A',text:''}]}));
 assert.equal(f.getSession().drafts.A,' にほん');
});
