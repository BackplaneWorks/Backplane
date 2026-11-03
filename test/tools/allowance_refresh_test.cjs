// Synthetic private-account actor; no provider, credentials or network access.
const {test}=require('node:test'),assert=require('node:assert/strict');
const fs=require('node:fs'),os=require('node:os'),path=require('node:path'),vm=require('node:vm');
function fixture(t){
 let now=1000000;const clock=class extends Date{static now(){return now;}};
 const source=fs.readFileSync(path.join(__dirname,'../../src/server/effects/accounts.js'),'utf8').replace(/CID\(([^)]+)\)/g,(_,s)=>JSON.stringify(s));
 const Actor=vm.runInNewContext(source+'\nBPAccountsIO',{require,process:{...process,env:{...process.env,ANTHROPIC_API_KEY:'fixture-disabled-default'}},Date:clock,Buffer,URL,setTimeout,clearTimeout,io_eff(){}});
 const home=fs.mkdtempSync(path.join(os.tmpdir(),'bp-allowance-refresh-'));t.after(()=>fs.rmSync(home,{recursive:true,force:true}));
 return{actor:new Actor(home),advance:ms=>{now+=ms;}};
}
test('all idle accounts refresh on a five minute attempt budget; failures retain identity and numbers',async t=>{
 const h=fixture(t),a=h.actor,calls=[];
 a.profiles=['claude','codex'].map((provider,i)=>({id:'account-'+i,provider,state:'connected',epoch:2,identity:{email:'fixture'},allowance:{at:600,state:'ok',windows:[{used_percent:30}]}}));
 a.verify=async p=>{calls.push(p.provider);if(p.provider==='claude')throw Error('Transient failure');p.allowance={at:1000,state:'ok',windows:[{used_percent:10}]};};
 await Promise.all([a.refresh(),a.refresh()]);assert.deepEqual(calls,['claude','codex']);
 assert.equal(a.profiles[0].allowance.at,600);assert.equal(a.profiles[0].state,'connected');assert.equal(a.profiles[1].allowance.windows[0].used_percent,10);
 await a.refresh();assert.equal(calls.length,2);h.advance(299000);await a.refresh();assert.equal(calls.length,2);
 h.advance(1000);await a.refresh();assert.equal(calls.length,4);
});
test('login, launch and running children are skipped until free; disconnected accounts stay disconnected',async t=>{
 const h=fixture(t),a=h.actor,calls=[];
 a.profiles=['login','launch','running','disconnected'].map(id=>({id,provider:'claude',state:id==='disconnected'?'disconnected':'connected'}));
 a.jobs.set('login',{});a.launchQueues.set('launch',Promise.resolve());a.children.set('running',new Set([{}]));a.verify=async p=>calls.push(p.id);
 await a.refresh();assert.deepEqual(calls,[]);a.jobs.clear();a.launchQueues.clear();a.children.clear();await a.refresh();assert.deepEqual(calls,['login','launch','running']);
});
test('Claude structured CLI usage accepts only actual bounded percentages and parses reset dates',t=>{
 const a=fixture(t).actor;
 const ws=a.claudeWindows({rate_limits_available:true,rate_limits:{five_hour:{utilization:25,resets_at:'2026-11-02T12:00:00Z'},seven_day:{utilization:'20'},seven_day_opus:{utilization:110},seven_day_sonnet:{utilization:0}}});
 assert.equal(ws.length,2);assert.equal(ws[0].used_percent,25);assert.equal(ws[0].resets_at,1793620800);assert.equal(ws[1].used_percent,0);
 assert.equal(a.claudeWindows({rate_limits_available:false,rate_limits:{five_hour:{utilization:10}}}).length,0);
});
