// GPUI client soak under 20 simulated agents. bun cmp_soak.mjs MINUTES   (own Xvfb :62 + i3, fixture hub 3807, client in a 3700M scope)
import {readFileSync,writeFileSync,mkdirSync,appendFileSync,existsSync,readdirSync} from 'node:fs';
import {spawn,execSync} from 'node:child_process';import {tree,sleep} from './lib.mjs';
const REPO='/home/h/Documents/i2cjak/backplane-gpui';const ROOT=REPO+'/build/gpui/perf-run/cmp';const OUT=REPO+'/build/gpui/results/perf/gpui-soak';mkdirSync(OUT,{recursive:true});
const mins=Number(process.argv[2]||40);const rd=p=>{try{return readFileSync(p,'utf8')}catch{return ''}};
const X=REPO+'/build/gpui/xvfb';
const xv=spawn(X+'/usr/bin/Xvfb',[':62','-screen','0','1440x900x24','-xkbdir','/usr/share/X11/xkb','-nolisten','tcp'],{stdio:'ignore',env:{...process.env,LD_LIBRARY_PATH:X+'/usr/lib/x86_64-linux-gnu'}});await sleep(2000);
const i3=spawn('i3',['-c',X+'/i3.conf'],{stdio:'ignore',env:{...process.env,DISPLAY:':62'}});await sleep(2000);
import('node:fs').then(()=>{});const {rmSync,openSync}=await import('node:fs');rmSync(ROOT+'/state.json',{force:true});writeFileSync(REPO+'/build/gpui/results/perf/cmp/hub.out','');
const ho=openSync(REPO+'/build/gpui/results/perf/cmp/hub.out','a');const hub=spawn('bun',['cmp_hub.mjs','3807'],{cwd:REPO+'/test/perf',stdio:['ignore',ho,ho]});
for(let i=0;i<120;i++){if(existsSync(ROOT+'/state.json')&&rd(ROOT+'/state.json').length>20&&rd(REPO+'/build/gpui/results/perf/cmp/hub.out').includes('ready'))break;await sleep(1000)}
await sleep(8000);const st=JSON.parse(rd(ROOT+'/state.json'));
// hub guard: stand-ins only
const env=rd(`/proc/${st.pid}/environ`).split('\0').find(x=>x.startsWith('BACKPLANE_AGENTS='));if(!env||!env.includes('perf-run/cmp/bin'))throw Error('hub is not on stand-ins');
writeFileSync(ROOT+'/load','1');await sleep(15000);
const sf=ROOT+'/gp-soak.json';writeFileSync(sf,JSON.stringify([{select:0},{wait:(mins+5)*60000}]));
const g=spawn('systemd-run',['--user','--scope','--quiet','-p','MemoryMax=3700M','-p','MemorySwapMax=0','env','DISPLAY=:62',`HOME=${ROOT}/fakehome-gp`,'VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/nvidia_icd.json',REPO+'/build/gpui/linux/backplane-gpui','--url',st.url,'--token-file',ROOT+'/home/token','--script',sf],{stdio:'ignore'});
await sleep(4000);
writeFileSync(OUT+'/samples.jsonl','');const t0=Date.now();
const snap=()=>{const pids=[g.pid,...tree(g.pid)];let rss=0,pss=0,priv=0,fds=0,thr=0,ticks=0;for(const p of pids){const sm=rd(`/proc/${p}/smaps_rollup`);if(!sm)continue;const k=n=>Number(new RegExp(n+':\\s+(\\d+)').exec(sm)?.[1]||0);rss+=k('Rss');pss+=k('Pss');priv+=k('Private_Clean')+k('Private_Dirty');try{fds+=readdirSync(`/proc/${p}/fd`).length}catch{}thr+=Number(/Threads:\s+(\d+)/.exec(rd(`/proc/${p}/status`))?.[1]||0);const f=rd(`/proc/${p}/stat`).split(') ')[1]?.split(' ');if(f)ticks+=Number(f[11])+Number(f[12])}
 return {rss,pss,priv,fds,thr,ticks}};
let hubOk=true;
while(Date.now()-t0<mins*60000){const s=snap();let gpu=NaN;try{gpu=Number(execSync('nvidia-smi --query-gpu=memory.used --format=csv,noheader,nounits',{encoding:'utf8'}).trim())}catch{}
 appendFileSync(OUT+'/samples.jsonl',JSON.stringify({t:Math.round((Date.now()-t0)/1000),...s,gpu,hubRss:Number(/VmRSS:\s+(\d+)/.exec(rd(`/proc/${st.pid}/status`))?.[1]||0)})+'\n');await sleep(30000-((Date.now()-t0)%30000))}
try{process.kill(g.pid,'SIGTERM')}catch{}await sleep(2000);writeFileSync(ROOT+'/stop','1');await sleep(5000);
try{xv.kill('SIGTERM');i3.kill('SIGTERM')}catch{}writeFileSync(OUT+'/done','1');process.exit(0);
