// End to end: the pairing links a hub issues (effects/pairing.js, docs/pairing.md).
// A private hub records every invitation, lists it (issued, used by which
// device, revoked), pairs only with one it issued and has not revoked, and
// revoking a used one revokes its device; the records survive a restart. With
// ELECTRON_BINARY set, the desktop app (this checkout's desktop/, its own home
// and service) is given backplane:// links as the OS would hand them over:
// one for a hub its own service knows (BACKPLANE_PEERS) pairs at once, one for
// any other address asks first (the dialog is answered Cancel) and pairs with
// nothing. Hubs and stand-in servers listen on loopback only, with a stand-in
// tailscale first on PATH (no real tailnet is read or reached); every process
// is stopped by the handle that started it.
//
//   node test/tools/pairing_links_e2e.cjs BINARY WEB [--keep]
//   (ELECTRON_BINARY, a DISPLAY of your own Xvfb, PLAYWRIGHT_MODULE optional)
const fs=require('node:fs'),os=require('node:os'),path=require('node:path'),http=require('node:http'),{spawn}=require('node:child_process');
const [binary,web]=process.argv.slice(2).filter(x=>!x.startsWith('--'));
if(!binary||!web)throw Error('BINARY WEB required');
const root=fs.mkdtempSync(path.join(os.tmpdir(),'bp-pairing-links-')),home=path.join(root,'hub'),fake=path.join(root,'agents');
for(const d of [home,fake])fs.mkdirSync(d,{recursive:true,mode:0o700});
for(const cli of ['claude','codex','grok','muse','opencode','pi','omp','tailscale'])fs.writeFileSync(path.join(fake,cli),'#!/bin/sh\nexit 127\n',{mode:0o755});
const port=Number(process.env.PAIRING_LINKS_PORT||47885),origin='http://127.0.0.1:'+port;
let hub,failed=false,app,desktopPid;
const check=(ok,label,got)=>{console.log((ok?'ok ':'FAIL ')+label+(ok?'':': '+JSON.stringify(got)?.slice(0,300)));if(!ok)failed=true;};
const pause=ms=>new Promise(r=>setTimeout(r,ms));
async function waitFor(fn,ms=30000){const end=Date.now()+ms;while(Date.now()<end){try{const v=await fn();if(v)return v;}catch{}await pause(150);}return null;}
function start(){
 hub=spawn(path.resolve(binary),['--home',home,'--web',path.resolve(web),'--port',String(port),'--host','127.0.0.1','--no-tailscale'],{
  env:{...process.env,HOME:root,PATH:fake+':/usr/bin:/bin',BACKPLANE_AGENTS:fake,BACKPLANE_KISTACK_SYNC:'0',BACKPLANE_NO_UPDATE:'1',BACKPLANE_TELEMETRY:'off',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',BACKPLANE_PEERS:'',DISPLAY:'',WAYLAND_DISPLAY:''},
  stdio:['ignore',fs.openSync(path.join(root,'hub.out'),'a'),fs.openSync(path.join(root,'hub.err'),'a')]});
 return waitFor(async()=>(await fetch(origin+'/hello')).status===200,90000);
}
async function stop(){if(hub&&hub.exitCode===null){hub.kill('SIGTERM');await Promise.race([new Promise(r=>hub.once('exit',r)),pause(5000)]);if(hub.exitCode===null)hub.kill('SIGKILL');}}
let rootToken='';
async function api(p,{token=rootToken,method='GET',headers={}}={}){const r=await fetch(origin+p,{method,headers:{Authorization:'Bearer '+token,...headers},redirect:'error'});let value={};try{value=await r.json()}catch{}return{status:r.status,value};}
const secretOf=link=>new URLSearchParams(new URL(link).hash.slice(1)).get('invite');
const enroll=(secret,name)=>api('/auth/enroll',{token:secret,method:'POST',headers:{'X-Backplane-Name':name,'X-Backplane-Client':'native'}});
const listed=async id=>(await api('/auth/devices')).value.invitations?.find(x=>x.id===id);
// a server that answers anything with 200, standing in for someone else's
const strangers=[];
const stranger=http.createServer((req,res)=>{strangers.push(req.method+' '+req.url);res.writeHead(200,{'content-type':'application/json'});res.end('{"ok":true,"token":"'+'e'.repeat(32)+'"}');});
// (a stuck step must not leave processes behind: stop everything by handle)
const watchdog=setTimeout(()=>{console.log('FAIL watchdog: the run took too long');failed=true;stopAll().finally(()=>process.exit(1));},Number(process.env.PAIRING_LINKS_LIMIT_MS||480000));
async function stopAll(){
 try{await Promise.race([app?.close(),pause(5000)]);}catch{}
 // (the desktop's own service: only when its command line names this fixture's home)
 if(desktopPid){try{if(fs.readFileSync('/proc/'+desktopPid+'/cmdline','utf8').includes(root))process.kill(desktopPid,'SIGTERM')}catch{}}
 stranger.close();await stop();
}
(async()=>{try{
 check(await start(),'hub ready');
 rootToken=fs.readFileSync(path.join(home,'token'),'utf8').trim();
 const a=(await api('/auth/invitations',{method:'POST'})).value, b=(await api('/auth/invitations',{method:'POST'})).value;
 check(a.link&&b.link,'two invitations issued');
 check((await listed(a.id))?.state==='open','an issued link is listed, not used yet',await listed(a.id));
 const phone=await enroll(secretOf(a.link),'Fixture phone');check(phone.status===200&&phone.value.token,'a link the hub issued pairs',phone);
 const x=await listed(a.id);check(x?.state==='used'&&x.deviceName==='Fixture phone'&&x.device===phone.value.id&&x.usedAt>0,'listed as used, by which device, when',x);
 const forged=await enroll('f'.repeat(32),'Forged');check(forged.status===400&&forged.value.error==='wrong_code','a link the hub never issued pairs nothing',forged);
 const rb=await api('/auth/revoke',{method:'POST',headers:{'X-Backplane-Invitation':b.id}});check(rb.status===200,'an unused link revoked',rb);
 check((await enroll(secretOf(b.link),'Late')).value.error==='revoked_code','a revoked link pairs nothing');
 check((await enroll(b.code,'Late')).value.error==='revoked_code','nor its short code');
 check((await api('/auth/devices',{token:phone.value.token})).status===200,'the paired device has access');
 const ra=await api('/auth/revoke',{method:'POST',headers:{'X-Backplane-Invitation':a.id}});check(ra.status===200,'a used link revoked',ra);
 check((await api('/auth/devices',{token:phone.value.token})).status===401,'and the device it paired lost access');
 const y=await listed(a.id);check(y?.state==='revoked'&&y.deviceRevoked===true,'listed as revoked with its device',y);
 check(!(await api('/auth/devices')).value.devices.find(d=>d.id===phone.value.id&&!d.revoked),'the device shows revoked');
 const c=(await api('/auth/invitations',{method:'POST'})).value;
 await stop();check(await start(),'hub restarted');
 const z=(await api('/auth/devices')).value.invitations||[];check(z.length===3&&z.find(i=>i.id===b.id)?.state==='revoked','the records survive a restart',z.map(i=>i.state));
 check((await enroll(c.code,'After restart')).status===200,'an open link still works after a restart');
 check((await enroll(secretOf(b.link),'Late')).value.error==='revoked_code','a revoked one still pairs nothing');
 const disk=fs.readFileSync(path.join(home,'devices-v1.json'),'utf8');check(![a,b,c].some(i=>disk.includes(i.code)||disk.includes(secretOf(i.link))),'no invitation secret on disk');
 const kn=async o=>(await api('/auth/known',{method:'POST',headers:{'X-Backplane-Origin':o}})).value.known;
 check(await kn(origin)===true,'the hub knows its own address');
 check(await kn('https://pairing.example')===false,'and no stranger\'s');
 check((await fetch(origin+'/auth/known',{method:'POST',headers:{'X-Backplane-Origin':origin}})).status===401,'only its clients may ask');
 if(process.env.ELECTRON_BINARY){
  const {_electron}=require(process.env.PLAYWRIGHT_MODULE||path.resolve('desktop/node_modules/playwright-core'));
  await new Promise(r=>stranger.listen(47886,'127.0.0.1',r));
  const strangeOrigin='http://127.0.0.1:47886';
  const desk=path.join(root,'desktop');fs.mkdirSync(desk);
  const {hello}=require(path.resolve('desktop/service.cjs'));
  const version=hello(Buffer.from(await(await fetch(origin+'/hello')).arrayBuffer())).backplane;
  // (a stand-in tailscale first on PATH: the desktop's service never sees a real tailnet)
  const env={...process.env,HOME:root,PATH:fake+':'+process.env.PATH,BACKPLANE_HOME:desk,BACKPLANE_SERVICE:path.resolve(binary),BACKPLANE_WEB:path.resolve(web),BACKPLANE_SERVICE_VERSION:version,BACKPLANE_AGENTS:fake,BACKPLANE_NO_UPDATE:'1',BACKPLANE_KISTACK_SYNC:'0',BACKPLANE_TELEMETRY:'off',BACKPLANE_DESKTOP_BROWSER:'0',BACKPLANE_PEERS:origin};delete env.ELECTRON_RUN_AS_NODE;
  app=await _electron.launch({executablePath:process.env.ELECTRON_BINARY,args:[path.resolve('desktop'),'--no-sandbox','--ozone-platform=x11'],env,timeout:60000});
  const page=await app.firstWindow({timeout:30000});await page.waitForURL(/^http:/,{timeout:90000});
  desktopPid=Number(fs.readFileSync(path.join(desk,'hub.lock'),'utf8').split(' ')[0]);
  const local=new URL(page.url()).origin;check(local!==origin,'the desktop app runs its own service');
  // the desktop's own service lists the hub (its BACKPLANE_PEERS) once it answers /hello
  const deskToken=fs.readFileSync(path.join(desk,'token'),'utf8').trim();
  const known=await waitFor(async()=>{const r=await fetch(local+'/auth/known',{method:'POST',headers:{Authorization:'Bearer '+deskToken,'X-Backplane-Origin':origin}});return (await r.json()).known===true;},150000);
  check(known,'the desktop\'s own service knows the hub');
  await app.evaluate(({dialog})=>{globalThis.__asked=[];globalThis.__errors=[];dialog.showMessageBox=async(_w,o)=>{globalThis.__asked.push(o.message+' | '+o.detail);return {response:0};};dialog.showErrorBox=(t,m)=>{globalThis.__errors.push(t)};});
  console.log('ok (desktop app ready, dialogs stubbed)');
  const follow=link=>app.evaluate(({app},l)=>{app.emit('second-instance',{},['backplane',l],process.cwd());},'backplane://pair?url='+encodeURIComponent(link));
  // a stranger's address: asked, Cancel, nothing paired, nothing sent there
  await follow(strangeOrigin+'/pair#invite='+'d'.repeat(32));await pause(2500);
  const asked=await app.evaluate(()=>globalThis.__asked);
  check(asked.length===1&&asked[0].includes(strangeOrigin),'a link to an address no hub knows asks first, naming it',asked);
  check(new URL(page.url()).origin===local,'and, cancelled, the window stays where it was',page.url());
  check(strangers.length===0,'nothing was sent to that address',strangers);
  check(!fs.existsSync(path.join(desk,'desktop-remote.json')),'and no hub was saved');
  // the owner's hub (known to the desktop's service): paired at once
  const d=(await api('/auth/invitations',{method:'POST'})).value;
  await Promise.all([page.waitForURL(origin+'/',{timeout:30000}).catch(()=>{}),follow(d.link)]);
  check((await app.evaluate(()=>globalThis.__asked)).length===1,'a link to one of the owner\'s hubs asks nothing');
  check(new URL(page.url()).origin===origin,'and pairs the window with it',page.url());
  const used=await listed(d.id);check(used?.state==='used'&&!!used.deviceName,'the hub lists the link as used, by the desktop',used);
  // a link that hub revoked: refused there, nothing changes
  const e=(await api('/auth/invitations',{method:'POST'})).value;await api('/auth/revoke',{method:'POST',headers:{'X-Backplane-Invitation':e.id}});
  await follow(e.link);await pause(2500);
  check((await app.evaluate(()=>globalThis.__errors)).length===1,'a revoked link from the owner\'s hub is refused (and said so)');
  // revoking the link the desktop used revokes the desktop too
  await api('/auth/revoke',{method:'POST',headers:{'X-Backplane-Invitation':d.id}});
  const repair=await waitFor(async()=>await page.getByRole('heading',{name:'Re-pair this device',exact:true}).isVisible(),30000);
  check(repair,'revoking its link sends the desktop back to Re-pair');
 }
}catch(e){failed=true;console.error('fixture error: '+String(e&&e.message||e).replace(/[a-f0-9]{32}/g,'[redacted]').slice(0,1500));}
finally{
 clearTimeout(watchdog);await stopAll();
 if(failed||process.argv.includes('--keep'))console.log('fixture kept: '+root);else fs.rmSync(root,{recursive:true,force:true});
 process.exitCode=failed?1:0;
}})();
