'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),vm=require('node:vm');
const source=fs.readFileSync(path.resolve(__dirname,'../../src/web/host.js'),'utf8');
function fixture(connected=true){
 const calls=[],status=[],ctx={viewMode:'view-v1',remoteReady:false,thin:{ready:false,opening:false,greeted:true,request:(action,value)=>{calls.push({action,value});return true;}},socket:{readyState:connected?1:3},
  document:{getElementById:()=>null,querySelector:()=>null},currentSession:()=>({sel:'STM32'}),checkpointFocused(){},terminalOutput:{clear(){}},composerKeeper:{selecting(){}},drawerShut(){},showViewStatus:m=>status.push(m)};
 const a=source.indexOf('function dispatch('),b=source.indexOf('// a right click on the plot',a);vm.createContext(ctx);vm.runInContext(source.slice(a,b),ctx);return{ctx,calls,status};
}
test('production dispatcher forwards explicit bot navigation on a live transport after picture failure',()=>{
 const f=fixture();assert.equal(f.ctx.dispatch('bot','bub'),true);assert.deepEqual(f.calls,[{action:'bot',value:'bub'}]);assert.equal(f.status.length,0);
});
test('failed picture does not admit ordinary mutations or bot navigation on a disconnected transport',()=>{
 for(const action of ['bot','flag']){const f=fixture(false);f.ctx.dispatch(action,'bub');assert.equal(f.calls.length,0);assert.equal(f.status.length,1);}
 const f=fixture();f.ctx.dispatch('flag','settings');assert.equal(f.calls.length,0);assert.equal(f.status.length,1);
});
