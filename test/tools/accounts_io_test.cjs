const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const vm = require('node:vm');

const root = fs.mkdtempSync(path.join(os.tmpdir(), 'bp-accounts-test-'));
const bin = path.join(root, 'bin'); fs.mkdirSync(bin);
const fake = `#!/usr/bin/env bun
const fs=require('node:fs'),path=require('node:path'),readline=require('node:readline');
const provider=path.basename(process.argv[1]), args=process.argv.slice(2);
const home=provider==='claude'?process.env.CLAUDE_CONFIG_DIR:process.env.CODEX_HOME;
if(!home || !home.includes('accounts-v1')) process.exit(93);
const marker=path.join(home,'auth.marker.json');
const config=()=>{try{return JSON.parse(fs.readFileSync(fs.existsSync(path.join(home,'fixture.json'))?path.join(home,'fixture.json'):path.join(path.dirname(home),'fixture.json'),'utf8'));}catch{return {};}};
const identity=()=>{try{return JSON.parse(fs.readFileSync(marker,'utf8'));}catch{return null;}};
const save=()=>fs.writeFileSync(marker,JSON.stringify({email:config().email||'fixture@example.test',workspace:config().workspace||'fixture-workspace'}),{mode:0o600});
fs.appendFileSync(path.join(home,'audit.jsonl'),JSON.stringify({args,home,systemHome:process.env.HOME,hasApiKey:!!process.env.ANTHROPIC_API_KEY,hasOpenAI:!!process.env.OPENAI_API_KEY,hasFederation:!!process.env.OPENAI_FEDERATION_RULE_ID,hasConsoleProfile:!!process.env.ANTHROPIC_PROFILE})+'\\n');
if(provider==='claude'){
 if(args[0]==='auth'&&args[1]==='status'){const i=identity();console.log(JSON.stringify({loggedIn:!!i,authMethod:i?'claude.ai':'none',email:i?.email,orgUuid:i?.workspace,configDirectory:config().hideDirectory?undefined:home}));process.exit(i?0:1);}
 if(args[0]==='auth'&&args[1]==='logout'){try{fs.unlinkSync(marker);}catch{}console.log('{}');process.exit(0);}
 if(args[0]==='auth'&&args[1]==='login'){
  console.log('https://claude.ai/oauth/authorize?state=fake-state');
  const rl=readline.createInterface({input:process.stdin});rl.on('line',line=>{if(line==='fixture-code'){save();process.exit(0);}else process.exit(1);});
 }else if(args.includes('--input-format')){const rl=readline.createInterface({input:process.stdin});rl.on('line',line=>{const q=JSON.parse(line);if(q.type!=='control_request'||q.request?.subtype!=='initialize')process.exit(94);console.log(JSON.stringify({type:'control_response',response:{request_id:q.request_id,response:{models:[{value:'fixture-claude',displayName:'Fixture Claude',supportedEffortLevels:['low']}]}}}));});}else process.exit(91);
}else{
 const rl=readline.createInterface({input:process.stdin});const send=x=>console.log(JSON.stringify(x));
 rl.on('line',line=>{const q=JSON.parse(line);if(!q.id)return;let r={};
 if(q.method==='initialize')r={userAgent:'codex/fixture'};
 else if(q.method==='account/read'){const i=identity();r={account:i?{type:'chatgpt',email:i.email,planType:'fixture'}:null,workspaceRouting:i?{chatgptAccountId:i.workspace}:null};}
 else if(q.method==='account/rateLimits/read')r={accountId:identity()?.workspace,rateLimitsByLimitId:{codex:{limitId:'codex',primary:{usedPercent:23,windowDurationMins:300,resetsAt:Math.floor(Date.now()/1000)+300}}}};
 else if(q.method==='model/list')r={data:[{model:'fixture-model',displayName:'Fixture model',supportedReasoningEfforts:[{reasoningEffort:'low'}]}]};
 else if(q.method==='account/login/start'){
  r={loginId:'fixture-login',verificationUrl:'https://auth.openai.com/codex/device',userCode:'FIXTURE-ONLY'};
  setTimeout(()=>{save();send({method:'account/login/completed',params:{loginId:'fixture-login',success:true}});},config().delay||30);
 }else if(q.method==='account/logout'){try{fs.unlinkSync(marker);}catch{}}
 else if(q.method==='account/login/cancel')setTimeout(()=>process.exit(0),1);
 else {send({id:q.id,error:{code:-32601,message:'Unsupported fixture call'}});return;}
 send({id:q.id,result:r});});
}
`;
for (const name of ['claude','codex']) fs.writeFileSync(path.join(bin,name),fake,{mode:0o700});

