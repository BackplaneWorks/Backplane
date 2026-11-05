// Capture the real hub's view-protocol conversation (GPUI client flow) incl. a send.
import {mkdirSync,writeFileSync,rmSync} from 'node:fs';
import {makeStandins,startHub,stopHub,sleep} from '../perf/lib.mjs';
import {decode,encode} from '../wire.mjs';
const R='/home/h/Documents/i2cjak/backplane-gpui';
const port=Number(process.argv[2]||3820),bin=process.argv[3]||R+'/build/gpui/backplane-serve';
const root=R+'/build/gpui/slim-run/capv';rmSync(root,{recursive:true,force:true});mkdirSync(root,{recursive:true});
makeStandins(root+'/bin');if(process.env.LOG){mkdirSync(root+'/home',{recursive:true});(await import('node:fs')).copyFileSync(process.env.LOG,root+'/home/events.jsonl')}
const h=await startHub({bin,home:root+'/home',port,standins:root+'/bin',extraEnv:{FAKE_SECS:'2',FAKE_PER:'2',FAKE_CYCLE:'5'}});
mkdirSync(root+'/proj',{recursive:true});
const log=[];const ws=new WebSocket(`ws://127.0.0.1:${port}/ws?view=1&mobile=1&token=${h.token}`);ws.binaryType='arraybuffer';
let greet,rev=0,target={kind:'recent',id:''},viewId='',next=2**47+100;const session={sel:'',rid:Date.now(),drafts:{},open:{},adding:false,panel:false,info:{}};
const req=(m,extra={})=>{const id=++next;const p={...extra,...(process.env.NATIVE==='1'?{nativeSidebar:1}:{}),version:1,hub:h.url,generation:greet.generation,epoch:greet.epoch,requestRevision:++rev,target,viewId,session,cid:'cap'};log.push({dir:'out',ts:performance.now(),o:{id,m,p}});ws.send(encode({id,m,p}))};
const raw=(id,m,p)=>{log.push({dir:'out',o:{id,m,p}});ws.send(encode({id,m,p}))};
ws.onmessage=({data})=>{const x=decode(data);log.push({dir:'in',ts:performance.now(),n:data.byteLength,o:x});
 if(x.t==='view'){greet=x;req('view.open')}
 if(x.t==='reply'&&x.ok&&x.result?.session){Object.assign(session,x.result.session);if(x.result.nextTarget&&JSON.stringify(x.result.nextTarget)!==JSON.stringify(target)){target=x.result.nextTarget;viewId='';req('view.open');return}}
 if(x.t==='reply'&&x.ok&&x.result?.viewId)viewId=x.result.viewId;
 if(x.t==='view.changed'&&x.viewId===viewId){req('view.update')}
 for(const c of x.result?.cmds??[])if(c.type==='send'){const r=JSON.parse(c.text);raw(r.id,r.m,r.p)}
};
await new Promise(r=>ws.onopen=r);await sleep(1500);
if(process.env.LOG){const o=log.filter(l=>l.dir==='out'&&l.o.m==='view.open')[0],i=log.find(l=>l.dir==='in'&&l.o.id===o.o.id);console.log(JSON.stringify({openMs:i.ts-o.ts,bytes:i.n,ok:i.o.ok}));writeFileSync(R+'/build/gpui/results/slim/js-open.json','{}');stopHub(h);await sleep(300);process.exit(0)}raw(1,'project.add',{path:root+'/proj'});await sleep(1500);
const sh=log.filter(l=>l.o.result?.shown).at(-1).o.result.shown;
const tid=sh.projects[0].threads[0].id;
req('view.action',{action:'select',value:tid});await sleep(1500);
session.drafts[tid.split('|').pop()]='hello world';
req('view.action',{action:'draft',value:'hello world'});await sleep(800);
req('view.action',{action:'send',value:''});await sleep(4500);
writeFileSync(R+'/build/gpui/results/slim/capture-view.json',JSON.stringify(log,null,1));
stopHub(h);await sleep(500);process.exit(0);
