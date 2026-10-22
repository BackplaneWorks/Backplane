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
test('greeting alone is not ready; matching current result renders without any canonical history request',async()=>{
 const f=await fixture();f.host.greeting(f.greeting);assert.equal(f.host.ready,false);assert.equal(f.sent.length,1);assert.equal(f.sent[0].m,'view.open');
 f.host.receive(f.reply());assert.equal(f.host.ready,true);assert.equal(f.views.length,1);assert(f.sent.every(x=>x.m.startsWith('view.')));
});
test('a draft typed while first view travels remains local and overtakes its old response',async()=>{
 const f=await fixture();f.host.greeting(f.greeting);const old=f.reply();f.host.request('draft','NEW 日本語 👩‍💻');f.host.receive(old);
 assert.equal(f.getSession().drafts.A,'NEW 日本語 👩‍💻');assert.equal(f.disk.get('A'),'NEW 日本語 👩‍💻');assert.equal(f.sent.length,1);
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
test('an obsolete action nextTarget cannot navigate away from the newer selected thread',async()=>{
 const f=await fixture();f.open();f.host.request('bot-open','bot-C');const action=f.sent.at(-1);f.host.request('select','B');f.host.receive(f.reply());
 const response=f.reply(action,{nextTarget:{kind:'bot',id:'bot-C'},remoteReady:false});delete response.result.shown;const count=f.sent.length;f.host.receive(response);assert.equal(f.sent.length,count);assert.equal(f.getSession().sel,'B');assert.equal(f.host.ready,true);
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
test('form ordering: stale failed field cannot release a dependent Send after a newer refresh',async()=>{
 const f=await fixture();f.open();f.host.request('bfield','mrreply\x1fkeep');f.host.request('mail-send','');const field=f.sent.at(-1);
 f.host.update();f.host.receive(f.reply());f.host.receive({t:'reply',id:field.id,ok:false,error:{code:'UNAVAILABLE'}});
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
 let scope='A';const f=await fixture({operationScope:(a)=>a==='mail-send'?scope:undefined});f.open();f.host.request('mail-send','');const send=f.sent.at(-1);scope='B';f.host.update();f.host.receive(f.reply());
 const response=f.reply(send,{cmds:[{type:'send',text:JSON.stringify({id:24,m:'mail.send',p:{conv:'A'}})}]});f.host.receive(response);assert.equal(f.commands.length,1);assert.equal(JSON.parse(f.commands[0].text).p.conv,'A');assert.equal(f.host.receive(response),false);
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
test('request privacy: read views send only language and bounded choices; actions send only the selected captured draft',async()=>{
 const privateSession={sel:'A',rid:1,drafts:{A:'initial A',B:'PRIVATE_UNSELECTED_'.repeat(20000),'@lang':'ja','@lang.os':'ja-JP','@lang.tag':'ja','@lang.preview':'1','@f.password':'PRIVATE_FORM','@mr.read':'PRIVATE_MAIL_CACHE','@wm:pending':'PRIVATE_MAIL_STATE','@img:body':'PRIVATE_DERIVED_CACHE'},outbox:{'22':'PRIVATE_PENDING_SEND'},info:{'fold:row':'1','subs:A':'0','@f.secret':'PRIVATE_INFO'},open:{settings:true},adding:false,panel:true,error:'PRIVATE_ERROR',note:'PRIVATE_NOTE'};
 const original=JSON.stringify(privateSession),f=await fixture({session:()=>privateSession});f.open();assert.equal(f.host.ready,true);assert.equal(f.sent[0].m,'view.open');assert(Buffer.byteLength(JSON.stringify(f.sent[0]))<4096,'filter before shared request-size validation');
 const language={'@lang':'ja','@lang.os':'ja-JP','@lang.tag':'ja','@lang.preview':'1'};
 function readOnly(request){assert.deepEqual(request.p.session.drafts,language);assert.deepEqual(request.p.session.outbox,{});assert.deepEqual(request.p.session.info,{'fold:row':'1','subs:A':'0'});assert.deepEqual(request.p.session.open,{settings:true});assert.equal(request.p.session.panel,true);assert(!JSON.stringify(request).includes('PRIVATE_'));}
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
test('composer popup: a keystroke in a trigger word sends view.update with the draft through the real transport, once more after, then none',async()=>{
 const f=await fixture({draftRefresh:fn=>fn()});f.open();
 const last=()=>f.sent.at(-1),answer=()=>f.host.receive(f.reply(last()));
 const type=t=>{const n=f.sent.length;f.host.request('draft',t);assert.equal(f.sent.length,n+1,'a read for '+t);assert.equal(last().m,'view.update');const r=last();answer();return r;};
 for(const t of ['@','>','%','#','$','/'])assert.equal(type(t+'al').p.session.drafts.A,t+'al',t);
 assert.equal(type('/al done').p.session.drafts.A,'/al done','once more, so the hub shuts the popup');
 assert(!('A' in type('/al done.').p.session.drafts),'then an ordinary draft stays local');
 type('see %pr');const event={t:'view.changed',version:1,generation:'7',epoch:'actual',viewId:'view-A',target:{kind:'thread',id:'A'},viewRevision:'2'};
 f.host.receive(event);assert.equal(last().m,'view.update');assert.equal(last().p.session.drafts.A,'see %pr','a hub change keeps the popup draft');
});

test('composer popup: coalesced clearing edits still deliver the closing draft',async()=>{
 const f=await fixture({draftRefresh:fn=>fn()});f.open();
 f.host.request('draft','@Zo');const opening=f.sent.at(-1),count=f.sent.length;
 f.host.request('draft','plain');f.host.request('draft','');
 assert.equal(f.sent.length,count,'clearing edits coalesce behind the popup read');
 f.host.receive(f.reply(opening));const closing=f.sent.at(-1);
 assert.notEqual(closing.id,opening.id);assert.equal(closing.p.session.drafts.A,'','the deferred read closes the hub popup');
 f.host.receive(f.reply(closing));
 f.host.request('draft','plain again');assert(!Object.hasOwn(f.sent.at(-1).p.session.drafts,'A'),'ordinary reads stop carrying the draft');
});
