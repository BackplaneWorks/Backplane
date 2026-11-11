// node step_timing.mjs HUB_BIN OUT.json STEP...     (env STEP2GLB: the backplane-step2glb to use, or "off" for FreeCAD alone; PORT: default 3920)
// A STEP opened in a 3D view, timed through a real hub (stand-in agents, temp HOME and models cache): from the watch request to the parts'
// start, the first streamed part, and the whole model, each file cold (an empty cache), opened again on the same hub, and opened on a hub
// started afresh on that cache (the cached reopen after a restart). The frames are told apart by their CBOR map's size: parts {3}, a part
// {8}, the model {5} (kicad/plot.rs). Results: one row a file and run, and the model's bytes.
import {mkdirSync,rmSync,writeFileSync,copyFileSync} from 'node:fs';
import {makeStandins,startHub,stopHub,sleep} from '../perf/lib.mjs';
import {decode,encode} from '../wire.mjs';
const [bin,out,...steps]=process.argv.slice(2);
const BP_ROOT=new URL('../..',import.meta.url).pathname.replace(/\/$/,'');
const R=`${process.env.BP_TMP||BP_ROOT+'/build'}/step-timing-${process.pid}`;rmSync(R,{recursive:true,force:true});mkdirSync(R+'/proj',{recursive:true});makeStandins(R+'/bin');
// each file alone in a project of its own: the Mech read-ahead (watch.rs warm_mech) meshes a shown part's neighbours, which would make every
// file after the first a cached one
const proj=R+'/proj';const names=steps.map((s,i)=>{const n=s.split('/').pop();mkdirSync(`${proj}/p${i}/mech`,{recursive:true});copyFileSync(s,`${proj}/p${i}/mech/${n}`);return `p${i}/mech/${n}`});
const log=[{$:'ProjectCreated',id:'p1',title:'p',root:proj,at:1},{$:'ThreadCreated',id:'t1',project:'p1',title:'T',env:'local',branch:'',worktree:'',provider:'claude',model:'',at:2}].map(x=>JSON.stringify(x)).join('\n')+'\n';
const uh=R+'/user';mkdirSync(uh,{recursive:true});
const env={HOME:uh,XDG_CACHE_HOME:uh+'/.cache',XDG_CONFIG_HOME:uh+'/.config',XDG_DATA_HOME:uh+'/.local/share',BACKPLANE_PLOT_BIN:'/nonexistent/backplane-serve',...(process.env.STEP2GLB?{BACKPLANE_STEP2GLB:process.env.STEP2GLB}:{})};
const port=Number(process.env.PORT||3920);const rows=[];
async function hub(n){
  const home=`${R}/home${n}`;mkdirSync(home,{recursive:true});writeFileSync(home+'/events.jsonl',log);
  const h=await startHub({bin,home,port,standins:R+'/bin',extraEnv:env,log:R+`/hub${n}.log`});
  const frames=[];const replies=new Map();
  const ws=new WebSocket(`ws://127.0.0.1:${port}/ws?view=1&mobile=1&enc=cbor&token=${h.token}`);ws.binaryType='arraybuffer';
  ws.onmessage=({data})=>{const raw=Buffer.from(data);const at=performance.now();
    if(raw.subarray(1,7).toString('hex')==='0164706c6f74'){const kind={0xa3:'parts',0xa8:'part',0xa5:'model'}[raw[0]]||'plot';if(process.env.DEBUG)console.error(kind,raw.length,raw.subarray(0,120).toString('latin1').replace(/[^ -~]/g,'.'));frames.push({kind:kind==='model'&&!raw.includes(Buffer.from('box3'))?'plot':kind,bytes:raw.length,raw,at});return}
    let x=null;try{x=decode(data)}catch{return}if(!x)return;if(x.t==='reply')replies.set(x.id,x);if(x.t==='plot'&&x.none)frames.push({kind:'none',why:x.none,at})};
  await new Promise((r,j)=>{ws.onopen=r;ws.onerror=()=>j(new Error('ws'))});
  let id=0;const rpc=async(m,p)=>{const i=++id;ws.send(encode({id:i,m,p}));for(let k=0;k<60000&&!replies.has(i);k++)await sleep(5);return replies.get(i)};
  const open=async(name,run)=>{const k=frames.length;const t0=performance.now();await rpc('kicad.watch',{kind:'3d',root:`${proj}/${name.split('/mech/')[0]}`,path:`${proj}/${name}`,tree:'',sheet:'',stamps:''});
    for(let i=0;i<30000&&!frames.slice(k).some(f=>f.kind==='model'||f.kind==='none');i++)await sleep(5);
    const fs=frames.slice(k);const t=kind=>{const f=fs.find(f=>f.kind===kind);return f?Math.round(f.at-t0):null};
    const row={file:name.split('/').pop(),run,parts:t('parts'),first_part:t('part'),part_frames:fs.filter(f=>f.kind==='part').length,model:t('model'),model_bytes:fs.find(f=>f.kind==='model')?.bytes??0,none:fs.find(f=>f.kind==='none')?.why};
    rows.push(row);console.log(JSON.stringify(row));
    const m=fs.find(f=>f.kind==='model');if(m&&run==='cold'){mkdirSync(out+'.frames',{recursive:true});writeFileSync(`${out}.frames/${name.split('/').pop()}.model.cbor`,m.raw)}
    await rpc('kicad.watch',{kind:'',root:proj,path:'',tree:'',sheet:'',stamps:''});await sleep(200)};
  return {h,ws,open}}
// cold, then again on the same hub
let a=await hub(1);for(const n of names){await open1(a,n)}
async function open1(x,n){await x.open(n,'cold');await x.open(n,'reopen')}
a.ws.close();stopHub(a.h);await sleep(1500);
// a new hub on the same models cache
let b=await hub(2);for(const n of names)await b.open(n,'restart');b.ws.close();stopHub(b.h);await sleep(500);
writeFileSync(out,JSON.stringify(rows,null,1));
if(!process.env.KEEP)rmSync(R,{recursive:true,force:true});
