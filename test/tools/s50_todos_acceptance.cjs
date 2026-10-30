#!/usr/bin/env node
// S50 only. Real UI, hub.sh homes and deterministic stand-ins; no build or install.
// BACKPLANE_SERVICE=/absolute/backplane-serve BACKPLANE_WEB=/absolute/web \
//   BACKPLANE_DESKTOP=/absolute/desktop/runtime/backplane-desktop \
//   node test/tools/s50_todos_acceptance.cjs
// Optional: S50_PLATFORMS=W,D, S50_OUT, BP_E2E_FIXTURE, S50_CHROME,
// BACKPLANE_PLAYWRIGHT_MODULE. D owns an Xvfb display on Linux; never uses :0.
// Without BACKPLANE_DESKTOP, D is recorded as skipped. A/I belong to release QA.
// Syntax only: node --check test/tools/s50_todos_acceptance.cjs
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const assert = require('node:assert/strict');
const { spawn, execFile } = require('node:child_process');
const { promisify } = require('node:util');
const { pathToFileURL } = require('node:url');
const net = require('node:net');
const run = promisify(execFile);
const repo = path.resolve(__dirname, '../..');
const hubScript = path.join(repo, 'skills/backplane-e2e/hub.sh');
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const quote = s => "'" + s.replaceAll("'", "'\"'\"'") + "'";
const css = s => '"' + s.replace(/["\\\x00-\x1f]/g, c => '\\' + c.codePointAt(0).toString(16) + ' ') + '"';
// The nine short wire keys are shared with skills/backplane-e2e/rpc.mjs.
// Decode passively: a binary WebSocket frame is CBOR, not UTF-8 JSON.
function frameObject(payload) {
  if(typeof payload==='string')return JSON.parse(payload);
  const b=Buffer.from(payload);let i=0;
  const keys=['$','t','id','m','p','ok','error','items','thread'];
  function length(ai) {
    if(ai<24)return ai;if(ai===24)return b[i++];
    const size=ai===25?2:ai===26?4:8,at=i;i+=size;
    return size===8?Number(b.readBigUInt64BE(at)):b.readUIntBE(at,size);
  }
  function value() {
    const tag=b[i++],mt=tag>>5,ai=tag&31;
    if(mt===7){if(ai===20)return false;if(ai===21)return true;if(ai===22)return null;
      if(ai===26){const n=b.readFloatBE(i);i+=4;return n;}
      if(ai===27){const n=b.readDoubleBE(i);i+=8;return n;}throw Error('Unsupported CBOR simple value');}
    const n=length(ai);
    if(mt===0)return n;if(mt===1)return -1-n;
    if(mt===2||mt===3){const s=b.subarray(i,i+n);i+=n;return mt===3?s.toString('utf8'):s;}
    if(mt===4)return Array.from({length:n},value);
    if(mt===5){const o={};for(let j=0;j<n;j++){const k=value();o[typeof k==='number'&&keys[k]?keys[k]:k]=value();}return o;}
    if(mt===6)return value();throw Error('Unsupported CBOR major type');
  }
  return value();
}
const jsonLines = file => {
  if (!fs.existsSync(file)) return [];
  return fs.readFileSync(file, 'utf8').split('\n').filter(Boolean).flatMap(line => {
    try { return [JSON.parse(line)]; } catch { return []; } // an append can be mid-line
  });
};
async function until(test, timeout = 10000) {
  const end = Date.now() + timeout;
  do { const value = await test(); if (value) return value; await sleep(30); } while (Date.now() < end);
  throw Error('Expected condition did not arrive within ' + timeout + 'ms');
}
async function freePort() {
  const server = net.createServer();
  await new Promise((resolve, reject) => { server.once('error', reject); server.listen(0, '127.0.0.1', resolve); });
  const port = server.address().port;
  await new Promise(resolve => server.close(resolve));
  assert(port >= 3799, 'Refuse a reserved/live port');
  return port;
}
async function stopChild(child) {
  if (!child?.pid || child.exitCode !== null || child.signalCode !== null) return;
  const ended = new Promise(resolve => child.once('exit', resolve));
  child.kill('SIGTERM');
  await Promise.race([ended, sleep(3000)]);
  if (child.exitCode === null && child.signalCode === null) { child.kill('SIGKILL'); await ended; }
}

// Written ONLY to a new temporary hub's fake/ directory. Stable TodoWrite IDs
// and texts make a repeated snapshot byte-identical. Gates keep timing under
// the driver's control instead of racing a long stream. Notes are audited at
// stdin receipt without opening a second artificial model turn.
const standin = String.raw`
import { appendFileSync, existsSync } from 'node:fs';
import { createInterface } from 'node:readline';
import { join } from 'node:path';
const gate = process.env.S50_GATES, audit = process.env.S50_AUDIT;
const args = process.argv.slice(2), r = args.indexOf('--resume');
const session = r >= 0 ? args[r + 1] : 's50-' + crypto.randomUUID();
const uuid = () => crypto.randomUUID();
const emit = o => process.stdout.write(JSON.stringify(o) + '\n');
const log = o => appendFileSync(audit, JSON.stringify({pid:process.pid,session,at:Date.now(),...o}) + '\n');
const lc = (id,state) => emit({type:'command_lifecycle',command_uuid:id,state,uuid:uuid(),session_id:session});
const init = () => emit({type:'system',subtype:'init',session_id:session,cwd:process.cwd(),tools:['TodoWrite','Agent'],mcp_servers:[],model:'claude-haiku-4-5-20251001',permissionMode:'bypassPermissions',apiKeySource:'none',claude_code_version:'2.1.286',capabilities:['interrupt_receipt_v1','interrupt_cancel_queued_v1','msg_lifecycle_v1'],uuid:uuid()});
const assistant = content => emit({type:'assistant',message:{id:'msg_'+uuid(),type:'message',role:'assistant',model:'claude-haiku-4-5-20251001',content,usage:{input_tokens:10,output_tokens:5}},parent_tool_use_id:null,session_id:session,uuid:uuid()});
const tool = (name,input) => {const id='toolu_'+uuid();assistant([{type:'tool_use',id,name,input}]);emit({type:'user',message:{role:'user',content:[{type:'tool_result',tool_use_id:id,content:'ok'}]},parent_tool_use_id:null,session_id:session,uuid:uuid()});return id;};
const tasks = (n,d) => Array.from({length:n},(_,k)=>({content:'Step '+(k+1),activeForm:'Doing step '+(k+1),status:k<d?'completed':k===d?'in_progress':'pending'}));
const snapshot = (label,phase,todos) => {tool('TodoWrite',{todos});log({kind:'snapshot',label,phase,todos});};
async function wait(label,phase) {const end=Date.now()+120000;while(!existsSync(join(gate,label+'-'+phase))){if(Date.now()>end)throw Error('gate timeout '+label+'-'+phase);await new Promise(r=>setTimeout(r,25));}}
async function command(o,text,label) {
  const id=o.uuid||uuid();lc(id,'queued');lc(id,'started');init();log({kind:'started',label,id,text});
  let task,toolId;
  if(label==='alpha'){
    snapshot(label,'initial',tasks(7,3));
    task='a'+uuid();toolId='toolu_'+uuid();assistant([{type:'tool_use',id:toolId,name:'Agent',input:{description:'S50 subagent',prompt:'fixture only',subagent_type:'general-purpose',run_in_background:true}}]);
    emit({type:'system',subtype:'task_started',task_id:task,tool_use_id:toolId,description:'S50 subagent',task_type:'local_agent',is_backgrounded:true,session_id:session});
    emit({type:'user',message:{role:'user',content:[{type:'tool_result',tool_use_id:toolId,content:'Async agent launched: '+task}]},parent_tool_use_id:null,session_id:session,uuid:uuid()});
    await wait(label,'repeat');snapshot(label,'repeat',tasks(7,3));
    await wait(label,'next');const next=tasks(8,3);next[4].content='Agent revised step 5';snapshot(label,'next',next);
  }else if(label==='replay'){const next=tasks(8,3);next[4].content='Agent revised step 5';snapshot(label,'initial',next);}
  else if(label==='done')snapshot(label,'initial',tasks(2,0));
  else if(label.startsWith('isolate'))snapshot(label,'initial',tasks(2,0));
  else if(label!=='empty')throw Error('unknown S50 label '+label);
  await wait(label,'end');
  if(task)emit({type:'system',subtype:'task_notification',task_id:task,tool_use_id:toolId,status:'completed',summary:'fixture done',session_id:session});
  assistant([{type:'text',text:'S50 finished '+label}]);
  emit({type:'result',subtype:'success',is_error:false,result:'S50 finished '+label,num_turns:1,duration_ms:10,total_cost_usd:0,usage:{},modelUsage:{'claude-haiku-4-5-20251001':{contextWindow:200000}},session_id:session,uuid:uuid()});
  lc(id,'completed');log({kind:'ended',label,id});
}
let pending=Promise.resolve();
createInterface({input:process.stdin}).on('line',line=>{
  let o;try{o=JSON.parse(line);}catch{return;}
  if(o.type==='control_request'){emit({type:'control_response',response:{subtype:'success',request_id:o.request_id,response:{still_queued:[]}}});return;}
  if(o.type!=='user')return;
  const c=o.message?.content;
  const text=typeof c==='string'?c:(c||[]).map(b=>typeof b==='string'?b:b.text||'').join(' ');
  log({kind:'input',text,uuid:o.uuid});
  if(text.includes('The person updated your to-do list:'))log({kind:'note',text,uuid:o.uuid});
  const label=/<S50 scenario=([a-z0-9-]+)>/.exec(text)?.[1];
  if(!label)return; // edit receipts never start an artificial turn
  pending=pending.then(()=>command(o,text,label)).catch(e=>{log({kind:'error',error:String(e)});process.exit(1);});
}).on('close',()=>process.exit(0));
`;

async function main() {
  const binary = process.env.BACKPLANE_SERVICE, web = process.env.BACKPLANE_WEB;
  assert(binary && path.isAbsolute(binary) && fs.existsSync(binary), 'Set BACKPLANE_SERVICE to an absolute built artifact');
  assert(web && path.isAbsolute(web) && fs.existsSync(web), 'Set BACKPLANE_WEB to its matching built web directory');
  const desktop = process.env.BACKPLANE_DESKTOP || process.argv[2];
  if (desktop) assert(path.isAbsolute(desktop) && fs.existsSync(desktop), 'Desktop must be an absolute packaged executable');
  const modulePath = process.env.BACKPLANE_PLAYWRIGHT_MODULE || [
    path.join(repo,'tools/browser/node_modules/playwright-core'), path.join(repo,'desktop/node_modules/playwright-core')
  ].find(fs.existsSync);
  assert(modulePath, 'Install the existing Playwright dependency before running');
  const { chromium } = require(modulePath);
  const { connect } = await import(pathToFileURL(path.join(repo,'skills/backplane-e2e/rpc.mjs')).href);
  const runName = new Date().toISOString().replace(/[:.]/g,'-') + '-S50';
  const out = path.resolve(process.env.S50_OUT || path.join(repo,'skills/backplane-e2e/results',runName));
  fs.mkdirSync(out,{recursive:true});
  const temp = fs.mkdtempSync(path.join(os.tmpdir(),'bp-S50-'));
  const reports = [];
  const platforms = (process.env.S50_PLATFORMS || 'W,D').split(',');
  assert(platforms.every(p=>['W','D'].includes(p)), 'Only W and D run here; A/I are release-thread checks');
  for (const platform of platforms) {
    const report = {story:'S50',platform,result:'pass',build:'unknown',steps:[],failures:[],limitations:[
      'Android and iOS are not executed by this driver.',
      'Stand-in note receipt proves transport delivery, not real-model interpretation.'
    ]};
    reports.push(report);
    if (platform==='D' && !desktop) {
      report.result='skip';report.limitations.push('BACKPLANE_DESKTOP was not supplied.');
      fs.writeFileSync(path.join(out,'S50-D.json'),JSON.stringify(report,null,2)+'\n');continue;
    }
    const name='S50-'+platform, base=path.join(temp,name), home=path.join(base,'home');
    const gates=path.join(base,'gates'), auditFile=path.join(base,'standin.jsonl');
    fs.mkdirSync(gates,{recursive:true});
    const env={...process.env,BP_E2E_BIN:binary,BP_E2E_WEB:web,BP_E2E_ROOT:temp,
      BACKPLANE_HOME:home,BACKPLANE_KISTACK_SYNC:'0',BACKPLANE_TELEMETRY:'off',
      BACKPLANE_LOCK_DIR:path.join(base,'locks'),S50_GATES:gates,S50_AUDIT:auditFile};
    delete env.BACKPLANE_KISTACK_SOURCE;
    delete env.BACKPLANE_COPY; // fresh hub; bots must create their homes inside it
    delete env.ELECTRON_RUN_AS_NODE;
    let rpc, page, browser, desktopChild, xvfb, launch=0, started=false, seq=0;
    const pageErrors=[], uiRequests=[];
    report.fixture={root:base,home,service:binary,web,desktop:platform==='D'?desktop:undefined};
    const events=()=>jsonLines(path.join(home,'events.jsonl'));
    const audit=()=>jsonLines(auditFile);
    const changed=(thread,who)=>events().filter(e=>e.$==='TodosChanged'&&e.thread===thread&&e.who===who);
    const gate=(label,phase)=>fs.writeFileSync(path.join(gates,label+'-'+phase),'go\n');
    const hub=async command=>(await run('sh',[hubScript,command,name],{cwd:repo,env,timeout:30000,maxBuffer:2**20})).stdout;
    async function shot(label) {
      const file=path.join(out,`S50-${platform}-${String(++seq).padStart(2,'0')}-${label.replace(/[^a-z0-9]+/gi,'-')}.png`);
      if(page)await page.screenshot({path:file,fullPage:true,timeout:5000});
      return file;
    }
    async function step(label,action,expect,kind='press') {
      const [ok,fail]=kind==='nav'?[1000,3000]:kind==='reply'?[3000,8000]:kind==='restart'?[15000,30000]:[300,2000];
      const start=performance.now();let error;
      try {await action();await expect(fail);}catch(e){error=e;}
      const ms=Math.round(performance.now()-start), verdict=error||ms>fail?'fail':ms>ok?'slow':'ok';
      const entry={step:label,ms,verdict,note:error?String(error):''};report.steps.push(entry);
      console.log(platform,verdict,ms+'ms',label);
      const screenshot=await shot(label).catch(e=>{entry.note+=' screenshot: '+e;return '';});
      entry.screenshot=screenshot;
      if(verdict==='fail') {report.result='fail';report.failures.push({step:label,expected:'S50 condition within '+fail+'ms',got:entry.note||'Timing budget exceeded',screenshot});}
      if(error)throw error;
    }
    async function section(label,body) {
      try {await body();}catch(e){report.result='fail';report.failures.push({step:label,expected:'Complete independent S50 scenario',got:String(e),screenshot:await shot('failure-'+label).catch(()=>'' )});}
    }
    const waitPage=(fn,arg,timeout)=>page.waitForFunction(fn,arg,{timeout});
    const rows=()=>page.locator('div.todo');
    const row=(thread,id)=>rows().filter({has:page.locator('input[data-pre='+css(thread+'\x1f'+id+'\x1f')+']')});
    const head=()=>page.locator('.todos-head:visible');
    const snapshot=()=>rows().evaluateAll(es=>es.map(e=>({
      id:e.querySelector('input').getAttribute('data-pre').split('\x1f')[1],
      text:e.querySelector('input').value,status:e.classList.contains('completed')?'completed':e.classList.contains('in_progress')?'in_progress':'pending'
    })));
    async function waitRows(expected,timeout) {
      await waitPage(expected=>JSON.stringify([...document.querySelectorAll('div.todo')].map(e=>({
        id:e.querySelector('input').getAttribute('data-pre').split('\x1f')[1],text:e.querySelector('input').value,
        status:e.classList.contains('completed')?'completed':e.classList.contains('in_progress')?'in_progress':'pending'
      })))===JSON.stringify(expected),expected,timeout);
    }
    async function ready(thread,timeout=10000) {
      await waitPage(thread=>document.querySelector('main')?.dataset.thread===thread&&document.querySelector('main')?.getAttribute('aria-busy')!=='true',thread,timeout);
    }
    async function select(thread) {
      const target=page.locator('[data-on-click="select"][data-value='+css(thread)+']:visible').first();
      // Use exact thread IDs, never a first/last row or an inferred active bot.
      // A fresh persisted selection is the fallback for a folded sidebar only;
      // record it so it cannot be mistaken for a successful navigation gesture.
      if(await target.count())await target.click();
      else {report.limitations.push('Selected hidden target '+thread+' via persisted selection and reload.');await page.evaluate(id=>localStorage.setItem('backplane-selected',id),thread);await page.reload();}
      await ready(thread);
    }
    async function open() {if(await head().getAttribute('aria-expanded')!=='true')await head().click();await rows().first().waitFor();}
    async function send(thread,label) {
      await ready(thread);const composer=page.locator('#composer');
      await composer.fill('<S50 scenario='+label+'>');await composer.press('Enter');
    }
    async function end(label,thread) {
      const before=events().length;gate(label,'end');
      await until(()=>events().slice(before).some(e=>e.$==='TurnChanged'&&e.thread===thread&&e.state!=='running'));
    }
    async function openClient(origin,token,thread) {
      launch++;
      if(platform==='W') {
        browser=await chromium.launch({headless:true,...(process.env.S50_CHROME?{executablePath:process.env.S50_CHROME}:{}),args:['--no-sandbox']});
        const context=await browser.newContext({viewport:{width:1400,height:900}});
        await context.route('**/*',route=>new URL(route.request().url()).origin===origin?route.continue():route.abort());
        page=await context.newPage();
      }else{
        assert(process.platform==='linux','D currently requires Linux and an isolated Xvfb');
        const displayLog=fs.openSync(path.join(base,'xvfb-'+launch+'.log'),'a');
        xvfb=spawn('Xvfb',['-displayfd','1','-screen','0','1400x900x24','-nolisten','tcp'],{stdio:['ignore','pipe',displayLog]});
        fs.closeSync(displayLog);
        let display='',xerror;xvfb.stdout.on('data',b=>display+=b.toString());xvfb.on('error',e=>xerror=e);
        await until(()=>{if(xerror)throw xerror;return /^\d+\s/.test(display);});
        const debugPort=await freePort();
        const appEnv={...env,HOME:path.join(base,'userhome'),XDG_CACHE_HOME:path.join(base,'userhome/.cache'),XDG_CONFIG_HOME:path.join(base,'userhome/.config'),
          XDG_DATA_HOME:path.join(base,'userhome/.local/share'),XDG_STATE_HOME:path.join(base,'userhome/.local/state'),
          BACKPLANE_SERVICE:binary,BACKPLANE_WEB:web,BACKPLANE_SERVICE_VERSION:report.build,
          DISPLAY:':'+display.trim(),WAYLAND_DISPLAY:'',BACKPLANE_AGENTS:path.join(base,'fake'),BACKPLANE_NO_UPDATE:'1',BACKPLANE_PEERS:''};
        const appLog=fs.openSync(path.join(base,'electron-'+launch+'.log'),'a');
        desktopChild=spawn(desktop,['--remote-debugging-address=127.0.0.1','--remote-debugging-port='+debugPort,'--ozone-platform=x11'],{cwd:base,env:appEnv,stdio:['ignore',appLog,appLog]});
        fs.closeSync(appLog);let appError;desktopChild.on('error',e=>appError=e);
        await until(async()=>{if(appError)throw appError;try{return(await fetch('http://127.0.0.1:'+debugPort+'/json/version')).ok;}catch{return false;}},20000);
        browser=await chromium.connectOverCDP('http://127.0.0.1:'+debugPort);
        page=await until(()=>browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().startsWith(origin)),20000);
        await page.setViewportSize({width:1400,height:900});
      }
      page.setDefaultTimeout(2000);
      page.on('pageerror',e=>pageErrors.push(String(e)));
      // Observe the actual web/Electron action envelope without dispatching test
      // actions. RPC audit below proves that the saved target is the explicit ID.
      page.on('websocket',socket=>socket.on('framesent',frame=>{
        try{const o=frameObject(frame.payload);if(o.m)uiRequests.push(o);}catch{}
      }));
      await page.addInitScript(id=>{localStorage.setItem('backplane-selected',id);localStorage.setItem('backplane-lang','en');},thread);
      await page.goto(origin+'/#token='+token);await ready(thread,20000);
      const skip=page.locator('[data-on-click="ob-skip"]:visible');if(await skip.count())await skip.click();
    }
    async function closeClient() {
      await browser?.close().catch(()=>{});browser=null;page=null;
      await stopChild(desktopChild);desktopChild=null;await stopChild(xvfb);xvfb=null;
    }
    try {
      started=true;await hub('start');
      const port=Number(fs.readFileSync(path.join(base,'port'),'utf8'));assert(port>=3799);
      const origin='http://127.0.0.1:'+port,token=fs.readFileSync(path.join(home,'token'),'utf8').trim();
      const hello = require(path.join(repo,'desktop/service.cjs')).hello;
      report.build=hello(Buffer.from(await(await fetch(origin+'/hello')).arrayBuffer())).backplane;
      assert(report.build,'/hello must name its build');
      fs.writeFileSync(path.join(base,'fake/s50-standin.ts'),standin);
      const bun=process.env.S50_BUN||'bun';
      fs.writeFileSync(path.join(base,'fake/claude'),'#!/bin/sh\nexec '+quote(bun)+' '+quote(path.join(base,'fake/s50-standin.ts'))+' "$@"\n',{mode:0o755});
      rpc=await connect(port,token);
      const call=async(method,params)=>Promise.race([rpc.call(method,params),sleep(10000).then(()=>{throw Error(method+' timeout');})]);
      for(const [key,value] of [['onboard.done','1'],['onboard.uber','1'],['telemetry.share','off'],['restart.continue','off'],['settle.done','off']])await call('setting.set',{key,value});
      const a=fs.readFileSync(path.join(base,'thread'),'utf8').trim();
      const project=events().find(e=>e.$==='ThreadCreated'&&e.id===a).project;
      const b=(await call('thread.create',{project,title:'S50 sibling',provider:'claude'})).thread;
      const botIds=[];
      for(const name of ['s50-one','s50-two']) {
        const bot=(await call('bots.create',{name,provider:'claude'})).bot;
        botIds.push(await until(()=>events().find(e=>e.$==='BotSet'&&e.id===bot)?.thread));
      }
      report.targets={a,b,bots:botIds};assert(a&&b&&botIds.every(Boolean));
      await openClient(origin,token,a);
      let persisted,idleAuditFloor;
      await section('edit, merge, notes and persistence',async()=>{
        await step('fresh list starts closed beside Subagents',()=>send(a,'alpha'),async timeout=>{
          await head().waitFor({timeout});assert.match(await head().innerText(),/3\/7/);assert.equal(await head().getAttribute('aria-expanded'),'false');assert.equal(await rows().count(),0);
          await waitPage(()=>{const a=document.querySelector('.subs-head'),b=document.querySelector('.todos-head');return !!a&&a.nextElementSibling===b;},null,timeout);
        },'reply');
        await step('open all seven editable rows',()=>head().click(),async timeout=>{await waitPage(()=>document.querySelectorAll('div.todo').length===7,null,timeout);assert.equal(await page.getByText('Show all 7',{exact:true}).count(),0);});
        async function edit(label,id,selector,expected) {
          const before=changed(a,'person').length;
          const button=row(a,id).locator(selector);
          assert.equal(await button.getAttribute('data-on-click'),'todo-edit');
          const action=JSON.parse(await button.getAttribute('data-value'));assert.equal(action.thread,a);assert.equal(action.id,id);
          await step(label,()=>row(a,id).locator(selector).click(),async timeout=>{
            await until(()=>changed(a,'person').length===before+1,timeout);
            const event=JSON.parse(changed(a,'person').at(-1).data);assert.equal(event.id,id);await expected(timeout);
          });
        }
        await edit('check explicit item 4','4','.todo-check',async timeout=>{await waitPage(()=>document.querySelectorAll('div.todo.completed').length===4,null,timeout);});
        await edit('uncheck explicit item 1','1','.todo-check',async timeout=>{await waitPage(()=>document.querySelectorAll('div.todo.completed').length===3,null,timeout);});
        const rename='Human text 日本語 "quoted" \\ path';
        assert.equal(await row(a,'2').locator('input').getAttribute('data-on-change'),'todo-text');
        assert.equal(await row(a,'2').locator('input').getAttribute('data-pre'),a+'\x1f2\x1f');
        let before=changed(a,'person').length;
        await step('change text saves on blur',async()=>{await row(a,'2').locator('input').fill(rename);await row(a,'2').locator('input').press('Tab');},async timeout=>{
          await until(()=>changed(a,'person').length===before+1,timeout);const e=JSON.parse(changed(a,'person').at(-1).data);assert.equal(e.id,'2');assert.equal(e.text,rename);
        });
        before=changed(a,'person').length;
        assert.equal(await page.locator('.todo-add button').getAttribute('data-on-click'),'todo-add');
        assert.equal(await page.locator('.todo-add button').getAttribute('data-space'),a+'\x1f');
        await step('add a human item',async()=>{await page.locator('.todo-add input').fill('Human extra');await page.locator('.todo-add button').click();},async timeout=>{
          await until(()=>changed(a,'person').length===before+1,timeout);const e=JSON.parse(changed(a,'person').at(-1).data);assert.equal(e.op,'add');assert.match(e.id,/^h:/);await waitPage(()=>document.querySelectorAll('div.todo').length===8,null,timeout);
        });
        await edit('delete explicit item 3','3','.todo-delete',async()=>{assert.equal(await row(a,'3').count(),0);});
        await edit('move item 7 up','7','.todo-up',async()=>{assert.equal((await snapshot())[4].id,'7');});
        await edit('move item 7 down','7','.todo-down',async()=>{assert.equal((await snapshot())[5].id,'7');});
        await edit('leave a human order change','7','.todo-up',async()=>{assert.equal((await snapshot())[4].id,'7');});
        const human=await snapshot();
        await step('running edits deliver notes to the same agent',async()=>{},async timeout=>{
          await until(()=>audit().filter(x=>x.kind==='note'&&x.text.includes('Human text')).length>0,timeout);
          assert(events().some(e=>e.$==='MessagePosted'&&e.thread===a&&String(e.text).startsWith('The person updated your to-do list:')));
        });
        await step('identical agent repeat preserves every human edit',()=>gate('alpha','repeat'),async timeout=>{
          await until(()=>audit().some(x=>x.label==='alpha'&&x.phase==='repeat'),timeout);await until(()=>changed(a,'tools').length>=2,timeout);await waitRows(human,timeout);
          const snaps=audit().filter(x=>x.label==='alpha'&&x.kind==='snapshot');assert.deepEqual(snaps[0].todos,snaps[1].todos);
        });
        const next=human.map(x=>x.id==='5'?{...x,text:'Agent revised step 5'}:x).concat([{id:'8',text:'Step 8',status:'pending'}]);
        await step('next agent update merges intended text and new item',()=>gate('alpha','next'),async timeout=>{await waitRows(next,timeout);});
        await step('open choice stays with its exact thread',()=>select(b),async()=>{assert.equal(await head().count(),0);},'nav');
        await step('return restores the open edited list',()=>select(a),async timeout=>{await waitRows(next,timeout);},'nav');
        await step('shut choice persists across thread switches',async()=>{await head().click();await select(b);await select(a);},async()=>{assert.equal(await head().getAttribute('aria-expanded'),'false');assert.equal(await rows().count(),0);},'nav');
        await open();await step('unfinished list remains after the turn',()=>end('alpha',a),async timeout=>{await waitRows(next,timeout);},'reply');
        before=events().length;idleAuditFloor=audit().length;const starts=audit().filter(x=>x.kind==='started').length;
        await step('idle edit saves and queues a note without starting',async()=>{await row(a,'6').locator('input').fill('Idle persisted text');await row(a,'6').locator('input').press('Tab');},async timeout=>{
          await until(()=>events().slice(before).some(e=>e.$==='TurnQueued'&&e.thread===a&&String(e.text).includes('Idle persisted text')),timeout);
          assert.equal(audit().filter(x=>x.kind==='started').length,starts);
          assert(!audit().slice(idleAuditFloor).some(x=>x.kind==='note'&&x.text.includes('Idle persisted text')),'Idle edit has not been delivered to a running agent');
          assert(!events().slice(before).some(e=>e.$==='TurnChanged'&&e.thread===a&&e.state==='running'),'Idle edit never starts a turn');
        });
        persisted=await snapshot();
        await step('page reload preserves saved edits',()=>page.reload(),async timeout=>{await ready(a,timeout);await open();await waitRows(persisted,timeout);},'nav');
        await step('isolated hub restart replays editable list',async()=>{await closeClient();rpc.close();await hub('restart');rpc=await connect(port,token);await openClient(origin,token,a);},async timeout=>{await open();await waitRows(persisted,timeout);},'restart');
        await step('restart preserves merge baseline and delivers queued note',()=>send(a,'replay'),async timeout=>{
          await until(()=>audit().some(x=>x.label==='replay'&&x.kind==='snapshot'),timeout);
          await until(()=>audit().slice(idleAuditFloor).some(x=>x.kind==='input'&&x.text.includes('Idle persisted text')&&x.text.includes('The person updated your to-do list:')),timeout);
          await open();await waitRows(persisted,timeout);
        },'reply');
        await end('replay',a);
        // The following independent command deliberately publishes no TodoWrite.
        await step('next turn without todos hides old unfinished list',()=>send(a,'empty'),async timeout=>{
          await until(()=>audit().some(x=>x.kind==='started'&&x.label==='empty'),timeout);
          await waitPage(()=>!document.querySelector('.todos-head')&&!document.querySelector('div.todo'),null,timeout);
        },'reply');
        await step('no stale list returns when the empty turn ends',()=>end('empty',a),async()=>{assert.equal(await head().count(),0);},'reply');
      });
      await section('thread and bot isolation',async()=>{
        const fixtures=[{id:b,label:'isolate-thread'},{id:botIds[0],label:'isolate-bot1'},{id:botIds[1],label:'isolate-bot2'}];
        for(const f of fixtures){await select(f.id);await send(f.id,f.label);await until(()=>audit().some(x=>x.kind==='snapshot'&&x.label===f.label));await open();await end(f.label,f.id);}
        const captured=[];
        for(const f of fixtures){await select(f.id);await open();captured.push(await snapshot());}
        for(let i=0;i<fixtures.length;i++){
          const f=fixtures[i];await select(f.id);await open();const before=changed(f.id,'person').length;
          await step('edit only '+f.label+' item 1',async()=>{await row(f.id,'1').locator('input').fill('Private '+f.label);await row(f.id,'1').locator('input').press('Tab');},async timeout=>{
            await until(()=>changed(f.id,'person').length===before+1,timeout);assert.equal(JSON.parse(changed(f.id,'person').at(-1).data).id,'1');
          });captured[i]=(await snapshot());
          for(let j=0;j<fixtures.length;j++){await select(fixtures[j].id);await open();await waitRows(captured[j],3000);}
        }
        await shot('thread-and-two-bots-isolated');
      });
      await section('narrow editing and completion',async()=>{
        const c=(await call('thread.create',{project,title:'S50 done',provider:'claude'})).thread;
        report.targets.done=c;await select(c);await send(c,'done');await until(()=>audit().some(x=>x.label==='done'&&x.kind==='snapshot'));await open();
        await step('420px rows and every edit control fit',()=>page.setViewportSize({width:420,height:900}),async timeout=>{
          await waitPage(()=>innerWidth===420&&document.documentElement.scrollWidth<=420,null,timeout);
          const geometry=await page.evaluate(()=>{const rect=s=>document.querySelector(s)?.getBoundingClientRect().toJSON();return {head:rect('.todos-head'),title:rect('.topbar .crumbs'),controls:[...document.querySelectorAll('div.todo input,div.todo button,.todo-add input,.todo-add button')].map(e=>e.getBoundingClientRect().toJSON())};});
          assert(geometry.head.y>=geometry.title.y+geometry.title.height-1,'To-do is in the second row');
          for(const r of geometry.controls)assert(r.width>0&&r.x>=0&&r.x+r.width<=421,'Editable control fits 420px');report.narrow=geometry;
        });
        let before=changed(c,'person').length;
        await step('narrow text input saves via normal change',async()=>{await row(c,'2').locator('input').fill('Narrow edit');await row(c,'2').locator('input').press('Tab');},async timeout=>{await until(()=>changed(c,'person').length===before+1,timeout);assert.equal(JSON.parse(changed(c,'person').at(-1).data).text,'Narrow edit');});
        await step('narrow Add works',async()=>{await page.locator('.todo-add input').fill('Narrow extra');await page.locator('.todo-add button').click();},async timeout=>{await waitPage(()=>document.querySelectorAll('div.todo').length===3,null,timeout);});
        const added=(await snapshot()).find(t=>t.text==='Narrow extra').id;
        await step('narrow move up works',()=>row(c,added).locator('.todo-up').click(),async timeout=>{await until(async()=>((await snapshot())[1]?.id===added),timeout);});
        await step('narrow move down works',()=>row(c,added).locator('.todo-down').click(),async timeout=>{await until(async()=>((await snapshot())[2]?.id===added),timeout);});
        await step('narrow delete works',()=>row(c,added).locator('.todo-delete').click(),async timeout=>{await waitPage(()=>document.querySelectorAll('div.todo').length===2,null,timeout);});
        await step('narrow checkbox completes first item',()=>row(c,'1').locator('.todo-check').click(),async timeout=>{await waitPage(()=>document.querySelectorAll('div.todo.completed').length===1,null,timeout);});
        await step('narrow checkbox unchecks first item',()=>row(c,'1').locator('.todo-check').click(),async timeout=>{await waitPage(()=>document.querySelectorAll('div.todo.completed').length===0,null,timeout);});
        await step('narrow checkbox checks first item again',()=>row(c,'1').locator('.todo-check').click(),async timeout=>{await waitPage(()=>document.querySelectorAll('div.todo.completed').length===1,null,timeout);});
        await step('completing last item auto-collapses to done',()=>row(c,'2').locator('.todo-check').click(),async timeout=>{
          await waitPage(()=>document.querySelector('.todos-head')?.getAttribute('aria-expanded')==='false'&&document.querySelector('.todos-head')?.textContent.includes('done')&&!document.querySelector('div.todo'),null,timeout);
          assert(!events().some(e=>e.$==='TurnChanged'&&e.thread===c&&e.state!=='running'),'Completion was observed during its turn');
        });
        await step('completed list clears at turn end',()=>end('done',c),async timeout=>{await waitPage(()=>!document.querySelector('.todos-head'),null,timeout);},'reply');
      });
      await section('runtime and target audit',async()=>{
        assert.deepEqual(pageErrors,[]);assert(!audit().some(x=>x.kind==='error'));
        const edits=events().filter(e=>e.$==='TodosChanged'&&e.who==='person');
        assert(edits.length>0);assert(edits.every(e=>Object.values(report.targets).flat().includes(e.thread)));
        assert(edits.some(e=>JSON.parse(e.data).op==='set')&&edits.some(e=>JSON.parse(e.data).op==='add')&&edits.some(e=>JSON.parse(e.data).op==='delete')&&edits.some(e=>JSON.parse(e.data).op==='move'));
        assert(uiRequests.some(x=>x.m==='todos.edit'||(JSON.stringify(x.p||{}).includes('todo-edit'))),'Actual UI wire carries todos.edit or its thin UI action');
        fs.writeFileSync(path.join(out,'S50-'+platform+'-events.json'),JSON.stringify(events(),null,2)+'\n');
        fs.writeFileSync(path.join(out,'S50-'+platform+'-standin.json'),JSON.stringify(audit(),null,2)+'\n');
      });
    }catch(e){report.result='fail';report.failures.push({step:'bootstrap or fatal exception',expected:'Isolated built S50 UI',got:String(e),screenshot:await shot('fatal').catch(()=>'' )});}
    finally {
      rpc?.close();await closeClient();if(started)await hub('stop').catch(e=>{report.result='fail';report.failures.push({step:'cleanup',got:String(e)});});
      report.pageErrors=pageErrors;report.uiRequests=uiRequests;
      report.limitations.push('Temporary homes and logs retained at '+base+' for inspection.');
      fs.writeFileSync(path.join(out,'S50-'+platform+'.json'),JSON.stringify(report,null,2)+'\n');
    }
  }
  const summary=['# S50 acceptance','','| Story | Platform | Result | Build | Slowest step (ms) |','|---|---|---|---|---|',
    ...reports.map(r=>`| S50 | ${r.platform} | ${r.result} | ${r.build} | ${Math.max(0,...r.steps.map(s=>s.ms))} |`),'',
    `${reports.reduce((n,r)=>n+r.failures.length,0)} failures; ${reports.reduce((n,r)=>n+r.steps.filter(s=>s.verdict==='slow').length,0)} slow steps.`,
    'A/I: not run here; release thread owns native app execution.','',
    ...reports.flatMap(r=>r.failures.map(f=>`- ${r.platform} ${f.step}: expected ${f.expected||'clean shutdown'}; got ${f.got}${f.screenshot?' — '+f.screenshot:''}`))];
  fs.writeFileSync(path.join(out,'report.md'),summary.join('\n')+'\n');
  console.log('S50 results:',out,'; isolated homes:',temp);
  process.exitCode=reports.some(r=>r.result==='fail')?1:0;
}
main().catch(e=>{console.error(e.stack||e);process.exitCode=1;});
