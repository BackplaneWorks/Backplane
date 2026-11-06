// GPUI adapter: today's script contract (select, popup, type, send, wait) + GPUI_METRIC lines. Anything beyond is `blocked: G<n>` (CONTRACT.md).
import {spawn} from 'node:child_process';import {writeFileSync,mkdirSync,readFileSync,readdirSync} from 'node:fs';
import {sleep} from '../../perf/lib.mjs';
import {clickStats,frameStats,layoutShift,deadControls,rssSlope} from '../lib/frames.mjs';
import {changedFrames,acks,toEpochMs,fps as videoFps} from '../lib/video-frames.mjs';
import {startRecording,screenshot} from '../lib/capture.mjs';
import {judgeSteps,stepSummary,DEFAULT_BUDGETS} from '../lib/report.mjs';
import {hubCounts,checkHub} from '../lib/hubcheck.mjs';
import {xinput,have} from '../input/xinput.mjs';
export const name='gpui';
// capability -> request id in CONTRACT.md (what must land for it); null = exists today. Checked against the GPUI script contract of
// native/gpui/src/script.rs on 2026-11-06: viewers open with a rail click (`click rail:Board`), Files and downloads are `files`/`files_download`,
// Settings is the gear control (`click ui:Settings`), bots and rooms are sidebar controls (`click bot:<name>`, `room:<name>`), the selected
// thread's own actions are view actions (`action [pin|settle|archive|snooze|interrupt|fork, value]`), a dropped/picked file is `attach PATH`.
// Still missing: an image on the clipboard (paste_image), pairing from the client, 3D orbit, rename by script, and every key binding unless
// xdotool drives the Xvfb display (keys).
const KEYS=have('xdotool');
export const capabilities={select:null,popup:null,type:null,send:null,wait_reply:null,watch_stream:null,agents:null,memory:null,
  pair:'G5',scroll:null,paste_image:'G5',attach:null,bots:null,rooms:null,viewer_board:null,viewer_schematic:null,viewer_3d:null,viewer_split:null,viewer_mech:null,viewer_review:null,viewer_diff:null,
  files:null,download:null,settings:null,network_sleep:null,controls:null,frames:null,layout:null,ack:null,thread_actions:null,history:null,probe:null,read:null,hub_ops:null,fs_ops:null,
  orbit:'G7',rename:'G7',keys:KEYS?null:'xdotool',model_picker:'G7',search:'G7',window_size:'G7',tailnet:'G7'}; // G1-G5 landed in c373ceb37 and after; G7 = requests in CONTRACT.md "Story steps"
