// Agents start on any systemd (native/host/src/scopeprobe.rs, memscope.rs): a throwaway hub runs a real claude turn and a real codex turn,
// on the machine it runs on (an old distro in a container: Debian 10's systemd 241, Ubuntu 20.04's 245, 22.04's 249 that has no OOMPolicy for
// scopes; or a current one). MODE says how scopes should go:
//   scoped   a user systemd answers: the hub's log names the systemd and what it left out, each agent runs in a bp-agent scope
//   noscope  BACKPLANE_NO_SCOPE=1: one log line, agents unscoped
//   nobus    no user session bus (XDG_RUNTIME_DIR and DBUS_SESSION_BUS_ADDRESS unset, as in a container or WSL): one log line, agents unscoped
//   failrun  a systemd-run whose probe scopes work but every real launch fails ("Unknown assignment"): the launch is retried unscoped, the
//            log says so once, and every later turn starts unscoped
// Checked in every mode: the turns finish with the model's answer, the hub never logs a failed agent start. CODEX=skip leaves codex out
// (no login on the machine).
// usage: bun test/hub/scope_compat_e2e.mjs HUB_BIN PORT OUTDIR MODE CLAUDE_BIN CODEX_BIN
import {mkdirSync,writeFileSync,chmodSync,rmSync,readFileSync} from 'node:fs';
import {startHub,stopHub,client,sleep,tree} from '../perf/lib.mjs';
const [bin,port,out,mode,claudeBin,codexBin]=process.argv.slice(2);
const work=`${out}/scope-${mode}`;rmSync(work,{recursive:true,force:true});mkdirSync(work+'/bin',{recursive:true});mkdirSync(work+'/proj',{recursive:true});
const home=work+'/home';mkdirSync(home,{recursive:true});
const t0=1700000000,ev=[{$:'ProjectCreated',id:'p1700000000-1',title:'scope',root:work+'/proj',at:t0},
  {$:'ThreadCreated',id:'t1700000001-2',project:'p1700000000-1',title:'claude',env:'local',branch:'',worktree:'',provider:'claude',model:'',at:t0+1},
  {$:'ThreadCreated',id:'t1700000002-3',project:'p1700000000-1',title:'codex',env:'local',branch:'',worktree:'',provider:'codex',model:'',at:t0+2}];
