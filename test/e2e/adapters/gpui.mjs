// GPUI adapter: today's script contract (select, popup, type, send, wait) + GPUI_METRIC lines. Anything beyond is `blocked: G<n>` (CONTRACT.md).
import {spawn} from 'node:child_process';import {writeFileSync,mkdirSync,readFileSync,readdirSync} from 'node:fs';
import {sleep} from '../../perf/lib.mjs';
import {clickStats,frameStats,layoutShift,deadControls,rssSlope} from '../lib/frames.mjs';
import {changedFrames,acks,toEpochMs,fps as videoFps} from '../lib/video-frames.mjs';
import {startRecording,screenshot} from '../lib/capture.mjs';
export const name='gpui';
// capability -> request id in CONTRACT.md (what must land for it); null = exists today
export const capabilities={select:null,popup:null,type:null,send:null,wait_reply:null,watch_stream:'G2',agents:null,memory:null,
  pair:'G5',scroll:null,paste_image:'G5',bots:'G5',rooms:'G5',viewer_board:'G5',viewer_schematic:'G5',viewer_3d:'G5',files:'G5',download:'G5',settings:'G5',network_sleep:null,controls:null,frames:null,layout:null,ack:null}; // G1-G4, scroll and net_sleep landed in c373ceb37; the rest answer not_implemented
const rd=p=>{try{return readFileSync(p,'utf8')}catch{return ''}};
function toScript(j){const out=[];const blocked=new Set();let sweep=false;
  for(const s of j.steps){switch(s.do){
    case 'select_thread':out.push({select:s.index},{wait:400});break;
    case 'switch_burst':for(let i=0;i<s.count;i++)out.push({select:i%20},{wait:s.gap_ms});break;
    case 'popup':out.push({popup:s.trigger},{wait:400});break;
    case 'type':out.push({type:s.text},{wait:200});break;
    case 'send':out.push({send:true},{wait:300});break;
    case 'wait_reply':case 'watch_stream':out.push({wait:(s.secs||10)*1000});break;
    case 'open_project':out.push({home:0},{wait:400});break;
    case 'expect':case 'agents':case 'wake_network':break;
    case 'scroll':for(let i=0;i<(s.screens||3)*4;i++)out.push({scroll:(i<(s.screens||3)*2?1:-1)*220},{wait:120});break;
    case 'sleep_network':out.push({net_sleep:s.secs||45},{wait:500});break;
    case 'click_every_control':sweep=true;break;
    case 'session':{const secs=(Number(process.env.E2E_SESSION_MIN)||s.minutes)*60;let r=7;const rnd=()=>(r=(r*1103515245+12345)&0x7fffffff)/0x7fffffff;for(let t=0;t<secs;){const i=Math.floor(rnd()*20);out.push({select:i},{wait:700});t+=1;if(rnd()<0.5){out.push({popup:['/','@','>','%','$'][Math.floor(rnd()*5)]},{wait:600});t+=1}else{out.push({wait:1500});t+=1.5}}break}
    case 'paste_image':case 'pair':case 'open_bots':case 'open_bot':case 'open_rooms':case 'open_room':case 'post_room':case 'open_viewer':case 'pan_zoom':case 'orbit':case 'open_files':case 'download':case 'open_settings':
      blocked.add(capabilities[{open_project:'select',paste_image:'paste_image',scroll:'scroll',pair:'pair',open_bots:'bots',open_bot:'bots',open_rooms:'rooms',open_room:'rooms',post_room:'rooms',open_viewer:'viewer_board',pan_zoom:'viewer_board',orbit:'viewer_3d',open_files:'files',download:'download',open_settings:'settings',click_every_control:'controls',}[s.do]]||'G5');break;
    default:blocked.add('unknown step '+s.do)}}
  return {script:out,blocked:[...blocked].filter(Boolean),sweep}}

