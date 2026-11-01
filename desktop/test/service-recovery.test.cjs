'use strict';
// Actual service discovery/start logic with deterministic IO and clock.
// No process, socket, profile or Electron is launched.
const {test}=require('node:test'),assert=require('node:assert/strict'),vm=require('node:vm'),fs=require('node:fs'),path=require('node:path');
function fixture(){
 const state={alive:true,failures:0,time:0,spawn:0,access:[],error:null,platform:'linux',auth:0};const module={exports:{}};
 const files={async mkdir(){},async readFile(p){if(p.endsWith('hub.lock'))return '123 40001 flock\n';if(p.endsWith('token'))return 'a'.repeat(32);throw Error('Unexpected file')},async access(p){state.access.push(p);if(state.error)throw state.error;if(!state.alive)throw Object.assign(Error('gone'),{code:'ENOENT'})}};
 const fakeProcess={...process,platform:'linux',env:{}};
 const fetch=async(url,opts)=>{if(url.endsWith('/hello')){if(state.failures-->0)throw Object.assign(Error('refused'),{cause:{code:'ECONNREFUSED'}});const bytes=Buffer.from('a1696261636b706c616e6565312e322e33','hex');return{ok:true,arrayBuffer:async()=>bytes}}state.auth++;assert.equal(opts.headers.Authorization,'Bearer '+'a'.repeat(32));return{ok:true}};
 class Clock extends Date{static now(){return state.time}}
 vm.runInNewContext(fs.readFileSync(path.join(__dirname,'../service.cjs'),'utf8'),{module,Buffer,process:fakeProcess,AbortSignal,fetch,Date:Clock,setTimeout(fn,ms){state.time+=ms;queueMicrotask(fn);},require(n){if(n==='node:fs/promises')return files;if(n==='node:child_process')return{spawn(){state.spawn++;throw Error('Unexpected second backend')},execFile(){throw Error('Unexpected helper')}};return n==='./service-diagnostics.cjs'?require('../service-diagnostics.cjs'):require(n)}},{filename:'service.cjs'});
 const service=new module.exports.Service({home:'/synthetic/recovery',expectedVersion:'1.2.3'});return{state,service,process:fakeProcess};
}
test('recovery: live Linux lock owner before listener is busy, without credentials or spawn',async()=>{const f=fixture();f.state.failures=1;await assert.rejects(()=>f.service.discover(),e=>e.code==='ERR_SERVICE_BUSY');assert.deepEqual(f.state.access,['/proc/123']);assert.equal(f.state.auth,0);assert.equal(f.state.spawn,0)});
test('recovery: a vanished Linux lock owner permits normal launch fallback',async()=>{const f=fixture();f.state.failures=1;f.state.alive=false;assert.equal(await f.service.discover(),null);assert.equal(f.state.spawn,0)});
test('recovery: unverifiable owner fails closed instead of spawning',async()=>{const f=fixture();f.state.failures=1;f.state.error=Object.assign(Error('denied'),{code:'EACCES'});await assert.rejects(()=>f.service.start(),e=>e.code==='EACCES');assert.equal(f.state.spawn,0)});
test('recovery: non-Linux refusal retains platform behavior, never interprets POSIX PID natively',async()=>{const f=fixture();f.process.platform='win32';f.state.failures=1;assert.equal(await f.service.discover(),null);assert.deepEqual(f.state.access,[])});
test('recovery: history recovery beyond old45s deadline attaches once without a second backend',async()=>{const f=fixture();f.state.failures=501;const c=await f.service.start();assert(f.state.time>45000&&f.state.time<300000);assert.equal(c.version,'1.2.3');assert.equal(f.state.auth,1);assert.equal(f.state.spawn,0);assert.equal(f.service.child,null)});
test('recovery: five-minute deadline is finite and never spawns over a live owner',async()=>{const f=fixture();f.state.failures=10000;await assert.rejects(()=>f.service.start(),e=>e.code==='ERR_SERVICE_BUSY');assert.equal(f.state.time,300000);assert.equal(f.state.spawn,0);assert.equal(f.state.auth,0)});
test('recovery: recovered service still passes ordinary version and home-auth discovery',async()=>{const f=fixture();const c=await f.service.start();assert.equal(c.origin,'http://127.0.0.1:40001');assert.equal(c.version,'1.2.3');assert.equal(f.state.spawn,0);assert.equal(f.state.auth,1)});
