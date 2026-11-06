// bun test/gpui/e2e/run.mjs --client gpui|android|ios [--journey id|all] [--hub-bin PATH] [--gpui-bin PATH] [--out DIR] [--hz N] [--record] [--threads N]
// Runs on Boat (Xvfb) for GPUI; never starts a display or hub on the desktop without E2E_ALLOW_DESKTOP=1 (the person's word).
import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import {fileURLToPath} from 'node:url';import {dirname,resolve} from 'node:path';
const here=dirname(fileURLToPath(import.meta.url));
const arg=(k,d)=>{const i=process.argv.indexOf('--'+k);return i>=0?process.argv[i+1]:d};const flag=k=>process.argv.includes('--'+k);
const hubKind=arg('hub-kind','js'),realHw=flag('real-hardware');const client=arg('client','gpui'),which=arg('journey','all'),out=resolve(arg('out','out/e2e')+'/'+client),hubBin=arg('hub-bin',process.env.HUB_BIN),gpuiBin=arg('gpui-bin',process.env.GPUI_BIN);
const J=JSON.parse(readFileSync(here+'/journeys.json','utf8'));const hz=Number(arg('hz',J.refresh_hz_budget[client]));
const adapter=await import(`./adapters/${client}.mjs`);mkdirSync(out,{recursive:true});
const todo=J.journeys.filter(j=>which==='all'||which===j.id);
const summary={client,when:new Date().toISOString(),hz,hub:hubBin||null,results:[],captures:[],env:{}};
let display=null,fx=null;
try{
  if(client==='gpui'){
    const cap=await import('./lib/capture.mjs');summary.env=cap.installMissing();
    if(!summary.env.tools.Xvfb||!summary.env.tools.i3)throw Error('no Xvfb/i3 on this machine (run on Boat): '+JSON.stringify(summary.env));
    if(!gpuiBin)throw Error('--gpui-bin (the Linux GPUI binary) is required');
    display=await cap.startDisplay(62);
  }
  if(client==='gpui'||hubBin){
    if(!hubBin)throw Error('--hub-bin is required');
    const {startFixtureHub}=await import('./lib/hubfx.mjs');
    fx=await startFixtureHub({bin:resolve(hubBin),root:resolve(arg('root','out/e2e-run')+'/'+client),port:Number(arg('port',3811)),threads:Number(arg('threads',20))});
    summary.seedReport=fx.seedReport;summary.hubPid=fx.hub.pid;
  }
  const {startAgents}=await import('./lib/hubfx.mjs');
  for(const j of todo){
    if(j.needs.includes('agents')&&fx)startAgents(fx,20);
    let r;try{r=await adapter.run(j,{gpuBin:gpuiBin?resolve(gpuiBin):null,display,hub:fx,outDir:out,hz,record:flag('record')})}catch(e){r={journey:j.id,client,status:'fail',metrics:{},notes:['harness error: '+String(e.stack||e).slice(0,400)]}}
    // a hub that refused a seed RPC the journey needs makes the journey blocked, not failed
    r.item=j.item;summary.results.push(r);for(const k of ['capture','screenshot'])if(r[k])summary.captures.push(r[k]);
    console.log(`${r.status.padEnd(8)} ${j.id.padEnd(16)} ${(r.blocked||[]).join(', ')} ${r.notes?.[0]||''}`.slice(0,200));
  }
}catch(e){summary.fatal=String(e.stack||e).slice(0,600);console.error('fatal:',summary.fatal)}
finally{try{await fx?.stop()}catch{}try{display?.stop()}catch{}}
// pass rule (CONTRACT.md): a pass needs the Rust hub on real hardware; anything else is demoted
for(const r of summary.results){if(r.status==='pass'&&!(hubKind==='rust'&&realHw&&r.gpu&&r.gpu.software===false)){r.status='partial';r.notes=[...(r.notes||[]),`would pass, but not on rust hub + real hardware (hub=${hubKind}, real-hardware=${realHw})`]}}
summary.hubKind=hubKind;summary.realHardware=realHw;
// gate mapping: pass -> pass, fail -> fail, everything else stays open
summary.gate=Object.fromEntries(summary.results.map(r=>[r.item,r.status==='pass'?'pass':r.status==='fail'?'fail':'open']));
summary.counts=summary.results.reduce((a,r)=>(a[r.status]=(a[r.status]||0)+1,a),{});
writeFileSync(out+'/summary.json',JSON.stringify(summary,null,1));console.log(JSON.stringify(summary.counts),'->',out+'/summary.json');
process.exit(flag('strict')&&(summary.fatal||summary.counts.fail)?1:0);