// one client process over a script: returns {metrics (all GPUI_METRIC), done, offsetMs, capture, screenshot, memSamples}
async function launch(script,j,ctx,tag,{record=false,rects=false}={}){
  const sf=`${ctx.outDir}/${j.id}${tag}.script.json`;writeFileSync(sf,JSON.stringify(script));
  const rec=record?startRecording(ctx.display.display,`${ctx.outDir}/${j.id}${tag}.mp4`):null;
  const metrics=[];let buf='';const memSamples=[];const t00=Date.now();
  let scriptDone;const sd=new Promise(r=>{scriptDone=r}); // the client quits itself after script_done: BACKPLANE_GPUI_HOLD keeps it up so the screenshot shows the window
  const g=spawn(ctx.gpuBin,['--url',ctx.hub.state.url,'--token-file',ctx.hub.state.root+'/home/token','--script',sf],{env:{...ctx.display.env,HOME:ctx.hub.state.root+'/fakehome',BACKPLANE_GPUI_HOLD:'1',...(rects?{BACKPLANE_GPUI_RECTS:'1'}:{})},stdio:['ignore','ignore','pipe']});
  g.stderr.on('data',d=>{buf+=d;let i;while((i=buf.indexOf('\n'))>=0){const l=buf.slice(0,i);buf=buf.slice(i+1);const k=l.indexOf('GPUI_METRIC ');if(k>=0)try{const m=JSON.parse(l.slice(k+12));metrics.push({...m,wall:Date.now()});if(m.event==='script_done')scriptDone(true)}catch{}}});
  const memTimer=setInterval(()=>{try{const sm=readFileSync(`/proc/${g.pid}/smaps_rollup`,'utf8');const k=n=>Number(new RegExp(n+':\\s+(\\d+)').exec(sm)?.[1]||0);memSamples.push({t_s:(Date.now()-t00)/1000,rssMB:k('Rss')/1024,pssMB:k('Pss')/1024})}catch{}},j.measure.includes('memory')?30000:5000);
  const done=await Promise.race([new Promise(r=>g.on('close',()=>r(true))),sd,sleep(script.reduce((a,x)=>a+(x.wait||600)+(x.net_sleep?x.net_sleep*1000:0),0)+90000).then(()=>false)]);
  const screenshot=screenshot_(ctx,`${j.id}${tag}`); // before the client goes away (a screenshot after it quit is just the i3 bar)
  try{g.kill('SIGTERM')}catch{}
  clearInterval(memTimer);const offsetMs=rec?.offsetMs?.();const capture=rec?(await rec.stop()).file:null;
  return {metrics,done,offsetMs,capture,screenshot,memSamples}}
