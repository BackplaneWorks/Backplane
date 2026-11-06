// Start a hub binary on a fresh fixture home with stand-in agents, verified from its own environment.
import {mkdirSync,readFileSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import {makeStandins,startHub,stopHub} from '../perf/lib.mjs';
import {sleep} from './lib.mjs';
import {makeFixture} from './fixture.mjs';
// the hub's own start-up jobs (machine name, time zone, accounts...) finish at varying times: wait until its log has been quiet for 3 s
async function quiesce(h){
  const ws=new WebSocket(`ws://${h.host}:${h.port}/ws?token=${h.token}`);let last=Date.now(),n=0;
  ws.onmessage=e=>{last=Date.now();n++};await new Promise(r=>ws.onopen=r);
  const end=Date.now()+30000;while(Date.now()-last<3000&&Date.now()<end)await sleep(100);ws.close();await sleep(200);
}
import {fileURLToPath} from 'node:url';
export const R=process.env.BACKPLANE_PARITY_ROOT||fileURLToPath(new URL('../../..',import.meta.url)).replace(/\/$/,'');
// where fixture homes and project folders live: outside any git checkout, or the hub makes worktrees with random names
export const WORK=process.env.PARITY_WORK||(R+'/build/gpui/parity');
export async function startFixtureHub({bin,port,work,extraArgs,deleted=false,privateLines=false}){
  const home=work+'/home';mkdirSync(work+'/fixroot',{recursive:true});makeFixture(home,work+'/fixroot',{deleted,privateLines});makeStandins(work+'/bin');
  const h=await startHub({bin,home,port,cap:'3700M',standins:work+'/bin',extraArgs:(process.env.PARITY_HUB_ARGS||'').split(' ').filter(Boolean),extraEnv:{TZ:'Etc/UTC',BACKPLANE_VAULT_KEYRING:'file',FAKE_SECS:'2',FAKE_PER:'2',FAKE_CYCLE:'5'}});
  const check=execFileSync(R+'/test/gpui/slim/check_agents.sh',[String(h.pid),work+'/bin']).toString().trim();
  await quiesce(h);
  mkdirSync(work+'/proj',{recursive:true});mkdirSync(work+'/proj-2',{recursive:true});
  h.projectDir=work+'/proj';h.work=work;h.check=check;
  h.finish=async()=>{await sleep(300);
    const log=readFileSync(home+'/events.jsonl','utf8');
    const sessions=[...log.matchAll(/"session":"([^"]+)"/g)].map(m=>m[1]);
    if(sessions.some(x=>!x.startsWith('fake-')))throw Error('a non-stand-in session id in the log: '+sessions.join(','));
    stopHub(h);await sleep(800);return sessions.length};
  return h;
}
