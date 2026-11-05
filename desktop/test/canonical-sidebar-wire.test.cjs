// Canonical sidebar v1 on the wire (src/web/sidebar-region.js, thin.js;
// the hub's half is src/server/canonical-sidebar.bend): a seed once, then ops,
// across threads, with every way a reply can be wrong falling back to a seed.
const {test}=require('node:test'),assert=require('node:assert/strict');
const txt=text=>({type:'text',text}),el=(tag,key,kids=[],attrs=[])=>({type:'element',tag,key,attrs,kids});
const row=(id,on)=>el('div',id,[txt(id)],[{name:'class',value:on?'thread on':'thread'}]);
const side=(on='A')=>el('aside','sidebar',['A','B','C'].map(id=>row(id,id===on)));
const slot=()=>el('bp-canonical-sidebar','sidebar');
const root=(sidebar,text='body')=>el('div','workspace-root',[sidebar,el('main','main',[txt(text)])]);
const stub=()=>el('div','workspace-root',[],[{name:'data-window-region',value:'root'}]);
async function fixture({capable=true,enabled=true,renderer='web'}={}){
  const {ThinViews,VIEW_LIMITS,VIEW_ID_BASE}=await import('../../src/web/thin.js');
  const {validatePresentation}=await import('../../src/web/startup.js');
  const validateResult=r=>{try{validatePresentation({session:r.session,view:r.shown,cmds:r.cmds});return true;}catch{return false;}};
  const v=new ThinViews({now:()=>0,send:(_h,r)=>r,command:c=>c,validateResult,canonicalSidebar:enabled},VIEW_ID_BASE+100);
  const greeting={t:'view',version:1,renderer,generation:'7',epoch:'epoch',limits:{...VIEW_LIMITS},...(capable?{canonicalSidebar:{version:1,sidebarBytes:2097152,bases:3}}:{})};
  v.connect('hub',greeting);
  const session=sel=>({sel,rid:2,drafts:{},open:{},info:{}});
  const reply=(id,canonical,{binding='acct',shownRoot=root(slot())}={})=>{const c=v.calls.get(id);
    const result={version:1,hub:'hub',generation:'7',epoch:'epoch',target:c.target,requestRevision:c.revision,viewId:'v1',viewRevision:'1',renderer,cmds:[],remoteReady:true,session:session(c.target.id),accountBinding:binding,boundedFrame:1,shown:stub(),windowProjection:{root:shownRoot,logicalBytes:1}};
    if(canonical)result.canonicalSidebar=canonical(c);
    return {t:'reply',ok:true,id,result};};
  const seed=(node=side(),binding='acct')=>c=>({version:1,token:String(c.revision),base:'',accountBinding:binding,node});
  const delta=(ops,base,binding='acct')=>c=>({version:1,token:String(c.revision),base:base??c.input.canonicalSidebar.token,accountBinding:binding,ops});
  const open=id=>v.select('hub',{kind:'thread',id},session(id),'cid');
  const offer=r=>v.calls.get(r.id).input.canonicalSidebar;
  // what the hub does with a reply over replyBytes: every field but shown goes
  // as view.control parts, and the reply itself carries shown and the id
  const split=e=>{const {shown,...rest}=e.result,text=JSON.stringify(rest),n=Math.ceil(text.length/32768);
    for(let i=0;i<n;i++)v.control('hub',{t:'view.control',id:e.id,part:i,last:i===n-1,text:text.slice(i*32768,(i+1)*32768)});
    return v.reply('hub',{t:'reply',ok:true,id:e.id,result:{shown,controlId:e.id}});};
  return {v,reply,seed,delta,open,offer,split,get link(){return v.links.get('hub')}};
}
const aside=shown=>shown.kids[0];
const moveOn=[['s',['kids',0,'attrs',0,'value'],'thread'],['s',['kids',1,'attrs',0,'value'],'thread on']];

test('a seed fills the slot, and the next thread is asked for with its token',async()=>{
  const f=await fixture(),a=f.open('A');
  assert.deepEqual(f.offer(a),{version:1,token:'',accountBinding:''});
  const r=f.v.reply('hub',f.reply(a.id,f.seed()));
  assert.equal(r.failed,undefined);assert.equal(aside(r.shown).tag,'aside');
  assert.equal(Object.hasOwn(r.result,'canonicalSidebar'),false,'the transport field stays out of what the page gets');
  const b=f.open('B');
  assert.deepEqual(f.offer(b),{version:1,token:'1',accountBinding:'acct'},'it survives select');
});

