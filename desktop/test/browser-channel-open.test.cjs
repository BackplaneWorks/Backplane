'use strict';
// The in-app browser's socket never holds the app: a hub that accepts the
// connection and never answers the upgrade fails the open within its deadline.
const {test}=require('node:test'),assert=require('node:assert/strict'),net=require('node:net'),{EventEmitter}=require('node:events');
const {openBrowserChannel}=require('../browser-channel.cjs');
const token='a'.repeat(32);

test('an open that never completes fails at its own deadline',async()=>{
  class Silent extends EventEmitter{constructor(){super();Silent.made++;}terminate(){this.terminated=true;}}
  Silent.made=0;Silent.OPEN=1;let closed=null;
  const started=Date.now();
  await assert.rejects(openBrowserChannel({origin:'http://127.0.0.1:9',token,onMessage(){},onClose:e=>closed=e,WebSocketClass:Silent,openTimeoutMs:50}),{code:'OPEN_TIMEOUT'});
  assert(Date.now()-started<1000);assert.equal(closed?.code,'OPEN_TIMEOUT');assert.equal(Silent.made,1);
});

test('a hub that accepts TCP and never answers the upgrade: the open fails, no hang',async()=>{
  const held=[],server=net.createServer(s=>{held.push(s);s.on('error',()=>{});});
  await new Promise(r=>server.listen(0,'127.0.0.1',r));
  try{
    const started=Date.now();
    await assert.rejects(openBrowserChannel({origin:`http://127.0.0.1:${server.address().port}`,token,onMessage(){},onClose(){},openTimeoutMs:1500}),e=>['OPEN_TIMEOUT','SOCKET_ERROR'].includes(e.code));
    const took=Date.now()-started;assert(took<4000,`open failed after ${took} ms`);
    assert.equal(held.length,1,'the socket really reached the silent hub');
  }finally{for(const s of held)s.destroy();await new Promise(r=>server.close(r));}
});
