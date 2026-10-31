'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),vm=require('node:vm');
const source=fs.readFileSync(path.join(__dirname,'../../src/web/host.js'),'utf8');
test('streamed views do not count as the person using the desk',()=>{
  let checks=0;const ctx={thinSession:{sel:'A'},deskCheck:()=>checks++,later(){}};
  const start=source.indexOf('      sessionChanged: (session, detail) => {'),end=source.indexOf('      ack:',start);
  const fn=source.slice(start,end).trim().replace(/^sessionChanged: /,'').replace(/,$/,'');
  vm.createContext(ctx);vm.runInContext('this.changed = '+fn,ctx);
  for(let i=0;i<100;i++)ctx.changed({sel:'A'},{inputOnly:true});assert.equal(checks,0);
  ctx.changed({sel:'B'});assert.equal(checks,1);
});
test('only becoming ready reports a new connection, not every ready frame',()=>{
  let checks=0;const ctx={remoteReady:false,current:()=>true,deskCheck:()=>checks++};
  const start=source.indexOf('      state: state => {',source.indexOf('async function startThin(')),end=source.indexOf('        if (!remoteReady)',start);
  const prefix=source.slice(start,end).trim().replace(/^state: /,'');
  vm.createContext(ctx);vm.runInContext('this.state = '+prefix+'}',ctx);
  ctx.state({remoteReady:true});for(let i=0;i<100;i++)ctx.state({remoteReady:true});assert.equal(checks,1);
  ctx.state({remoteReady:false});ctx.state({remoteReady:true});assert.equal(checks,2);
});
test('a connected thin view reports real focus through the existing Bend desk policy',()=>{
  const calls=[],sent=[];
  const ctx={canonicalBoot:false,remoteReady:false,socket:{readyState:1},document:{visibilityState:'visible',hasFocus:()=>false},ui:{sel:'selected'},now:()=>123,
    App:{desk:(ui,focus,now)=>{calls.push([ui.sel,focus,now]);return{ui:{...ui,reported:true},cmds:['look']};},presentation:()=>({drafts:{'@desk':'0 selected','@desk.at':'123'}})},is:()=>true,Wire:{plain:x=>x},run:cmds=>sent.push(cmds)};
  vm.createContext(ctx);vm.runInContext(source.slice(source.indexOf('function deskCheck()'),source.indexOf('for (const ev of ["focus", "blur"])')),ctx);
  ctx.deskCheck();assert.equal(calls.length,0);
  ctx.remoteReady=true;ctx.deskCheck();assert.deepEqual(calls,[['selected',false,123]]);assert.deepEqual(sent,[['look']]);
  ctx.document.hasFocus=()=>true;ctx.deskCheck();assert.equal(calls.at(-1)[1],true);
  ctx.document.visibilityState='hidden';ctx.deskCheck();assert.equal(calls.at(-1)[1],false);
  ctx.socket.readyState=3;ctx.deskCheck();assert.equal(calls.length,3);
  const kept=ctx.keepDesk({sel:'other',drafts:{'@desk':'old remote receipt',other:'unsent'}});
  assert.equal(kept.drafts['@desk'],'0 selected');assert.equal(kept.drafts['@desk.at'],'123');assert.equal(kept.drafts.other,'unsent');assert.equal(kept.sel,'other');
});
test('thin socket notification frames request a native notification and its click opens the thread',()=>{
  const created=[],selected=[];let focused=0,closed=0;
  function Notification(title,options){created.push({title,options,notification:this});this.close=()=>closed++;}
  Notification.permission='granted';
  const envelope={t:'notify',thread:'other-thread',title:'Reply ready',body:'Finished',key:'turn-key'};
  const ctx={s:{},window:{Notification,focus:()=>focused++},Notification,dispatch:(a,v)=>selected.push([a,v]),current:()=>true,heard:0n,
    BigInt,Date,Uint8Array,performance:{now:()=>0},Solid:{isPlot:()=>false},diagnostic:{frame(){},note(){assert.fail('notification decoding failed');}},
    Wire:{cbor:()=>envelope,plain:x=>x},KEYS:[],WORDS:[],viewerRpc:{receive(){assert.fail('notification fell through to unrelated RPC');}}};
  vm.createContext(ctx);
  vm.runInContext(source.slice(source.indexOf('function notify(c)'),source.indexOf('// run an action;')),ctx);
  const start=source.indexOf('  s.onmessage = event => {',source.indexOf('async function startThin('));
  vm.runInContext(source.slice(start,source.indexOf('  // Capture the actual CloseEvent',start)),ctx);
  ctx.s.onmessage({data:new ArrayBuffer(1)});assert.equal(created.length,1);
  assert.equal(created[0].title,'Reply ready');assert.equal(created[0].options.body,'Finished');assert.equal(created[0].options.tag,'turn-key');
  created[0].notification.onclick();assert.equal(focused,1);assert.equal(closed,1);assert.deepEqual(selected,[['select','other-thread']]);
  Notification.permission='denied';ctx.s.onmessage({data:new ArrayBuffer(1)});assert.equal(created.length,1);
});
