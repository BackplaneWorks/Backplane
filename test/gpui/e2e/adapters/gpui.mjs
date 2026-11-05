// GPUI adapter: today's script contract (select, popup, type, send, wait) + GPUI_METRIC lines. Anything beyond is `blocked: G<n>` (CONTRACT.md).
import {spawn} from 'node:child_process';import {writeFileSync,mkdirSync,readFileSync,readdirSync} from 'node:fs';
import {sleep} from '../../perf/lib.mjs';
import {clickStats,frameStats,layoutShift,deadControls,rssSlope} from '../lib/frames.mjs';
import {changedFrames,acks,toEpochMs,fps as videoFps} from '../lib/video-frames.mjs';
import {startRecording,screenshot} from '../lib/capture.mjs';
export const name='gpui';
// capability -> request id in CONTRACT.md (what must land for it); null = exists today
export const capabilities={select:null,popup:null,type:null,send:null,wait_reply:null,watch_stream:'G2',agents:null,memory:null,
  pair:'G5',scroll:'G5',paste_image:'G5',bots:'G5',rooms:'G5',viewer_board:'G5',viewer_schematic:'G5',viewer_3d:'G5',files:'G5',download:'G5',settings:'G5',network_sleep:'G5',controls:'G4',frames:'G2',layout:'G3',ack:'G1'};
const rd=p=>{try{return readFileSync(p,'utf8')}catch{return ''}};
function toScript(j){const out=[];const blocked=new Set();
  for(const s of j.steps){switch(s.do){
    case 'select_thread':out.push({select:s.index},{wait:400});break;
    case 'switch_burst':for(let i=0;i<s.count;i++)out.push({select:i%20},{wait:s.gap_ms});break;
    case 'popup':out.push({popup:s.trigger},{wait:400});break;
    case 'type':out.push({type:s.text},{wait:200});break;
    case 'send':out.push({send:true},{wait:300});break;
    case 'wait_reply':case 'watch_stream':out.push({wait:(s.secs||10)*1000});break;
    case 'expect':case 'agents':break;
    case 'session':{const secs=(Number(process.env.E2E_SESSION_MIN)||s.minutes)*60;let r=7;const rnd=()=>(r=(r*1103515245+12345)&0x7fffffff)/0x7fffffff;for(let t=0;t<secs;){const i=Math.floor(rnd()*20);out.push({select:i},{wait:700});t+=1;if(rnd()<0.5){out.push({popup:['/','@','>','%','$'][Math.floor(rnd()*5)]},{wait:600});t+=1}else{out.push({wait:1500});t+=1.5}}break}
    case 'paste_image':case 'scroll':case 'pair':case 'open_project':case 'open_bots':case 'open_bot':case 'open_rooms':case 'open_room':case 'post_room':case 'open_viewer':case 'pan_zoom':case 'orbit':case 'open_files':case 'download':case 'open_settings':case 'click_every_control':case 'sleep_network':case 'wake_network':
      blocked.add(capabilities[{open_project:'select',paste_image:'paste_image',scroll:'scroll',pair:'pair',open_bots:'bots',open_bot:'bots',open_rooms:'rooms',open_room:'rooms',post_room:'rooms',open_viewer:'viewer_board',pan_zoom:'viewer_board',orbit:'viewer_3d',open_files:'files',download:'download',open_settings:'settings',click_every_control:'controls',sleep_network:'network_sleep',wake_network:'network_sleep'}[s.do]]||'G5');break;
    default:blocked.add('unknown step '+s.do)}}
  return {script:out,blocked:[...blocked].filter(Boolean)}}
