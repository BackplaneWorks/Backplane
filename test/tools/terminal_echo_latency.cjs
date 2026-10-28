// Actual packaged terminal echo measurement, isolated home and browser only.
// Run: node test/tools/terminal_echo_latency.cjs /ABS/STAGE /ABS/NEW/OUT
// Add --accept for p95 <30ms + ordered-input/session-retention acceptance.
// This is run by the sole integrator against its one qualified build.
const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),assert=require('node:assert/strict');
const {chromium}=require('/home/h/.bun/install/cache/playwright-core@1.64.1@@@1');
const {Service}=require(path.join(process.cwd(),'desktop/service.cjs'));
const stage=process.argv[2],out=process.argv[3];if(!stage||!out)throw Error('stage output');fs.mkdirSync(out,{recursive:true});
const home=fs.mkdtempSync(path.join(os.tmpdir(),'bp-terminal-echo-')),project=path.join(home,'project'),agents=path.join(home,'agents');fs.mkdirSync(project);fs.mkdirSync(agents);
for(const n of ['claude','codex','grok','muse','agy','opencode','pi','omp'])fs.writeFileSync(path.join(agents,n),'#!/bin/sh\nexit 127\n',{mode:0o755});
const ptyMeta=path.join(home,'pty.json');
const ptyLog=path.join(home,'echo.jsonl'),shell=path.join(home,'echo-shell');
fs.writeFileSync(shell,`#!/usr/bin/python3\nimport os,tty,time,json\ntty.setraw(0)\nwith open(${JSON.stringify(ptyMeta)},'w') as m: json.dump({'pid':os.getpid()},m)\nf=os.open(${JSON.stringify(ptyLog)},os.O_WRONLY|os.O_CREAT|os.O_APPEND,0o600)\nos.write(1,b'PTY_ECHO_READY\\r\\n')\nn=0\nwhile True:\n b=os.read(0,4096)\n if not b: break\n for c in b:\n  n+=1\n  before=time.time_ns()/1000000\n  os.write(1,('E%04d:%s\\r\\n'%(n,chr(c) if 32<=c<127 else '0x%02x'%c)).encode())\n  after=time.time_ns()/1000000\n  os.write(f,(json.dumps({'n':n,'byte':c,'readMs':before,'writeMs':after})+'\\n').encode())\n`,{mode:0o755});
let t=Math.floor(Date.now()/1000),events=[];const ev=($,x)=>events.push({$,...x,at:t++});
ev('ProjectCreated',{id:'fixture',title:'Terminal echo measurement',root:project});ev('ThreadCreated',{id:'thread',project:'fixture',title:'Disposable popup terminal trace',env:'local',branch:'',worktree:project,provider:'codex',model:''});
for(let i=0;i<80;i++)ev('MessagePosted',{thread:'thread',msg:'m'+i,role:i%2?'user':'assistant',text:('Generated measurement history '+i+'. ').repeat(12)});
for(let i=0;i<16;i++)ev('ThreadCreated',{id:'side'+i,project:'fixture',title:'Generated sidebar agent '+i,env:'local',branch:'',worktree:project,provider:'codex',model:''});
for(const key of ['restart.continue','mail.on','telemetry.share'])ev('SettingSet',{key,value:'off'});ev('SettingSet',{key:'onboard.done',value:'1'});fs.writeFileSync(path.join(home,'events.jsonl'),events.map(JSON.stringify).join('\n')+'\n');
Object.assign(process.env,{BACKPLANE_HOME:home,BACKPLANE_AGENTS:agents,BACKPLANE_COPY:'1',BACKPLANE_NO_UPDATE:'1',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',BACKPLANE_TELEMETRY:'off',BACKPLANE_PEERS:'',BACKPLANE_LOCK_DIR:home+'/locks',SHELL:shell});
let service,browser;const result={stage,home,ptyLog,state:'started',limits:['Generated conversation/sidebar, disposable real PTY with raw byte echo process; no live user terminal/window/profile.','DOM echo is measured; second requestAnimationFrame is a next-frame presentation opportunity, not display scanout.'],frames:[]};
(async()=>{try{
 const version=JSON.parse(fs.readFileSync(path.join(stage,'LOCAL.json'))).version;result.version=version;
 service=new Service({home,binary:path.join(stage,'payload/backplane-serve'),web:path.join(stage,'payload/web'),expectedVersion:version});const conn=await service.start();result.origin=conn.origin;result.pid=service.child?.pid;
 browser=await chromium.launch({headless:true,executablePath:'/home/h/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome',args:['--no-sandbox','--disable-dev-shm-usage']});const ctx=await browser.newContext({viewport:{width:1440,height:900}});
 await ctx.request.post(conn.origin+'/auth/pair',{headers:{Authorization:'Bearer '+await service.token()}});const page=await ctx.newPage();page.setDefaultTimeout(12000);
 const errors=[];page.on('pageerror',e=>errors.push(String(e)));const cdp=await ctx.newCDPSession(page);await cdp.send('Network.enable');
 for(const [name,dir] of [['Network.webSocketFrameSent','send'],['Network.webSocketFrameReceived','recv']])cdp.on(name,e=>result.frames.push({dir,t:e.timestamp,opcode:e.response.opcode,payload:e.response.payloadData}));
 await page.addInitScript(()=>{
  localStorage.setItem('backplane-selected','thread');window.backplaneDesktop={terminalFocus(){}};
  window.echoTrace=[];window.echoes={};window.keyCount=0;const now=()=>performance.timeOrigin+performance.now();
  const NativeWS=window.WebSocket;
  window.WebSocket=new Proxy(NativeWS,{construct(T,args){const ws=new T(...args);ws.addEventListener('message',e=>window.echoTrace.push({kind:'ws-recv',ms:now(),bytes:e.data?.byteLength||0}));const send=ws.send;ws.send=function(data){window.echoTrace.push({kind:'ws-send',ms:now(),bytes:data?.byteLength||data?.length||0});return send.call(this,data)};return ws}});
  document.addEventListener('keydown',e=>{if(e.target.id==='term')window.echoTrace.push({kind:'key',ms:now(),key:e.key,n:++window.keyCount})},true);
  const start=()=>new MutationObserver(()=>{const text=document.querySelector('.term-pane .screen')?.textContent||'';const matches=[...text.matchAll(/E(\d{4}):([^\r\n])/g)];for(const m of matches){const n=+m[1];if(window.echoes[n])continue;const item={kind:'dom-echo',n,byte:m[2],ms:now()};window.echoes[n]=item;window.echoTrace.push(item);requestAnimationFrame(()=>requestAnimationFrame(()=>window.echoTrace.push({kind:'frame-opportunity',n,ms:now()})));}}).observe(document.body,{subtree:true,childList:true,characterData:true});
  document.addEventListener('DOMContentLoaded',start,{once:true});
 });
 await page.goto(conn.origin);await page.waitForFunction(()=>document.documentElement.dataset.backplaneRemoteReady==='true'&&document.getElementById('composer'));
 const skip=page.locator('[data-on-click="ob-skip"]');if(await skip.count())await skip.click();await page.waitForFunction(()=>!document.querySelector('.app.tour-on'));
 await page.locator('.workspace-bar [data-on-click="term-toggle"]:visible').first().click();await page.waitForFunction(()=>document.querySelector('.term-pane .screen')?.textContent.includes('PTY_ECHO_READY'));
 await page.locator('#term').focus();result.originalPty=JSON.parse(fs.readFileSync(ptyMeta));result.bundle=await page.evaluate(()=>[...document.scripts].map(s=>s.src));result.eventLogBefore=fs.statSync(path.join(home,'events.jsonl')).size;
 const chars='asdfghjklqwertyuiopzxcvbnm12345';
 for(let i=0;i<chars.length;i++){await page.keyboard.press(chars[i]);await page.waitForFunction(n=>!!window.echoes[n],i+1,{timeout:6000});}
 result.trace=await page.evaluate(()=>window.echoTrace);result.errors=errors;result.eventLogAfter=fs.statSync(path.join(home,'events.jsonl')).size;
 result.pty=fs.readFileSync(ptyLog,'utf8').trim().split('\n').map(JSON.parse);
 const trace=result.trace;result.samples=trace.filter(x=>x.kind==='key').slice(0,chars.length).map(k=>{const end=trace.find(x=>x.kind==='dom-echo'&&x.n===k.n),pty=result.pty.find(x=>x.n===k.n),sends=trace.filter(x=>x.kind==='ws-send'&&x.ms>=k.ms&&x.ms<=end.ms),receives=trace.filter(x=>x.kind==='ws-recv'&&x.ms>=k.ms&&x.ms<=end.ms);return {n:k.n,key:k.key,keyMs:k.ms,domMs:end.ms,keyToDom:end.ms-k.ms,keyToFirstSend:sends[0]?.ms-k.ms,keyToPty:pty.readMs-k.ms,ptyToDom:end.ms-pty.writeMs,sendCount:sends.length,recvCount:receives.length,frame:trace.find(x=>x.kind==='frame-opportunity'&&x.n===k.n)?.ms-k.ms}});
 const q=(a,p)=>a.slice().sort((a,b)=>a-b)[Math.min(a.length-1,Math.floor(a.length*p))];for(const name of ['keyToDom','keyToFirstSend','keyToPty','ptyToDom','frame']){const vals=result.samples.map(x=>x[name]).filter(Number.isFinite);result[name]={n:vals.length,min:Math.min(...vals),p50:q(vals,.5),p95:q(vals,.95),max:Math.max(...vals)}}
 const termNode=await page.locator('#term').elementHandle();
 const composer=page.locator('#composer'),draft='Unsent latency fixture draft';await composer.fill(draft);const composerNode=await composer.elementHandle();
 await page.locator('#term').focus();
 const readPty=()=>fs.readFileSync(ptyLog,'utf8').split('\n').filter(Boolean).flatMap(line=>{try{return[JSON.parse(line)]}catch{return[]}});
 const waitPty=count=>new Promise((resolve,reject)=>{
   let watcher,timer;const done=()=>{const rows=readPty();if(rows.length<count)return;clearTimeout(timer);watcher?.close();resolve(rows)};
   watcher=fs.watch(ptyLog,done);timer=setTimeout(()=>{watcher.close();reject(Error('PTY input count '+readPty().length+'/'+count))},6000);done();
 });
 const burst='BurstInput0123456789'.repeat(3);await page.keyboard.type(burst);await waitPty(chars.length+burst.length);
 result.burst={bytes:burst.length,ordered:readPty().slice(chars.length).map(x=>String.fromCharCode(x.byte)).join('')===burst};
 let expected=Buffer.from(chars+burst);
 await page.keyboard.press('`');expected=Buffer.concat([expected,Buffer.from([96])]);await waitPty(expected.length);
 await page.keyboard.press('Control+c');expected=Buffer.concat([expected,Buffer.from([3])]);await waitPty(expected.length);
 await page.keyboard.press('ArrowUp');expected=Buffer.concat([expected,Buffer.from([27,91,65])]);await waitPty(expected.length);
 const pasted='line one\ncafé';await page.evaluate(text=>{const transfer=new DataTransfer();transfer.setData('text/plain',text);document.getElementById('term').dispatchEvent(new ClipboardEvent('paste',{clipboardData:transfer,bubbles:true,cancelable:true}))},pasted);
 // Existing Vt.paste converts LF/CRLF to the terminal's CR input; UTF-8 stays exact.
 expected=Buffer.concat([expected,Buffer.from(pasted.replace(/\r?\n/g,'\r'))]);await waitPty(expected.length);
 result.inputOrder=Buffer.from(readPty().map(x=>x.byte)).equals(expected);
 result.termNodeRetained=await termNode.evaluate(e=>e===document.getElementById('term'));
 result.composerNodeRetained=await composerNode.evaluate(e=>e===document.getElementById('composer'));
 result.draftRetained=await composer.inputValue()===draft;
 await page.locator('[data-on-click="ws-term-hide"]').click();await page.locator('.floating-terminal').waitFor({state:'detached'});
 await page.locator('.workspace-bar [data-on-click="term-toggle"]:visible').first().click();await page.locator('#term').waitFor();
 result.reopenedPty=JSON.parse(fs.readFileSync(ptyMeta));result.samePty=result.reopenedPty.pid===result.originalPty.pid;
 result.retainedHistory=await page.locator('.term-pane .screen').textContent().then(t=>t.includes('E00'));
 await page.setViewportSize({width:1024,height:600});await page.locator('#term').focus();await page.keyboard.press('z');expected=Buffer.concat([expected,Buffer.from('z')]);await waitPty(expected.length);
 result.inputOrderAfterResize=Buffer.from(readPty().map(x=>x.byte)).equals(expected);
 result.trace=await page.evaluate(()=>window.echoTrace);result.pty=readPty();result.eventLogAfter=fs.statSync(path.join(home,'events.jsonl')).size;
 result.ordered=result.pty.slice(0,chars.length).map(p=>String.fromCharCode(p.byte)).join('')===chars;await page.screenshot({path:path.join(out,'terminal.png')});result.state='measured';
 if(process.argv.includes('--accept')){assert(result.keyToDom.p95<30,'p95 key-to-DOM echo must be under30ms');assert(result.ordered&&result.burst.ordered&&result.inputOrder&&result.inputOrderAfterResize,'exact input order');assert(result.samePty&&result.retainedHistory&&result.termNodeRetained&&result.composerNodeRetained&&result.draftRetained,'PTY/history/draft/node retention');assert.equal(result.eventLogAfter,result.eventLogBefore,'terminal input/output must not persist events');result.state='accepted';}
}catch(e){result.state='failed';result.error=e.stack;process.exitCode=1;}finally{if(browser)await browser.close();if(service)await service.stop();fs.writeFileSync(path.join(out,'MEASUREMENT.json'),JSON.stringify(result,null,2));console.log(JSON.stringify({state:result.state,error:result.error,ordered:result.ordered,dom:result.keyToDom,burst:result.burst,inputOrder:result.inputOrder,samePty:result.samePty,draftRetained:result.draftRetained,keyToPty:result.keyToPty,ptyToDom:result.ptyToDom,frame:result.frame,eventLogDelta:result.eventLogAfter-result.eventLogBefore}));}})();
