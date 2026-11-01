// The hub's keyed view delta through ThinViews (src/web/thin.js,
// src/web/view-delta.js; hub side src/server/thin-view-delta.bend).
const {test}=require('node:test'),assert=require('node:assert/strict');
const txt=text=>({type:'text',text}),el=(tag,key,kids=[])=>({type:'element',tag,key,attrs:[],kids});
const page=(rows,live='')=>el('div','app',[el('aside','sidebar',[el('button','row',[txt('Project / thread')])]),el('main','main',[...rows.map(r=>el('bp-memo',r,[txt('row '+r)])),el('bp-memo','live',[txt(live)])])]);
async function fixture(enabled=true){
 const {ThinViews,VIEW_LIMITS,VIEW_ID_BASE}=await import('../../src/web/thin.js'),{validateThinResult}=await import('../../src/web/thin-results.js');
 const v=new ThinViews({now:()=>0,send:(_hub,r)=>r,command:c=>c,validateResult:validateThinResult,viewDelta:enabled},VIEW_ID_BASE+100);
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
