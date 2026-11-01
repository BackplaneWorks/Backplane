// The hub's keyed view delta through ThinViews (src/web/thin.js,
// src/web/view-delta.js; hub side src/server/thin-view-delta.bend).
const {test}=require('node:test'),assert=require('node:assert/strict');
const txt=text=>({type:'text',text}),el=(tag,key,kids=[])=>({type:'element',tag,key,attrs:[],kids});
const page=(rows,live='')=>el('div','app',[el('aside','sidebar',[el('button','row',[txt('Project / thread')])]),el('main','main',[...rows.map(r=>el('bp-memo',r,[txt('row '+r)])),el('bp-memo','live',[txt(live)])])]);
async function fixture(enabled=true,shape=null){
 const {ThinViews,VIEW_LIMITS,VIEW_ID_BASE}=await import('../../src/web/thin.js'),{validateThinResult}=await import('../../src/web/thin-results.js');
 const v=new ThinViews({now:()=>0,send:(_hub,r)=>r,command:c=>c,validateResult:validateThinResult,viewDelta:enabled,...(shape?{shape}:{})},VIEW_ID_BASE+100);
 const session={sel:'A',rid:2,drafts:{A:''},open:{},info:{}},target={kind:'thread',id:'A'};
 v.connect('hub',{t:'view',version:1,renderer:'web',generation:'7',epoch:'epoch',limits:{...VIEW_LIMITS}});
 const win={ids:[],older:'',hasMore:false,anchor:{id:'',offset:0},partial:[]};
 const envelope=(id,extra)=>{const c=v.calls.get(id);return {t:'reply',ok:true,id,result:{version:1,hub:'hub',generation:'7',epoch:'epoch',target:c.target,requestRevision:c.revision,viewId:c.viewId||'view',viewRevision:'9',renderer:'web',remoteReady:true,cmds:[],...extra}}};
 const whole=(id,shown)=>envelope(id,{session,window:win,shown,viewDelta:{version:1,base:'',token:String(v.calls.get(id).revision)}});
 const delta=(id,base,ops)=>envelope(id,{viewDelta:{version:1,base,token:String(v.calls.get(id).revision),ops}});
 const open=v.select('hub',target,session,'cid');
 return {v,session,whole,delta,open,update(){return v.request('hub','view.update',{session})},get link(){return v.links.get('hub')}};
}
test('view delta: offered on presentation requests; a whole reply seeds the base it names',async()=>{
 const f=await fixture();const input=f.v.calls.get(f.open.id).input;assert.deepEqual(input.viewDelta,{version:1,token:''});
 const r=f.v.reply('hub',f.whole(f.open.id,page(['a','b'])));assert(r.remoteReady);assert.equal(r.result.viewDelta,undefined);
 const u=f.update();assert.equal(f.v.calls.get(u.id).input.viewDelta.token,String(input.requestRevision));
 const q=f.v.request('hub','view.search',{query:'x'});assert(!Object.hasOwn(f.v.calls.get(q.id).input,'viewDelta'));});
test('view delta: ops rebuild the picture; untouched subtrees stay the base\'s objects',async()=>{
 const f=await fixture();f.v.reply('hub',f.whole(f.open.id,page(['a','b'],'he')));const before=f.link.shown,base=String(f.v.calls.size?0:0);
 const u=f.update(),token=f.v.calls.get(u.id).input.viewDelta.token;
 const r=f.v.reply('hub',f.delta(u.id,token,[['s',['shown','kids',1,'kids',2,'kids',0,'text'],'hello']]));
 assert(r.remoteReady);assert.deepEqual(r.shown,page(['a','b'],'hello'));
 assert.equal(r.shown.kids[0],before.kids[0],'sidebar kept by identity');assert.equal(r.shown.kids[1].kids[0],before.kids[1].kids[0],'row kept by identity');
 assert.notEqual(r.shown.kids[1],before.kids[1]);assert.deepEqual(r.session,f.session);void base;});
test('view delta: an empty delta is the same picture, and a keyed append moves rows',async()=>{
 const f=await fixture();f.v.reply('hub',f.whole(f.open.id,page(['a','b'])));
 let u=f.update(),r=f.v.reply('hub',f.delta(u.id,f.v.calls.get(u.id).input.viewDelta.token,[]));assert.deepEqual(r.shown,page(['a','b']));
 u=f.update();const t=f.v.calls.get(u.id).input.viewDelta.token;
 r=f.v.reply('hub',f.delta(u.id,t,[['a',['shown','kids',1,'kids'],[[0,1],[1,el('bp-memo','c',[txt('row c')])],[0,2]]]]));
 assert.deepEqual(r.shown,page(['b','c']));});
test('view delta: a base the link does not hold fails and the next request asks whole',async()=>{
 const f=await fixture();f.v.reply('hub',f.whole(f.open.id,page(['a'])));
 const u=f.update(),r=f.v.reply('hub',f.delta(u.id,'99999',[]));assert.equal(r.failed,'invalid view delta');
 const w=f.update();assert.equal(f.v.calls.get(w.id).input.viewDelta.token,'');});
