// Fixture hub for the e2e journeys: Rust or JS hub, stand-in agents only, seeded project/threads/files/bots/rooms.
import {mkdirSync,writeFileSync,copyFileSync,existsSync,rmSync,readFileSync,chmodSync,realpathSync,cpSync} from 'node:fs';import {resolve as resolvePath} from 'node:path';
import {execSync} from 'node:child_process';
import {makeStandins,startHub,stopHub,client,sleep} from '../../perf/lib.mjs';
const PNG='iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAYAAADED76LAAAAF0lEQVR4nGNgGAWjYBSMglEwCkbBKBgFAAAEIAAB4rJbrgAAAABJRU5ErkJggg=='; // 8x8 PNG
// standin: 'perf' (test/tools/perf_standin.ts: zqshort/zqlong, FAKE_SECS) or 'story' (test/tools/claude_standin.ts in stream mode: tools:N, todos:N/D,
// ask:, delegate:, ... what the user-story journeys send). seed: 'fixture' (the threads below) or 'lived-in' (lib/seed.mjs: five projects, ~19 threads,
// three long ones, pinned/settled/snoozed/archived). Either way the hub's HOME and XDG folders are inside ROOT (a hub must never write the person's
// ~/.cache) and the vault keyring is a file in its home (never the login keyring).
export async function startFixtureHub({bin,root,port,threads=20,seedTurns=true,extraArgs=[],extraEnv={},standin='perf',seed='fixture',repo=process.env.PERF_REPO||process.cwd()}){
  rmSync(root,{recursive:true,force:true});mkdirSync(root+'/proj/fixtures',{recursive:true});makeStandins(root+'/bin');writeFileSync(root+'/'+E2E_MARK,'test/e2e fixture root\n');if(standin==='story')storyStandin(root+'/bin',repo);
  // KiStack as a release ships it beside the hub (the `$` popup lists its skills; a hub built in target/ has none beside it)
  extraEnv={...isolatedEnv(root),...(existsSync(repo+'/deploy/kistack/skills')?{BACKPLANE_KISTACK:repo+'/deploy/kistack'}:{}),...(standin==='story'?STORY_ENV:{}),...extraEnv};
  writeFileSync(root+'/proj/fixtures/pic.png',Buffer.from(PNG,'base64'));writeFileSync(root+'/proj/readme.txt','e2e fixture file\n');
  // the board and schematic come from test/ (drive.sh uploads only that folder to the hub machine; test/fixtures/ is not there)
  for(const [src,dst] of [['test/files/fixtures/plot/ecc83-pp.kicad_pcb','e2e.kicad_pcb'],['test/files/fixtures/plot/ecc83-pp.kicad_sch','e2e.kicad_sch'],['test/files/fixtures/mesh/bracket.step','part.step']])
    if(existsSync(`${repo}/${src}`))copyFileSync(`${repo}/${src}`,`${root}/proj/${dst}`);else console.error('fixture source missing: '+src);
  // a KiCad project next to the board and schematic, so the Board/Schematic/3D viewers have something to open
  writeFileSync(root+'/proj/e2e.kicad_pro',JSON.stringify({meta:{filename:'e2e.kicad_pro',version:1}}));writeFileSync(root+'/proj/.backplane.json',JSON.stringify({project:'e2e.kicad_pro'}));
  try{execSync('git init -q',{cwd:root+'/proj'})}catch{}
  const h=await startHub({bin,home:root+'/home',port,standins:root+'/bin',extraEnv:{FAKE_SECS:'600',FAKE_PER:'2',FAKE_CYCLE:'20',...extraEnv},extraArgs,log:root+'/hub.log'});
  const report={refused:[],ok:[]};const c=await client(h);
  const call=async(m,p)=>{const r=await c.rpc(m,p,10000);const ok=!!r?.o?.ok;(ok?report.ok:report.refused).push(ok?m:`${m}: ${r?.o?.error||'no reply'}`);return r?.o};
  const pr=await call('project.add',{path:root+'/proj'});
  const project=await c.until(8000,()=>c.seen.find(x=>x.$==='ProjectCreated'))||{id:pr?.project};
  if(standin==='story')await call('setting.set',{key:'order.enabled',value:'off'}); // (on by default for a person; the order server is a dead address besides)
  if(seed==='lived-in'){const {seedLivedIn}=await import('./seed.mjs');const lived=await seedLivedIn({c,call,root,repo,project:project?.id});await call('setting.set',{key:'onboard.done',value:'1'});
    return withRestart({hub:h,state:{url:h.url,token:h.token,project:project?.id,threads:lived.threads.map(x=>x.thread),lived,root,pid:h.pid},seedReport:report,client:c},{bin,port,standins:root+'/bin',extraEnv,extraArgs,log:root+'/hub.log'})}
  const ths=[];for(let i=0;i<threads;i++){const r=await call('thread.create',{project:project?.id,title:'e2e thread '+i,env:'local'});if(r?.thread)ths.push(r.thread)}
  if(seedTurns)for(const t of ths){c.send('turn.start',{thread:t,text:'zqshort seed history'})}
  if(seedTurns)await sleep(20000);
  for(const n of ['Bub','Mochi'])await call('bots.create',{name:n,persona:`${n} the stand-in bot`,provider:'claude',model:''});
  await sleep(1500);
  // a room: bots.room {room:'' (new), name, members: newline-separated bot ids}; the ids come from the bots' creation events when the hub logs them
  const botIds=[...new Set(c.seen.filter(x=>/Bot/.test(x.$||'')&&x.id&&['Bub','Mochi'].includes(x.name)).map(x=>x.id))];
  await call('bots.room',{room:'',name:'e2e-room',members:botIds.join('\n')});
  await call('setting.set',{key:'onboard.done',value:'1'});
  return withRestart({hub:h,state:{url:h.url,token:h.token,project:project?.id,threads:ths,root,pid:h.pid},seedReport:report,client:c},{bin,port,standins:root+'/bin',extraEnv,extraArgs,log:root+'/hub.log'});
}
// stop() and restart() (the same home and port: a journey's `hub restart` step) on a started hub; only the pid this harness started is signalled
function withRestart(fx,opts){
  const gone=async pid=>{for(let i=0;i<100&&existsSync(`/proc/${pid}`);i++)await sleep(100)};
  fx.stop=async()=>{try{fx.client.ws.close()}catch{}stopHub(fx.hub);await gone(fx.hub.pid);await sleep(300)};
  fx.restart=async(downMs=0)=>{try{fx.client.ws.close()}catch{}stopHub(fx.hub);await gone(fx.hub.pid);if(downMs)await sleep(downMs);
    fx.hub=await startHub({...opts,home:fx.state.root+'/home'});fx.client=await client(fx.hub);fx.state.pid=fx.hub.pid;return fx.hub.pid};
  return fx}
