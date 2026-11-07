const BP_ROOT=new URL('../..',import.meta.url).pathname.replace(/\/$/,'');
// bun windows_ref.mjs : the JS hub's `window` (and the exact request) for several threads of the pages fixture,
// the reference the native window computation must equal. Synthetic log, stand-ins only,
// 3.5 GiB cap.
import {mkdirSync,rmSync,copyFileSync,writeFileSync} from 'node:fs';
import {makeStandins,startHub,stopHub,sleep} from '../perf/lib.mjs';
import {viewClient} from '../perf/view.mjs';
const R=BP_ROOT,root=R+'/build/gpui/hub-run/windows-ref';rmSync(root,{recursive:true,force:true});mkdirSync(root+'/home',{recursive:true});makeStandins(root+'/bin');
copyFileSync(R+'/test/hub/fixtures/pages.jsonl',root+'/home/events.jsonl');
const h=await startHub({bin:R+'/build/gpui/backplane-serve',home:root+'/home',port:3818,standins:root+'/bin',cap:'3500M'});
const v=await viewClient(h,{mobile:true,delta:false});await v.open();
const out=[],pages=[];
for(const t of ['t1','t2','t3','t5','t7','t19']){
 v.target={kind:'thread',id:t};v.viewId='';const r=await v.req('view.open');
 const res=r.x.result;out.push({thread:t,epoch:res.epoch,window:res.window,request:{target:v.target},shown:res.shown,overlay:res.overlay});
 if(t==='t19'){ // an older page, by the cursor the window gave
  const r2=await v.req('view.page',{cursor:res.window.older});const x=r2.x.result;pages.push({thread:t,cursor:res.window.older,window:x.window,shown:x.shown,overlay:x.overlay,epoch:x.epoch});}
}
writeFileSync(R+'/test/hub/fixtures/windows.json',JSON.stringify(out.map(({shown,overlay,...rest})=>rest),null,0));
writeFileSync(R+'/test/hub/fixtures/shown.json',JSON.stringify({threads:out.map(o=>({thread:o.thread,epoch:o.epoch,shown:o.shown,overlay:o.overlay})),pages}));
console.log(out.map(o=>[o.thread,o.window.ids.length,o.window.historyUnits.length,o.window.messageIds.length,o.window.contextMessages,o.window.partial.length,o.window.remaining,o.window.total,o.window.hasMore]));
v.close();stopHub(h);await sleep(500);process.exit(0);
