'use strict';
// Pure child-process model: never launches a backend or Electron.
const{test}=require('node:test'),assert=require('node:assert/strict'),vm=require('node:vm'),fs=require('node:fs'),path=require('node:path'),{EventEmitter}=require('node:events');
function model(residentService){
 let spawnOptions,spawnArgs,killCalls=0;const child=Object.assign(new EventEmitter(),{exitCode:null,unref(){},kill(){killCalls++;}}),module={exports:{}};
 vm.runInNewContext(fs.readFileSync(path.join(__dirname,'../service.cjs'),'utf8'),{module,Buffer,process:{...process,env:{BACKPLANE_NO_UPDATE:'0'}},AbortSignal,fetch,setTimeout,clearTimeout,require(name){
  if(name==='node:child_process')return{execFile(){},spawn(_binary,args,options){spawnArgs=args;spawnOptions=options;return child;}};
  if(name==='node:fs/promises')return{mkdir:async()=>{}};
  if(name==='node:net')return{createServer(){return{once(){},listen(_port,_host,done){done();},address(){return{port:40001};},close(done){done();}};}};
  return require(name);
 }},{filename:'service.cjs'});
 const service=new module.exports.Service({home:'/tmp/synthetic-resident',binary:'/synthetic/serve',web:'/synthetic/web',residentService});let attempts=0;service.discover=async()=>++attempts===1?null:{origin:'http://127.0.0.1:40001',version:'fixture'};
 return{service,options:()=>spawnOptions,args:()=>spawnArgs,killCalls:()=>killCalls};
}
test('frozen resident launch disables service updater and does not stop its native child',async()=>{
 const m=model(true);await m.service.start();assert.equal(m.options().env.BACKPLANE_NO_UPDATE,'1');assert.equal(m.options().env.BACKPLANE_HOME,'/tmp/synthetic-resident');assert.equal(m.options().detached,true);assert.equal(m.killCalls(),0);
});
test('other launch policy does not silently alter updater inputs; malformed policy rejected',async()=>{
 const m=model(false);await m.service.start();assert.equal(m.options().env.BACKPLANE_NO_UPDATE,'0');assert.equal(m.killCalls(),0);assert.throws(()=>model('resident'),/Invalid resident/);
});
test('the hub starts like backplane --headless: loopback listener, tailnet left to the hub',async()=>{
 const m=model(true);await m.service.start();assert.deepEqual([...m.args()],['--home','/tmp/synthetic-resident','--port','40001','--host','127.0.0.1','--web','/synthetic/web','--foreground']);assert(!m.args().includes('--no-tailscale'));
});
