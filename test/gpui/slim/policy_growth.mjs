// What one committed event costs the policy: resident memory per event kind and how a view request slows as the state grows.
// usage: bun policy_growth.mjs BIN PORT   (synthetic events only; run capped on Boat). Prints bytes/event and V latency per kind.
import {spawn} from 'node:child_process';import {readFileSync} from 'node:fs';import net from 'node:net';
const [bin,port]=process.argv.slice(2);
const p=spawn('sh',['-c',`ulimit -s 1048576; exec ${bin} ${port}`],{stdio:['ignore','pipe','inherit']});
await new Promise(r=>p.stdout.once('data',r));
const pid=p.pid;const rss=()=>{for(const q of [pid,...readFileSync(`/proc/${pid}/task/${pid}/children`,'utf8').trim().split(/\s+/).filter(Boolean).map(Number)]){try{const m=/VmRSS:\s+(\d+)/.exec(readFileSync(`/proc/${q}/status`,'utf8'));if(m&&readFileSync(`/proc/${q}/comm`,'utf8').includes('bp-policy'))return +m[1]}catch{}}return +(/VmRSS:\s+(\d+)/.exec(readFileSync(`/proc/${pid}/status`,'utf8'))||[0,0])[1]};
const s=net.connect(Number(port),'127.0.0.1');await new Promise(r=>s.once('connect',r));
let buf='',w=null;s.on('data',d=>{buf+=d;let i;while((i=buf.indexOf('\n'))>=0){const l=buf.slice(0,i);buf=buf.slice(i+1);w?.(l)}});
const ask=l=>new Promise(r=>{w=r;s.write(l+'\n')});
const t0=1700000000;let seq=0;const th='t1700000001-2';
const base=[{$:'ProjectCreated',id:'p1700000000-1',title:'soak',root:'/tmp/soak',at:t0},{$:'ThreadCreated',id:th,project:'p1700000000-1',title:'T',env:'local',branch:'',worktree:'',provider:'claude',model:'',at:t0+1}];
const kinds={
  message:i=>({$:'MessagePosted',thread:th,msg:`m${t0+i}-${i+10}`,role:i%2?'assistant':'user',text:'Answer '+i+': the board layout keeps U3 near the 3V3 rail, and decoupling sits within 2 mm of each supply pin.',at:t0+10+i}),
  activity:i=>({$:'ActivityLogged',thread:th,act:`g${t0+i}-${i+300}`,tone:'tool',kind:'Bash',summary:'ls -la src/core | head -'+i,at:t0+10+i}),
  turn:i=>({$:'TurnChanged',thread:th,turn:i%2?'running':'completed',at:t0+10+i}),
  setting:i=>({$:'SettingSet',key:'tok.basis.'+th+'.'+(i%50),value:'{"n":'+i+'}',at:t0+10+i}),
};
const send=async(evs)=>{for(let i=0;i<evs.length;i+=200)await ask('F '+JSON.stringify({t:'changes',items:evs.slice(i,i+200)}))};
await ask('F '+JSON.stringify({t:'log',since:0,origin:'1',version:'x',info:{},items:base}));
const sess={sel:th,rid:1,drafts:{},open:{},adding:false,panel:false,info:{}};
const V=async()=>{const t=performance.now();await ask('V '+JSON.stringify({hub:'h',cid:'c',session:sess,target:{kind:'thread',id:th},action:'',value:''}));return Math.round(performance.now()-t)};
const N=Number(process.env.N||5000);
const STRIP=process.env.STRIP==='1';
if(STRIP){const m=kinds.message,a=kinds.activity;kinds.message=i=>({...m(i),text:''});kinds.activity=i=>({...a(i),summary:''})}
console.log((STRIP?'STRIPPED text/summary':'full payloads')+'\nkind        events   rss_kb_before   rss_kb_after   bytes/event   fold ms/1000 ev   V_ms before->after');
for(const [k,mk] of Object.entries(kinds)){
  const r0=rss(),v0=await V();const evs=[];for(let i=0;i<N;i++)evs.push(mk(seq++));const tf=performance.now();await send(evs);const fold=Math.round((performance.now()-tf)*1000/N);const r1=rss(),v1=await V();
  console.log(k.padEnd(10),String(N).padStart(8),String(r0).padStart(14),String(r1).padStart(14),String(Math.round((r1-r0)*1024/N)).padStart(13),String(fold).padStart(16),`     ${v0} -> ${v1}`);
}
p.kill();process.exit(0);
