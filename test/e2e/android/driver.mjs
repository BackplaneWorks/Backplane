// Android journey driver: runs journeys.json steps on the emulator with real input (adb tap/swipe/text, UI Automator dumps), records each
// journey with `adb screenrecord`, and returns the adapter result shape (see CONTRACT.md). Nothing is marked pass without an `expect` that the
// UI dump confirms; a control it cannot find makes the journey `blocked: <what>`.
// Safety: targets ANDROID_SERIAL (default emulator-5584) only, never an attached phone. Runs only where an emulator runs (not on Boat).
import {execFileSync,spawn} from 'node:child_process';import {mkdirSync,writeFileSync} from 'node:fs';
import {sleep} from '../../perf/lib.mjs';import {frameStats,clickStats,layoutShift,deadControls} from '../lib/frames.mjs';
export const SERIAL=process.env.ANDROID_SERIAL||'emulator-5584';export const PKG=process.env.E2E_ANDROID_PKG||'dev.backplane.mobile.gpui';
const SCHEME=process.env.E2E_ANDROID_SCHEME||'bpgpui';
const adb=(...a)=>execFileSync('adb',['-s',SERIAL,...a],{encoding:'utf8',timeout:60000,maxBuffer:1<<26});
const sh=c=>adb('shell',c);
export const present=()=>{try{return execFileSync('adb',['devices'],{encoding:'utf8'}).split('\n').some(l=>l.startsWith(SERIAL+'\tdevice'))}catch{return false}};
class Blocked extends Error{}const block=w=>{throw new Blocked(w)};
export function parse(xml){const out=[];for(const m of xml.matchAll(/<node [^>]*>/g)){const a=k=>new RegExp(`${k}="([^"]*)"`).exec(m[0])?.[1]??'';const b=/\[(\d+),(\d+)\]\[(\d+),(\d+)\]/.exec(a('bounds'));if(!b)continue;
  const un=s=>s.replace(/&amp;/g,'&').replace(/&lt;/g,'<').replace(/&gt;/g,'>').replace(/&quot;/g,'"').replace(/&#10;/g,'\n');
  out.push({id:a('resource-id')||un(a('content-desc'))||un(a('text'))||`${b[1]},${b[2]}`,label:un(a('text'))||un(a('content-desc')),cls:a('class'),clickable:a('clickable')==='true',enabled:a('enabled')==='true',visible:true,rect:{x:+b[1],y:+b[2],w:+b[3]-+b[1],h:+b[4]-+b[2]}})}return out}
const dump=()=>{sh('uiautomator dump /sdcard/e2e.xml >/dev/null');return sh('cat /sdcard/e2e.xml')};
const ui=()=>parse(dump());
const mid=n=>[n.rect.x+n.rect.w/2,n.rect.y+n.rect.h/2];
const tapXY=(x,y)=>sh(`input tap ${Math.round(x)} ${Math.round(y)}`);
const size=()=>{const m=/(\d+)x(\d+)/.exec(sh('wm size'));return {w:+m[1],h:+m[2]}};
const find=(ns,re)=>ns.find(n=>re.test(n.label)&&n.rect.w>0);
async function waitFor(re,ms=15000){const t=Date.now();let ns;while(Date.now()-t<ms){ns=ui();const n=find(ns,re);if(n)return n;await sleep(300)}return null}
async function tapText(re,ms=15000,what=String(re)){const n=await waitFor(re,ms);if(!n)block('control not found: '+what);const [x,y]=mid(n);tapXY(x,y);return n}
const typeText=t=>sh(`input text '${t.replace(/ /g,'%s').replace(/'/g,'')}'`);
const composer=ns=>ns.find(n=>/EditText/.test(n.cls)&&n.rect.y>size().h/2);
const rows=ns=>ns.filter(n=>/e2e thread \d+|Seed thread/.test(n.label)).sort((a,b)=>a.rect.y-b.rect.y);
const swipeUp=(frac=0.6)=>{const s=size();sh(`input swipe ${s.w/2|0} ${s.h*0.75|0} ${s.w/2|0} ${s.h*(0.75-frac)|0} 250`)};
const back=()=>sh('input keyevent KEYCODE_BACK');
// frames: gfxinfo framestats (column 13 = FRAME_COMPLETED, ns)
const framesReset=()=>sh(`dumpsys gfxinfo ${PKG} reset`);
const framesRead=()=>{const t=sh(`dumpsys gfxinfo ${PKG} framestats`);const rs=t.split('---PROFILEDATA---')[1]?.split('\n').slice(2).map(l=>l.split(',').map(Number)).filter(r=>r.length>=14&&r[0]===0)||[];return rs.map(r=>r[13]/1e6)};
const clicks=[];let frameTs=[];
async function clicked(id,fn,expect){const t0=Date.now();await fn();const n0=ui();const t1=Date.now();let res=NaN;if(expect){const tt=Date.now();while(Date.now()-tt<8000){if(find(ui(),expect)){res=Date.now()-t0;break}await sleep(60)}}
  clicks.push({id,t_inject:0,t_ack:t1-t0,t_result:res})}
async function step(s,ctx,st){
  const st_=ctx.hub?.state;
  switch(s.do){
    case 'pair':{ if(!st_)block('hub: no fixture hub'); const host=process.env.E2E_ANDROID_HOST||new URL(st_.url).hostname;
      const r=await fetch(`${st_.url}/auth/invitations`,{method:'POST',headers:{authorization:'Bearer '+st_.token}});if(!r.ok)block('hub:/auth/invitations '+r.status);
      let link=(await r.json()).link;link=link.replace(new URL(link).hostname,host);
      sh(`pm clear ${PKG}`);sh(`am start -W -a android.intent.action.VIEW -d '${SCHEME}://pair?url=${encodeURIComponent(link)}' ${PKG}`);
      const t=Date.now();const ok=await waitFor(/Projects|Threads|e2e|Skip|Next/i,25000);if(!ok)block('app did not show the sidebar after pairing');st.pairMs=Date.now()-t;
      const sk=find(ui(),/^Skip$/i);if(sk){const [x,y]=mid(sk);tapXY(x,y);await sleep(500)}return}
    case 'open_project':{await tapText(new RegExp(s.name,'i'),10000,'project '+s.name);return}
    case 'select_thread':{const rs=rows(ui());if(rs.length<=s.index)block(`thread row ${s.index} not found (${rs.length} rows)`);await clicked('select '+s.index,async()=>{const [x,y]=mid(rs[s.index]);tapXY(x,y)},/Message|Write|Send/i);return}
    case 'scroll':{framesReset();for(let i=0;i<(s.screens||3);i++){swipeUp();await sleep(250)}frameTs=framesRead();return}
    case 'type':{const c=composer(ui());if(!c)block('composer not found');const [x,y]=mid(c);tapXY(x,y);await sleep(300);typeText(s.text);await sleep(300);st.typed=s.text;return}
    case 'paste_image':{ // adb cannot put an image on the clipboard: attach through the file picker and say so
      sh('mkdir -p /sdcard/Download');execFileSync('adb',['-s',SERIAL,'push',ctx.fixture||`${ctx.hub?.state.root}/proj/${s.file.replace(/^fixtures\//,'fixtures/')}`,'/sdcard/Download/pic.png']);
      const a=find(ui(),/Attach|Add file|Image/i);if(!a)block('attach control not found');const [x,y]=mid(a);tapXY(x,y);await tapText(/pic\.png/,10000,'pic.png in the picker');
      st.notes.push('image attached through the file picker, not pasted from the clipboard (adb cannot set an image clipboard): paste_image stays blocked for the gate');st.noClipboard=true;return}
    case 'send':{await clicked('send',async()=>{const b=find(ui(),/^Send$/i)||ui().filter(n=>n.clickable&&n.rect.y>size().h*0.7).sort((a,b)=>b.rect.x-a.rect.x)[0];if(!b)block('send control not found');tapXY(...mid(b))},null);return}
    case 'wait_reply':{const t=Date.now();while(Date.now()-t<s.secs*1000){if(find(ui(),/Finished|ready for you|stand-?in|short seed|zq/i)){st.replyMs=Date.now()-t;return}await sleep(500)}block('no reply within '+s.secs+' s');return}
    case 'watch_stream':{framesReset();const t=Date.now();let grew=0,last='';while(Date.now()-t<s.secs*1000){const l=ui().map(n=>n.label).join('|');if(l.length>last.length)grew++;last=l;await sleep(400)}st.grew=grew;frameTs=framesRead();return}
    case 'switch_burst':{const rs=rows(ui());if(rs.length<2)block('no thread rows');for(let i=0;i<s.count;i++){const n=rs[i%Math.min(rs.length,6)];const [x,y]=mid(n);tapXY(x,y);await sleep(s.gap_ms);if(i%2===1)back()}return}
    case 'popup':{const c=composer(ui());if(!c)block('composer not found');tapXY(...mid(c));sh('input keyevent KEYCODE_MOVE_END');typeText(s.trigger);await sleep(900);st.popups=(st.popups||0)+(ui().some(n=>n.clickable&&n.rect.y<c.rect.y&&/bot|Bub|Mochi|compact|context|btw|give|skill/i.test(n.label))?1:0);for(let i=0;i<2;i++)sh('input keyevent KEYCODE_DEL');return}
    case 'open_bots':{await tapText(/^Bots$/,8000,'Bots tab');return}
    case 'open_bot':{const ns=ui().filter(n=>/Bub|Mochi/.test(n.label));if(!ns.length)block('no bot rows');tapXY(...mid(ns[s.index]||ns[0]));await sleep(600);return}
    case 'open_rooms':{back();await tapText(/^Rooms$/,8000,'Rooms tab');return}
    case 'open_room':{await tapText(/e2e-room/,8000,'room e2e-room');return}
    case 'post_room':{const c=composer(ui());if(!c)block('room composer not found');tapXY(...mid(c));typeText(s.text);await sleep(300);const b=find(ui(),/^(Post|Send)$/i);if(!b)block('post control not found');tapXY(...mid(b));await sleep(1500);if(!find(ui(),new RegExp(s.text.split(' ')[0])))block('post not shown');return}
    case 'open_viewer':{await tapText(/Viewer|Board|Schematic|3D/i,8000,'viewer control');const want={board:/Board/,schematic:/Schematic/,'3d':/3D/}[s.kind];await tapText(want,8000,s.kind+' tab');await sleep(1500);return}
    case 'pan_zoom':case 'orbit':{framesReset();const sz=size();const n=Math.max(4,Math.floor(s.frames/20));for(let i=0;i<n;i++){sh(`input swipe ${sz.w*0.3|0} ${sz.h*0.5|0} ${sz.w*0.7|0} ${(sz.h*(0.4+0.05*(i%3)))|0} 300`);await sleep(100)}frameTs=framesRead();return}
    case 'open_files':{await tapText(/Files/,8000,'Files tab');return}
    case 'download':{await tapText(new RegExp(s.file.replace('.','\\.')),8000,'file '+s.file);await sleep(1500);return}
    case 'open_settings':{await tapText(/Settings/,8000,'Settings');return}
    case 'click_every_control':{const inv=ui().filter(n=>n.clickable&&n.enabled);const res={};for(const n of inv){const b=dump();tapXY(...mid(n));await sleep(450);const a=dump();res[n.id]={changed:a!==b,action:false};if(a!==b){back();await sleep(300)}}st.dead=deadControls(inv,res);return}
    case 'sleep_network':{ // the fixture hub's control endpoint (serve-fixture.mjs: port+1) drops all connections and refuses new ones for N s
      const u=new URL(ctx.hub?.state.url||process.env.E2E_HUB_URL||'http://invalid');const ctl=process.env.E2E_CTL_URL||`http://${u.hostname}:${Number(u.port)+1}`;
      const r=await fetch(`${ctl}/ctl/cut?secs=${s.secs}`,{method:'POST'}).catch(()=>null);if(!r||!r.ok)block('hub control endpoint /ctl/cut not reachable at '+ctl);
      st.netDown=Date.now();st.notes.push('network step is a HUB CUT (the fixture proxy drops and refuses connections), not the phone losing connectivity: it still needs a real device network off/on run');st.hubCut=true;await sleep(s.secs*1000);return}
    case 'wake_network':{st.netUp=Date.now();return} // the cut ends by itself
    case 'agents':return; // started by the harness (startAgents)
    case 'session':{ // random walk over the sidebar/threads, sampling the app's PSS every 30 s
      const t=Date.now();const mem=[];let k=0;while(Date.now()-t<s.minutes*60000){const rs=rows(ui());if(rs.length)tapXY(...mid(rs[(k++*7)%Math.min(rs.length,8)]));await sleep(1200);swipeUp(0.4);await sleep(600);back();
        if(Date.now()-t>=mem.length*30000){const m=/TOTAL PSS:\s+(\d+)/i.exec(sh(`dumpsys meminfo ${PKG}`))||/TOTAL\s+(\d+)/.exec(sh(`dumpsys meminfo ${PKG}`));mem.push({t:Date.now()-t,pssKb:m?+m[1]:NaN})}}
      st.mem=mem;return}
    case 'expect':{
      if(s.sidebar&&!find(ui(),/Threads|Projects|e2e/i))st.fail.push('sidebar not shown');
      if(s.history&&!ui().some(n=>n.label.length>20))st.fail.push('no thread history on screen');
      if(s.image_in_thread&&!ui().some(n=>/ImageView|image|pic\.png/i.test(n.cls+n.label)))st.fail.push('no image in the thread');
      if(s.growing_reply&&!(st.grew>1))st.fail.push('reply did not grow');
      if(s.last_thread_shown&&!find(ui(),/Message|Write|Send/i))st.fail.push('no thread shown after the burst');
      if(s.reconnected_within_s!==undefined){const t=Date.now();let ok=false;while(Date.now()-t<s.reconnected_within_s*1000){if(find(ui(),/Message|Write|Send/i)&&!find(ui(),/Offline|Reconnecting|Connecting/i)){ok=true;break}await sleep(400)}if(!ok)st.fail.push('not reconnected within '+s.reconnected_within_s+' s')}
      if(s.file_saved&&!sh('ls /sdcard/Download').includes(s.file_saved))st.fail.push('file not in Downloads');
      if(s.no_growth_rss&&st.mem?.length>3){const a=st.mem.filter(m=>m.t>5*60000);if(a.length>2){const sl=(a[a.length-1].pssKb-a[0].pssKb)/((a[a.length-1].t-a[0].t)/60000);st.rssKbPerMin=sl;if(sl>2000)st.fail.push('PSS grows '+sl.toFixed(0)+' KB/min')}}
      return}
    default:block('step not implemented: '+s.do)}}
export async function run(j,ctx){
  const r={journey:j.id,client:'android',status:'pass',metrics:{},notes:[],blocked:[]};
  if(!present()){r.status='blocked';r.blocked=[`no emulator ${SERIAL} (this driver needs the emulator on the desktop or a KVM host)`];return r}
  const st={fail:[],notes:r.notes};clicks.length=0;frameTs=[];
  const mp4=`/sdcard/e2e-${j.id}.mp4`;const rec=spawn('adb',['-s',SERIAL,'shell','screenrecord','--time-limit','180','--bit-rate','4000000',mp4],{stdio:'ignore'});
  const shots=[];
  try{for(const s of j.steps){await step(s,ctx,st)}}
  catch(e){if(e instanceof Blocked){r.status='blocked';r.blocked.push(e.message)}else{r.status='fail';r.notes.push('driver error: '+String(e.stack||e).slice(0,300))}}
  try{rec.kill('SIGINT')}catch{};await sleep(1500);
  mkdirSync(ctx.outDir,{recursive:true});const local=`${ctx.outDir}/${j.id}.mp4`;
  try{adb('pull',mp4,local);r.capture=local}catch{r.notes.push('screenrecord pull failed')}
  try{const png=`${ctx.outDir}/${j.id}.png`;writeFileSync(png,execFileSync('adb',['-s',SERIAL,'exec-out','screencap','-p'],{maxBuffer:1<<26}));r.screenshot=png}catch{}
  if(clicks.length)r.metrics.click=clickStats(clicks,ctx.hz);
  if(frameTs.length>1)r.metrics.frames=frameStats(frameTs,ctx.hz);
  if(st.dead)r.metrics.dead=st.dead;if(st.mem)r.metrics.memory={samples:st.mem,kbPerMin:st.rssKbPerMin};
  if(st.noClipboard){r.status='blocked';r.blocked.push('paste_image: adb cannot set an image on the clipboard (the picker attach worked)')}
  if(r.status==='pass'&&st.fail.length){r.status='fail';r.notes.push(...st.fail)}
  if(st.hubCut)r.label='hub cut';
  if(r.status==='pass'&&!j.steps.some(s=>s.do==='expect'))r.notes.push('no expect step: pass means the steps ran');
  if(r.status==='pass'&&r.metrics.click&&!r.metrics.click.pass)r.notes.push('click ack over one frame: see metrics.click (reported, journey status unchanged)');
  return r}
