// bun test/e2e/ios/copy-hub.mjs HUB_BIN OUT PORT: a private hub on a temp home for the iOS copy rows (UITests/CopyTests.swift, copy-rows.sh): the reply
// of test/e2e/gpui-copy-rows.seed.py (an inline command, a commit, a fenced command writing a marker file) in a project's thread (t5), the uberbot Bub's
// chat (tb), a bot Pip's chat (tc) and the room `bringup` (r1). The shell is bash (it turns on bracketed paste). Writes OUT/state.json {url, token, pid}
// and stays up; f-ctl.mjs answers /find?name= for the markers. Stand-in agents only (lib/standin_rows.py); HOME is the temp home.
import {spawn} from 'node:child_process';
import {mkdirSync,rmSync,writeFileSync,readFileSync,chmodSync,copyFileSync,openSync} from 'node:fs';
const [BIN,OUT,PORT]=process.argv.slice(2);const sleep=ms=>new Promise(r=>setTimeout(r,ms));
rmSync(OUT,{recursive:true,force:true});for(const d of ['home','proj','bin'])mkdirSync(`${OUT}/${d}`,{recursive:true});
const HERE=new URL('.',import.meta.url).pathname;
copyFileSync(HERE+'../lib/standin_rows.py',OUT+'/bin/claude');chmodSync(OUT+'/bin/claude',0o755);for(const n of['codex','grok','muse','opencode']){writeFileSync(`${OUT}/bin/${n}`,'#!/bin/sh\nexit 1\n');chmodSync(`${OUT}/bin/${n}`,0o755)}
writeFileSync(OUT+'/proj/readme.txt','copy rows fixture\n');
const INLINE='sudo apt-get install -y rootlesskit slirp4netns';
const reply=m=>`One thing needs you: please run \`${INLINE}\` first. Commit \`3552c7cfd\` has the fix.\n\n\`\`\`bash\necho zq > ${m}\n\`\`\`\n\nThen tell me.`;
const now=Math.floor(Date.now()/1000);
const th=(id,project,title,at)=>({$:'ThreadCreated',id,project,title,env:'local',branch:'',worktree:'',provider:'claude',model:'',at});
const seed=[{$:'ProjectCreated',id:'p1',title:'rows',root:OUT+'/proj',at:now},
  th('t5','p1','Needs a package',now+1),{$:'MessagePosted',thread:'t5',msg:'cp0',role:'user',text:'zqshort can you speed up the builds?',at:now+1},
  {$:'MessagePosted',thread:'t5',msg:'cp1',role:'assistant',text:reply('zq-thread.marker'),at:now+1},
  th('tb','','Bub',now+2),{$:'BotSet',id:'b1',name:'Bub',thread:'tb',look:0,persona:'',at:now+2},{$:'UberSet',bot:'b1',uid:'u-rows',at:now+2},
  {$:'MessagePosted',thread:'tb',msg:'cp2',role:'assistant',text:reply('zq-bub.marker'),at:now+3},
  th('tc','','Pip',now+4),{$:'BotSet',id:'b2',name:'Pip',thread:'tc',look:3,persona:'',at:now+4},
  {$:'MessagePosted',thread:'tc',msg:'cp3',role:'assistant',text:reply('zq-pip.marker'),at:now+5},
  {$:'RoomSet',id:'r1',name:'bringup',members:'b1',at:now+6},{$:'RoomPosted',id:'rp1',room:'r1',from:'Bub',text:reply('zq-room.marker'),hop:0,at:now+7},
  {$:'SettingSet',key:'onboard.done',value:'1',at:now+7}];
writeFileSync(OUT+'/home/events.jsonl',seed.map(e=>JSON.stringify(e)+'\n').join(''));
const env={...process.env,BACKPLANE_AGENTS:OUT+'/bin',PATH:`${OUT}/bin:${process.env.PATH}`,BACKPLANE_NO_UPDATE:'1',SHELL:'/bin/bash',FAKE_LOG:OUT+'/fake.log',FAKE_SECS:'600',HOME:OUT+'/home'};
delete env.DISPLAY;delete env.WAYLAND_DISPLAY;
const out=openSync(OUT+'/hub.log','a');const hub=spawn(BIN,['--home',OUT+'/home','--port',PORT,'--host','127.0.0.1','--no-tailscale','--foreground'],{env,stdio:['ignore',out,out],cwd:OUT+'/proj'});
for(let i=0;i<300;i++){try{if((await fetch(`http://127.0.0.1:${PORT}/hello`)).status===200)break}catch{}await sleep(100)}
const token=readFileSync(OUT+'/home/token','utf8').trim();
writeFileSync(OUT+'/state.json',JSON.stringify({url:`http://127.0.0.1:${PORT}`,token,pid:hub.pid}));console.log(JSON.stringify({url:`http://127.0.0.1:${PORT}`,pid:hub.pid}));
process.on('SIGTERM',()=>{hub.kill();process.exit(0)});setInterval(()=>{},1e6);
