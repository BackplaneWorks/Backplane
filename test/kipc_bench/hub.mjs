// A benchmark hub (docs/kicad-ipc-bench.md): bun hub.mjs HUB_BIN RUNTIME PORT WORK  -> starts it and prints {pid,port,home,run}; it stays up
// until WORK/stop exists. A new home at each start (the last one kept as WORK/home.N), temp HOME/XDG for KiCad, the wrapper `claude` as the only agent, KiStack the hub's own pinned copy (KIPC_BENCH_KISTACK: another).
import {mkdirSync,mkdtempSync,writeFileSync,chmodSync,copyFileSync,existsSync,rmSync,renameSync} from 'node:fs';
import {startHub,stopHub,sleep} from '../perf/lib.mjs';
const [bin,rt,port,work]=process.argv.slice(2);const KS=process.env.KIPC_BENCH_KISTACK,CL=process.env.KIPC_BENCH_CLAUDE,LG=process.env.KIPC_BENCH_LOGIN,MD=process.env.KIPC_BENCH_MODEL,EF=process.env.KIPC_BENCH_EFFORT,UH=process.env.HOME,DIS=process.env.KIPC_BENCH_DISALLOW??'',PR=process.env.KIPC_BENCH_PROMPT??'',NM=process.env.KIPC_BENCH_NOMCP??'';const GC=process.env.KIPC_BENCH_GROK_CLEAN??'';const here=import.meta.dir;
for(const d of ['bin','out','runs','user/.config/kicad','user/.cache','user/.local/share','user/.local/state'])mkdirSync(`${work}/${d}`,{recursive:true});
rmSync(`${work}/stop`,{force:true});
// A fresh home at every start: on an old one the hub takes up the turns its last life left running, and such a turn writes over its run's
// transcript in WORK/out and works beside the new runs. The old home is kept beside it (its event and simulation logs are a dead run's record).
if(existsSync(`${work}/home`)){let n=1;while(existsSync(`${work}/home.${n}`))n++;renameSync(`${work}/home`,`${work}/home.${n}`)}
const run=mkdtempSync('/tmp/kb-');
copyFileSync(`${here}/claude`,`${work}/bin/claude`);chmodSync(`${work}/bin/claude`,0o755);
for(const n of['codex','muse']){writeFileSync(`${work}/bin/${n}`,'#!/bin/sh\nexit 1\n');chmodSync(`${work}/bin/${n}`,0o755)}
copyFileSync(`${here}/grok`,`${work}/bin/grok`);chmodSync(`${work}/bin/grok`,0o755);
copyFileSync(`${here}/agy`,`${work}/bin/agy`);chmodSync(`${work}/bin/agy`,0o755); // the Antigravity cells (KIPC_BENCH_PROVIDER=antigravity): the person's agy behind a wrapper, their own sign-in
writeFileSync(`${work}/bin/sh`,'#!/bin/sh\nexec /bin/sh "$@"\n');chmodSync(`${work}/bin/sh`,0o755); // (the hub starts agy as `sh -c ... agy ARGS`, and a benchmark hub runs only what is in this folder) // the Grok cells: the real Grok CLI behind a wrapper, as claude's
const u=`${work}/user`;
const wrapper=`${work}/hub.sh`;writeFileSync(wrapper,`#!/bin/sh\nexport HOME='${u}' XDG_CONFIG_HOME='${u}/.config' XDG_CACHE_HOME='${u}/.cache' XDG_DATA_HOME='${u}/.local/share' XDG_STATE_HOME='${u}/.local/state' KICAD_CONFIG_HOME='${u}/.config/kicad' XDG_RUNTIME_DIR='${run}'\nexec '${bin}' "$@" >> '${work}/hub.log' 2>&1\n`);chmodSync(wrapper,0o755);
for(const k of Object.keys(process.env))if(/^CLAUDE|^BACKPLANE_|^ANTHROPIC|^KIPC_BENCH/.test(k))delete process.env[k]; // this shell may be an agent's own
process.env.PATH=process.env.PATH.split(':').filter(d=>!/\/shims$/.test(d)).join(':');
const h=await startHub({bin:wrapper,home:`${work}/home`,port:Number(port),standins:`${work}/bin`,cap:'14G', // (its output goes to hub.log by the wrapper's own redirect, never through a pipe to this launcher: a hub whose stderr reader has gone fails at its next line)
 
 extraEnv:{BACKPLANE_KICAD_CLI:`${rt}/bin/kicad-cli`,...(KS?{BACKPLANE_KISTACK:KS}:{}),KIPC_BENCH_OUT:`${work}/out`,
  KIPC_BENCH_CLAUDE:CL??`${UH}/.local/bin/claude`,KIPC_BENCH_GROK:process.env.KIPC_BENCH_GROK_BIN??`${UH}/.grok/bin/grok`,KIPC_BENCH_AGY:process.env.KIPC_BENCH_AGY_BIN??`${UH}/.local/bin/agy`,KIPC_BENCH_REALHOME:UH,KIPC_BENCH_GROK_CLEAN:GC,KIPC_BENCH_LOGIN:LG??`${UH}/.claude`,
  KIPC_BENCH_MODEL:MD??'claude-fable-5-2',KIPC_BENCH_EFFORT:EF??'medium',KIPC_BENCH_DISALLOW:DIS,KIPC_BENCH_PROMPT:PR,KIPC_BENCH_NOMCP:NM}});
writeFileSync(`${work}/hub.json`,JSON.stringify({pid:h.pid,port:h.port,home:h.home,run,token:h.token}));
console.log(JSON.stringify({pid:h.pid,port:h.port,home:h.home,run}));
while(!existsSync(`${work}/stop`))await sleep(500);
stopHub(h);for(let i=0;i<200&&existsSync(`/proc/${h.pid}`);i++)await sleep(50);rmSync(run,{recursive:true,force:true});process.exit(0);
