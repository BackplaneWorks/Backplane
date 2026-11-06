// Hub-level rows through the phone data/1 socket (JSON), on a temp home with stand-in agents: bun test/e2e/f-rows.mjs HUB_BIN OUTDIR [port]
// F01 hello, F03 thread actions (rename, pin, settle, archive, delete) survive a restart, F04 a stand-in turn streams and a restart ends it as interrupted.
// Prints one JSON line per check: {id, check, ok, evidence}. Exit code 0 always: read the lines.
import {mkdirSync,rmSync,writeFileSync,readFileSync,existsSync} from 'node:fs';
import {makeStandins,startHub,stopHub,sleep,client} from '../perf/lib.mjs';
const [bin,out,port0]=process.argv.slice(2);const port=Number(port0||3850);process.env.NO_SCOPE='1';
rmSync(out,{recursive:true,force:true});mkdirSync(out+'/bin',{recursive:true});makeStandins(out+'/bin');mkdirSync(out+'/proj',{recursive:true});
const res=[];const rec=(id,check,ok,evidence)=>{const o={id,check,ok,evidence};res.push(o);console.log(JSON.stringify(o))};
let hub;const up=async()=>{hub=await startHub({bin,home:out+'/home',port,standins:out+'/bin',extraEnv:{FAKE_SECS:'600',FAKE_PER:'2',FAKE_CYCLE:'20'},log:out+'/hub.log'})};
const down=async()=>{stopHub(hub);for(let i=0;i<100&&existsSync(`/proc/${hub.pid}`);i++)await sleep(100);await sleep(400)};
async function sock(){const ws=new WebSocket(`ws://127.0.0.1:${port}/ws?data=1&enc=json&token=${hub.token}`);const c={ws,id:0,w:new Map(),push:[],hello:null};
  ws.onmessage=e=>{const o=JSON.parse(e.data);if(o.t==='hello')c.hello=o;else if(o.t==='reply'){c.w.get(o.id)?.(o);c.w.delete(o.id)}else c.push.push(o)};
  await new Promise((r,j)=>{ws.onopen=r;ws.onerror=j});
  c.rpc=(m,p={},ms=20000)=>new Promise(r=>{const i=++c.id;const t=setTimeout(()=>{c.w.delete(i);r({ok:false,error:'timeout'})},ms);c.w.set(i,o=>{clearTimeout(t);r(o)});ws.send(JSON.stringify({id:i,m,p}))});
  for(let i=0;i<50&&!c.hello;i++)await sleep(100);return c}
const threads=async c=>(await c.rpc('threads.list',{limit:100,archived:false})).result?.threads||[];
await up();
// F01
const hr=await fetch(`http://127.0.0.1:${port}/hello`);const hb=Buffer.from(await hr.arrayBuffer());
rec('F01','/hello answers on a temp home',hr.status===200&&hb.length>10,`status ${hr.status}, ${hb.length} bytes, home ${out}/home, BACKPLANE_AGENTS=${readFileSync(`/proc/${hub.pid}/environ`,'utf8').split('\0').find(x=>x.startsWith('BACKPLANE_AGENTS='))}`);
let c=await sock();rec('F01','data/1 hello frame',!!c.hello,JSON.stringify(c.hello||{}).slice(0,200));
const idof=x=>x&&typeof x==='object'?x.id:x;
const p=idof((await c.rpc('project.add',{path:out+'/proj'})).result?.project);
const mk=async n=>idof((await c.rpc('thread.create',{project:p,title:'walk '+n})).result?.thread);const t1=await mk('one'),t2=await mk('two'),t3=await mk('three');
rec('F02','project.add and thread.create answer; threads.list shows them',!!(p&&t1&&t2&&t3)&&(await threads(c)).length>=3,`project ${p} threads ${[t1,t2,t3]}`);
// F03
const act=async(t,a,x={})=>(await c.rpc('thread.act',{thread:t,action:a,...x}));
const get=async t=>(await threads(c)).find(x=>x.id===t);
await act(t1,'rename',{title:'renamed one'});await act(t1,'pin');await act(t2,'settle');await act(t3,'archive');
const s1=await get(t1),s2=await get(t2);const arch=(await c.rpc('threads.list',{limit:100,archived:true})).result?.threads||[];
rec('F03','rename, pin, settle, archive take effect',s1?.title==='renamed one'&&s1?.pinned===true&&s2?.settled===true&&arch.some(x=>x.id===t3),`t1 ${JSON.stringify({title:s1?.title,pinned:s1?.pinned})} t2.settled ${s2?.settled} archived has t3 ${arch.some(x=>x.id===t3)}`);
// F04 turn streams
const sent=await c.rpc('message.send',{thread:t1,text:'zqshort hello stand-in'});
let streamed=false;for(let i=0;i<150&&!streamed;i++){await sleep(200);const e=(await c.rpc('thread.entries',{thread:t1,limit:20})).result?.entries||[];streamed=e.some(x=>x.kind==='assistant'&&x.text)}
rec('F04','stand-in turn streams an assistant entry',streamed&&sent.ok!==false,`message.send ok=${sent.ok}`);
// a long turn, then restart mid-turn
await c.rpc('message.send',{thread:t2,text:'stream for the long session'});await sleep(1500);
const live=(await threads(c)).find(x=>x.id===t2);const pre=live?.state;
{const k=await client(hub);const r=await k.rpc('setting.set',{key:'restart.continue',value:'off'},10000);console.log(JSON.stringify({setting_set:r?.o?.ok}));k.ws.close()} // the default resumes cut turns (F04b below); off ends them as interrupted
await act(t1,'unpin');await act(t3,'unarchive');
c.ws.close();await down();await up();c=await sock();
const s1b=await get(t1),s2b=await get(t2);const e2=(await c.rpc('thread.entries',{thread:t2,limit:50})).result?.entries||[];
rec('F03','thread state survives a hub restart',s1b?.title==='renamed one'&&s1b?.pinned===false&&s2b?.settled!==undefined,`title ${s1b?.title} pinned ${s1b?.pinned} t2.settled ${s2b?.settled}`);
rec('F04','restart.continue off: a restart ends a running turn (not still working)',pre==='working'?s2b?.state!=='working':false,`state before ${pre}, after restart ${s2b?.state}; last entries ${e2.slice(-2).map(x=>x.kind+':'+String(x.text||'').slice(0,40)).join(' | ')}`);
await act(t2,'delete');rec('F03','delete removes the thread',!(await get(t2)),'');
c.ws.close();await down();
writeFileSync(out+'/f-rows.json',JSON.stringify(res,null,1));process.exit(0);
