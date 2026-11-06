const {test}=require('node:test'),assert=require('node:assert/strict');
test('prefetch is bounded, serial, private and never asks for actions or older pages',async()=>{
 const {ViewPrefetch}=await import('../../src/web/view-prefetch.js'),{VIEW_LIMITS}=await import('../../src/web/thin.js');
 const sent=[],pictures=[];let socket;
 const scope={origin:'https://hub',identity:'owner',epoch:'e',language:'en'};
 const api={hub:scope.origin,scope:()=>scope,has:()=>false,idle:()=>true,encode:x=>x,decode:x=>x,
  socket:()=>socket={send:r=>sent.push(r),close(){}},picture:(...args)=>pictures.push(args)};
 const p=new ViewPrefetch(api);
 for(let i=0;i<40;i++)p.enqueue({kind:'thread',id:'t'+i});assert.equal(p.queue.length,24);clearTimeout(p.timer);p.timer=null;p.pump();
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
test('a picture known to be behind is fetched again, once, and replaces the one held',async()=>{
 const {ViewPrefetch}=await import('../../src/web/view-prefetch.js');
 const scope={origin:'https://hub',identity:'owner',epoch:'e',language:'en'};let held=true;
 const p=new ViewPrefetch({hub:scope.origin,scope:()=>scope,has:()=>held,idle:()=>true,encode:x=>x,decode:x=>x,socket:()=>({send(){},close(){}}),picture(){}});
 p.enqueue({kind:'thread',id:'A'});assert.equal(p.queue.length,0,'a held picture is not fetched again by itself');
 p.enqueue({kind:'thread',id:'A'},false,true);assert.equal(p.queue.length,1);assert.equal(p.queue[0].fresh,true);
 p.attempted.add(p.queue[0].key);p.queue=[];
 p.enqueue({kind:'thread',id:'A'});assert.equal(p.queue.length,0,'attempted and held');
 p.enqueue({kind:'thread',id:'A'},false,true);assert.equal(p.queue.length,1,'behind: attempted again');
 p.stop();
});
test('a row that keeps moving (a streaming bot) is fetched again at most once per floor, and once after it',async()=>{
 const {ViewPrefetch}=await import('../../src/web/view-prefetch.js');
 const scope={origin:'https://hub',identity:'owner',epoch:'e',language:'en'};let clock=0;
 const p=new ViewPrefetch({hub:scope.origin,scope:()=>scope,has:()=>true,idle:()=>true,now:()=>clock,encode:x=>x,decode:x=>x,socket:()=>({send(){},close(){}}),picture(){}});
 const key=JSON.stringify([scope.origin,scope.identity,scope.epoch,{kind:'thread',id:'A'}]);
 p.fetchedAt.set(key,0);
 for(let i=1;i<=50;i++){clock=i*340;p.enqueue({kind:'thread',id:'A'},false,true);}
 assert.equal(p.queue.length,0,'within the floor: not fetched again');assert.equal(p.later.size,1,'one fetch owed for after it');
 clock=31000;p.later.clear();p.enqueue({kind:'thread',id:'A'},false,true);assert.equal(p.queue.length,1,'past the floor: fetched again');
 p.stop();assert.equal(p.later.size,0);
});
test('a destination asked for soon goes while input is recent; the others still wait for the page to be idle',async()=>{
 const {ViewPrefetch}=await import('../../src/web/view-prefetch.js'),{VIEW_LIMITS}=await import('../../src/web/thin.js');
 const sent=[];let socket,typing=true;
 const scope={origin:'https://hub',identity:'owner',epoch:'e',language:'en'};
 const p=new ViewPrefetch({hub:scope.origin,scope:()=>scope,has:()=>false,idle:soon=>soon||!typing,encode:x=>x,decode:x=>x,socket:()=>socket={send:r=>sent.push(r),close(){}},picture(){}});
 p.enqueue({kind:'thread',id:'side'});clearTimeout(p.timer);p.timer=null;p.pump();assert.equal(socket,undefined,'not idle: nothing opened');clearTimeout(p.timer);p.timer=null;
 p.enqueue({kind:'thread',id:'far'},true,false,true);clearTimeout(p.timer);p.timer=null;p.pump();
 socket.onmessage({data:{t:'view',version:1,renderer:'web',generation:'1',epoch:'e',limits:VIEW_LIMITS}});
 assert.equal(sent.length,1);assert.equal(sent[0].p.session.sel,'far','the soon one first, while typing');
 p.stop();
});
test('a thread’s shut runs are read ahead open once, handed to folds, never kept as its picture',async()=>{
 const {ViewPrefetch}=await import('../../src/web/view-prefetch.js'),{VIEW_LIMITS}=await import('../../src/web/thin.js');
 const sent=[],pictures=[],folds=[];let socket;
 const scope={origin:'https://hub',identity:'owner',epoch:'e',language:'en'};
 const p=new ViewPrefetch({hub:scope.origin,scope:()=>scope,has:()=>true,idle:()=>true,encode:x=>x,decode:x=>x,socket:()=>socket={send:r=>sent.push(r),close(){}},picture:(...a)=>pictures.push(a),folds:(...a)=>folds.push(a)});
 p.enqueueFolds({kind:'thread',id:'t'},['r1','r2']);p.enqueueFolds({kind:'thread',id:'t'},['r1','r2','r3']);assert.equal(p.queue.length,1,'once per thread');
 clearTimeout(p.timer);p.timer=null;p.pump();
 socket.onmessage({data:{t:'view',version:1,renderer:'web',generation:'1',epoch:'e',limits:VIEW_LIMITS}});
 assert.equal(sent.length,1,'asked even though a picture of the thread is held');assert.deepEqual(sent[0].p.session.info,{'fold:r1':'1','fold:r2':'1'});
 p.stop();
});
test('a thread’s viewers are read ahead open, once each, with their kept reads, and handed to viewer only',async()=>{
 const {ViewPrefetch}=await import('../../src/web/view-prefetch.js'),{VIEW_LIMITS}=await import('../../src/web/thin.js');
 const sent=[],pictures=[];let socket;
 const scope={origin:'https://hub',identity:'owner',epoch:'e',language:'en'};
 const p=new ViewPrefetch({hub:scope.origin,scope:()=>scope,has:()=>true,idle:()=>true,encode:x=>x,decode:x=>x,socket:()=>socket={send:r=>sent.push(r),close(){}},picture:(...a)=>pictures.push(a),viewer(){}});
 const opens=[{action:'fm',value:'',kind:'files'},{action:'view-kind',value:'board',kind:'board'}];
 p.enqueueViewers({kind:'thread',id:'t'},opens);p.enqueueViewers({kind:'thread',id:'t'},opens);assert.equal(p.queue.length,2,'once per viewer');
 p.enqueueViewers({kind:'bot',id:'b'},opens);assert.equal(p.queue.length,2,'threads only');
 clearTimeout(p.timer);p.timer=null;p.pump();
 socket.onmessage({data:{t:'view',version:1,renderer:'web',generation:'1',epoch:'e',limits:VIEW_LIMITS}});
 assert.equal(sent.length,1,'asked even though a picture of the thread is held');
 assert.deepEqual(sent[0].p.session.info,{'panel:t':'1','view.kind:t':'files','view.path:t':''});assert.equal(sent[0].p.session.panel,true);assert.equal(sent[0].p.cid,'ahead-files');
 p.stop();
});
