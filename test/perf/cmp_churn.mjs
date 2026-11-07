const BP_ROOT=new URL('../..',import.meta.url).pathname.replace(/\/$/,'');
// GPUI churn: one client selects/pops over all threads for N minutes while the seeded hub sees reconnecting plain clients,
// thread create/delete, and two hub restarts (the client must reconnect). bun cmp_churn.mjs MINUTES
import {readFileSync,writeFileSync,mkdirSync,appendFileSync,existsSync,readdirSync,openSync,rmSync} from 'node:fs';
import {spawn,execSync} from 'node:child_process';import {tree,sleep} from './lib.mjs';import {startHub,client} from './lib.mjs';
const REPO=BP_ROOT;const ROOT=REPO+'/build/gpui/perf-run/cmp';const OUT=REPO+'/build/gpui/results/perf/gpui-churn';mkdirSync(OUT,{recursive:true});
const mins=Number(process.argv[2]||20);const rd=p=>{try{return readFileSync(p,'utf8')}catch{return ''}};
const X=REPO+'/build/gpui/xvfb';
const xv=spawn(X+'/usr/bin/Xvfb',[':62','-screen','0','1440x900x24','-xkbdir','/usr/share/X11/xkb','-nolisten','tcp'],{stdio:'ignore',env:{...process.env,LD_LIBRARY_PATH:X+'/usr/lib/x86_64-linux-gnu'}});await sleep(2000);
const i3=spawn('i3',['-c',X+'/i3.conf'],{stdio:'ignore',env:{...process.env,DISPLAY:':62'}});await sleep(2000);
rmSync(ROOT+'/state.json',{force:true});writeFileSync(REPO+'/build/gpui/results/perf/cmp/hub.out','');
const ho=openSync(REPO+'/build/gpui/results/perf/cmp/hub.out','a');const hubp=spawn('bun',['cmp_hub.mjs','3807'],{cwd:REPO+'/test/perf',stdio:['ignore',ho,ho]});
for(let i=0;i<120;i++){if(rd(ROOT+'/state.json').length>20&&rd(REPO+'/build/gpui/results/perf/cmp/hub.out').includes('ready'))break;await sleep(1000)}
await sleep(5000);let st=JSON.parse(rd(ROOT+'/state.json'));
const agentsOk=hp=>rd(`/proc/${hp}/environ`).split('\0').some(x=>x.startsWith('BACKPLANE_AGENTS=')&&x.includes('perf-run/cmp/bin'));if(!agentsOk(st.pid))throw Error('hub not on stand-ins');
// GPUI script: select i, popup, forever
const steps=[];for(let k=0;k<Math.ceil(mins*60/1.6);k++){const i=k%20;steps.push({select:i},{wait:500},{popup:'/'},{wait:500})}
const sf=ROOT+'/gp-churn.json';writeFileSync(sf,JSON.stringify(steps));
const metrics=[];let buf='';
const g=spawn('systemd-run',['--user','--scope','--quiet','-p','MemoryMax=3700M','-p','MemorySwapMax=0','env','DISPLAY=:62',`HOME=${ROOT}/fakehome-gp`,'VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/nvidia_icd.json',REPO+'/build/gpui/linux/backplane-gpui','--url',st.url,'--token-file',ROOT+'/home/token','--script',sf],{stdio:['ignore','ignore','pipe']});
g.stderr.on('data',d=>{buf+=d;let i;while((i=buf.indexOf('\n'))>=0){const l=buf.slice(0,i);buf=buf.slice(i+1);const k=l.indexOf('GPUI_METRIC ');if(k>=0)try{const m=JSON.parse(l.slice(k+12));m.wall=Date.now();metrics.push(m)}catch{}}});
await sleep(5000);
const t0=Date.now();const ops={plain:0,created:0,deleted:0,err:0,hubRestarts:0};let cur={...st};let stop=false;
const hubReady=()=>({host:'127.0.0.1',port:3807,token:st.token,url:st.url});
const w1=(async()=>{while(!stop){try{const c=await client(hubReady());await sleep(300);c.ws.close();ops.plain++}catch{ops.err++}await sleep(150)}})();
const w2=(async()=>{while(!stop){try{const cc=await client(hubReady());const r=await cc.rpc('thread.create',{project:st.project,title:'tmp',env:'local'},8000);if(r?.o?.thread){ops.created++;const d=await cc.rpc('thread.delete',{thread:r.o.thread},8000);if(d?.o?.ok)ops.deleted++}cc.ws.close()}catch{ops.err++}await sleep(400)}})();
const rss=[];const snap=()=>{const pids=[g.pid,...tree(g.pid)];let r=0,p=0,fd=0,th=0;for(const q of pids){const sm=rd(`/proc/${q}/smaps_rollup`);if(!sm)continue;const k=n=>Number(new RegExp(n+':\\s+(\\d+)').exec(sm)?.[1]||0);r+=k('Rss');p+=k('Pss');try{fd+=readdirSync(`/proc/${q}/fd`).length}catch{}th+=Number(/Threads:\s+(\d+)/.exec(rd(`/proc/${q}/status`))?.[1]||0)}return {rss:r,pss:p,fds:fd,thr:th}};
writeFileSync(OUT+'/samples.jsonl','');
const restartAt=[mins*60*0.33,mins*60*0.66];let nextRestart=0;
while(Date.now()-t0<mins*60000){const t=(Date.now()-t0)/1000;
 appendFileSync(OUT+'/samples.jsonl',JSON.stringify({t:Math.round(t),...snap(),...ops,inputOk:metrics.filter(m=>m.event==='input'&&m.ok).length,inputFail:metrics.filter(m=>m.event==='input'&&!m.ok).length,hubRss:Number(/VmRSS:\s+(\d+)/.exec(rd(`/proc/${cur.pid}/status`))?.[1]||0)})+'\n');
 if(nextRestart<2&&t>=restartAt[nextRestart]){nextRestart++;ops.hubRestarts++;const old=cur.pid;process.kill(old,'SIGTERM');for(let i=0;i<100&&existsSync('/proc/'+old);i++)await sleep(200);
  appendFileSync(OUT+'/events.log',`${Math.round(t)}s hub restart ${ops.hubRestarts}\n`);
  const h=await startHub({bin:REPO+'/build/gpui/backplane-serve',home:ROOT+'/home',port:3807,host:'127.0.0.1',standins:ROOT+'/bin',extraEnv:{FAKE_SECS:'3000'},log:OUT+'/hub-restart.log'});cur={...st,pid:h.pid}}
 await sleep(15000)}
stop=true;await Promise.all([w1,w2]);
const lastOkAfter=[...metrics].reverse().find(m=>m.event==='input'&&m.ok);
const fails=metrics.filter(m=>m.event==='input'&&!m.ok);
writeFileSync(OUT+'/summary.json',JSON.stringify({mins,ops,inputs:metrics.filter(m=>m.event==='input').length,inputOk:metrics.filter(m=>m.event==='input'&&m.ok).length,inputFail:fails.length,failWallOffsetsS:fails.map(m=>Math.round((m.wall-t0)/1000)).slice(0,40),lastOkOffsetS:lastOkAfter&&Math.round((lastOkAfter.wall-t0)/1000),scriptDone:!!metrics.find(m=>m.event==='script_done')},null,1));
try{process.kill(g.pid,'SIGTERM')}catch{}await sleep(2000);try{process.kill(cur.pid,'SIGTERM')}catch{}writeFileSync(ROOT+'/stop','1');await sleep(5000);
try{xv.kill('SIGTERM');i3.kill('SIGTERM')}catch{}writeFileSync(OUT+'/done','1');process.exit(0);
