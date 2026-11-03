// Real private hub/browser: entering a tool-heavy conversation must expose
// messages and useful history immediately, with closed tool bodies unloaded.
const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),assert=require('node:assert/strict');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright-core');
const {Service}=require('../service.cjs');
const [binary,web,output]=process.argv.slice(2);assert([binary,web,output].every(path.isAbsolute));
const home=fs.mkdtempSync(path.join(os.tmpdir(),'bp-thread-entry-')),agents=path.join(home,'agents');
if(process.env.ENTRY_REUSE){const prior=path.resolve(process.env.ENTRY_REUSE);assert(prior.startsWith(path.join(os.tmpdir(),'bp-thread-entry-')));fs.cpSync(prior,home,{recursive:true});}
fs.mkdirSync(agents,{recursive:true});
for(const name of ['claude','codex','grok','muse','agy','opencode','pi','omp'])fs.writeFileSync(path.join(agents,name),'#!/bin/sh\nexit 127\n',{mode:0o755});
const events=[{$:'ProjectCreated',id:'p',title:'Private thread entry',root:home,at:1},
 ...['chat','control'].map(id=>({$:'ThreadCreated',id,project:'p',title:id,env:'local',branch:'',worktree:home,provider:'codex',model:'',at:1})),
 ...(process.env.ENTRY_BOT==='1'?[{$:'BotSet',id:'fixture-bot',name:'Fixture bot',thread:'chat',look:0,persona:'',at:1}]:[])];
