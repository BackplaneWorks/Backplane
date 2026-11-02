// Built hub + real web UI, isolated home and fake provider-owned authentication.
// Never opens a real sign-in page, contacts a provider, or uses live accounts.
import {mkdtempSync,mkdirSync,writeFileSync,readFileSync,readdirSync,chmodSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
const [binary='build/provider-onboarding/payload/backplane-serve',web='build/provider-onboarding/payload/web',wire='build/provider-onboarding/wire']=process.argv.slice(2);
const {chromium}=await import(process.env.BACKPLANE_PLAYWRIGHT_MODULE||resolve('tools/browser/node_modules/playwright-core/index.mjs'));
for(const f of readdirSync(wire).filter(f=>f.endsWith('.js')))(0,eval)(readFileSync(join(wire,f),'utf8'));
const W=(globalThis as any).Wire;
const root=mkdtempSync(join(tmpdir(),'bp-provider-setup-')),home=join(root,'home'),fake=join(root,'bin');mkdirSync(home);mkdirSync(fake);
const output=resolve('build/provider-onboarding/ui');mkdirSync(output,{recursive:true});
const quote=(s:string)=>"'"+s.replaceAll("'","'\\''")+"'";
for(const name of ['claude','codex','grok','muse']){
 writeFileSync(join(fake,name),['claude','codex'].includes(name)?`#!/bin/sh\nexec ${quote(process.execPath)} ${quote(resolve('test/tools/accounts_standin.cjs'))} ${name} "$@"\n`:'#!/bin/sh\nexit 127\n');chmodSync(join(fake,name),0o755);
}
const listener=Bun.listen({hostname:'127.0.0.1',port:0,socket:{data(){}}}),port=listener.port;listener.stop(true);
const proc=Bun.spawn([resolve(binary),'--home',home,'--port',String(port),'--host','127.0.0.1','--no-tailscale','--headless','--foreground','--web',resolve(web)],{env:{...process.env,DISPLAY:'',WAYLAND_DISPLAY:'',BACKPLANE_AGENTS:fake,BACKPLANE_NO_UPDATE:'1',BACKPLANE_KISTACK_SYNC:'0',BACKPLANE_TELEMETRY:'off',BACKPLANE_PEERS:'',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',BP_ACCOUNTS_AUDIT:join(root,'audit.jsonl')},stdout:Bun.file(join(output,'hub.stdout')),stderr:Bun.file(join(output,'hub.stderr'))});
let browser:any,ws:any,page:any,failed=false;const checks:any[]=[],errors:string[]=[];
const check=(name:string,ok:any,got?:any)=>{console.log((ok?'ok ':'FAIL ')+name+(ok?'':': '+JSON.stringify(got)?.slice(0,500)));checks.push({name,pass:!!ok});if(!ok)failed=true;};
const pause=(ms:number)=>new Promise(r=>setTimeout(r,ms));
async function until(fn:()=>any){for(let i=0;i<200;i++){if(await fn())return;await pause(50);}throw Error('bounded condition timeout');}
try{
 await until(async()=>{try{return (await fetch(`http://127.0.0.1:${port}/hello`)).ok;}catch{return false;}});
 const token=readFileSync(join(home,'token'),'utf8').trim();
 ws=new WebSocket(`ws://127.0.0.1:${port}/ws?token=${token}`);ws.binaryType='arraybuffer';const replies=new Map();let n=0;
 ws.onmessage=(e:any)=>{const x=JSON.parse(W.decode(new Uint8Array(e.data)));if(x.t==='reply')replies.set(Number(x.id),x);};
 await new Promise((r,j)=>{ws.onopen=r;ws.onerror=j;});
 async function rpc(m:string,p:any){const id=++n;ws.send(W.encode(JSON.stringify({id,m,p})));await until(()=>replies.has(id));const r=replies.get(id);if(!r.ok)throw Error(JSON.stringify(r));return r.result;}
 browser=await chromium.launch({executablePath:process.env.BACKPLANE_CHROMIUM||'/home/h/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome',headless:true,args:['--no-sandbox']});
 const ctx=await browser.newContext({viewport:{width:1440,height:1000}});
 await ctx.route('**/*',(r:any)=>new URL(r.request().url()).hostname==='127.0.0.1'?r.continue():r.abort());
 await ctx.addInitScript(()=>localStorage.setItem('backplane-lang','en'));
 page=await ctx.newPage();page.on('pageerror',(e:any)=>errors.push(String(e)));
 await page.goto(`http://127.0.0.1:${port}/#token=${token}`);
 const click=(action:string,value?:string)=>page.locator('[data-on-click="'+action+'"]'+(value===undefined?'':'[data-value="'+value+'"]')).first().click();
 const visible=(s:string)=>page.getByText(s,{exact:false}).first().waitFor({timeout:15000});
 await visible('Welcome to Backplane');await page.screenshot({path:join(output,'desktop-welcome.png')});
 await click('ob-next');await visible('1 · Choose your agent');
 check('first-run Next leads to agent setup',await page.locator('.tour').count()===0);
 await page.screenshot({path:join(output,'desktop-setup.png')});
 await click('provider-setup-close');await visible('Welcome to Backplane');
 check('return resumes same tour step',(await page.locator('.tour').textContent()).includes('1 of 11'));
 await click('ob-next');await visible('2 of 11');check('set up later continues tour',(await page.locator('.tour').textContent()).includes('2 of 11'));
 await click('ob-skip');await page.locator('.tour').waitFor({state:'detached'});
 await click('provider-setup-open');await visible('1 · Choose your agent');
 await page.locator('[data-bfield="accountlabel"]').fill('Onboarding Codex');await click('acct-add');
 const registry=()=>{try{return JSON.parse(readFileSync(join(home,'providers/accounts-v1/profiles.json'),'utf8')).profiles;}catch(e:any){if(e.code==='ENOENT')return [];throw e;}};
 await until(()=>registry().some((p:any)=>p.label==='Onboarding Codex'));const codex=registry().find((p:any)=>p.label==='Onboarding Codex');
 const fixture=join(home,'providers/accounts-v1',codex.id,'fixture.json');
 writeFileSync(fixture,JSON.stringify({email:'onboard@example.test',delay:1200,loginFailure:true}));
 await visible('Account status: disconnected');await click('acct-login');await visible('Sign-in did not finish');
 check('failure offers a retry',await page.getByRole('button',{name:'Sign in / Try again',exact:true}).isVisible());
 await page.screenshot({path:join(output,'desktop-failure.png')});
 writeFileSync(fixture,JSON.stringify({email:'onboard@example.test',delay:3000}));
 await click('acct-login');await visible('FIXTURE-ONLY');
 check('pending sign-in offers provider link and cancel',await page.getByRole('link',{name:'Open provider sign-in',exact:true}).count()===1&&await page.getByRole('button',{name:'Cancel sign-in',exact:true}).isVisible());
 await page.screenshot({path:join(output,'desktop-waiting.png')});
 await click('acct-cancel');await visible('Sign-in cancelled');
 writeFileSync(fixture,JSON.stringify({email:'onboard@example.test',delay:1500}));
 await click('acct-login');await visible('FIXTURE-ONLY');await click('provider-setup-close');
 await until(()=>readFileSync(join(home,'events.jsonl'),'utf8').trim().split('\n').filter(Boolean).map(x=>JSON.parse(x)).some((e:any)=>e.$==='SettingSet'&&e.key==='accounts.inventory'&&JSON.parse(e.value).profiles.some((p:any)=>p.id===codex.id&&p.state==='connected')));
 await click('provider-setup-open');await visible('Connection verified');
 check('connection can finish while away and update on return',true);await click('provider-setup-use');await visible('Ready for new threads');
 check('verified connection updates next action',await page.getByText('Ready for new threads',{exact:false}).isVisible());
 const events=()=>readFileSync(join(home,'events.jsonl'),'utf8').trim().split('\n').filter(Boolean).map(x=>JSON.parse(x));
 await until(()=>events().some((e:any)=>e.$==='SettingSet'&&e.key==='account.default.codex'&&JSON.parse(e.value).id===codex.id));
 check('explicit use records verified account for new threads',true);
 await page.screenshot({path:join(output,'desktop-connected.png')});
 await page.setViewportSize({width:390,height:844});await page.screenshot({path:join(output,'phone-connected.png'),fullPage:true});
 check('phone setup fits screen',await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));
 await click('provider-setup-pick','claude');await page.locator('[data-bfield="accountlabel"]').fill('Onboarding Claude');await click('acct-add');await visible('Account status: disconnected');
 await click('acct-login');await page.waitForSelector('[data-sealed^="account:"]');
 const claudeSignIn=await page.getByRole('link',{name:'Open provider sign-in',exact:true}).getAttribute('href');
 check('phone Claude offers current provider authorization URL',claudeSignIn?.startsWith('https://claude.com/cai/oauth/authorize?code=true&'),claudeSignIn);
 await page.locator('[data-sealed^="account:"]').fill('fixture-code');await page.getByRole('button',{name:'Complete sign-in',exact:true}).click();await visible('including #');
 check('incomplete code keeps phone sign-in open with an actionable message',await page.getByRole('link',{name:'Open provider sign-in',exact:true}).isVisible());
 await page.locator('[data-sealed^="account:"]').fill('fixture-code#old-attempt');await page.getByRole('button',{name:'Complete sign-in',exact:true}).click();await visible('different sign-in attempt');
 check('stale code keeps phone sign-in open with its current link',await page.getByRole('link',{name:'Open provider sign-in',exact:true}).getAttribute('href')===claudeSignIn);
 await page.locator('[data-sealed^="account:"]').fill('fixture-code#'+new URL(claudeSignIn).searchParams.get('state'));await page.getByRole('button',{name:'Complete sign-in',exact:true}).click();await visible('Connection verified');
 check('phone Claude completes existing sealed ceremony',await page.getByText('Connection verified',{exact:false}).isVisible());
 check('sealed response never enters browser storage',await page.evaluate(()=>!Object.values(localStorage).some(x=>x.includes('fixture-code'))));
 await page.screenshot({path:join(output,'phone-claude.png'),fullPage:true});
 check('phone Claude form fits screen',await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));
 await click('provider-setup-close');await page.reload();await page.getByRole('button',{name:'Set up an agent',exact:true}).first().waitFor();
 check('completed onboarding does not trap a returning user',await page.locator('.tour').count()===0);
 // No provider inference is ever needed to verify or complete setup.
 const audit=readFileSync(join(root,'audit.jsonl'),'utf8');check('setup never starts a model turn',!audit.includes('"kind":"turn"')&&!audit.includes('"kind":"writing"'));
 check('no browser exceptions',errors.length===0,errors);
}catch(e){check('bounded browser flow completed',false,String(e));if(page)await page.screenshot({path:join(output,'failure.png'),fullPage:true}).catch(()=>{});}
finally{writeFileSync(join(output,'RESULT.json'),JSON.stringify({passed:!failed,checks,errors,root,scope:'built web/hub with isolated fake CLI authentication only'},null,2));ws?.close();await browser?.close();proc.kill();await proc.exited;}
process.exit(failed?1:0);
