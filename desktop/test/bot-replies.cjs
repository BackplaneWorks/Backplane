// Bot chat replies and context headers (core/reply.bend) in the real
// Electron app on an isolated hub: a bot's message heads with the thread it
// links most; Reply sends the quote to the hub (kept ahead of the text in
// the log, for the agent) while the person's message shows only an
// "In reply to" line that brings the original back into view.
const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),assert=require('node:assert/strict');
const {_electron}=require('playwright-core');
const {Service}=require('../service.cjs');
const stage=path.resolve(process.env.BACKPLANE_LOCAL_STAGE);
const out=path.join(stage,'bot-replies-'+Date.now());fs.mkdirSync(out);
const bub='t1790292962-8017',boost='t1793000000-1',other='t1793000000-2';
const title='12→48 V 480 W Discrete Boost';
const said=`Routed: [Boost](backplane://thread/${boost}) is done; see [it again](backplane://thread/${boost}) and [the other](backplane://thread/${other}).`;
// long enough that the hub pages its body (src/web/body-windows.js): the
// case where Reply and Copy were lost
const plan='Plan for the inductor swap\nSecond line only the agent should read\n\n'+Array.from({length:120},(_,i)=>`- step ${i+1}: measure the ripple at the output and log the inductor temperature`).join('\n');
// a message wider than any phone: a long command in a code block, a long
// unbroken token and a wide table (the page must still never scroll sideways)
const wide='Here is the command:\n\n```\n'+'find /home/user/.local/share/backplane-local -maxdepth 1 -mindepth 1 -type d -not -name 20261101-032220 -exec rm -rf {} + && du -sh ~/.cache/uv\n'.repeat(3)+'```\n\n`a-very-long-inline-code-token-that-does-not-break-anywhere-at-all-ever` and '+'A'.repeat(90)+'\n\n| Path | Size | Kept |\n|---|---|---|\n| `/home/user/Documents/i2cjak/bp-coord/windows-vm/disk-image-qcow2-65GB.img` | 65 GB | yes |';
const home=fs.mkdtempSync(path.join(os.tmpdir(),'backplane-bot-replies-'));
const project=path.join(home,'project'),agents=path.join(home,'agents');fs.mkdirSync(project);fs.mkdirSync(agents);
for(const name of ['claude','codex','grok','muse','agy','opencode'])fs.writeFileSync(path.join(agents,name),'#!/bin/sh\nexit 127\n',{mode:0o755});
const at=Math.floor(Date.now()/1000),event=($,fields)=>({$,...fields,at});
const thread=(id,t)=>event('ThreadCreated',{id,title:t,project:'fixture',env:'local',branch:'',worktree:project,provider:'codex',model:'gpt-6.1-sol'});
const events=[event('ProjectCreated',{id:'fixture',title:'Reply fixture',root:project}),thread(bub,'Bub Chat'),thread(boost,title),thread(other,'Other work'),
 event('BotSet',{id:'bub',name:'Bub',thread:bub,look:0,persona:''}),event('UberSet',{bot:'bub',uid:'bub-reply-fixture'}),
 ...['restart.continue','mail.on','telemetry.share'].map(key=>event('SettingSet',{key,value:'off'})),event('SettingSet',{key:'onboard.done',value:'1'}),
 event('MessagePosted',{thread:bub,msg:'m-bot1',role:'assistant',text:said}),
 event('MessagePosted',{thread:bub,msg:'m-bot2',role:'assistant',text:plan}),
 event('MessagePosted',{thread:bub,msg:'m-wide',role:'assistant',text:wide})];
