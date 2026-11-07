const BP_ROOT=new URL('../..',import.meta.url).pathname.replace(/\/$/,'');
// bun bench_rpc.mjs RUST_BIN JS_BIN OUT.json : files.dir round trip with an event-driven client (no polling), ports 3858/3859
import {mkdirSync,rmSync,writeFileSync,cpSync} from 'node:fs';
import {makeStandins,startHub,stopHub,sleep} from '../perf/lib.mjs';
import {decode,encode} from '../wire.mjs';
const [rustBin,jsBin,out]=process.argv.slice(2);
const R=`${BP_ROOT}/build/gpui/files-run/rpc`;rmSync(R,{recursive:true,force:true});mkdirSync(R,{recursive:true});makeStandins(R+'/bin');
const proj=R+'/p';mkdirSync(proj+'/d',{recursive:true});writeFileSync(proj+'/a.txt','x');
const log=[{$:'ProjectCreated',id:'p1',title:'e',root:proj,at:1},{$:'ThreadCreated',id:'t1',project:'p1',title:'T',env:'local',branch:'',worktree:'',provider:'claude',model:'',at:2}].map(x=>JSON.stringify(x)).join('\n')+'\n';
const q=(xs,p)=>{const s=[...xs].sort((a,b)=>a-b);return s[Math.min(s.length-1,Math.floor(p*s.length))]};
const res={};
for(const [name,bin,port] of [['rust',rustBin,3858],['js',jsBin,3859]]){
 const home=`${R}/${name}-home`;mkdirSync(home,{recursive:true});writeFileSync(home+'/events.jsonl',log);
 const h=await startHub({bin,home,port,standins:R+'/bin'});
 const ws=new WebSocket(`ws://127.0.0.1:${port}/ws?view=1&mobile=1&enc=cbor&token=${h.token}`);ws.binaryType='arraybuffer';
 const waiting=new Map();ws.onmessage=({data})=>{let x;try{x=decode(data)}catch{return}if(x?.t==='reply'&&waiting.has(x.id)){waiting.get(x.id)();waiting.delete(x.id)}};
 await new Promise((r,j)=>{ws.onopen=r;ws.onerror=j});await sleep(300);
 let id=0;const rt=(m,p)=>new Promise(res=>{const i=++id;const t=performance.now();waiting.set(i,()=>res(performance.now()-t));ws.send(encode({id:i,m,p}))});
 for(let i=0;i<30;i++)await rt('files.dir',{thread:'t1',path:''}); // warm
 const dir=[],unknown=[];for(let i=0;i<300;i++){dir.push(await rt('files.dir',{thread:'t1',path:i%2?'':'d'}))}
 res[name]={n:300,dirMedianMs:q(dir,.5),dirP95Ms:q(dir,.95),dirP99Ms:q(dir,.99)};ws.close();stopHub(h)}
writeFileSync(out,JSON.stringify(res,null,1));console.log(JSON.stringify(res));process.exit(0);
