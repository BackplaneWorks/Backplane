// Electron (installed 0.13.0 desktop) vs GPUI on one fixture hub (3807), isolated display :62, RTX 5080 via Vulkan.
// bun cmp_run.mjs PHASE   PHASE: startup | idle | latency | load   (writes build/gpui/results/perf/cmp/<phase>.json)
import {readFileSync,writeFileSync,mkdirSync,rmSync,existsSync,readdirSync} from 'node:fs';
import {spawn,execSync} from 'node:child_process';
import {cdp} from './cdp.mjs';import {tree,pct,sleep} from './lib.mjs';
const REPO='/home/h/Documents/i2cjak/backplane-gpui';const ROOT=REPO+'/build/gpui/perf-run/cmp';const OUT=REPO+'/build/gpui/results/perf/cmp';mkdirSync(OUT,{recursive:true});
const st=JSON.parse(readFileSync(ROOT+'/state.json','utf8'));
const HOME=ROOT+'/home';const ELHOME=ROOT+'/fakehome-el',GPHOME=ROOT+'/fakehome-gp';mkdirSync(GPHOME,{recursive:true});
const CAP=['--user','--scope','--quiet','-p','MemoryMax=3700M','-p','MemorySwapMax=0'];
const NV='/usr/share/vulkan/icd.d/nvidia_icd.json';
const ELFLAGS=['--remote-debugging-port=9231','--ozone-platform=x11','--use-angle=vulkan','--enable-features=Vulkan,DefaultANGLEVulkan,VulkanFromANGLE','--ignore-gpu-blocklist','--disable-gpu-sandbox','--enable-gpu-rasterization','--enable-zero-copy'];
const rd=p=>{try{return readFileSync(p,'utf8')}catch{return ''}};
const phase=process.argv[2];const stat=a=>a.length?{n:a.length,p50:pct(a,50),p95:pct(a,95),max:Math.max(...a),min:Math.min(...a)}:null;
// ---- process tree metrics (/proc): RSS, PSS (smaps_rollup), private, utime+stime, fds
function snap(root){const pids=[root,...tree(root)];let rss=0,pss=0,priv=0,ticks=0,fds=0,n=0;for(const p of pids){const s=rd(`/proc/${p}/stat`);if(!s)continue;n++;const f=s.split(') ')[1].split(' ');ticks+=Number(f[11])+Number(f[12]);
  const sm=rd(`/proc/${p}/smaps_rollup`);const g=k=>Number(new RegExp(k+':\\s+(\\d+)').exec(sm)?.[1]||0);rss+=g('Rss');pss+=g('Pss');priv+=g('Private_Clean')+g('Private_Dirty');try{fds+=readdirSync(`/proc/${p}/fd`).length}catch{}}
 return {t:Date.now(),procs:n,rssMB:rss/1024,pssMB:pss/1024,privMB:priv/1024,ticks,fds}}
const gpuMem=()=>{try{return Number(execSync('nvidia-smi --query-gpu=memory.used --format=csv,noheader,nounits',{encoding:'utf8'}).trim())}catch{return NaN}};
async function sampleFor(root,secs){const s=[];const t0=Date.now();while(Date.now()-t0<secs*1000){s.push(snap(root));await sleep(1000)}
 const a=s[0],b=s.at(-1);return {secs:(b.t-a.t)/1000,cpuCorePct:(b.ticks-a.ticks)*10/(b.t-a.t)*100,rssMB:{median:pct(s.map(x=>x.rssMB),50),max:Math.max(...s.map(x=>x.rssMB))},pssMB:{median:pct(s.map(x=>x.pssMB),50),max:Math.max(...s.map(x=>x.pssMB))},privMB:{median:pct(s.map(x=>x.privMB),50)},procs:b.procs,fds:b.fds,gpuUsedMB:gpuMem()}}
async function killTree(root){const pids=[root,...tree(root)];try{process.kill(root,'SIGTERM')}catch{}for(let i=0;i<60;i++){if(!pids.some(p=>existsSync('/proc/'+p)))break;await sleep(200)}
 const left=pids.filter(p=>existsSync('/proc/'+p));for(const p of left){try{process.kill(p,'SIGKILL')}catch{}}await sleep(300);return left.length}
