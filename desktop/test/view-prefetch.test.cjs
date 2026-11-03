const {test}=require('node:test'),assert=require('node:assert/strict');
test('prefetch is bounded, serial, private and never asks for actions or older pages',async()=>{
 const {ViewPrefetch}=await import('../../src/web/view-prefetch.js'),{VIEW_LIMITS}=await import('../../src/web/thin.js');
 const sent=[],pictures=[];let socket;
 const scope={origin:'https://hub',identity:'owner',epoch:'e',language:'en'};
 const api={hub:scope.origin,scope:()=>scope,has:()=>false,idle:()=>true,encode:x=>x,decode:x=>x,
  socket:()=>socket={send:r=>sent.push(r),close(){}},picture:(...args)=>pictures.push(args)};
 const p=new ViewPrefetch(api);
 for(let i=0;i<20;i++)p.enqueue({kind:'thread',id:'t'+i});assert.equal(p.queue.length,5);clearTimeout(p.timer);p.timer=null;p.pump();
 socket.onmessage({data:{t:'view',version:1,renderer:'web',generation:'1',epoch:'e',limits:VIEW_LIMITS}});
 assert.equal(sent.length,1);assert.equal(sent[0].m,'view.open');assert.deepEqual(sent[0].p.session.drafts,{'@lang':'en'});assert.equal(sent[0].p.session.rid,1);assert.equal(sent[0].p.session.panel,false);
 p.pump();assert.equal(sent.length,1,'one in flight');
 socket.onmessage({data:{t:'view.changed',version:1}});assert.equal(sent.length,1,'background invalidation never downloads history');
 assert.equal(pictures.length,0);p.stop();assert.equal(p.queue.length,0);
});
test('late prefetch replies are suppressed after pairing or transport changes',async()=>{
 const {ViewPrefetch}=await import('../../src/web/view-prefetch.js');let socket;const p=new ViewPrefetch({scope:()=>({identity:'owner',epoch:'e'}),has:()=>false,idle:()=>true,hub:'hub',socket:()=>socket={close(){},send(){}},encode:x=>x,decode:()=>{throw Error('late frame must not decode')}});
 p.enqueue({kind:'thread',id:'A'});clearTimeout(p.timer);p.timer=null;p.pump();const receive=socket.onmessage;p.stop();receive({data:{}});assert.equal(p.socket,null);
});
