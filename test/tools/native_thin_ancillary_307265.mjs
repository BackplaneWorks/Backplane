// Sanitized source-adapter regression; no actual pairing values or app runtime.
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
export async function registerAncillaryChecks(check, harness, stage) {
 const empty={t:'info',info:{pair:'','pair.qr':''}},pair={t:'info',info:{pair:'synthetic-private-link','pair.qr':'synthetic-private-qr'}},hb={t:'hb'};
 const plotRevision={t:'plotrevision',key:'2|fixture-root|',revision:'fixture-revision'};
 const stable=v=>JSON.stringify(v,(_k,x)=>x instanceof Map?{map:[...x]}:x instanceof Set?{set:[...x]}:x);
 const snapshot=h=>stable({states:h.states,pending:h.pending,ordinary:h.ordinary,client:h.client,commands:h.commands,sync:h.sync,paired:h.paired,current:h.current});
 for(const [name,value] of [['empty pair info',empty],['nonempty private pair info',pair],['heartbeat',hb],
  ...['0','1','2','3','4'].map(kind=>['plot revision kind '+kind,{...plotRevision,key:kind+'|fixture-root|'}])]) {
  check('inert '+name+' preserves pending and ready snapshots',()=>{
   const x=harness();x.h.hubs(['h']);x.h.connection('h','1','view-v1');const request=x.greeting().cmds[0].request;
   let before=snapshot(x.h);assert.equal(x.h.receive('h',value,'1'),true);assert.equal(snapshot(x.h),before);
   x.answer(request.id);before=snapshot(x.h);assert.equal(x.h.receive('h',value,'1'),true);assert.equal(snapshot(x.h),before);
   assert(x.h.states.get('h').ready);assert(!x.h.savePresentation().includes('synthetic-private'));
  });
 }
 check('sanitized actual07 ordered info/reply/info sequence readies exactly once',()=>{
  const x=harness();x.h.hubs(['h']);x.h.connection('h','1','view-v1');const r=x.greeting().cmds[0].request;
  x.h.receive('h',empty,'1');assert.deepEqual(x.h.drain().sync,[]);
  const reply=x.answer(r.id);assert.equal(reply.sync.filter(s=>s.phase==='presented').length,1);
  x.h.receive('h',empty,'1');assert.deepEqual(x.h.drain().sync,[]);assert(x.h.states.get('h').ready);
  x.h.receive('h',{t:'reply',id:r.id,ok:true,result:{}},'1');assert.deepEqual(x.h.drain().sync,[]);
  assert.equal(x.h.client.calls.size,0);x.h.request('h','earlier','');const page=x.h.drain();
  assert.equal(page.cmds.length,1);assert.equal(page.cmds[0].request.m,'view.page');assert.equal(page.cmds[0].request.p.cursor,'older');
 });
 check('inert frames cannot renew deadline or rescue expired request',()=>{
  const x=harness();x.h.hubs(['h']);x.h.connection('h','1','view-v1');const r=x.greeting().cmds[0].request,deadline=x.h.deadline();
  x.advance(4999);x.h.receive('h',hb,'1');x.h.receive('h',empty,'1');x.h.receive('h',plotRevision,'1');assert.equal(x.h.deadline(),deadline);
  x.advance(2);x.h.expire();const expired=x.h.drain();assert.equal(expired.sync[0].phase,'failed');assert.equal(expired.cmds.length,0);
  x.h.receive('h',{t:'reply',id:r.id,ok:true,result:{}},'1');assert(!x.h.states.get('h').ready);assert.equal(x.h.drain().cmds.length,0);
 });
 check('stale epoch and unpaired ancillary cannot affect current pending',()=>{
  const x=harness();x.open();const before=snapshot(x.h);
  assert.equal(x.h.receive('h',empty,'old'),false);assert.equal(x.h.receive('h',plotRevision,'old'),false);
  assert.equal(x.h.receive('foreign',hb,'1'),false);assert.equal(x.h.receive('foreign',plotRevision,'1'),false);assert.equal(snapshot(x.h),before);
 });
 const malformed=[{t:'hb',id:1},{t:'hb',info:{}},{t:'info'},{t:'info',info:[]},{t:'info',info:null},
  {t:'info',info:{pair:''}},{t:'info',info:{pair:'','pair.qr':'',other:''}},
  {t:'info',info:{pair:0,'pair.qr':''}},{t:'info',info:{pair:'','pair.qr':false}},
  {t:'info',info:{pair:'','pair.qr':''},result:{}},
  {t:'info',info:{pair:'界'.repeat(90000),'pair.qr':''}},
  JSON.parse('{"t":"info","info":{"pair":"","pair.qr":"","__proto__":"x"}}'),
  {t:'info',info:{pair:'','pair.qr':'',constructor:''}}, {t:'log',items:[]},
  {t:'plotrevision',key:'2|fixture-root|'}, {t:'plotrevision',revision:'r'},
  {t:'plotrevision',key:'',revision:'r'}, {t:'plotrevision',key:'0|fixture-root|',revision:''},
  {t:'plotrevision',key:'0|fixture-root|',revision:1}, {t:'plotrevision',key:'0|fixture-root|',revision:'r',extra:true},
  {t:'plotrevision',key:'5|fixture-root|',revision:'r'}, {t:'plotrevision',key:'0||',revision:'r'},
  {t:'plotrevision',key:'0|fixture-root',revision:'r'},
  {t:'plotrevision',key:'0|fixture-root|',revision:'x'.repeat(65536)}];
 malformed.forEach((frame,i)=>check('malformed/unknown ancillary remains refused '+i,()=>{
  const x=harness();x.open();x.h.receive('h',frame,'1');const out=x.h.drain();
  assert.equal(out.sync[0]?.phase,'failed');assert.deepEqual(out.cmds,[]);assert(!x.h.states.get('h').ready);
 }));
 // Exercise actual bridge source + actual CBOR decoder already loaded by the
 // binding controls, with the existing pure Bend substitute only.
 const B=globalThis.Backplane;
 const head=(m,n)=>n<24?Buffer.from([(m<<5)|n]):n<256?Buffer.from([(m<<5)|24,n]):n<65536?Buffer.from([(m<<5)|25,n>>8,n&255]):Buffer.from([(m<<5)|26,n>>>24,(n>>>16)&255,(n>>>8)&255,n&255]);
 const enc=v=>v===null?Buffer.from([246]):typeof v==='boolean'?Buffer.from([v?245:244]):typeof v==='number'?(v>4294967295?Buffer.concat([Buffer.from([199]),enc(String(v))]):head(0,v)):typeof v==='string'?Buffer.concat([head(3,Buffer.byteLength(v)),Buffer.from(v)]):Array.isArray(v)?Buffer.concat([head(4,v.length),...v.map(enc)]):Buffer.concat([head(5,Object.keys(v).length),...Object.entries(v).flatMap(([k,x])=>[enc(k),enc(x)])]);
 const recv=(v,quiet=false,owed=false,epoch='1')=>JSON.parse(B.recv('h',enc(v).toString('base64'),epoch,quiet,owed));
 const greeting={t:'view',version:1,renderer:'mobile',generation:'1',epoch:'origin',limits:{replyBytes:262144,entries:32,recent:24,searchResults:25,searchCandidates:128,chunkBytes:65536}};
 const request=o=>JSON.parse(o.cmds.find(c=>c.type==='send').data);
 function boot(){B.start('fixture','{}');B.hubs(['h']);B.local('mailroom-renderer','ios-v1');B.connection('h','1','view-v1');B.act('select','h|a');return request(recv(greeting));}
 function reply(r,quiet=false){const shown={hub:'h',sel:'h|a',online:true,version:'fixture',error:'',note:'',empty:'',hubs:[],found:[],projects:[],bots:[],rooms:[],thread:{id:'h|a',draft:'',entries:[{id:'e',kind:'assistant',text:'current',blocks:[]}]}};
  return recv({t:'reply',id:r.id,ok:true,result:{version:1,hub:'h',generation:'1',epoch:'origin',requestRevision:r.p.requestRevision,target:r.p.target,viewId:r.p.viewId||'v',viewRevision:'1',renderer:'mobile',remoteReady:true,session:r.p.session,shown,overlay:{key:'h',name:'hub',j:shown},window:{ids:['e'],older:'older',hasMore:true,anchor:{id:'e',offset:0},partial:[]},cmds:[]}},quiet);
 }
 check('actual bridge inert output preserves timer, commands and cached draw boundary',()=>{
  const r=boot(),deadline=JSON.parse(B.pulse()).deadline,compact=B.savePresentation();
  for(const frame of [empty,pair,hb,plotRevision]){const o=recv(frame);assert.deepEqual(o,{screen:null,cmds:[],sync:[],deadline,inert:true});assert.equal(B.savePresentation(),compact);assert(!JSON.stringify(o).includes('synthetic-private'));}
  assert.equal(reply(r).sync[0].phase,'presented');assert.equal(recv(empty).screen,null);
  assert.equal(request(JSON.parse(B.act('earlier',''))).m,'view.page');
 });
 check('actual bridge inert does not set or consume owed screen',()=>{
  let r=boot();reply(r,true);assert.equal(recv(empty,true,true).screen,null);
  assert.equal(JSON.parse(B.flush(false)).screen.thread.entries[0].text,'current');
  r=boot();reply(r);B.flush(false);const noOwe=JSON.parse(B.flush(false));assert.equal(noOwe.screen,null);
  recv(hb,true,true);assert.equal(JSON.parse(B.flush(false)).screen,null);
 });
 check('actual bridge malformed ancillary still fails and cannot emit canonical history',()=>{
  boot();const out=recv({t:'info',info:{other:''}});assert.equal(out.sync[0].phase,'failed');assert.equal(out.cmds.length,0);
 });
 check('native Android calls tagged epoch in the actual bridge argument position',()=>{
  const source=fs.readFileSync(path.join(process.cwd(),'mobile/android/app/src/main/java/dev/backplane/mobile/Engine.kt'),'utf8');
  assert(source.includes('Backplane.recv(${q(key)}, ${q(data)}, ${q(epoch)}, $quiet, $owed)'));
 });
}
