'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict');
const tick=()=>new Promise(resolve=>setImmediate(resolve));

// Run the actual renderer controller with Settings in front and no guest
// surface. The private bridge is the only permitted recovery effect.
async function fixture({phase='error',reconnect=async()=>({phase:'ready'})}={}){
 const {installDesktopBrowser}=await import('../../src/web/desktop-browser.js');
 const listeners=new Map(),frames=new Map(),calls=[];
 let nextFrame=0,onState;
 const button={hidden:true,disabled:false,style:{display:'none'}};
 const doc={hidden:false,defaultView:null,
  querySelector(selector){assert.equal(selector,'[data-desktop-browser-owner]','recovery must not query conversation controls');return null;},
  querySelectorAll(selector){assert.equal(selector,'[data-desktop-browser-reconnect]');return[button];},
  addEventListener(name,fn){listeners.set(name,fn);},removeEventListener(name,fn){if(listeners.get(name)===fn)listeners.delete(name);}};
 const win={ResizeObserver:class{observe(){throw Error('No guest surface');}disconnect(){}},
  requestAnimationFrame(fn){frames.set(++nextFrame,fn);return nextFrame;},cancelAnimationFrame(id){frames.delete(id);},
  addEventListener(){},removeEventListener(){}};
 doc.defaultView=win;
 const bridge={state:async()=>({phase}),onState(fn){onState=fn;return()=>{onState=null;};},
  reconnect(){calls.push('reconnect');return reconnect();},
  hide(){calls.push('hide');},present(){calls.push('present');throw Error('No presentation allowed');}};
 const controller=installDesktopBrowser({bridge,root:doc});
 function frame(){const pending=[...frames.values()];frames.clear();for(const fn of pending)fn();}
 function click(recovery=true){let prevented=false;listeners.get('click')?.({target:{closest:()=>recovery?button:null},preventDefault(){prevented=true;}});return prevented;}
 await tick();frame();
 return{button,calls,controller,frame,click,state:value=>onState?.(value),listeners};
}

test('Settings offers explicit Browser recovery without presenting a guest or querying conversation controls',async()=>{
 const h=await fixture();
 try{
  assert.equal(h.button.hidden,false);assert.equal(h.button.style.display,'');assert.deepEqual(h.calls,[]);
  assert.equal(h.click(false),false);assert.deepEqual(h.calls,[]);
  assert.equal(h.click(),true);assert.equal(h.button.hidden,true);assert.equal(h.button.disabled,true);
  await tick();h.frame();assert.deepEqual(h.calls,['reconnect']);assert.equal(h.button.hidden,true);
 }finally{h.controller.dispose();}
});

test('duplicate gestures and intermediate error notifications share one pending recovery',async()=>{
 let resolve;const pending=new Promise(r=>{resolve=r;});const h=await fixture({reconnect:()=>pending});
 try{
  h.click();assert.equal(h.click(),false);await tick();
  h.state({phase:'error',message:'Browser unavailable'});assert.equal(h.button.disabled,true);assert.equal(h.click(),false);
  assert.deepEqual(h.calls,['reconnect']);resolve({phase:'ready'});await tick();h.frame();assert.equal(h.button.hidden,true);assert.equal(h.button.disabled,false);
 }finally{h.controller.dispose();}
});

test('failed recovery remains explicit and retryable without replaying any guest command',async()=>{
 let attempts=0;const h=await fixture({reconnect:()=>{if(++attempts===1)throw Error('Synthetic refusal');return Promise.resolve({phase:'ready'});}});
 try{
  h.click();await tick();assert.equal(h.button.hidden,false);assert.equal(h.button.disabled,false);assert.deepEqual(h.calls,['reconnect']);
  h.frame();await tick();assert.equal(attempts,1,'no automatic retry');
  h.click();await tick();h.frame();assert.deepEqual(h.calls,['reconnect','reconnect']);assert.equal(h.button.hidden,true);
 }finally{h.controller.dispose();}
});

test('healthy native controllers and ordinary browser clients expose no recovery action',async()=>{
 const h=await fixture({phase:'ready'});
 try{assert.equal(h.button.hidden,true);assert.equal(h.button.style.display,'none');assert.equal(h.click(),false);assert.deepEqual(h.calls,[]);}finally{h.controller.dispose();}
 const {installDesktopBrowser}=await import('../../src/web/desktop-browser.js');
 const ordinary=installDesktopBrowser({bridge:null,root:{get ownerDocument(){throw Error('Ordinary client must remain untouched');}}});
 ordinary.sync();ordinary.hide();ordinary.dispose();
});

test('renderer replacement reapplies error visibility and disposal retires recovery listeners',async()=>{
 let resolve;const h=await fixture({reconnect:()=>new Promise(r=>{resolve=r;})});
 h.button.hidden=true;h.button.style.display='none';h.controller.sync();h.frame();assert.equal(h.button.hidden,false);
 h.click();await tick();h.controller.dispose();assert(!h.listeners.has('click'));
 const after={...h.button,style:{...h.button.style}};resolve({phase:'ready'});await tick();h.frame();assert.deepEqual(h.button,after);assert.deepEqual(h.calls,['reconnect']);
});
