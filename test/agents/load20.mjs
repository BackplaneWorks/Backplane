// 20 stand-in agents at once, the JS hub or the Rust hub on the same synthetic log.
// usage: bun test/agents/load20.mjs js|rust BIN PORT OUTDIR [SECS=30]
// Measures: message -> first live delta per thread (ms), hub CPU (ticks) and RSS over the run, events written, turns completed.
// Stand-in agents only: BACKPLANE_AGENTS is read back from the hub's own environment before any turn (startHub).
import {mkdirSync,writeFileSync,rmSync,readFileSync} from 'node:fs';
import {startHub,stopHub,makeStandins,client,sleep,rssKb,cpuTicks,hwmKb,pct,treeRssKb,threads} from '../perf/lib.mjs';
const [kind,bin,port,out,secs='30']=process.argv.slice(2);
const root=out+'/'+kind;rmSync(root,{recursive:true,force:true});mkdirSync(root+'/proj',{recursive:true});makeStandins(root+'/bin');
const N=20,ids=[...Array(N)].map((_,i)=>`t17000010${String(i).padStart(2,'0')}-${i}`);
const L=[JSON.stringify({$:'ProjectCreated',id:'p1',title:'load',root:root+'/proj',at:1700000000}),...ids.map((id,i)=>JSON.stringify({$:'ThreadCreated',id,project:'p1',title:'load '+i,env:'local',branch:'',worktree:'',provider:'claude',model:'',at:1700000001+i}))];
mkdirSync(root+'/home',{recursive:true});writeFileSync(root+'/home/events.jsonl',L.join('\n')+'\n');
const h=await startHub({bin,home:root+'/home',port:Number(port),host:'127.0.0.1',standins:root+'/bin',extraEnv:{FAKE_SECS:secs,FAKE_PER:'2',FAKE_CYCLE:'20'},log:root+'/hub.log'});
await sleep(4000);
const first=new Map(),sent=new Map();let deltas=0,frames=0;
const c=await client(h,{onMsg:(o,now)=>{frames++;if(o.t==='delta'){deltas++;if(!first.has(o.thread)&&sent.has(o.thread))first.set(o.thread,now-sent.get(o.thread))}}});
const ticks0=cpuTicks(h.pid),t0=Date.now(),rss0=rssKb(h.pid);const samples=[];
for(const id of ids){sent.set(id,Date.now());c.send('turn.start',{thread:id,text:'stream for the load test'})}
const end=t0+Number(secs)*1000+3000;
while(Date.now()<end){samples.push({t:(Date.now()-t0)/1000,rss:rssKb(h.pid),tree:treeRssKb(h.pid),cpu:cpuTicks(h.pid)-ticks0,thr:threads(h.pid)});await sleep(1000)}
const ticks1=cpuTicks(h.pid),hwm=hwmKb(h.pid),treeMax=Math.max(...samples.map(s=>s.tree));
await sleep(500);
const log=readFileSync(root+'/home/events.jsonl','utf8').split('\n').filter(Boolean).map(l=>JSON.parse(l));
const mine=log.slice(L.length);
const lat=[...first.values()].sort((a,b)=>a-b);
const r={kind,bin,machine:'this desktop, '+new Date().toISOString(),secs:Number(secs),agents:N,deltaFirstMs:{n:lat.length,median:pct(lat,50),p95:pct(lat,95),max:lat[lat.length-1]},
 cpuSeconds:(ticks1-ticks0)/100,cpuPercentOfOneCore:+(((ticks1-ticks0)/100)/((Date.now()-t0)/1000)*100).toFixed(1),rssStartKb:rss0,rssEndKb:samples.at(-1).rss,rssMaxKb:Math.max(...samples.map(s=>s.rss)),hwmKb:hwm,treeRssMaxKb:treeMax,
 deltaFrames:deltas,frames,eventsWritten:mine.length,assistantMessages:mine.filter(e=>e.$==='MessagePosted'&&e.role==='assistant').length,completed:mine.filter(e=>e.$==='TurnChanged'&&e.state==='completed').length,failed:mine.filter(e=>e.$==='TurnChanged'&&e.state==='failed').length,
 realModelUsage:mine.filter(e=>e.$==='TurnUsage'&&!/fake/.test(e.model||'fake')).length};
writeFileSync(root+'/result.json',JSON.stringify(r,null,1));console.log(JSON.stringify(r));
const kids=readFileSync('/proc/'+h.pid+'/task/'+h.pid+'/children','utf8').trim();
stopHub(h);await sleep(3000);
const left=(()=>{try{return readFileSync('/proc/'+h.pid+'/stat','utf8')?'hub still up':''}catch{return ''}})();
console.log('children at end:',kids.split(' ').filter(Boolean).length,left||'hub gone');process.exit(0);
