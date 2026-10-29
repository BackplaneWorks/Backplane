'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict');
async function fixture(){
 const {ThinHost}=await import('../../src/web/thin-host.js'),{ThinViews,VIEW_LIMITS}=await import('../../src/web/thin.js');
 let session={sel:'A',drafts:{A:'Unsent 日本語'},rid:1},counter=0;
 const sent=[],views=[],bodies=[],failures=[];
 const host=new ThinHost({hub:'https://fixture',cid:'c',bodyPages:true,now:()=>0,session:()=>session,target:s=>({kind:'thread',id:s.sel}),
  choice:(s,a,v)=>a==='select'?{...s,sel:v}:s,prepare:(s,p)=>({...p,rid:s.rid}),counter:n=>counter=n,
  send:o=>{assert.equal(o.id,counter);sent.push(structuredClone(o))},decode:n=>n,sessionChanged:s=>session=s,view:(n,d)=>views.push({n,d}),
  body:(m,p)=>bodies.push({m,p}),bodyFailed:(p,code)=>failures.push({p,code}),local(){},state(){},command(){},blocked(){},warning(){},anchor:()=>({id:'row',offset:0})},ThinViews,2**47);
 const greeting={t:'view',version:1,renderer:'web',generation:'7',epoch:'e',limits:{...VIEW_LIMITS}};
 const reply=(q=sent.at(-1),extra={})=>({t:'reply',id:q.id,ok:true,result:{version:1,hub:'https://fixture',generation:'7',epoch:'e',renderer:'web',target:q.p.target,requestRevision:q.p.requestRevision,viewId:q.p.viewId||'view-'+q.p.target.id,viewRevision:'1',remoteReady:true,shown:{type:'text',text:q.p.target.id},session:q.p.session||session,cmds:[],window:{ids:['row'],older:'',hasMore:false,anchor:{id:'row',offset:0},partial:[]},...extra}});
 host.greeting(greeting);host.receive(reply());
 const part={id:'m',thread:'A',version:'7',bodyPage:1,automatic:true,cursor:'cursor'};
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
