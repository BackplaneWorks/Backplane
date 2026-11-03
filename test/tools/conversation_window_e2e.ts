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
const viewerPressure=process.env.E2E_VIEWER==='1',modelPath=join(root,'project','pressure.glb');
if(viewerPressure){
 const positions=Buffer.alloc(36);[-1,0,0,1,0,0,0,1,0].forEach((v,i)=>positions.writeFloatLE(v,i*4));
 const scene={asset:{version:'2.0'},scene:0,scenes:[{nodes:[0]}],nodes:[{mesh:0}],meshes:[{primitives:[{attributes:{POSITION:0},material:0}]}],materials:[{pbrMetallicRoughness:{baseColorFactor:[.8,.2,.1,1],metallicFactor:0,roughnessFactor:.5}}],buffers:[{byteLength:36}],bufferViews:[{buffer:0,byteLength:36}],accessors:[{bufferView:0,componentType:5126,count:3,type:'VEC3',min:[-1,0,0],max:[1,1,0]}]};
 const json=Buffer.from(JSON.stringify(scene)),pad=Buffer.alloc(Math.ceil(json.length/4)*4,32);json.copy(pad);
 const glb=Buffer.alloc(28+pad.length+positions.length);glb.writeUInt32LE(0x46546c67,0);glb.writeUInt32LE(2,4);glb.writeUInt32LE(glb.length,8);glb.writeUInt32LE(pad.length,12);glb.writeUInt32LE(0x4e4f534a,16);pad.copy(glb,20);glb.writeUInt32LE(positions.length,20+pad.length);glb.writeUInt32LE(0x004e4942,24+pad.length);positions.copy(glb,28+pad.length);writeFileSync(modelPath,glb);
}
const events:any[]=[event('ProjectCreated',{id:'p',title:'Window regression',root:join(root,'project')}),thread('real','Copied Bub history'),thread('triple','Triple bot history'),thread('other','Navigation control'),event('BotSet',{id:'b-real',name:'Bub copy',thread:'real',look:0,persona:''}),event('BotSet',{id:'b-triple',name:'Triple bot',thread:'triple',look:1,persona:''}),event('UberSet',{bot:'b-real',uid:'fixture-bub'}),event('SettingSet',{key:'machine.id',value:'fixture'})];
for(const [t,copies] of [['real',1],['triple',3]] as const)for(let k=0;k<copies;k++)for(let i=0;i<real.length;i++){const j=real[i];events.push({...j,thread:t,...(j.$==='MessagePosted'?{msg:`${t}-${k}-${i}`}:{act:`${t}-${k}-${i}`})});}
// A single pathological Markdown body must also leave real content visible.
events.push(event('MessagePosted',{thread:'triple',msg:'huge',role:'assistant',text:'Oversized Unicode entry\n'+('😀\u0001\\\"\n- **a** [b](https://example.com)\n'.repeat(30000))}));
events.push(thread('tools','Tool-prefix coverage'),event('MessagePosted',{thread:'tools',msg:'tools-person',role:'user',text:'SOURCE BEFORE 51 TOOLS'}),event('MessagePosted',{thread:'tools',msg:'tools-assistant',role:'assistant',text:'ASSISTANT BEFORE 51 TOOLS'}));
for(let i=0;i<51;i++)events.push(event('ActivityLogged',{thread:'tools',act:`tool-${i}`,tone:'tool',kind:'shell',summary:`COLLAPSED TOOL DETAIL ${i}`}));
events.push(event('MessagePosted',{thread:'other',msg:'control',role:'assistant',text:'Navigation is working'}));
for(const [key,value]of [['restart.continue','off'],['onboard.done','1'],['onboard.uber','1']])events.push(event('SettingSet',{key,value}));
const projectCount=Number(process.env.E2E_PROJECTS||12);
for(let i=0;i<projectCount;i++){events.push(event('ProjectCreated',{id:`pressure-${i}`,title:`Live project ${i} `+'long title '.repeat(8),root:join(root,`pressure-${i}`)}),event('ThreadCreated',{id:`pressure-thread-${i}`,title:`Live thread ${i}`,project:`pressure-${i}`,env:'local',branch:'',worktree:'',provider:'claude',model:''}));}
if(projectCount)events.push(event('MessagePosted',{thread:'pressure-thread-0',msg:'sidebar-control',role:'assistant',text:'Sidebar navigation is working'}));
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
 const{chromium}=await import(pw);browser=await chromium.launch({headless:true,...(process.env.E2E_CHROME?{executablePath:process.env.E2E_CHROME}:{}),...(viewerPressure?{args:['--use-gl=angle','--use-angle=swiftshader','--enable-unsafe-swiftshader']}:{})});
 const page=await browser.newPage({viewport:{width:1400,height:950}});
 page.on('websocket',(s:any)=>{
  s.on('framesent',(e:any)=>{try{requests.push(typeof e.payload==='string'?JSON.parse(e.payload):plain(cbor(new Uint8Array(e.payload),keys,words)));}catch{}});
  s.on('framereceived',(e:any)=>{try{const j=typeof e.payload==='string'?JSON.parse(e.payload):plain(cbor(new Uint8Array(e.payload),keys,words));frames.push(j);if(j.t==='reply')replies.push(j);}catch{}});
 });
 page.on('pageerror',(e:any)=>console.log('pageerror',e.message));
 await page.addInitScript(({viewerPressure,modelPath})=>{
  localStorage.setItem('backplane-selected','real');
  if(viewerPressure)for(const thread of ['real','triple'])localStorage.setItem('backplane-viewer:'+thread,JSON.stringify({open:'1',thread,kind:'3d',path:modelPath}));
 },{viewerPressure,modelPath});
 await page.goto(`http://127.0.0.1:${port}/#token=${token}`);
 await page.locator('#composer').waitFor({timeout:60000});
 const checkSidebar=async()=>{
  for(let i=0;i<projectCount;i++)if(!await page.locator(`.sidebar [data-value="pressure-${i}"]`).count())throw Error(`open project ${i} disappeared from sidebar`);
 };
 await checkSidebar();
 if(viewerPressure){
  await page.locator('#viewer #solid-workbench canvas#solid').waitFor({timeout:30000});
  const initial=await page.evaluate(()=>{const canvas=document.querySelector('#solid') as HTMLCanvasElement,gl=canvas.getContext('webgl');(window as any).__pressureViewer={canvas,gl,viewer:document.querySelector('#viewer'),workbench:document.querySelector('#solid-workbench'),key:canvas.dataset.key,scope:canvas.dataset.scope};return{context:!!gl,key:canvas.dataset.key,scope:canvas.dataset.scope};});
  if(!initial.context)throw Error('pressure fixture did not create a WebGL context');console.log('initial requested viewer',JSON.stringify(initial));
 }
 let pressureChecked=false;
 for(const title of ['Copied Bub history','Triple bot history','Tool-prefix coverage','Navigation control','Copied Bub history']){
  await page.getByText('Jump or run a command').first().click();await page.locator('#workspace-query').fill(title);
  await page.locator('.workspace-command').filter({hasText:title}).first().click({timeout:15000});
  await page.locator('.workspace-palette-backdrop').waitFor({state:'hidden',timeout:15000});
  if(title==='Tool-prefix coverage')await page.getByText('ASSISTANT BEFORE 51 TOOLS',{exact:false}).waitFor({timeout:30000});
  await page.waitForFunction(()=>document.querySelector('#timeline')?.textContent?.trim().length!>0,undefined,{timeout:30000});
  if(!viewerPressure)await page.keyboard.press('Escape');
  await delay(500);
  const state=await page.evaluate(()=>({content:document.querySelector('#timeline')?.textContent?.trim().length||0,rows:document.querySelectorAll('#timeline [data-id]').length,status:document.querySelector('#view-connection')?.textContent,bar:!!document.querySelector('#view-connection:not([hidden])')&&(document.querySelector('#view-connection')as HTMLElement).style.display!=='none'}));
  if(viewerPressure&&!pressureChecked){
   const preserved=await page.evaluate(()=>{const p=(window as any).__pressureViewer,c=document.querySelector('#solid') as HTMLCanvasElement;return{viewer:document.querySelector('#viewer')===p.viewer,workbench:document.querySelector('#solid-workbench')===p.workbench,canvas:c===p.canvas,context:c?.getContext('webgl')===p.gl,key:c?.dataset.key===p.key,scope:c?.dataset.scope===p.scope};});
   if(Object.values(preserved).some(v=>v!==true))throw Error('pressure remounted requested viewer: '+JSON.stringify(preserved));console.log('PASS requested 3D DOM/WebGL identity under pressure',JSON.stringify(preserved));pressureChecked=true;
  }
  console.log(JSON.stringify({title,...state}));if(!state.content||!state.rows||state.bar)throw Error(`conversation did not paint: ${JSON.stringify(state)}`);
  await checkSidebar();
  if(title==='Triple bot history'){
   const before=requests.filter(r=>r.m==='view.page').length;await page.locator('#timeline').evaluate((el:any)=>el.scrollTop=0);await page.locator('#timeline').hover();await page.mouse.wheel(0,-2000);await delay(2000);
   if(requests.filter(r=>r.m==='view.page').length===before)throw Error('older history navigation issued no page');
  }
 }
 if(projectCount){
  const project=page.locator('.sidebar .project').filter({has:page.locator('[data-value="pressure-0"]')}).first();
  // Expand through the project's normal control, then follow its thread row.
  if(!await page.locator('.sidebar .thread[data-value="pressure-thread-0"]').isVisible())await project.locator('[data-on-click="toggle-settled"][data-value="pressure-0"]').click();
  await page.locator('.sidebar .thread[data-value="pressure-thread-0"]').click();
  await page.getByText('Sidebar navigation is working',{exact:true}).waitFor({timeout:30000});
  await checkSidebar();
  console.log('PASS open projects retained and another thread reachable directly from sidebar');
 }
 await page.keyboard.press('Control+Shift+U');await delay(2000);
 if(!requests.some(r=>r.m==='view.open'&&r.p.target.kind==='bot'))throw Error('bot navigation did not open a bot target');
 if(!(await page.locator('#timeline').textContent())?.trim())throw Error('bot route did not paint');
 if(replies.some(r=>r.error?.code==='LIMIT'))throw Error('presentation returned LIMIT');
 await page.screenshot({path:join(root,'painted.png')});
 console.log('PASS copied real history, 3x bot history, oversized Unicode, 51-tool prefix, paging and navigation; no status bar');
}catch(error){console.log('refusals',JSON.stringify(replies.filter(r=>r.ok===false)));writeFileSync(join(root,'frames.json'),JSON.stringify(frames));writeFileSync(join(root,'requests.json'),JSON.stringify(requests));throw error;}finally{await browser?.close();hub.kill('SIGTERM');await hub.exited;console.log('artifacts',root);}
