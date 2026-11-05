// Is the policy's screen (apart from the selected thread's entries) the same when its log keeps only the last K messages and activities of
// each thread? usage: bun policy_compact.mjs BIN PORT [K]  (synthetic events; capped, Boat). Prints, per view, the paths that differ.
import {spawn} from 'node:child_process';import net from 'node:net';
const [bin,port,Kx]=process.argv.slice(2);const K=Number(Kx||2);
const t0=1700000000;const L=[];let at=t0;const P=o=>L.push({...o,at:++at});
const rnd=(()=>{let s=7;return()=>{s=(s*1103515245+12345)&0x7fffffff;return s/0x7fffffff}})();
const TH=[];P({$:'ProjectCreated',id:'p1700000000-1',title:'soak',root:'/tmp/soak'});P({$:'ProjectCreated',id:'p1700000000-2',title:'second',root:'/tmp/soak2'});
for(let i=0;i<14;i++){const id=`t17000000${String(i).padStart(2,'0')}-${i+5}`;TH.push(id);P({$:'ThreadCreated',id,project:i%3?'p1700000000-1':'p1700000000-2',title:'Thread '+i,env:'local',branch:'',worktree:'',provider:'claude',model:'',at:0});}
let n=0;
for(let r=0;r<120;r++)for(const id of TH){
  P({$:'MessagePosted',thread:id,msg:`m${t0+n}-${n+10}`,role:(r+n)%2?'assistant':'user',text:`Message ${r} of ${id}: the layout keeps U3 near the rail and the decoupling close, ${'x'.repeat(n%40)}`});n++;
  if(rnd()<0.5)P({$:'ActivityLogged',thread:id,act:`g${t0+n}-${n+300}`,tone:'tool',kind:'Bash',summary:'ls -la src | head -'+(n%90)});n++;
  if(rnd()<0.1)P({$:'TurnChanged',thread:id,turn:rnd()<0.5?'running':'completed'});
}
// the rows the host puts in V `page` for a thread's newest page: [seq, ord, id, thread, src, kind, role, akind, text, at], oldest first
const pageOf=(evs,th,n=40)=>{const rows=[];evs.forEach((e,i)=>{if(e.thread!==th)return;
  if(e.$==='MessagePosted')rows.push([String(i+1),String(i+1),e.msg,th,'','msg',e.role,'',e.text,String(e.at)]);
  else if(e.$==='ActivityLogged')rows.push([String(i+1),String(i+1),e.act,th,'','act',e.tone,e.kind,e.summary,String(e.at)])});
  const last=rows.slice(-n);return {thread:th,before:0,below:rows.length-last.length,above:0,rows:last.slice().reverse()}};
const compact=(evs,K)=>{const keep=new Set();const seen=new Map();
  for(let i=evs.length-1;i>=0;i--){const e=evs[i];if(e.$==='MessagePosted'||e.$==='ActivityLogged'){const k=e.$+'|'+e.thread;const c=seen.get(k)||0;if(c<K)keep.add(i);seen.set(k,c+1)}else keep.add(i)}
  return evs.filter((_,i)=>keep.has(i))};
const pairs=[];async function run(evs){
  const p=spawn('sh',['-c',`ulimit -s 1048576; exec ${bin} ${port}`],{stdio:['ignore','pipe','inherit']});
  await new Promise(r=>p.stdout.once('data',r));
  const s=net.connect(Number(port),'127.0.0.1');await new Promise(r=>s.once('connect',r));
  let buf='',w=null;s.on('data',d=>{buf+=d;let i;while((i=buf.indexOf('\n'))>=0){const l=buf.slice(0,i);buf=buf.slice(i+1);w?.(l)}});
  const ask=l=>new Promise(r=>{w=r;s.write(l+'\n')});
  await ask('F '+JSON.stringify({t:'log',since:0,origin:'1',version:'x',info:{},items:evs.slice(0,200)}));
  for(let i=200;i<evs.length;i+=200)await ask('F '+JSON.stringify({t:'changes',items:evs.slice(i,i+200)}));
  const base={hub:'h',cid:'c',now:(t0+100000)*1000,action:'',value:''};
  const V=async(sel,target,extra={})=>{const params={...base,session:{sel,rid:1,drafts:{},open:{},adding:false,panel:false,info:{}},target,...extra};const o=JSON.parse(await ask('V '+JSON.stringify(params)));pairs.push({params,out:o});return o};
  const out={recent:await V('',{kind:'recent',id:''}),thread:await V(TH[3],{kind:'thread',id:TH[3]}),threadPage:await V(TH[3],{kind:'thread',id:TH[3]},{page:pageOf(L,TH[3])})};
  p.kill();await new Promise(r=>setTimeout(r,300));return out;
}
const diff=(a,b,path,out)=>{if(JSON.stringify(a)===JSON.stringify(b))return;if(a&&b&&typeof a==='object'&&typeof b==='object'&&!Array.isArray(a)&&!Array.isArray(b)){for(const k of new Set([...Object.keys(a),...Object.keys(b)]))diff(a[k],b[k],path+'.'+k,out);return}
  if(Array.isArray(a)&&Array.isArray(b)&&a.length===b.length){a.forEach((x,i)=>diff(x,b[i],path+'['+i+']',out));return}out.push(path+': '+JSON.stringify(a)?.slice(0,80)+' vs '+JSON.stringify(b)?.slice(0,80))};
await run(L);
import {mkdirSync,writeFileSync} from 'node:fs';const dir=process.env.OUT||'dumps';mkdirSync(dir,{recursive:true});
pairs.forEach((p,i)=>writeFileSync(`${dir}/pair-${i}.json`,JSON.stringify(p)));
writeFileSync(`${dir}/log.jsonl`,L.map(e=>JSON.stringify(e)).join('\n')+'\n');
console.log('pairs',pairs.length);const t=pairs[2].out.shown.thread;console.log('thread keys',Object.keys(t).join(','),'entries',t.entries?.length);console.log(JSON.stringify(t.entries?.slice(0,3)).slice(0,1500));console.log('shown keys',Object.keys(pairs[2].out.shown).join(','));
process.exit(0);
