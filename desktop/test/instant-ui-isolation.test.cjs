const {test}=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const os=require('node:os');
const path=require('node:path');
const http=require('node:http');
const {spawn}=require('node:child_process');
const runner=path.resolve(__dirname,'../../test/tools/instant_ui_remote.mjs');
const fixture=()=>import('../../test/tools/instant_ui_fixture.mjs');
const cleanEnv=()=>Object.fromEntries(Object.entries(process.env).filter(([k])=>!k.startsWith('INSTANT_')));
async function run(env,input='') {
 const child=spawn(process.execPath,[runner],{env:{...cleanEnv(),PLAYWRIGHT_MODULE:'/does-not-exist',...env},stdio:['pipe','pipe','pipe']});
 let output='';child.stdout.on('data',x=>output+=x);child.stderr.on('data',x=>output+=x);child.stdin.end(input);
 const timer=setTimeout(()=>child.kill('SIGKILL'),5000);
 const code=await new Promise((resolve,reject)=>{child.once('error',reject);child.once('close',resolve)});clearTimeout(timer);
 return {code,output};
}

test('legacy target settings fail before imports, startup, enrollment or requests',async()=>{
 let calls=0;const canary=http.createServer((req,res)=>{calls++;res.end('{}')});
 await new Promise(resolve=>canary.listen(0,'127.0.0.1',resolve));
 try {
  for(const origin of ['http://100.64.84.41:3787','https://desktop.tail9e0cf2.ts.net','http://127.0.0.1:'+canary.address().port,'']) {
   const result=await run({INSTANT_ORIGIN:origin},'{"code":"unused"}');
   assert.notEqual(result.code,0);assert.match(result.output,/INSTANT_ORIGIN is forbidden/);assert.doesNotMatch(result.output,/Cannot find module/);
  }
  assert.equal(calls,0,'even an explicitly supplied loopback server must receive no requests');
 } finally {await new Promise(resolve=>canary.close(resolve))}
});

test('default invocation has no live target',async()=>{
 const result=await run({});assert.notEqual(result.code,0);assert.match(result.output,/INSTANT_HUB and INSTANT_WEB required/);
});

test('caller cannot supply a home, token, invitation or CLI target',async()=>{
 const {configuration}=await fixture();
 for(const key of ['INSTANT_HOME','INSTANT_TOKEN','INSTANT_CODE','INSTANT_INVITE']) assert.throws(()=>configuration({[key]:'supplied'},[]),/is forbidden/);
 assert.throws(()=>configuration({},['--origin','http://127.0.0.1:3787']),/No target/);
});

test('old stdin enrollment pipeline fails before loading browser or starting hub',async()=>{
 const root=fs.mkdtempSync(path.join(os.tmpdir(),'bp-isolation-guard-'));
 try {
  fs.writeFileSync(path.join(root,'index.html'),'fixture');
  const result=await run({INSTANT_HUB:process.execPath,INSTANT_WEB:root},'{"code":"unused"}');
  assert.notEqual(result.code,0);assert.match(result.output,/Pairing input is forbidden/);assert.doesNotMatch(result.output,/Cannot find module/);
 } finally {fs.rmSync(root,{recursive:true,force:true})}
});

test('HTTP and WebSocket URL fence rejects external destinations and protocol tricks',async()=>{
 const {assertOwnedURL}=await fixture(),origin='http://127.0.0.1:45678';
 assert.equal(assertOwnedURL(origin+'/auth/enroll',origin),origin+'/auth/enroll');
 assert.equal(assertOwnedURL('ws://127.0.0.1:45678/ws',origin,true),'ws://127.0.0.1:45678/ws');
 for(const url of ['http://100.64.84.41:3787/auth/enroll','https://desktop.tail9e0cf2.ts.net','http://127.0.0.1:3787','http://localhost:45678','http://127.0.0.1:45678@external.invalid','http://user@127.0.0.1:45678','file:///etc/passwd','ws://127.0.0.1:45678/ws']) assert.throws(()=>assertOwnedURL(url,origin),/Blocked request/);
 assert.throws(()=>assertOwnedURL('wss://127.0.0.1:45678/ws',origin,true),/Blocked request/);
 assert.throws(()=>assertOwnedURL('ws://127.0.0.1:3787/ws',origin,true),/Blocked request/);
});

test('child arguments and environment cannot inherit live-home or provider settings',async()=>{
 const {startFixture}=await fixture(),root=fs.mkdtempSync(path.join(os.tmpdir(),'bp-isolation-child-'));
 const binary=path.join(root,'fake-hub');
 const script=`#!${process.execPath}\nconst fs=require('node:fs'),path=require('node:path');const args=process.argv.slice(2),home=args[args.indexOf('--home')+1];fs.writeFileSync(path.join(home,'captured.json'),JSON.stringify({args,env:Object.fromEntries(['BACKPLANE_HOME','BACKPLANE_PEERS','HTTP_PROXY','ANTHROPIC_API_KEY','BACKPLANE_AGENTS','PATH'].map(k=>[k,process.env[k]??null]))}));`;
 fs.writeFileSync(binary,script,{mode:0o700});
 const inherited={BACKPLANE_HOME:'/must-not-use-live-home',BACKPLANE_PEERS:'must-not-contact-live-peer',HTTP_PROXY:'http://must-not-use-proxy.invalid',ANTHROPIC_API_KEY:'synthetic-test-sentinel'};
 const saved=Object.fromEntries(Object.keys(inherited).map(k=>[k,process.env[k]]));
 Object.assign(process.env,inherited);
 let owned;
 try {
  await assert.rejects(startFixture({binary,web:root}),error=>{
   const match=/inspect (.*)\/stderr$/.exec(error.message);assert(match,'private startup failure preserves its owned directory');owned=match[1];
   const captured=JSON.parse(fs.readFileSync(path.join(owned,'home','captured.json'),'utf8'));
   assert.deepEqual(captured.args.slice(0,2),['--home',path.join(owned,'home')]);
   assert.equal(captured.args[captured.args.indexOf('--host')+1],'127.0.0.1');
   assert(captured.args.includes('--no-tailscale'));
   assert.equal(captured.env.BACKPLANE_HOME,null);assert.equal(captured.env.BACKPLANE_PEERS,'');
   assert.equal(captured.env.HTTP_PROXY,null);assert.equal(captured.env.ANTHROPIC_API_KEY,null);
   assert.equal(captured.env.BACKPLANE_AGENTS,path.join(owned,'agents'));
   assert.equal(captured.env.PATH,path.join(owned,'agents')+':/usr/bin:/bin');
   assert.equal(fs.statSync(owned).mode&0o777,0o700);
   return true;
  });
 } finally {
  for(const [key,value]of Object.entries(saved))if(value===undefined)delete process.env[key];else process.env[key]=value;
  fs.rmSync(root,{recursive:true,force:true});if(owned)fs.rmSync(owned,{recursive:true,force:true});
 }
});


test('resource fence permits only fixture-origin blobs; network fence stays strict',async()=>{
 const {assertOwnedResource,assertOwnedURL}=await fixture(),origin='http://127.0.0.1:45678';
 assert.equal(assertOwnedResource('blob:'+origin+'/worker-id',origin),'blob:'+origin+'/worker-id');
 for(const url of ['blob:http://127.0.0.1:3787/worker-id','blob:https://desktop.tail9e0cf2.ts.net/worker-id','blob:null/worker-id','data:text/html,fixture'])assert.throws(()=>assertOwnedResource(url,origin),/Blocked request/);
 assert.throws(()=>assertOwnedURL('blob:'+origin+'/worker-id',origin,true),/Blocked request/);
});
