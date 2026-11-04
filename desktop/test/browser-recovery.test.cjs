'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict');
const fs=require('node:fs'),vm=require('node:vm');
const tick=()=>new Promise(resolve=>setImmediate(resolve));

// Run the actual renderer controller with Settings in front and no guest
// surface. The private bridge is the only permitted recovery effect.
async function fixture({phase='error',reconnect,surface=false,overlays=[],present=async()=>({shown:false,message:'Enter an address to open a page.'}),onRestore=()=>false}={}){
 const {installDesktopBrowser}=await import('../../src/web/desktop-browser.js');
 const listeners=new Map(),frames=new Map(),calls=[],timers=[];
 let nextFrame=0,onState;
 const button={hidden:true,disabled:false,style:{display:'none'}};
 const label={textContent:'Connecting Browser…'};
 const node={getAttribute:()=> 'self',querySelector:()=>label,getBoundingClientRect:()=>({left:0,top:0,right:600,bottom:400})};
 const doc={hidden:false,defaultView:null,
  querySelector(selector){if(surface)return selector==='[data-desktop-browser-owner]'?node:null;assert.equal(selector,'[data-desktop-browser-owner]','recovery must not query conversation controls');return null;},
  querySelectorAll(selector){if(surface&&selector!=='[data-desktop-browser-reconnect]')return overlays;assert.equal(selector,'[data-desktop-browser-reconnect]');return[button];},
  addEventListener(name,fn){listeners.set(name,fn);},removeEventListener(name,fn){if(listeners.get(name)===fn)listeners.delete(name);}};
 const win={innerWidth:800,innerHeight:600,getComputedStyle:node=>node.css||({visibility:'visible',display:'block'}),ResizeObserver:class{observe(){assert(surface,'No guest surface');}disconnect(){}},
  requestAnimationFrame(fn){frames.set(++nextFrame,fn);return nextFrame;},cancelAnimationFrame(id){frames.delete(id);},
  setTimeout(fn,ms){const timer={fn,ms};timers.push(timer);return timer;},clearTimeout(timer){timer.cancelled=true;},
  addEventListener(){},removeEventListener(){}};
 doc.defaultView=win;
 const bridge={state:async()=>({phase}),onState(fn){onState=fn;return()=>{onState=null;};},
  restored(owner){calls.push('restored:'+owner);},
  hide(){calls.push('hide');},present(value){calls.push('present');assert(surface,'No presentation allowed');return present(value);}};
 if(reconnect)bridge.reconnect=()=>{calls.push('reconnect');return reconnect();};
 const controller=installDesktopBrowser({bridge,root:doc,onRestore});
 function frame(){const pending=[...frames.values()];frames.clear();for(const fn of pending)fn();}
 function click(recovery=true){let prevented=false;listeners.get('click')?.({target:{closest:()=>recovery?button:null},preventDefault(){prevented=true;}});return prevented;}
 await tick();frame();
 return{button,label,calls,timers,controller,frame,click,state:value=>onState?.(value),listeners};
}

test('connection errors stay within Browser with no recovery control or click handler',async()=>{
 const h=await fixture({phase:'error'});
 try{assert(!h.listeners.has('click'));assert.deepEqual(h.calls,[]);assert.equal(h.button.hidden,true);assert.equal(h.timers.length,0,'current desktops keep recovery in main');}
 finally{h.controller.dispose();}
});

test('updated web UI recovers an older manual-reconnect desktop even with Settings open',async()=>{
 const h=await fixture({reconnect:async()=>({phase:'ready'})});
 try{
  assert.equal(h.timers.length,1);assert.equal(h.timers[0].ms,500);
  h.timers[0].fn();await tick();h.frame();
  assert.deepEqual(h.calls,['reconnect']);assert(!h.listeners.has('click'));
  assert.equal(h.timers.length,1,'ready ends recovery without touching the conversation or opening a page');
 }finally{h.controller.dispose();}
});