writeFileSync(home+'/events.jsonl',ev.map(e=>JSON.stringify(e)).join('\n')+'\n');
const sh=(n,body)=>{writeFileSync(`${work}/bin/${n}`,`#!/bin/sh\n${body}\n`);chmodSync(`${work}/bin/${n}`,0o755)};
sh('claude',`exec ${claudeBin} "$@"`);sh('codex',`exec ${codexBin} "$@"`);
// failrun: probes (`-- true`) work, a real launch fails as an older systemd does
const real=(()=>{for(const d of ['/usr/bin','/bin'])try{readFileSync(`${d}/systemd-run`,{flag:'r'});return `${d}/systemd-run`}catch{}return 'systemd-run'})();
sh('fake-systemd-run',`for a; do last=$a; done\ncase "$1" in --version) exec ${real} --version;; esac\nseen=0; for a; do [ "$seen" = 1 ] && { [ "$a" = true ] && exec ${real} "$@"; echo "Unknown assignment: OOMPolicy=continue" >&2; echo "Failed to start transient scope unit: Invalid argument" >&2; exit 1; }; [ "$a" = -- ] && seen=1; done; exit 1`);
const extraEnv={},unsetEnv=[];
if(mode==='noscope')extraEnv.BACKPLANE_NO_SCOPE='1';
if(mode==='nobus')unsetEnv.push('XDG_RUNTIME_DIR','DBUS_SESSION_BUS_ADDRESS');
if(mode==='failrun')extraEnv.BACKPLANE_SYSTEMD_RUN=`${work}/bin/fake-systemd-run`;
process.env.NO_SCOPE='1'; // the harness starts the hub directly (lib.mjs), so the hub's own probe is what is tested
const h=await startHub({bin,home,port:Number(port),standins:work+'/bin',log:work+'/hub.log',extraEnv,unsetEnv});
let fails=0;const check=(what,ok,detail='')=>{console.log(`${ok?'PASS':'FAIL'} ${what}${ok?'':' ('+detail+')'}`);if(!ok)fails++};
const events=()=>readFileSync(home+'/events.jsonl','utf8').split('\n').filter(Boolean).map(l=>{try{return JSON.parse(l)}catch{return {}}});
const ended=th=>events().filter(e=>e.$==='TurnChanged'&&e.thread===th&&['completed','failed','stopped'].includes(e.state));
const answer=th=>events().filter(e=>e.$==='MessagePosted'&&e.thread===th&&e.role==='assistant').map(e=>e.text||'').join(' | ');
const rd=p=>{try{return readFileSync(p,'utf8')}catch{return ''}};
// the cgroups the agent processes ran in, seen while the turn runs
const cgroupsOf=re=>{const s=new Set();for(const d of tree(h.pid).map(String)){/* this hub's own children only, never another hub's agents */if(re.test(rd(`/proc/${d}/cmdline`).replace(/\0/g,' '))){const g=/0::(\S+)/.exec(rd(`/proc/${d}/cgroup`));if(g)s.add(g[1])}}return s};
const hubLog=()=>rd(work+'/hub.log');
await sleep(1500);const c=await client(h);
const ms=hubLog().split('\n').filter(l=>/memscope/.test(l));console.log('hub log: '+ms.join(' | '));
if(mode==='scoped')check('the hub runs agents in scopes, naming what this systemd takes',/memscope: agents and jobs run in scopes of their own in bpagent\.slice .*systemd \d+/.test(hubLog()),ms.join(' | '));
else if(mode==='failrun')check('the probe found scopes',/memscope: agents and jobs run in scopes of their own/.test(hubLog()),ms.join(' | '));
else check('scopes off, said once, never an error',ms.length===1&&/memscope: off \(/.test(ms[0])&&!/error/i.test(ms[0]),ms.join(' | '));
const runTurn=async(th,name,re,n)=>{
  const e0=ended(th).length;const seen=new Set();
  c.send('turn.start',{thread:th,text:`Reply with exactly the word ok${n} and nothing else. Do not use any tools.`});
  for(let i=0;i<1800&&ended(th).length===e0;i++){for(const g of cgroupsOf(re))seen.add(g);await sleep(100)}
  const e=ended(th).slice(e0)[0];const a=answer(th);
  console.log(`${name} turn ${n}: ${e?.state??'none'}; answer: ${JSON.stringify(a.slice(-120))}; ran in ${[...seen].join(', ')||'(not seen)'}`);
  return {state:e?.state,answer:a,cg:[...seen]};
};
const scopedWant=mode==='scoped';
for(const [th,name,re,skip] of [['t1700000001-2','claude',/claude\.exe|claude\/versions\/|\/claude /,false],['t1700000002-3','codex',/codex/,process.env.CODEX==='skip']]){
  if(skip){console.log(`SKIP ${name} (CODEX=skip: no login here)`);continue}
  for(const n of [1,2]){
    const r=await runTurn(th,name,re,n);
    check(`${name} turn ${n} finishes with the model's answer`,r.state==='completed'&&new RegExp(`ok${n}`,'i').test(r.answer),`${r.state}: ${r.answer.slice(-200)}`);
    const inScope=r.cg.some(g=>new RegExp(`/bp-agent-${h.pid}-\\d+\\.scope$`).test(g)); // one this hub made (the harness may itself run in another hub's)
    if(r.cg.length)check(`${name} turn ${n} ${scopedWant?'in a bp-agent scope':'not in a bp-agent scope'}`,inScope===scopedWant,r.cg.join(', '));
  }
}
const log=hubLog();
check('no agent failed to start',!/could not start|exit code 1|Unknown assignment.*agent/i.test(log.split('\n').filter(l=>!/memscope/.test(l)).join('\n')),log.split('\n').filter(l=>/could not start|exit code 1/i.test(l)).slice(0,3).join(' | '));
if(mode==='failrun')check('the failed scope said once, then scopes off for the session',(log.match(/memscope: a scoped launch failed \(Unknown assignment: OOMPolicy=continue\)/g)||[]).length===1,log.split('\n').filter(l=>/memscope/.test(l)).join(' | '));
stopHub(h);await sleep(1000);
console.log(`scope_compat_e2e ${mode}: ${fails?'FAILED':'ok'} (${fails} failed) artifacts=${work}`);process.exit(fails?1:0);
