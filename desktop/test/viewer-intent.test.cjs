const {test}=require('node:test'),assert=require('node:assert/strict');
async function fixture(){
 const {ThinHost}=await import('../../src/web/thin-host.js'),{ThinViews,VIEW_LIMITS}=await import('../../src/web/thin.js');
 const sent=[],views=[],commands=[],feedback=[];let session={sel:'A',drafts:{A:'kept'},rid:1};
 const host=new ThinHost({hub:'https://fixture',cid:'test',now:()=>0,session:()=>session,target:s=>({kind:'thread',id:s.sel}),
  choice:(s,a,v)=>a==='select'?{...s,sel:v}:s,prepare:s=>s,counter(){},send:r=>sent.push(r),local(){},sessionChanged:s=>session=s,ack:s=>s,
  decode:n=>n,command:c=>commands.push(c),state(){},view:n=>views.push(n),anchor:()=>({id:'',offset:0}),blocked(){},viewerChoice:v=>feedback.push(v)},ThinViews,2**47);
 const reply=(r=sent.at(-1),extra={})=>({t:'reply',id:r.id,ok:true,result:{version:1,hub:'https://fixture',generation:'7',epoch:'test',requestRevision:r.p.requestRevision,target:r.p.target,viewId:r.p.viewId||'A',viewRevision:'1',remoteReady:true,renderer:'web',shown:{type:'text',text:r.p.value||'initial'},session:structuredClone(r.p.session),cmds:[],window:{ids:[],older:'',hasMore:false,anchor:{id:'',offset:0},partial:[]},...extra}});
 host.greeting({t:'view',version:1,renderer:'web',generation:'7',epoch:'test',limits:{...VIEW_LIMITS}});host.receive(reply());
 return{host,sent,views,commands,feedback,reply};
}
test('accepted destination acknowledges immediately; adjacent unsent choices collapse and superseded picture never paints',async()=>{
 const f=await fixture();f.host.request('view-kind','board');const first=f.sent.at(-1);
 assert.deepEqual(f.feedback.at(-1),{action:'view-kind',value:'board'});
 f.host.request('view-kind','schematic');f.host.request('view-kind','3d');assert.equal(f.host.actions.length,1);assert.equal(f.host.actions[0].value,'3d');
 const before=f.views.length;f.host.receive(f.reply(first,{cmds:[{type:'copy',text:'one command'}]}));
 assert.equal(f.commands.length,1,'already-sent commands settle exactly once');assert.equal(f.views.length,before,'overtaken picture is suppressed');assert.equal(f.sent.at(-1).p.value,'3d');
 f.host.receive(f.reply());assert.equal(f.views.at(-1).text,'3d');assert.equal(f.feedback.at(-1),null);
});
test('background invalidations cannot retire a viewer choice or require a second click',async()=>{
 const f=await fixture();f.host.request('view-kind','schematic');const action=f.sent.at(-1),count=f.sent.length;
 f.host.receive({t:'view.changed',version:1,generation:'7',epoch:'test',target:{kind:'thread',id:'A'},viewId:'A',viewRevision:'2'});
 assert.equal(f.sent.length,count);f.host.receive(f.reply(action));assert.equal(f.views.at(-1).text,'schematic');
});
test('Split has the same pending, coalescing and stale-picture protection as Board',async()=>{
 const f=await fixture();f.host.request('view-kind','board');const first=f.sent.at(-1);
 f.host.request('view-kind','split');assert.deepEqual(f.feedback.at(-1),{action:'view-kind',value:'split'});
 const before=f.views.length;f.host.receive(f.reply(first));assert.equal(f.views.length,before);
 f.host.receive(f.reply());assert.equal(f.views.at(-1).text,'split');assert.equal(f.feedback.at(-1),null);
});
test('viewer coalescing never crosses a message or destructive-action ordering fence',async()=>{
 const f=await fixture();f.host.request('view-kind','board');f.host.request('view-kind','schematic');f.host.request('send','');f.host.request('view-kind','3d');
 assert.deepEqual(f.host.actions.map(x=>x.action),['view-kind','send','view-kind']);
});
test('changing conversation clears visual intent and a late old reply cannot restore it',async()=>{
 const f=await fixture();f.host.request('view-kind','board');const old=f.sent.at(-1);f.host.request('select','B');assert.equal(f.feedback.at(-1),null);
 f.host.receive(f.reply());const count=f.views.length;f.host.receive(f.reply(old));assert.equal(f.views.length,count);
});
