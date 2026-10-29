'use strict';
// Bounded ASCII-home startup of the exact supplied compiled stage. The normal
// repository desktop smoke separately exercises its Unicode/space home.
const fs=require('node:fs'),path=require('node:path'),os=require('node:os');
const [stage,tools,out]=process.argv.slice(2),s=JSON.parse(fs.readFileSync(path.join(stage,'STAGE.json')));
const producer=require(path.join(tools,'desktop/produce-release.cjs'));
const resources=path.join(stage,producer.TARGETS[s.target].resources),binary=path.join(resources,'backplane-service',process.platform==='win32'?'backplane-serve.exe':'backplane-serve');
const {Service}=require(path.join(resources,'app/service.cjs'));
const home=fs.mkdtempSync(path.join(os.tmpdir(),'backplane-ascii-startup-')),agents=path.join(home,'agents');
fs.mkdirSync(agents);
fs.writeFileSync(path.join(home,'events.jsonl'),['mail.on','restart.continue','telemetry.share'].map(key=>JSON.stringify({$:'SettingSet',key,value:'off',at:Math.floor(Date.now()/1000)})).join('\n')+'\n');
Object.assign(process.env,{BACKPLANE_HOME:home,BACKPLANE_AGENTS:agents,BACKPLANE_NO_UPDATE:'1',BACKPLANE_TELEMETRY:'off',BACKPLANE_PEERS:''});
for(const key of Object.keys(process.env))if(/TOKEN|API_KEY|SECRET|PASSWORD|CREDENTIAL|AUTH_COOKIE/.test(key))delete process.env[key];
const service=new Service({home,binary,web:path.join(resources,'backplane-service/web'),expectedVersion:s.release.version,residentService:true});
(async()=>{let timer,error,connection;const started=Date.now();try{connection=await Promise.race([service.start(),new Promise((_,reject)=>timer=setTimeout(()=>reject(Error('Bounded startup timeout')),30000))]);}catch(e){error=String(e.message).slice(0,220);}finally{clearTimeout(timer);if(service.child&&service.child.exitCode===null){service.child.kill('SIGTERM');await Promise.race([new Promise(r=>service.child.once('exit',r)),new Promise(r=>setTimeout(r,3000))]);}}
const exited=!service.child||service.child.exitCode!==null||service.child.signalCode!==null;
const result={status:!error&&exited?'PASS':'FAIL',sourceCommit:s.release.commit,sourceTree:s.release.tree,target:s.target,version:connection?.version,error,wallMs:Date.now()-started,ownedHome:home,capturedChildPid:service.child?.pid,childExited:exited,scope:'Actual compiled stage ASCII-home startup only; Unicode home and UI checked by standard desktop smoke; no private profiles or providers'};
fs.mkdirSync(path.dirname(out),{recursive:true});fs.writeFileSync(out,JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify({...result,ownedHome:undefined}));if(result.status!=='PASS')process.exitCode=1;
})().catch(()=>{console.error('Owned ASCII startup runner failed; no raw payload output printed');process.exitCode=1;});
