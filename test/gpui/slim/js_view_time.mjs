// view.open latency of the JS hub on a synthetic log. usage: bun js_view_time.mjs LOG [PORT]
import {mkdirSync,copyFileSync,rmSync} from 'node:fs';
import {makeStandins,startHub,stopHub,sleep} from '../perf/lib.mjs';
import {decode,encode} from '../wire.mjs';
import {execFileSync} from 'node:child_process';
const R='/home/h/Documents/i2cjak/backplane-gpui';const log=process.argv[2],port=Number(process.argv[3]||3824);
const root=R+'/build/gpui/slim-run/jshub';rmSync(root,{recursive:true,force:true});mkdirSync(root+'/home',{recursive:true});
makeStandins(root+'/bin');copyFileSync(log,root+'/home/events.jsonl');
const h=await startHub({bin:R+'/build/gpui/backplane-serve',home:root+'/home',port,standins:root+'/bin'});
console.log(execFileSync(R+'/test/gpui/slim/check_agents.sh',[String(h.pid),root+'/bin']).toString().trim());
await sleep(4000);const ws=new WebSocket(`ws://127.0.0.1:${port}/ws?view=1&mobile=1&token=${h.token}`);ws.binaryType='arraybuffer';
let g,t0,rev=0;const done=new Promise(r=>{ws.onmessage=({data})=>{const x=decode(data);if(x.t==='view'){g=x;t0=performance.now();ws.send(encode({id:1,m:'view.open',p:{version:1,hub:h.url,generation:g.generation,epoch:g.epoch,requestRevision:++rev,target:{kind:'recent',id:''},viewId:'',session:{sel:'',rid:1,drafts:{},open:{},adding:false,panel:false,info:{}},cid:'t'}}))}
 if(x.t==='reply'&&x.id===1)r({ms:performance.now()-t0,bytes:data.byteLength,ok:x.ok,err:JSON.stringify(x.error),threads:x.result?.shown?.projects?.reduce((a,p)=>a+(p.threads?.length||0),0)})}});
await new Promise(r=>ws.onopen=r);const r=await Promise.race([done,sleep(120000).then(()=>({timeout:true}))]);
console.log(JSON.stringify(r));stopHub(h);await sleep(500);process.exit(0);
