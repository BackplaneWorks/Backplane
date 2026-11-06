// Samples the hub during a start on a copy: the hub process's own RSS (anon/file/shmem split) AND its cgroup's memory.current and memory.stat
// (anon vs file page cache), because a scope cap counts page cache and dirty pages, not only the process's RSS.
// The hub is found by its command line (backplane-hub or backplane-serve with --home <M0_HOME>), not from hub.lock.
import {readFileSync,appendFileSync,writeFileSync,readdirSync} from 'node:fs';
const OUT=(process.env.M0_OUT||'/home/h/Documents/i2cjak/backplane-gpui/build/gpui/results/perf/m0')+'/import-samples.jsonl';writeFileSync(OUT,'');
const HOME=process.env.M0_HOME||'/home/h/Documents/i2cjak/backplane-gpui/build/gpui/perf-run/m0/home';
const sleep=ms=>new Promise(r=>setTimeout(r,ms));const rd=p=>{try{return readFileSync(p,'utf8')}catch{return ''}};
const t0=Date.now();let pid=0,cg='',last=null;
const find=()=>{for(const d of readdirSync('/proc')){if(!/^\d+$/.test(d))continue;const c=rd(`/proc/${d}/cmdline`).split('\0');if(/backplane-(hub|serve)$/.test(c[0]||'')&&c.includes('--home')&&c[c.indexOf('--home')+1]===HOME)return Number(d)}return 0};
const num=(s,k)=>Number(new RegExp(k+':\\s+(\\d+)').exec(s)?.[1]||0)/1024;
while(Date.now()-t0<1800000){
  if(!pid){pid=find();if(pid){const m=/0::(.*)/.exec(rd(`/proc/${pid}/cgroup`));cg=m?'/sys/fs/cgroup'+m[1].trim():''}}
  if(pid){const st=rd(`/proc/${pid}/status`);
    if(!st){appendFileSync(OUT,JSON.stringify({t:(Date.now()-t0)/1000,gone:true,last})+'\n');break}
    const ms=rd(cg+'/memory.stat');const g=k=>Number(new RegExp('^'+k+' (\\d+)','m').exec(ms)?.[1]||0)/1048576;
    last={t:(Date.now()-t0)/1000,rssMB:num(st,'VmRSS'),rssAnonMB:num(st,'RssAnon'),rssFileMB:num(st,'RssFile'),hwmMB:num(st,'VmHWM'),cgCurrentMB:Number(rd(cg+'/memory.current'))/1048576,cgAnonMB:g('anon'),cgFileMB:g('file'),cgDirtyMB:g('file_dirty'),cgWritebackMB:g('file_writeback'),cgShmemMB:g('shmem')};
    // which process in the cgroup holds the memory: the top three by anonymous RSS
    const procs=[];for(const d of readdirSync('/proc')){if(!/^\d+$/.test(d))continue;if(!rd(`/proc/${d}/cgroup`).includes(cg.replace('/sys/fs/cgroup','')))continue;const s=rd(`/proc/${d}/status`);if(!s)continue;procs.push({pid:Number(d),cmd:(rd(`/proc/${d}/cmdline`).split('\0')[0]||'').split('/').pop(),anonMB:num(s,'RssAnon')})}
    last.top=procs.sort((a,b)=>b.anonMB-a.anonMB).slice(0,3).map(p=>({...p,anonMB:Math.round(p.anonMB)}));
    appendFileSync(OUT,JSON.stringify(last)+'\n')}
  await sleep(Number(process.env.WATCH_MS)||2000)}
