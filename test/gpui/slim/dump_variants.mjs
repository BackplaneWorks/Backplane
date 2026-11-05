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
const pairs=[];async function run(evs,tag){
  const p=spawn('sh',['-c',`ulimit -s 1048576; exec ${bin} ${port}`],{stdio:['ignore','pipe','inherit']});
  await new Promise(r=>p.stdout.once('data',r));
  const s=net.connect(Number(port),'127.0.0.1');await new Promise(r=>s.once('connect',r));
  let buf='',w=null;s.on('data',d=>{buf+=d;let i;while((i=buf.indexOf('\n'))>=0){const l=buf.slice(0,i);buf=buf.slice(i+1);w?.(l)}});
  const ask=l=>new Promise(r=>{w=r;s.write(l+'\n')});
  await ask('F '+JSON.stringify({t:'log',since:0,origin:'1',version:'x',info:{},items:evs.slice(0,200)}));
  for(let i=200;i<evs.length;i+=200)await ask('F '+JSON.stringify({t:'changes',items:evs.slice(i,i+200)}));
  const base={hub:'h',cid:'c',now:(t0+100000)*1000,action:'',value:''};
  const V=async(sel,target,extra={})=>{const params={...base,session:{sel,rid:1,drafts:{},open:{},adding:false,panel:false,info:globalThis.INFO||{}},target,...extra};const o=JSON.parse(await ask('V '+JSON.stringify(params)));pairs.push({params,out:o,tag});return o};
  const out={recent:await V('',{kind:'recent',id:''}),thread:await V(TH[3],{kind:'thread',id:TH[3]}),threadPage:await V(TH[3],{kind:'thread',id:TH[3]},{page:pageOf(evs,TH[3])})};
  p.kill();await new Promise(r=>setTimeout(r,300));return out;
}
const diff=(a,b,path,out)=>{if(JSON.stringify(a)===JSON.stringify(b))return;if(a&&b&&typeof a==='object'&&typeof b==='object'&&!Array.isArray(a)&&!Array.isArray(b)){for(const k of new Set([...Object.keys(a),...Object.keys(b)]))diff(a[k],b[k],path+'.'+k,out);return}
  if(Array.isArray(a)&&Array.isArray(b)&&a.length===b.length){a.forEach((x,i)=>diff(x,b[i],path+'['+i+']',out));return}out.push(path+': '+JSON.stringify(a)?.slice(0,80)+' vs '+JSON.stringify(b)?.slice(0,80))};

