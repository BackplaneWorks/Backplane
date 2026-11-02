'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict');
const tick=()=>new Promise(resolve=>setImmediate(resolve));

// Run the actual renderer controller with Settings in front and no guest
// surface. The private bridge is the only permitted recovery effect.
async function fixture({phase='error',reconnect=async()=>({phase:'ready'}),surface=false,overlays=[],present=async()=>({shown:false,message:'Enter an address to open a page.'})}={}){
 const {installDesktopBrowser}=await import('../../src/web/desktop-browser.js');
 const listeners=new Map(),frames=new Map(),calls=[];
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
  addEventListener(){},removeEventListener(){}};
 doc.defaultView=win;
 const bridge={state:async()=>({phase}),onState(fn){onState=fn;return()=>{onState=null;};},
  reconnect(){calls.push('reconnect');return reconnect();},
  hide(){calls.push('hide');},present(value){calls.push('present');assert(surface,'No presentation allowed');return present(value);}};
 const controller=installDesktopBrowser({bridge,root:doc});
 function frame(){const pending=[...frames.values()];frames.clear();for(const fn of pending)fn();}
 function click(recovery=true){let prevented=false;listeners.get('click')?.({target:{closest:()=>recovery?button:null},preventDefault(){prevented=true;}});return prevented;}
 await tick();frame();
 return{button,label,calls,controller,frame,click,state:value=>onState?.(value),listeners};
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

test('an empty connected Browser gives an address hint and keeps it through UI patches',async()=>{
 const h=await fixture({phase:'ready',surface:true});
 try{
  await tick();assert.equal(h.label.textContent,'Enter an address to open a page.');assert.equal(h.button.hidden,true);
  h.label.textContent='Connecting Browser…';h.controller.sync();h.frame();await tick();
  assert.equal(h.label.textContent,'Enter an address to open a page.');assert.deepEqual(h.calls,['present']);
 }finally{h.controller.dispose();}
});

test('a late presentation reply cannot erase a newer connection failure or its recovery button',async()=>{
 let resolve;const h=await fixture({phase:'ready',surface:true,present:()=>new Promise(r=>{resolve=r;})});
 try{
  h.state({phase:'error',message:'Browser connection unavailable'});
  resolve({shown:false,phase:'connecting',message:'Connecting Browser…'});await tick();
  assert.equal(h.label.textContent,'Browser connection unavailable');assert.equal(h.button.hidden,false);
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