const screenshot_=(ctx,n)=>screenshot(ctx.display.display,`${ctx.outDir}/${n}.png`);
const flatRects=r=>{const o={};const walk=(p,v)=>{if(v&&typeof v==='object'&&'x' in v&&'w' in v)o[p]=v;else if(v&&typeof v==='object')for(const [k,x] of Object.entries(v))walk(p?p+'/'+k:k,x)};walk('',r);return o};
export async function run(j,ctx){ // ctx: {gpuBin, display:{env,display}, hub:{state}, outDir, hz, record}
  const {script,blocked,sweep}=toScript(j);
  const res={journey:j.id,client:'gpui',status:'blocked',blocked,metrics:{},notes:[]};
  if(blocked.length){res.notes.push('needs '+blocked.join(', ')+' (test/e2e/CONTRACT.md)');if(!script.some(s=>s.select!==undefined||s.popup!==undefined||s.home!==undefined))return res}
  mkdirSync(ctx.outDir,{recursive:true});
  const wantFrames=j.measure.some(x=>x==='stream'||x==='scroll'),wantLayout=j.measure.includes('layout')||sweep,wantRec=ctx.record||wantFrames;
  // G2: frame timestamps over the whole script
  const body=wantFrames?[{record:'start',name:j.id},...script,{record:'stop'}]:script;
  const first=await launch(body,j,ctx,'',{record:wantRec,rects:wantLayout||sweep});
  const {metrics,done,offsetMs}=first;res.capture=first.capture;res.screenshot=first.screenshot;
  const inputs=metrics.filter(m=>m.event==='input');const gpu=metrics.find(m=>m.event==='gpu');
  res.gpu=gpu?{device:gpu.device,software:gpu.software}:null;res.metrics.inputs=inputs.length;res.metrics.failed=inputs.filter(m=>!m.ok).length;
  res.metrics.failedSteps=inputs.filter(m=>!m.ok).map(m=>m.step+(m.error?' ('+m.error+')':''));
  // G1: the ack event pairs with the input event of the same step; ack_ms and latency_ms share GPUI's clock (both from the injected event)
  const acksBy=new Map();for(const m of metrics.filter(m=>m.event==='ack'))if(!acksBy.has(m.step))acksBy.set(m.step,m.ack_ms);
  const okIn=inputs.filter(m=>m.ok);
  const clicks=okIn.map((m,i)=>({id:m.step+'#'+i,t_inject:0,t_result:m.latency_ms,t_ack:acksBy.has(m.step)?acksBy.get(m.step):NaN}));
  res.metrics.click=clickStats(clicks,ctx.hz,0,0);res.metrics.click.source='GPUI ack event (G1): first presented frame after the injected event';
  // fall back to video only if the client gave no ack at all
  if(!acksBy.size&&first.capture&&Number.isFinite(offsetMs)){try{const thr=Number(process.env.E2E_SCENE)||0.00005;const ch=changedFrames(first.capture,thr);const chE=toEpochMs(ch,offsetMs);const inj=okIn.map(m=>m.epoch_ms-m.latency_ms);const a=acks(chE,inj);const res_=1000/(videoFps(first.capture)||30)+30;res.metrics.click=clickStats(okIn.map((m,i)=>({id:m.step+'#'+i,t_inject:0,t_result:m.latency_ms,t_ack:a[i]})),ctx.hz,0,res_);res.metrics.click.source=`video-derived (no ack event), resolution ~${res_.toFixed(0)} ms`}catch(e){res.notes.push('video analysis failed: '+String(e.message).slice(0,100))}}
  // G2: frame times over the scripted scroll/stream
  const fr=metrics.find(m=>m.event==='frames');const frT=fr&&(fr.frames_ms||fr.times_ms||(Array.isArray(fr.t_ms)?fr.t_ms:null));
  if(fr&&!frT)res.notes.push('frames event has no timestamp array: script.rs metric() overwrites the payload field t_ms with the event time; G2 needs the array under another name (frames_ms)');
  if(frT?.length>10){res.metrics.frames=frameStats(frT,60);res.metrics.frames.source='GPUI frames event (G2): every drawn frame while recording'}
  else if(wantFrames)res.notes.push('no frames event (G2) in this run');
  // G3: layout snapshots; inputT = time of the latest input event before the snapshot (same clock, t_ms)
  const lay=metrics.filter(m=>m.event==='layout');
  if(lay.length>=1){const inT=inputs.map(m=>m.t_ms);const snaps=lay.map(m=>{const prior=inT.filter(t=>t<=m.t_ms);return {t:m.t_ms,inputT:prior.length?prior[prior.length-1]:null,rects:flatRects(m.rects)}});
    res.metrics.layout=layoutShift(snaps,{w:1440,h:900});res.metrics.layout.snapshots=snaps.length;res.metrics.layout.note='GPUI emits a layout event only when a rectangle changed: one snapshot means stable'}
  else if(wantLayout)res.notes.push('no layout event at all (G3 needs BACKPLANE_GPUI_RECTS, set for this run): layout shift unmeasured');
  if(first.memSamples.length>=3){res.metrics.memory={samples:first.memSamples.length,rssMBlast:first.memSamples.at(-1).rssMB,pssMBlast:first.memSamples.at(-1).pssMB,slope:rssSlope(first.memSamples,Math.min(300,first.memSamples.at(-1).t_s/4))}}
  // G4: dead-control sweep: phase 2 clicks every control the first run reported on its final screen
  if(sweep){const ctlEv=[...metrics].reverse().find(m=>m.event==='controls'&&m.controls?.length);
    if(!ctlEv)res.notes.push('no controls event (G4) to sweep');
    else{const inv=ctlEv.controls.map(c=>({id:c.id,label:c.label,enabled:c.enabled,visible:c.visible,wired:c.wired,disabledLook:c.disabled_look===true}));
      const prefix=script.filter(x=>!x.wait);const sweepScript=[...script.filter(x=>x.select!==undefined||x.home!==undefined).slice(-1),{wait:500},...inv.flatMap(c=>[{click:c.id},{wait:300}])];
      const second=await launch(sweepScript,j,ctx,'.sweep',{rects:true});const results={};
      // a control the click step could not find in the second run (rows of a streaming thread have new ids every few seconds) is transient, not dead
      const gone=new Set(second.metrics.filter(m=>m.event==='click'&&m.found===false).map(m=>m.id));const transient=inv.filter(c=>gone.has(c.id));if(gone.size)inv.splice(0,inv.length,...inv.filter(c=>!gone.has(c.id)));
      for(const m of second.metrics.filter(m=>m.event==='click'&&m.found!==false))results[m.id]={changed:!!m.changed,action:m.action!=null};
      for(const m of second.metrics.filter(m=>m.event==='input'&&m.step?.startsWith('click:')&&m.ok)){const id=m.step.slice(6);results[id]={changed:true,action:results[id]?.action||false}}
      res.metrics.dead=deadControls(inv,results);res.metrics.dead.controlsFound=inv.length;res.metrics.dead.transientSkipped=transient.length;
      // Controls with no handler: drawn disabled (disabled_look) = 'disabled, not dead'; drawn enabled = dead. The gate item says every visible
      // button must do something, so the sweep's pass is false while ANY visible control has no handler; `passIfDisabledAccepted` is the other reading (the reviewer's call).
      const noHandler=inv.filter(c=>c.wired===false&&c.visible!==false);
      const disabledNoHandler=noHandler.filter(c=>c.disabledLook).map(c=>({id:c.id,label:c.label}));const deadLooksEnabled=noHandler.filter(c=>!c.disabledLook).map(c=>({id:c.id,label:c.label}));
      res.metrics.dead.disabledNoHandler=disabledNoHandler;res.metrics.dead.deadLooksEnabled=deadLooksEnabled;res.metrics.dead.inventory=inv.map(c=>({id:c.id,label:c.label,wired:c.wired,disabledLook:c.disabledLook}));
      const wiredDead=res.metrics.dead.dead.length;res.metrics.dead.passIfDisabledAccepted=wiredDead===0&&deadLooksEnabled.length===0&&res.metrics.dead.clickable>0;
      res.metrics.dead.pass=res.metrics.dead.passIfDisabledAccepted&&disabledNoHandler.length===0;
      if(noHandler.length)res.metrics.dead.note=`${noHandler.length} visible controls have no handler: ${disabledNoHandler.length} drawn disabled, ${deadLooksEnabled.length} drawn enabled (dead). Pass stays false while any exists; whether disabled-looking counts as not dead is the reviewer's decision.`}}
  if(!inputs.length)res.notes.push('no input metrics (script did not run?)');
  if(!done)res.notes.push('client did not finish its script in time');
  // status: pass only when every measured gate for this journey passes (the pass rule in run.mjs still demotes non-Rust/non-hardware runs)
  const gates=[];if(j.measure.includes('click'))gates.push(res.metrics.click.pass);if(res.metrics.frames)gates.push(res.metrics.frames.pass);if(res.metrics.layout)gates.push(res.metrics.layout.pass);if(res.metrics.dead)gates.push(res.metrics.dead.pass);if(j.measure.includes('memory')&&res.metrics.memory)gates.push(res.metrics.memory.slope.pass);
  const needed=[j.measure.includes('click'),j.measure.some(x=>x==='stream'||x==='scroll'),j.measure.includes('layout'),j.measure.includes('dead'),j.measure.includes('memory')];
  const ok=res.metrics.failed===0&&inputs.length>0&&done;
  res.status=blocked.length?'blocked':!ok?'fail':gates.includes(false)?'fail':gates.includes(null)||gates.includes(undefined)||gates.length<needed.filter(Boolean).length?'partial':'pass';
  if(res.gpu?.software)res.notes.push('software-rendered ('+res.gpu.device+'): not GPU frame evidence');
  return res}
