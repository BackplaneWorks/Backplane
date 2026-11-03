'use strict';
const fs=require('node:fs'),path=require('node:path');
const {randomBytes,createHash}=require('node:crypto');
const {AsyncLocalStorage}=require('node:async_hooks');
const {openBrowserChannel}=require('./browser-channel.cjs');
const {validateBrowserHello,validateBrowserEnvelope}=require('./browser-envelope.cjs');
const {exactObject,targetKey,encodeBrowserMessage,decodeBrowserMessage}=require('./browser-wire.cjs');
const {createBrowserLeases}=require('./browser-leases.cjs');
const {createGuestDocuments}=require('./browser-documents.cjs');
const {createBrowserDispatch}=require('./browser-dispatch.cjs');
const {createBrowserGuestManager}=require('./browser-guest.cjs');
const {createBrowserSurface}=require('./browser-surface.cjs');
const MAX_SEQUENCE=4294967295;
const NAVIGATION=new Set(['open','navigate','goto','reload','back','forward','newtab','click','type','press','select','fill','check','hover','evaluate']);
const navigationAction=action=>NAVIGATION.has(action.op)||(action.op==='tabs'&&!!action.args.new)||(action.op==='cdp'&&['Page.navigate','Page.reload','Page.navigateToHistoryEntry','Runtime.evaluate','Runtime.callFunctionOn','Input.dispatchMouseEvent','Input.dispatchKeyEvent','Input.insertText','Input.imeSetComposition','DOM.focus'].includes(action.args.method));
const ref=e=>({owner:e.owner,tabId:e.tabId,generation:e.generation});
const tabKey=t=>JSON.stringify([t.owner,t.tabId]);
const hex=value=>typeof value==='string'&&/^[a-f0-9]{32}$/.test(value);
const snapshot=value=>decodeBrowserMessage(encodeBrowserMessage(value));
const same=(a,b)=>targetKey(a)===targetKey(b);

// A canonical policy change renews the idle guest's lifecycle authority too.
// Never rebase preparation, unacknowledged navigation or any owner's running
// command: those retain their original correlated execution/acknowledgement.
function adoptIdleBrowserGrant({leases,tabs,preparing,acknowledgedRevisions},target){
 const tab=tabs.get(tabKey(target)),grant=leases.targetGrantOf(target);
 if(!grant||!tab?.offered||!tab.creationCompleted||tab.prep||preparing.has(target.owner)||
   !same(tab.target,target)||!same(tab.ackTarget,target)||grant.revision<tab.revision||
   [...tabs.values()].some(item=>item.target.owner===target.owner&&item.activeCall))return false;
 tab.revision=grant.revision;acknowledgedRevisions.set(target.owner,grant.revision);return true;
}

// Settle previously queued registry work before this command can change the
// native document. Its original target/revision must still be current after
// the await; a later policy or generation cannot supply replacement authority.
async function settleBrowserCallRegistry({drain,leases,acknowledgedRevisions},tab,payload){
 await drain(payload.owner);
 const grant=leases.targetGrantOf(payload.target);
 if(!grant||grant.revision!==payload.grantRevision||
   !same(tab.target,payload.target)||!same(tab.ackTarget,payload.target)||
   acknowledgedRevisions.get(payload.owner)!==payload.grantRevision)
  throw Object.assign(new Error('Browser registry changed before command dispatch'),{dispatched:false});
}