export const isolatedEnv=root=>{const u=root+'/userhome';for(const d of ['.cache','.config','.local/share','.local/state'])mkdirSync(`${u}/${d}`,{recursive:true});
  return {HOME:u,XDG_CACHE_HOME:u+'/.cache',XDG_CONFIG_HOME:u+'/.config',XDG_DATA_HOME:u+'/.local/share',XDG_STATE_HOME:u+'/.local/state',BACKPLANE_VAULT_KEYRING:'file'}};
const STORY_ENV={BP_STANDIN_MODE:'stream',BP_STANDIN_USAGE:'1',BP_STREAM_N:'60',BP_STREAM_MS:'50',BP_STREAM_SEP:'\n'};
function storyStandin(dir,repo){const bun=/bun/.test(process.execPath)?process.execPath:'bun';writeFileSync(dir+'/claude',`#!/bin/sh\nexec "${bun}" "${repo}/test/tools/claude_standin.ts" "$@"\n`);chmodSync(dir+'/claude',0o755)}

// A hub on a COPY of a home (real-data runs): the source is only read (events.jsonl up to its last full line, attachments); the copy gets
// restart.continue off (no cut turn resumes), BACKPLANE_COPY=1 (docs/COPY-GUARD.md: no network, no git that writes, no watched work, stand-ins
// only), the shell stand-in (it writes nothing but its own log), and its own HOME/XDG/vault. A source that looks like a person's real store is
// refused unless E2E_COPY_REAL=person (the person's word, given for this run); a home this harness made (its root carries .e2e-root) is fine.
export const E2E_MARK='.e2e-root';
export const LIVE_HOMES=['.backplane-bend','.backplane','.backplane-gpui','.backplane-gpui-fresh','.backplane-gpui-copy','.local/share/backplane'];
export function looksReal(src){const home=process.env.HOME||'';const r=realpathSync(src);
  for(const l of LIVE_HOMES){const p=home+'/'+l;if(r===p||r.startsWith(p+'/'))return 'a live home ('+p+')'}
  if(existsSync(src+'/store.db')||existsSync(src+'/live-checkpoints'))return 'it holds another hub\'s store.db or live checkpoints';
  if(existsSync(src+'/../'+E2E_MARK))return null; // a home this harness made (startFixtureHub/startCopyHub mark their root)
  return 'not a home this harness made (no '+E2E_MARK+' beside it)'}