test('ops move the selection; rows no op reaches keep their objects',async()=>{
  const f=await fixture(),a=f.open('A'),first=f.v.reply('hub',f.reply(a.id,f.seed()));
  const b=f.open('B'),r=f.v.reply('hub',f.reply(b.id,f.delta(moveOn)));
  assert.equal(r.failed,undefined);
  assert.equal(aside(r.shown).kids[1].attrs[0].value,'thread on');
  assert.equal(aside(r.shown).kids[2],aside(first.shown).kids[2]);
  assert.equal(aside(first.shown).kids[1].attrs[0].value,'thread','the base is not written to');
});

test('no ops is the very sidebar the link holds',async()=>{
  const f=await fixture(),a=f.open('A'),first=f.v.reply('hub',f.reply(a.id,f.seed()));
  const b=f.open('B'),r=f.v.reply('hub',f.reply(b.id,f.delta([])));
  assert.equal(aside(r.shown),aside(first.shown));
});

test('three sidebars are kept, newest offered; a late reply does not move the offer back',async()=>{
  const f=await fixture(),a=f.open('A');f.v.reply('hub',f.reply(a.id,f.seed()));
  const late=f.open('B'),next=f.open('C');
  f.v.reply('hub',f.reply(next.id,f.delta([])));
  f.v.reply('hub',f.reply(late.id,f.delta([])));
  assert.equal(f.offer(f.open('A')).token,String(next.revision??f.link.canonical.entries[0].token));
  for(const id of ['B','C','A','B']){const r=f.open(id);f.v.reply('hub',f.reply(r.id,f.delta([])));}
  assert.equal(f.link.canonical.entries.length,3);
});

test('a seed under a new binding replaces the old; a delta under one fails and clears',async()=>{
  const f=await fixture(),a=f.open('A');f.v.reply('hub',f.reply(a.id,f.seed()));
  const b=f.open('B'),r=f.v.reply('hub',f.reply(b.id,f.seed(side('B'),'other'),{binding:'other'}));
  assert.equal(r.failed,undefined);assert.equal(f.offer(f.open('C')).accountBinding,'other');
  const g=await fixture(),c=g.open('A');g.v.reply('hub',g.reply(c.id,g.seed()));
  const d=g.open('B'),bad=g.v.reply('hub',g.reply(d.id,g.delta([],undefined,'other'),{binding:'other'}));
  assert.equal(bad.failed,'invalid canonical sidebar');assert.equal(g.offer(g.open('C')).token,'');
});

for(const [name,change] of [
  ['a base this call did not offer',(f,id)=>f.reply(id,f.delta([],'99'))],
  ['a token that is not this request',(f,id)=>f.reply(id,c=>({...f.delta([])(c),token:'99'}))],
  ['node and ops at once',(f,id)=>f.reply(id,c=>({...f.delta([])(c),node:side()}))],
  ['a binding that is not the reply\'s',(f,id)=>f.reply(id,f.delta([]),{binding:'other'})],
  ['ops that break the sidebar',(f,id)=>f.reply(id,f.delta([['s',['tag'],'div']]))],
  ['a t op',(f,id)=>f.reply(id,f.delta([['t',['kids',0]]]))],
])test(`malformed: ${name} fails and the next request asks for a seed`,async()=>{
  const f=await fixture(),a=f.open('A');f.v.reply('hub',f.reply(a.id,f.seed()));
  const b=f.open('B'),r=f.v.reply('hub',change(f,b.id));
  assert.equal(r.failed,'invalid canonical sidebar');assert.equal(f.offer(f.open('C')).token,'');
});

for(const [name,shownRoot] of [['no slot',root(side())],['two slots',el('div','workspace-root',[slot(),slot()])],['a slot with kids',root(el('bp-canonical-sidebar','sidebar',[txt('x')]))],
  ['a slot with attrs',root(el('bp-canonical-sidebar','sidebar',[],[{name:'class',value:'x'}]))],['a slot under another key',root(el('bp-canonical-sidebar','other'))],['a slot and an aside',el('div','workspace-root',[slot(),side()])]])