const RAIL={board:'Board',schematic:'Schematic','3d':'3D',split:'Split',mech:'Mech',files:'Files',review:'Review',diff:'Diff',browser:'Browser'};
const BOTS=['Bub','Mochi']; // hubfx seeds these and the room e2e-room
// ops: harness actions the client cannot do itself (restart the hub, real keys, a file on disk); the script carries a `mark` and the
// adapter acts when the client reports it, so the act lands between the right steps
function toScript(j,ctx={}){const out=[];const blocked=new Set();let sweep=false;const ops=[];const expects=[];const copy=!!ctx.hub?.state?.copyOf;
  const R=v=>typeof v==='string'?v.replaceAll('$ROOT',ctx.hub?.state?.root||'.'):v; // a journey names fixture paths as $ROOT/...
  const op=(kind,o,wait)=>{ops.push({kind,...o});out.push({mark:`${kind}:${ops.length-1}`},{wait:wait??800})};
  for(const s of j.steps){switch(s.do){
    case 'select_thread':out.push({select:s.index},{wait:400});break;
    case 'switch_burst':for(let i=0;i<s.count;i++)out.push({select:i%(s.threads||20)},{wait:s.gap_ms});break;
    case 'popup':out.push({popup:s.trigger},{wait:400});break;
    case 'type':out.push({type:s.text},{wait:200});break;
    case 'send':out.push({send:true},{wait:300});break;
    case 'wait_reply':case 'watch_stream':out.push({wait:(s.secs||10)*1000});break;
    case 'open_project':out.push({home:s.index||0},{wait:400});break;
    case 'agents':case 'wake_network':break;
    case 'expect':if(s.hub)expects.push({hub:s.hub});if(s.scroll_dups!==undefined)expects.push({scroll_dups:s.scroll_dups});if(s.scroll_same)expects.push({scroll_same:s.scroll_same});if(s.composer!==undefined){out.push({read:'composer'});expects.push({composer:s.composer})}break;
    case 'scroll':for(let i=0;i<(s.screens||3)*4;i++)out.push({scroll:(i<(s.screens||3)*2?1:-1)*220*(s.up?-1:1)},{wait:120});break;
    case 'scroll_by':out.push({scroll:s.px},{wait:s.wait??150});break;
    case 'scroll_state':out.push({scroll_state:s.name||'here'});break;
    case 'sleep_network':out.push({net_sleep:s.secs||45},{wait:500});break;
    case 'click_every_control':sweep=true;break;
    case 'session':{const secs=(Number(process.env.E2E_SESSION_MIN)||s.minutes)*60;let r=7;const rnd=()=>(r=(r*1103515245+12345)&0x7fffffff)/0x7fffffff;for(let t=0;t<secs;){const i=Math.floor(rnd()*20);out.push({select:i},{wait:700});t+=1;if(rnd()<0.5){out.push({popup:['/','@','>','%','$'][Math.floor(rnd()*5)]},{wait:600});t+=1}else{out.push({wait:1500});t+=1.5}}break}
    // landed since CONTRACT.md's first G5 table
    case 'open_viewer':if(!RAIL[s.kind]){blocked.add('viewer_'+s.kind);break}out.push({click:'rail:'+RAIL[s.kind]},{wait:s.wait??2500});break;
    case 'close_viewer':out.push({open_viewer:''},{wait:600});break;
    case 'pan_zoom':{const n=Math.max(8,Math.floor((s.frames||120)/2));for(let i=0;i<n;i++)out.push({pan:[(i%20<10?14:-14),(i%40<20?4:-4)]},{wait:16});for(const z of [3,3,-3,-3])out.push({zoom:[z,0.5,0.5]},{wait:60});break}
    case 'fit':out.push({fit:true},{wait:300});break;
    case 'layers':out.push({layers:s.open!==false},{wait:400});break;
    case 'sheets':out.push({sheets:s.open!==false},{wait:400});break;
    case 'probe':out.push({tap:[s.fx??0.5,s.fy??0.5,s.pane??0]},{wait:600});break;
    case 'history':out.push(s.play?{histplay:true}:s.mode!==undefined?{histmode:s.mode}:{hist:s.step??null},{wait:s.wait??1500});break;
    case 'compare':out.push({compare:s.index??null},{wait:1500});break;
    case 'open_files':out.push({click:'rail:Files'},{wait:2000});break;
    case 'files':out.push({files:s.op},{wait:s.wait??800});break;
    case 'download':out.push({files_download:s.file},{wait:2000});break;
    case 'open_settings':out.push({click:'ui:Settings'},{wait:1500});break;
    case 'close_settings':out.push({click:'ui:Settings'},{wait:800});break;
    case 'open_bots':case 'open_rooms':break; // GPUI lists bots and rooms in the sidebar: nothing to open
    case 'open_bot':out.push({click:'bot:'+BOTS[s.index||0]},{wait:1200});break;
    case 'open_room':out.push({click:'room:'+(s.name||'e2e-room')},{wait:1200});break;
    case 'post_room':out.push({field:['rpost',s.text]},{wait:300},{action:['room-post','']},{wait:1500});break; // the room's own post box (screens/bots.rs), not the thread composer
    case 'attach':out.push({attach:s.file.startsWith('/')?s.file:(ctx.hub?.state?.root||'.')+'/proj/'+s.file},{wait:800});break;
    case 'thread_action':case 'view_action':out.push({action:[s.action,R(s.value||'')]},{wait:s.wait??800});break;
    case 'needs':blocked.add(s.what);break; // a capability no client script has yet (CONTRACT.md, "Story steps")
    case 'click':out.push({click:s.control},{wait:s.wait??600});break;
    case 'palette':out.push({click:'rail:Commands'},{wait:600});break;
    case 'field':out.push({field:[s.name,R(s.text)]},{wait:s.wait??300});break;
    case 'hub':if(s.op==='restart'||s.op==='down')op('hub',{op:s.op,secs:s.secs||0},(s.secs||0)*1000+(s.wait??8000));else blocked.add('hub_ops:'+s.op);break;
    case 'fs':if(copy)blocked.add('fs_ops (a copy run never writes into the copy\'s real project folders)');else op('fs',{op:s.op,from:s.from,to:s.to},s.wait??1500);break;
    case 'key':case 'type_keys':if(!KEYS)blocked.add('keys');else op('x',s.do==='key'?{key:s.combo}:{text:s.text,gap:s.gap_ms||20},s.wait??(s.do==='key'?400:(s.text||'').length*(s.gap_ms||20)+600));break;
    case 'paste_image':case 'pair':case 'orbit':case 'rename':case 'model_picker':case 'search':case 'window_size':case 'tailnet':
      blocked.add(({paste_image:'paste_image',pair:'pair',orbit:'orbit',rename:'rename',model_picker:'model_picker',search:'search',window_size:'window_size',tailnet:'tailnet'})[s.do]);break;
    default:blocked.add('unknown step '+s.do)}}
  return {script:out,blocked:[...blocked].filter(Boolean),sweep,ops,expects}}

