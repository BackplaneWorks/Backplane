// Renderer-local geometry and existing Bend intents only. The private main
// registry/service grants decide which real Chromium guest may be shown.
export function installDesktopBrowser({bridge,root=document,isReady=()=>true,onTakeover=()=>{}}={}) {
  if(!bridge)return Object.freeze({sync(){},hide(){},dispose(){}});
  const doc=root.ownerDocument||root,win=doc.defaultView;
  let current=null,signature='',ticket=0,raf=0,disposed=false,stateRevision=0;
  let state={phase:'connecting',message:'Connecting Browser…'};
  // A hub can update its web UI while the resident desktop still exposes
  // the old manual reconnect bridge. That shell has no main-owned retry.
  // Only recover its private connection; never repeat a page operation.
  const legacy=typeof bridge.reconnect==='function';
  let legacyPhase='connecting',recoveryTimer=null,recoveryPending=false,recoveryTries=0;
  const recovering='Browser unavailable · reconnecting automatically…';
  function clearRecovery(){if(recoveryTimer!==null)win.clearTimeout(recoveryTimer);recoveryTimer=null;}
  function recoverLegacy(){
    if(!legacy||disposed)return;
    if(legacyPhase==='ready'){recoveryTries=0;clearRecovery();return;}
    if(legacyPhase!=='error'||recoveryPending||recoveryTimer!==null)return;
    recoveryTimer=win.setTimeout(()=>{
      recoveryTimer=null;if(disposed)return;
      recoveryPending=true;const revision=stateRevision;
      void Promise.resolve().then(()=>bridge.reconnect()).then(value=>{
        if(!disposed&&revision===stateRevision)receiveState(value||{phase:'error',message:recovering});
      }).catch(()=>{
        if(!disposed&&revision===stateRevision)receiveState({phase:'error',message:recovering});
      }).finally(()=>{recoveryPending=false;recoverLegacy();});
    },Math.min(30000,500*2**Math.min(recoveryTries++,6)));
  }
  const ownerOf=node=>node?.getAttribute('data-desktop-browser-owner');
  const observers=[];
  const resize=new win.ResizeObserver(()=>sync());
  function status(node){
    const label=node?.querySelector('.desktop-browser-status');
    if(label){
      // This bridge owns the status between page renders too. The DOM
      // patcher must not flash its initial connecting text over live state.
      label.__desktopBrowserStatus=true;
      // A selected owner's ready state says nothing about a different page.
      const selected=!state.owner||state.owner===ownerOf(node);
      const message=selected?(state.phase==='ready'?'':String(state.message||'Browser unavailable').slice(0,240)):'Connecting Browser…';
      // Keep the text node tracked by the DOM patcher. Replacing it with
      // textContent strands its reference and can duplicate the placeholder.
      if(label.firstChild?.nodeType===3&&label.childNodes.length===1){if(label.firstChild.nodeValue!==message)label.firstChild.nodeValue=message;}
      else if(label.textContent!==message)label.textContent=message;
    }

  }
  function hide(){
    ticket++;signature='';
    if(current){current=null;resize.disconnect();void Promise.resolve(bridge.hide()).catch(()=>{});}
  }
  function measure(){
    raf=0;if(disposed)return;status(current);
    const node=root.querySelector('[data-desktop-browser-owner]'),owner=ownerOf(node);
    if(!isReady()||doc.hidden||!node||typeof owner!=='string'||!owner.length||owner.length>512){hide();return;}
    const style=win.getComputedStyle(node),r=node.getBoundingClientRect();
    const x=Math.max(0,r.left),y=Math.max(0,r.top),right=Math.min(win.innerWidth,r.right),bottom=Math.min(win.innerHeight,r.bottom);
    // Thin views retain hidden pictures of old pages and their menus. Their
    // DOM presence must not hide the native guest in the current page.
    const blocked=[...root.querySelectorAll('.modal-back,.shade,.lightbox,.model-menu,.rowmenu,.picker,.hist-menu,.floating-terminal,[data-lb-close],[aria-modal="true"]')].some(overlay=>{
      const box=overlay.getBoundingClientRect(),css=win.getComputedStyle(overlay);
      return css.visibility!=='hidden'&&css.display!=='none'&&box.width>0&&box.height>0&&box.right>0&&box.bottom>0&&box.left<win.innerWidth&&box.top<win.innerHeight;
    });
    if(style.visibility==='hidden'||style.display==='none'||right-x<1||bottom-y<1||blocked){hide();return;}
    if(current!==node){resize.disconnect();current=node;resize.observe(node);signature='';}
    status(node);
    const bounds={x,y,width:right-x,height:bottom-y},next=JSON.stringify([owner,bounds]);
    if(next===signature)return;signature=next;const stamp=++ticket;
    const revision=stateRevision;
    // Main converts CSS coordinates using the shell's actual zoom factor and
    // clamps to its content bounds. DPR/roles/target IDs from DOM grant nothing.
    return Promise.resolve(bridge.present({owner,bounds})).then(result=>{
      if(disposed||stamp!==ticket||current!==node||revision!==stateRevision)return;
      // No tab is an idle browser, not an outstanding connection attempt.
      state=result?.shown===true?{owner,phase:'ready',message:''}:{owner,phase:result?.phase||'empty',message:legacy&&result?.phase==='error'?recovering:result?.message||'Enter an address to open a page.'};status(node);
    }).catch(()=>{if(!disposed&&stamp===ticket&&revision===stateRevision){state={owner,phase:'error',message:'Browser unavailable · reconnecting automatically…'};status(node);}});
  }
  function sync(){if(!disposed&&!raf)raf=win.requestAnimationFrame(measure);}
  const listen=(target,event,callback,options)=>{target.addEventListener(event,callback,options);observers.push(()=>target.removeEventListener(event,callback,options));};
  listen(win,'resize',sync);listen(doc,'scroll',sync,true);listen(doc,'visibilitychange',()=>{if(doc.hidden)hide();else{signature='';sync();}});
  if(win.visualViewport){listen(win.visualViewport,'resize',sync);listen(win.visualViewport,'scroll',sync);}
  const receiveState=value=>{if(disposed||!value)return;stateRevision++;legacyPhase=value.phase;state=legacy&&value.phase==='error'?{...value,message:recovering}:value;signature='';status(current);sync();recoverLegacy();};
  if(bridge.onState)observers.push(bridge.onState(receiveState));
  // Main may already be ready (or unavailable) before this renderer loads.
  // An event that arrives during the query always wins over its older answer.
  if(bridge.state){const revision=stateRevision;void Promise.resolve(bridge.state()).then(value=>{if(revision===stateRevision)receiveState(value);}).catch(()=>{if(revision===stateRevision)receiveState({phase:'error',message:'Browser unavailable'});});}
  if(bridge.onTakeover)observers.push(bridge.onTakeover(value=>{
    if(!disposed&&current&&value?.owner===ownerOf(current)&&isReady())onTakeover(value.owner);
  }));
  if(bridge.onFocusAddress)observers.push(bridge.onFocusAddress(()=>{
    const address=root.querySelector('[data-bfield^="brurl:"]');if(address){address.focus();address.select();}
  }));
  function dispose(){if(disposed)return;hide();disposed=true;clearRecovery();if(raf)win.cancelAnimationFrame(raf);resize.disconnect();for(const remove of observers)if(typeof remove==='function')remove();}
  listen(win,'pagehide',dispose,{once:true});
  return Object.freeze({sync,hide,dispose,async forCapture(){if(raf){win.cancelAnimationFrame(raf);raf=0;}await measure();}});
}