export async function run(j,ctx){ // ctx: {gpuBin, display:{env,display}, hub:{state}, outDir, hz, record}
  const {script,blocked}=toScript(j);
  const res={journey:j.id,client:'gpui',status:'blocked',blocked,metrics:{},notes:[]};
  if(blocked.length){res.notes.push('needs '+blocked.join(', ')+' (test/gpui/e2e/CONTRACT.md)');if(!script.some(s=>s.select!==undefined||s.popup!==undefined))return res}
  mkdirSync(ctx.outDir,{recursive:true});const sf=`${ctx.outDir}/${j.id}.script.json`;writeFileSync(sf,JSON.stringify(script));
  const rec=ctx.record?startRecording(ctx.display.display,`${ctx.outDir}/${j.id}.mp4`):null;
  const metrics=[];let buf='';
  const g=spawn(ctx.gpuBin,['--url',ctx.hub.state.url,'--token-file',ctx.hub.state.root+'/home/token','--script',sf],{env:{...ctx.display.env,HOME:ctx.hub.state.root+'/fakehome'},stdio:['ignore','ignore','pipe']});
  g.stderr.on('data',d=>{buf+=d;let i;while((i=buf.indexOf('\n'))>=0){const l=buf.slice(0,i);buf=buf.slice(i+1);const k=l.indexOf('GPUI_METRIC ');if(k>=0)try{metrics.push({...JSON.parse(l.slice(k+12)),wall:Date.now()})}catch{}}});
  const memSamples=[];const t00=Date.now();const memTimer=setInterval(()=>{try{const sm=readFileSync(`/proc/${g.pid}/smaps_rollup`,'utf8');const k=n=>Number(new RegExp(n+':\\s+(\\d+)').exec(sm)?.[1]||0);memSamples.push({t_s:(Date.now()-t00)/1000,rssMB:k('Rss')/1024,pssMB:k('Pss')/1024})}catch{}},j.measure.includes('memory')?30000:5000);
  const done=await Promise.race([new Promise(r=>g.on('exit',()=>r(true))),sleep((script.reduce((a,s)=>a+(s.wait||600),0)+60000)).then(()=>false)]);
  if(!done){try{g.kill('SIGTERM')}catch{}res.notes.push('client did not finish its script in time')}
  clearInterval(memTimer);const offsetMs=rec?.offsetMs?.();if(rec)res.capture=(await rec.stop()).file;res.screenshot=screenshot(ctx.display.display,`${ctx.outDir}/${j.id}.png`);
  const inputs=metrics.filter(m=>m.event==='input');res.metrics.failedSteps=inputs.filter(m=>!m.ok).map(m=>m.step+(m.error?' ('+m.error+')':''));const gpu=metrics.find(m=>m.event==='gpu');
  res.gpu=gpu?{device:gpu.device,software:gpu.software}:null;res.metrics.inputs=inputs.length;res.metrics.failed=inputs.filter(m=>!m.ok).length;
  // `input.latency_ms` is the RESULT time; without G1 there is no ack time, so the "within one frame" claim is not made
  // inject epoch = completion epoch - result latency (GPUI stamps both on its own clock = this machine's)
  const okIn=inputs.filter(m=>m.ok);const injectEpoch=okIn.map(m=>m.epoch_ms-m.latency_ms);
  let ackMs=okIn.map(()=>NaN),resolution=Infinity,framesChanged=null;
  if(res.capture&&Number.isFinite(offsetMs)){try{const thr=Number(process.env.E2E_SCENE)||0.0003;const ch=changedFrames(res.capture,thr);const chEpoch=toEpochMs(ch,offsetMs);res.metrics.video={changedFrames:ch.length,threshold:thr,recordingOffsetEpochMs:Math.round(offsetMs),firstChangedEpochMs:Math.round(chEpoch[0]||0),firstInjectEpochMs:Math.round(injectEpoch[0]||0),lastChangedEpochMs:Math.round(chEpoch.at(-1)||0),fps:videoFps(res.capture)};ackMs=acks(chEpoch,injectEpoch);resolution=Math.max(1000/(videoFps(res.capture)||30),0)+30;framesChanged=chEpoch}catch(e){res.notes.push('video analysis failed: '+String(e.message).slice(0,120))}}
  const clicks=okIn.map((m,i)=>({id:m.step+'#'+i,t_inject:0,t_result:m.latency_ms,t_ack:ackMs[i]}));
  res.metrics.click=clickStats(clicks,ctx.hz,0,resolution);res.metrics.click.source=Number.isFinite(resolution)?`video-derived ack (x11grab, software render), resolution ~${resolution.toFixed(0)} ms`:'result latency only (no ack evidence)';
  if(framesChanged&&framesChanged.length>10&&j.measure.some(x=>x==='stream'||x==='scroll')){const w=framesChanged.filter(t=>t>=(injectEpoch[0]||0));res.metrics.frames=frameStats(w,60);res.metrics.frames.source='changed frames in the recording (content motion cadence, not GPU presents)'}
  if(memSamples.length>=3){res.metrics.memory={samples:memSamples.length,rssMBlast:memSamples.at(-1).rssMB,pssMBlast:memSamples.at(-1).pssMB,slope:rssSlope(memSamples,Math.min(300,memSamples.at(-1).t_s/4))}}
  if(!inputs.length)res.notes.push('no input metrics (script did not run?)');
  res.status=blocked.length?'blocked':(res.metrics.failed===0&&inputs.length>0&&done?'partial':'fail');
  if(res.status==='partial')res.notes.push('layout shifts and dead controls need G3/G4; click ack and frames here are video-derived at software-render resolution, so no pass is claimed');
  if(res.gpu?.software)res.notes.push('software-rendered ('+res.gpu.device+'): not GPU frame evidence');
  return res}
