// Actual isolated hub + provider-owned stand-ins. No network authentication or inference.
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, readdirSync, chmodSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { ThinViews } from '../../src/web/thin.js';

const [binary='build/multi-account/backplane-serve', wire='build/multi-account/wire', web='build/multi-account/web'] = process.argv.slice(2);
for (const f of readdirSync(wire).filter(f=>f.endsWith('.js'))) (0,eval)(readFileSync(join(wire,f),'utf8'));
const W=(globalThis as any).Wire;
const root=mkdtempSync(join(tmpdir(),'bp-accounts-hub-')), home=join(root,'home'), fake=join(root,'bin');
mkdirSync(home);mkdirSync(fake);
const standin=resolve('test/tools/accounts_standin.cjs'), bun=process.execPath;
for(const name of ['claude','codex','grok','muse']) {
  writeFileSync(join(fake,name), ['claude','codex'].includes(name)?`#!/bin/sh\nexec '${bun.replaceAll("'","'\\''")}' '${standin.replaceAll("'","'\\''")}' ${name} "$@"\n`:'#!/bin/sh\nexit 127\n');
  chmodSync(join(fake,name),0o755);
}
const listener=Bun.listen({hostname:'127.0.0.1',port:0,socket:{data(){}}}), port=listener.port;listener.stop(true);
const auditFile=join(root,'audit.jsonl');
writeFileSync(join(home,'events.jsonl'),'');
console.log('fixture root '+root);
writeFileSync('build/multi-account/last-fixture.txt',root+'\n');
let proc:any,failed=false;const clients:any[]=[];
const check=(name:string,ok:any,got?:any)=>{console.log((ok?'ok ':'FAIL ')+name+(ok?'':': '+JSON.stringify(got)?.slice(0,700)));if(!ok)failed=true;};
const pause=(ms:number)=>new Promise(r=>setTimeout(r,ms));
async function until<T>(f:()=>T|false|null|undefined,ms=10000):Promise<T>{const end=Date.now()+ms;do{const v=f();if(v)return v;await pause(40);}while(Date.now()<end);throw Error('bounded condition timed out');}
const audit=()=>{try{return readFileSync(auditFile,'utf8').trim().split('\n').filter(Boolean).map(x=>JSON.parse(x));}catch{return [];}};
async function ready(){for(let i=0;i<150;i++){try{if((await fetch(`http://127.0.0.1:${port}/hello`)).ok)return;}catch{}await pause(50);}throw Error('hub did not start');}
async function connect(mode='legacy'){
 const token=readFileSync(join(home,'token'),'utf8').trim();
 const c:any={frames:[],replies:new Map(),n:0,mode,views:null};
 c.ws=new WebSocket(`ws://127.0.0.1:${port}/ws?token=${token}`+(mode==='legacy'?'':mode==='web'?'&view=1&frontend=1':'&view=1&mobile=1'));
 c.ws.binaryType='arraybuffer';c.ws.onmessage=(e:any)=>{const o=JSON.parse(W.decode(new Uint8Array(e.data)));c.frames.push(o);if(o.t==='reply')c.replies.set(Number(o.id),o);};
 await new Promise((r,j)=>{c.ws.onopen=r;c.ws.onerror=j;});clients.push(c);
 if(mode!=='legacy'){
   const g:any=await until(()=>c.frames.find((f:any)=>f.t==='view'));
   c.views=new ThinViews({now:()=>performance.now(),send:(_h:any,o:any)=>o});
   if(!c.views.connect('fixture',g).connected)throw Error('real thin greeting rejected');
 }
 return c;
}
async function rpc(c:any,m:string,p:any={}){const id=++c.n;c.ws.send(W.encode(JSON.stringify({id,m,p})));return await until(()=>c.replies.get(id),20000);}
async function picture(c:any,thread:string,extra:any={}){
 const session={sel:thread,rid:1,drafts:{},open:{},adding:false,panel:false,info:{}};
 const call=c.views.select('fixture',thread?{kind:'thread',id:thread}:{kind:'recent',id:''},session,'accounts-fixture');
 if(call.failed)throw Error('thin select '+call.failed);
 c.ws.send(W.encode(JSON.stringify(call.cmds[0])));
 const raw:any=await until(()=>c.replies.get(call.id),20000), taken=c.views.reply('fixture',raw);
 check('actual '+c.mode+' receiver accepts current picture',taken?.current,raw);
 return raw;
}
let admin:any,client:any,thread='';
const events=()=>admin.frames.flatMap((f:any)=>f.items||[]);
const setting=(key:string)=>events().filter((e:any)=>e.$==='SettingSet'&&e.key===key).at(-1)?.value;
const binding=()=>setting('account.thread.'+thread+'.codex')||'';
async function account(p:any){const r=await rpc(client,'accounts',p);return {...r,...r.result};}
async function list(){const r=await account({op:'list'});if(!r.ok)throw Error('account list '+JSON.stringify(r));return r;}
async function connected(id:string){for(let i=0;i<100;i++){const r=await list(),p=r.profiles?.find((p:any)=>p.id===id);if(p?.state==='connected'){await until(()=>JSON.parse(setting('accounts.inventory')||'{}').profiles?.some((x:any)=>x.id===id&&x.epoch===p.epoch));return p;}await pause(60);}throw Error('login not verified');}
async function add(label:string,provider='codex',delay=150){const r=await account({op:'add',provider,label}),p=r.profiles?.find((x:any)=>x.label===label);if(!p)throw Error('profile creation failed '+JSON.stringify(r));writeFileSync(join(home,'providers/accounts-v1',p.id,'fixture.json'),JSON.stringify({email:label.toLowerCase()+'@example.test',workspace:'workspace-'+label,delay,used:label==='A'?12:63}));return p;}
async function select(p:any){const prior=binding();const r=await rpc(client,'accounts.select',{thread,profile:p?.id||'',epoch:p?.epoch||0,expected:prior});check('explicit selection '+(p?.label||'legacy'),r.ok,r);await until(()=>binding()===JSON.stringify({id:p?.id||'',epoch:p?.epoch||0}));return binding();}
async function turn(label:string,owner=binding()){
 const n=audit().filter(x=>x.kind==='turn'&&x.provider==='codex').length, eventStart=events().length;
 const r=await rpc(client,'turn.start',{thread,text:'fixture turn '+label,msg:'account-'+label+'-'+Date.now(),mode:'queue',accountBinding:owner});check('turn '+label+' accepted',r.ok,r);
 await until(()=>audit().filter(x=>x.kind==='turn'&&x.provider==='codex').length>n,20000);
 await until(()=>events().slice(eventStart).some((e:any)=>e.$==='TurnChanged'&&e.thread===thread&&['idle','completed','failed'].includes(e.state)),15000).catch(()=>{});
 await pause(300);return audit().filter(x=>x.kind==='turn'&&x.provider==='codex').at(-1);
}
try{
 // No secret material is written to this harness's fixtures.
 proc=Bun.spawn([resolve(binary),'--home',home,'--port',String(port),'--host','127.0.0.1','--no-tailscale','--headless','--foreground','--web',resolve(web)],{env:{...process.env,DISPLAY:'',WAYLAND_DISPLAY:'',BACKPLANE_AGENTS:fake,PATH:fake+':'+process.env.PATH,BACKPLANE_NO_UPDATE:'1',BACKPLANE_PEERS:'',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',BP_ACCOUNTS_AUDIT:auditFile,OPENAI_API_KEY:'fixture-not-a-key',ANTHROPIC_API_KEY:'fixture-not-a-key',ANTHROPIC_PROFILE:'fixture-not-a-profile'},stdout:Bun.file(join(root,'hub.stdout')),stderr:Bun.file(join(root,'hub.stderr'))});
 await ready();admin=await connect();client=await connect('web');await picture(client,'');
 await rpc(admin,'setting.set',{key:'telemetry.enabled',value:'false'});
 const projectRoot=join(root,'project');mkdirSync(projectRoot);await rpc(admin,'project.add',{path:projectRoot});
 const project:any=await until(()=>events().find((e:any)=>e.$==='ProjectCreated'&&e.root===projectRoot));
 await rpc(admin,'thread.create',{project:project.id,title:'Accounts fixture'});
 const created:any=await until(()=>events().find((e:any)=>e.$==='ThreadCreated'&&e.title==='Accounts fixture'));thread=created.id;
 const provider=await rpc(admin,'thread.provider',{thread,provider:'codex',model:'fixture-model',effort:'low'});check('legacy provider switch',provider.ok,provider);
 const legacy=binding();check('existing thread begins with legacy credentials',!legacy||JSON.parse(legacy).id==='');
 const A=await add('A'),B=await add('B');
 check('adding labels alone leaves profiles disconnected',A.state==='disconnected'&&B.state==='disconnected');
 const loginA=await account({op:'login',profile:A.id}),loginB=await account({op:'login',profile:B.id});
 check('two provider-owned device attempts initiated here',loginA.ok&&loginB.ok, [loginA,loginB]);
 const a=await connected(A.id),b=await connected(B.id);
 check('actual provider verification identifies separate accounts',a.identity?.email==='a@example.test'&&b.identity?.email==='b@example.test',[a,b]);
 check('actual catalog and quota belong to each profile',a.catalog?.length&&b.catalog?.length&&a.allowance?.windows?.[0]?.used_percent===12&&b.allowance?.windows?.[0]?.used_percent===63,[a,b]);
 await select(a);const ownerA=binding(),one=await turn('A1');await select(b);const ownerB=binding(),two=await turn('B1');await select(a);const three=await turn('A2');
 check('A to B to A uses separate stores and resumes only A',one.store!==two.store&&three.store===one.store&&three.resumed===one.session&&!two.resumed,[one,two,three]);
 check('child launch scrubs inherited credential selectors and preserves HOME',[one,two,three].every(x=>!x.ambientApi&&!x.ambientProfile&&x.home===process.env.HOME));
 const holding=await rpc(client,'turn.start',{thread,text:'hold-fixture between-turn qualification',msg:'holding-'+Date.now(),mode:'queue',accountBinding:ownerA});
 check('bounded stand-in hold starts',holding.ok,holding);
 const changing=await rpc(client,'accounts.select',{thread,profile:b.id,epoch:b.epoch,expected:ownerA});
 const relogin=await account({op:'login',profile:a.id}),disconnectActive=await account({op:'disconnect',profile:a.id});
 check('active turn refuses account switch',changing.ok===false,changing);
 check('active account refuses login and disconnect',relogin.ok===false&&disconnectActive.ok===false,[relogin,disconnectActive]);
 await until(()=>events().filter((e:any)=>e.$==='TurnChanged'&&e.thread===thread).at(-1)?.state==='completed',12000);
 const reconnected=await connect('web');await picture(reconnected,thread);client.ws.close();client=reconnected;
 const resumed=await turn('A3-after-reconnect');
 check('reconnected client resumes only the selected account',resumed.store===one.store&&resumed.resumed===one.session,resumed);
 const defaultA=await rpc(client,'accounts.default',{provider:'codex',profile:a.id,epoch:a.epoch});
 check('verified account may become new-thread default',defaultA.ok,defaultA);
 await rpc(client,'setting.set',{key:'agent.provider',value:'codex'});
 const createDefault=await rpc(client,'thread.create',{project:project.id,title:'Default account fixture',env:'local'});check('new thread creation with account default succeeds',createDefault.ok,createDefault);
 const newer:any=await until(()=>events().find((e:any)=>e.$==='ThreadCreated'&&e.title==='Default account fixture'));
 await until(()=>setting('account.thread.'+newer.id+'.codex'));
 check('new thread freezes verified default identity generation',setting('account.thread.'+newer.id+'.codex')===ownerA);
 const concurrentStart=events().length,concurrentAudit=audit().length;
 const sameAccount=await Promise.all([
   rpc(client,'turn.start',{thread,text:'hold-fixture concurrency',msg:'concurrency-main',mode:'queue',accountBinding:ownerA}),
   rpc(client,'turn.start',{thread:newer.id,text:'fixture parallel child',msg:'concurrency-other',mode:'queue',accountBinding:ownerA})]);
 check('two threads on one account may start independently',sameAccount.every(x=>x.ok),sameAccount);
 await until(()=>events().slice(concurrentStart).some((e:any)=>e.$==='TurnChanged'&&e.thread===newer.id&&e.state==='completed'),12000);
 check('parallel thread completes while same-account turn is still running',events().filter((e:any)=>e.$==='TurnChanged'&&e.thread===thread).at(-1)?.state==='running');
 const concurrentTurns=audit().slice(concurrentAudit).filter(x=>x.kind==='turn'&&x.provider==='codex');
 check('same-account threads have distinct native sessions in one private store',concurrentTurns.length===2&&concurrentTurns[0].store===concurrentTurns[1].store&&concurrentTurns[0].session!==concurrentTurns[1].session,concurrentTurns);
 await until(()=>events().filter((e:any)=>e.$==='TurnChanged'&&e.thread===thread).at(-1)?.state==='completed',12000);
 await rpc(client,'accounts.default',{provider:'codex',profile:b.id,epoch:b.epoch});
 check('changing default preserves existing thread owners',setting('account.thread.'+newer.id+'.codex')===ownerA&&binding()===ownerA);
 await rpc(client,'accounts.default',{provider:'codex',profile:'',epoch:0});
 const before=audit().filter(x=>x.kind==='turn').length;
 const stale=await rpc(client,'turn.start',{thread,text:'must refuse stale B',msg:'stale-account',mode:'queue',accountBinding:ownerB});
 check('captured stale account binding is refused before enqueue',stale.ok===false,stale);
 const old=await connect('mobile'),read=await picture(old,thread);
 writeFileSync(join(root,'old-native-read.json'),JSON.stringify(read));
 check('old native read has safe visible error and no commands',read.ok&&read.result?.shown?.error&&read.result?.overlay?.j?.error&&read.result.cmds?.length===0,read);
 for(const m of ['turn.start','thread.model','thread.retry','accounts.select']){const r=await rpc(old,m,{thread,text:'old native must refuse',msg:'old-'+m,model:'fixture-model',profile:B.id,epoch:b.epoch,expected:ownerA,accountBinding:ownerA});check('old native refuses '+m,r.ok===false,r);}
 await pause(250);check('refusals spawn no provider child',audit().filter(x=>x.kind==='turn').length===before);
 const C=await add('Cancel','codex',1200);const cstart=await account({op:'login',profile:C.id});const attempt=cstart.attempts?.find((x:any)=>x.profile===C.id);check('cancel ceremony has a bound attempt',attempt?.id,cstart);
 if(attempt){const cancel=await account({op:'cancel',profile:C.id,attempt:attempt.id});check('cancel account ceremony',cancel.ok,cancel);await pause(1400);check('late cancelled completion cannot create a connected account',(await list()).profiles.find((x:any)=>x.id===C.id)?.state!=='connected');}
 const cl=await add('Claude','claude');await account({op:'login',profile:cl.id});
 let ca:any;for(let i=0;i<100;i++){ca=(await list()).attempts?.find((x:any)=>x.profile===cl.id&&x.state==='waiting');if(ca?.url)break;await pause(30);}
 check('Claude login exposes only its provider ceremony',ca?.url?.startsWith('https://claude.ai/'),ca);
 if(ca){const response=await account({op:'respond',profile:cl.id,attempt:ca.id,code:'fixture-code'});check('Claude in-app response accepted for its bound attempt',response.ok,response);const cp=await connected(cl.id);check('Claude authenticated identity and init-only model catalog',cp.identity?.email==='claude@example.test'&&cp.catalog?.length,cp);}
 const logout=await account({op:'disconnect',profile:B.id});check('disconnect B succeeds',logout.ok,logout);const after=await list();check('disconnect B preserves A and legacy thread',after.profiles.find((x:any)=>x.id===A.id)?.state==='connected'&&binding()===ownerA);
 writeFileSync(join(root,'RESULT.json'),JSON.stringify({passed:!failed,scope:'fake CLI actual isolated hub only',home,port,thread,base:'7eccc1e6f29187929d24e0cbb4a8d85127f9d713'},null,2));
}catch(e){check('bounded actual-hub qualification completed',false,String(e));}
finally{for(const c of clients)c.ws.close();proc?.kill();if(proc)await proc.exited;console.log('retained fixture '+root);}
process.exit(failed?1:0);
