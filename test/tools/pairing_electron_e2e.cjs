// Node is required by Playwright's Electron launcher. Secrets arrive on stdin,
// never command arguments, and the app/home/service are all isolated fixtures.
const fs=require('node:fs'),path=require('node:path');
const {_electron}=require(process.env.PLAYWRIGHT_MODULE||'playwright-core');
const {hello}=require('../../desktop/service.cjs');
const input=JSON.parse(fs.readFileSync(0,'utf8'));
const {origin,token,root,fake,web,binary}=input;
let app,pid;const home=path.join(root,'desktop');fs.mkdirSync(home);
const check=(ok,label)=>{if(!ok)throw Error(label);console.log('ok '+label)};
async function api(p,method='GET',headers={}){const r=await fetch(origin+p,{method,headers:{Authorization:'Bearer '+token,...headers},redirect:'error'});if(!r.ok)throw Error('fixture API refused request');return r.json()}
(async()=>{try{
 const version=hello(Buffer.from(await(await fetch(origin+'/hello')).arrayBuffer())).backplane;
 const env={...process.env,HOME:root,BACKPLANE_HOME:home,BACKPLANE_SERVICE:path.resolve(binary),BACKPLANE_WEB:path.resolve(web),BACKPLANE_SERVICE_VERSION:version,BACKPLANE_AGENTS:fake,BACKPLANE_NO_UPDATE:'1',BACKPLANE_KISTACK_SYNC:'0',BACKPLANE_TELEMETRY:'off',BACKPLANE_DESKTOP_BROWSER:'0',BACKPLANE_PEERS:''};delete env.ELECTRON_RUN_AS_NODE;
 app=await _electron.launch({executablePath:process.env.ELECTRON_BINARY,args:[path.resolve('desktop'),'--no-sandbox','--ozone-platform=x11'],env,timeout:60000});
 console.log("ok Electron test app launched");
 const page=await app.firstWindow({timeout:30000});page.setDefaultTimeout(20000);console.log("ok Electron test window created");await page.waitForURL(/^http:/,{timeout:60000});
 console.log('ok Electron local service loaded');
 pid=Number(fs.readFileSync(path.join(home,'hub.lock'),'utf8').split(' ')[0]);
 const local=fs.readFileSync(path.join(home,'token'),'utf8'),invite=await api('/auth/invitations','POST');
 await Promise.all([page.waitForURL(origin+'/'),page.evaluate(async link=>{const r=await window.backplaneDesktop.pair(link,'Fixture desktop');if(!r.ok)throw Error('pairing exchange rejected')},invite.link).catch(e=>{if(!e.message.includes('Execution context was destroyed'))throw e})]);
 check(true,'actual Electron main pairs and selects remote hub');check(fs.readFileSync(path.join(home,'token'),'utf8')===local,'desktop preserves its local hub credential');
 const device=(await api('/auth/devices')).devices.find(d=>d.name==='Fixture desktop');check(!!device,'Electron has its own device session');
 await api('/auth/revoke','POST',{'X-Backplane-Device':device.id});await page.getByRole('heading',{name:'Re-pair this device',exact:true}).waitFor();check(true,'actual Electron automatically shows full-screen re-pair without reload');
}finally{if(!pid&&fs.existsSync(path.join(home,'hub.lock')))pid=Number(fs.readFileSync(path.join(home,'hub.lock'),'utf8').split(' ')[0]);if(pid){try{process.kill(pid,15)}catch{}}await app?.close();}})().catch(e=>{console.error(String(e.message).replace(/[a-f0-9]{32}/g,'[redacted]').slice(0,1800));process.exitCode=1});
