const BP_ROOT=new URL('../..',import.meta.url).pathname.replace(/\/$/,'');
import {mkdirSync} from 'node:fs';import {makeStandins,startHub,stopHub,client,sleep} from './lib.mjs';import {viewClient} from './view.mjs';
const REPO=BP_ROOT;const root=REPO+'/build/gpui/perf-run/m2probe';mkdirSync(root,{recursive:true});makeStandins(root+'/bin');
const h=await startHub({bin:REPO+'/build/gpui/backplane-serve',home:root+'/home',port:3803,standins:root+'/bin',extraEnv:{FAKE_SECS:'30'}});
const c=await client(h);mkdirSync(root+'/proj',{recursive:true});c.send('project.add',{path:root+'/proj'});const pc=await c.until(10000,()=>c.seen.find(x=>x.$==='ProjectCreated'));
c.send('thread.create',{project:pc.id,title:'t',env:'local'});const th=await c.until(10000,()=>c.seen.find(x=>x.$==='ThreadCreated'));c.send('turn.start',{thread:th.id,text:'go'});
for(const mobile of [true]){const v=await viewClient(h,{mobile});let r=await v.open();console.log(mobile,'open',r?.ms,r?.bytes,Object.keys(r?.x?.result||{}),r?.x?.ok,JSON.stringify(r?.x?.error));
 v.target={kind:'thread',id:th.id};v.viewId='';r=await v.req('view.open');console.log('thread open',r?.ms,r?.bytes);
 for(let i=0;i<4;i++){await sleep(700);r=await v.req('view.update');console.log('update',r?.ms.toFixed(1),r?.bytes,'delta',r?.delta,JSON.stringify(r?.x?.result?.viewDelta)?.slice(0,80))}v.close()}
stopHub(h);process.exit(0);
