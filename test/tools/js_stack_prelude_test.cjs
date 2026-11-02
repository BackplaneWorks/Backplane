'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict'),vm=require('node:vm'),fs=require('node:fs'),path=require('node:path');
const source=fs.readFileSync(path.join(__dirname,'../../scripts/js-stack-prelude.js'),'utf8');
function boot({marker,soft=8n<<20n,hard=4n<<30n,jsc='4000000000',pid=200,argv='/$bunfs/root/backplane-serve',platform='linux',failRaise=false,spawnResult={status:0},args=['--plot','0','/fixture/board.kicad_pcb']}={}){
 const calls={raise:[],exec:0,spawn:[],exit:[],ffi:0},env={};if(marker!==undefined)env.BACKPLANE_STACK_SET=marker;if(jsc)env.BUN_JSC_maxPerThreadStackUsage=jsc;
 const proc={pid,platform,env,argv:['bun',argv,...args],execPath:'/fixture/backplane-serve',exit:code=>{calls.exit.push(code);throw Error('exit')}};
 const ffi={ptr:x=>x,dlopen:()=>({symbols:{getrlimit:(_r,lim)=>{lim[0]=soft;lim[1]=hard;return 0},setrlimit:(_r,lim)=>{calls.raise.push([...lim]);return failRaise?-1:0},execve:(_file,args,envs)=>{calls.exec++;return -1}}})};
 // Pointers need only distinct numeric addresses for the argv/env vectors.
 let address=1n;ffi.ptr=x=>x instanceof BigUint64Array?x:address++;
 vm.runInNewContext(source,{process:proc,Buffer,BigUint64Array,require:n=>{
  if(n==='node:child_process')return {spawnSync:(file,args,options)=>{calls.spawn.push({file,args:[...args],env:{...options.env},stdio:options.stdio,windowsHide:options.windowsHide});return spawnResult}};
  assert.equal(n,'bun:ffi');calls.ffi++;return ffi;
 }});
 return {calls,env};
}
test('a plot child with its parent marker raises its lowered stack and re-execs',()=>{
 const r=boot({marker:'100'});assert.deepEqual(r.calls.raise,[[4n<<30n,4n<<30n]]);assert.equal(r.calls.exec,1);assert.equal(r.env.BACKPLANE_STACK_SET,'200');
});
test('the legacy inherited marker also cannot suppress child setup',()=>{
 const r=boot({marker:'1'});assert.equal(r.calls.exec,1);assert.equal(r.calls.raise.length,1);
});
test('the same PID after exec skips setup, preventing an exec loop',()=>{
 const r=boot({marker:'200'});assert.equal(r.calls.exec,0);assert.equal(r.calls.raise.length,0);
});
test('an already sufficient inherited stack needs no exec',()=>{
 const r=boot({marker:'100',soft:4n<<30n});assert.equal(r.calls.exec,0);assert.equal(r.calls.raise.length,0);
});
test('a constrained hard stack limit is respected',()=>{
 const r=boot({marker:'100',hard:64n<<20n});assert.deepEqual(r.calls.raise,[[64n<<20n,64n<<20n]]);assert.equal(r.calls.exec,1);
});
test('failed setup continues without an exec loop; uncompiled and other platforms are untouched',()=>{
 assert.equal(boot({marker:'100',failRaise:true}).calls.exec,0);
 assert.equal(boot({argv:'/fixture/program.js'}).calls.exec,0);
 assert.equal(boot({platform:'win32'}).calls.exec,0);
});
test('Windows compiled hubs and plot children relaunch with 16 MB before JSC starts',()=>{
 for(const argv of ['B:/~BUN/root/backplane-serve.exe','B:\\~BUN\\root\\backplane-serve.exe','/$bunfs/root/backplane-serve.exe']){
  for(const args of [[],['--plot','0','C:\\user data 日本\\board.kicad_sch'],['--review','x'],['--version'],['--no-such-flag']]){
   const r=boot({platform:'win32',argv,jsc:'',marker:'100',args});
   assert.equal(r.calls.spawn.length,1);const s=r.calls.spawn[0];
   assert.equal(s.file,'/fixture/backplane-serve');assert.deepEqual(s.args,args);
   assert.equal(s.env.BUN_JSC_maxPerThreadStackUsage,'16000000');
   assert.equal(s.stdio,'inherit');assert.equal(s.windowsHide,true);
   assert.deepEqual(r.calls.exit,[0]);assert.equal(r.calls.ffi,0);
   // The launched process and its own children inherit the setting and stop here.
   assert.equal(boot({platform:'win32',argv,jsc:s.env.BUN_JSC_maxPerThreadStackUsage}).calls.spawn.length,0);
  }
 }
});
test('Windows preserves an explicit JSC bound and ignores inherited PID markers',()=>{
 assert.equal(boot({platform:'win32',argv:'B:/~BUN/root/hub.exe',jsc:'8000000'}).calls.spawn.length,0);
 assert.equal(boot({platform:'win32',argv:'B:/~BUN/root/hub.exe',jsc:'',marker:'200'}).calls.spawn.length,1);
});
test('Windows forwards child exit codes and continues if launching fails',()=>{
 for(const status of [2,17])assert.deepEqual(boot({platform:'win32',jsc:'',spawnResult:{status}}).calls.exit,[status]);
 const r=boot({platform:'win32',jsc:'',spawnResult:{status:null,error:Error('spawn failed')}});
 assert.equal(r.calls.spawn.length,1);assert.deepEqual(r.calls.exit,[]);
});
test('Windows source execution and unsupported platforms never relaunch',()=>{
 for(const [platform,argv] of [['win32','C:\\source\\serve.js'],['win32','/fixture/program.js'],['freebsd','/$bunfs/root/hub']]){
  const r=boot({platform,argv,jsc:''});assert.equal(r.calls.spawn.length,0);assert.equal(r.calls.ffi,0);
 }
});