// ---- clients
async function launchEl({fresh}){if(fresh){rmSync(ELHOME,{recursive:true,force:true});rmSync(HOME+'/electron-profile',{recursive:true,force:true})}mkdirSync(ELHOME,{recursive:true});
 const spawnEpoch=Date.now();
 const p=spawn('systemd-run',[...CAP,'env','DISPLAY=:62','WAYLAND_DISPLAY=',`HOME=${ELHOME}`,`BACKPLANE_HOME=${HOME}`,'BACKPLANE_SERVICE_VERSION=0.14.0','BACKPLANE_NO_UPDATE=1',`VK_ICD_FILENAMES=${NV}`,process.env.HOME+'/.local/share/backplane/desktop/runtime/backplane-desktop',...ELFLAGS],{stdio:'ignore'});
 let c=null;let usableEpoch=null;const dl=Date.now()+60000;
 while(Date.now()<dl&&usableEpoch===null){try{if(!c)c=await cdp(9231,u=>u.includes(':3807'));if(await c.ev(`document.querySelectorAll('.thread[data-value]').length>0`))usableEpoch=Date.now()}catch{c=null}await sleep(20)}
 if(c&&usableEpoch!==null){ // dismiss the first-run tour once (persists in the hub's settings); not part of the timing
  for(let k=0,idle=0;k<14&&idle<12;k++){const p=await c.ev(`(()=>{const b=[...document.querySelectorAll('button')].find(e=>e.textContent.trim()==='Skip'&&e.offsetParent!==null);if(!b)return null;const r=b.getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2}})()`);if(!p){idle++;await sleep(250);k--;continue}idle=0;await click(c,p.x,p.y);await sleep(300)}}
 return {p,pid:p.pid,spawnEpoch,usableEpoch,c,usableMs:usableEpoch&&usableEpoch-spawnEpoch}}
function launchGp(script,exitAfter){const m={};const spawnEpoch=Date.now();
 const args=[...CAP,'env','DISPLAY=:62',`HOME=${GPHOME}`,`VK_ICD_FILENAMES=${NV}`,REPO+'/build/gpui/linux/backplane-gpui','--url',st.url,'--token-file',HOME+'/token'];if(script)args.push('--script',script);if(exitAfter)args.push('--exit-after',String(exitAfter));
 const p=spawn('systemd-run',args,{stdio:['ignore','ignore','pipe']});const metrics=[];let buf='';p.stderr.on('data',d=>{buf+=d;let i;while((i=buf.indexOf('\n'))>=0){const l=buf.slice(0,i);buf=buf.slice(i+1);const k=l.indexOf('GPUI_METRIC ');if(k>=0)try{metrics.push(JSON.parse(l.slice(k+12)))}catch{}}});
 return {p,pid:p.pid,spawnEpoch,metrics,ev:e=>metrics.find(x=>x.event===e)}}
const waitFor=async(f,ms)=>{const d=Date.now()+ms;while(Date.now()<d){const v=f();if(v)return v;await sleep(20)}return null};
// ---- electron latency step helpers (real input events through CDP; end = frame after the state shows)
// no eval (the page's CSP forbids it): fixed predicates. End = the message channel after the rAF that follows the state change.
const ARM=`window.__frame=(f)=>new Promise(res=>{const chk=()=>{if(!f()){requestAnimationFrame(chk);return}requestAnimationFrame(()=>{const mc=new MessageChannel();mc.port1.onmessage=()=>res(performance.timeOrigin+performance.now());mc.port2.postMessage(0)})};chk()});
window.__armSel=(id)=>window.__frame(()=>{const e=document.querySelector('.thread[data-value="'+id+'"]');return !!(e&&e.getAttribute('aria-current')==='page'&&document.querySelectorAll('.msg').length>0&&document.querySelector('textarea#composer'))});
window.__armPop=()=>window.__frame(()=>{const p=document.querySelector('.skills');return !!(p&&p.offsetParent!==null&&p.querySelectorAll('.skill').length>0)});true`;
async function click(c,x,y){for(const t of['mousePressed','mouseReleased'])await c.send('Input.dispatchMouseEvent',{type:t,x,y,button:'left',clickCount:1})}
async function elSelect(c,i){await c.ev(`(()=>{if(!window.__armSel){${ARM}}})()`);const pos=await c.ev(`(()=>{const e=[...document.querySelectorAll('.thread[data-value]')][${i}];if(!e)return null;e.scrollIntoView({block:'nearest'});const r=e.getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2,id:e.dataset.value}})()`);if(!pos)return null;
 const w=c.ev(`window.__armSel(${JSON.stringify(pos.id)})`);const e0=performance.timeOrigin+performance.now();await click(c,pos.x,pos.y);const e1=await Promise.race([w,sleep(15000).then(()=>null)]);return e1===null?null:e1-e0}
async function elPopup(c){await c.ev(`(()=>{if(!window.__armPop){${ARM}}})()`);await c.ev(`document.querySelector('textarea#composer').focus()`);
 const w=c.ev(`window.__armPop()`);const e0=performance.timeOrigin+performance.now();await c.send('Input.insertText',{text:'/'});const e1=await Promise.race([w,sleep(15000).then(()=>null)]);return e1===null?null:e1-e0}
