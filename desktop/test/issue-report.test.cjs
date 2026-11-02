const {test} = require('node:test'), assert = require('node:assert/strict');
const fs = require('node:fs'), os = require('node:os'), path = require('node:path');
const load = name => import('../../src/web/' + name);

test('publication boundary drops arbitrary contents and identifiers even from modified journals', async () => {
 const {diagnosticReport,osFamily,issueLink,ISSUE_URL}=await load('issue-report.js');
 const secret='PRIVATE prompt token bearer /home/private user@example.com 100.64.0.1';
 const out=diagnosticReport({connections:[{at:Date.now(),event:'closed',closeCode:1006,cause:secret,page:secret,prompt:secret,account:secret,closeReason:secret,frameClass:secret}],
  hub:{events:[{event:'latency',name:'hub.rpc',at:42,count:1,us:3000,message:secret,path:secret},{event:'error',code:secret},{event:secret,name:secret}]}});
 assert(!out.includes('PRIVATE'));assert(!out.includes('100.64'));assert(!out.includes('/home'));assert.match(out,/hub.rpc/);assert.match(out,/1006/);
 assert.equal(osFamily({platform:'Linux',userAgent:'Android'}),'Android');assert.equal(osFamily({platform:'MacIntel',maxTouchPoints:5}),'iOS');
 const text='A !\'()* 日本語\r\n 🐛';assert.equal(new URL(issueLink(text)).searchParams.get('body'),text);
 assert(issueLink(text).startsWith(ISSUE_URL+'?'));assert.equal(issueLink('x'.repeat(9000)), '');
});
test('GitHub handoff persists consent before open, opens once, and fails closed on full disk', async()=>{
 const {IssueHandoff}=await load('issue-report.js');let calls=0,saves=0;const draft={issueBody:'exact body'}, preview={body:'exact body',url:'https://github.com/BackplaneWorks/Backplane/issues/new?body=exact%20body'};
 const handoff=new IssueHandoff({save:async d=>{saves++;assert(d.issueOpened);await Promise.resolve();},open:async url=>{assert.equal(url,preview.url);calls++;}});
 await Promise.all([handoff.send(draft,preview),handoff.send(draft,preview)]);assert.equal(calls,1);assert.equal(saves,1);assert.equal(await handoff.send(draft,preview),false);
 const full=new IssueHandoff({save:async()=>{throw Error('full disk');},open:()=>assert.fail('must stay local')});const d={issueBody:'exact body'};
 await assert.rejects(()=>full.send(d,preview),/full disk/);assert.equal(d.issueOpened,false);
});
test('report writer uses selected project, normal account binding and retries stable identity, upload and start',async()=>{
 const {ReportWriter}=await load('report-writer.js');const calls=[],saves=[];let fail=true,uploads=0;
 const rpc={request:async(m,p)=>{calls.push({m,p});if(m==='thread.create')return{thread:'improvement-'+p.creationKey};if(m==='improvements.context')return{thread:p.sourceThread,accountBinding:'fixture-generation'};if(m==='improvements.report'){assert.equal(p.accountBinding,'fixture-generation');return{thread:p.thread,done:true,body:'  Exact filled template\n日本語  '};}if(fail){fail=false;throw Error('lost reply');}return{ok:true};}};
 const writer=new ReportWriter({rpc,save:async d=>saves.push(structuredClone(d)),upload:async()=>{uploads++;return'/fixture/evidence.png';}}), draft={text:'description',writerProject:'reports'};
 await assert.rejects(()=>writer.start(draft,'Actual report prompt',{image:new Blob(['image'])}),/lost reply/);
 const pending=writer.start(draft,'Changed value is not replayed');assert.equal(await writer.start(draft,'Duplicate'),false);await pending;
 const creates=calls.filter(c=>c.m==='thread.create'), starts=calls.filter(c=>c.m==='turn.start');assert.equal(uploads,1);assert.deepEqual(creates[0].p,creates[1].p);assert.deepEqual(starts[0].p,starts[1].p);
 assert.equal(creates[0].p.project,'reports');assert.equal(starts[0].p.accountBinding,'fixture-generation');assert.match(starts[0].p.text,/Actual report prompt/);assert.match(starts[0].p.text,/\[attachment: \/fixture\/evidence.png\]/);
 assert.equal(await writer.load(draft),'  Exact filled template\n日本語  ');assert(!calls.some(c=>/github/.test(c.m)));
});
test('desktop collector includes temp/control in measured cap, cleans torn write and retains off across restart',()=>{
 const {createConnectionLog,DISK_CAP_BYTES}=require('../connection-log.cjs');const home=fs.mkdtempSync(path.join(os.tmpdir(),'bp-diagnostic-control-'));let now=100000;
 try {let log=createConnectionLog(home,{now:()=>now});for(let i=0;i<500;i++){now+=1000;log.record({event:'closed',closeCode:1006});}
 assert(log.snapshot().bytes<=DISK_CAP_BYTES);log.command('off');log=createConnectionLog(home,{now:()=>now});assert.equal(log.snapshot().enabled,false);assert.equal(log.record({event:'open'}),false);log.command('on');log.command('clear');assert.equal(log.snapshot().records.length,0);
 const fault=new Proxy(fs,{get(target,key){if(key==='writeFileSync')return()=>{const e=Error();e.code='ENOSPC';throw e;};return target[key];}});const full=createConnectionLog(home,{fs:fault,now:()=>now});assert.equal(full.record({event:'error'}),false);assert(!fs.existsSync(full.file+'.tmp'));
 now+=2*86400000;assert.equal(createConnectionLog(home,{now:()=>now}).snapshot().records.length,0);
 }finally{fs.rmSync(home,{recursive:true,force:true});}
});
test('browser collector handles quota, retention and explicit off/clear without copying text',async()=>{
 const {ConnectionLog}=await load('connection-log.js');let now=100000;const data=new Map(),storage={getItem:k=>data.get(k),setItem:(k,v)=>data.set(k,v),removeItem:k=>data.delete(k)};
 const j=new ConnectionLog({storage,now:()=>now,emit:()=>{}});j.record('open');j.setEnabled(false);assert.equal(new ConnectionLog({storage}).enabled(),false);j.record('closed');assert.equal(j.rows.length,1);j.clear();assert.equal(j.rows.length,0);
 j.setEnabled(true);storage.setItem=()=>{throw Error('quota PRIVATE');};assert.doesNotThrow(()=>j.record('error',{prompt:'PRIVATE'}));assert(!JSON.stringify(j.rows).includes('PRIVATE'));now+=86400001;assert.equal(j.snapshot().length,0);
});

