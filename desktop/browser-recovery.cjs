'use strict';
// Main owns reconnection even when Settings is open or the renderer reloads.
// This retries channel establishment only, never an outstanding browser call.
function createBrowserRecovery({attempt,active,schedule=setTimeout,cancel=clearTimeout}) {
 let timer=null,tries=0,pending=null,disposed=false;
 function clear(){if(timer!==null)cancel(timer);timer=null;}
 function changed(state){
  if(disposed)return;
  if(state.phase==='ready'){tries=0;clear();return;}
  if(state.phase!=='error'||timer!==null||!active())return;
  timer=schedule(()=>{timer=null;void retry();},Math.min(30000,500*2**Math.min(tries++,6)));
  timer?.unref?.();
 }
 function retry(){
  if(disposed||!active())return Promise.resolve();
  if(pending)return pending;
  clear();
  pending=Promise.resolve().then(attempt).catch(()=>({phase:'error'})).then(state=>{
   pending=null;if(!disposed)changed(state||{phase:'error'});
  });
  return pending;
 }
 return Object.freeze({changed,retry,dispose(){disposed=true;clear();}});
}
module.exports={createBrowserRecovery};
