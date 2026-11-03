// Candidate hub on this machine, paired mobile browser on h@backplane.
// One local job owns bend for the hub's lifetime. All state is disposable.
import assert from 'node:assert/strict';
import {mkdtempSync,mkdirSync,writeFileSync,readFileSync,rmSync,existsSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
const [binary,web]=process.argv.slice(2);
const log=(stage:string)=>console.log(new Date().toISOString()+' '+stage);
const abort=new AbortController();process.on('SIGTERM',()=>abort.abort());
async function step<T>(label:string,p:Promise<T>,ms=30000):Promise<T>{
 log(label);let timer:any,stop:any;
 try{return await Promise.race([p,new Promise<T>((_,reject)=>{timer=setTimeout(()=>reject(Error(label+' timed out after '+ms+'ms')),ms);stop=()=>reject(Error('test cancelled during '+label));abort.signal.addEventListener('abort',stop,{once:true});if(abort.signal.aborted)stop();})]);}
 finally{clearTimeout(timer);if(stop)abort.signal.removeEventListener('abort',stop);}
}
assert(binary&&web,'HUB WEB required');
const root=mkdtempSync(join(tmpdir(),'bp-message-image-acceptance-')),home=join(root,'home'),fake=join(root,'agents');
for(const d of [home,fake])mkdirSync(d,{mode:0o700});
for(const cli of ['claude','codex','grok','muse','opencode','pi','omp','tailscale','gh'])writeFileSync(join(fake,cli),'#!/bin/sh\nexit 127\n',{mode:0o755});
const paths=['dark-1280','dark-390','light-1280','light-390'].map(x=>'/home/h/.backplane-bend/exports/star-nudge-20261103/compare-'+x+'.png');
paths.forEach(p=>assert(existsSync(p),p));
const thread='t1790292962-8017',bot='b1790292962-8017',entry='test-star-nudge-regenerated';
const ev=($:string,fields:object)=>({$,at:1793670000,...fields});
const events=[ev('ProjectCreated',{id:'p-images',title:'TEST image acceptance',root}),ev('ThreadCreated',{id:thread,project:'p-images',title:'Bub',env:'local',branch:'',worktree:'',provider:'claude',model:''}),ev('BotSet',{id:bot,name:'Bub',thread,look:0,persona:''}),ev('SettingSet',{key:'onboard.done',value:'1'}),ev('SettingSet',{key:'onboard.uber',value:'1'}),ev('MessagePosted',{thread,msg:entry,role:'assistant',text:'Regenerated #775 comparison sheets: before, version A, and version B preview.\n\n'+paths.join('\n')})];
writeFileSync(join(home,'events.jsonl'),events.map(e=>JSON.stringify(e)+'\n').join(''));
const lease=Bun.listen({hostname:'0.0.0.0',port:0,socket:{data(){}}});const port=lease.port;lease.stop(true);
const base='http://127.0.0.1:'+port,origin='http://desktop.tail9e0cf2.ts.net:'+port;
const out=resolve('build/message-images/acceptance');mkdirSync(out,{recursive:true});
log('Starting disposable candidate hub');
const child=Bun.spawn([resolve(binary),'--home',home,'--web',resolve(web),'--port',String(port),'--host','0.0.0.0','--no-tailscale','--foreground','--headless'],{env:{...process.env,BACKPLANE_AGENTS:fake,BACKPLANE_COPY:'1',BACKPLANE_KISTACK_SYNC:'0',BACKPLANE_NO_UPDATE:'1',BACKPLANE_VAULT_KEYRING:'file',BACKPLANE_TELEMETRY:'off',BACKPLANE_PEERS:'',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9'},stdout:Bun.file(join(out,'hub.stdout')),stderr:Bun.file(join(out,'hub.stderr'))});
let rootToken='',device:any,route='',browser:any;let done=false;const remoteLog:string[]=[];
async function capture(stream:ReadableStream<Uint8Array>){const decoder=new TextDecoder();for await(const bytes of stream){const text=decoder.decode(bytes,{stream:true});remoteLog.push(text);process.stdout.write(text.replace(/[a-f0-9]{32}/g,'[redacted]'));}}
const api=async(path:string,token:string,method='GET',headers:Record<string,string>={})=>fetch(base+path,{method,signal:AbortSignal.timeout(10000),headers:{Authorization:'Bearer '+token,...headers}});
try{
 const until=Date.now()+60000;
 while(true){if(abort.signal.aborted)throw Error('test cancelled at startup');try{if((await fetch(base+'/hello',{signal:AbortSignal.timeout(3000)})).ok)break;}catch{}if(child.exitCode!==null)throw Error('candidate hub exited');if(Date.now()>until)throw Error('candidate hub timeout');await Bun.sleep(80);}
 log('Candidate hub ready at '+origin);
 rootToken=readFileSync(join(home,'token'),'utf8').trim();
 log('Creating disposable TEST pairing');
 const invitation=await(await api('/auth/invitations',rootToken,'POST')).json();assert(invitation.code);
 device=await(await api('/auth/enroll',invitation.code,'POST',{'X-Backplane-Client':'native','X-Backplane-Name':'TEST remote message images'})).json();assert(device.token&&device.id);
 browser=Bun.spawn(['ssh','h@backplane','cd /home/h/bp-message-images && timeout --kill-after=5 180 /home/h/.bun/bin/bun test/tools/message_images_browser.mjs'],{stdin:'pipe',stdout:'pipe',stderr:'pipe'});
 browser.stdin.write(JSON.stringify({origin,token:device.token,paths,entry,out:'/home/h/bp-message-images/build/message-images/acceptance',bot:'Bub',botId:bot}));browser.stdin.end();
 await step('Remote paired browser assertions',Promise.all([capture(browser.stdout),capture(browser.stderr)]),200000);
 writeFileSync(join(out,'remote.log'),remoteLog.join(''));assert.equal(await browser.exited,0,'remote paired mobile browser');
 route='/message-image?entry='+entry+'&path='+encodeURIComponent(paths[0]);
 assert.equal((await api(route,device.token)).status,200);done=true;
 console.log('PASS remote four-PNG mobile conversation, browser/native auth and negative controls');
 }finally{
 log('Cleanup: revoke TEST device and stop owned candidate processes');
 if(browser&&browser.exitCode===null){browser.kill();await browser.exited;}
 try {
  if(device?.id){const r=await api('/auth/revoke',rootToken,'POST',{'X-Backplane-Device':device.id});assert.equal(r.status,200);assert.equal((await api('/message-image?entry='+entry+'&path='+encodeURIComponent(paths[0]),device.token)).status,401);console.log('PASS TEST device revoked; image request rejected afterwards');}
 } finally {
  writeFileSync(join(out,'remote.log'),remoteLog.join(''));child.kill();await child.exited;rmSync(root,{recursive:true,force:true});log('Cleanup complete; runtime '+(done?'passed':'failed'));
 }
}
