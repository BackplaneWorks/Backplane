// Fixture hub for the e2e journeys: Rust or JS hub, stand-in agents only, seeded project/threads/files/bots/rooms.
import {mkdirSync,writeFileSync,copyFileSync,existsSync,rmSync,readFileSync} from 'node:fs';
import {execSync} from 'node:child_process';
import {makeStandins,startHub,stopHub,client,sleep} from '../../perf/lib.mjs';
const PNG='iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAYAAADED76LAAAAF0lEQVR4nGNgGAWjYBSMglEwCkbBKBgFAAAEIAAB4rJbrgAAAABJRU5ErkJggg=='; // 8x8 PNG
export async function startFixtureHub({bin,root,port,threads=20,seedTurns=true,extraArgs=[],extraEnv={},repo=process.env.PERF_REPO||process.cwd()}){
  rmSync(root,{recursive:true,force:true});mkdirSync(root+'/proj/fixtures',{recursive:true});makeStandins(root+'/bin');
  writeFileSync(root+'/proj/fixtures/pic.png',Buffer.from(PNG,'base64'));writeFileSync(root+'/proj/readme.txt','e2e fixture file\n');
  // the board and schematic come from test/gpui/ (drive.sh uploads only that folder to the hub machine; test/fixtures/ is not there)
  for(const [src,dst] of [['test/gpui/files/fixtures/plot/ecc83-pp.kicad_pcb','e2e.kicad_pcb'],['test/gpui/files/fixtures/plot/ecc83-pp.kicad_sch','e2e.kicad_sch'],['test/gpui/files/fixtures/mesh/bracket.step','part.step']])
    if(existsSync(`${repo}/${src}`))copyFileSync(`${repo}/${src}`,`${root}/proj/${dst}`);else console.error('fixture source missing: '+src);
  // a KiCad project next to the board and schematic, so the Board/Schematic/3D viewers have something to open
  writeFileSync(root+'/proj/e2e.kicad_pro',JSON.stringify({meta:{filename:'e2e.kicad_pro',version:1}}));writeFileSync(root+'/proj/.backplane.json',JSON.stringify({project:'e2e.kicad_pro'}));
  try{execSync('git init -q',{cwd:root+'/proj'})}catch{}
  const h=await startHub({bin,home:root+'/home',port,standins:root+'/bin',extraEnv:{FAKE_SECS:'600',FAKE_PER:'2',FAKE_CYCLE:'20',...extraEnv},extraArgs,log:root+'/hub.log'});
  const report={refused:[],ok:[]};const c=await client(h);
  const call=async(m,p)=>{const r=await c.rpc(m,p,10000);const ok=!!r?.o?.ok;(ok?report.ok:report.refused).push(ok?m:`${m}: ${r?.o?.error||'no reply'}`);return r?.o};
  const pr=await call('project.add',{path:root+'/proj'});
  const project=await c.until(8000,()=>c.seen.find(x=>x.$==='ProjectCreated'))||{id:pr?.project};
  const ths=[];for(let i=0;i<threads;i++){const r=await call('thread.create',{project:project?.id,title:'e2e thread '+i,env:'local'});if(r?.thread)ths.push(r.thread)}
  if(seedTurns)for(const t of ths){c.send('turn.start',{thread:t,text:'zqshort seed history'})}
  if(seedTurns)await sleep(20000);
  for(const n of ['Bub','Mochi'])await call('bots.create',{name:n,persona:`${n} the stand-in bot`,provider:'claude',model:''});
  await sleep(1500);
  // a room: bots.room {room:'' (new), name, members: newline-separated bot ids}; the ids come from the bots' creation events when the hub logs them
  const botIds=[...new Set(c.seen.filter(x=>/Bot/.test(x.$||'')&&x.id&&['Bub','Mochi'].includes(x.name)).map(x=>x.id))];
  await call('bots.room',{room:'',name:'e2e-room',members:botIds.join('\n')});
  await call('setting.set',{key:'onboard.done',value:'1'});
  return {hub:h,state:{url:h.url,token:h.token,project:project?.id,threads:ths,root,pid:h.pid},seedReport:report,client:c,
    stop:async()=>{try{c.ws.close()}catch{}stopHub(h);await sleep(1500)}};
}
export const startAgents=(fx,count)=>{for(const t of fx.state.threads.slice(0,count))fx.client.send('turn.start',{thread:t,text:'stream for the long session'})};
