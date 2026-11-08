// bun test/e2e/ios/f-ctl.mjs HUB_BIN OUT PORT: the control port (PORT+1) the iOS F-rows test (FRowsTests.swift) asks, beside the private hub
// test/e2e/pixel-hub.mjs runs on OUT/hub (state.json, events.jsonl, the stand-in's fake.log). Answers from those files, never from the test's say-so:
//   GET /count?f=events|fake&re=REGEX -> {n}: matching lines
//   GET /kill -> the hub process is killed;  GET /restart -> started again on the same home, as pixel-f-rows.sh does;  GET /up -> {up}
import {spawn} from 'node:child_process';
import {existsSync,openSync,readFileSync} from 'node:fs';
const [BIN,OUT,PORT_]=process.argv.slice(2), PORT=Number(PORT_), H=OUT+'/hub';
const files={events:H+'/home/events.jsonl',fake:H+'/fake.log'};
const J=o=>new Response(JSON.stringify(o),{headers:{'content-type':'application/json'}});
let pid=JSON.parse(readFileSync(H+'/state.json','utf8')).pid;
const alive=p=>{try{process.kill(p,0);return true}catch{return false}};
const up=async()=>{try{return (await fetch(`http://127.0.0.1:${PORT}/hello`)).status===200}catch{return false}};
Bun.serve({port:PORT+1,hostname:'127.0.0.1',idleTimeout:60,async fetch(r){
  const u=new URL(r.url),q=Object.fromEntries(u.searchParams);
  switch(u.pathname){
    case '/count':{const f=files[q.f];if(!f||!existsSync(f))return J({n:0});let re;try{re=new RegExp(q.re||'')}catch{return J({n:-1,error:'bad re'})}
      return J({n:readFileSync(f,'utf8').split('\n').filter(l=>re.test(l)).length})}
    case '/kill':{if(alive(pid))process.kill(pid,'SIGKILL');for(let i=0;i<50&&await up();i++)await Bun.sleep(100);return J({killed:pid,up:await up()})}
    case '/restart':{
      const env={...process.env,BACKPLANE_AGENTS:H+'/bin',PATH:`${H}/bin:${process.env.PATH}`,BACKPLANE_NO_UPDATE:'1',SHELL:'/bin/sh',FAKE_LOG:files.fake,FAKE_SECS:'600',HOME:H+'/home'};
      const out=openSync(OUT+'/hub2.log','a');
      const p=spawn(BIN,['--home',H+'/home','--port',String(PORT),'--host','127.0.0.1','--no-tailscale','--foreground'],{env,stdio:['ignore',out,out],cwd:H+'/proj',detached:false});
      pid=p.pid;for(let i=0;i<300&&!await up();i++)await Bun.sleep(100);return J({pid,up:await up()})}
    case '/up':return J({up:await up()});
    case '/stop':setTimeout(()=>{if(alive(pid))process.kill(pid,'SIGTERM');process.exit(0)},100);return J({stopping:true});
    default:return new Response('no',{status:404});
  }}});
console.log(`ready ctl ${PORT+1}`);
