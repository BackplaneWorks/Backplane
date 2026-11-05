// The must-pass GPUI input actions with real OS input on Xvfb (Boat): drag-select, Ctrl-C, paste an image, drop a file, IME, paperclip, link click.
// bun test/gpui/e2e/input-run.mjs --hub-bin PATH --gpui-bin PATH [--out DIR]   (Boat only; see boat-run.sh input-run)
import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';import {spawn,execFileSync} from 'node:child_process';import {resolve} from 'node:path';
import {startDisplay,installMissing} from './lib/capture.mjs';import {startFixtureHub} from './lib/hubfx.mjs';import {Input} from './input/driver.mjs';import {startIme} from './input/ime.mjs';
const arg=(k,d)=>{const i=process.argv.indexOf('--'+k);return i>=0?process.argv[i+1]:d};const sleep=ms=>new Promise(r=>setTimeout(r,ms));
const hubBin=resolve(arg('hub-bin')),gpuBin=resolve(arg('gpui-bin')),out=resolve(arg('out','out/e2e/input'));mkdirSync(out,{recursive:true});
const summary={when:new Date().toISOString(),actions:{},blocked:{},env:installMissing()};
const dsp=await startDisplay(64);const ime=await startIme(dsp.display); // before the client, so it can connect to it through XIM
summary.ime={status:ime.status,engine:ime.engine};const fx=await startFixtureHub({bin:hubBin,root:resolve('out/e2e-run/input'),port:3813,threads:6});
try{
  const sf=out+'/hold.script.json';writeFileSync(sf,JSON.stringify([{select:0},{wait:120000}]));
  const metrics=[];let buf='';const g=spawn(gpuBin,['--url',fx.state.url,'--token-file',fx.state.root+'/home/token','--script',sf],{env:{...dsp.env,...ime.env,HOME:fx.state.root+'/fakehome',BACKPLANE_GPUI_RECTS:'1'},stdio:['ignore','ignore','pipe']});
  g.stderr.on('data',d=>{buf+=d;let i;while((i=buf.indexOf('\n'))>=0){const l=buf.slice(0,i);buf=buf.slice(i+1);const k=l.indexOf('GPUI_METRIC ');if(k>=0)try{metrics.push(JSON.parse(l.slice(k+12)))}catch{}}});
  await sleep(6000);const inp=new Input(dsp.display,out);const win=inp.win('backplane');inp.xi.focus(win.id);await sleep(500);
  summary.window=win;summary.gpu=metrics.find(m=>m.event==='gpu')||null;
  // regions the client reports (G3 layout rects, window-relative physical px); the composer is the click/paste/drop/IME target
  const lay=[...metrics].reverse().find(m=>m.event==='layout');const flat={};const walk=(p,v)=>{if(v&&typeof v==='object'&&'x' in v&&'w' in v)flat[p]=v;else if(v&&typeof v==='object')for(const[k,x]of Object.entries(v))walk(p?p+'/'+k:k,x)};walk('',lay?.rects||{});
  summary.regions=Object.fromEntries(Object.entries(flat).map(([k,v])=>[k,[v.x,v.y,v.w,v.h].map(Math.round)]));
  const mid=r=>[r.x+r.w/2,r.y+r.h/2];const comp=flat.composer?mid(flat.composer):null;
  const main=Object.entries(flat).filter(([k])=>!/sidebar|rail|composer/.test(k)).sort((a,b)=>b[1].w*b[1].h-a[1].w*a[1].h)[0];
  summary.mainRegion=main?main[0]:null;
  const run=async(name,f)=>{try{summary.actions[name]=await f()}catch(e){summary.actions[name]={ok:false,evidence:{error:String(e.message).slice(0,200)}}}};
  if(main){const r=main[1];await run('drag-select',()=>inp.dragSelect(win,[r.x+40,r.y+60],[r.x+Math.min(r.w-40,500),r.y+110]));await run('ctrl-c',()=>inp.copy())}
  else summary.blocked['drag-select/ctrl-c']='no main region in the layout event';
  execFileSync('convert',['-size','240x160','gradient:red-blue',out+'/paste.png'],{env:dsp.env});
  // Evidence comes from the hub's own log (the message the client actually sent), not from pixels: pixels also change when a thread streams.
  const evPath=fx.state.root+'/home/events.jsonl';const sentSince=(t0)=>{try{return readFileSync(evPath,'utf8').split('\n').filter(l=>l.includes('"MessagePosted"')&&l.includes('"role":"user"')).map(l=>JSON.parse(l)).filter(e=>e.at>=t0&&!/zqshort/.test(e.text||''))}catch{return []}};
  const attachments=()=>{try{return execFileSync('sh',['-c',`find ${fx.state.root}/home -path '*attach*' -type f -printf '%f %s\\n' 2>/dev/null | head -10`],{encoding:'utf8'}).trim().split('\n').filter(Boolean)}catch{return []}};
  const newEvents=(t0)=>{try{const ev=readFileSync(evPath,'utf8').split('\n').filter(Boolean).map(l=>{try{return JSON.parse(l)}catch{return null}}).filter(e=>e&&e.at>=t0);return {kinds:Object.entries(ev.reduce((a,e)=>(a[e.$]=(a[e.$]||0)+1,a),{})),mentionsPng:ev.filter(e=>/png|pasted|attach|image/i.test(JSON.stringify(e))).map(e=>({kind:e.$,sample:JSON.stringify(e).slice(0,200)})).slice(0,4)}}catch{return null}};
  const sendAndRead=async(t0)=>{inp.xi.key('Return');await sleep(1500);const m=sentSince(t0);return m.at(-1)||null};
  const clearComposer=async()=>{inp.xi.click(win.x+comp[0],win.y+comp[1]);inp.xi.key('ctrl+a');inp.xi.key('BackSpace');await sleep(200)};
  if(comp){
    await run('paste-image',async()=>{await clearComposer();const t0=Math.floor(Date.now()/1000);inp.xi.type('see pic');const r=await inp.pasteImage(out+'/paste.png',win,comp);await sleep(6000);inp.s('before-send-paste');const m=await sendAndRead(t0);const blob=JSON.stringify(m||{});
      return {ok:!!m&&/png|image|pasted/i.test(blob)&&/see pic/.test(m.text||''),evidence:{...r.evidence,attachmentsOnHub:attachments(),newEvents:newEvents(t0),hubMessage:m?{keys:Object.keys(m),text:m.text}:null,mentionsImage:/png|image|pasted/i.test(blob)}}});
    await run('drop-file',async()=>{await clearComposer();const t0=Math.floor(Date.now()/1000);inp.xi.type('see drop');const r=inp.dropFile(win,resolve(out+'/paste.png'),comp);await sleep(6000);inp.s('before-send-drop');const m=await sendAndRead(t0);const blob=JSON.stringify(m||{});
      return {ok:r.ok&&!!m&&/paste\.png|png|image/i.test(blob),evidence:{...r.evidence,attachmentsOnHub:attachments(),newEvents:newEvents(t0),hubMessage:m?{keys:Object.keys(m),text:m.text}:null,mentionsFile:/paste\.png/i.test(blob)}}});
    await run('ime-hangul',async()=>{await clearComposer();const t0=Math.floor(Date.now()/1000);const r=await inp.imeCompose(ime,comp,win);const m=await sendAndRead(t0);
      return {ok:!!m&&/한/.test(m.text||''),evidence:{...r.evidence,hubMessageText:m?.text??null,expected:'한 (composed); raw keys would arrive as "gks"'}}});
  } else summary.blocked['paste/drop/ime']='no composer region in the layout event';
  // the paperclip and links need their rectangles: the controls event has none yet (request G6)
  const ctl=[...metrics].reverse().find(m=>m.event==='controls');const attach=ctl?.controls?.find(c=>/attach/i.test(c.label+c.id));
  summary.blocked['paperclip']=attach&&attach.rect?null:'controls event has no rect (G6): cannot aim a click at the paperclip';
  summary.blocked['link-click']='no link rectangles or link events (G6)';
  if(attach?.rect){const r=attach.rect;await run('paperclip',async()=>{inp.s('before-paperclip');inp.clickAt(win,[r.x+r.w/2,r.y+r.h/2]);await sleep(800);return {ok:true,evidence:{shot:inp.s('after-paperclip')}}});delete summary.blocked.paperclip}
  try{g.kill('SIGTERM')}catch{}
}finally{try{ime.stop()}catch{}await fx.stop();dsp.stop()}
// an action only counts when evidence says so; pass also needs the Rust hub on real hardware (CONTRACT.md), which this Boat run is not
for(const a of Object.values(summary.actions))a.status=a.ok?'partial':'fail';
writeFileSync(out+'/input-summary.json',JSON.stringify(summary,null,1));console.log(JSON.stringify({actions:Object.fromEntries(Object.entries(summary.actions).map(([k,v])=>[k,v.ok])),blocked:summary.blocked},null,1));
