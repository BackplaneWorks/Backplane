// Real Git effects and canonical dispatch against a throwaway hub; fake provider only.
import {mkdtempSync,mkdirSync,writeFileSync,readFileSync,readdirSync,chmodSync,existsSync,unlinkSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {ThinViews} from '../../src/web/thin.js';
const [binary='build/multi-account/backplane-accounts-review']=process.argv.slice(2);
for(const f of readdirSync('build/multi-account/wire').filter(x=>x.endsWith('.js')))(0,eval)(readFileSync(join('build/multi-account/wire',f),'utf8'));
const W=(globalThis as any).Wire,root=mkdtempSync(join(tmpdir(),'bp-account-metadata-')),home=join(root,'home'),fake=join(root,'bin'),repo=join(root,'repo');
for(const p of [home,fake,repo])mkdirSync(p);
for(const provider of ['claude','codex','grok','muse']){const file=join(fake,provider);writeFileSync(file,['claude','codex'].includes(provider)?`#!/bin/sh\nexec '${process.execPath}' '${resolve('test/tools/accounts_standin.cjs')}' ${provider} "$@"\n`:'#!/bin/sh\nexit 127\n');chmodSync(file,0o755);}
function git(cwd:string,...args:string[]){const r=Bun.spawnSync(['git',...args],{cwd,stdout:'pipe',stderr:'pipe'});if(r.exitCode)throw Error('fixture Git failed '+args[0]);return r.stdout.toString().trim();}
git(repo,'init','-b','main');writeFileSync(join(repo,'fixture'),'bounded metadata regression\n');git(repo,'add','fixture');git(repo,'-c','user.name=Fixture','-c','user.email=fixture@example.test','commit','-m','fixture');
const listener=Bun.listen({hostname:'127.0.0.1',port:0,socket:{data(){}}}),port=listener.port;listener.stop(true);
const hold=join(root,'hold-worktree'),entered=join(root,'hold-worktree.entered'),realGit=Bun.which('git')!;
const gitWrapper=join(fake,'git');writeFileSync(gitWrapper,`#!/bin/sh
if [ "$1" = worktree ] && [ "$2" = add ] && [ -f "$BP_METADATA_HOLD" ]; then
  : > "$BP_METADATA_HOLD.entered"
  remaining=120
  while [ -f "$BP_METADATA_HOLD" ] && [ "$remaining" -gt 0 ]; do sleep 0.05; remaining=$((remaining - 1)); done
fi
exec '${realGit}' "$@"
`);chmodSync(gitWrapper,0o755);
const audit=join(root,'audit.jsonl'),proc=Bun.spawn([resolve(binary),'--home',home,'--port',String(port),'--host','127.0.0.1','--no-tailscale','--headless','--foreground','--web',resolve('build/multi-account/web-phone')],{env:{...process.env,DISPLAY:'',WAYLAND_DISPLAY:'',BACKPLANE_AGENTS:fake,PATH:fake+':'+process.env.PATH,BACKPLANE_NO_UPDATE:'1',BACKPLANE_PEERS:'',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',BP_ACCOUNTS_AUDIT:audit,BP_METADATA_HOLD:hold},stdout:Bun.file(join(root,'hub.stdout')),stderr:Bun.file(join(root,'hub.stderr'))});
console.log('fixture root '+root);let failed=false;const clients:any[]=[],checks:any[]=[];
const pause=(ms:number)=>new Promise(r=>setTimeout(r,ms));
async function until(f:()=>any,ms=12000){const end=Date.now()+ms;do{const v=f();if(v)return v;await pause(40);}while(Date.now()<end);return null;}
function check(name:string,ok:any,got?:any){checks.push({name,pass:!!ok});console.log((ok?'ok ':'FAIL ')+name+(ok?'':': '+JSON.stringify(got)?.slice(0,600)));if(!ok)failed=true;}
async function connect(web=false){const c:any={ws:null,frames:[],n:0,replies:new Map()};const token=readFileSync(join(home,'token'),'utf8').trim();c.ws=new WebSocket(`ws://127.0.0.1:${port}/ws?token=${token}`+(web?'&view=1&frontend=1':''));c.ws.binaryType='arraybuffer';c.ws.onmessage=(e:any)=>{const o=JSON.parse(W.decode(new Uint8Array(e.data)));c.frames.push(o);if(o.t==='reply')c.replies.set(Number(o.id),o);};await new Promise((r,j)=>{c.ws.onopen=r;c.ws.onerror=j;});clients.push(c);return c;}
async function rpc(c:any,m:string,p:any){const id=++c.n;c.ws.send(W.encode(JSON.stringify({id,m,p})));const r=await until(()=>c.replies.get(id),20000);if(!r)throw Error('RPC timeout '+m);return r;}
let events:any[]=[];
try{
 for(let i=0;i<150;i++){try{if((await fetch(`http://127.0.0.1:${port}/hello`)).ok)break;}catch{}await pause(50);}
 const admin=await connect(),web=await connect(true),g=await until(()=>web.frames.find((x:any)=>x.t==='view'));
 const v=new ThinViews({now:()=>performance.now(),send:(_h:any,x:any)=>x});v.connect('fixture',g);const open=v.select('fixture',{kind:'recent',id:''},{sel:'',rid:1,drafts:{},open:{},adding:false,panel:false,info:{}},'metadata-fixture');web.ws.send(W.encode(JSON.stringify(open.cmds[0])));const opened=await until(()=>web.replies.get(open.id));if(!v.reply('fixture',opened)?.current)throw Error('current view not admitted');
 const all=()=>admin.frames.flatMap((x:any)=>x.items||[]),setting=(key:string)=>all().filter((x:any)=>x.$==='SettingSet'&&x.key===key).at(-1)?.value;
 const account=async(p:any)=>{const r=await rpc(web,'accounts',p);return {...r,...r.result};};
 await rpc(admin,'project.add',{path:repo});const project=await until(()=>all().find((x:any)=>x.$==='ProjectCreated'&&x.root===repo));
 const added=await account({op:'add',provider:'codex',label:'Metadata'}),profile=added.profiles[0];writeFileSync(join(home,'providers/accounts-v1',profile.id,'fixture.json'),JSON.stringify({email:'metadata@example.test',workspace:'metadata-workspace'}));await account({op:'login',profile:profile.id});
 let verified:any;for(let i=0;i<100;i++){verified=(await account({op:'list'})).profiles.find((x:any)=>x.id===profile.id&&x.state==='connected');if(verified)break;await pause(40);}if(!verified)throw Error('fake account not verified');
 // Make the worktree under legacy ownership first to isolate the reported branch callback.
 await rpc(web,'thread.create',{project:project.id,title:'New thread',provider:'codex',model:'fixture-model',env:'worktree'});const thread=await until(()=>all().find((x:any)=>x.$==='ThreadCreated'&&x.title==='New thread'&&x.provider==='codex'&&x.project===project.id));
 const ready=await until(()=>all().find((x:any)=>x.$==='WorktreeSet'&&x.id===thread.id&&x.worktree));if(!ready)throw Error('legacy worktree control did not complete');
 const initial=git(ready.worktree,'symbolic-ref','--short','HEAD');
 const expected=setting('account.thread.'+thread.id+'.codex');const chosen=await rpc(web,'accounts.select',{thread:thread.id,profile:profile.id,epoch:verified.epoch,expected});if(!chosen.ok)throw Error('account selection refused '+JSON.stringify(chosen));
 const owner=await until(()=>{const x=setting('account.thread.'+thread.id+'.codex');return x&&JSON.parse(x).id===profile.id?x:null;});
 await rpc(web,'turn.start',{thread:thread.id,text:'Name the bounded metadata regression',msg:'metadata-turn',mode:'queue',accountBinding:owner});
 const renamed=await until(()=>{const b=git(ready.worktree,'symbolic-ref','--short','HEAD');return b!==initial?b:null;},18000);check('real Git branch rename completed',renamed,{initial});await pause(300);
 const reported=all().filter((x:any)=>x.$==='WorktreeSet'&&x.id===thread.id).at(-1);
 check('named thread branch callback reconciles canonical metadata with real Git',renamed&&reported?.branch===renamed,{filesystem:renamed,canonical:reported?.branch});
 const before=all().length;await rpc(web,'git.act',{thread:thread.id,action:'fixture-invalid',message:'fixture',accountBinding:owner});const note=await until(()=>all().slice(before).find((x:any)=>x.$==='ActivityLogged'&&x.thread===thread.id&&x.kind==='git'));
 check('ordinary Git job note reaches named thread activity',!!note,note);
 for(const m of ['thread.note','thread.branch','thread.worktree','thread.local','thread.unrooted','thread.merged']){const raw=await rpc(web,m,{thread:thread.id,summary:'forged internal report',branch:'forged-branch',internal:true,jobReceipt:{kind:'branch'}});check('raw lookalike cannot claim internal metadata origin '+m,raw.ok===false,raw);}
 const stale=await rpc(web,'turn.start',{thread:thread.id,text:'stale must not run',msg:'metadata-stale',mode:'queue',accountBinding:'{"id":"other","epoch":2}'});check('provider-sensitive stale request remains refused',stale.ok===false,stale);
 await rpc(web,'accounts.default',{provider:'codex',profile:profile.id,epoch:verified.epoch});
 await rpc(web,'thread.create',{project:project.id,title:'Named default worktree',provider:'codex',model:'fixture-model',env:'worktree'});const named=await until(()=>all().find((x:any)=>x.$==='ThreadCreated'&&x.title==='Named default worktree'));
 const namedReady=await until(()=>all().find((x:any)=>x.$==='WorktreeSet'&&x.id===named.id&&x.worktree));check('named-default worktree completion is dispatched',!!namedReady);
 if(namedReady)check('named-default worktree path is a real Git worktree',git(namedReady.worktree,'rev-parse','--show-toplevel')===namedReady.worktree);
 // The actual Git child pauses at worktree creation; selection can change while
 // IO is pending. Completion must record filesystem truth without releasing a
 // queued provider turn under the new owner. No product test hook is involved.
 writeFileSync(hold,'bounded hold');
 await rpc(web,'thread.create',{project:project.id,title:'Stale readiness',provider:'codex',model:'fixture-model',env:'worktree'});
 const delayed=await until(()=>all().find((x:any)=>x.$==='ThreadCreated'&&x.title==='Stale readiness'));
 if(!await until(()=>existsSync(entered)))throw Error('real worktree child did not reach bounded hold');
 const delayedOwner=await until(()=>setting('account.thread.'+delayed.id+'.codex'));
 const switched=await rpc(web,'accounts.select',{thread:delayed.id,profile:'',epoch:0,expected:delayedOwner});
 check('account may explicitly change while its worktree IO is pending',switched.ok,switched);
 const queued=await rpc(web,'turn.start',{thread:delayed.id,text:'queued under explicit legacy owner',msg:'stale-readiness-queue',mode:'queue'});
 check('new owner queues a turn while old worktree IO is pending',queued.ok,queued);
 const turnCount=()=>existsSync(audit)?readFileSync(audit,'utf8').trim().split('\n').filter(Boolean).map(x=>JSON.parse(x)).filter(x=>x.kind==='turn').length:0;
 const countBefore=turnCount();unlinkSync(hold);
 const reconciled=await until(()=>all().find((x:any)=>x.$==='WorktreeSet'&&x.id===delayed.id));
 check('stale readiness still records the actual completed worktree',reconciled&&git(reconciled.worktree,'rev-parse','--show-toplevel')===reconciled.worktree);
 await pause(400);
 check('stale readiness cannot release queued work under another account',!all().some((x:any)=>x.$==='TurnChanged'&&x.thread===delayed.id&&x.state==='running')&&turnCount()===countBefore);
 // Same-owner completion must continue to release queued work normally.
 unlinkSync(entered);writeFileSync(hold,'bounded hold');
 await rpc(web,'thread.create',{project:project.id,title:'Current readiness',provider:'codex',model:'fixture-model',env:'worktree'});
 const current=await until(()=>all().find((x:any)=>x.$==='ThreadCreated'&&x.title==='Current readiness'));
 if(!await until(()=>existsSync(entered)))throw Error('current worktree child did not reach hold');
 const currentOwner=await until(()=>setting('account.thread.'+current.id+'.codex'));
 await rpc(web,'turn.start',{thread:current.id,text:'current queued turn',msg:'current-readiness-queue',mode:'queue',accountBinding:currentOwner});unlinkSync(hold);
 const currentRun=await until(()=>all().find((x:any)=>x.$==='TurnChanged'&&x.thread===current.id&&x.state==='running'));
 check('matching owner readiness releases its queued provider turn',!!currentRun);
 const folder=join(root,'no-git');mkdirSync(folder);await rpc(web,'project.add',{path:folder});const plain=await until(()=>all().find((x:any)=>x.$==='ProjectCreated'&&x.root===folder));
 await rpc(web,'thread.create',{project:plain.id,title:'Named default fallback',provider:'codex',model:'fixture-model',env:'worktree'});const fallback=await until(()=>all().find((x:any)=>x.$==='ThreadCreated'&&x.title==='Named default fallback'));
 const skipped=await until(()=>all().find((x:any)=>x.$==='WorktreeSkipped'&&x.id===fallback.id));check('named-default thread.local fallback is dispatched',!!skipped);
 check('fallback Git note remains visible',all().some((x:any)=>x.$==='ActivityLogged'&&x.thread===fallback.id&&x.kind==='git'));
 events=all().filter((x:any)=>['WorktreeSet','WorktreeSkipped','ActivityLogged'].includes(x.$));
}catch(e){check('bounded metadata dispatch fixture completed',false,String(e));}
finally{if(existsSync(hold))unlinkSync(hold);writeFileSync(join(root,'RESULT.json'),JSON.stringify({passed:!failed,checks,events},null,2));for(const c of clients)c.ws.close();proc.kill();await proc.exited;console.log('retained fixture '+root);}
process.exit(failed?1:0);