test('legacy recovery coalesces errors and backs off until the selected service returns',async()=>{
 let finish;const h=await fixture({reconnect:()=>new Promise(r=>{finish=r;})});
 try{
  h.state({phase:'error'});assert.equal(h.timers.length,1);
  h.timers[0].fn();await tick();
  h.state({phase:'connecting'});h.state({phase:'error'});assert.equal(h.timers.length,1);
  finish({phase:'ready'});await tick();
  assert.equal(h.timers.length,2,'a stale ready reply cannot replace the newer error');
  assert.equal(h.timers[1].ms,1000);assert.deepEqual(h.calls,['reconnect']);
  h.timers[1].fn();await tick();finish({phase:'ready'});await tick();
  h.state({phase:'error'});assert.equal(h.timers.at(-1).ms,500,'success resets backoff');
 }finally{h.controller.dispose();}
});

test('legacy recovery handles rejected and synchronously throwing reconnect calls',async()=>{
 let attempts=0;const h=await fixture({reconnect:()=>{
  if(++attempts===1)return Promise.reject(Error('service stopped'));
  throw Error('service not ready');
 }});
 try{
  for(const ms of [500,1000,2000,4000,8000,16000,30000,30000]){
   const timer=h.timers.at(-1);assert.equal(timer.ms,ms);timer.fn();await tick();
  }
  assert.equal(attempts,8);
 }finally{h.controller.dispose();assert(h.timers.at(-1).cancelled);}
});

test('legacy recovery keeps a newer ready event over a stale failed reply',async()=>{
 let finish;const h=await fixture({reconnect:()=>new Promise(r=>{finish=r;})});
 try{
  h.timers[0].fn();await tick();h.state({phase:'ready'});
  finish({phase:'error'});await tick();assert.equal(h.timers.length,1);
 }finally{h.controller.dispose();}
});

test('legacy recovery cancels timers and ignores in-flight completion after renderer disposal',async()=>{
 const queued=await fixture({reconnect:async()=>({phase:'ready'})});
 queued.controller.dispose();assert(queued.timers[0].cancelled);queued.timers[0].fn();await tick();assert.deepEqual(queued.calls,[]);
 let finish;const pending=await fixture({reconnect:()=>new Promise(r=>{finish=r;})});
 pending.timers[0].fn();await tick();pending.controller.dispose();finish({phase:'error'});await tick();
 assert.equal(pending.timers.length,1);assert.deepEqual(pending.calls,['reconnect']);
});

test('a legacy presentation error explains automatic recovery and preserves the tracked text node',async()=>{
 const h=await fixture({surface:true,reconnect:async()=>({phase:'ready'}),present:async()=>({shown:false,phase:'error',message:'Browser unavailable'})});
 try{
  await tick();assert.equal(h.label.textContent,'Browser unavailable · reconnecting automatically…');
  h.state({phase:'error',message:'Reconnect to try again.'});assert.equal(h.label.textContent,'Browser unavailable · reconnecting automatically…');
 }finally{h.controller.dispose();}
});

test('ordinary web clients do not install a desktop controller',async()=>{
 const {installDesktopBrowser}=await import('../../src/web/desktop-browser.js');
 const ordinary=installDesktopBrowser({bridge:null,root:{get ownerDocument(){throw Error('Ordinary client must remain untouched');}}});
 ordinary.sync();ordinary.hide();ordinary.dispose();
});

test('an empty connected Browser gives an address hint and keeps it through UI patches',async()=>{
 const h=await fixture({phase:'ready',surface:true});
 try{
  await tick();assert.equal(h.label.textContent,'Enter an address to open a page.');assert.equal(h.button.hidden,true);
  h.label.textContent='Connecting Browser…';h.controller.sync();h.frame();await tick();
  assert.equal(h.label.textContent,'Enter an address to open a page.');assert.deepEqual(h.calls,['present']);
 }finally{h.controller.dispose();}
});

