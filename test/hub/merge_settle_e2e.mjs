// bun merge_settle_e2e.mjs HUB_BINARY [PORT]
// A hub on a temp home (no helper process, stand-in agents blocked): four threads with worktrees of one repository. One lands by a
// fast-forward, one by cherry-pick, one is half landed (one of its two commits is on main), one has made no commit. Only the first two
// may settle, each once, and their worktree and branch go; a new message brings a settled thread back. The merge ticker runs each minute.
import {readFileSync,existsSync,mkdirSync,mkdtempSync,writeFileSync,rmSync} from 'node:fs';
import {execFileSync,spawn} from 'node:child_process';
import {tmpdir} from 'node:os';
import {encode,decode} from '../wire.mjs';
const [bin,port='3841']=process.argv.slice(2);
const tmp=mkdtempSync(tmpdir()+'/bp-merge-e2e-');const home=tmp+'/home',repo=tmp+'/repo',stand=tmp+'/standins';
for(const d of [home,repo,stand])mkdirSync(d);
const g=(cwd,...a)=>execFileSync('git',a,{cwd,encoding:'utf8',env:{...process.env,GIT_AUTHOR_NAME:'t',GIT_AUTHOR_EMAIL:'t@t',GIT_COMMITTER_NAME:'t',GIT_COMMITTER_EMAIL:'t@t'}}).trim();
g(repo,'init','-q','-b','main');writeFileSync(repo+'/f','a\n');g(repo,'add','f');g(repo,'commit','-q','-m','a');
const hub=spawn(bin,['--home',home,'--port',port,'--no-tailscale','--foreground'],{env:{...process.env,BACKPLANE_AGENTS:stand,BACKPLANE_NO_UPDATE:'1'},stdio:['ignore','inherit','inherit']});
console.log('hub pid',hub.pid,'agents',readFileSync(`/proc/${hub.pid}/environ`,'utf8').split('\0').find(x=>x.startsWith('BACKPLANE_AGENTS=')));
let done=false;hub.on('exit',c=>{if(!done){console.log('FAIL the hub exited',c);process.exit(1)}});
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
let bad=0;const check=(ok,what)=>{console.log(ok?'ok  ':'FAIL',what);if(!ok)bad++};
const log=()=>existsSync(home+'/events.jsonl')?readFileSync(home+'/events.jsonl','utf8').split('\n').filter(Boolean).map(l=>{try{return JSON.parse(l)}catch{return {}}}):[];
const ev=(k,id)=>log().filter(e=>e.$===k&&(id===undefined||e.id===id||e.thread===id));
try{
 for(let i=0;i<200&&!existsSync(home+'/token');i++)await sleep(100);
 const token=readFileSync(home+'/token','utf8').trim();
 let ws;for(let i=0;i<100;i++){const w=new WebSocket(`ws://127.0.0.1:${port}/ws?view=1&mobile=1&enc=cbor&token=${token}`);w.binaryType='arraybuffer';
 if(await new Promise(r=>{w.onopen=()=>r(true);w.onerror=()=>r(false)})){ws=w;break}await sleep(200)} // the token is written before the port listens
 const waiting=new Map();ws.onmessage=({data})=>{try{const x=decode(data);if(x.t==='reply'&&waiting.has(x.id)){waiting.get(x.id)(x);waiting.delete(x.id)}}catch{}};
 let n=100;
 const ask=(m,p)=>new Promise(r=>{const id=++n;waiting.set(id,r);ws.send(encode({id,m,p}));setTimeout(()=>{if(waiting.delete(id))r({ok:false,timeout:m})},20000)});
 const added=await ask('project.add',{path:repo});check(added.ok,'project added '+JSON.stringify(added).slice(0,120));
 const pid=ev('ProjectCreated').find(e=>e.root===repo)?.id;
 const th={};
 for(const k of ['ff','pick','half','fresh']){const r=await ask('thread.create',{project:pid,env:'worktree',title:k});th[k]=r.thread;check(!!r.thread,'thread '+k+' '+r.thread)}
 for(let i=0;i<300&&Object.values(th).some(t=>!ev('WorktreeSet',t).length);i++)await sleep(100);
 const wt={},br={};for(const k in th){const w=ev('WorktreeSet',th[k])[0];wt[k]=w?.worktree;br[k]=w?.branch;check(wt[k]&&existsSync(wt[k]),k+' worktree '+wt[k]+' on '+br[k])}
 const put=(k,f,m)=>{writeFileSync(wt[k]+'/'+f,m+'\n');g(wt[k],'add',f);g(wt[k],'commit','-q','-m',m)};
 // landing: ff first (main has not moved), then main moves and the others are picked
 put('ff','ff.txt','ff work');g(repo,'merge','-q','--ff-only',br.ff);
 writeFileSync(repo+'/other','x\n');g(repo,'add','other');g(repo,'commit','-q','-m','main moves');
 put('pick','pick.txt','pick work');g(repo,'cherry-pick',g(wt.pick,'rev-parse','HEAD'));
 put('half','h1.txt','half 1');put('half','h2.txt','half 2');g(repo,'cherry-pick',g(wt.half,'rev-parse','HEAD~1'));
 console.log('landed: ff by fast-forward, pick by cherry-pick, half one of two commits; waiting for the ticker (60 s each)');
 for(let i=0;i<200&&!(ev('ThreadMerged',th.ff).length&&ev('ThreadMerged',th.pick).length);i++)await sleep(1000);
 await sleep(5000);
 const merged=k=>ev('ThreadMerged',th[k]).length;
 check(merged('ff')===1,'ff settled once ('+merged('ff')+')');check(merged('pick')===1,'cherry-pick settled once ('+merged('pick')+')');
 check(merged('half')===0,'half landed not settled');check(merged('fresh')===0,'a thread with no commit not settled');
 for(let i=0;i<30&&(existsSync(wt.ff)||existsSync(wt.pick));i++)await sleep(1000);
 const hasBranch=b=>{try{g(repo,'rev-parse','--verify','-q','refs/heads/'+b);return true}catch{return false}};
 check(!existsSync(wt.ff)&&!hasBranch(br.ff),'ff worktree and branch removed');check(!existsSync(wt.pick)&&!hasBranch(br.pick),'cherry-pick worktree and branch removed');
 check(existsSync(wt.half)&&hasBranch(br.half),'half landed worktree and branch kept');check(existsSync(wt.fresh)&&hasBranch(br.fresh),'fresh worktree and branch kept');
 // a message brings a settled thread back
 const before=ev('ThreadWoke',th.ff).length;
 const sent=await ask('turn.start',{thread:th.ff,text:'back again',msg:'back1'});check(sent.ok,'message to the settled thread accepted '+JSON.stringify(sent).slice(0,100));
 await sleep(3000);check(ev('ThreadWoke',th.ff).length>before,'the settled thread came back (ThreadWoke)');
 // once: another two ticks change nothing
 await sleep(125000);
 check(merged('ff')===1&&merged('pick')===1&&merged('half')===0&&merged('fresh')===0,'after two more ticks: ff '+merged('ff')+', pick '+merged('pick')+', half '+merged('half')+', fresh '+merged('fresh'));
 console.log('events:',log().filter(e=>e.$==='ThreadMerged').map(e=>e.id).join(' '));
}catch(e){console.log('FAIL exception',e);bad++}
done=true;hub.kill();await sleep(500);rmSync(tmp,{recursive:true,force:true});
console.log(bad?`FAILED ${bad}`:'ALL OK');process.exit(bad?1:0);
