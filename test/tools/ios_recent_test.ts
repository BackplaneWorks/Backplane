import assert from 'node:assert/strict';
import { Recent } from '../../src/mobile/recent.js';
let clock=0;
const adopted:any[]=[];
const sessions=new Map<string,any>();
const performed:any[]=[];
const api={
 now:()=>clock,cid:()=>'phone',session:(key:string)=>sessions.get(key) ?? ({sel:'t1',drafts:{t1:''},rid:0}),
 prepare:(s:any,input:any,action:string)=> action==='send' ? {...s,drafts:{...s.drafts,[input.sel]:input.drafts[input.sel]}} : s,
 draft:(s:any,text:string)=>({session:{...s,drafts:{...s.drafts,[s.sel]:text}},cmds:[{type:'keep',text}]}),
 send:(key:string,wire:any)=>({key,wire}),command:(c:any)=>c,adopt:(key:string,s:any)=>{sessions.set(key,s);adopted.push({key,s});},
 perform:(key:string,op:any)=>{
  const input=api.prepare(api.session(key),op.input,op.action);
  performed.push({key,action:op.action,input});
  if(op.action==='send') {
   sessions.set(key,{...input,drafts:{...input.drafts,[input.sel]:''}});
   return [{type:'send',text:'ordinary-turn-rpc'}];
  }
  return [];
 },
};
const reply=(r:Recent,key:string,command:any,session:any,cmds:any[]=[])=>r.reply(key,{id:command.wire.id,ok:true,result:{mobile:1,session,shown:{key,j:{}},cmds}});
const r=new Recent(api);
const first:any=r.open('desk','g1')[0];
assert.equal(first.wire.m,'mobile.screen');
assert.equal(first.wire.p.generation,'g1');
const initial:any=reply(r,'desk',first,first.wire.p.session);
assert.equal(initial.cmds[0].wire.m,'mobile.history');
assert.equal(initial.cmds.filter((c:any)=>c.wire?.m==='mobile.history').length,1);
assert.equal(r.reply('desk',{id:first.wire.id,ok:true,result:{mobile:1}}),null,'duplicate response ignored');
r.draft('desk','A');
const select:any=r.request('desk','pin','')[0];
assert.deepEqual(r.request('desk','send',''),[],'only one session request in flight');
r.draft('desk','B');
const next:any=reply(r,'desk',select,select.wire.p.session);
const send:any=next.cmds.find((c:any)=>c.wire?.m==='mobile.screen');
assert.equal(send.wire.p.session.drafts.t1,'A','send captures text at tap, even when later typing overtakes a response');
const sent:any=r.end('desk');
assert.equal(sent.filter((c:any)=>c.type==='send').length,1,'canonical handoff issues unanswered send exactly once');
assert.equal(r.active('desk'),false,'completed history never waits for an old presentation');
assert.equal(performed.at(-1).input.drafts.t1,'A','handoff sends captured text');
assert.equal(sessions.get('desk').drafts.t1,'B','typing after send survives handoff');
assert.deepEqual(r.end('desk'),[],'duplicate history end cannot repeat a send');
assert.equal(r.reply('desk',{id:send.wire.id,ok:true,result:{mobile:1}}),null,'replayed reply cannot duplicate send');
const old:any=r.open('desk','g2')[0];
const fresh:any=r.open('desk','g3')[0];
assert.equal(reply(r,'desk',old,{sel:'old'}),null,'old socket response ignored');
assert.equal(reply(r,'other',fresh,{}),null,'response cannot cross hubs');
reply(r,'desk',fresh,fresh.wire.p.session);
const pending:any=r.request('desk','select','desk|t2')[0];
clock=5001;
assert.equal(r.expire().length,1,'request deadline clears pending lifecycle');
assert.equal(r.active('desk'),false);
assert.equal(reply(r,'desk',pending,{sel:'late'}),null,'timed-out reply cannot navigate');
const preempt:any=r.open('desk','g4')[0];
const action:any=r.request('desk','thread-permissions','auto-accept-edits')[0];
assert.ok(action,'person action preempts a read-only refresh immediately');
assert.equal(action.wire.p.action,'thread-permissions');
assert.equal(reply(r,'desk',preempt,{sel:'obsolete'}),null,'preempted refresh cannot issue commands or replace selection');
assert.deepEqual(r.end('desk'),[]);
assert.equal(performed.at(-1).action,'thread-permissions','canonical handoff executes pending choice');
assert.equal(reply(r,'desk',action,{sel:'obsolete'},[{type:'send',text:'duplicate'}]),null,'late renderer cannot duplicate the canonical choice');
console.log('PASS recent transport: ordering, exact send capture, one history ack, handoff, generations, hub isolation, expiry');
