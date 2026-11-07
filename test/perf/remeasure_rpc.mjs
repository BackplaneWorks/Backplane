const BP_ROOT=new URL('../..',import.meta.url).pathname.replace(/\/$/,'');
// Event-driven re-measure of ping and write acks (the earlier sleep(20) poll quantised them to 20 ms). bun remeasure_rpc.mjs OUT.json JS_BIN [RUST_BIN]
import {mkdirSync,rmSync,writeFileSync} from 'node:fs';
import {makeStandins,startHub,stopHub,client,sleep,pct} from './lib.mjs';
const [out,jsBin,rustBin]=process.argv.slice(2);const R=`${BP_ROOT}/build/gpui/perf-run/rpc2`;rmSync(R,{recursive:true,force:true});mkdirSync(R,{recursive:true});makeStandins(R+'/bin');
const proj=R+'/p';mkdirSync(proj,{recursive:true});
const log=[{$:'ProjectCreated',id:'p1',title:'e',root:proj,at:1},{$:'ThreadCreated',id:'t1',project:'p1',title:'T',env:'local',branch:'',worktree:'',provider:'claude',model:'',at:2},{$:'SettingSet',key:'restart.continue',value:'off',at:3}].map(x=>JSON.stringify(x)).join('\n')+'\n';
const st=a=>a.length?{n:a.length,p50:pct(a,50),p95:pct(a,95),p99:pct(a,99),max:Math.max(...a)}:null;
const res={sample:{},method:'event-driven client (resolved in onmessage)'};
for(const [name,bin,port] of [['js',jsBin,3801],['rust',rustBin,3802]]){if(!bin)continue;
 const home=`${R}/${name}-home`;mkdirSync(home,{recursive:true});writeFileSync(home+'/events.jsonl',log);
 const h=await startHub({bin,home,port,standins:R+'/bin'});const c=await client(h);await sleep(300);
 res.sample[name]={ping:JSON.stringify((await c.rpc('load.ping',{}))?.o).slice(0,160),rename:JSON.stringify((await c.rpc('thread.rename',{thread:'t1',title:'x'}))?.o).slice(0,160)};
 for(let i=0;i<30;i++)await c.rpc('load.ping',{});
 const ping=[];let okPing=0;for(let i=0;i<500;i++){const r=await c.rpc('load.ping',{});if(r){ping.push(r.ms);if(r.o?.ok)okPing++}}
 const wr=[];let okW=0;for(let i=0;i<2000;i++){const r=await c.rpc('thread.rename',{thread:'t1',title:'w'+i});if(r){wr.push(r.ms);if(r.o?.ok)okW++}}
 res[name]={ping:st(ping),pingOk:okPing,rename:st(wr),renameOk:okW,renamePerSec:2000/(wr.reduce((a,b)=>a+b,0)/1000)};
 c.ws.close();stopHub(h);await sleep(1500)}
writeFileSync(out,JSON.stringify(res,null,1));console.log(JSON.stringify(res,null,1));process.exit(0);