const imported=process.env.ENTRY_HISTORY?fs.readFileSync(process.env.ENTRY_HISTORY,'utf8').trim().split('\n').map(JSON.parse):[];
assert(imported.every(e=>e.$!=='ActivityLogged'||typeof e.act==='string'&&e.act),'Every imported activity has its actual codec act ID');
if(imported.length)events.push(...imported);else for(let i=0;i<120;i++)events.push({$:'MessagePosted',thread:'chat',msg:'message-'+i,role:'assistant',text:'Visible engineering message '+i+'\n\n'+'Measured work and results. '.repeat(12),at:i+2});
events.push({$:'MessagePosted',thread:'chat',msg:'human-context',role:'user',text:'Please inspect the saved design and explain the result.',at:99999});
const tail=Number(process.env.ENTRY_TAIL||128),sentinel='PRIVATE_TOOL_DETAIL_SENTINEL';
for(let i=0;i<tail;i++)events.push({$:'ActivityLogged',thread:'chat',act:'tail-tool-'+i,tone:'tool',kind:'shell',summary:'Inspecting the saved design\n'+sentinel+' '+i+'\n'+'Large private detail. '.repeat(1000),at:100000+i});
for(const [key,value]of [['restart.continue','off'],['mail.on','off'],['telemetry.share','off'],['onboard.done','1'],['onboard.uber','1']])events.push({$:'SettingSet',key,value,at:1});
if(!process.env.ENTRY_REUSE)fs.writeFileSync(path.join(home,'events.jsonl'),events.map(JSON.stringify).join('\n')+'\n',{mode:0o600});
if(process.env.ENTRY_REUSE&&process.env.ENTRY_BOT==='1')fs.appendFileSync(path.join(home,'events.jsonl'),JSON.stringify({$:'BotSet',id:'fixture-bot',name:'Fixture bot',thread:'chat',look:0,persona:'',at:1})+'\n');
const extraTools=Number(process.env.ENTRY_BUDGET_TAIL||0);
if(extraTools){assert(extraTools>1024);fs.appendFileSync(path.join(home,'events.jsonl'),Array.from({length:extraTools},(_,i)=>JSON.stringify({$:'ActivityLogged',thread:'chat',act:'budget-tool-'+i,tone:'tool',kind:'shell',summary:'Bounded metadata probe',at:200000+i})).join('\n')+'\n');}
Object.assign(process.env,{BACKPLANE_HOME:home,BACKPLANE_COPY:'1',BACKPLANE_AGENTS:agents,BACKPLANE_KISTACK_SYNC:'0',BACKPLANE_NO_UPDATE:'1',BACKPLANE_TELEMETRY:'off',BACKPLANE_PEERS:'',BACKPLANE_LOCK_DIR:home+'/locks',XDG_CACHE_HOME:home+'/cache',SHELL:'/bin/sh'});
const realIds=events.filter(e=>e.$==='MessagePosted'&&e.role==='assistant'&&e.text.trim()).map(e=>e.msg);
const service=new Service({home,binary,web}),result={state:'running',home,binary,web,tail,imported:imported.length,errors:[],wire:{sentBytes:0,receivedBytes:0,requests:[],closedDetailBytes:0},snapshots:[]};
let browser,page,phase='start';const privateFrames=[];
(async()=>{try{
 const connection=await service.start();browser=await chromium.launch({executablePath:process.env.BACKPLANE_CHROME||'/home/h/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome',headless:true,ignoreDefaultArgs:['--hide-scrollbars']});
 const context=await browser.newContext({viewport:{width:Number(process.env.ENTRY_WIDTH||2556),height:1398}});assert((await context.request.post(connection.origin+'/auth/pair',{headers:{Authorization:'Bearer '+await service.token()}})).ok());
 page=await context.newPage();page.setDefaultTimeout(12000);page.on('pageerror',e=>result.errors.push(e.message));
 const Wire=await import('../../src/web/wire.js'),cbor=fs.readFileSync(path.join(__dirname,'../../src/core/cbor.bend'),'utf8');
 const dictionary=name=>JSON.parse(new RegExp('def Cbor\\.'+name+'\\(\\) -> List<&2, String>:\\s*(\\[[\\s\\S]*?\\])').exec(cbor)[1]),keys=dictionary('keys'),words=dictionary('words');
 page.on('websocket',socket=>{for(const event of ['framesent','framereceived'])socket.on(event,frame=>{
  const bytes=typeof frame.payload==='string'?Buffer.from(frame.payload):frame.payload;result.wire[event==='framesent'?'sentBytes':'receivedBytes']+=bytes.length;
  if(event==='framereceived'&&phase==='entry'&&bytes.includes(Buffer.from(sentinel)))result.wire.closedDetailBytes+=bytes.length;
  if(event==='framesent')try{const o=typeof frame.payload==='string'?JSON.parse(frame.payload):Wire.plain(Wire.cbor(frame.payload,keys,words));privateFrames.push({event,phase,envelope:o});if(o.m)result.wire.requests.push({method:o.m,action:o.p?.action,target:o.p?.target,cursor:o.p?.cursor,phase});}catch{}
  if(event==='framereceived')try{const o=typeof frame.payload==='string'?JSON.parse(frame.payload):Wire.plain(Wire.cbor(frame.payload,keys,words));privateFrames.push({event,phase,envelope:o});const r=typeof o.result==='string'?JSON.parse(o.result):o.result;result.wire.replies||=[];if(o.t==='reply')result.wire.replies.push({id:o.id,ok:o.ok,target:r?.target,selected:r?.session?.sel,next:r?.nextTarget,ready:r?.remoteReady,error:o.error?.code,cmds:r?.cmds?.map(c=>({type:c.type,thread:c.thread})),phase});}catch{}
 });});
 await page.addInitScript(()=>{localStorage.setItem('backplane-selected','control');window.__bpPerf={trace:[],render:[],part:[],live:[],act:[]};window.__bpRender={n(){},t(){},sent(){},got(){}};window.__entryEvents=[];for(const event of ['keydown','focusin','click'])document.addEventListener(event,e=>{if(e.target.id==='history-fill-more'||event==='keydown'&&['Tab','Enter'].includes(e.key))queueMicrotask(()=>__entryEvents.push({event,key:e.key,target:e.target.id,trusted:e.isTrusted,prevented:e.defaultPrevented,active:document.activeElement?.id}));},true);window.__entryMetrics={longTasks:[],firstMessage:null,firstScroll:null};new PerformanceObserver(list=>__entryMetrics.longTasks.push(...list.getEntries().map(e=>({at:e.startTime,ms:e.duration})))).observe({type:'longtask',buffered:true});});
 if(process.env.ENTRY_DELAY)await page.routeWebSocket(/\/ws\?view=1/,ws=>{const remote=ws.connectToServer();remote.onMessage(message=>setTimeout(()=>ws.send(message),Number(process.env.ENTRY_DELAY)));});
 await page.goto(connection.origin);await page.waitForFunction(()=>document.documentElement.dataset.backplaneRemoteReady==='true'&&document.getElementById('composer'));
 if(process.env.ENTRY_WAIT_QUIET==='1')await page.waitForFunction(()=>__bpRender.paging().ready&&!__bpRender.paging().pending);
 if(Number(process.env.ENTRY_WIDTH||2556)<760)await page.locator('button.back[data-on-click="select"]').first().click();
 phase='entry';const start=performance.now();
 if(process.env.ENTRY_BOT==='1')await page.locator('[data-on-click="bot"][data-value="fixture-bot"]').first().click();else await page.locator('[data-on-click="select"][data-value="chat"]').first().click();
 await page.waitForFunction(()=>document.querySelector('main')?.dataset.thread==='chat'&&document.documentElement.dataset.backplaneRemoteReady==='true');result.readyMs=performance.now()-start;
 const entry=()=>page.evaluate(()=>{const t=document.getElementById('timeline'),b=t.getBoundingClientRect();return{top:t.scrollTop,height:t.scrollHeight,view:t.clientHeight,messages:[...t.querySelectorAll('.msg')].filter(e=>{const r=e.getBoundingClientRect();return r.bottom>b.top&&r.top<b.bottom&&r.height>0}).map(e=>({id:e.dataset.id,text:e.innerText.slice(0,100)})),folds:[...t.querySelectorAll('button.fold')].map(e=>({value:e.dataset.value,text:e.textContent})),rows:t.querySelectorAll('.timeline-virtual-row').length};});
 result.snapshots.push({label:'ready',...(await entry())});
 try{await page.waitForFunction(ids=>{const t=document.getElementById('timeline'),b=t?.getBoundingClientRect();return t&&[...t.querySelectorAll('.msg')].some(e=>{const r=e.getBoundingClientRect();return ids.includes(e.dataset.id)&&r.height>0&&r.bottom>b.top&&r.top<b.bottom;});},realIds,{timeout:20000});result.firstMessageMs=performance.now()-start;}catch{}
 result.snapshots.push({label:'entry-settled',...(await entry())});await page.screenshot({path:output+'.entry.png'});
 assert(result.firstMessageMs,'Entering a tool-heavy conversation paints committed text without a wheel gesture');
 await page.waitForFunction(()=>__bpRender.paging().ready&&!__bpRender.paging().paging&&!__bpRender.paging().pending);
 await page.waitForFunction(()=>{const t=document.getElementById('timeline');return [...t.querySelectorAll('.msg.user')].some(e=>e.dataset.id==='human-context');});
 const settled=await entry();result.snapshots.push({label:'context-complete',...settled});
 assert.equal(settled.folds.length,1,'all closed calls remain in one drawer across the source pages');
 const initialPages=result.wire.requests.filter(r=>r.phase==='entry'&&r.method==='view.page').length;
 assert(initialPages>0,'text context loads automatically through the folded source pages');
 assert(initialPages<Math.ceil((tail+extraTools)/8)+20,'source cursors bound the initial context seek');
 result.budget={extraTools,initialPageRequests:initialPages};
 phase='fold-expansion';
 await page.locator('#timeline button.fold').click();
 await page.waitForFunction(()=>document.querySelector('#timeline button.fold')?.dataset.value==='0tail-tool-0'&&document.querySelector('#timeline [data-run="tail-tool-0"]'));
 await page.locator('#timeline button.fold').click();
 await page.waitForFunction(()=>document.querySelector('#timeline button.fold')?.dataset.value==='1tail-tool-0'&&!document.querySelector('#timeline [data-run="tail-tool-0"]'));

 assert.equal(result.wire.closedDetailBytes,0,'closed tool payloads do not ride ordinary view frames');assert.equal(result.wire.requests.filter(r=>r.phase==='entry'&&r.method==='view.message').length,0,'closed tools do not request body pages');
 assert.equal(result.wire.requests.filter(r=>r.method==='view.action'&&['send','send-alt'].includes(r.action)||['thread.send','thread.post'].includes(r.method)).length,0,'reader navigation never sends a composer turn');assert.deepEqual(result.errors,[]);result.state='passed';
}catch(e){result.state='failed';result.phase=phase;result.error=e.stack;process.exitCode=1;
}finally{if(page){result.metrics=await page.evaluate(()=>({...__entryMetrics,inputEvents:window.__entryEvents,trace:window.__bpPerf?.trace,selected:document.querySelector('main')?.dataset.thread,ready:document.documentElement.dataset.backplaneRemoteReady,paging:__bpRender.paging?.(),draftLength:document.getElementById('composer')?.value.length})).catch(()=>null);await page.screenshot({path:output+'.final.png'}).catch(()=>{});}fs.writeFileSync(output+'.wire-private.json',JSON.stringify(privateFrames)+'\n',{mode:0o600});if(browser)await browser.close();await service.stop();fs.writeFileSync(output,JSON.stringify(result,null,2)+'\n',{mode:0o600});console.log(JSON.stringify({state:result.state,phase:result.phase,error:result.error,output}));}})();