// the act behind a `mark` (see toScript): a hub restart through hubfx, real keys through xdotool on this run's Xvfb, a file under the fixture root
async function doOp(o,ctx,notes){try{
  if(o.kind==='hub'){if(o.op==='restart'){const pid=await ctx.hub.restart(0);notes.push(`hub restarted (pid ${pid})`)}else{const pid=await ctx.hub.restart(o.secs*1000);notes.push(`hub down ${o.secs} s, back as pid ${pid}`)}}
  else if(o.kind==='x'){const x=xinput(ctx.display.display);if(o.key)x.key(o.key);else x.type(o.text,o.gap)}
  else if(o.kind==='fs'){const {cpSync,rmSync}=await import('node:fs');const root=ctx.hub.state.root+'/';const at=p=>{const r=(root+p).replace(/\/+/g,'/');if(!r.startsWith(root)||r.includes('/../'))throw Error('fs op outside the fixture root: '+p);return r};
    if(o.op==='copy')cpSync(at(o.from),at(o.to),{recursive:true});else if(o.op==='remove')rmSync(at(o.from),{recursive:true,force:true});else throw Error('fs op '+o.op)}
}catch(e){notes.push(`${o.kind} op failed: ${String(e.message).slice(0,160)}`)}}

// one client process over a script: returns {metrics (all GPUI_METRIC), done, offsetMs, capture, screenshot, memSamples}
async function launch(script,j,ctx,tag,{record=false,rects=false,ops=[],notes=[]}={}){
  const sf=`${ctx.outDir}/${j.id}${tag}.script.json`;writeFileSync(sf,JSON.stringify(script));
  const rec=record?startRecording(ctx.display.display,`${ctx.outDir}/${j.id}${tag}.mp4`):null;
  const metrics=[];let buf='';const memSamples=[];const t00=Date.now();
  let scriptDone;const sd=new Promise(r=>{scriptDone=r}); // the client quits itself after script_done: BACKPLANE_GPUI_HOLD keeps it up so the screenshot shows the window
  const g=spawn(ctx.gpuBin,['--url',ctx.hub.state.url,'--token-file',ctx.hub.state.root+'/home/token','--script',sf],{env:{...ctx.display.env,HOME:ctx.hub.state.root+'/fakehome',BACKPLANE_GPUI_HOLD:'1',...(rects?{BACKPLANE_GPUI_RECTS:'1'}:{})},stdio:['ignore','ignore','pipe']});
  g.stderr.on('data',d=>{buf+=d;let i;while((i=buf.indexOf('\n'))>=0){const l=buf.slice(0,i);buf=buf.slice(i+1);const k=l.indexOf('GPUI_METRIC ');if(k>=0)try{const m=JSON.parse(l.slice(k+12));metrics.push({...m,wall:Date.now()});if(m.event==='script_done')scriptDone(true);const mk=m.event==='mark'&&/^(hub|x|fs):(\d+)$/.exec(m.name||'');if(mk&&ops[+mk[2]])doOp(ops[+mk[2]],ctx,notes)}catch{}}});
  const memTimer=setInterval(()=>{try{const sm=readFileSync(`/proc/${g.pid}/smaps_rollup`,'utf8');const k=n=>Number(new RegExp(n+':\\s+(\\d+)').exec(sm)?.[1]||0);memSamples.push({t_s:(Date.now()-t00)/1000,rssMB:k('Rss')/1024,pssMB:k('Pss')/1024})}catch{}},j.measure.includes('memory')?30000:5000);
  const done=await Promise.race([new Promise(r=>g.on('close',()=>r(true))),sd,sleep(script.reduce((a,x)=>a+(x.wait||600)+(x.net_sleep?x.net_sleep*1000:0),0)+90000).then(()=>false)]);
  const screenshot=screenshot_(ctx,`${j.id}${tag}`); // before the client goes away (a screenshot after it quit is just the i3 bar)
  try{g.kill('SIGTERM')}catch{}
  clearInterval(memTimer);const offsetMs=rec?.offsetMs?.();const capture=rec?(await rec.stop()).file:null;
  return {metrics,done,offsetMs,capture,screenshot,memSamples}}
