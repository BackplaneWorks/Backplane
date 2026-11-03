// Real hub + real browser, private home and stand-in agents only.
// Run after the pairing web/service builds. The emitted hub is preloaded
// with a fixture-only clock (not shipped) so expiry needs no ten-minute wait.
import {mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync, existsSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {createRequire} from 'node:module';
const require = createRequire(import.meta.url);
const {chromium} = require(process.env.PLAYWRIGHT_MODULE || 'playwright-core');
const decodeQR = require('../../src/web/vendor/jsqr.cjs');
const [hubScript, web] = process.argv.slice(2);
if (!hubScript || !web) throw Error('emitted-hub-script web-directory required');
const root = mkdtempSync(join(tmpdir(), 'bp-pairing-e2e-')), home = join(root,'hub'), fake = join(root,'agents'), clock = join(root,'clock');
for (const d of [home,fake]) mkdirSync(d,{recursive:true,mode:0o700});
writeFileSync(clock,'0');
for (const cli of ['claude','codex','grok','muse','opencode','pi','omp','tailscale']) writeFileSync(join(fake,cli),'#!/bin/sh\nexit 127\n',{mode:0o755});
const preload = join(root,'clock.cjs');
writeFileSync(preload,"const fs=require('node:fs'),native=Date.now;Date.now=()=>native()+Number(fs.readFileSync(process.env.BACKPLANE_FIXTURE_CLOCK,'utf8'));\n");
const lease = Bun.listen({hostname:'127.0.0.1',port:0,socket:{data(){}}});const port=lease.port;lease.stop(true);
const origin = 'http://127.0.0.1:' + port;
let child:any, browser:any, electron:any, desktopPid:number|undefined, passed=0;
const check = (ok:unknown, label:string) => { if (!ok) throw Error(label);console.log('ok '+label);passed++; };
const pause = (ms:number)=>new Promise(r=>setTimeout(r,ms));
async function waitFor(fn:()=>Promise<any>, label:string, ms=20000) {
 const end=Date.now()+ms;
 while(Date.now()<end) { if(child?.exitCode != null) throw Error('isolated hub exited');try{const v=await fn();if(v)return v;}catch{}await pause(80); }
 throw Error('timeout: '+label);
}
try {
 child = Bun.spawn([process.execPath,'--preload',preload,resolve(hubScript),'--home',home,'--web',resolve(web),'--port',String(port),'--host','127.0.0.1','--no-tailscale'],{
  env:{...process.env,HOME:root,BACKPLANE_FIXTURE_CLOCK:clock,BACKPLANE_AGENTS:fake,BACKPLANE_KISTACK_SYNC:'0',BACKPLANE_NO_UPDATE:'1',BACKPLANE_TELEMETRY:'off',BACKPLANE_GOOGLE_BAKE:'0',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9'},
  stdout:Bun.file(join(root,'stdout')),stderr:Bun.file(join(root,'stderr')),
 });
 await waitFor(async()=>{const r=await fetch(origin+'/hello');return r.status===200},'hub ready',60000);
 const rootToken=readFileSync(join(home,'token'),'utf8').trim();
 async function api(path:string, token=rootToken, method='GET', headers:any={}) {
  const r=await fetch(origin+path,{method,headers:{Authorization:'Bearer '+token,...headers},redirect:'error'});
  let value:any={};try{value=await r.json()}catch{}return{status:r.status,value,headers:r.headers};
 }
 async function invite(){const r=await api('/auth/invitations',rootToken,'POST');check(r.status===200 && !!r.value.code,'authorized invitation issuance');return r.value;}
 async function enroll(secret:string,name='Fixture device'){return api('/auth/enroll',secret,'POST',{'X-Backplane-Client':'native','X-Backplane-Name':name});}
 check((await fetch(origin+'/auth/cache')).status===401,'fresh browser has no authenticated identity');
 check((await api('/auth/invitations','bad','POST')).status===401,'unpaired clients cannot create invitations');
 check((await fetch(origin+'/auth/invitations',{method:'POST',headers:{Authorization:'Bearer '+rootToken,Origin:'https://foreign.invalid'}})).status===401,'cross-origin invitation issuance rejected');
 const a=await invite(), secret=new URLSearchParams(new URL(a.link).hash.slice(1)).get('invite')!;
 check(!new URL(a.link).search && secret!==rootToken,'share link uses a disposable fragment');
 const link=await enroll(secret,'Laptop');check(link.status===200 && !!link.value.token,'link exchange creates a device session');
 check((await enroll(a.code)).value.error==='used_code','link and short code share single-use state');
 const b=await invite(), rows=String(b.qr).split('\n'), scale=5, side=(rows.length+8)*scale, pixels=new Uint8ClampedArray(side*side*4).fill(255);
 rows.forEach((row:string,y:number)=>[...row].forEach((bit,x)=>{if(bit==='1')for(let yy=0;yy<scale;yy++)for(let xx=0;xx<scale;xx++){const i=(((y+4)*scale+yy)*side+(x+4)*scale+xx)*4;pixels[i]=pixels[i+1]=pixels[i+2]=0;}}));
 const qr=decodeQR(pixels,side,side);check(qr?.data===b.link,'real QR decoder recovers the issued link');
 const qrSecret=new URLSearchParams(new URL(qr.data).hash.slice(1)).get('invite')!;
 const scanned=await enroll(qrSecret,'QR phone');check(scanned.status===200,'decoded QR pairs through the same endpoint');
 const c=await invite(), coded=await enroll(c.code,'Code phone');check(coded.status===200,'short-code exchange creates a separate device');
 check((await enroll('ZZZZZZZZ')).value.error==='wrong_code','wrong code has an actionable response');
 const expiring=await invite();writeFileSync(clock,String(600001));
 check((await enroll(expiring.code)).value.error==='expired_code','expiry enforced by real HTTP enrollment');
 writeFileSync(clock,'0');
 const listed=await api('/auth/devices',link.value.token);check(listed.status===200 && listed.value.devices.some((d:any)=>d.name==='Laptop') && !JSON.stringify(listed.value).includes('hash'),'device names and last activity listed without credential hashes');
 // An already-open session must close on revoke, not only on next refresh.
 const socket=new WebSocket(origin.replace('http:','ws:')+'/ws?view=1&token='+link.value.token);
 let opened=false,closed=false;socket.onopen=()=>{opened=true};socket.onclose=()=>{closed=true};
 await waitFor(async()=>opened,'device websocket');
 const revoked=await api('/auth/revoke',rootToken,'POST',{'X-Backplane-Device':link.value.id});
 check(revoked.status===200,'specific device revoked');
 await waitFor(async()=>closed,'revoked socket closes');check(closed,'revocation closes an active connection');
 check((await api('/auth/cache',link.value.token)).status===401,'revoked credential is rejected');
 check((await api('/auth/cache',coded.value.token)).status===200,'other device remains paired');
 check((await fetch(origin+'/pair',{headers:{Cookie:'bp_token='+link.value.token}})).status===200,'re-pair page still loads with a revoked cookie');
 const repaired=await enroll((await invite()).code,'Laptop');check(repaired.status===200,'new invitation re-pairs the revoked device');
 // Real browser startup must never settle on the empty application.
 browser=await chromium.launch({headless:true,executablePath:process.env.CHROMIUM_PATH,args:['--no-sandbox']});
 const fresh=await browser.newContext({viewport:{width:1200,height:850}}), page=await fresh.newPage();
 page.on('pageerror',(e:any)=>console.log('browser error: '+e.message.replace(/[a-f0-9]{32}|#\S+/g,'[redacted]').slice(0,500)));
 await page.goto(origin);
 try { await page.getByRole('heading',{name:'Connect to your Backplane hub',exact:true}).waitFor(); }
 catch(e) { await page.screenshot({path:resolve('build/pairing-ux/browser-failure.png')});console.log('body: '+(await page.locator('body').innerText()).replace(/[a-f0-9]{32}|#\S+/g,'[redacted]').slice(0,900));throw e; }
 await page.screenshot({path:resolve('build/pairing-ux/fresh-desktop.png')});
 check(await page.locator('#root').getAttribute('inert')!==null,'fresh desktop presents a full-screen pairing gate');
 const webInvite=await invite();await page.goto(origin+'/pair'+new URL(webInvite.link).hash);
 await page.getByRole('button',{name:'Connect this device',exact:true}).waitFor();
 check(!(await page.evaluate(()=>location.hash)),'invitation scrubbed from browser history before exchange');
 await page.getByRole('button',{name:'Connect this device',exact:true}).click();
 await page.waitForURL(origin+'/');await waitFor(async()=>await fresh.cookies().then((c:any[])=>c.some(x=>x.name==='bp_token'&&x.httpOnly)),'durable browser cookie');
 check(true,'browser link pairing installs an HttpOnly session cookie');
 await page.goto(origin+'/devices');await page.getByRole('heading',{name:'Devices',exact:true}).waitFor();await page.getByRole('button',{name:'Add a device',exact:true}).click();
 await page.getByRole('button',{name:'Copy link',exact:true}).waitFor({timeout:30000});
 check(await page.locator('.pairing-qr').count()===1,'device manager shows a scannable QR invitation');
 const browserDevices=await api('/auth/devices'), last=browserDevices.value.devices.find((d:any)=>d.id!==link.value.id && d.id!==scanned.value.id && d.id!==coded.value.id && d.id!==repaired.value.id);
 check(!!last,'browser session appears in device management');
 await api('/auth/revoke',rootToken,'POST',{'X-Backplane-Device':last.id});
 await page.getByRole('heading',{name:'Re-pair this device',exact:true}).waitFor({timeout:20000});check(true,'revoked browser automatically sees full-screen re-pair without reload');
 const mobile=await browser.newContext({viewport:{width:390,height:844},isMobile:true,hasTouch:true}), phone=await mobile.newPage();await phone.goto(origin);await phone.getByRole('heading',{name:'Connect to your Backplane hub',exact:true}).waitFor();
 check(await phone.evaluate(()=>document.querySelector('.pairing-screen')!.scrollWidth<=innerWidth),'mobile web gate has no horizontal overflow');await phone.screenshot({path:resolve('build/pairing-ux/fresh-mobile.png')});
 // Exercise manual code and QR-image controls, not only their HTTP endpoint.
 const manual=await invite();await phone.getByText('Type a short code',{exact:true}).click();
 await phone.getByLabel('Hub address',{exact:true}).fill(origin);await phone.getByLabel('8-character code',{exact:true}).fill(manual.code);
 await phone.getByRole('button',{name:'Connect with code',exact:true}).click();await phone.waitForURL(origin+'/');
 await waitFor(async()=>await mobile.cookies().then((c:any[])=>c.some(x=>x.name==='bp_token'&&x.httpOnly)),'manual code cookie');check(true,'mobile-width manual code controls complete pairing');
 const qrContext=await browser.newContext(),qrPage=await qrContext.newPage(),imageInvite=await invite();
 const png=await page.evaluate((rows:string[])=>{const s=(rows.length+8)*6,c=document.createElement('canvas');c.width=c.height=s;const ctx=c.getContext('2d')!;ctx.fillStyle='white';ctx.fillRect(0,0,s,s);ctx.fillStyle='black';rows.forEach((r,y)=>[...r].forEach((v,x)=>{if(v==='1')ctx.fillRect((x+4)*6,(y+4)*6,6,6)}));return c.toDataURL('image/png').split(',')[1]},String(imageInvite.qr).split('\n'));
 await qrPage.goto(origin);await qrPage.getByRole('heading',{name:'Connect to your Backplane hub',exact:true}).waitFor();
 await qrPage.getByLabel('Or choose a QR image',{exact:true}).setInputFiles({name:'fixture-qr.png',mimeType:'image/png',buffer:Buffer.from(png,'base64')});
 await waitFor(async()=>await qrContext.cookies().then((c:any[])=>c.some(x=>x.name==='bp_token'&&x.httpOnly)),'QR image cookie');check(true,'actual QR image control completes pairing locally');
 if(process.env.ANDROID_SERIAL) {
   const run=(args:string[])=>{const r=Bun.spawnSync([process.env.ADB||'adb','-s',process.env.ANDROID_SERIAL!,...args]);if(r.exitCode)throw Error('isolated Android command failed');return r.stdout.toString()};
   // The owned emulator may sleep while browser checks run. Wake it before UI assertions.
   run(['shell','input','keyevent','KEYCODE_WAKEUP']);
   run(['shell','wm','dismiss-keyguard']);
   run(['shell','am','start','-W','-n','dev.backplane.mobile/.MainActivity']);
   const xml=()=>{run(['shell','uiautomator','dump','/sdcard/pairing.xml']);return run(['shell','cat','/sdcard/pairing.xml'])};
   await waitFor(async()=>xml().includes('Connect to your Backplane hub'),'Android first-run',45000);check(true,'native Android has full-screen first-run view');
   const before=(await api('/auth/devices')).value.devices;
   const deviceInvite=await api('/auth/invitations',rootToken,'POST',{Host:'10.0.2.2:'+port});
   const link='http://10.0.2.2:'+port+'/pair'+new URL(deviceInvite.value.link).hash;
   run(['shell','am','start','-a','android.intent.action.VIEW','-d','backplane://pair?url='+encodeURIComponent(link),'-n','dev.backplane.mobile/.MainActivity']);
   const device=await waitFor(async()=>{const r=await api('/auth/devices');return r.value.devices.find((d:any)=>!before.some((old:any)=>old.id===d.id))},'Android invitation exchange',45000);
   check(!!device,'native Android deep link exchanges disposable invitation');
   const preferences=()=>run(['shell','run-as','dev.backplane.mobile','cat','shared_prefs/backplane.xml']);
   const savedBefore=await waitFor(async()=>{const value=/<string name="links">([^<]*)<\/string>/.exec(preferences())?.[1];return value?.includes('#token=')?value:null},'native pairing durably saved',15000);
   const broken='http://10.0.2.2:'+port+'/pair#invite=invalid';
   run(['shell','am','start','-W','-a','android.intent.action.VIEW','-d','backplane://pair?url='+encodeURIComponent(broken),'-n','dev.backplane.mobile/.MainActivity']);
   const savedAfter=/<string name="links">([^<]*)<\/string>/.exec(preferences())?.[1];
   check(!!savedBefore&&savedBefore===savedAfter,'malformed invitation preserves native Android saved pairing');
   await api('/auth/revoke',rootToken,'POST',{'X-Backplane-Device':device.id});
   await waitFor(async()=>xml().includes('Re-pair this device'),'Android re-pair prompt',45000);check(true,'native Android has full-screen re-pair after revoke');
   const previousIds=(await api('/auth/devices')).value.devices.map((d:any)=>d.id);
   const repairInvite=await api('/auth/invitations',rootToken,'POST',{Host:'10.0.2.2:'+port});
   const repairLink='http://10.0.2.2:'+port+'/pair#code='+repairInvite.value.code;
   run(['shell','am','start','-a','android.intent.action.VIEW','-d','backplane://pair?url='+encodeURIComponent(repairLink),'-n','dev.backplane.mobile/.MainActivity']);
   await waitFor(async()=>{const r=await api('/auth/devices');return r.value.devices.some((d:any)=>!previousIds.includes(d.id))},'Android short-code re-pair',45000);
   await waitFor(async()=>{const value=/<string name="links">([^<]*)<\/string>/.exec(preferences())?.[1];return value?.includes('#token=')&&value!==savedBefore},'Android replacement saved',15000);
   check(true,'native Android re-pairs with a new short code');
 }
 if(process.env.ELECTRON_BINARY) {
   const native=Bun.spawn(['node','test/tools/pairing_electron_e2e.cjs'],{env:process.env,stdin:new Response(JSON.stringify({origin,token:rootToken,root,fake,web,binary:'build/pairing-ux/backplane-serve'})),stdout:'inherit',stderr:'pipe'});
   const [err,code]=await Promise.all([new Response(native.stderr).text(),native.exited]);
   if(code)throw Error('Electron fixture failed: '+err.replace(/[a-f0-9]{32}/g,'[redacted]').slice(0,1600));
 }
 const privateValues=[rootToken,secret,a.code,link.value.token,scanned.value.token,coded.value.token,repaired.value.token];
 const logs=readFileSync(join(root,'stdout'),'utf8')+readFileSync(join(root,'stderr'),'utf8');check(privateValues.every(x=>!logs.includes(x)),'hub logs contain no issued or durable credentials');
 console.log('PASS '+passed+' pairing end-to-end checks');
} finally {
 await electron?.close();if(desktopPid){try{process.kill(desktopPid,15)}catch{}}
 await browser?.close();child?.kill();if(child)await child.exited;
 if(process.env.KEEP_PAIRING_FIXTURE!=='1')rmSync(root,{recursive:true,force:true});
}
