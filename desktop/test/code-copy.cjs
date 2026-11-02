'use strict';
// Real rendered assistant messages, isolated service/profile and clipboard.
// Usage: CODE_COPY_WEB=ABS_WEB CODE_COPY_SERVICE=ABS_BINARY node desktop/test/code-copy.cjs
// Set CODE_COPY_ELECTRON_RUNTIME=ABS_RUNTIME to also exercise the native bridge.
const fs = require('node:fs'), path = require('node:path'), os = require('node:os'), assert = require('node:assert/strict');
const {chromium, _electron} = require(process.env.PLAYWRIGHT_MODULE || 'playwright-core');
const {Service} = require('../service.cjs');
// Each platform gets a fresh hub: a prior platform's live reply must never
// satisfy this platform's wait merely because virtualization unmounted it.
if(!process.env.CODE_COPY_CASE&&process.env.CODE_COPY_ONLY_ELECTRON!=='1') {
  const output=path.resolve(process.env.CODE_COPY_OUT||path.join(__dirname,'../../build/code-copy/results'));
  fs.mkdirSync(output,{recursive:true});
  const results=[],sessions=[];
  for(const name of ['web','mobile-web',...(process.env.CODE_COPY_ELECTRON_RUNTIME?['electron']:[])]) {
    const dir=path.join(output,name);
    const child=require('node:child_process').spawnSync(process.execPath,[__filename],{stdio:'inherit',env:{...process.env,CODE_COPY_CASE:name,CODE_COPY_OUT:dir}});
    if(child.status!==0)process.exit(child.status||1);
    const result=JSON.parse(fs.readFileSync(dir+'/RESULT.json'));
    results.push(...result.results);sessions.push({name,home:result.home,liveMessageId:result.liveMessageId,liveText:result.liveText});
  }
  assert.equal(new Set(sessions.map(s=>s.home)).size,sessions.length,'each platform owns a fresh conversation home');
  assert.equal(new Set(sessions.map(s=>s.liveMessageId)).size,sessions.length,'each platform receives a distinct new live message');
  fs.writeFileSync(output+'/RESULT.json',JSON.stringify({ok:true,isolatedPerPlatform:true,results,sessions},null,2));
  console.log(JSON.stringify({ok:true,isolatedPerPlatform:true,results,sessions,out:output}));process.exit(0);
}
const root = path.resolve(__dirname, '../..'), home = fs.mkdtempSync(path.join(os.tmpdir(), 'bp-code-copy-'));
const out = path.resolve(process.env.CODE_COPY_OUT || root + '/build/code-copy/results');fs.mkdirSync(out, {recursive:true});
const web = path.resolve(process.env.CODE_COPY_WEB || root + '/build/code-copy/web');
const binary = process.env.CODE_COPY_SERVICE;assert(binary && path.isAbsolute(binary), 'Provide an isolated service binary');
const thread = 'bot-chat', project = home + '/project', agents = home + '/agents';fs.mkdirSync(project);fs.mkdirSync(agents);
const sample = "ssh -t h@backplane 'sudo bash /home/h/memtest/apply-memmap.sh && sudo reboot'";
const older = '# a literal shell comment\nprintf \'日本語 🦊 & <script>literal</script>\'\n  echo "quoted"';
const caseId=process.env.CODE_COPY_CASE||'electron';
const live = 'printf "live assistant 日本語 '+caseId+'"\necho "$HOME"';
const markdown = text => 'Use this command:\n\n```sh\n' + text + '\n```\n\nAfter the block.';
const response = markdown(live);
for (const name of ['claude','grok','muse','agy','opencode','pi','omp']) fs.writeFileSync(agents + '/' + name, '#!/bin/sh\nexit 127\n', {mode:0o755});
fs.writeFileSync(agents + '/codex', '#!/usr/bin/env node\nif(process.argv[2]==="--version"){console.log("codex-cli 1.0.0");}else{for(const event of ' + JSON.stringify([
  {type:'thread.started',thread_id:'copy-fixture-session'},
  {type:'item.completed',item:{id:'live-answer-'+caseId,type:'agent_message',text:response}},
  {type:'turn.completed',usage:{input_tokens:1,output_tokens:1}},
]) + '){if(event.item)event.item.id+="-"+process.pid;console.log(JSON.stringify(event));}}\n', {mode:0o755});
const events = [{$:'ProjectCreated',id:'fixture',title:'Copy fixture',root:project,at:1},
  {$:'ThreadCreated',id:thread,project:'fixture',title:'Bot conversation',env:'local',branch:'',worktree:project,provider:'codex',model:'',at:2},
  {$:'BotSet',id:'fixture-bot',name:'Copy Bot',thread,look:0,persona:'',at:3},{$:'UberSet',bot:'fixture-bot',uid:'copy-fixture',at:4}];
