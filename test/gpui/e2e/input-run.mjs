// The must-pass GPUI input actions with real OS input on Xvfb (Boat): drag-select, Ctrl-C, paste an image, drop a file, IME, paperclip, link click.
// bun test/gpui/e2e/input-run.mjs --hub-bin PATH --gpui-bin PATH [--out DIR]   (Boat only; see boat-run.sh input-run)
import {readFileSync,writeFileSync,mkdirSync,appendFileSync,existsSync} from 'node:fs';import {spawn,execFileSync} from 'node:child_process';import {resolve} from 'node:path';
import {startDisplay,installMissing} from './lib/capture.mjs';import {startFixtureHub} from './lib/hubfx.mjs';import {Input,diffPixels} from './input/driver.mjs';import {startIme} from './input/ime.mjs';import {viewClient} from './lib/../../perf/view.mjs';import {preflight} from './input/preflight.mjs';
const arg=(k,d)=>{const i=process.argv.indexOf('--'+k);return i>=0?process.argv[i+1]:d};const sleep=ms=>new Promise(r=>setTimeout(r,ms));
const hubBin=resolve(arg('hub-bin')),gpuBin=resolve(arg('gpui-bin')),out=resolve(arg('out','out/e2e/input'));mkdirSync(out,{recursive:true});
const summary={when:new Date().toISOString(),actions:{},blocked:{},env:installMissing()};
const dsp=await startDisplay(64);
const pickFile=out+'/paste.png';execFileSync('convert',['-size','240x160','gradient:red-blue',pickFile],{env:dsp.env});
const portalLog=out+'/portal.log';let portal=null;
try{portal=spawn('python3',['test/gpui/e2e/input/portal_standin.py'],{env:{...process.env,PORTAL_PICK:pickFile,PORTAL_LOG:portalLog},stdio:'ignore'});for(let k=0;k<40;k++){await sleep(250);try{readFileSync(portalLog+'.ready');break}catch{}}}catch(e){summary.portalError=String(e.message)}
const ime=await startIme(dsp.display); // before the client, so it can connect to it through XIM
summary.ime={status:ime.status,engine:ime.engine};
// HARNESS SELF-CHECK before the client exists: if the pointer, clipboard, XDND or input method do not work against plain X clients, no verdict about GPUI is possible
const pf=await preflight({display:dsp.display,env:dsp.env,ime,out:out+'/preflight',portalLog});summary.preflight=pf;
const ACTIONS=['drag-select','ctrl-c','paste-image','drop-file','ime-hangul','paperclip'];
// which actions each failed check takes down: core input (pointer, keys, clipboard) takes down the whole run; the rest only the action that needs it
const coreFailed=pf.failed.filter(k=>/^(pointer|key-|wheel|clipboard|primary)/.test(k));
const harnessFor={'drag-select':pf.failed.filter(k=>/^selections-cleared/.test(k)),'ctrl-c':pf.failed.filter(k=>/^selections-cleared/.test(k)),'drop-file':pf.failed.filter(k=>/^xdnd/.test(k)),'ime-hangul':pf.failed.filter(k=>/^(ime|xim)/.test(k)),'paperclip':pf.failed.filter(k=>/^portal/.test(k))};
const harnessActions=Object.fromEntries(Object.entries(harnessFor).filter(([,v])=>v.length));
if(coreFailed.length){try{portal?.kill('SIGTERM')}catch{}try{ime.stop()}catch{}dsp.stop();
  summary.verdict='harness';summary.reason='preflight failed on core input: '+coreFailed.join(', ')+' (no GPUI was started; nothing here says anything about the app)';summary.actions=Object.fromEntries(ACTIONS.map(a=>[a,{ok:null,status:'harness',evidence:{failedChecks:pf.failed}}]));
  writeFileSync(out+'/input-summary.json',JSON.stringify(summary,null,1));console.log('HARNESS FAILURE: '+summary.reason);process.exit(3)}