const T=TH[3];const A=t0+100000-50; // the events' own clock is before the requests' `now`
const variants={
  base:[],
  running:[{$:'TurnChanged',thread:T,state:'running',at:A}],
  failed:[{$:'TurnChanged',thread:T,state:'failed',at:A}],
  interrupted:[{$:'TurnChanged',thread:T,state:'interrupted',at:A}],
  pinned:[{$:'ThreadPinned',id:T,pinned:true,at:A}],
  archived:[{$:'ThreadArchived',id:T,archived:true,at:A}],
  snoozed:[{$:'ThreadSnoozed',id:T,until:t0+100000+86400,at:A}],
  codex:[{$:'ProviderSet',id:T,provider:'codex',model:'',at:A}],
  opus:[{$:'ModesSet',id:T,model:'claude-opus-4',runtime:'',interaction:'',at:A}],
  completed:[{$:'TurnChanged',thread:T,state:'completed',at:A}],
  settledhand:[{$:'ThreadSettled',id:T,manual:true,at:A}],
  askapproval:[{$:'AskOpened',id:'k1',thread:T,kind:'approval',tool:'Bash',detail:'rm -rf build',options:'',at:A}],
  askinput:[{$:'AskOpened',id:'k2',thread:T,kind:'input',tool:'',detail:'Which board?',options:'A\nB',at:A}],
  askplan:[{$:'AskOpened',id:'k3',thread:T,kind:'plan',tool:'',detail:'# Plan\n\n- step one\n- step two',options:'',at:A}],
  asksend:[{$:'AskOpened',id:'k4',thread:T,kind:'approval',tool:'gmail_send',detail:'Draft: hi',options:'',at:A}],
  stepchange:[{$:'ActivityLogged',thread:T,act:'step:c2',tone:'step',kind:'kicad',summary:JSON.stringify({e:'g1700003339-3639',c:'c2',p:'c1',g:'/x/.backplane/history.git',t:'/x',f:['M\tpower/board.kicad_pcb','A\tnew.kicad_sch'],w:''}),at:A}],
  stepbase:[{$:'ActivityLogged',thread:T,act:'step:c1',tone:'step',kind:'kicad',summary:JSON.stringify({e:'m1700003338-3348',c:'c1',p:'',g:'/x/.backplane/history.git',t:'/x',f:['A\tboard.kicad_pcb'],w:'start'}),at:A}],
  stepfirst:[{$:'ActivityLogged',thread:T,act:'step:c3',tone:'step',kind:'kicad',summary:JSON.stringify({e:'g1700003339-3639',c:'c3',p:'',g:'/x/.backplane/history.git',t:'/x',f:['A\tboard.kicad_pcb','D\told.kicad_sch'],w:''}),at:A}],
  stepopen:{info:{'fold:g1700003339-3639':'1'},extra:[{$:'ActivityLogged',thread:T,act:'step:c2',tone:'step',kind:'kicad',summary:JSON.stringify({e:'g1700003339-3639',c:'c2',p:'c1',g:'/x/.backplane/history.git',t:'/x',f:['M\tboard.kicad_pcb'],w:''}),at:A},{$:'ActivityLogged',thread:T,act:'err1',tone:'error',kind:'result',summary:'',at:A+1}]},
  apierror:[{$:'MessagePosted',thread:T,msg:'ma1',role:'assistant',text:'API Error: 529 {"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}',at:A},{$:'ActivityLogged',thread:T,act:'e1',tone:'error',kind:'result',summary:'',at:A+1}],
  apierror2:[{$:'MessagePosted',thread:T,msg:'ma2',role:'assistant',text:'API Error: 400 {"error":{"type":"invalid_request_error","message":"prompt is too long."}}',at:A},{$:'ActivityLogged',thread:T,act:'e2',tone:'error',kind:'result',summary:'API Error: 400 {"error":{"type":"invalid_request_error","message":"prompt is too long."}}',at:A+1}],
  restartcarry:[{$:'MessagePosted',thread:T,msg:'mu1',role:'user',text:'Backplane restarted while you were working. Continue where you left off.',at:A},{$:'ActivityLogged',thread:T,act:'r1',tone:'info',kind:'restart',summary:'Continuing after the restart',at:A+1}],
  blankreply:[{$:'MessagePosted',thread:T,msg:'mb1',role:'assistant',text:'  \n ',at:A}],
  thought:[{$:'ActivityLogged',thread:T,act:'h1',tone:'thinking',kind:'thinking',summary:'hmm',at:A}],
  thoughton:{extra:[{$:'SettingSet',key:'agent.thinking',value:'on',at:A},{$:'ActivityLogged',thread:T,act:'h2',tone:'thinking',kind:'thinking',summary:'hmm',at:A}]},
  tonethink:[{$:'ActivityLogged',thread:T,act:'h3',tone:'think',kind:'thinking',summary:'hmm',at:A}],
  imagereply:[{$:'MessagePosted',thread:T,msg:'mi1',role:'assistant',text:'see /tmp/board.png and ![x](y.jpg)',at:A}],
  attached:[{$:'MessagePosted',thread:T,msg:'mt1',role:'user',text:'look at this\n\n[attachment: /tmp/a.pdf]',at:A}],
  effort:[{$:'SettingSet',key:'effort.'+T,value:'high',at:A}],
  waitset:[{$:'SettingSet',key:'wait.'+T,value:'mq9',at:A}],
  filesmax:[{$:'SettingSet',key:'files.max.mb',value:'1024',at:A}],
  permdefault:[{$:'SettingSet',key:'permissions.default',value:'approval-required',at:A}],
  stalled:[{$:'SettingSet',key:'liveness.'+T,value:JSON.stringify({last:1,nudged:0,stalled:true,reported:false,reason:'no output for 10m'}),at:A}],
  askbrowser:[{$:'AskOpened',id:'kb',thread:T,kind:'browser',tool:'',detail:'Sign in please',options:'',at:A}],
  askbuy:[{$:'SettingSet',key:'buy.'+T,value:'p1',at:A},{$:'AskOpened',id:'kq',thread:T,kind:'input',tool:'order_ask',detail:'Which finish?',options:'HASL\nENIG',at:A}],
  queued:[{$:'TurnQueued',thread:T,msg:'q1',text:'later please',at:A}],
};
const out={};
for(const [name,spec] of Object.entries(variants)){const extra=Array.isArray(spec)?spec:spec.extra;globalThis.INFO=Array.isArray(spec)?{}:spec.info;pairs.length=0;await run([...L,...extra],name);const p=pairs[2];out[name]={extra,info:globalThis.INFO,page:extra.some(e=>e.$==='ActivityLogged'||e.$==='MessagePosted')?p.params.page:undefined,thread:p.out.shown?.thread??null,refused:p.out.refused??null,keys:Object.keys(p.out)}}
import {writeFileSync} from 'node:fs';
const base=out.base.thread;const slim={};
for(const [n,v] of Object.entries(out)){if(!v.thread){slim[n]={extra:v.extra,page:v.page,thread:null};continue}const d={};for(const k of Object.keys(v.thread))if(k!=='entries'&&JSON.stringify(v.thread[k])!==JSON.stringify(base[k]))d[k]=v.thread[k];
  slim[n]={extra:v.extra,page:v.page,info:v.info||{},all:Object.keys(v.info||{}).length?v.thread.entries:undefined,diff:d,tail:v.thread.entries.slice(-2),entries:v.thread.entries.length}}
writeFileSync(process.env.OUT||'variants.json',JSON.stringify(slim));
for(const [n,v] of Object.entries(out))console.log(n,v.thread?Object.keys(v.thread).length:'none',v.keys.join(','));
process.exit(0);
