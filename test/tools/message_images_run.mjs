// One managed job, deterministic lock order: reserve remote browser admission
// before taking the local bend lock. Waiting for a remote proof holds no local
// heavy-work slot. No providers, build or installation runs in this test.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {createReadStream,readdirSync,readFileSync} from 'node:fs';
import {join,resolve} from 'node:path';
const [binary,web]=process.argv.slice(2);assert(binary&&web,'HUB WEB required');
const log=x=>console.log(new Date().toISOString()+' '+x);
const abort=new AbortController();process.on('SIGTERM',()=>abort.abort());
async function timed(label,p,ms){
 log(label);let timer,listener;
 try{return await Promise.race([p,new Promise((_,reject)=>{timer=setTimeout(()=>reject(Error(label+' timed out after '+ms+'ms')),ms);listener=()=>reject(Error('Cancelled during '+label));abort.signal.addEventListener('abort',listener,{once:true});if(abort.signal.aborted)listener();})]);}
 finally{clearTimeout(timer);abort.signal.removeEventListener('abort',listener);}
}
async function hash(file){const h=createHash('sha256');for await(const chunk of createReadStream(file))h.update(chunk);return h.digest('hex');}
const webFiles=[];function walk(dir,base=''){for(const e of readdirSync(dir,{withFileTypes:true})){const p=join(base,e.name);if(e.isDirectory())walk(join(dir,e.name),p);else webFiles.push(p);}}walk(web);
const h=createHash('sha256');for(const p of webFiles.sort())h.update(p+'\0'+await hash(join(web,p))+'\n');
log('Runtime artifact base: Boat j20261103022433-3715563, source product inputs from 7982813b+packet; product tree equals 128fd23c3');
log('Candidate SHA256 '+await hash(binary));log('Web tree SHA256 '+h.digest('hex'));
let reserve,run;
try{
 reserve=Bun.spawn(['ssh','h@backplane',`cd /home/h/bp-message-images && timeout --kill-after=5 900 scripts/bp-lock.sh bend -- sh -c 'echo MESSAGE_IMAGE_REMOTE_READY; read -r release'`],{stdin:'pipe',stdout:'pipe',stderr:'inherit'});
 const reader=reserve.stdout.getReader();
 await timed('Wait for remote bend admission (no local lock held)',(async()=>{let text='';for(;;){const {value,done}=await reader.read();if(done)throw Error('Remote admission closed');text+=new TextDecoder().decode(value);if(text.includes('MESSAGE_IMAGE_REMOTE_READY'))break;}reader.releaseLock();})(),600000);
 log('Remote browser admission reserved; acquiring local bend');
 run=Bun.spawn(['scripts/bp-lock.sh','bend','--',process.execPath,'test/tools/message_images_e2e.ts',resolve(binary),resolve(web)],{stdout:'inherit',stderr:'inherit'});
 const code=await timed('One local candidate and remote browser job',run.exited,840000);assert.equal(code,0,'paired mobile runtime acceptance');
 log('PASS complete acceptance job');
}finally{
 if(run&&run.exitCode===null){run.kill();await run.exited;}
 if(reserve){try{reserve.stdin.write('release\n');reserve.stdin.end();}catch{}await timed('Release remote browser reservation',reserve.exited,15000).catch(()=>reserve.kill());}
 log('Owned lock reservations closed');
}
