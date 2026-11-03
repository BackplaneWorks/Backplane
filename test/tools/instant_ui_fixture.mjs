// The latency runner owns its hub. There is deliberately no endpoint option.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import net from 'node:net';
import {randomBytes} from 'node:crypto';
import {spawn} from 'node:child_process';

export function configuration(env=process.env, args=process.argv.slice(2)) {
 for (const key of ['INSTANT_ORIGIN','INSTANT_HOME','INSTANT_TOKEN','INSTANT_CODE','INSTANT_INVITE']) {
  if (Object.hasOwn(env,key)) throw Error(`${key} is forbidden: latency tests create their own isolated hub`);
 }
 if (args.length) throw Error('No target, invitation or command-line arguments accepted; isolated hubs only');
 if (!env.INSTANT_HUB || !env.INSTANT_WEB) throw Error('INSTANT_HUB and INSTANT_WEB required: explicit binary and web assets, never a hub address');
 const binary=fs.realpathSync(env.INSTANT_HUB), web=fs.realpathSync(env.INSTANT_WEB);
 if (!fs.statSync(binary).isFile() || !fs.statSync(web).isDirectory() || !fs.existsSync(path.join(web,'index.html'))) throw Error('Expected a hub binary and emitted web directory');
 fs.accessSync(binary,fs.constants.X_OK);
 return {binary,web};
}

export function assertOwnedURL(url, origin, websocket=false) {
 const parsed=new URL(url), owned=new URL(origin);
 const expected=websocket?'ws:':'http:';
 if (parsed.protocol!==expected || parsed.hostname!=='127.0.0.1' || parsed.port!==owned.port || parsed.username || parsed.password) throw Error('Blocked request outside the owned isolated hub');
 return parsed.href;
}

// WebKit routes blob-backed workers through the resource interceptor. They
// resolve in browser memory; only blobs created by this fixture origin pass.
export function assertOwnedResource(url, origin) {
 const parsed=new URL(url);
 if(parsed.protocol==='blob:' && parsed.origin===origin)return parsed.href;
 return assertOwnedURL(url,origin);
}