const GPSCRIPT=(n=20)=>{const s=[];for(let i=0;i<n;i++){s.push({select:i},{wait:400},{popup:'/'},{wait:400})}return s};
// ---- phases
const R=existsSync(`${OUT}/${phase}.json`)?JSON.parse(readFileSync(`${OUT}/${phase}.json`,'utf8')):{};
const save=()=>writeFileSync(`${OUT}/${phase}.json`,JSON.stringify(R,null,1));
R.when=new Date().toISOString();R.machine='desktop 32 cores, RTX 5080 (driver 595.99.02), isolated Xvfb :62 + i3, 1440x900';R.fixture='JS hub 0.14.0 build/gpui/backplane-serve, 20 threads x short seeded history, stand-in agents; client caps 3700M swap 0';
if(phase==='startup'){
 R.gpui={cold:[],warm:[]};R.electron={fresh:[],reuse:[]};
 for(let i=0;i<30;i++){const g=launchGp(null,12);const u=await waitFor(()=>g.ev('usable'),30000);const ff=g.ev('first_frame');R.gpui[i<10?'cold':'warm'].push({usableMs:u&&u.epoch_ms-g.spawnEpoch,usableInternalMs:u&&u.t_ms,firstFrameMs:ff&&ff.epoch_ms-g.spawnEpoch,gpu:g.ev('gpu')?.device,software:g.ev('gpu')?.software});await sleep(500);await killTree(g.pid);await sleep(500);save()}
 for(let i=0;i<30;i++){const e=await launchEl({fresh:i<10});let fcp=null;try{fcp=await e.c.ev(`(()=>{const p=performance.getEntriesByName('first-contentful-paint')[0];return p?performance.timeOrigin+p.startTime:null})()`)}catch{}
  R.electron[i<10?'fresh':'reuse'].push({usableMs:e.usableMs,fcpMs:fcp&&fcp-e.spawnEpoch});try{e.c?.close()}catch{}const left=await killTree(e.pid);if(left)R.electronLeftover=(R.electronLeftover||0)+left;await sleep(1500);save()}
 for(const [k,o] of [['gpui',R.gpui],['electron',R.electron]])for(const [kk,a] of Object.entries(o)){R[k+'_'+kk+'_usableMs']=stat(a.map(x=>x.usableMs).filter(Number.isFinite));if(a[0]?.firstFrameMs!==undefined)R[k+'_'+kk+'_firstFrameMs']=stat(a.map(x=>x.firstFrameMs).filter(Number.isFinite));if(a[0]?.fcpMs!==undefined)R[k+'_'+kk+'_fcpMs']=stat(a.map(x=>x.fcpMs).filter(Number.isFinite))}
 save();
}
if(phase==='idle'||phase==='load'){
 const loaded=phase==='load';if(loaded){writeFileSync(ROOT+'/load','1');await sleep(20000)}
 R.runs={gpui:[],electron:[]};
 for(let r=0;r<3;r++){
  { // GPUI: select thread 0 then idle on it
   const sf=ROOT+`/gp-${phase}.json`;writeFileSync(sf,JSON.stringify([{select:0},{wait:95000}]));const g=launchGp(sf,0);await waitFor(()=>g.ev('usable'),30000);await sleep(loaded?10000:30000);
   const m=await sampleFor(g.pid,60);m.gpu=g.ev('gpu');R.runs.gpui.push(m);await killTree(g.pid);await sleep(2000);save()}
  { const e=await launchEl({fresh:true});if(e.c){await elSelect(e.c,0)}await sleep(loaded?10000:30000);
   const m=await sampleFor(e.pid,60);R.runs.electron.push(m);try{e.c?.close()}catch{}await killTree(e.pid);await sleep(2000);save()}
 }
 save();
}
if(phase==='latency'||phase==='latencyLoad'){
 if(phase==='latencyLoad'){writeFileSync(ROOT+'/load','1');await sleep(20000)}
 R.gpui={select:[],popup:[],fail:0};R.electron={select:[],popup:[],fail:0};
 for(let r=0;r<2;r++){
  const sf=ROOT+'/gp-lat.json';writeFileSync(sf,JSON.stringify(GPSCRIPT(20)));
  const g2=launchGp(sf,0);await waitFor(()=>g2.metrics.find(x=>x.event==='script_done'),120000);
  for(const x of g2.metrics.filter(x=>x.event==='input'&&x.step!=='wait')){if(!x.ok){R.gpui.fail++;continue}(x.step.startsWith('select')?R.gpui.select:R.gpui.popup).push(x.latency_ms)}
  await killTree(g2.pid);await sleep(1500);
  const e=await launchEl({fresh:true});if(e.c){for(let i=0;i<20;i++){const a=await elSelect(e.c,i);if(a===null)R.electron.fail++;else R.electron.select.push(a);await sleep(400);const b=await elPopup(e.c);if(b===null)R.electron.fail++;else R.electron.popup.push(b);await sleep(400)}}
  try{e.c?.close()}catch{}await killTree(e.pid);await sleep(1500);save()}
 for(const k of['gpui','electron'])for(const m of['select','popup'])R[k][m+'Stats']=stat(R[k][m]);
 save();
}
console.log(JSON.stringify(R,null,1).slice(0,4000));process.exit(0);
