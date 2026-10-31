'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict'),{EventEmitter}=require('node:events');
const {installLoadRecovery}=require('../load-recovery.cjs');
async function settle(){await new Promise(r=>setImmediate(r));}
function fixture(){
  const window=new EventEmitter(),wc=new EventEmitter(),state={url:'http://127.0.0.1:4000/',ready:false,probes:0,reconnects:0,loads:0,status:[],timers:new Map(),quit:false};let id=0;
  window.webContents=wc;window.isDestroyed=()=>false;wc.getURL=()=>state.url;
  window.loadFile=async file=>{assert.equal(file,'/app/startup.html');state.loads++;wc.emit('did-finish-load');queueMicrotask(()=>{state.url='file:///app/startup.html';wc.emit('did-finish-load');});};
  installLoadRecovery(window,{startupPath:'/app/startup.html',startupURL:'file:///app/startup.html',origin:()=> 'http://127.0.0.1:4000',
    session:{fetch:async url=>{assert.equal(url,'http://127.0.0.1:4000/hello');state.probes++;return{ok:state.ready};}},status:(...args)=>state.status.push(args),
    reconnect:async()=>{state.reconnects++;state.url='http://127.0.0.1:4000/';},quitting:()=>state.quit,
    setTimer:fn=>{state.timers.set(++id,fn);return id;},clearTimer:id=>state.timers.delete(id)});
  return{window,state,fail:(code=-102,url='http://127.0.0.1:4000/',main=true)=>wc.emit('did-fail-load',{},code,'refused',url,main),tick:async()=>{const timers=[...state.timers.values()];state.timers.clear();for(const fn of timers)fn();await settle();}};
}
test('a failed hard reload returns to local recovery and reconnects when the selected hub returns',async()=>{
  const f=fixture();f.fail();await settle();assert.equal(f.state.loads,1);assert.equal(f.state.status[0][0],'error');assert.equal(f.state.reconnects,0);
  await f.tick();assert.equal(f.state.reconnects,0);f.state.ready=true;await f.tick();assert.equal(f.state.reconnects,1);assert.equal(f.state.url,'http://127.0.0.1:4000/');assert.equal(f.state.timers.size,0);
});
test('aborted, subframe, foreign-origin and local-file failures cannot replace the app',async()=>{
  const f=fixture();f.fail(-3);f.fail(-102,undefined,false);f.fail(-102,'https://other/');f.fail(-6,'file:///app/startup.html');await settle();assert.equal(f.state.loads,0);assert.equal(f.state.probes,0);
});
test('closing or leaving recovery cancels probing and cannot reopen a window',async()=>{
  for(const change of [f=>f.window.emit('close'),f=>f.state.url='http://127.0.0.1:4000/',f=>f.state.quit=true]){
    const f=fixture();f.fail();await settle();const probes=f.state.probes;change(f);await f.tick();assert.equal(f.state.probes,probes);assert.equal(f.state.reconnects,0);
  }
});