for (let i = 0; i < 70; i++) events.push({$:'MessagePosted',thread,msg:'m' + i,role:'assistant',text:i === 0 ? markdown(older) : i === 69 ? markdown(sample) : 'History message ' + i + '\n\n' + 'Retained history text. '.repeat(10),at:10+i});
for (const key of ['restart.continue','mail.on','telemetry.share','order.enabled','order.estimates']) events.push({$:'SettingSet',key,value:'off',at:100});
for (const key of ['onboard.done','onboard.uber']) events.push({$:'SettingSet',key,value:'1',at:100});
fs.writeFileSync(home + '/events.jsonl', events.map(JSON.stringify).join('\n') + '\n');
const env = {...process.env,BACKPLANE_HOME:home,BACKPLANE_AGENTS:agents,BACKPLANE_COPY:'1',BACKPLANE_NO_UPDATE:'1',BACKPLANE_KISTACK_SYNC:'0',BACKPLANE_TELEMETRY:'off',BACKPLANE_PEERS:'',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',BACKPLANE_LOCK_DIR:home+'/locks',BACKPLANE_SERVICE:binary,BACKPLANE_WEB:web};
Object.assign(process.env, env);
const service = new Service({home,binary,web});
const results = [], errors = [];
let browser, app, page, liveMessageId;
async function scenario(name, context, read, native) {
  page.setDefaultTimeout(15000);
  page.on('pageerror', error => errors.push(error.message));
  if(native)await page.waitForFunction(()=>document.documentElement.dataset.backplaneRemoteReady==='true',null,{timeout:30000});
  let actions = 0, pages = 0;
  const {cbor,plain}=await import('../../src/web/wire.js'),source=fs.readFileSync(root+'/src/core/cbor.bend','utf8');
  const dictionary=name=>JSON.parse(new RegExp('def Cbor\\.'+name+'\\(\\) -> List<&2, String>:\\s*(\\[[\\s\\S]*?\\])\\n').exec(source)[1]);
  const keys=dictionary('keys'),words=dictionary('words');
  page.on('websocket', socket => socket.on('framesent', frame => {
    const q=typeof frame.payload==='string'?JSON.parse(frame.payload):plain(cbor(frame.payload,keys,words));
    if(q.m==='view.action'&&q.p?.action==='copy')actions++;if(q.m==='view.page')pages++;
  }));
  await page.reload();
  await page.waitForFunction(() => document.documentElement.dataset.backplaneRemoteReady === 'true');
  // Open the bot through the same sidebar action as the reported click.
  await page.locator('[data-on-click="uber"][data-value="fixture-bot"]').first().click();
  await page.waitForFunction(() => document.documentElement.dataset.backplaneRemoteReady === 'true' && document.querySelector('.codeblock .copy'));
  const click = async (selector, expected) => {
    const button = page.locator(selector).first();await button.waitFor();
    const before = actions;
    await button.click();
    await page.waitForFunction(selector => document.querySelector(selector)?.textContent === 'Copied', selector);
    assert.equal(await read(), expected, name + ': exact clipboard text');
    assert.equal(actions, before, name + ': copying visible text must not send a hub action');
    results.push(name + ': ' + selector);
  };
  if (native) {
    // Force a denied web API: Electron must use the actual preload/main bridge.
    await page.evaluate(() => Object.defineProperty(navigator, 'clipboard', {configurable:true,value:{writeText:async()=>{throw Error('Web clipboard refused');}}}));
  }
  await click('[data-id="m69"] .codeblock .copy', sample);
  // Once text is on screen, copying must not wait for a view.action ACK.
  // Preserve other traffic: dropping unrelated frames could strand a real
  // outstanding action and prevent the later live-message fixture send.
  await page.evaluate(fs.readFileSync(root+'/src/web/wire.js','utf8').replace(/^export /gm,'')+'\nwindow.__copyWire={cbor,plain};');
  await page.evaluate(({keys,words}) => {
    const send=WebSocket.prototype.send;
    window.__copyRestoreSend=()=>WebSocket.prototype.send=send;
    WebSocket.prototype.send=function(value) {
      const q=typeof value==='string'?JSON.parse(value):window.__copyWire.plain(window.__copyWire.cbor(new Uint8Array(value),keys,words));
      if(q.m==='view.action'&&q.p?.action==='copy')return;
      return send.call(this,value);
    };
  }, {keys,words});
  await click('[data-id="m69"] .codeblock .copy', sample);
  await page.evaluate(() => window.__copyRestoreSend());
  await page.locator('#composer').fill('Unsent draft 日本語');
  const saved = await page.evaluate(() => localStorage.getItem('backplane-draft:bot-chat'));
  // Missing API fallback uses execCommand and restores the composer's range.
  if (!native) {
    await page.evaluate(() => {window.__clipboard= navigator.clipboard;Object.defineProperty(navigator,'clipboard',{configurable:true,value:undefined});});
    await click('[data-id="m69"] .codeblock .copy', sample);
    await page.evaluate(() => Object.defineProperty(navigator,'clipboard',{configurable:true,value:{writeText:async()=>{throw new DOMException('Denied','NotAllowedError');}}}));
    await click('[data-id="m69"] .codeblock .copy', sample);
    await page.evaluate(() => Object.defineProperty(navigator,'clipboard',{configurable:true,value:window.__clipboard}));
  }
  assert.equal(await page.locator('#composer').inputValue(), 'Unsent draft 日本語');
  assert.equal(await page.evaluate(() => localStorage.getItem('backplane-draft:bot-chat')), saved);
  // Request older entries using the actual scroller; event delegation must
  // still reach Copy after page landing and virtualized DOM replacement.
  for (let i = 0; i < 12 && !await page.locator('[data-id="m0"] .codeblock .copy').count(); i++) {
    await page.evaluate(() => {const t=document.getElementById('timeline');t.scrollTop=0;t.dispatchEvent(new Event('scroll'));});
    await page.waitForTimeout(350);
  }
  assert(pages > 0, name + ': exercised actual entry paging');
  await click('[data-id="m0"] .codeblock .copy', older);
  // A genuine stand-in agent response arrives over the live service socket.
  await page.evaluate(() => {const t=document.getElementById('timeline');t.scrollTop=t.scrollHeight;});
  const oldIds = await page.locator('.msg[data-id]').evaluateAll(es=>es.map(e=>e.dataset.id));
  const journal=()=>fs.readFileSync(home+'/events.jsonl','utf8').trim().split('\n').map(JSON.parse);
  assert(!journal().some(e=>e.$==='MessagePosted'&&e.role==='assistant'&&e.text===response),'live response must not exist before the send');
  await page.locator('#composer').fill('Please reply with the fixture code');await page.locator('.thread-view button[data-on-click="send"]').click();
  await page.waitForFunction(({text,oldIds}) => [...document.querySelectorAll('.codeblock .copy')].some(b=>b.getAttribute('data-value')===text&&!oldIds.includes(b.closest('.msg')?.dataset.id)), {text:live,oldIds});
  const liveSelector = await page.evaluate(({text,oldIds}) => {
    const b=[...document.querySelectorAll('.codeblock .copy')].find(b=>b.getAttribute('data-value')===text&&!oldIds.includes(b.closest('.msg')?.dataset.id));
    return '[data-id="' + CSS.escape(b.closest('.msg').dataset.id) + '"] .codeblock .copy';
  }, {text:live,oldIds});
  liveMessageId=await page.locator(liveSelector).evaluate(b=>b.closest('.msg').dataset.id);
  assert(liveMessageId.startsWith('live-answer-'+caseId+'-'),'new response has the platform-specific ID');
  assert(journal().some(e=>e.$==='MessagePosted'&&e.role==='assistant'&&e.msg===liveMessageId&&e.text===response),'new live response is committed by the hub after the send');
  await click(liveSelector, live);
  await page.screenshot({path:path.join(out,name+'.png')});
}
(async () => {try {
  const connection = await service.start(), token = await service.token();
  const onlyElectron=process.env.CODE_COPY_ONLY_ELECTRON==='1'||process.env.CODE_COPY_CASE==='electron';
  if(!onlyElectron)browser = await chromium.launch({executablePath:process.env.BACKPLANE_CHROME,headless:true});
  for (const [name, viewport] of (onlyElectron?[]:[['web',{width:1440,height:1000}],['mobile-web',{width:390,height:844}]]).filter(([name])=>!process.env.CODE_COPY_CASE||process.env.CODE_COPY_CASE===name)) {
    const context = await browser.newContext({viewport,...(name==='mobile-web'?{isMobile:true,hasTouch:true}:{}),permissions:['clipboard-read','clipboard-write']});
    await context.request.post(connection.origin+'/auth/pair',{headers:{Authorization:'Bearer '+token}});
    page = await context.newPage();await page.goto(connection.origin);
    const reader = await context.newPage();await reader.goto(connection.origin);
    await scenario(name,context,async()=>{await reader.bringToFront();return reader.evaluate(()=>navigator.clipboard.readText());},false);
    await context.close();
  }
  await browser?.close();browser=null;
  if (process.env.CODE_COPY_ELECTRON_RUNTIME&&onlyElectron) {
    const runtime = path.resolve(process.env.CODE_COPY_ELECTRON_RUNTIME), staged = home+'/runtime';fs.mkdirSync(staged);
    for (const file of fs.readdirSync(runtime)) {
      if (file==='resources') continue;
      const source=path.join(runtime,file),dest=path.join(staged,file==='electron'?'backplane-desktop':file);
      if (file==='backplane-desktop'||file==='electron') fs.copyFileSync(source,dest,fs.constants.COPYFILE_FICLONE);
      else fs.symlinkSync(source,dest);
    }
    const resources=staged+'/resources',appRoot=resources+'/app';fs.mkdirSync(appRoot,{recursive:true});
    for (const file of fs.readdirSync(runtime+'/resources')) if(file!=='app')fs.symlinkSync(runtime+'/resources/'+file,resources+'/'+file);
    if(!fs.existsSync(resources+'/DESKTOP-BUILD.json')&&!fs.existsSync(resources+'/backplane-service/BUILD.json'))fs.writeFileSync(resources+'/DESKTOP-BUILD.json',JSON.stringify({serviceVersion:connection.version}));
    for (const file of require('../prepare-components.cjs').appFiles(path.join(root,'desktop'))) fs.copyFileSync(path.join(root,'desktop',file),appRoot+'/'+file);
    if(fs.existsSync(runtime+'/resources/app/node_modules'))fs.symlinkSync(runtime+'/resources/app/node_modules',appRoot+'/node_modules');
    else {fs.mkdirSync(appRoot+'/node_modules');fs.symlinkSync(path.dirname(require.resolve('ws/package.json')),appRoot+'/node_modules/ws');}
    app=await _electron.launch({executablePath:staged+'/backplane-desktop',env,args:['--ozone-platform=headless','--disable-gpu']});
    page=await app.firstWindow();
    await scenario('electron',app.context(),()=>app.evaluate(async({clipboard})=>await clipboard.readText()),true);
    await app.close();app=null;
  }
  assert.deepEqual(errors,[]);fs.writeFileSync(out+'/RESULT.json',JSON.stringify({ok:true,results,web,binary,home,liveMessageId,liveText:live},null,2));console.log(JSON.stringify({ok:true,results,out,home,liveMessageId}));
}catch(error){console.error(error.stack);console.error(JSON.stringify({pageErrors:errors,results}));if(page&&!page.isClosed()){fs.writeFileSync(out+'/failure.html',await page.content());await page.screenshot({path:out+'/failure.png'}).catch(()=>{});}process.exitCode=1;}
finally{await app?.close();await browser?.close();service.stop();}})();