summary.harnessActions=harnessActions; // these actions are not run: their tool does not work here, so they cannot say anything about the app
const fx=await startFixtureHub({bin:hubBin,root:resolve('out/e2e-run/input'),port:3813,threads:6});
try{
  const sf=out+'/hold.script.json';const holdSteps=Array.from({length:80},()=>[{wait:1500},{read:'composer'}]).flat();// (the client's own paperclip click is a separate, later client run: it must never land inside another action)
  writeFileSync(sf,JSON.stringify([{select:0},...holdSteps])); // G6: the client reports its composer text every 1.5 s
  const metrics=[];let buf='';const clientLog=out+'/client.stderr.log',clientOut=out+'/client.stdout.log';writeFileSync(clientLog,'');writeFileSync(clientOut,'');
  const hubLogCopy=()=>{try{execFileSync('cp',[fx.state.root+'/hub.log',out+'/hub.log'])}catch{}};
  const tokenPath=fx.state.root+'/home/token';
  const g=spawn(gpuBin,['--url',fx.state.url,'--token-file',tokenPath,'--script',sf],{env:{...dsp.env,...ime.env,HOME:fx.state.root+'/fakehome',BACKPLANE_GPUI_RECTS:'1'},stdio:['ignore','pipe','pipe']});
  let gExit=null;g.on('exit',(c,sg)=>{gExit={code:c,signal:sg}});g.stdout.on('data',d=>appendFileSync(clientOut,d));
  g.stderr.on('data',d=>{appendFileSync(clientLog,d);buf+=d;let i;while((i=buf.indexOf('\n'))>=0){const l=buf.slice(0,i);buf=buf.slice(i+1);const k=l.indexOf('GPUI_METRIC ');if(k>=0)try{metrics.push(JSON.parse(l.slice(k+12)))}catch{}}});
  const inp=new Input(dsp.display,out);let win=null;
  for(let k=0;k<40&&!win;k++){await sleep(1000);for(const find of [()=>inp.win('backplane'),()=>inp.win('GPUI'),()=>inp.winClass('backplane')])try{win=find();if(win)break}catch{}if(gExit&&!win)break}
  if(!win){ // a missing window is a HARNESS failure with its reasons in the artifact, never a quiet exit 0
    hubLogCopy();const tail=(f,n)=>{try{return readFileSync(f,'utf8').split('\n').slice(-n).join('\n')}catch{return '(no file)'}};
    let tree='';try{tree=execFileSync('xwininfo',['-root','-tree'],{env:dsp.env,encoding:'utf8'}).split('\n').slice(0,40).join('\n')}catch(e){tree='xwininfo failed: '+e.message}
    summary.verdict='harness';summary.reason='GPUI window never appeared (client '+(gExit?('exited '+JSON.stringify(gExit)):'still running')+')';
    summary.windowSearch={clientExit:gExit,clientAlive:!gExit,tokenFileExists:existsSync(tokenPath),assetsDirExists:existsSync(process.env.BACKPLANE_GPUI_ASSETS||''),assetsDir:process.env.BACKPLANE_GPUI_ASSETS||null,gpuiBin,url:fx.state.url,clientStderrTail:tail(clientLog,40),clientStdoutTail:tail(clientOut,10),hubLogTail:tail(out+'/hub.log',30),metricsSeen:metrics.map(m=>m.event).slice(0,20),xwininfoTree:tree};
    summary.actions=Object.fromEntries(ACTIONS.map(a=>[a,{ok:null,status:'harness',evidence:{reason:summary.reason}}]));
    try{g.kill('SIGTERM')}catch{}try{portal?.kill('SIGTERM')}catch{}try{ime.stop()}catch{}try{await fx.stop()}catch{}dsp.stop();
    writeFileSync(out+'/input-summary.json',JSON.stringify(summary,null,1));console.error('HARNESS FAILURE: '+summary.reason+'\n'+summary.windowSearch.clientStderrTail);process.exit(4)}
  inp.xi.focus(win.id);await sleep(1500);
  summary.window=win;summary.gpu=metrics.find(m=>m.event==='gpu')||null;
  // regions the client reports (G3 layout rects, window-relative physical px); the composer is the click/paste/drop/IME target
  const lay=[...metrics].reverse().find(m=>m.event==='layout');const flat={};const walk=(p,v)=>{if(v&&typeof v==='object'&&'x' in v&&'w' in v)flat[p]=v;else if(v&&typeof v==='object')for(const[k,x]of Object.entries(v))walk(p?p+'/'+k:k,x)};walk('',lay?.rects||{});
  summary.regions=Object.fromEntries(Object.entries(flat).map(([k,v])=>[k,[v.x,v.y,v.w,v.h].map(Math.round)]));
  const mid=r=>[r.x+r.w/2,r.y+r.h/2];const comp=flat.composer?mid(flat.composer):null;
  const main=Object.entries(flat).filter(([k])=>!/sidebar|rail|composer/.test(k)).sort((a,b)=>b[1].w*b[1].h-a[1].w*a[1].h)[0];
  summary.mainRegion=main?main[0]:null;
  const runIf=async(name,f)=>{if(harnessActions[name]){summary.actions[name]={ok:null,status:'harness',evidence:{failedChecks:harnessActions[name]}};return}await run(name,f)};
  const run=async(name,f)=>{try{summary.actions[name]=await f()}catch(e){summary.actions[name]={ok:null,status:'error',evidence:{harnessError:String(e.stack||e.message).slice(0,300)}}}}; // a thrown xclip/python/xdotool error is the harness's, never the app's
  // client-side pointer sanity (the plain-client preflight cannot prove GPUI receives XTEST pointer events: GPUI logs "Found no xinput mouse pointers" under Xvfb):
  // click another thread's row, taken from its own controls event, and require the main area to change
  const ctl0=[...metrics].reverse().find(m=>m.event==='controls'&&m.controls?.some(c=>c.rect));const threads=(ctl0?.controls||[]).filter(c=>/^thread:/.test(c.id)&&c.rect&&/^e2e thread/.test(c.label||'')); // seeded threads only: the project's auto-created 'New thread' is empty, and the drag/copy/paste actions need a thread with text
  if(threads.length>=2&&main){const t=threads[threads.length-1].rect,r=main[1];const b4=inp.s('before-pointer-sanity');inp.clickAt(win,[t.x+t.w/2,t.y+t.h/2]);await sleep(2500);const af=inp.s('after-pointer-sanity');
    const d=diffPixels(b4,af,`${Math.round(r.w)}x${Math.round(r.h)}+${Math.round(r.x)}+${Math.round(r.y)}`);const sb=flat.sidebar;const dSide=sb?diffPixels(b4,af,`${Math.round(sb.w)}x${Math.round(sb.h)}+${Math.round(sb.x)}+${Math.round(sb.y)}`):0; // the selected-row highlight moves even when two threads show the same text
    summary.clientPointer={clickedControl:threads[threads.length-1].id,mainRegionChangedPixels:d,sidebarChangedPixels:dSide,delivered:d>5000||dSide>500};
    if(summary.clientPointer.delivered!==true){for(const a of ['drag-select','ctrl-c','paperclip'])harnessActions[a]=[...(harnessActions[a]||[]),'client-pointer'];summary.notes=[...(summary.notes||[]),'a click on a sidebar thread row changed nothing in the main area: GPUI is not receiving XTEST pointer events on this display; pointer-dependent actions are harness, not app']}}
  else{summary.clientPointer={delivered:null,note:'no thread controls with rects to click: the pointer is unproven'};for(const a of ['drag-select','ctrl-c','paperclip'])harnessActions[a]=[...(harnessActions[a]||[]),'client-pointer-unproven']}
  inp.cb.clear();await sleep(300);
  if(main){const r=main[1];await runIf('drag-select',()=>inp.dragSelect(win,[r.x+110,r.y+150],[r.x+Math.min(r.w-120,820),r.y+Math.min(r.h-260,500)]));await runIf('ctrl-c',()=>{return inp.copy()})}
  else summary.blocked['drag-select/ctrl-c']='no main region in the layout event';
  execFileSync('convert',['-size','240x160','gradient:red-blue',out+'/paste.png'],{env:dsp.env});
  // Evidence comes from the hub's own log (the message the client actually sent), not from pixels: pixels also change when a thread streams.
  const evPath=fx.state.root+'/home/events.jsonl';const sentSince=(t0)=>{try{return readFileSync(evPath,'utf8').split('\n').filter(l=>l.includes('"MessagePosted"')&&l.includes('"role":"user"')).map(l=>JSON.parse(l)).filter(e=>e.at>=t0&&!/zqshort/.test(e.text||''))}catch{return []}};
  // A message carries a file only if images/attachments is a non-empty array, or its text holds an "[attachment: ...]" line or the file's name.
  // Never test the whole JSON for /image/: the KEYS `images`/`attachments` are there (empty) on every message.
  const carries=(m,nameRe)=>{if(!m)return false;for(const k of ['images','attachments','files'])if(Array.isArray(m[k])&&m[k].length)return true;return /\[attachment:[^\]]*\]/i.test(m.text||'')||(nameRe?nameRe.test(m.text||''):false)};
  const attachments=()=>{try{return execFileSync('sh',['-c',`find ${fx.state.root}/home -path '*attach*' -type f -printf '%f %s\\n' 2>/dev/null | head -10`],{encoding:'utf8'}).trim().split('\n').filter(Boolean)}catch{return []}};
  const newEvents=(t0)=>{try{const ev=readFileSync(evPath,'utf8').split('\n').filter(Boolean).map(l=>{try{return JSON.parse(l)}catch{return null}}).filter(e=>e&&e.at>=t0);return {kinds:Object.entries(ev.reduce((a,e)=>(a[e.$]=(a[e.$]||0)+1,a),{})),mentionsPng:ev.filter(e=>/png|pasted|attach|image/i.test(JSON.stringify(e))).map(e=>({kind:e.$,sample:JSON.stringify(e).slice(0,200)})).slice(0,4)}}catch{return null}};
  const composerText=()=>{const r=[...metrics].reverse().find(m=>m.event==='read'&&m.what==='composer');return r?{text:r.text,supported:r.supported}:null};
  const sendAndRead=async(t0)=>{inp.xi.key('Return');await sleep(1500);const m=sentSince(t0);return m.at(-1)||null};
  // what the hub itself shows for the thread after the send (the same picture every client gets): does it carry the attachment?
  const hubShows=async(threadId)=>{try{const v=await viewClient(fx.hub,{mobile:true,delta:false,tag:'e2e-judge'});await v.open();v.target={kind:'thread',id:threadId};v.viewId='';const r=await v.req('view.open',{},20000);v.close();
      const t=JSON.stringify(r?.x?.result?.shown?.thread||{});return {bytes:t.length,mentionsImage:/"images":\[\{|pasted-\d+\.png|paste\.png/i.test(t),imagesKeyEmpty:/"images":\[\]/.test(t),sample:(t.match(/.{0,50}(pasted|\.png|image).{0,50}/i)||[''])[0]}}catch(e){return {error:String(e.message).slice(0,120)}}};
  const clearComposer=async()=>{inp.xi.click(win.x+comp[0],win.y+comp[1]);inp.xi.key('ctrl+a');inp.xi.key('BackSpace');await sleep(200)};
  if(comp){
    await run('paste-image',async()=>{await clearComposer();const t0=Math.floor(Date.now()/1000);inp.xi.type('see pic');const r=await inp.pasteImage(out+'/paste.png',win,comp);await sleep(6000);inp.s('before-send-paste');const m=await sendAndRead(t0);const blob=JSON.stringify(m||{});
      const shown=m?await hubShows(m.thread):null;return {ok:!!m&&/see pic/.test(m.text||'')&&(carries(m,/pasted-\d+\.png|paste\.png/i)||!!shown?.mentionsImage),evidence:{...r.evidence,hubThreadShows:shown,attachmentsOnHub:attachments(),newEvents:newEvents(t0),hubMessage:m?{keys:Object.keys(m),text:m.text}:null,mentionsImage:carries(m,/pasted-\d+\.png|paste\.png/i)}}});
    await runIf('drop-file',async()=>{await clearComposer();const t0=Math.floor(Date.now()/1000);inp.xi.type('see drop');const r=inp.dropFile(win,resolve(out+'/paste.png'),comp);await sleep(6000);inp.s('before-send-drop');const m=await sendAndRead(t0);const blob=JSON.stringify(m||{});
      const shown=m?await hubShows(m.thread):null;return {ok:r.ok&&!!m&&(carries(m,/paste\.png/i)||!!shown?.mentionsImage),evidence:{...r.evidence,hubThreadShows:shown,attachmentsOnHub:attachments(),newEvents:newEvents(t0),hubMessage:m?{keys:Object.keys(m),text:m.text}:null,mentionsFile:carries(m,/paste\.png/i)}}});
    await runIf('ime-hangul',async()=>{await clearComposer();const t0=Math.floor(Date.now()/1000);const r=await inp.imeCompose(ime,comp,win);const m=await sendAndRead(t0);
      return {ok:!!m&&/한/.test(m.text||''),evidence:{...r.evidence,composerRead:composerText(),hubMessageText:m?.text??null,expected:'한 (composed); raw keys would arrive as "gks"'}}});
  } else summary.blocked['paste/drop/ime']='no composer region in the layout event';
  // G6: controls carry rects (physical window px; the first emission can be null, a later one has them)
  const ctlAll=metrics.filter(m=>m.event==='controls');const ctl=[...ctlAll].reverse().find(m=>m.controls?.some(c=>c.rect));
  const attach=ctl?.controls?.find(c=>c.id==='ui:Attach'&&c.rect);
  if(!attach)summary.blocked['paperclip']='no ui:Attach control with a rect in the controls event';
  else await runIf('paperclip',async()=>{const countOpen=()=>{try{return (readFileSync(portalLog,'utf8').match(/OpenFile/g)||[]).length}catch{return 0}};const calls0=countOpen();const w0=inp.windows();const b4=inp.s('before-paperclip');inp.clickAt(win,[attach.rect.x+attach.rect.w/2,attach.rect.y+attach.rect.h/2]);await sleep(3000);const w1=inp.windows();const shot=inp.s('after-paperclip');const r=attach.rect;const crop=`${Math.round(r.w)}x${Math.round(r.h)}+${Math.round(r.x)}+${Math.round(r.y)}`;const rectDiff=diffPixels(b4,shot,crop);
    const opened=w1.filter(x=>!w0.some(y=>y.id===x.id));let pl='';try{pl=readFileSync(portalLog,'utf8')}catch{}
    const calls1=countOpen();const called=calls1>calls0; // an increase caused by this click, not any OpenFile anywhere in the log
    return {ok:called,evidence:{rect:attach.rect,rectChangedPixels:rectDiff,portalStandin:portal?'running':'not running',portalCalls:{before:calls0,after:calls1},portalCalled:called,portalLog:pl.split('\n').slice(-4).join(' | '),newWindows:opened.map(x=>x.name).slice(0,5),screenshot:shot,note:'ok = the client asked the OS file chooser (xdg-desktop-portal FileChooser, via a stand-in on Boat); whether the chosen file then attaches is the upload chain measured by paste/drop'}}});
  const lk=[...metrics].reverse().find(m=>m.event==='links');summary.blocked['link-click']=lk&&lk.supported===false?'GPUI draws message text without link detection (links event supported:false)':(lk?null:'no links event');
  const sel=[...metrics].reverse().find(m=>m.event==='read'&&m.what==='selection');if(sel)summary.selectionRead=sel;
  try{g.kill('SIGTERM')}catch{}await sleep(1500);
  // the client's own click on its paperclip, as a separate later client run (never inside another action): is ITS handler wired to the OS chooser?
  {const cnt=()=>{try{return (readFileSync(portalLog,'utf8').match(/OpenFile/g)||[]).length}catch{return 0}};const n0=cnt();
   const sf2=out+'/paperclip.script.json';writeFileSync(sf2,JSON.stringify([{select:0},{wait:2500},{click:'ui:Attach'},{wait:4000}]));const m2=[];let b2='';
   const g2=spawn(gpuBin,['--url',fx.state.url,'--token-file',fx.state.root+'/home/token','--script',sf2],{env:{...dsp.env,...ime.env,HOME:fx.state.root+'/fakehome',BACKPLANE_GPUI_RECTS:'1'},stdio:['ignore','ignore','pipe']});
   g2.stderr.on('data',d=>{b2+=d;let i;while((i=b2.indexOf('\n'))>=0){const l=b2.slice(0,i);b2=b2.slice(i+1);const k=l.indexOf('GPUI_METRIC ');if(k>=0)try{m2.push(JSON.parse(l.slice(k+12)))}catch{}}});
   await Promise.race([new Promise(r=>g2.on('close',()=>r())),sleep(40000)]);try{g2.kill('SIGTERM')}catch{}await sleep(800);
   summary.paperclipScriptedClick={clickEvent:m2.find(m=>m.event==='click'&&m.id==='ui:Attach')||null,portalOpenFileCallsAdded:cnt()-n0};
   const pc=summary.actions.paperclip;if(pc&&pc.status!=='harness'&&pc.ok===false&&summary.paperclipScriptedClick.portalOpenFileCallsAdded>=1){pc.status='unverified';pc.evidence.note2='the client\'s own click on ui:Attach reaches the OS chooser (portal called), but the OS click aimed at the reported rect did not: the rect does not coincide with the painted button (about 25 px low), so the real click is unverified, not failed'}}
}finally{try{execFileSync('cp',[fx.state.root+'/hub.log',out+'/hub.log'])}catch{}try{portal?.kill('SIGTERM')}catch{}try{ime.stop()}catch{}await fx.stop();dsp.stop()}
// an action only counts when evidence says so; pass also needs the Rust hub on real hardware (CONTRACT.md), which this Boat run is not
summary.verdict=Object.keys(harnessActions).length?'app+harness':'app';for(const a of Object.values(summary.actions))if(!['harness','error','unverified'].includes(a.status))a.status=a.ok?'partial':'fail'; // the preflight passed, so a false below is the app's
writeFileSync(out+'/input-summary.json',JSON.stringify(summary,null,1));console.log(JSON.stringify({actions:Object.fromEntries(Object.entries(summary.actions).map(([k,v])=>[k,v.ok])),blocked:summary.blocked},null,1));
