'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict');
const fs=require('node:fs'),vm=require('node:vm');
const {stripTypeScriptTypes}=require('node:module');
const source=fs.readFileSync(require.resolve('../src/main.ts'),'utf8');

// Exercise the actual handlers with a controlled page. No external site,
// helper profile, timers or native browser are needed for lifecycle races.
function fixture({existing=false,goto=async()=>{}}={}){
 const owners=new Map(),homeStarted=new Set(),calls=[];
 const ownerFor=name=>{
  if(!owners.has(name))owners.set(name,{tabs:existing?[{}]:[],opening:null,homeOpening:null,shot:{fast:0,lease:null}});
  return owners.get(name);
 };
 const context={owners,homeStarted,ownerFor,performance:{now:()=>1},switching:null,engine:'chrome',
  CEF_INPUT:new Set(),normalizeUrl:url=>url,timeoutOf:()=>15000,
  ensurePage:async name=>{const p={goto:async url=>{calls.push(url);await goto();}};ownerFor(name).tabs.push(p);return p;},
  tellTabs(){},castCheck:async()=>{},castQuiet(){},shotNow:async()=>{},shotFps:()=>30,
  setTimeout:()=>1,clearTimeout(){}};
 vm.createContext(context);
 const cast=vm.runInContext('({'+source.slice(source.indexOf('  async cast(a, name) {'),source.indexOf('  async status(_a, name) {'))+'}).cast',context);
 const start=source.indexOf('async function dispatch(op:');
 const dispatch=vm.runInContext(stripTypeScriptTypes(source.slice(start,source.indexOf('\nasync function handle(',start)))+';dispatch',context);
 return{owners,calls,ownerFor,cast,dispatch};
}
const args={ms:2500,homeUrl:'https://www.google.com/'};
const tick=()=>new Promise(resolve=>setImmediate(resolve));

test('a fresh watched Browser opens home once and renewals preserve navigation',async()=>{
 const h=fixture();await h.cast(args,'thread');await h.cast(args,'thread');
 assert.deepEqual(h.calls,[args.homeUrl]);
 h.owners.delete('thread');await h.cast(args,'thread');
 assert.deepEqual(h.calls,[args.homeUrl],'closed owner stays closed');
});
test('watching an existing or opening page never navigates it home',async()=>{
 const h=fixture({existing:true});await h.cast(args,'thread');assert.deepEqual(h.calls,[]);
 const opening=fixture();opening.ownerFor('thread').opening=Promise.resolve();
 await opening.cast(args,'thread');assert.deepEqual(opening.calls,[]);
});
test('a stopped lease does not open a page',async()=>{
 const h=fixture();await h.cast({...args,ms:0},'thread');assert.deepEqual(h.calls,[]);
});
test('an explicit navigation arriving during home load wins after it completes',async()=>{
 let finish;const h=fixture({goto:()=>new Promise(resolve=>{finish=resolve;})});
 const home=h.cast(args,'thread');await tick();
 const requested=h.dispatch('navigate',async()=>{h.calls.push('https://example.com/');},{},'thread');
 assert.deepEqual(h.calls,[args.homeUrl]);finish();await home;await requested;
 assert.deepEqual(h.calls,[args.homeUrl,'https://example.com/']);
});
test('a failed home load does not replay or block later explicit navigation',async()=>{
 let fail;const h=fixture({goto:()=>new Promise((_,reject)=>{fail=reject;})});
 const home=assert.rejects(h.cast(args,'thread'),/offline/);await tick();
 const requested=h.dispatch('navigate',async()=>{h.calls.push('https://example.com/');},{},'thread');
 fail(Error('offline'));await home;await requested;await h.cast(args,'thread');
 assert.deepEqual(h.calls,[args.homeUrl,'https://example.com/']);
});