test('view delta: malformed ops dispatch nothing and drop the bases',async()=>{
 for(const ops of [[['x',[],1]],[['s',['shown','nope',3],1]],'bad',[['a',['shown','kids'],[[0,9]]]]]){
  const f=await fixture();f.v.reply('hub',f.whole(f.open.id,page(['a'])));const u=f.update(),t=f.v.calls.get(u.id).input.viewDelta.token;
  const e=f.delta(u.id,t,ops);e.result.cmds=[];const r=f.v.reply('hub',e);assert(r.failed);assert.equal(f.link.bases.map.size,0);}});
test('view delta: selection drops the bases; a host that does not offer it gets nothing new',async()=>{
 const f=await fixture();f.v.reply('hub',f.whole(f.open.id,page(['a'])));const b=f.v.select('hub',{kind:'thread',id:'B'},{...f.session,sel:'B'},'cid');
 assert.equal(f.v.calls.get(b.id).input.viewDelta.token,'');
 const g=await fixture(false);assert(!Object.hasOwn(g.v.calls.get(g.open.id).input,'viewDelta'));});
test('view delta: replies not shown still become bases (the hub kept them)',async()=>{
 const f=await fixture();f.v.reply('hub',f.whole(f.open.id,page(['a'])));
 const t0=f.v.calls.size;void t0;
 const a=f.v.request('hub','view.action',{session:f.session,action:'noop',value:''}),t1=f.v.calls.get(a.id).input.viewDelta.token;
 const u=f.update(),e2=f.delta(u.id,t1,[['s',['shown','kids',1,'kids',1,'kids',0,'text'],'x']]),e1=f.delta(a.id,t1,[]);
 assert(f.v.reply('hub',e2).remoteReady);const r1=f.v.reply('hub',e1);assert.equal(r1.current,false);
 assert(f.link.bases.map.has(String(e1.result.requestRevision)));assert(f.link.bases.map.has(String(e2.result.requestRevision)));});
test('view delta: a draft echo puts back the draft this request carried',async()=>{
 const f=await fixture();f.v.reply('hub',f.whole(f.open.id,page(['a'])));
 const draft='pasted '.repeat(30000),session={...f.session,drafts:{A:draft}};
 const u=f.v.request('hub','view.update',{session}),t=f.v.calls.get(u.id).input.viewDelta.token;
 const e=f.delta(u.id,t,[['t',['session','drafts','A']]]);assert(JSON.stringify(e).length<1024);
 const r=f.v.reply('hub',e);assert(r.remoteReady);assert.equal(r.session.drafts.A,draft);});
test('view delta: after a reconnect, a whole view with a 200 KB draft arrives without it and within the limit',async()=>{
 const f=await fixture(),draft='pasted ✓ '.repeat(22000),session={...f.session,drafts:{A:draft}};
 assert(draft.length>190000);
 // the reconnect: a fresh link, the first view asked with the big draft
 f.v.connect('hub',f.link.greeting);const o=f.v.select('hub',{kind:'thread',id:'A'},session,'cid'),c=f.v.calls.get(o.id);
 assert.equal(c.input.viewDelta.token,'');
 const shown=page(['a']);shown.kids[1].kids.push({type:'element',tag:'textarea',key:'composer',attrs:[{name:'value',value:''}],kids:[]});
 // what the hub sends: ops from nothing, the body with the draft blanked, the draft by reference
 const e=f.delta(o.id,'',[['s',[],{shown,session:{...session,drafts:{A:''}},window:{ids:[],older:'',hasMore:false,anchor:{id:'',offset:0},partial:[]}}],
   ['t',['session','drafts','A']],['t',['shown','kids',1,'kids',2,'attrs',0,'value']]]);
 assert(JSON.stringify(e).length<4096,'nothing of the draft on the wire');
 const r=f.v.reply('hub',e);assert(r.remoteReady,JSON.stringify(r.failed));
 assert.equal(r.session.drafts.A,draft);assert.equal(r.shown.kids[1].kids[2].attrs[0].value,draft);
 // the link stays up and the next request offers this body
 const u=f.update();assert.equal(f.v.calls.get(u.id).input.viewDelta.token,String(c.revision));});
test('view delta: a draft key\'s ops (composer value by "t", session) leave the timeline and window the base\'s own objects',async()=>{
 const f=await fixture(),draft='x '.repeat(100000),session={...f.session,drafts:{A:draft,'@draft.bound':''}};
 f.session.drafts['@draft.bound']='';
 const shown=page(['a','b','c'],'');shown.kids[1].kids.push({type:'element',tag:'textarea',key:'composer',attrs:[{name:'value',value:'old'}],kids:[]});
 f.v.reply('hub',f.whole(f.open.id,shown));const before=f.link.bases.map.get(f.link.bases.newest);
 const u=f.v.request('hub','view.update',{session}),t=f.v.calls.get(u.id).input.viewDelta.token;
 const r=f.v.reply('hub',f.delta(u.id,t,[['t',['session','drafts','A']],['s',['session','drafts','@draft.bound'],'200000 ab'],['t',['shown','kids',1,'kids',4,'attrs',0,'value']]]));
 assert(r.remoteReady,JSON.stringify(r).slice(0,300));assert.equal(r.shown.kids[1].kids[4].attrs[0].value,draft);
 for(let i=0;i<4;i++)assert.equal(r.shown.kids[1].kids[i],before.shown.kids[1].kids[i],'timeline row '+i+' kept');
 assert.equal(r.shown.kids[0],before.shown.kids[0],'sidebar kept');assert.equal(r.window,before.window,'window kept');});