test(`root with ${name} fails and clears`,async()=>{
  const f=await fixture(),a=f.open('A'),r=f.v.reply('hub',f.reply(a.id,f.seed(),{shownRoot}));
  assert.ok(r.failed);assert.equal(f.offer(f.open('B')).token,'');
});

test('a whole projected reply clears the history; a hub that never said so gets no offer',async()=>{
  const f=await fixture(),a=f.open('A');f.v.reply('hub',f.reply(a.id,f.seed()));
  const b=f.open('B'),r=f.v.reply('hub',f.reply(b.id,null,{shownRoot:root(side('B'))}));
  assert.equal(r.failed,undefined);assert.equal(f.offer(f.open('C')).token,'');
  for(const opts of [{capable:false},{enabled:false},{renderer:'mobile'}]){const g=await fixture(opts);assert.equal(g.offer(g.open('A')),undefined);}
});

test('budgets: the rest as received, and the sidebar apart from it',async()=>{
  const f=await fixture(),a=f.open('A');
  const r=f.split(f.reply(a.id,f.seed(el('aside','sidebar',[txt('x'.repeat(300000))]))));
  assert.equal(r.failed,undefined,'a 300 KB sidebar is over replyBytes and fine here');
  assert.equal(aside(r.shown).kids[0].text.length,300000);
  const b=f.open('B'),big=f.split(f.reply(b.id,f.seed(el('aside','sidebar',[txt('x'.repeat(2097153))]))));
  assert.equal(big.failed,'invalid canonical sidebar');
  const c=f.open('C'),e=f.reply(c.id,f.seed());e.result.padding='x'.repeat(262145);
  assert.equal(f.split(e).failed,'invalid canonical sidebar');
});

test('a reconnect starts with nothing',async()=>{
  const f=await fixture(),a=f.open('A');f.v.reply('hub',f.reply(a.id,f.seed()));
  const {VIEW_LIMITS}=await import('../../src/web/thin.js');
  f.v.connect('hub',{t:'view',version:1,renderer:'web',generation:'8',epoch:'epoch',limits:{...VIEW_LIMITS},canonicalSidebar:{version:1,sidebarBytes:2097152,bases:3}});
  assert.equal(f.offer(f.open('A')).token,'');
});

test('a slot left in a reply with no field never reaches the page, and nothing is kept from it',async()=>{
  const f=await fixture(),a=f.open('A');f.v.reply('hub',f.reply(a.id,f.seed()));
  const b=f.open('B'),r=f.v.reply('hub',f.reply(b.id,null));
  assert.equal(r.failed,'invalid view presentation');assert.equal(r.shown,undefined);
  assert.equal(f.link.shown.kids[0].tag,'aside','the sidebar on screen stays');
  assert.equal(f.offer(f.open('C')).token,'');
  const {validatePresentation}=await import('../../src/web/startup.js');
  assert.throws(()=>validatePresentation({session:{sel:'',rid:0,drafts:{}},view:root(slot()),cmds:[]}),/Invalid presentation element/);
});

test('aliases leave what they do not reach as it was: no-op and unrelated',async()=>{
  const {restoreFrameAliases}=await import('../../src/web/conversation-frame.js');
  const held=side(),tree=root(held);
  assert.equal(restoreFrameAliases(tree,{},null),tree,'no aliases: the same tree');
  assert.equal(restoreFrameAliases(tree,{'@compact.thread':'A'},null),tree,'an alias nothing uses: the same tree');
  const main=el('main','main',[],[{name:'data-thread',value:'@compact.thread'}]),withMain=el('div','workspace-root',[held,main]);
  const out=restoreFrameAliases(withMain,{'@compact.thread':'A'},null);
  assert.notEqual(out,withMain);assert.equal(out.kids[0],held,'the sidebar is the object held');
  assert.equal(out.kids[1].attrs[0].value,'A');
  assert.equal(main.attrs[0].value,'@compact.thread','the input is not written to');
});

test('a reply with aliases keeps the sidebar held through to the page',async()=>{
  const f=await fixture(),a=f.open('A'),first=f.v.reply('hub',f.reply(a.id,f.seed()));
  const b=f.open('B'),e=f.reply(b.id,f.delta([]));e.result.entryAliases={'@compact.thread':'B'};
  assert.equal(aside(f.v.reply('hub',e).shown),aside(first.shown));
});