const original = {...process.env};
process.env.PATH = bin + path.delimiter + process.env.PATH;
process.env.ANTHROPIC_API_KEY = 'fixture-secret-anthropic';
process.env.OPENAI_API_KEY = 'fixture-secret-openai';
process.env.OPENAI_FEDERATION_RULE_ID = 'fixture-federation';
process.env.ANTHROPIC_PROFILE = 'fixture-console-profile';
process.env.CODEX_HOME = path.join(root,'existing-default');
fs.mkdirSync(process.env.CODEX_HOME);fs.writeFileSync(path.join(process.env.CODEX_HOME,'preserve'),'unchanged');

const context = vm.createContext({require,process,console,Buffer,URL,setTimeout,clearTimeout,
  io_eff(){},BP_ACCOUNT_SPAWN:(cmd,args,cwd,err,env)=>({cmd,args,cwd,err,env})});
let source = fs.readFileSync(path.join(__dirname,'../../src/server/effects/accounts.js'),'utf8').replace(/CID\(([^)]+)\)/g, (_,v)=>JSON.stringify(v));
vm.runInContext(source+'\n globalThis.AccountsTest={BPAccountsIO,BPAccountRpc};',context);
const {BPAccountsIO}=context.AccountsTest;
const delay = ms=>new Promise(resolve=>setTimeout(resolve,ms));
async function terminal(m,id) {
  for(let i=0;i<60;i++){const a=m.attempts.get(id);if(['done','cancelled','expired','failed'].includes(a.state))return a;await delay(20);}
  assert.fail('bounded fixture login did not finish');
}
let passed=0;
function ok(name){passed++;console.log('ok '+name);}
(async()=>{
 const home=path.join(root,'hub');fs.mkdirSync(home);const m=new BPAccountsIO(home);
 for(const [provider,label] of [['codex','Work'],['codex','Personal'],['claude','Claude Work']])await m.request({op:'add',provider,label});
 const [a,b,c]=m.profiles;
 for(const [i,p] of [a,b,c].entries())fs.writeFileSync(path.join(m.directory(p),'fixture.json'),JSON.stringify({email:'fixture-'+i+'@example.test',workspace:'workspace-'+i}),{mode:0o600});
 await Promise.all([m.request({op:'login',profile:a.id}),m.request({op:'login',profile:b.id})]);
 const aa=[...m.attempts.values()].find(x=>x.profile===a.id),bb=[...m.attempts.values()].find(x=>x.profile===b.id);
 assert.equal((await terminal(m,aa.id)).state,'done');assert.equal((await terminal(m,bb.id)).state,'done');
 assert.notEqual(a.identity.email,b.identity.email);assert.notEqual(m.directory(a),m.directory(b));ok('A and B device logins verify independently');
 assert.equal(a.allowance.windows[0].used_percent,23);assert.equal(a.allowance.windows[0].scope,'codex');assert.equal(a.catalog[0].id,'fixture-model');ok('catalog and actual reported quota stay on their account');
 const env=m.environment(a);assert.equal(env.ANTHROPIC_API_KEY,undefined);assert.equal(env.OPENAI_API_KEY,undefined);assert.equal(env.OPENAI_FEDERATION_RULE_ID,undefined);assert.equal(env.ANTHROPIC_PROFILE,undefined);assert.equal(env.HOME,original.HOME);assert.equal(env.CODEX_HOME,m.directory(a));ok('inherited billing/profile selectors scrubbed; real HOME preserved');
 const launched=await m.launch(a.id,String(a.epoch),'codex',['exec','resume','session-a'],root,0);
 assert.equal(launched.env.CODEX_HOME,m.directory(a));assert(launched.args.includes('session-a'));await assert.rejects(()=>m.launch(a.id,'1','codex',[],root,0));ok('child environment and epoch checked at launch');
 await m.request({op:'login',profile:c.id});const ca=[...m.attempts.values()].find(x=>x.profile===c.id);
 for(let i=0;i<30&&ca.state!=='waiting';i++)await delay(10);
 assert.equal(ca.state,'waiting');assert(ca.url.startsWith('https://claude.ai/'));
 await assert.rejects(()=>m.request({op:'respond',profile:c.id,attempt:aa.id,code:'fixture-code'}));
 await m.request({op:'respond',profile:c.id,attempt:ca.id,code:'fixture-code'});assert.equal((await terminal(m,ca.id)).state,'done');ok('Claude browser ceremony takes a response only for its own active attempt');
 await m.request({op:'add',provider:'codex',label:'Cancel'});const cancel=m.profiles.at(-1);
 fs.writeFileSync(path.join(m.directory(cancel),'fixture.json'),JSON.stringify({delay:500}),{mode:0o600});
 await m.request({op:'login',profile:cancel.id});const can=[...m.attempts.values()].find(x=>x.profile===cancel.id);
 await m.request({op:'cancel',profile:cancel.id,attempt:can.id});await delay(50);assert.equal(cancel.state,'disconnected');assert.equal(can.url,'');assert.equal(can.userCode,'');assert.equal(a.state,'connected');ok('cancel closes only its own sign-in and clears ephemeral ceremony');
 await m.request({op:'login',profile:cancel.id});const exp=[...m.attempts.values()].filter(x=>x.profile===cancel.id).at(-1);m.cancel(exp,'expired');assert.equal(exp.state,'expired');await assert.rejects(()=>m.request({op:'respond',profile:cancel.id,attempt:exp.id,code:'fixture-code'}));ok('expired attempts reject completion');
 const oldEpoch=a.epoch;fs.writeFileSync(path.join(m.directory(a),'auth.marker.json'),JSON.stringify({email:'replacement@example.test',workspace:'replacement'}),{mode:0o600});
 await assert.rejects(()=>m.launch(a.id,String(oldEpoch),'codex',[],root,0));assert.equal(a.epoch,oldEpoch+1);assert.equal(b.state,'connected');ok('identity replacement invalidates selected generation before process launch');
 const restarted=new BPAccountsIO(home);const saved=restarted.profile(b.id);assert.equal(saved.state,'unchecked');await restarted.request({op:'status',profile:b.id});assert.equal(saved.identity.email,b.identity.email);assert.equal(saved.epoch,b.epoch);ok('restart requires provider verification and preserves stable identity epoch');
 await m.request({op:'disconnect',profile:b.id});assert.equal(b.state,'disconnected');assert.equal(b.allowance,null);assert.equal(b.catalog.length,0);assert.equal(c.state,'connected');ok('disconnect invalidates only selected profile');
 const registry=fs.readFileSync(m.index,'utf8');for(const secret of ['fixture-secret-anthropic','fixture-secret-openai','fixture-code','FIXTURE-ONLY'])assert(!registry.includes(secret));assert.equal(fs.statSync(m.index).mode&0o777,0o600);assert.equal(fs.statSync(m.root).mode&0o777,0o700);assert.equal(fs.readFileSync(path.join(path.join(root,'existing-default'),'preserve'),'utf8'),'unchanged');ok('private metadata excludes credential/ceremony values and existing default stays intact');
 assert.equal(m.url('https://attacker.example/auth','claude'),'');assert.equal(m.url('https://claude.ai/?access_token=fixture','claude'),'');assert.equal(m.url('http://claude.ai/','claude'),'');ok('provider authorization links use exact HTTPS allowlist');
 assert.equal(c.catalog[0].id,'fixture-claude');ok('Claude catalog is initialized without any user/model prompt');
 const beforeStore=c.store,beforeEpoch=c.epoch,beforeIdentity=c.identity.email;
 await m.request({op:'login',profile:c.id});const retry=[...m.attempts.values()].find(x=>x.profile===c.id);
 for(let i=0;i<50&&retry.state==='starting';i++)await delay(5);assert.equal(retry.state,'waiting');
 await m.request({op:'cancel',profile:c.id,attempt:retry.id});
 assert.equal(c.store,beforeStore);assert.equal(c.epoch,beforeEpoch);assert.equal(c.state,'connected');assert.equal(c.identity.email,beforeIdentity);ok('cancelled reconnect preserves the active verified store, identity and generation');
 for(let i=0;i<10;i++){await m.request({op:'login',profile:cancel.id});const attempt=[...m.attempts.values()].find(x=>x.profile===cancel.id&&x.state==='waiting');await m.request({op:'cancel',profile:cancel.id,attempt:attempt.id});}
 assert(m.attempts.size<=8);ok('finished attempts do not exhaust the bounded login slots');
 const concurrent=await Promise.all([m.launch(c.id,String(c.epoch),'claude',[],root,0),m.launch(c.id,String(c.epoch),'claude',[],root,0)]);
 assert.equal(concurrent.length,2);assert.equal(concurrent[0].env.CLAUDE_CONFIG_DIR,concurrent[1].env.CLAUDE_CONFIG_DIR);ok('simultaneous threads on one account serialize verification and keep the same private store');
 m.observe(c.id,String(c.epoch),JSON.stringify({five_hour:{utilization:0.23,resetsAt:123},seven_day:{utilization:0.7},unreported:{utilization:0}}));
 assert.equal(c.allowance.windows.length,2);assert.equal(c.allowance.windows[0].used_percent,23);assert.equal(c.allowance.windows[1].resets_at,undefined);
 const observed=c.allowance;m.observe(c.id,String(c.epoch-1),JSON.stringify({five_hour:{utilization:1}}));assert.equal(c.allowance,observed);ok('passive Claude pools stay on the current identity generation, missing reset remains unknown');
 const cancelledDir=path.join(m.root,can.profile,can.candidate.store);for(let i=0;i<30&&fs.existsSync(cancelledDir);i++)await delay(10);assert(!fs.existsSync(cancelledDir));ok('cancelled login store is removed after its owned ceremony exits');
 const configPath=path.join(m.root,c.id,'fixture.json'),previousConfig=JSON.parse(fs.readFileSync(configPath,'utf8'));
 fs.writeFileSync(configPath,JSON.stringify({...previousConfig,hideDirectory:true}));await assert.rejects(()=>m.verify(c));fs.writeFileSync(configPath,JSON.stringify(previousConfig));ok('Claude cannot verify an account without reporting its exact isolated directory');
 const {EventEmitter}=require('node:events'),ownedChild=new EventEmitter(),originalSpawn=context.BP_ACCOUNT_SPAWN;
 context.BP_ACCOUNT_SPAWN=(...args)=>{args[6](ownedChild);return {env:args[4]};};
 await m.launch(c.id,String(c.epoch),'claude',[],root,0);assert(m.busy(c.id));
 await assert.rejects(()=>m.request({op:'login',profile:c.id}));await assert.rejects(()=>m.request({op:'disconnect',profile:c.id}));assert.equal(c.state,'connected');
 ownedChild.emit('exit',0);assert(!m.busy(c.id));context.BP_ACCOUNT_SPAWN=originalSpawn;ok('login and disconnect cannot mutate credentials held by an owned provider child');
 const retiredStore=c.store,retiredDir=path.join(m.root,c.id,retiredStore);
 await m.request({op:'login',profile:c.id});const promoted=[...m.attempts.values()].find(x=>x.profile===c.id);for(let i=0;i<40&&promoted.state!=='waiting';i++)await delay(10);
 await m.request({op:'respond',profile:c.id,attempt:promoted.id,code:'fixture-code'});assert.equal((await terminal(m,promoted.id)).state,'done');assert.notEqual(c.store,retiredStore);assert(!fs.existsSync(retiredDir));ok('verified reconnect retires the obsolete private credential store');
 const disconnectedDir=path.join(m.root,c.id,c.store);await m.request({op:'disconnect',profile:c.id});assert(!fs.existsSync(disconnectedDir));assert.equal(c.state,'disconnected');ok('disconnect removes its own provider store after successful provider logout');
 assert.equal(m.environment(a).RUST_LOG,'error');ok('named Codex children suppress inherited verbose authentication diagnostics');
 const aMarker=path.join(m.directory(a),'auth.marker.json'),aIdentity=JSON.parse(fs.readFileSync(aMarker,'utf8'));
 fs.writeFileSync(aMarker,JSON.stringify({email:'',workspace:''}));await assert.rejects(()=>m.verify(a));fs.writeFileSync(aMarker,JSON.stringify(aIdentity));ok('authenticated provider without reported account identity cannot establish a profile');
 const workspaceEpoch=a.epoch;fs.writeFileSync(aMarker,JSON.stringify({...aIdentity,workspace:'another-workspace'}));await assert.rejects(()=>m.launch(a.id,String(workspaceEpoch),'codex',[],root,0));assert.equal(a.epoch,workspaceEpoch+1);ok('same email with a different reported workspace invalidates session ownership');
 console.log('passed '+passed+' account IO cases');
})().finally(()=>{
 for(const m of context.bp_account_managers?.values?.()??[])for(const a of m.attempts.values())m.cancel(a);
 for(const key of Object.keys(process.env))if(!(key in original))delete process.env[key];Object.assign(process.env,original);
 fs.rmSync(root,{recursive:true,force:true});
}).catch(e=>{console.error(e);process.exitCode=1;});