fs.writeFileSync(path.join(home,'events.jsonl'),events.map(JSON.stringify).join('\n')+'\n');
Object.assign(process.env,{BACKPLANE_AGENTS:agents,BACKPLANE_COPY:'1',BACKPLANE_NO_UPDATE:'1',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',BACKPLANE_TELEMETRY:'off',BACKPLANE_PEERS:'',BACKPLANE_LOCK_DIR:path.join(home,'locks')});
const info=JSON.parse(fs.readFileSync(path.join(stage,'LOCAL.json')));
const service=new Service({home,binary:path.join(stage,'payload/backplane-serve'),web:path.join(stage,'payload/web'),expectedVersion:info.version});
let app,page;const start=performance.now();
(async()=>{try{
 await service.start();app=await _electron.launch({executablePath:path.join(stage,'desktop/runtime/backplane-desktop'),env:{...process.env,BACKPLANE_HOME:home,BACKPLANE_SERVICE:path.join(stage,'payload/backplane-serve'),BACKPLANE_WEB:path.join(stage,'payload/web'),BACKPLANE_SERVICE_VERSION:info.version},args:[]});
 page=await app.firstWindow();page.setDefaultTimeout(15000);
 const ready=async id=>page.waitForFunction(id=>document.querySelector('main')?.dataset.thread===id&&document.documentElement.dataset.backplaneRemoteReady==='true',id);
 const chat=async()=>{await page.locator(`[data-on-click="select"][data-value="${bub}"]`).first().click();await ready(bub);};
 await page.waitForFunction(()=>document.documentElement.dataset.backplaneRemoteReady==='true');await chat();
 // the header: the thread linked most, its title, +1 for the other
 const head=page.locator('[data-id="m-bot1"] .msg-ctx a.ref.ctx');
 assert.equal(await head.innerText(),title);
 assert.equal(await page.locator('[data-id="m-bot1"] .msg-ctx .more').innerText(),'+1');
 assert.equal(await page.locator('[data-id="m-bot2"] .msg-ctx').count(),0);
 await page.screenshot({path:path.join(out,'header.png')});
 await head.click();await ready(boost);await chat();
 // Reply: the chip over the composer, then the sent message's line
 // the plan is paged, and keeps its own Reply and a whole-message Copy
 await page.locator('[data-id="m-bot2"] .body-window').first().waitFor();
 assert.equal(await page.locator('[data-id="m-bot2"] > button.reply').count(),1,'Reply sits on the paged message');
 assert.equal(await page.getByText('Copy displayed part').count(),0);
 const copyBtn=page.locator('[data-id="m-bot2"] > button.copy');
 assert.equal((await copyBtn.innerText()).trim(),'Copy');
 await app.evaluate(({clipboard})=>clipboard.writeText(''));
 await page.locator('[data-id="m-bot2"]').hover();await copyBtn.click();
 await page.waitForFunction(()=>true);
 let copied='';for(let i=0;i<50&&copied!==plan;i++){copied=await app.evaluate(({clipboard})=>clipboard.readText());if(copied!==plan)await new Promise(r=>setTimeout(r,100));}
 assert.equal(copied,plan,'Copy gives the whole message, not the pages on screen');
 await page.locator('[data-id="m-bot2"]').hover();
 await page.locator('[data-id="m-bot2"] button.reply').click();
 const chip=page.locator('.reply-chip');
 await chip.waitFor();assert.equal(await chip.locator('a.rto').innerText(),'Plan for the inductor swap');
 await page.screenshot({path:path.join(out,'replying.png')});
 await page.locator('#composer').fill('yes, go ahead');await page.locator('#composer').press('Enter');
 const mine=page.locator('.msg.user',{hasText:'yes, go ahead'}).last();
 await mine.waitFor();
 await page.waitForFunction(()=>!document.querySelector('.reply-chip'));
 const line=mine.locator('.msg-reply');
 assert.equal(await line.locator('a.rto').innerText(),'Plan for the inductor swap');
 assert.match(await line.innerText(),/in reply to/i);
 const shown=await mine.innerText();
 assert(!shown.includes('Second line'),shown);assert(!shown.includes('in_reply_to'),shown);
 // the log keeps the quote for the agent
 const log=fs.readFileSync(path.join(home,'events.jsonl'),'utf8').split('\n').filter(Boolean).map(JSON.parse);
 const sent=log.find(e=>e.$==='MessagePosted'&&e.thread===bub&&e.role==='user');
 assert(sent,'no person message in the log');
 assert(sent.text.startsWith('<in_reply_to msg=m-bot2 chars='),sent.text);
 assert(sent.text.includes('Second line only the agent should read'),sent.text);
 assert(sent.text.includes('\n</in_reply_to>\n\nyes, go ahead'),sent.text);
 await page.screenshot({path:path.join(out,'replied.png')});
 // the line brings the original into view, flashed
 await line.locator('a.rto').click();
 await page.waitForFunction(()=>document.querySelector('[data-id="m-bot2"]')?.closest('.jumped,[class*="jumped"]')||document.querySelector('[data-id="m-bot2"].jumped'));
 // A phone (430 x 932, an iPhone's CSS width): nothing scrolls the page or
 // the timeline sideways, with the long code on screen and the reply bar
 // open, anywhere up the thread. It came back once after a fix (#567).
 await page.setViewportSize({width:430,height:932});
 await page.waitForFunction(()=>matchMedia('(max-width: 760px)').matches&&innerWidth<=440);
 // (the chat stays open; a phone hides the sidebar while a thread is)
 await ready(bub);
 // (the jump above left the thread on the plan: back to its end, where the wide message is)
 for(let i=0;i<20&&!(await page.locator('[data-id="m-wide"] button.reply').count());i++){await page.evaluate(()=>{const t=document.getElementById('timeline');t.scrollTop=t.scrollHeight;});await page.waitForTimeout(200);}
 await page.locator('[data-id="m-wide"] button.reply').click();
 await page.locator('.reply-chip').waitFor();
 await page.locator('#composer').fill('Reply is back');
 const sideways=()=>page.evaluate(async()=>{
  const d=document.scrollingElement,t=document.getElementById('timeline');
  // a sideways scroll from anywhere (a swipe, iOS bringing a field into view) does not stick
  t.scrollLeft=200;t.dispatchEvent(new Event('scroll'));await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));
  return {page:d.scrollWidth-d.clientWidth,timeline:t.scrollWidth-t.clientWidth,left:t.scrollLeft,overflowX:getComputedStyle(t).overflowX};
 });
 for(let step=0;step<12;step++){
  const s=await sideways();
  assert(s.page<=0,`the page is ${s.page}px wider than a phone (step ${step})`);
  assert.equal(s.overflowX,'hidden','the timeline cannot scroll sideways on a phone');
  assert(s.timeline<=1,`the thread's content is ${s.timeline}px wider than the phone: it must wrap, not be cut (step ${step})`);
  assert.equal(s.left,0,`the timeline stayed scrolled ${s.left}px sideways (step ${step})`);
  if(step===0)await page.screenshot({path:path.join(out,'phone-replying.png')});
  const top=await page.evaluate(()=>{const t=document.getElementById('timeline');t.scrollTop=Math.max(0,t.scrollTop-t.clientHeight*0.8);return t.scrollTop;});
  await page.waitForTimeout(150);if(top===0)break;
 }
 const result={state:'passed',seconds:(performance.now()-start)/1000,out,version:info.version,commit:info.commit};
 fs.writeFileSync(path.join(out,'RESULT.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result));
}catch(e){console.error(e);process.exitCode=1;if(page){fs.writeFileSync(path.join(out,'failure.html'),await page.content().catch(()=>''));await page.screenshot({path:path.join(out,'failure.png')}).catch(()=>{});}}
finally{if(app)await app.close();service.stop();}})();