export async function startFixture(config) {
 const root=fs.mkdtempSync(path.join(os.tmpdir(),'bp-instant-isolated-'));
 fs.chmodSync(root,0o700);
 const home=path.join(root,'home'), project=path.join(root,'project'), agents=path.join(root,'agents');
 for (const dir of [home,project,agents]) fs.mkdirSync(dir,{mode:0o700});
 const token=randomBytes(32).toString('hex');
 fs.writeFileSync(path.join(home,'token'),token,{mode:0o600});
 const claude=`#!/bin/sh
if [ "$1" = "--version" ]; then echo '2.1.286 (isolated fixture)'; exit 0; fi
n=0
while IFS= read -r line; do
 case "$line" in
 *'"type":"user"'*)
 n=$((n + 1))
 printf '%s\\n' '{"type":"assistant","message":{"id":"fixture-'"$$-$n"'","role":"assistant","content":[{"type":"text","text":"ACK from isolated stand-in"}]}}'
 printf '%s\\n' '{"type":"result","is_error":false,"result":"ACK from isolated stand-in"}' ;;
 esac
done
`;
 // Block every other provider and discovery helper, including those found by
 // the login-shell PATH probe. BACKPLANE_AGENTS also enforces stand-in-only.
 for (const name of ['claude','codex','grok','muse','opencode','pi','omp','agy_acp_server','tailscale']) fs.writeFileSync(path.join(agents,name),name==='claude'?claude:'#!/bin/sh\nexit 127\n',{mode:0o700});
 const at=Math.floor(Date.now()/1000)-1000;
 const rows=[{$:'ProjectCreated',id:'fixture-project',title:'Isolated latency fixture',root:project,at}];
 for (const [key,value] of Object.entries({'restart.continue':'off','telemetry.share':'off','fallback.on':'off','onboard.done':'1','onboard.uber':'1','sidebar.view':'projects'})) rows.push({$:'SettingSet',key,value,at});
 const threads=['fixture-short','fixture-long','fixture-medium','fixture-other','fixture-last'];
 for (let t=0;t<threads.length+2;t++) {
  const id=threads[t] || 'fixture-bot-thread-'+(t-5);
  rows.push({$:'ThreadCreated',id,project:'fixture-project',title:'Isolated '+id,env:'local',branch:'',worktree:project,provider:'claude',model:'fixture',at});
  const count=t===1?240:t===2?48:4;
  for(let i=0;i<count;i++) rows.push({$:'MessagePosted',thread:id,msg:id+'-'+i,role:i%2?'assistant':'user',text:'Synthetic '+id+' message '+i+(t===1?'\n\nFixture paragraph. '.repeat(20):''),at:at+i});
  rows.push({$:'TurnChanged',thread:id,state:'completed',at:at+count});
  if(t>=5) rows.push({$:'BotSet',id:'fixture-bot-'+(t-5),name:'Fixture bot '+(t-5),thread:id,look:0,persona:'Isolated fixture only',at});
 }
 const log=path.join(home,'events.jsonl');fs.writeFileSync(log,rows.map(x=>JSON.stringify(x)).join('\n')+'\n');
 const listener=net.createServer();await new Promise((resolve,reject)=>{listener.once('error',reject);listener.listen(0,'127.0.0.1',resolve)});
 const port=listener.address().port;await new Promise(resolve=>listener.close(resolve));
 if(port===3787) throw Error('Refusing the live hub port');
 const origin='http://127.0.0.1:'+port;
 const stdout=fs.openSync(path.join(root,'stdout'),'w'), stderr=fs.openSync(path.join(root,'stderr'),'w');
 // Do not inherit tokens, peer addresses, provider configuration, proxy
 // settings or a live BACKPLANE_HOME. All writable hub paths belong to root.
 const env={PATH:agents+':/usr/bin:/bin',LANG:'C.UTF-8',TMPDIR:root,XDG_CONFIG_HOME:path.join(root,'config'),XDG_CACHE_HOME:path.join(root,'cache'),XDG_DATA_HOME:path.join(root,'data'),BACKPLANE_AGENTS:agents,BACKPLANE_PEERS:'',BACKPLANE_NAME:'Isolated latency fixture',BACKPLANE_KISTACK_SYNC:'0',BACKPLANE_NO_UPDATE:'1',BACKPLANE_TELEMETRY:'off',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',BEND_NO_TELEMETRY:'1'};
 const child=spawn(config.binary,['--home',home,'--web',config.web,'--port',String(port),'--host','127.0.0.1','--headless','--no-tailscale'],{env,cwd:project,detached:true,stdio:['ignore',stdout,stderr]});
 fs.closeSync(stdout);fs.closeSync(stderr);
 let spawnError;child.on('error',e=>{spawnError=e});
 const exited=new Promise(resolve=>child.once('close',resolve));
 const terminate=signal=>{if(child.pid)try{process.kill(-child.pid,signal)}catch{}};
 const onExit=()=>terminate('SIGKILL');process.once('exit',onExit);
 const stop=async()=>{
  if(!spawnError) {
   terminate('SIGTERM');
   const timer=setTimeout(()=>terminate('SIGKILL'),5000);
   try{await exited}finally{clearTimeout(timer)}
  }
  process.removeListener('exit',onExit);
 };
 const request=async(endpoint,options={})=>{
  if(child.exitCode!==null || spawnError) throw Error('Owned isolated hub is not alive');
  assertOwnedURL(origin+endpoint,origin);
  return fetch(origin+endpoint,{...options,redirect:'error',signal:AbortSignal.timeout(2000)});
 };
 try {
  const deadline=Date.now()+45000;
  for(;;) {
   if(spawnError) throw spawnError;
   if(child.exitCode!==null) throw Error('Isolated hub exited; inspect '+root+'/stderr');
   // A reserved port can be stolen before spawn. Require the child-owned
   // lock receipt and the fresh private bearer before any mutating request.
   let lock='';try{lock=fs.readFileSync(path.join(home,'hub.lock'),'utf8')}catch{}
   if(lock.startsWith(child.pid+' '+port+' flock')) {
    try {if((await request('/auth/cache',{headers:{Authorization:'Bearer '+token}})).ok) break}catch{}
   }
   if(Date.now()>deadline) throw Error('Isolated hub startup timeout; inspect '+root);
   await new Promise(resolve=>setTimeout(resolve,50));
  }
  return {root,home,origin,threads,log,request,stop,
   async invitation(){const r=await request('/auth/invitations',{method:'POST',headers:{Authorization:'Bearer '+token}});if(!r.ok)throw Error('Private invitation failed (HTTP '+r.status+'); explicit hub binary must support invitation pairing');return (await r.json()).code},
   evidence(){const events=fs.readFileSync(log,'utf8').trim().split('\n').map(line=>JSON.parse(line));return {root,home,port,pid:child.pid,threads:events.filter(e=>e.$==='ThreadCreated').map(e=>e.id),messages:events.filter(e=>e.$==='MessagePosted').length,standinACKs:events.filter(e=>e.$==='MessagePosted'&&e.text==='ACK from isolated stand-in').length}},
  };
 } catch(e) {await stop();throw e}
}
