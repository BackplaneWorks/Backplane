// Run on a remote machine with an explicit binary and web assets. The only
// target is a child hub created here with a fresh private home and fake agents.
import fs from 'node:fs';
import path from 'node:path';
import {createRequire} from 'node:module';
import {createHash} from 'node:crypto';
import {configuration,startFixture,assertOwnedURL,assertOwnedResource} from './instant_ui_fixture.mjs';
// Validate before importing Playwright, starting a hub or touching the network.
const config=configuration();
if (!process.stdin.isTTY && fs.readFileSync(0,'utf8').trim()) throw Error('Pairing input is forbidden; the isolated fixture issues its own invitation');
const engine=process.env.INSTANT_ENGINE || 'chromium';
if (!['chromium','webkit'].includes(engine)) throw Error('Expected chromium or webkit');
const require=createRequire(import.meta.url);
const {chromium,webkit}=require(process.env.PLAYWRIGHT_MODULE || 'playwright-core');
const out=path.resolve(process.env.INSTANT_OUT || 'build/instant-ui/isolated-'+engine+'.json');
fs.mkdirSync(path.dirname(out),{recursive:true});
const fixture=await startFixture(config), origin=fixture.origin;
const report={origin,isolated:true,fixtureRoot:fixture.root,binary:{path:config.binary,sha256:createHash('sha256').update(fs.readFileSync(config.binary)).digest('hex')},engine,started:new Date().toISOString(),viewport:{width:390,height:844},actions:[],errors:[],warnings:[],http:[],revoked:false};
let browser,context,device,activePage;
try {
 browser=await (engine==='webkit'?webkit:chromium).launch({headless:true,...(engine==='chromium'?{executablePath:process.env.CHROMIUM_PATH,args:['--no-sandbox']}:{executablePath:process.env.WEBKIT_PATH})});
 context=await browser.newContext({serviceWorkers:'block',viewport:report.viewport,deviceScaleFactor:3,isMobile:true,hasTouch:true,userAgent:'Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1'});
 // Fail closed if this Playwright cannot intercept WebSockets as well as HTTP.
 if(typeof context.routeWebSocket!=='function')throw Error('Playwright routeWebSocket support is required for isolation');
 await context.route('**/*',route=>{
  try{assertOwnedResource(route.request().url(),origin);return route.continue()}
  catch{report.errors.push('Blocked HTTP outside isolated fixture ('+new URL(route.request().url()).protocol+' '+new URL(route.request().url()).origin+')');return route.abort('blockedbyclient')}
 });
 await context.routeWebSocket('**/*',socket=>{
  try{assertOwnedURL(socket.url(),origin,true);socket.connectToServer()}
  catch{report.errors.push('Blocked WebSocket outside isolated fixture');socket.close()}
 });
 const name='TEST isolated instant-ui '+Date.now();
 const input={code:await fixture.invitation()};
 const enrolled=await context.request.post(origin+'/auth/enroll',{headers:{Authorization:'Bearer '+input.code,'X-Backplane-Name':name},maxRedirects:0});
 if(!enrolled.ok())throw Error('TEST enrollment failed: '+enrolled.status());
 device=(await (await context.request.get(origin+'/auth/devices',{maxRedirects:0})).json()).devices.find(d=>d.name===name)?.id;
 if(!device)throw Error('TEST enrollment not listed');
 for(let i=0;i<5;i++){const t=performance.now();const r=await context.request.get(origin+'/hello',{maxRedirects:0});const body=await r.body();report.http.push({ms:performance.now()-t,status:r.status(),bytes:body.length});report.hubHello=body.toString('utf8').replace(/[^\x20-\x7e]/g,'');}
 const page=activePage=await context.newPage();
 report.requests=[];page.on('requestfailed',r=>report.requests.push({path:new URL(r.url()).pathname,failure:r.failure()?.errorText}));page.on('response',r=>{if(['/hello','/auth/cache','/client/capabilities'].includes(new URL(r.url()).pathname))report.requests.push({path:new URL(r.url()).pathname,status:r.status()});});
 await page.addInitScript(()=>{
  window.__bpPerf={trace:[],render:[],recv:[],part:[],live:[],act:[]};
  window.__instant={received:[],times:{},counts:{},wire:[],connections:[]};
  const NativeSocket=window.WebSocket;
  window.WebSocket=class extends NativeSocket{
   constructor(url,...args){super(url,...args);const channel=String(url).includes('since=0')?'foreground':'prefetch';this.__channel=channel;window.__instant.connections.push({at:performance.now(),channel});this.addEventListener('message',event=>window.__instant.wire.push({at:performance.now(),channel,direction:'in',bytes:typeof event.data==='string'?event.data.length:event.data.byteLength||event.data.size||0}));}
   send(data){window.__instant.wire.push({at:performance.now(),channel:this.__channel,direction:'out',bytes:typeof data==='string'?data.length:data.byteLength||data.size||0});super.send(data);}
  };

  window.__bpRender={n(k,v=1){window.__instant.counts[k]=(window.__instant.counts[k]||0)+v},t(k,v){window.__instant.times[k]=(window.__instant.times[k]||0)+v},sent(id,method){window.__instant.sent ||= [];window.__instant.sent.push({at:performance.now(),id,method});},got(e,bytes,decodeMs){window.__instant.received.push({at:performance.now(),id:e.id,type:e.t,ok:e.ok,bytes,decodeMs,target:e.result?.target,nextTarget:e.result?.nextTarget,ready:e.result?.remoteReady})}};
 });
 page.on('pageerror',e=>{const bucket=e.message==='ResizeObserver loop completed with undelivered notifications.'?report.warnings:report.errors;bucket.push(e.message.slice(0,300))});
 const cdp=engine==='chromium'?await context.newCDPSession(page):null;
 if(cdp)await cdp.send('Performance.enable');
 const metrics=async()=>cdp?Object.fromEntries((await cdp.send('Performance.getMetrics')).metrics.map(x=>[x.name,x.value])):{};
 const boot=performance.now();await page.goto(origin);
 await page.waitForSelector('[data-on-click=select]',{timeout:90000});
 report.bootMs=performance.now()-boot;
 const controls=await page.evaluate(()=>[...document.querySelectorAll('[data-on-click]')].map(e=>({action:e.getAttribute('data-on-click'),value:e.getAttribute('data-value'),name:e.querySelector('.bot-name')?.textContent,label:e.textContent.slice(0,80)})));
 const threads=controls.filter(c=>c.action==='select').filter((c,i,a)=>a.findIndex(x=>x.value===c.value)===i);
 const preferred=fixture.threads;
 const five=preferred.map(id=>threads.find(c=>c.value===id)).filter(Boolean);
 if(five.length!==5)throw Error('Missing isolated thread controls');
 const bots=controls.filter(c=>c.action==='bot'&&c.value.startsWith('fixture-bot-')).slice(0,2);
 if(bots.length!==2)throw Error('Missing isolated bot controls');
 report.targets={threads:five,bots};
 async function action(control,cached=false,sendText=null){
  const before=await metrics();
  const result=await page.evaluate(async({control,cached,sendText})=>{
   const p=window.__bpPerf,i=window.__instant;const traceStart=p.trace.length,recvStart=i.received.length,renderStart=p.render.length,sentStart=i.sent?.length||0,wireStart=i.wire.length;
   i.times={};i.counts={};
   if(sendText!==null){const field=document.querySelector('#composer');if(!field)throw Error('Missing TEST composer');field.value=sendText;field.dispatchEvent(new Event('input',{bubbles:true}));}
   const el=[...document.querySelectorAll('[data-on-click]')].find(e=>e.getAttribute('data-on-click')===control.action && (control.value===null||e.getAttribute('data-value')===control.value));
   if(!el)throw Error('Missing action '+control.action);
   const start=performance.now(),prior=document.querySelector('main.thread-view')?.getAttribute('data-thread');let first=null,settled=null;
   el.click();
   const deadline=start+45000;
   while(performance.now()<deadline){
    await new Promise(r=>requestAnimationFrame(()=>setTimeout(r,0)));
    const main=document.querySelector('main.thread-view'),timeline=document.querySelector('#timeline');
    const matches=control.action==='select'?main?.getAttribute('data-thread')===control.value
      :sendText!==null?[...document.querySelectorAll('#timeline .msg')].some(e=>e.textContent.includes(sendText))
      :control.action==='new-thread'?!!document.querySelector('#composer')&&main?.getAttribute('data-thread')!==prior
      :document.querySelector('main .bot-title')?.textContent===control.name;
    if(matches&&first===null)first=performance.now()-start;
    const traces=p.trace.slice(traceStart),replies=i.received.slice(recvStart);
    const wanted=control.action==='bot'?{kind:'bot',id:control.value}:control.action==='select'?{kind:'thread',id:control.value}:null;
    const landed=replies.find(r=>r.ready===true && (!wanted || r.target?.kind===wanted.kind&&r.target?.id===wanted.id));
    const committed=sendText===null || [...document.querySelectorAll('#timeline .msg:not(.sending):not(.pending):not(.queued)')].some(e=>e.textContent.includes(sendText));
    const paint=traces.find(t=>(t.phase==='paint'||t.phase==='paint-part')&&t.ms>=(landed?.at||Infinity));
    if(first!==null && landed && paint && committed){settled=performance.now()-start;break;}
   }
   const end=performance.now();
   return {action:control.action,target:control.value,cached,firstContentMs:first,settledMs:settled,elapsedMs:end-start,
    wire:i.wire.slice(wireStart).map(t=>({...t,at:t.at-start})),sent:(i.sent||[]).slice(sentStart).map(t=>({...t,at:t.at-start})),trace:p.trace.slice(traceStart).map(t=>({...t,ms:t.ms-start,epochMs:undefined})),replies:i.received.slice(recvStart).map(t=>({...t,at:t.at-start})),render:p.render.slice(renderStart),times:i.times,counts:i.counts,
    thread:document.querySelector('main.thread-view')?.getAttribute('data-thread'),visibleRows:document.querySelectorAll('#timeline .msg').length};
  },{control,cached,sendText});
  const after=await metrics();result.browser=Object.fromEntries(['ScriptDuration','TaskDuration','LayoutDuration','RecalcStyleDuration','LayoutCount','RecalcStyleCount'].map(k=>[k,(after[k]||0)-(before[k]||0)]));
  const sentById=new Map(result.sent.map(t=>[t.id,t]));
  result.roundTrips=result.trace.filter(t=>t.phase==='send').map(t=>({method:sentById.get(t.id)?.method||t.method,id:t.id,at:t.ms,replyMs:result.replies.find(r=>r.id===t.id)?.at-t.ms,bytes:result.replies.find(r=>r.id===t.id)?.bytes}));
  report.actions.push(result);fs.writeFileSync(out,JSON.stringify(report,null,2));console.log(JSON.stringify({action:result.action,target:result.target,cached,firstContentMs:result.firstContentMs,settledMs:result.settledMs,requests:result.roundTrips.length}));
  return result;
 }
 for(let lap=0;lap<2;lap++)for(const t of five)await action(t,lap===1);
 for(let lap=0;lap<2;lap++)for(const b of bots)await action(b,lap===1);
 // Send and reply to an existing synthetic conversation in our private home.
 await action(five[0],true);
 report.testThread=five[0].value;
 for(const marker of ['Isolated latency send','Isolated latency reply']) {
  await action({action:'send',value:null},false,marker);
  const expected=marker.endsWith('reply')?2:1;
  await page.waitForFunction(n=>[...document.querySelectorAll('#timeline .assistant')].filter(e=>e.textContent.includes('ACK from isolated stand-in')).length>=n,expected,{timeout:15000});
 }
 report.fixture=fixture.evidence();
 if(report.fixture.standinACKs<2)throw Error('Both messages must reach the isolated stand-in and private event log');
 if(report.fixture.threads.length!==7 || report.fixture.threads.some(id=>!id.startsWith('fixture-')))throw Error('Unexpected conversation outside seeded fixture');
 if(report.actions.some(x=>x.firstContentMs===null||x.settledMs===null))throw Error('An isolated action did not paint and settle');
 if(report.errors.length)throw Error('Browser errors or blocked external requests');
 report.connections=await page.evaluate(()=>window.__instant.connections);
 try{report.serverPerf=await (await context.request.get(origin+'/debug/perf.json',{maxRedirects:0})).json();}catch{report.errors.push('Server perf unavailable');}
 await page.screenshot({path:out.replace(/\.json$/,'.png')});
 if(process.env.INSTANT_GATE==='1'){
  report.budgetFailures=report.actions.filter(x=>x.cached&&!(x.firstContentMs!==null&&x.firstContentMs<=100)||x.action==='send'&&!(x.firstContentMs!==null&&x.firstContentMs<=100)).map(x=>({action:x.action,target:x.target,ms:x.firstContentMs}));
  if(report.budgetFailures.length)process.exitCode=1;
 }

} catch(e){report.errors.push(e.message);process.exitCode=1;
 if(activePage)try{report.failureState=await activePage.evaluate(()=>({title:document.title,status:document.querySelector('#view-connection')?.textContent,body:document.body.innerText.slice(0,1000),trace:window.__bpPerf?.trace?.slice(-30),received:window.__instant?.received?.slice(-10),resources:performance.getEntriesByType('resource').map(r=>({path:new URL(r.name).pathname,ms:r.duration,bytes:r.transferSize}))}));await activePage.screenshot({path:out.replace(/\.json$/,'.failure.png')});}catch{}
}

finally{
 try {
  if(context&&device){try{report.revoked=(await context.request.post(origin+'/auth/revoke',{headers:{'X-Backplane-Device':device},maxRedirects:0})).ok();}catch{report.errors.push('Isolated TEST revocation request failed');}}
  if(!report.revoked)process.exitCode=1;
 } finally {
  try{await browser?.close()}finally{
   await fixture.stop();report.ended=new Date().toISOString();fs.writeFileSync(out,JSON.stringify(report,null,2));console.log('report '+out+'; isolated TEST revoked '+report.revoked+'; retained private fixture '+fixture.root);
  }
 }
}
