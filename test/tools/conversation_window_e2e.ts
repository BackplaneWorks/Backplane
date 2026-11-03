// Real browser and isolated hub. Optional fixture is an exported, read-only copy
// of a bot's history (never point the test hub at the user's home).
// bun test/tools/conversation_window_e2e.ts BINARY WEB [COPIED_HISTORY.jsonl]
import {existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync} from 'node:fs';
import {tmpdir,homedir} from 'node:os';
import {cbor,plain} from '../../src/web/wire.js';
import {join,resolve} from 'node:path';
const [binary,web,fixture]=process.argv.slice(2).map(x=>resolve(x));
const root=mkdtempSync(join(tmpdir(),'bp-conversation-window-')), home=join(root,'home'), fake=join(root,'agents');
for(const p of [home,fake,join(root,'locks'),join(root,'project')])mkdirSync(p,{recursive:true});
const token='5f0c2e7d948ab62350cfa19e6d834b10', port=Number(process.env.E2E_PORT||3947),at=Math.floor(Date.now()/1000)-7200;
if(port===3787)throw Error('live port forbidden');
const event=($:string,fields:object)=>({$,...fields,at});
const thread=(id:string,title:string)=>event('ThreadCreated',{id,title,project:'p',env:'local',branch:'',worktree:'',provider:'claude',model:''});
const real=fixture?readFileSync(fixture,'utf8').trim().split('\n').map(x=>JSON.parse(x)).filter(j=>['MessagePosted','ActivityLogged'].includes(j.$)):Array.from({length:18000},(_,i)=>event('MessagePosted',{thread:'real',msg:`m${i}`,role:i%2?'assistant':'user',text:`Post ${i}\n\n${'## Mailroom\n- message 日本語 😀\n'.repeat(15)}`}));
const events:any[]=[event('ProjectCreated',{id:'p',title:'Window regression',root:join(root,'project')}),thread('real','Copied Bub history'),thread('triple','Triple bot history'),thread('other','Navigation control'),event('BotSet',{id:'b-real',name:'Bub copy',thread:'real',look:0,persona:''}),event('BotSet',{id:'b-triple',name:'Triple bot',thread:'triple',look:1,persona:''}),event('UberSet',{bot:'b-real',uid:'fixture-bub'}),event('SettingSet',{key:'machine.id',value:'fixture'})];
for(const [t,copies] of [['real',1],['triple',3]] as const)for(let k=0;k<copies;k++)for(let i=0;i<real.length;i++){const j=real[i];events.push({...j,thread:t,...(j.$==='MessagePosted'?{msg:`${t}-${k}-${i}`}:{act:`${t}-${k}-${i}`})});}
// A single pathological Markdown body must also leave real content visible.
events.push(event('MessagePosted',{thread:'triple',msg:'huge',role:'assistant',text:'Oversized Unicode entry\n'+('😀\u0001\\\"\n- **a** [b](https://example.com)\n'.repeat(30000))}));
events.push(event('MessagePosted',{thread:'other',msg:'control',role:'assistant',text:'Navigation is working'}));
for(const [key,value]of [['restart.continue','off'],['onboard.done','1'],['onboard.uber','1']])events.push(event('SettingSet',{key,value}));
for(let i=0;i<Number(process.env.E2E_PROJECTS||0);i++){events.push(event('ProjectCreated',{id:`pressure-${i}`,title:`Live project ${i} `+'long title '.repeat(8),root:join(root,`pressure-${i}`)}),event('ThreadCreated',{id:`pressure-thread-${i}`,title:`Live thread ${i}`,project:`pressure-${i}`,env:'local',branch:'',worktree:'',provider:'claude',model:''}));}
writeFileSync(join(home,'events.jsonl'),events.map(x=>JSON.stringify(x)).join('\n')+'\n');writeFileSync(join(home,'token'),token,{mode:0o600});
console.log(JSON.stringify({root,realEntries:real.length,tripleEntries:3*real.length+1}));
const hub=Bun.spawn([binary,'--home',home,'--port',String(port),'--host','127.0.0.1','--no-tailscale','--headless','--foreground','--web',web],{env:{...process.env,BACKPLANE_AGENTS:fake,BACKPLANE_LOCK_DIR:join(root,'locks'),BACKPLANE_NO_UPDATE:'1',BACKPLANE_KISTACK_SYNC:'0',BACKPLANE_TELEMETRY:'off',BACKPLANE_PEERS:'',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9'},stdout:Bun.file(join(root,'hub.out')),stderr:Bun.file(join(root,'hub.err'))});
const delay=(ms:number)=>new Promise(r=>setTimeout(r,ms));
const pw=[join(resolve(import.meta.dir,'../..'),'tools/browser/node_modules/playwright-core'),join(homedir(),'Documents/i2cjak/backplane-bend/tools/browser/node_modules/playwright-core')].find(existsSync)!;
const dictionary=(name:string)=>{const src=readFileSync(resolve(import.meta.dir,'../../src/core/cbor.bend'),'utf8'),at=src.indexOf(`def Cbor.${name}()`);return JSON.parse(src.slice(src.indexOf('[',at),src.indexOf(']',at)+1));};
const keys=dictionary('keys'),words=dictionary('words');
let browser:any;const replies:any[]=[],requests:any[]=[],frames:any[]=[];
try{
 let up=false;for(let i=0;i<600&&!up;i++){try{up=(await fetch(`http://127.0.0.1:${port}/hello`)).ok;}catch{}if(!up)await delay(100);}if(!up)throw Error('hub did not start');
 const{chromium}=await import(pw);browser=await chromium.launch({headless:true,...(process.env.E2E_CHROME?{executablePath:process.env.E2E_CHROME}:{})});
 const page=await browser.newPage({viewport:{width:1400,height:950}});
 page.on('websocket',(s:any)=>{
  s.on('framesent',(e:any)=>{try{requests.push(typeof e.payload==='string'?JSON.parse(e.payload):plain(cbor(new Uint8Array(e.payload),keys,words)));}catch{}});
  s.on('framereceived',(e:any)=>{try{const j=typeof e.payload==='string'?JSON.parse(e.payload):plain(cbor(new Uint8Array(e.payload),keys,words));frames.push(j);if(j.t==='reply')replies.push(j);}catch{}});
 });
 page.on('pageerror',(e:any)=>console.log('pageerror',e.message));
 await page.addInitScript(()=>localStorage.setItem('backplane-selected','real'));
 await page.goto(`http://127.0.0.1:${port}/#token=${token}`);
 await page.locator('#composer').waitFor({timeout:60000});
 for(const title of ['Copied Bub history','Triple bot history','Navigation control','Copied Bub history']){
  await page.getByText('Jump or run a command').first().click();await page.locator('#workspace-query').fill(title);
  await page.locator('.workspace-command').filter({hasText:title}).first().click({timeout:15000});
  await page.locator('.workspace-palette-backdrop').waitFor({state:'hidden',timeout:15000});
  await page.waitForFunction(()=>document.querySelector('#timeline')?.textContent?.trim().length!>0,undefined,{timeout:30000});
  await page.keyboard.press('Escape');
  await delay(500);
  const state=await page.evaluate(()=>({content:document.querySelector('#timeline')?.textContent?.trim().length||0,rows:document.querySelectorAll('#timeline [data-id]').length,status:document.querySelector('#view-connection')?.textContent,bar:!!document.querySelector('#view-connection:not([hidden])')&&(document.querySelector('#view-connection')as HTMLElement).style.display!=='none'}));
  console.log(JSON.stringify({title,...state}));if(!state.content||!state.rows||state.bar)throw Error(`conversation did not paint: ${JSON.stringify(state)}`);
  if(title==='Triple bot history'){
   const before=requests.filter(r=>r.m==='view.page').length;await page.locator('#timeline').evaluate((el:any)=>el.scrollTop=0);await page.locator('#timeline').hover();await page.mouse.wheel(0,-2000);await delay(2000);
   if(requests.filter(r=>r.m==='view.page').length===before)throw Error('older history navigation issued no page');
  }
 }
 await page.keyboard.press('Control+Shift+U');await delay(2000);
 if(!requests.some(r=>r.m==='view.open'&&r.p.target.kind==='bot'))throw Error('bot navigation did not open a bot target');
 if(!(await page.locator('#timeline').textContent())?.trim())throw Error('bot route did not paint');
 if(replies.some(r=>r.error?.code==='LIMIT'))throw Error('presentation returned LIMIT');
 await page.screenshot({path:join(root,'painted.png')});
 console.log('PASS copied real history, 3x bot history, oversized Unicode, paging and navigation; no status bar');
}catch(error){console.log('refusals',JSON.stringify(replies.filter(r=>r.ok===false)));writeFileSync(join(root,'frames.json'),JSON.stringify(frames));writeFileSync(join(root,'requests.json'),JSON.stringify(requests));throw error;}finally{await browser?.close();hub.kill('SIGTERM');await hub.exited;console.log('artifacts',root);}
