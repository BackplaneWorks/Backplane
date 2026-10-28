// Browser qualification against the built isolated hub and fake provider stores.
import {readFileSync,readdirSync,writeFileSync,mkdirSync} from 'node:fs';
import {join,resolve} from 'node:path';
const {chromium}=await import(process.env.BACKPLANE_PLAYWRIGHT_MODULE||resolve('tools/browser/node_modules/playwright-core/index.mjs'));
const [root=readFileSync('build/multi-account/last-fixture.txt','utf8').trim(),binary='build/multi-account/backplane-accounts-review',web='build/multi-account/web-phone']=process.argv.slice(2);
const home=join(root,'home'),fake=join(root,'bin'),fixture=JSON.parse(readFileSync(join(root,'RESULT.json'),'utf8'));
const out=resolve('build/multi-account/ui');mkdirSync(out,{recursive:true});
for(const f of readdirSync('build/multi-account/wire').filter(f=>f.endsWith('.js')))(0,eval)(readFileSync(join('build/multi-account/wire',f),'utf8'));
const W=(globalThis as any).Wire;
const listener=Bun.listen({hostname:'127.0.0.1',port:0,socket:{data(){}}}),port=listener.port;listener.stop(true);
const proc=Bun.spawn([resolve(binary),'--home',home,'--port',String(port),'--host','127.0.0.1','--no-tailscale','--headless','--foreground','--web',resolve(web)],{env:{...process.env,DISPLAY:'',WAYLAND_DISPLAY:'',BACKPLANE_AGENTS:fake,PATH:fake+':'+process.env.PATH,BACKPLANE_NO_UPDATE:'1',BACKPLANE_PEERS:'',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',BP_ACCOUNTS_AUDIT:join(root,'ui-audit.jsonl')},stdout:Bun.file(join(out,'hub.stdout')),stderr:Bun.file(join(out,'hub.stderr'))});
let browser:any,ws:any,page:any;const wireTrace:any[]=[];let failed=false;const errors:string[]=[],checks:any[]=[];
const check=(name:string,ok:any,got?:any)=>{console.log((ok?'ok ':'FAIL ')+name+(ok?'':': '+JSON.stringify(got)?.slice(0,500)));checks.push({name,pass:!!ok});if(!ok)failed=true;};
const pause=(ms:number)=>new Promise(r=>setTimeout(r,ms));
try{
 for(let i=0;i<150;i++){try{if((await fetch(`http://127.0.0.1:${port}/hello`)).ok)break;}catch{}await pause(50);}
 const token=readFileSync(join(home,'token'),'utf8').trim();
 ws=new WebSocket(`ws://127.0.0.1:${port}/ws?token=${token}`);ws.binaryType='arraybuffer';const replies=new Map();let n=0;
 ws.onmessage=(e:any)=>{const x=JSON.parse(W.decode(new Uint8Array(e.data)));if(x.t==='reply')replies.set(Number(x.id),x);};
 await new Promise((r,j)=>{ws.onopen=r;ws.onerror=j;});
 async function rpc(m:string,p:any){const id=++n;ws.send(W.encode(JSON.stringify({id,m,p})));for(let i=0;i<200;i++){if(replies.has(id))return replies.get(id);await pause(30);}throw Error('setup RPC timeout');}
 await rpc('setting.set',{key:'onboard.done',value:'1'});await rpc('setting.set',{key:'onboard.uber',value:'1'});await rpc('setting.set',{key:'telemetry.enabled',value:'false'});
 browser=await chromium.launch({executablePath:'/home/h/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome',headless:true,args:['--no-sandbox']});
 const context=await browser.newContext({viewport:{width:1440,height:1000}});
 await context.route('**/*',(route:any)=>new URL(route.request().url()).hostname==='127.0.0.1'?route.continue():route.abort());
 await context.addInitScript((thread:string)=>{localStorage.setItem('backplane-selected',thread);localStorage.setItem('backplane-lang','en');},fixture.thread);
 page=await context.newPage();page.on('pageerror',(e:any)=>errors.push(String(e)));
 page.on('websocket',(socket:any)=>{for(const ev of ['framesent','framereceived'])socket.on(ev,(frame:any)=>{try{const raw=typeof frame.payload==='string'?frame.payload:W.decode(new Uint8Array(frame.payload));const x=JSON.parse(raw);if(x.m||x.ok===false)wireTrace.push({direction:ev,id:x.id,m:x.m,p:x.p?{action:x.p.action,thread:x.p.thread,accountBinding:x.p.accountBinding,op:x.p.op}:undefined,error:x.error});}catch{}});});
 await page.goto(`http://127.0.0.1:${port}/#token=${token}`);await page.waitForSelector('.account-chip',{timeout:30000});
 check('actual web restores selected named thread',(await page.locator('.account-chip').innerText()).includes('A'));
 await page.screenshot({path:join(out,'desktop-thread.png'),fullPage:true});
 await page.locator('[data-on-click="flag"][data-value="settings"]').click();
 await page.locator('[data-on-click="settings-page"][data-value="agents"]').first().click();
 await page.waitForSelector('[data-bfield="accountlabel"]');
 await page.screenshot({path:join(out,'desktop-accounts.png'),fullPage:true});
 const click=(action:string,value?:string)=>page.locator('[data-on-click="'+action+'"]'+(value===undefined?'':'[data-value="'+value+'"]')).first().click();
 async function waitBody(text:string){await page.getByText(text,{exact:false}).first().waitFor({timeout:15000});}
 const registry=()=>JSON.parse(readFileSync(join(home,'providers/accounts-v1/profiles.json'),'utf8')).profiles;
 const a=registry().find((p:any)=>p.label==='A');await click('acct-status',a.id);await waitBody('a@example.test');
 check('Status verifies existing account after hub restart',await page.getByText('a@example.test',{exact:false}).first().isVisible());
 const label='UI-'+Date.now();await page.locator('[data-bfield="accountlabel"]').fill(label);await click('acct-add');
 let added:any;for(let i=0;i<100;i++){added=registry().find((p:any)=>p.label===label);if(added)break;await pause(30);}check('Add account through actual UI creates private metadata',!!added);
 if(!added)throw Error('UI add failed');
 writeFileSync(join(home,'providers/accounts-v1',added.id,'fixture.json'),JSON.stringify({email:'ui@example.test',workspace:'ui-workspace',delay:2000,used:41}));
 await click('acct-login',added.id);await waitBody('FIXTURE-ONLY');
 await page.screenshot({path:join(out,'desktop-login.png'),fullPage:true});
 check('in-app login displays provider-owned device ceremony',await page.getByRole('link',{name:'Open provider sign-in',exact:true}).count()===1);
 await click('acct-cancel');await pause(2200);
 check('Cancel removes only the pending ceremony',await page.getByText('FIXTURE-ONLY',{exact:false}).count()===0);
 writeFileSync(join(home,'providers/accounts-v1',added.id,'fixture.json'),JSON.stringify({email:'ui@example.test',workspace:'ui-workspace',delay:350,used:41}));
 await click('acct-login',added.id);await waitBody('ui@example.test');
 check('UI login becomes connected only after provider verification',await page.getByText('ui@example.test',{exact:false}).first().isVisible());
 await click('acct-status',added.id);await waitBody('41%');
 await click('acct-provider','claude');const claudeLabel='UI-Claude-'+Date.now();
 await page.locator('[data-bfield="accountlabel"]').fill(claudeLabel);await click('acct-add');
 let claudeAdded:any;for(let i=0;i<100;i++){claudeAdded=registry().find((p:any)=>p.label===claudeLabel);if(claudeAdded)break;await pause(30);}if(!claudeAdded)throw Error('Claude UI Add failed');
 writeFileSync(join(home,'providers/accounts-v1',claudeAdded.id,'fixture.json'),JSON.stringify({email:'ui-claude@example.test',workspace:'ui-claude-workspace'}));
 await click('acct-login',claudeAdded.id);await page.waitForSelector('[data-sealed^="account:"]');
 await page.screenshot({path:join(out,'desktop-claude-login.png'),fullPage:true});
 await page.locator('[data-sealed^="account:"]').fill('fixture-code');
 await page.getByRole('button',{name:'Complete sign-in',exact:true}).click();await waitBody('ui-claude@example.test');
 check('Claude completes provider ceremony through sealed in-app input',await page.getByText('ui-claude@example.test',{exact:false}).first().isVisible());
 check('provider response is absent from localStorage',await page.evaluate(()=>!Object.values(localStorage).some(x=>x.includes('fixture-code'))));

 await page.setViewportSize({width:390,height:844});await page.screenshot({path:join(out,'phone-accounts.png'),fullPage:true});
 check('phone Settings fits width without horizontal overflow',await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));
 await page.locator('[data-on-click="flag"][data-value="settings"]').first().click();await page.waitForSelector('.account-chip');
 const composer=page.locator('textarea#composer');await composer.fill('Draft preserved 日本語');
 await composer.evaluate((el:any)=>{el.dispatchEvent(new CompositionEvent('compositionstart',{bubbles:true,data:'日本語'}));el.dispatchEvent(new CompositionEvent('compositionend',{bubbles:true,data:'日本語'}));});
 await page.screenshot({path:join(out,'phone-draft-before-popup.png'),fullPage:true});
 writeFileSync(join(out,'phone-hit.json'),JSON.stringify(await page.locator('.account-chip').evaluate((el:any)=>{const r=el.getBoundingClientRect();return {rect:r.toJSON(),hit:document.elementFromPoint(r.x+r.width/2,r.y+r.height/2)?.outerHTML,active:document.activeElement?.outerHTML};}),null,2));
 await click('acct-open');await page.waitForSelector('.account-picker');await page.screenshot({path:join(out,'phone-account-picker.png'),fullPage:true});
 check('phone account popup retains IME draft',await composer.inputValue()==='Draft preserved 日本語');
 check('phone popup fits screen',await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));
 await click('acct-select',added.id);await page.waitForFunction((label:string)=>document.querySelector('.account-chip')?.textContent?.includes(label),label);
 check('between-turn selection updates actual account label',await page.locator('.account-chip').innerText().then((x:string)=>x.includes(label)));
 check('account switch preserves draft exactly',await composer.inputValue()==='Draft preserved 日本語');
 await page.screenshot({path:join(out,'phone-selected-account.png'),fullPage:true});
 await page.setViewportSize({width:1440,height:1000});await page.screenshot({path:join(out,'desktop-selected-account.png'),fullPage:true});
 await page.locator('[data-on-click="flag"][data-value="settings"]').first().click();await click('settings-page','agents');await click('acct-disconnect',added.id);
 await pause(500);
 check('UI Disconnect reports selected profile disconnected',await page.locator('[data-on-click="acct-login"][data-value="'+added.id+'"]').evaluate((el:any)=>el.parentElement.parentElement.textContent.includes('disconnected')));
 await pause(300);writeFileSync(join(out,'wire-trace.json'),JSON.stringify(wireTrace,null,2));
 check('Settings actions produce no account-compatibility refusals',!wireTrace.some((x:any)=>x.direction==='framereceived'&&x.error));
 check('no browser runtime exceptions',errors.length===0,errors);
}catch(e){check('actual browser flow completed',false,String(e));console.log(String(e));if(page)await page.screenshot({path:join(out,'failure.png'),fullPage:true}).catch(()=>{});}
finally{writeFileSync(join(out,'wire-trace.json'),JSON.stringify(wireTrace,null,2));writeFileSync(join(out,'RESULT.json'),JSON.stringify({scope:'actual web, fake provider stores only',checks,errors,passed:!failed},null,2));ws?.close();await browser?.close();proc.kill();await proc.exited;}
process.exit(failed?1:0);