test('view delta: a bounded draft edited in its middle (the wire text unchanged) shows the page\'s draft and keeps the hub\'s base',async()=>{
 // the wire keeps a draft's first and last 10 characters (thin-host.js's boundedDraft does 16384)
 const wireOf=t=>t.length>40?t.slice(0,10)+'…'+t.slice(-10):t;
 const shape=p=>p.session?{...p,session:{...p.session,drafts:{...p.session.drafts,A:wireOf(p.session.drafts.A)}}}:p;
 const f=await fixture(true,shape),d1='head-text '+'x'.repeat(300000)+' tail-text',d2='head-text '+'x'.repeat(150000)+'MIDDLE'+'x'.repeat(150000)+' tail-text';
 assert.equal(wireOf(d1),wireOf(d2));
 const box=v=>({type:'element',tag:'textarea',key:'composer',attrs:[{name:'value',value:v}],kids:[]});
 const s1={...f.session,drafts:{A:d1,'@draft.bound':''}};
 // the first view after the draft: whole, as ops from nothing, the draft by reference
 f.v.reply('hub',f.whole(f.open.id,page(['a'])));
 const o=f.v.request('hub','view.update',{session:s1});
 const shown={...page(['a']),kids:[page(['a']).kids[0],{...page(['a']).kids[1],kids:[...page(['a']).kids[1].kids,box('')]}]};
 const r1=f.v.reply('hub',f.delta(o.id,'',[['s',[],{shown,session:{...s1,drafts:{A:'','@draft.bound':''}},window:{ids:[],older:'',hasMore:false,anchor:{id:'',offset:0},partial:[]}}],
   ['t',['session','drafts','A']],['t',['shown','kids',1,'kids',2,'attrs',0,'value']]]));
 assert(r1.remoteReady,JSON.stringify(r1).slice(0,200));assert.equal(r1.session.drafts.A,d1,'the page sees its own draft');
 assert.equal(f.link.bases.map.get(f.link.bases.newest).session.drafts.A,wireOf(d1),'the base keeps the hub\'s text');
 // a key in the middle: the hub's bounded text is the same, so its delta names no draft
 const u=f.v.request('hub','view.update',{session:{...s1,drafts:{A:d2,'@draft.bound':''}}}),t=f.v.calls.get(u.id).input.viewDelta.token;
 const r2=f.v.reply('hub',f.delta(u.id,t,[['s',['session','drafts','@draft.bound'],'300016 ab']]));
 assert(r2.remoteReady,JSON.stringify(r2).slice(0,200));
 assert.equal(r2.session.drafts.A,d2,'the page\'s current draft, not the last one');assert.equal(r2.shown.kids[1].kids[2].attrs[0].value,d2);
 assert.equal(r2.shown.kids[1].kids[0],r1.shown.kids[1].kids[0],'rows kept');});
test('page window: olderNext is optional, a string, and only beside an older cursor',async()=>{
 const {validateThinResult}=await import('../../src/web/thin-results.js');
 const f=await fixture();const ok=w=>{const e=f.whole(f.open.id,page(['a']));e.result.window={...e.result.window,...w};return validateThinResult(e.result,f.v.calls.get(f.open.id).input)===true;};
 assert(ok({}));assert(ok({older:'c1',hasMore:true,olderNext:'c2'}));assert(ok({older:'c1',hasMore:true,olderNext:''}));
 assert(!ok({older:'',hasMore:false,olderNext:'c2'}),'no older, no olderNext');assert(!ok({older:'c1',hasMore:true,olderNext:7}));});
test('view delta: a reply of no ops is marked unchanged (the page need not draw it again); one with ops is not',async()=>{
 const f=await fixture();f.v.reply('hub',f.whole(f.open.id,page(['a'])));
 let u=f.update(),t=f.v.calls.get(u.id).input.viewDelta.token;let r=f.v.reply('hub',f.delta(u.id,t,[]));assert.equal(r.unchanged,true);assert(r.remoteReady);
 u=f.update();t=f.v.calls.get(u.id).input.viewDelta.token;r=f.v.reply('hub',f.delta(u.id,t,[['s',['shown','kids',1,'kids',1,'kids',0,'text'],'x']]));assert.equal(r.unchanged,false);
 const w=f.v.request('hub','view.update',{session:f.session});r=f.v.reply('hub',f.whole(w.id,page(['b'])));assert.equal(r.unchanged,false,'a whole reply is not');});
