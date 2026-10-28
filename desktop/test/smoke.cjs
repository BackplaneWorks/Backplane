// Real Electron + existing standalone Bend service, isolated generated data.
// Seed data is deliberately synthetic; this never reads/copies the live home.
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const assert = require('node:assert/strict');
const { _electron } = require('playwright-core');
const vm = require('node:vm');
const {Service}=require('../service.cjs');
const root = path.resolve(__dirname, '../..');
const artifact = process.env.BACKPLANE_SERVICE;
const expectedVersion = process.env.BACKPLANE_SERVICE_VERSION || '0.11.2-dev.mail-45328b78';
const web = process.env.BACKPLANE_WEB || path.join(root, 'build/electron-web');
if (!artifact || !path.isAbsolute(artifact)) throw new Error('Set BACKPLANE_SERVICE to the audited standalone artifact');
const home = fs.mkdtempSync(path.join(os.tmpdir(), 'backplane-electron-smoke-'));
const project = path.join(home, 'project'); fs.mkdirSync(project);
const fake = path.join(home, 'agents'); fs.mkdirSync(fake);
for (const name of ['claude', 'codex', 'grok', 'muse', 'agy']) {
  fs.writeFileSync(path.join(fake, name), '#!/bin/sh\nexit 127\n', { mode: 0o755 });
}
fs.writeFileSync(path.join(fake,'claude'), '#!/bin/sh\nexec ' + JSON.stringify(process.execPath) + ' ' + JSON.stringify(path.join(__dirname,'stream-agent.cjs')) + ' "$@"\n', {mode:0o755});
const wireDir=process.env.BACKPLANE_TEST_WIRE || path.resolve(path.dirname(artifact),'../../../mail-4532/wire');
const wireFile=fs.readdirSync(wireDir).find(file=>file.endsWith('.js'));
vm.runInThisContext(fs.readFileSync(path.join(wireDir,wireFile),'utf8'));
const at = Math.floor(Date.now()/1000);
const event = ($, fields) => ({ $, ...fields, at });
const thread = (id, title) => event('ThreadCreated', { id, title, project:'electron-smoke', env:'local', branch:'', worktree:project, provider:'claude', model:'sonnet' });
const events = [event('ProjectCreated',{id:'electron-smoke',title:'Electron service smoke',root:project}),
  thread('smoke-chat','Service-backed conversation'), thread('bub-thread','Bub chat'),
  event('BotSet',{id:'bub',name:'Bub',thread:'bub-thread',look:0,persona:''}),
  event('UberSet',{bot:'bub',uid:'electron-smoke-bub'}),
  event('SettingSet',{key:'restart.continue',value:'off'}),
  event('SettingSet',{key:'mail.on',value:process.env.BP_TEST_MAIL_AUTOMATION || 'off'}),
  event('SettingSet',{key:'onboard.done',value:'1'}),
  event('ConvSet',{id:'smoke-conversation',subject:'Review the Electron mailbox',kind:'pair',parts:'bot:bub\nperson',project:'electron-smoke',tags:''}),
  event('MailPosted',{id:'smoke-mail',conv:'smoke-conversation',from:'person',to:'bot:bub',kind:'message',ref:'',want:'act',head:'Please verify real mailbox content and needs-reply state.',size:59}),
  event('MessagePosted',{thread:'smoke-chat',msg:'smoke-user',role:'user',text:'Does this conversation come from the Bend service?'}),
  event('MessagePosted',{thread:'smoke-chat',msg:'smoke-assistant',role:'assistant',text:'Yes. This project, thread and chat are loaded from the actual isolated Bend event log.'})];
const space={title:'Electron Space blocks',blocks:[
  {type:'heading',text:'Current project'},{type:'text',text:'Real **Bend** Space content'},
  {type:'stat',label:'Boards',value:3},{type:'progress',label:'Coverage',value:0.5},
  {type:'list',items:['Board','Schematic']},{type:'kv',rows:[['State','Review']]},
  {type:'table',head:['Part','State'],rows:[['Main','Ready']]},{type:'button',label:'Inspect',action:'inspect'},
  {type:'input',label:'Note',action:'note',placeholder:'Write a note'},{type:'divider'},
  {type:'badge',text:'Ready',tone:'ok'},{type:'code',text:'value = 3'},
  {type:'row',blocks:[{type:'stat',label:'Tests',value:4}]}]};
events.push(event('SpaceSet',{bot:'bub',spec:JSON.stringify(space)}),thread('smoke-approval','Approval fixture'),
  event('AskOpened',{id:'fixture-ask',thread:'smoke-approval',kind:'approval',tool:'Read',detail:'Fixture approval: inspect the board',options:''}));
