// Real browser + isolated hub. Only dummy secrets; never a user's home/vault.
const fs=require('node:fs'),os=require('node:os'),path=require('node:path'),assert=require('node:assert/strict');
const {chromium}=require(require.resolve('playwright-core',{paths:[process.cwd(),path.resolve('tools/browser')]}));
const [binary,web]=process.argv.slice(2).map(p=>path.resolve(p));
(async()=>{
const port=await new Promise(resolve=>{const server=require('node:net').createServer();server.listen(0,'127.0.0.1',()=>{const port=server.address().port;server.close(()=>resolve(port));});});
const home=fs.mkdtempSync(path.join(os.tmpdir(),'bp-secret-box-'));
const project=path.join(home,'project'),agents=path.join(home,'agents');fs.mkdirSync(project);fs.mkdirSync(agents);
for(const n of ['claude','codex','grok','muse','opencode','agy'])fs.writeFileSync(path.join(agents,n),'#!/bin/sh\nexit 0\n',{mode:0o755});
const thread='t-secret-bub',at=Math.floor(Date.now()/1000),event=($,p)=>({$,...p,at});
fs.writeFileSync(path.join(home,'events.jsonl'),[
 event('ProjectCreated',{id:'fixture',title:'Secret fixture',root:project}),
 event('ThreadCreated',{id:thread,title:'Bub Chat',project:'fixture',env:'local',branch:'',worktree:project,provider:'codex',model:'gpt-6.1-sol'}),
 event('BotSet',{id:'bub',name:'Bub',thread,look:0,persona:''}),
 ...['restart.continue','mail.on','telemetry.share'].map(key=>event('SettingSet',{key,value:'off'})),
 event('SettingSet',{key:'onboard.done',value:'1'}),
 event('SettingSet',{key:'onboard.uber',value:'1'}),
 event('SettingSet',{key:'telemetry.consent',value:'no'}),
 event('SettingSet',{key:'account.thread.'+thread+'.codex',value:JSON.stringify({id:'fixture-account',epoch:1})})
].map(JSON.stringify).join('\n')+'\n');
const {spawn}=require('node:child_process');
const proc=spawn(binary,['--home',home,'--web',web,'--port',String(port),'--no-tailscale','--foreground'],{
 env:{...process.env,HOME:home,BACKPLANE_HOME:home,BACKPLANE_AGENTS:agents,BACKPLANE_COPY:'1',BACKPLANE_NO_UPDATE:'1',BACKPLANE_TELEMETRY:'off',BACKPLANE_GOOGLE_BAKE:'0',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',BACKPLANE_PEERS:'',BACKPLANE_LOCK_DIR:path.join(home,'locks'),BACKPLANE_VAULT_KEYRING:'file',...(process.env.SECRET_BOX_FAULT?{BACKPLANE_VAULT_TEST:'1',BACKPLANE_VAULT_FAULT:process.env.SECRET_BOX_FAULT}:{})},stdio:['ignore','pipe','pipe']});
let logs='',browser;proc.stdout.on('data',b=>logs+=b);proc.stderr.on('data',b=>logs+=b);
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
await (async()=>{try{
 for(let i=0;i<200;i++){try{if((await fetch(`http://127.0.0.1:${port}/hello`)).ok)break;}catch{} await sleep(100);}
 browser=await chromium.launch({headless:true,executablePath:process.env.BACKPLANE_CHROME||undefined,args:['--no-sandbox']});
 const context=await browser.newContext();
 const token=fs.readFileSync(path.join(home,'token'),'utf8').trim();
 const mcp=async(name,args)=>{
  const response=await fetch(`http://127.0.0.1:${port}/mcp/${thread}`,{method:'POST',headers:{'Content-Type':'application/json',Authorization:'Bearer '+token},body:JSON.stringify({jsonrpc:'2.0',id:1,method:'tools/call',params:{name,arguments:args}})});
  const result=await response.json();assert(!result.error && !result.result?.isError,'isolated MCP request accepted');
  return JSON.parse(result.result.content[0].text);
 };
 const request=await mcp('secret_request',{name:'fixture-bub',purpose:'dummy bot request'});
 assert.equal(request.requested,true,'real secret_request opens the bot box');
 await context.addCookies([{name:'bp_token',value:token,url:`http://127.0.0.1:${port}`}]);
 const page=await context.newPage();
 // Bounded source-adapter mode reuses the installed Bend runtime and hub.
 // Only the new JS transport is loaded from source; this is not a full build.
 if(process.env.SECRET_BOX_SOURCE_ADAPTER){
  await page.addInitScript(()=>{const Original=window.WebSocket;window.WebSocket=class extends Original{constructor(...args){super(...args);this.addEventListener('message',event=>window.__secretReply?.(event));}};});
  const source=fs.readFileSync(path.resolve('src/web/secret-save.js'),'utf8').replace(/export /g,'');
  await page.route('**/chunk-*.js',async route=>{
   const response=await route.fetch();let body=await response.text();
   const match=body.match(/function (\w+)\((\w+)\)\{let (\w+);try\{\3=JSON\.parse\((\w+)\.secret_req\(\2\)\)\}catch\{return!0\}/);
   assert(match,'installed secure Save adapter marker');
   const start=body.indexOf(match[0]),end=body.indexOf('"secret-sent",""',start);
   const original=body.slice(start,end+30),socket=original.match(/if\(!([\w$]+)\|\|/)[1];
   const dispatch=original.match(/([\w$]+)\("secret-sent"/)[1];
   const list=original.match(/\.send\(([\w$]+)\(/)[1];
   const [fn,app]=[match[1],match[4]],binding=JSON.stringify(JSON.stringify({id:'fixture-account',epoch:process.env.SECRET_BOX_STALE?0:1}));
   body+=`;(()=>{${source};const controller=new SecretSaveHost({build:p=>${app}.secret_req(p),scope:()=>"bot:bub",binding:()=>${binding},send:rpc=>{if(!${socket}||${socket}.readyState!==1)throw Error("Disconnected");${socket}.send(${list}(${app}.wire_out(JSON.stringify(rpc))));},dispatch:${dispatch},note:text=>{const n=document.querySelector('.sbox-note');if(n)n.textContent=text;}});${fn}=packed=>controller.save(packed);window.__secretReply=event=>{if(typeof event.data==='string')return;const template=${app}.wire_out('{}');let end=template;while(end.tail)end=end.tail;let xs={$:end.$};const bytes=new Uint8Array(event.data);for(let i=bytes.length-1;i>=0;i--)xs={$:template.$,head:bytes[i],tail:xs};try{controller.receive(JSON.parse(${app}.show(${app}.wire_in(xs))));}catch{}};setInterval(()=>{controller.expire();controller.paint();},250);})();`;
   await route.fulfill({response,body,headers:{...response.headers(),'content-encoding':'identity'}});
  });
 }
 const errors=[];page.on('pageerror',e=>errors.push(e.name+': '+e.message));
 await page.goto(`http://127.0.0.1:${port}/#thread=${thread}`);
 await page.waitForFunction(()=>document.documentElement.dataset.backplaneRemoteReady==='true',null,{timeout:20000});
 const skip=page.locator('[data-on-click="ob-skip"]');if(await skip.count())await skip.first().click();
 const select=page.locator(`[data-on-click="bot"][data-value="bub"]`).first();if(await select.count())await select.click();
 await page.locator('#sbox-val').waitFor({timeout:15000});
 await page.locator('#sbox-val').fill('DUMMY-SECRET-BOX-123456');
 await page.locator('[data-on-click="secret-save"]').click();
 await sleep(1500);
 const state={boxes:await page.locator('#sbox-val').count(),note:await page.locator('.sbox-note').allTextContents(),hasVault:fs.existsSync(path.join(home,'secrets/vault.json')),errors};
 console.log(JSON.stringify(state));
 if(process.env.SECRET_BOX_EXPECT==='broken'){assert.equal(state.hasVault,false);assert.equal(state.boxes,1);return;}
 if(process.env.SECRET_BOX_STALE){assert.equal(state.hasVault,false);assert.equal(state.boxes,1);assert.match(state.note.join(' '),/named account/);console.log('ok stale account rejection is inline');return;}
 if(process.env.SECRET_BOX_FAULT){assert.equal(state.hasVault,false);assert.equal(state.boxes,1);assert.match(state.note.join(' '),/injected rename failure/);assert(!logs.includes('DUMMY-SECRET-BOX-123456'));console.log('ok failed vault write is an inline error, box stays open');return;}
 assert.equal(state.hasVault,true,'Save must reach the vault');
 const listed=await mcp('secret_list',{});assert(listed.some(item=>item.secret==='fixture-bub' && item.set===true));
 assert.equal(state.boxes,0,'acknowledged Save closes the bot request');
 const vault=fs.readFileSync(path.join(home,'secrets/vault.json'),'utf8');assert(vault.includes('fixture-bub'));assert(!vault.includes('DUMMY-SECRET-BOX-123456'));
 assert(!(await page.locator('main').innerText()).includes('DUMMY-SECRET-BOX-123456'));
 assert(!logs.includes('DUMMY-SECRET-BOX-123456'));assert(!fs.readFileSync(path.join(home,'events.jsonl'),'utf8').includes('DUMMY-SECRET-BOX-123456'));
 console.log('ok bot request saved, sealed and closed; no value in journal or chat');
}catch(e){console.error(e);process.exitCode=1;}finally{if(browser)await browser.close();if(proc.exitCode===null && proc.signalCode===null){const exited=new Promise(r=>proc.once('exit',r));proc.kill('SIGKILL');await exited;}fs.rmSync(home,{recursive:true,force:true});}})();

})().catch(error=>{console.error(error);process.exitCode=1;});