test('report operations fence identity after every persistence and reply boundary',async()=>{
 const {ReportWriter}=await load('report-writer.js');
 for(const changeAt of [1,2,3,4]){
  let current='A',saves=0;const calls=[];
  const writer=new ReportWriter({identity:()=>current,expectedScope:()=> 'A',save:async()=>{if(++saves===changeAt)current='B';},rpc:{request:async(m,p)=>{calls.push(m);if(m==='thread.create')return{thread:'improvement-'+p.creationKey};if(m==='improvements.context')return{thread:p.sourceThread,accountBinding:'named-generation'};assert.fail('must fence before start');}}});
  await assert.rejects(()=>writer.start({text:'Private A description',writerProject:'reports'},'Private A description'),/identity changed/);
  assert(!calls.includes('turn.start'));
 }
 let current='A';const writer=new ReportWriter({identity:()=>current,expectedScope:()=> 'A',rpc:{request:async()=>{current='B';return{thread:'r',done:true,body:'Private A result'};}}});
 await assert.rejects(()=>writer.load({writer:{thread:'r',scope:'A',binding:'named'}}),/identity changed/);
});
test('failed disable stops collection immediately and does not emit another record',async()=>{
 const {ConnectionLog}=await load('connection-log.js');let emitted=0;
 const journal=new ConnectionLog({storage:{getItem:()=>null,setItem:()=>{throw Error('quota');}},emit:()=>emitted++});
 assert.throws(()=>journal.setEnabled(false),/quota/);assert.equal(journal.enabled(),false);journal.record('open');assert.equal(emitted,0);
});

test('report agent requires a current project and does not recreate retired Scratch',async()=>{
 const {ReportWriter}=await load('report-writer.js');let calls=0;const writer=new ReportWriter({rpc:{request:async()=>{calls++;}},save:async()=>{}});
 await assert.rejects(()=>writer.start({text:'description'},'prompt'),/Choose a project/);
 await assert.rejects(()=>writer.start({writerProject:'scratch'},'prompt'),/Choose a project/);
 assert.equal(calls,0);
});

test('legacy writer recovery never dispatches an undefined or retired project',async()=>{
 const {ReportWriter}=await load('report-writer.js');let calls=0;const writer=new ReportWriter({rpc:{request:async()=>{calls++;}},save:async()=>{}});
 for(const old of [{key:'a'.repeat(32),thread:'original'}, {key:'a'.repeat(32),thread:'original',project:'scratch'}]) {
  const draft={writer:old,writerProject:'reports'};
  await assert.rejects(()=>writer.start(draft,'prompt'),/original report-writing thread/);assert.equal(draft.writer,old);
 }
 assert.equal(calls,0);
});