export async function startCopyHub({bin,source,root,port,extraEnv={},repo=process.env.PERF_REPO||process.cwd()}){
  if(!existsSync(source+'/events.jsonl'))throw Error('copy: no events.jsonl in '+source);
  const src=realpathSync(source),dst=resolvePath(root);if(dst.startsWith(src+'/')||dst===src||src.startsWith(dst+'/'))throw Error('copy: the copy root and the source overlap');
  const why=looksReal(src);if(why&&process.env.E2E_COPY_REAL!=='person')throw Error(`copy: ${src} looks like a real store (${why}); a copy of a real store needs E2E_COPY_REAL=person (the person's word for this run)`);
  rmSync(root,{recursive:true,force:true});mkdirSync(root+'/home',{recursive:true});makeStandins(root+'/bin');writeFileSync(root+'/'+E2E_MARK,'test/e2e copy root of '+src+'\n');
  const log=readFileSync(src+'/events.jsonl');const end=log.lastIndexOf(10)+1; // a hub may be appending: only whole lines
  writeFileSync(root+'/home/events.jsonl',Buffer.concat([log.subarray(0,end),Buffer.from(JSON.stringify({$:'SettingSet',key:'restart.continue',value:'off',at:Math.floor(Date.now()/1000)})+'\n')]));
  for(const d of ['attachments','images'])if(existsSync(src+'/'+d))cpSync(src+'/'+d,root+'/home/'+d,{recursive:true});
  const env={...isolatedEnv(root),...(existsSync(repo+'/deploy/kistack/skills')?{BACKPLANE_KISTACK:repo+'/deploy/kistack'}:{}),BACKPLANE_COPY:'1',...extraEnv};
  const h=await startHub({bin,home:root+'/home',port,standins:root+'/bin',extraEnv:env,log:root+'/hub.log'});
  const pe=readFileSync(`/proc/${h.pid}/environ`,'utf8').split('\0');if(!pe.includes('BACKPLANE_COPY=1')){stopHub(h);throw Error('copy: the hub does not run with BACKPLANE_COPY=1: stopped')}
  const c=await client(h);const threads=[];
  return withRestart({hub:h,state:{url:h.url,token:h.token,threads,root,pid:h.pid,copyOf:src,copyReal:!!why},seedReport:{ok:[],refused:[],copy:{source:src,bytes:end,real:why||null}},client:c},{bin,port,standins:root+'/bin',extraEnv:env,log:root+'/hub.log'})}
export const startAgents=(fx,count)=>{for(const t of fx.state.threads.slice(0,count))fx.client.send('turn.start',{thread:t,text:'stream for the long session'})};