const screenshot_=(ctx,n)=>screenshot(ctx.display.display,`${ctx.outDir}/${n}.png`);
const flatRects=r=>{const o={};const walk=(p,v)=>{if(v&&typeof v==='object'&&'x' in v&&'w' in v)o[p]=v;else if(v&&typeof v==='object')for(const [k,x] of Object.entries(v))walk(p?p+'/'+k:k,x)};walk('',r);return o};
export async function run(j,ctx){ // ctx: {gpuBin, display:{env,display}, hub:{state}, outDir, hz, record}
  const {script,blocked,sweep,ops,expects}=toScript(j,ctx);
  const res={journey:j.id,client:'gpui',status:'blocked',blocked,metrics:{},notes:[]};
  if(blocked.length){res.notes.push('needs '+blocked.join(', ')+' (test/e2e/CONTRACT.md)');if(!script.some(s=>s.wait===undefined&&s.mark===undefined))return res} // the steps it does have still run: partial evidence, the status stays blocked
  mkdirSync(ctx.outDir,{recursive:true});
  const wantFrames=j.measure.some(x=>x==='stream'||x==='scroll'),wantLayout=j.measure.includes('layout')||sweep,wantRec=ctx.record||wantFrames;
  // G2: frame timestamps over the whole script
  const body=wantFrames?[{record:'start',name:j.id},...script,{record:'stop'}]:script;
  const hubWant=expects.filter(x=>x.hub);const before=hubWant.length&&ctx.hub?.hub?await hubCounts(ctx.hub).catch(()=>null):null;
  const first=await launch(body,j,ctx,'',{record:wantRec,rects:wantLayout||sweep,ops,notes:res.notes});
  const {metrics,done,offsetMs}=first;res.capture=first.capture;res.screenshot=first.screenshot;
  const inputs=metrics.filter(m=>m.event==='input');const gpu=metrics.find(m=>m.event==='gpu');
  res.gpu=gpu?{device:gpu.device,software:gpu.software}:null;res.metrics.inputs=inputs.length;res.metrics.failed=inputs.filter(m=>!m.ok).length;
  res.metrics.failedSteps=inputs.filter(m=>!m.ok).map(m=>m.step+(m.error?' ('+m.error+')':''));
  // per-press timing (lib/report.mjs): every step in order, its ack (first frame after it), its result and where the time went; a click on a
  // control the client does not have is a failed step, not a silent one
  {const ackQ=new Map();for(const m of metrics.filter(m=>m.event==='ack'))(ackQ.get(m.step)||ackQ.set(m.step,[]).get(m.step)).push(m.ack_ms);
    const raw=metrics.filter(m=>m.event==='input'||(m.event==='click'&&m.found===false)).map(m=>m.event==='click'?{step:'click:'+m.id,ok:false,ms:NaN,error:'no such control'}:
      {step:m.step,ok:m.ok!==false,ms:m.latency_ms,ackMs:ackQ.get(m.step)?.shift(),error:m.error,source:m.source,phases:m.hub_rtt_ms!==undefined?{hub_rtt_ms:m.hub_rtt_ms,decode_ms:m.decode_ms,process_ms:m.process_ms,apply_ms:m.apply_ms,render_build_ms:m.render_build_ms,applied_to_frame_ms:m.applied_to_frame_ms}:undefined});
    res.steps=judgeSteps(raw,ctx.budgets||DEFAULT_BUDGETS);res.stepSummary=stepSummary(res.steps);
    const missing=raw.filter(x=>x.error==='no such control');if(missing.length){res.metrics.failed+=missing.length;res.metrics.failedSteps.push(...missing.map(x=>x.step+' (no such control)'))}}
  // expectations the hub or the client's own read can prove
  res.expects=[];if(hubWant.length&&before){const after=await hubCounts(ctx.hub).catch(()=>null);if(after)for(const w of hubWant)res.expects.push(...checkHub(w.hub,before,after));else res.notes.push('hub counts unreadable after the run')}
  {const sc=metrics.filter(m=>m.event==='scroll');const at=n=>sc.find(m=>m.name===n);
    for(const w of expects.filter(x=>x.scroll_dups!==undefined)){const d=sc.reduce((a,m)=>a+(m.dups||0),0);res.expects.push({what:`no duplicated timeline rows (${sc.length} scroll reads)`,got:sc.length?`${d} dups`:'no scroll_state event',ok:sc.length>0&&d===w.scroll_dups})}
    for(const w of expects.filter(x=>x.scroll_same)){const [a,b]=w.scroll_same.map(at);res.expects.push({what:`the view holds between ${w.scroll_same.join(' and ')}`,got:a&&b?`top ${a.top}+${a.px} then ${b.top}+${b.px}`:'missing scroll_state',ok:!!a&&!!b&&a.top===b.top&&Math.abs(a.px-b.px)<2})}}
  {const reads=metrics.filter(m=>m.event==='read'&&m.what==='composer');let i=0;for(const w of expects.filter(x=>x.composer!==undefined)){const r=reads[i++];res.expects.push({what:`composer reads "${w.composer}"`,got:r?JSON.stringify(r.text):'no read event',ok:!!r&&r.text===w.composer})}}
  for(const m of metrics.filter(m=>m.event==='files'&&m.ok===false))res.notes.push(`files ${m.step}: ${m.error}`);
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
  const ok=res.metrics.failed===0&&inputs.length>0&&done&&!(res.expects||[]).some(x=>!x.ok)&&!(res.steps||[]).some(x=>x.verdict==='fail');
  res.status=blocked.length?'blocked':!ok?'fail':gates.includes(false)?'fail':gates.includes(null)||gates.includes(undefined)||gates.length<needed.filter(Boolean).length?'partial':'pass';
  if(res.gpu?.software)res.notes.push('software-rendered ('+res.gpu.device+'): not GPU frame evidence');
  return res}