const history=Array.from({length:2000},(_,i)=>event('MessagePosted',{thread:'smoke-chat',msg:'history-'+i,role:i%2?'assistant':'user',text:`History entry ${i}: component placement, test results and a readable reply. `+'Measured fixture content. '.repeat(8)}));
events.splice(events.findIndex(e=>e.$==='MessagePosted'&&e.msg==='smoke-user'),0,...history);
fs.writeFileSync(path.join(home, 'events.jsonl'),events.map(JSON.stringify).join('\n')+'\n');
const out = path.join(root,'build/electron-evidence'); fs.mkdirSync(out,{recursive:true});
const sourceFiles=['desktop/main.cjs','desktop/preload.cjs','desktop/service.cjs','desktop/startup.html','desktop/startup.css','desktop/startup.js','src/web/app.bend','src/web/bots.bend','src/web/view.bend','src/web/host.js','src/web/style.css','src/web/mailroom.bend'];
const crypto=require('node:crypto');
const sourceHashes=Object.fromEntries(sourceFiles.map(file=>[file,crypto.createHash('sha256').update(fs.readFileSync(path.join(root,file))).digest('hex')]));

let app, page;
const testService=new Service({home,binary:artifact,web,expectedVersion});
(async () => {
  const errors=[];
  try {
    Object.assign(process.env,{BACKPLANE_AGENTS:fake,BP_AGENT_TRACE:path.join(home,'agent-trace.jsonl'),BACKPLANE_NO_UPDATE:'1',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',BACKPLANE_PEERS:'',BACKPLANE_KEYRING_LOCKED:'0',BACKPLANE_GOOGLE_BAKE:'0',BACKPLANE_LOCK_DIR:path.join(home,'locks')});
    await testService.start();
    app = await _electron.launch({executablePath:require('electron'),args:[path.join(root,'desktop')],env:{...process.env,
      BACKPLANE_HOME:home,BACKPLANE_SERVICE:artifact,BACKPLANE_SERVICE_VERSION:expectedVersion,BACKPLANE_WEB:web,BACKPLANE_AGENTS:fake,BP_AGENT_TRACE:path.join(home,'agent-trace.jsonl'),
      BACKPLANE_NO_UPDATE:'1',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',BACKPLANE_PEERS:'',BACKPLANE_KEYRING_LOCKED:'0',BACKPLANE_GOOGLE_BAKE:'0',
      BACKPLANE_LOCK_DIR:path.join(home,'locks'),DISPLAY:process.env.DISPLAY || ':0'},timeout:60000});
    page=await app.firstWindow({timeout:60000});
    page.on('pageerror',e=>errors.push(e.message));
    await page.addInitScript(()=>{globalThis.__bpPerf={act:[],msg:[],render:[],part:[],live:[],trace:[]};});
    await page.reload();
    const rpc = async (method,params) => {
      const bytes=Array.from(Wire.encode(JSON.stringify({id:'smoke-'+Date.now(),m:method,p:params})));
      await page.evaluate(bytes=>new Promise((resolve,reject)=>{
        const socket=new WebSocket(location.origin.replace('http','ws')+'/ws?enc=cbor');
        socket.onopen=()=>{socket.send(new Uint8Array(bytes));resolve();setTimeout(()=>socket.close(),1000);};socket.onerror=reject;
      }),bytes);
    };
    await page.waitForFunction(()=>document.documentElement.dataset.backplaneReady === 'true',null,{timeout:45000});
    await page.getByText('Bub',{exact:true}).first().waitFor({timeout:45000});
    await rpc('mail.send',{conv:'smoke-conversation',to:'bot:bub',text:'FULL BODY: verified real mailbox RPC with Japanese 日本語 and complete message content.',want:'fyi'});
    await page.locator('[data-on-click="uber"]').first().click();
    await page.locator('#mailroom').waitFor({timeout:15000}).catch(async()=>{
      await page.getByRole('button',{name:/^Mailroom/}).click();
      await page.locator('#mailroom').waitFor({timeout:15000});
    });
    await page.getByText('Review the Electron mailbox',{exact:true}).waitFor({timeout:20000});
    await page.getByText('Review the Electron mailbox',{exact:true}).click();
    await page.locator('.mail-text').filter({hasText:'FULL BODY: verified real mailbox RPC with Japanese 日本語 and complete message content.'}).waitFor({timeout:20000});
    await page.screenshot({path:path.join(out,'mailroom.png')});
    await page.locator('.mail-targets button').first().click();
    await page.locator('#mail-reply').fill('Electron mailbox reply accepted by the actual service.');
    await page.locator('.mail-conversation [data-on-click="mail-send"]').click();
    await page.locator('.mail-text').filter({hasText:'Electron mailbox reply accepted by the actual service.'}).waitFor({timeout:20000});
    await page.locator('.mail-send-status').filter({hasText:'Filed'}).waitFor({timeout:10000});
    if(process.env.BP_TEST_MAIL_AUTOMATION !== 'on') {
      await page.locator('.mail-send-status').filter({hasText:'Filed; automation paused'}).waitFor();
      const mailEvents=fs.readFileSync(path.join(home,'events.jsonl'),'utf8').trim().split('\n').map(JSON.parse);
      assert(!mailEvents.some(e=>e.$==='RunSet'),'quiet UI filing creates no hidden run');
      assert(!mailEvents.some(e=>e.$==='MessagePosted'&&e.thread==='bub-thread'),'quiet UI filing creates no hidden Bub post');
      assert(mailEvents.some(e=>e.$==='MailMarked'&&e.bot==='bub'&&e.conv==='smoke-conversation'),'actual receiver read mark');
    }
    await page.screenshot({path:path.join(out,'mail-reply.png')});
    await page.reload();
    await page.getByText('Bub',{exact:true}).first().waitFor({timeout:20000});
    const versions=await page.evaluate(()=>window.backplaneDesktop.version());
    const isolation=await page.evaluate(()=>({require:typeof window.require,process:typeof window.process,ipc:typeof window.backplaneDesktop.chooseFolder}));
    assert.deepEqual(isolation,{require:'undefined',process:'undefined',ipc:'function'});
    await page.locator('[data-on-click="select"][data-value="smoke-chat"]').first().click();
    await page.getByText('Yes. This project, thread and chat are loaded from the actual isolated Bend event log.',{exact:true}).waitFor({timeout:20000});
    await page.screenshot({path:path.join(out,'chat.png')});
    await page.evaluate(()=>{
      globalThis.__streamSamples=[];globalThis.__frameIntervals=[];
      let last=performance.now(),end=last+10000;
      function frame(now){globalThis.__frameIntervals.push(now-last);last=now;if(now<end)requestAnimationFrame(frame);} requestAnimationFrame(frame);
      const seen=new Set();new MutationObserver(()=>{
        const text=document.querySelector('.msg.live')?.textContent||'';
        for(const match of text.matchAll(/sample-(\d+)\((\d+)\)/g)) if(!seen.has(match[1])){seen.add(match[1]);globalThis.__streamSamples.push({sample:+match[1],emitted:+match[2],dom:Date.now()});}
      }).observe(document.querySelector('#timeline'),{subtree:true,childList:true,characterData:true});
    });
    await page.locator('#composer').fill('Start the sustained stream.');
    await page.locator('#composer').press('Enter');
    await page.locator('.msg.live').waitFor({timeout:20000});
    await page.locator('#composer').pressSequentially('Typing while the reply streams 日本語',{delay:20});
    await page.waitForFunction(()=>document.querySelector('#timeline')?.textContent.includes('sample-119('),null,{timeout:25000});
    const performanceEvidence=await page.evaluate(()=>({timings:globalThis.__bpPerf,samples:globalThis.__streamSamples,frames:globalThis.__frameIntervals,seq:document.documentElement.dataset.backplaneSeq}));
    const ordered=fs.readFileSync(path.join(home,'events.jsonl'),'utf8').trim().split('\n').map(JSON.parse);
    assert(ordered.some(e=>e.$==='MessagePosted'&&e.text==='Start the sustained stream.'),'service accepted typed chat input');
    fs.writeFileSync(path.join(out,'performance.json'),JSON.stringify({historyEntries:history.length,logBytes:fs.statSync(path.join(home,'events.jsonl')).size,...performanceEvidence},null,2));
    await page.screenshot({path:path.join(out,'stream.png')});
    // Broad UI/CAD flows are deliberately outside this mailbox stage.
    await page.setViewportSize({width:390,height:844});
    await page.screenshot({path:path.join(out,'mobile.png')});
    fs.writeFileSync(path.join(out,'smoke.json'),JSON.stringify({status:'passed',home,artifact,web,sourceHashes,versions,isolation,errors,checks:['real service boot','project/thread/chat DOM','local Bub Mailroom + full message body + actual reply RPC','reload + authoritative catchup','isolated preload bridge','390px layout screenshot','2000-entry history + sustained 120-delta stream + typing']},null,2));
    assert.equal(errors.length,0,errors.join('\n'));
    console.log('Electron service smoke passed; evidence: '+out);
  } catch(error) {
    if(page) { await page.screenshot({path:path.join(out,'failure.png')}).catch(()=>{}); fs.writeFileSync(path.join(out,'failure.html'),await page.content().catch(()=>'')); }
    fs.writeFileSync(path.join(out,'failure.json'),JSON.stringify({status:'failed',home,artifact,web,error:error.stack,errors},null,2));
    console.error(error); process.exitCode=1;
  } finally {
    if(app) await app.close();
    try {
      const afterClose=await testService.discover();
      fs.writeFileSync(path.join(out,'service-after-ui-close.json'),JSON.stringify({alive:!!afterClose,version:afterClose?.version},null,2));
      assert(afterClose,'service survives UI close');
    } finally { testService.stop(); }
  }
})();