test('status updates preserve the text node tracked by the DOM patcher',async()=>{
 const h=await fixture({phase:'ready',surface:true,present:async()=>({shown:true})});
 try{
  await tick();const text={nodeType:3,nodeValue:'Connecting Browser…'};h.label.firstChild=text;h.label.childNodes=[text];
  Object.defineProperty(h.label,'textContent',{get:()=>text.nodeValue,set:()=>assert.fail('Replacing the managed text node strands the DOM patcher')});
  h.controller.sync();h.frame();assert.equal(text.nodeValue,'');
  h.state({phase:'error',message:'Connection unavailable'});assert.equal(text.nodeValue,'Connection unavailable');assert.equal(h.label.firstChild,text);
 }finally{h.controller.dispose();}
});

test('real page patches leave live Browser status unchanged before the next animation frame',async()=>{
 const h=await fixture({phase:'ready',surface:true});
 try{
  await tick();
  const source=fs.readFileSync(require.resolve('../../src/web/host.js'),'utf8');
  const patchSource=source.slice(source.indexOf('function patch(el, v) {'),source.indexOf('// Timing, for a bench only:'));
  const patch=vm.runInNewContext(patchSource+';patch',{RR:null,setAttrs(){}});
  const text={nodeType:3,nodeValue:h.label.textContent};h.label.firstChild=text;h.label.childNodes=[text];
  h.label.__v={attrs:[]};
  for(const state of [null,{phase:'ready',owner:'self'}, {phase:'error',message:'Connection unavailable'}]){
   if(state)h.state(state);
   const before=text.nodeValue;
   const vnode={attrs:[],kids:[{text:'Connecting Browser…'}]};
   for(let i=0;i<30;i++){
    patch(h.label,vnode);
    assert.equal(text.nodeValue,before,'status stays steady synchronously, even before rAF');
    assert.equal(h.label.__v,vnode);
   }
  }
 }finally{h.controller.dispose();}
});

test('a late presentation reply cannot erase a newer connection failure status',async()=>{
 let resolve;const h=await fixture({phase:'ready',surface:true,present:()=>new Promise(r=>{resolve=r;})});
 try{
  h.state({phase:'error',message:'Browser connection unavailable'});
  resolve({shown:false,phase:'connecting',message:'Connecting Browser…'});await tick();
  assert.equal(h.label.textContent,'Browser connection unavailable');assert.equal(h.button.hidden,true);
 }finally{h.controller.dispose();}
});

test('a replaced renderer reapplies a ready surface without dispatching another presentation',async()=>{
 const h=await fixture({phase:'ready',surface:true,present:async()=>({shown:true})});
 try{
  await tick();assert.equal(h.label.textContent,'');
  h.label.textContent='Connecting Browser…';h.controller.sync();h.frame();await tick();
  assert.equal(h.label.textContent,'');assert.deepEqual(h.calls,['present']);
 }finally{h.controller.dispose();}
});

test('menus retained in hidden cached pages do not block the actual native Browser',async()=>{
 const hidden={getBoundingClientRect:()=>({width:0,height:0,left:0,top:0,right:0,bottom:0})};
 const h=await fixture({phase:'ready',surface:true,overlays:[hidden],present:async()=>({shown:true})});
 try{await tick();assert.equal(h.label.textContent,'');assert.deepEqual(h.calls,['present']);}
 finally{h.controller.dispose();}
});

test('a visible overlay after a hidden retained menu hides the guest until dismissed',async()=>{
 const overlays=[{getBoundingClientRect:()=>({width:0,height:0,left:0,top:0,right:0,bottom:0})}];
 const h=await fixture({phase:'ready',surface:true,overlays,present:async()=>({shown:true})});
 try{
  await tick();assert.deepEqual(h.calls,['present']);
  overlays.push({getBoundingClientRect:()=>({width:100,height:100,left:0,top:0,right:100,bottom:100})});
  h.controller.sync();h.frame();assert.deepEqual(h.calls,['present','hide']);
  overlays.pop();h.controller.sync();h.frame();await tick();assert.deepEqual(h.calls,['present','hide','present']);
 }finally{h.controller.dispose();}
});
