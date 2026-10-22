// Renderer-local geometry and existing Bend intents only. The private main
// registry/service grants decide which real Chromium guest may be shown.
export function installDesktopBrowser({bridge,root=document,isReady=()=>true,onTakeover=()=>{}}={}) {
  if(!bridge)return Object.freeze({sync(){},hide(){},dispose(){}});
  const doc=root.ownerDocument||root,win=doc.defaultView;
  let current=null,signature='',ticket=0,raf=0,disposed=false;
  let state={phase:'connecting',message:'Connecting Browser…'};
  const ownerOf=node=>node?.getAttribute('data-desktop-browser-owner');
  const observers=[];
  const resize=new win.ResizeObserver(()=>sync());
  function status(node){
    const label=node?.querySelector('.desktop-browser-status');
    if(label&&(!state.owner||state.owner===ownerOf(node)))label.textContent=state.phase==='ready'?'':String(state.message||'Browser unavailable').slice(0,240);
    const reconnect=node?.querySelector('[data-desktop-browser-reconnect]');
    if(reconnect)reconnect.hidden=state.phase!=='error';
  }
  function hide(){
    ticket++;signature='';
    if(current){current=null;resize.disconnect();void Promise.resolve(bridge.hide()).catch(()=>{});}
  }
  function measure(){
    raf=0;if(disposed)return;
    const node=root.querySelector('[data-desktop-browser-owner]'),owner=ownerOf(node);
    if(!isReady()||doc.hidden||!node||typeof owner!=='string'||!owner.length||owner.length>512){hide();return;}
    const style=win.getComputedStyle(node),r=node.getBoundingClientRect();
    const x=Math.max(0,r.left),y=Math.max(0,r.top),right=Math.min(win.innerWidth,r.right),bottom=Math.min(win.innerHeight,r.bottom);
    const blocked=root.querySelector('.modal-back,.shade,.lightbox,.model-menu,.rowmenu,.picker,.hist-menu,[data-lb-close],[aria-modal="true"]');
    if(style.visibility==='hidden'||style.display==='none'||right-x<1||bottom-y<1||blocked){hide();return;}
    if(current!==node){resize.disconnect();current=node;resize.observe(node);signature='';}
    status(node);
    const bounds={x,y,width:right-x,height:bottom-y},next=JSON.stringify([owner,bounds]);
    if(next===signature)return;signature=next;const stamp=++ticket;
    // Main converts CSS coordinates using the shell's actual zoom factor and
    // clamps to its content bounds. DPR/roles/target IDs from DOM grant nothing.
    void Promise.resolve(bridge.present({owner,bounds})).then(result=>{
      if(disposed||stamp!==ticket||current!==node)return;
      if(result?.shown!==true){state={owner,phase:result?.phase==='error'?'error':'connecting',message:result?.message||'Connecting Browser…'};status(node);}
    }).catch(()=>{if(stamp===ticket){state={owner,phase:'error',message:'Browser unavailable'};status(node);}});
  }
  function sync(){if(!disposed&&!raf)raf=win.requestAnimationFrame(measure);}
  const listen=(target,event,callback,options)=>{target.addEventListener(event,callback,options);observers.push(()=>target.removeEventListener(event,callback,options));};
  listen(win,'resize',sync);listen(doc,'scroll',sync,true);listen(doc,'visibilitychange',()=>{if(doc.hidden)hide();else{signature='';sync();}});
  listen(doc,'click',event=>{
    if(disposed||state.phase!=='error'||!current||!event.target.closest?.('[data-desktop-browser-reconnect]')||!bridge.reconnect)return;
    event.preventDefault();state={phase:'connecting',message:'Connecting Browser…'};status(current);
    // Reconnect creates a new private channel, never replays the prior action.
    void Promise.resolve(bridge.reconnect()).then(value=>{state=value;signature='';if(current)status(current);sync();}).catch(()=>{state={phase:'error',message:'Browser unavailable'};if(current)status(current);});
  });
  if(win.visualViewport){listen(win.visualViewport,'resize',sync);listen(win.visualViewport,'scroll',sync);}
  if(bridge.onState)observers.push(bridge.onState(value=>{state=value;signature='';if(current)status(current);sync();}));
  if(bridge.onTakeover)observers.push(bridge.onTakeover(value=>{
    if(!disposed&&current&&value?.owner===ownerOf(current)&&isReady())onTakeover(value.owner);
  }));
  if(bridge.onFocusAddress)observers.push(bridge.onFocusAddress(()=>{
    const address=root.querySelector('[data-bfield^="brurl:"]');if(address){address.focus();address.select();}
  }));
  function dispose(){if(disposed)return;hide();disposed=true;if(raf)win.cancelAnimationFrame(raf);resize.disconnect();for(const remove of observers)if(typeof remove==='function')remove();}
  listen(win,'pagehide',dispose,{once:true});
  return Object.freeze({sync,hide,dispose});
}