// Private main-only controller. Renderer intents contain owner+geometry only;
// a service browser.prepare/call and its exact live grant authorize all work.
function createDesktopBrowserController({electron,window,appSession,origin,version,token,
 guestManagerSha256,recorder,unknownState,executeCall,onState=()=>{},onTakeover=()=>{},onRecover=()=>{},
 channelFactory=openBrowserChannel,managerFactory=createBrowserGuestManager,surfaceFactory=createBrowserSurface}){
 if(typeof executeCall!=='function'||!unknownState)throw new TypeError('Missing Browser action/UNKNOWN integration');
 const actualSha=createHash('sha256').update(fs.readFileSync(path.join(__dirname,'browser-guest.cjs'))).digest('hex');
 if(actualSha!==guestManagerSha256)throw new Error('Browser guest source differs from reviewed package');
 let channel,manager,surface,dispatch,identity=null,closed=false,started=false;
 let clientSequence=0,serverSequence=0,heartbeat,expiry,desired=null,presentationQueued=false;
 let state={phase:'connecting',message:'Connecting Browser…'};
 const pending=new Map(),incoming=new Set(),preparing=new Map(),prepared=new Map(),tabs=new Map(),owners=new Map(),calls=new Map(),callTails=new Map(),lifecycleTails=new Map(),acknowledgedRevisions=new Map();
 const waiters=new Set(),initialization=new AsyncLocalStorage();
 const documents=createGuestDocuments();
 const nonce=()=>randomBytes(16).toString('hex');
 function announce(phase,message,owner){const next={phase,message,...(owner?{owner}:{})};if(JSON.stringify(next)===JSON.stringify(state))return;state=next;onState(Object.freeze({...state}));}
 function wake(){for(const fn of [...waiters])fn();}
 function reconcile(){
  clearTimeout(expiry);
  if(closed)return;
  manager?.reconcileInput();refreshPresentation();wake();
  const deadline=leases.deadline();
  if(Number.isFinite(deadline))expiry=setTimeout(()=>leases.expire(),Math.max(1,Math.ceil(deadline-performance.now())));
 }
 const leases=createBrowserLeases({onChange:reconcile});
 function fail(){
  if(closed)return;
  // Remember only an idle selected document. UNKNOWN/active operations are
  // never retried; a new controller gets fresh grants for URL restoration.
  const restore=[];
  for(const tab of tabs.values())if(tab.activeCall)unknownState.latch(tab.target.owner);
  for(const tab of tabs.values())if(tab.active&&tab.offered&&tab.creationCompleted&&!unknownState.has(tab.target.owner)&&/^https?:\/\//.test(tab.url)){
   restore.push({owner:tab.target.owner,url:tab.url});
  }
  onRecover(restore);close();announce('error','Browser unavailable · reconnecting automatically…');
 }
 function write(op,payload,id=nonce()){
  if(closed||!channel||clientSequence>=MAX_SEQUENCE)throw Object.assign(new Error('Private Browser writer unavailable'),{dispatched:false});
  const message={v:1,op,id,clientSequence:++clientSequence,...(identity?identity:{}),payload};
  channel.send(message);return id;
 }
 function rpc(op,payload,{expected='result',timeoutMs=15000}={}){
  if(pending.size+incoming.size>=32)return Promise.reject(Object.assign(new Error('Private Browser pending limit'),{dispatched:false}));
  const id=nonce();
  return new Promise((resolve,reject)=>{
   const timer=setTimeout(()=>{pending.delete(id);reject(Object.assign(new Error('Private Browser acknowledgement timeout'),{dispatched:false}));fail();},timeoutMs);
   pending.set(id,{expected,resolve,reject,timer});
   try{write(op,payload,id);}catch(error){clearTimeout(timer);pending.delete(id);reject(error);}
  });
 }
 function wait(predicate,timeoutMs=15000){
  return new Promise((resolve,reject)=>{
   let timer;const check=()=>{try{if(closed)throw new Error('Private Browser closed');const result=predicate();if(!result)return;clearTimeout(timer);waiters.delete(check);resolve(result);}catch(error){clearTimeout(timer);waiters.delete(check);reject(error);}};
   timer=setTimeout(()=>{waiters.delete(check);reject(new Error('Browser lifecycle deadline'));},timeoutMs);waiters.add(check);check();
  });
 }
 function currentGrant(target,revision){const grant=leases.targetGrantOf(target);if(!grant||grant.revision!==revision)throw new Error('Browser lifecycle authority changed');return grant;}
 function phaseCurrent(prep){return preparing.get(prep.owner)===prep&&!['cancelled','completed','failed'].includes(prep.phase)&&!closed;}
 function ack(payload,prepareId,target){
  exactObject(payload,['ok','prepareId','target','grantRevision']);
  if(payload.ok!==true||payload.prepareId!==prepareId||!same(payload.target,target)||!Number.isSafeInteger(payload.grantRevision)||payload.grantRevision<1)throw new Error('Uncorrelated Browser lifecycle acknowledgement');
  currentGrant(target,payload.grantRevision);return payload.grantRevision;
 }
 function append(tab,fn){
  if(++tab.queued>512){fail();return Promise.reject(new Error('Browser event backlog'));}
  const result=(lifecycleTails.get(tab.target.owner)||Promise.resolve()).then(()=>{if(closed)throw new Error('Browser controller closed');return fn();});
  tab.tail=result.catch(()=>{fail();}).finally(()=>{tab.queued--;wake();});lifecycleTails.set(tab.target.owner,tab.tail);return result;
 }
 async function drain(owner){let tail;do{tail=lifecycleTails.get(owner);await tail;}while(tail!==lifecycleTails.get(owner));}
 function registryAck(payload,target){
  exactObject(payload,['ok','target','grantRevision']);
  if(payload.ok!==true||!Number.isSafeInteger(payload.grantRevision)||payload.grantRevision<1||
    (target===null?payload.target!==null:!same(payload.target,target)))throw new Error('Uncorrelated Browser registry acknowledgement');
  return payload.grantRevision;
 }
 function inventory(tab,action){
  return append(tab,async()=>{
   if(preparing.has(tab.target.owner))return;
   const rows=manager.list(tab.target.owner).map(({owner,tabId,generation,url,title,active})=>({owner,tabId,generation,url,title,active}));
   // An unoffered local root must never be adopted through inventory, nor may a
   // partial snapshot silently remove a registered root. Retry after its offer.
   if(rows.some(row=>!tabs.get(tabKey(row))?.offered))return;
   const active=rows.find(row=>row.active),target=active?ref(active):null;
   const revision=acknowledgedRevisions.get(tab.target.owner);
   if(!revision)return;
   const response=await rpc('guest.tabs',{owner:tab.target.owner,grantRevision:revision,tabs:rows,
    ...(action?{dispatchId:action.id,dispatchRevision:action.revision,...(action.initialSelection?{prepareId:action.prepareId,createId:action.createId}:{})}:{})});
   const accepted=registryAck(response,target);acknowledgedRevisions.set(tab.target.owner,accepted);
   if(target){currentGrant(target,accepted);const selected=tabs.get(tabKey(target));if(selected)selected.revision=accepted;}
  });
 }
 function lifecycle(type,event){
  leases.observe(type,ref(event),{createId:event.createId??null});
  documents.observe(event);
 }
 function onEvent(event){
  try{
   const target=ref(event),key=tabKey(target);targetKey(target);
   if(event.type==='created'){
    const prep=preparing.get(target.owner);
    if(!prep||prep.createId!==event.createId||prep.tab)throw new Error('Uncorrelated local Browser creation');
    lifecycle('created',event);leases.consumeCreate(target.owner,event.createId);
    const tab={target,ackTarget:target,createId:event.createId,creationCompleted:false,prep,offered:false,revision:null,lastGrantedRevision:null,tail:Promise.resolve(),queued:0,url:'about:blank',title:'',active:true,navigation:0,domReady:-1,loaded:-1,console:[],activeCall:null};
    tabs.set(key,tab);prep.tab=tab;
    // Creation establishes registry identity only. prepareGuest enables
    // capture after the own recorder proof and initialization grant exist.
    wake();return;
   }
   const tab=tabs.get(key);if(!tab)return;
   if(event.type==='file-guard-lost'){
    tab.guardLost=true;manager.reconcileInput();surface.hide();
    // A lost debugger/file guard is never reused. Retire this controller;
    // automatic recovery creates a guarded document with fresh service grants.
    fail();return;
   }
   if(tab.guardLost&&['recorder-ready','record','navigated','load-state'].includes(event.type))return;
   const cancelling=preparing.get(target.owner)?.tab===tab&&preparing.get(target.owner)?.phase==='cancelled';
   if(['generation','crashed','closed','debugger-detached'].includes(event.type)){
    const priorTarget=tab.target;
    const observedAction=tab.activeCall,observedPrep=tab.prep;
    lifecycle(event.type,event);tab.target=target;
    if(event.type==='generation'){
     tab.loaded=-1;tab.domReady=-1;
     if(tab.offered)void append(tab,async()=>{
      if(observedPrep&&!phaseCurrent(observedPrep))return;
      if(!same(tab.ackTarget,priorTarget))throw new Error('Browser generation did not follow its acknowledged predecessor');
      const payload={owner:target.owner,...(observedPrep?{prepareId:observedPrep.prepareId}:{fromTarget:tab.ackTarget}),target,grantRevision:tab.revision,
       ...(!observedPrep&&observedAction&&navigationAction(observedAction)?{dispatchId:observedAction.id,dispatchRevision:observedAction.revision}:{})};
      tab.revision=ack(await rpc('guest.generation',payload),observedPrep?.prepareId??'',target);
      tab.ackTarget=target;acknowledgedRevisions.set(target.owner,tab.revision);
     });
    }else if(tab.offered&&tab.creationCompleted&&!cancelling)void append(tab,async()=>{
     const action=observedAction&&(['close','close_page'].includes(observedAction.op)||observedAction.op==='tabs'&&observedAction.args.closeId)?observedAction:null;
     const accepted=registryAck(await rpc('guest.closed',{owner:target.owner,event,grantRevision:tab.revision,
      ...(action?{dispatchId:action.id,dispatchRevision:action.revision}:{})}),target);
     tab.revision=accepted;acknowledgedRevisions.set(target.owner,accepted);
     if(observedAction&&!action){unknownState.latch(target.owner);fail();}
    });
    refreshPresentation();wake();return;
   }
   if(event.type==='recorder-ready'){
    documents.observe(event);const stamp=documents.observe({...event,type:'record'});
    if(tab.offered)void append(tab,()=>{if(documents.current(stamp)&&(!tab.prep||phaseCurrent(tab.prep)))write('guest.event',{event});});
    if(event.mainFrame&&tab.offered&&!tab.prep)void append(tab,async()=>{
     if(!documents.current(stamp))return;
     const payload={owner:target.owner,target,grantRevision:tab.revision,recorderReady:true,recorderSourceSha256:recorder.recorderSourceSha256,document:documents.mainProof(target)};
     tab.revision=ack(await rpc('target.ready',payload),'',target);
     acknowledgedRevisions.set(target.owner,tab.revision);
    });
    wake();return;
   }
   if(event.type==='recorder-document-retired'){
    documents.observe(event);if(tab.offered)void append(tab,()=>write('guest.event',{event}));return;
   }
   if(event.type==='record'){
    const doc=documents.observe(event),capture=leases.captureStamp({...target,document:event.document});
    if(tab.offered&&capture)void append(tab,()=>{
     if(documents.current(doc)&&leases.captureCurrent(capture)&&!unknownState.has(target.owner))write('guest.event',{event,captureRevision:capture.grantRevision});
    });return;
   }
   if(event.type==='tabs'){
    for(const item of event.tabs||[]){const known=tabs.get(tabKey(item));if(known){known.url=String(item.url||'');known.title=String(item.title||'');known.active=item.active===true;}}
    const action=tab.activeCall&&(tab.activeCall.initialSelection||(tab.activeCall.op==='tabs'&&(tab.activeCall.args.selectId||tab.activeCall.args.closeId))||['close','close_page'].includes(tab.activeCall.op))?tab.activeCall:null;
    if(tab.offered&&!cancelling)void inventory(tab,action);refreshPresentation();wake();return;
   }
   if(event.type==='navigated'){tab.url=String(event.url||'');tab.navigation++;if(tab.offered)write('guest.event',{event});wake();return;}
   if(event.type==='load-state'){
    if(event.nativeEvent==='dom-ready'&&event.mainDocumentReady===true&&event.document&&same(tab.target,target)){
     const own=documents.mainProof(target);
     if(event.document.documentNonce===own.documentNonce&&event.document.webContentsId===own.webContentsId)tab.domReady=target.generation;
    }
    if(event.nativeEvent==='did-finish-load'&&event.finishObserved===true&&event.mainDocumentReady===true&&event.document&&same(tab.target,target)){
     const own=documents.mainProof(target);
     if(event.document?.documentNonce===own.documentNonce&&event.document?.webContentsId===own.webContentsId)tab.loaded=target.generation;
    }wake();return;
   }
   if(event.type==='cdp'&&event.method==='Runtime.consoleAPICalled'){
    tab.console.push({type:event.params?.type||'log',text:(event.params?.args||[]).map(arg=>typeof arg.value==='string'?arg.value:arg.description||'').join(' ').slice(0,4000)});if(tab.console.length>100)tab.console.shift();
   }
  }catch{fail();}
 }
 async function prepareGuest({target,createId,command}){
  const tab=tabs.get(tabKey(target)),p=tab?.prep;
  if(!tab||!p||!phaseCurrent(p)||createId!==p.createId||!same(tab.target,target))throw new Error('Browser preparation changed');
  const proof=documents.mainProof(target);
  const response=await rpc('target.offer',{owner:p.owner,prepareId:p.prepareId,createId,grantRevision:p.grantRevision,target,webContentsId:proof.webContentsId});
  if(!phaseCurrent(p))throw new Error('Browser preparation cancelled during offer');
  tab.revision=ack(response,p.prepareId,target);tab.ackTarget=target;tab.offered=true;acknowledgedRevisions.set(target.owner,tab.revision);
  p.ackTarget=target;p.phase='offered';
  const fence=leases.dispatchFence(target,tab.revision,'initialize');
  tab.initializationFence=fence;
  await initialization.run(fence,()=>command('Runtime.enable',{}));
  if(!fence.current()||!same(tab.target,target))throw new Error('Browser initialization authority changed');
  if(leases.authorize({...target,operation:'recording'}))manager.setRecording(target,true);
  write('guest.event',{event:{type:'recorder-ready',...target,mainFrame:true,document:proof}});
 }
 function registerPreparation(id,p){
  exactObject(p,['owner','intentId','prepareId','createId','grantRevision']);
  if(id!==p.prepareId||![p.intentId,p.prepareId,p.createId].every(hex)||new Set([p.intentId,p.prepareId,p.createId]).size!==3||preparing.has(p.owner)||unknownState.has(p.owner))throw new Error('Invalid Browser preparation correlation');
  const fence=leases.ownerFence(p.owner,p.grantRevision,'create',p.createId),prep={...p,fence,tab:null,phase:'scheduled',createInvoked:false,ackTarget:null,cleanup:null};preparing.set(p.owner,prep);return prep;
 }
 async function prepare(id,p){
  const prep=preparing.get(p.owner);if(!prep||prep.prepareId!==id||!phaseCurrent(prep))return;
  try{
   if(!prep.fence.current())throw new Error('Browser creation expired');
   prep.phase='creating';prep.createInvoked=true;
   const target=await manager.create({owner:p.owner,url:'about:blank',createId:p.createId}),tab=prep.tab;
   await drain(p.owner);if(!phaseCurrent(prep))return;
   if(!same(tab.target,target))throw new Error('Final Browser generation changed');
   const document=documents.mainProof(target),revision=tab.revision;currentGrant(target,revision);
   const response=await rpc('target.ready',{owner:p.owner,prepareId:p.prepareId,createId:p.createId,grantRevision:revision,target,recorderReady:true,recorderSourceSha256:recorder.recorderSourceSha256,document});
   const accepted=ack(response,p.prepareId,target);acknowledgedRevisions.set(target.owner,accepted);
   if(!phaseCurrent(prep))return;
   if(accepted!==revision||!same(tab.target,target)||documents.mainProof(target).documentNonce!==document.documentNonce)throw new Error('Final Browser proof changed while acknowledging');
   if(prepared.size>=32)throw new Error('Browser prepared intent limit');
   const timer=setTimeout(()=>prepared.delete(p.prepareId),15000);
   prep.phase='prepared';prep.ackTarget=target;
   prepared.set(p.prepareId,{owner:p.owner,intentId:p.intentId,createId:p.createId,target,document,revision:accepted,timer});tab.prep=null;
   write('browser.prepared',{owner:p.owner,intentId:p.intentId,prepareId:p.prepareId,target,grantRevision:accepted});
  }catch(error){
   // manager.create owns exact cleanup on preparation-callback failure. A later
   // failure is reported honestly; no unrelated/replacement tab is destroyed.
   if(prep.phase==='cancelled'||closed)return;
   prep.phase='failed';prep.failureCause=error;prep.cleanup=error?.creationCleanup??null;
   if(!prep.failureSent){prep.failureSent=true;write('browser.prepare-failed',{owner:p.owner,phase:'preparation',prepareId:p.prepareId,createId:p.createId});}
  }
 }
 async function cancel(id,p){
  exactObject(p,['owner','phase','prepareId','createId','cancelId','target','grantRevision']);
  const prep=preparing.get(p.owner);
  if(!prep||prep.phase==='cancelled'||prep.phase==='completed'||p.phase!=='preparation'||id!==p.cancelId||!hex(id)||p.prepareId!==prep.prepareId||p.createId!==prep.createId||!Number.isSafeInteger(p.grantRevision)||p.grantRevision<prep.grantRevision)throw new Error('Stale or foreign Browser cancellation');
  if(p.target!==null){targetKey(p.target);if(!prep.tab||p.target.owner!==p.owner||p.target.tabId!==prep.tab.target.tabId||p.target.generation>prep.tab.target.generation)throw new Error('Foreign Browser cancellation root');}
  // This phase invalidation precedes any reentrant native close callbacks.
  prep.phase='cancelled';const item=prepared.get(prep.prepareId);if(item){clearTimeout(item.timer);prepared.delete(prep.prepareId);}
  let receipt,notCreated=false;
  if(!prep.createInvoked){notCreated=true;receipt={owner:p.owner,createId:p.createId,target:null,closed:true,nativeDestroyed:true,alreadyAborted:false};}
  else if(prep.cleanup){
   // creationCleanup is deliberately nonenumerable on the original Error.
   // Validate and explicitly project the module receipt, never stringify the
   // Error or retry an already retired creation's native close.
   const value=prep.cleanup;exactObject(value,['owner','createId','target','closed','nativeDestroyed','closeUnknown']);targetKey(value.target);
   if(value.owner!==p.owner||value.createId!==p.createId||!prep.tab||!same(value.target,prep.tab.target)||[value.closed,value.nativeDestroyed,value.closeUnknown].some(flag=>typeof flag!=='boolean'))throw new Error('Invalid owned Browser creation cleanup');
   receipt={owner:value.owner,createId:value.createId,target:ref(value.target),closed:value.closed,nativeDestroyed:value.nativeDestroyed,closeUnknown:value.closeUnknown};
  }else{
   try{receipt=manager.abortCreation({owner:p.owner,createId:p.createId});}
   catch(error){
    receipt={owner:p.owner,createId:p.createId,target:prep.tab?.target??null,closed:error?.closed===true,nativeDestroyed:false,alreadyAborted:false,closeUnknown:true};
   }
  }
  if(receipt.nativeDestroyed!==true||receipt.closeUnknown===true){unknownState.latch(p.owner);manager.reconcileInput();surface.hide();}
  await drain(p.owner);
  let response;
  try{response=await rpc('browser.cancelled',{owner:p.owner,phase:'preparation',prepareId:p.prepareId,createId:p.createId,cancelId:p.cancelId,receipt,notCreated});}
  catch(error){if(unknownState.has(p.owner))return;throw error;}
  if(response.cleanupVerified!==true||response.unknown!==false||unknownState.has(p.owner))return;
  if(!Number.isSafeInteger(response.grantRevision)||response.grantRevision<p.grantRevision)throw new Error('Invalid Browser cleanup revision');
  acknowledgedRevisions.set(p.owner,response.grantRevision);preparing.delete(p.owner);
  if(prep.tab)await inventory(prep.tab,null);
 }
 async function call(id,p,dispatchSequence){
  exactObject(p,['owner','intentId','target','grantRevision','op','args'],['prepareId','createId']);targetKey(p.target);
  if(p.owner!==p.target.owner||!hex(p.intentId)||typeof p.op!=='string'||unknownState.has(p.owner))throw new Error('Invalid Browser command owner');
  const tab=tabs.get(tabKey(p.target));
  if(!tab||!tab.offered||tab.prep||!same(tab.target,p.target)||dispatchSequence<=(calls.get(p.owner)||0))throw new Error('Browser command lacks fresh exact dispatch authority');
  // Revision names a permission body, not a command counter. Distinct ordered
  // calls may use the same still-live grant; transport sequence prevents frame
  // replay and each subcommand retains this call's exact original fence.
  currentGrant(p.target,p.grantRevision);calls.set(p.owner,dispatchSequence);tab.revision=p.grantRevision;acknowledgedRevisions.set(p.owner,p.grantRevision);
  if(!tab.creationCompleted&&!p.prepareId)throw new Error('Browser creation requires its prepared first dispatch');
  if(p.prepareId){
   const prior=prepared.get(p.prepareId);
   if(!prior||prior.owner!==p.owner||prior.intentId!==p.intentId||prior.createId!==p.createId||!same(prior.target,p.target)||p.grantRevision<=prior.revision||id===p.prepareId)throw new Error('Browser first dispatch does not match completed preparation');
   // A service-ready ACK alone must not release the module's creation cap.
   // Complete only this exact final own-ready root, under the distinct fresh
   // service dispatch, before any user operation can reach the guest.
   if(tab.creationCompleted||JSON.stringify(documents.mainProof(p.target))!==JSON.stringify(prior.document))throw new Error('Browser creation proof changed or already consumed');
   manager.completeCreation({owner:p.owner,createId:prior.createId,target:p.target});
   const prep=preparing.get(p.owner);if(prep){prep.phase='completed';preparing.delete(p.owner);}
   tab.creationCompleted=true;clearTimeout(prior.timer);prepared.delete(p.prepareId);
  }
  let result;
  tab.activeCall={id,op:p.op,args:p.args,revision:p.grantRevision,prepareId:p.prepareId,createId:p.createId,initialSelection:!!p.prepareId};
  const observe=()=>({target:{...tab.target},url:tab.url,title:tab.title,loading:tab.loaded!==tab.target.generation,console:[...tab.console]});
  try{
   if(tab.activeCall.initialSelection){
    await dispatch.invoke({id:nonce(),target:p.target,revision:p.grantRevision,operation:'select',params:{}});
    await drain(p.owner);
    if(acknowledgedRevisions.get(p.owner)!==p.grantRevision)throw Object.assign(new Error('Initial Browser selection changed execution authority'),{dispatched:false});
    currentGrant(p.target,p.grantRevision);
    const active=manager.list(p.owner).find(row=>row.active);
    if(!active||!same(ref(active),p.target))throw Object.assign(new Error('New Browser root is not the actual selected guest'),{dispatched:false});
    tab.activeCall.initialSelection=false;
   }
   await settleBrowserCallRegistry({drain,leases,acknowledgedRevisions},tab,p);
   result=await executeCall({payload:p,metadata:observe,list:()=>manager.list(p.owner),
    command:(method,params={})=>dispatch.invoke({id:nonce(),target:p.target,revision:p.grantRevision,operation:'cdp',method,params}),
    invoke:(operation,params={})=>dispatch.invoke({id:nonce(),target:p.target,revision:p.grantRevision,operation,params}),
    record:on=>{currentGrant(p.target,p.grantRevision);if(unknownState.has(p.owner))throw new Error('Browser owner outcome unresolved');return manager.setRecording(p.target,on);},
    waitNavigation:async({before,readiness='load',timeoutMs=15000})=>{
     await wait(()=>{
      if(tab.navigation<=before||(readiness==='load'&&tab.loaded!==tab.target.generation)||(readiness==='domContentLoaded'&&tab.domReady!==tab.target.generation))return false;
      try{return documents.mainProof(tab.target);}catch{return false;}
     },timeoutMs);await tab.tail;
     documents.mainProof(tab.target);return observe();
    },navigation:tab.navigation,grant:leases.targetGrantOf(p.target)});
   await drain(p.owner);
   write('result',{owner:p.owner,dispatchTarget:p.target,grantRevision:p.grantRevision,target:tab.target,resultRevision:acknowledgedRevisions.get(p.owner),reply:{ok:true,result}},id);
  }catch(error){
   const uncertain=error?.dispatched!==false;
   if(uncertain){unknownState.latch(p.owner);manager.reconcileInput();surface.hide();}
   if(!closed)write('result',{owner:p.owner,dispatchTarget:p.target,grantRevision:p.grantRevision,target:tab.target,resultRevision:acknowledgedRevisions.get(p.owner),reply:{ok:false,error:'Browser operation failed',dispatched:uncertain}},id);
   if(uncertain)fail();
  }finally{tab.activeCall=null;dispatch.retireSettled(p.owner);}
 }
 function receive(message){
  if(closed)return;
  if(!identity){
   validateBrowserHello(message,{requestId:helloId,expectedVersion:version});serverSequence=message.serverSequence;
   identity={serviceEpoch:message.serviceEpoch,controllerLease:message.controllerLease};leases.establish(identity.serviceEpoch,identity.controllerLease);leases.receiveSequence(serverSequence);
   manager=managerFactory({electron,window,appSession,trustedOrigins:[origin],recorder,popupPolicy:()=>false,prepareGuest,onEvent,agentInputProtected:target=>surface?.guards(target)===true,authorize:value=>{
    const local=typeof value.tabId==='string'?tabs.get(tabKey(value)):null;
    if(local?.guardLost&&!['hide','close','list'].includes(value.operation))return false;
    if(value.initialization){
     // The module installs its mandatory fixed file guard after prepareGuest
     // returns and before loading the requested URL. It keeps the exact offer
     // fence; this never borrows a newer grant or opens public CDP permission.
     const tab=tabs.get(tabKey(ref(value)));
     if(!tab?.prep||!phaseCurrent(tab.prep))return false;
     const internal=['Page.enable','Page.setInterceptFileChooserDialog'].includes(value.method)?tab.initializationFence:null;
     const fence=initialization.getStore()||internal;
     return !!fence&&fence.current()&&same(fence.target,ref(value))&&leases.authorize(value);
    }
    return dispatch?.authorize(value)===true;
   }});
   dispatch=createBrowserDispatch({leases,manager,unknownState,onUnknown:()=>fail()});
   surface=surfaceFactory({electron,window,manager,authorize:dispatch.authorize,onTakeover:target=>onTakeover({owner:target.owner}),onError:()=>{}});
   heartbeat=setInterval(()=>{void rpc('heartbeat',{}, {timeoutMs:2500}).catch(()=>fail());},1000);
   announce('ready','');wake();return;
  }
  validateBrowserEnvelope(message,{...identity,lastSequence:serverSequence});serverSequence=message.serverSequence;leases.receiveSequence(serverSequence);
  const p=message.payload;
  if(message.op==='owner.grant'){owners.set(p.owner,leases.ownerGrant(p));}
  else if(message.op==='target.grant'){
   leases.targetGrant(p);const tab=tabs.get(tabKey(p.target));if(tab)tab.lastGrantedRevision=p.revision;
   adoptIdleBrowserGrant({leases,tabs,preparing,acknowledgedRevisions},p.target);
  }
  else if(message.op==='recording.grant'){leases.recordingGrant(p);}
  else if(message.op==='grant.revoke'){
   leases.revoke(p);manager.reconcileInput();refreshPresentation();
   if(p.scope==='all')for(const [id,item]of prepared)if(item.owner===p.owner){clearTimeout(item.timer);prepared.delete(id);}
   write('result',{ok:true,executing:dispatch.barrier(p.owner)},message.id);
  }else if(['browser.prepare','browser.call','browser.cancel'].includes(message.op)){
   if(incoming.has(message.id)||incoming.size+pending.size>=32)throw new Error('Browser incoming request collision/limit');
   incoming.add(message.id);const body=snapshot(p);if(message.op==='browser.prepare')registerPreparation(message.id,body);
   const prior=message.op==='browser.call'?(callTails.get(p.owner)||Promise.resolve()):Promise.resolve();
   const task=message.op==='browser.cancel'?cancel(message.id,body):prior.then(()=>message.op==='browser.prepare'?prepare(message.id,body):call(message.id,body,message.serverSequence));
   if(message.op==='browser.call')callTails.set(p.owner,task.catch(()=>{}));
   void task.catch(()=>{try{write('result',{owner:p.owner,dispatchTarget:p.target??null,grantRevision:p.grantRevision,target:p.target??null,resultRevision:p.grantRevision,reply:{ok:false,error:'Browser request refused',dispatched:false}},message.id);}catch{fail();}}).finally(()=>incoming.delete(message.id));
  }else if(message.op!=='result')throw new Error('Unsupported private Browser operation');
  const request=pending.get(message.id);
  // A refusal is a terminal reply even when success would carry a grant.
  // Envelope/lease/sequence validation above still precedes settlement; an
  // unrelated success or a late reply never creates authority or retries work.
  if(request&&(request.expected===message.op||(message.op==='result'&&p.ok===false))){clearTimeout(request.timer);pending.delete(message.id);if(p.ok===false)request.reject(Object.assign(new Error('Browser service refused request'),{dispatched:false}));else request.resolve(p);}
  wake();
 }
 function refreshPresentation(){
  if(closed||!surface||!desired||presentationQueued)return;
  presentationQueued=true;queueMicrotask(()=>{presentationQueued=false;if(closed||!surface||!desired)return;
   try{const shown=surface.present(desired);if(shown.shown)announce('ready','',desired.owner);}catch{surface.hide();}
  });
 }
 async function present({owner,bounds}){
  desired={owner,bounds};
  if(closed||!identity)return{shown:false,phase:closed?'error':'connecting',message:'Browser connection unavailable'};
  if(!leases.authorize({owner,tabId:null,operation:'list'}))await rpc('owner.request',{owner},{expected:'owner.grant'});
  if(desired?.owner!==owner)return{shown:false,message:'Browser selection changed'};
  try{const result=surface.present(desired);return result.shown?result:{...result,message:'Open a page with the address bar.'};}catch{return{shown:false,message:'Open a page with the address bar.'};}
 }
 function hide(){desired=null;surface?.hide();}
 function edit(operation){
  if(!['copy','cut','paste','selectAll','undo','redo'].includes(operation)||closed)return false;
  const contents=electron.webContents.getFocusedWebContents();if(!contents||contents.isDestroyed())return false;
  if(contents===window.webContents){contents[operation]();return true;}
  for(const tab of tabs.values()){
   if(!desired||desired.owner!==tab.target.owner||!tab.active||!dispatch.authorize({...tab.target,operation:'direct-input'}))continue;
   try{if(documents.mainProof(tab.target).webContentsId===contents.id){contents[operation]();return true;}}catch{}
  }return false;
 }
 function close(){
  if(closed)return;closed=true;clearInterval(heartbeat);clearTimeout(expiry);dispatch?.close();leases.clear();hide();surface?.dispose();manager?.dispose();documents.reset();
  for(const request of pending.values()){clearTimeout(request.timer);request.reject(Object.assign(new Error('Private Browser disconnected'),{dispatched:false}));}pending.clear();wake();channel?.close();
  for(const item of prepared.values())clearTimeout(item.timer);prepared.clear();
 }
 const helloId=nonce();
 async function start(){if(started)throw new Error('Browser controller already started');started=true;
  try{channel=await channelFactory({origin,token,onMessage:receive,onClose:fail});write('hello',{clientInstance:nonce(),protocol:1,guestManagerSha:guestManagerSha256},helloId);await wait(()=>identity,3000);}catch{fail();}
 }
 return Object.freeze({start,present,hide,close,edit,state:()=>Object.freeze({...state}),isClosed:()=>closed});
}
module.exports={createDesktopBrowserController,MAX_SEQUENCE,adoptIdleBrowserGrant,settleBrowserCallRegistry};
