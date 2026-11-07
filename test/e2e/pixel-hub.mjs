// bun test/e2e/pixel-hub.mjs HUB_BIN OUT PORT: a private hub on a temp home with the F-rows stand-in agent, three threads (the third a buy thread, the first approval-required), a project with a board, a STEP part and a file;
// prints {url, token, invite} and stays up. The phone reaches it through `adb reverse tcp:PORT tcp:PORT`.
import {spawn} from 'node:child_process';
import {mkdirSync,rmSync,writeFileSync,readFileSync,chmodSync,copyFileSync,openSync} from 'node:fs';
const [BIN,OUT,PORT]=process.argv.slice(2);const sleep=ms=>new Promise(r=>setTimeout(r,ms));
rmSync(OUT,{recursive:true,force:true});for(const d of ['home','proj/mech','bin'])mkdirSync(`${OUT}/${d}`,{recursive:true});
const HERE=new URL('.',import.meta.url).pathname;
copyFileSync(HERE+'lib/standin_rows.py',OUT+'/bin/claude');chmodSync(OUT+'/bin/claude',0o755);for(const n of['codex','grok','muse','opencode']){writeFileSync(`${OUT}/bin/${n}`,'#!/bin/sh\nexit 1\n');chmodSync(`${OUT}/bin/${n}`,0o755)}
copyFileSync(HERE+'../files/fixtures/mesh/bracket.step',OUT+'/proj/mech/bracket.step');for(const e of['kicad_pcb','kicad_sch'])copyFileSync(HERE+`../files/fixtures/plot/ecc83-pp.${e}`,`${OUT}/proj/ecc83.${e}`);writeFileSync(OUT+'/proj/ecc83.kicad_pro','{}\n');writeFileSync(OUT+'/proj/readme.txt','pixel fixture\n');
const now=Math.floor(Date.now()/1000);
const seed=[{$:'ProjectCreated',id:'p1',title:'rows',root:OUT+'/proj',at:now},...[1,2,3].map(i=>({$:'ThreadCreated',id:'t'+i,project:'p1',title:'Thread '+i,env:'local',branch:'',worktree:'',provider:'claude',model:'',at:now+i})),
  {$:'SettingSet',key:'buy.t3',value:'t0',at:now+3},{$:'ModesSet',id:'t1',runtime:'approval-required',interaction:'default',model:'',at:now+3},{$:'SettingSet',key:'onboard.done',value:'1',at:now+3}];
writeFileSync(OUT+'/home/events.jsonl',seed.map(e=>JSON.stringify(e)+'\n').join(''));
const env={...process.env,BACKPLANE_AGENTS:OUT+'/bin',PATH:`${OUT}/bin:${process.env.PATH}`,BACKPLANE_NO_UPDATE:'1',SHELL:'/bin/sh',FAKE_LOG:OUT+'/fake.log',FAKE_SECS:'600',HOME:OUT+'/home'};
const out=openSync(OUT+'/hub.log','a');const hub=spawn(BIN,['--home',OUT+'/home','--port',PORT,'--host','127.0.0.1','--no-tailscale','--foreground'],{env,stdio:['ignore',out,out],cwd:OUT+'/proj'});
for(let i=0;i<300;i++){try{if((await fetch(`http://127.0.0.1:${PORT}/hello`)).status===200)break}catch{}await sleep(100)}
const token=readFileSync(OUT+'/home/token','utf8').trim();
const r=await fetch(`http://127.0.0.1:${PORT}/auth/invitations`,{method:'POST',headers:{authorization:'Bearer '+token}});const invite=(await r.json()).link;
writeFileSync(OUT+'/state.json',JSON.stringify({url:`http://127.0.0.1:${PORT}`,token,invite,pid:hub.pid}));console.log(JSON.stringify({url:`http://127.0.0.1:${PORT}`,invite,pid:hub.pid}));
process.on('SIGTERM',()=>{hub.kill();process.exit(0)});setInterval(()=>{},1e6);
