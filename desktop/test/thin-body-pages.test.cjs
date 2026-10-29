'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict');
async function fixture(options={}){
 const {ThinHost}=await import(process.env.THIN_HOST_MODULE||'../../src/web/thin-host.js'),{ThinViews,VIEW_LIMITS}=await import('../../src/web/thin.js');
 let session={sel:'A',drafts:{A:'Unsent 日本語'},rid:1},counter=0;
 const sent=[],views=[],bodies=[],failures=[];
 const host=new ThinHost({hub:'https://fixture',cid:'c',bodyPages:true,now:()=>0,session:()=>session,target:s=>({kind:'thread',id:s.sel}),
  choice:(s,a,v)=>a==='select'?{...s,sel:v}:s,prepare:(s,p)=>({...p,rid:s.rid}),counter:n=>counter=n,
  send:o=>{assert.equal(o.id,counter);sent.push(structuredClone(o))},decode:n=>n,sessionChanged:s=>session=s,view:(n,d)=>{views.push({n,d});options.view?.(n,d)},
  body:(m,p)=>{bodies.push({m,p});options.body?.(m,p)},bodyFailed:(p,code)=>failures.push({p,code}),local(){},state(){},command(){},blocked(){},warning(){},anchor:()=>({id:'row',offset:0})},ThinViews,2**47);
 const greeting={t:'view',version:1,renderer:'web',generation:'7',epoch:'e',limits:{...VIEW_LIMITS}};
 const part={id:'m',thread:'A',version:'7',bodyPage:1,automatic:true,cursor:'cursor',reason:'byte-limit'};
 const reply=(q=sent.at(-1),extra={})=>({t:'reply',id:q.id,ok:true,result:{version:1,hub:'https://fixture',generation:'7',epoch:'e',renderer:'web',target:q.p.target,requestRevision:q.p.requestRevision,viewId:q.p.viewId||'view-'+q.p.target.id,viewRevision:'1',accountBinding:options.owner||'',remoteReady:true,shown:{type:'text',text:q.p.target.id},session:q.p.session||session,cmds:[],window:{ids:['row'],older:'',hasMore:false,anchor:{id:'row',offset:0},partial:options.publishPart?[part]:[]},...extra}});
 host.greeting(greeting);host.receive(reply());
 const answer=q=>reply(q,{remoteReady:false,message:{id:'m',thread:'A',version:'7',bodyPage:1,text:'actual source',start:0,end:13,total:13,cursor:'',previous:'',hasMore:false,complete:true}});
 return {host,sent,views,bodies,failures,reply,part,answer};
}
test('body capability is explicit and read-only query never replaces the current picture',async()=>{
 const f=await fixture();assert.equal(f.sent[0].p.bodyPages,1);const count=f.views.length;
 assert(f.host.body(f.part,f.part.cursor));const q=f.sent.at(-1);assert.equal(q.m,'view.message');assert.equal(q.p.bodyVersion,'7');
 f.host.receive(f.answer(q));assert.equal(f.bodies.length,1);assert.equal(f.views.length,count);assert.equal(f.host.ready,true);
});
test('body query cannot delay navigation or publish its text after A to B',async()=>{
 const f=await fixture();f.host.body(f.part,f.part.cursor);const q=f.sent.at(-1);
 f.host.request('select','B');assert.equal(f.sent.at(-1).m,'view.open');assert.equal(f.sent.at(-1).p.target.id,'B');
 f.host.receive(f.reply());f.host.receive(f.answer(q));assert.equal(f.bodies.length,0);assert.equal(f.views.at(-1).n.text,'B');assert.equal(f.host.ready,true);
});
test('body failure preserves usable current view and never retries automatically',async()=>{
 const f=await fixture();f.host.body(f.part,f.part.cursor);const q=f.sent.at(-1),count=f.sent.length;
 f.host.receive({t:'reply',id:q.id,ok:false,error:{code:'STALE_CURSOR'}});
 assert.equal(f.failures.length,1);assert.equal(f.host.ready,true);assert.equal(f.sent.length,count);assert.equal(f.views.at(-1).n.text,'A');
});
test('invalid oversized body reply cannot reach the body renderer',async()=>{
 const f=await fixture();f.host.body(f.part,f.part.cursor);const q=f.sent.at(-1),r=f.answer(q);r.result.message.text='x'.repeat(8197);
 f.host.receive(r);assert.equal(f.bodies.length,0);assert.equal(f.failures.length,1);assert.equal(f.host.ready,true);
});
test('actual host presentation binds a body reader that paints its validated source and isolates another account',async()=>{
 const {BodyWindows}=await import('../../src/web/body-windows.js');
 const list=xs=>xs.reduceRight((tail,head)=>({$:'Con',head,tail}),{$:'Nil'});
 const root={$:'El',tag:'div',key:'m',attrs:list([{$:'At',name:'class',value:'msg assistant'},{$:'At',name:'data-id',value:'m'}]),kids:list([])};
 const text=n=>n?.$?.endsWith('Txt')?n.text:n?.kids?nodes(n.kids).map(text).join(''):'';
 const nodes=xs=>{const out=[];for(;xs?.$==='Con';xs=xs.tail)out.push(xs.head);return out};
 const painted=n=>nodes(n?.attrs).some(a=>a.name==='data-body-start')?text(n):nodes(n?.kids).map(painted).join('');
 const edge={dataset:{bodyId:'m',bodyEdge:'next'},getBoundingClientRect:()=>({top:200,bottom:220})};
 const timeline={clientHeight:600,scrollHeight:1200,scrollTop:0,getBoundingClientRect:()=>({top:0,bottom:600}),querySelectorAll:selector=>selector==='[data-body-edge]'?[edge]:[]};
 let active,accepted=false;
 const body=new BodyWindows({request:(p,c)=>active.host.body(p,c),changed(){}});
 const options=owner=>({owner,publishPart:true,view:(_n,d)=>body.bind('https://fixture',d,d.accountBinding||'',root),body:(m,p)=>accepted=body.accept(m,p)});
 active=await fixture(options('account-A'));body.observe(timeline,true);
 const request=active.sent.at(-1);assert.equal(request.m,'view.message');active.host.receive(active.answer(request));
 assert.equal(accepted,true,'real host publication must pass the body freshness fence');
 assert.equal(painted(body.project(root)),'actual source','validated source must reach the actual body projection');
 active=await fixture(options('account-B'));
 assert(!painted(body.project(root)).includes('actual source'),'another account cannot inherit the parked body cache');
 body.observe(timeline,true);const next=active.sent.at(-1),reply=active.answer(next);
 assert.equal(body.accept(reply.result.message,{...next.p,generation:'stale-generation'}),false);
 assert.equal(body.accept(reply.result.message,{...next.p,viewId:'stale-view'}),false);
 active.host.receive(reply);assert.equal(accepted,true);assert.equal(painted(body.project(root)),'actual source');
});
