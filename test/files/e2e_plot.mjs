const BP_ROOT=new URL('../..',import.meta.url).pathname.replace(/\/$/,'');
// bun e2e_plot.mjs RUST_HUB_BIN JS_HUB_BIN PLOT_BIN OUT.json   (env PLOT_DEMO: a demo folder, default ecc83; PLOT_PORTS: two ports, default 3860,3861)
// PLOT_BIN goes to the Rust hub only: a path that does not exist proves it spawns no plot child.
// Both hubs (stand-in agents) watch the same KiCad demo project; the first plot frame, a delta after a
// real edit, a no-change touch (must send nothing), kicad.info, a missing file and a hostile root are compared byte for byte.
import {mkdirSync,rmSync,writeFileSync,readFileSync,cpSync,utimesSync,appendFileSync} from 'node:fs';
import {makeStandins,startHub,stopHub,sleep} from '../perf/lib.mjs';
import {decode,encode} from '../wire.mjs';
const [rustBin,jsBin,plotBin,out]=process.argv.slice(2);
const R=`${BP_ROOT}/build/gpui/files-run/plot`;rmSync(R,{recursive:true,force:true});mkdirSync(R,{recursive:true});makeStandins(R+'/bin');
const demoName=process.env.PLOT_DEMO||'ecc83';const [portA,portB]=(process.env.PLOT_PORTS||'3860,3861').split(',').map(Number);
const demo='/home/h/.backplane-bend/kicad/10.99.0-bp.4/share/kicad/demos/'+demoName;
const proj=R+'/'+demoName;cpSync(demo,proj,{recursive:true});
const log=[{$:'ProjectCreated',id:'p1',title:'ecc',root:proj,at:1},{$:'ThreadCreated',id:'t1',project:'p1',title:'T',env:'local',branch:'',worktree:'',provider:'claude',model:'',at:2}].map(x=>JSON.stringify(x)).join('\n')+'\n';
const names=(await import('node:fs')).readdirSync(proj);const pcb=names.find(n=>n.endsWith('.kicad_pcb')),sch=names.find(n=>n.endsWith('.kicad_sch'));
const results={};
for(const [name,bin,port] of [['rust',rustBin,portA],['js',jsBin,portB]]){
 const home=`${R}/${name}-home`;mkdirSync(home,{recursive:true});writeFileSync(home+'/events.jsonl',log);
 // each hub reads the same files, restored before its run
 cpSync(demo,proj,{recursive:true});
 const h=await startHub({bin,home,port,standins:R+'/bin',extraEnv:name==='rust'?{BACKPLANE_PLOT_BIN:plotBin}:{}});
 const frames=[];const sheets=[];let greet;const replies=new Map();
 const ws=new WebSocket(`ws://127.0.0.1:${port}/ws?view=1&mobile=1&enc=cbor&token=${h.token}`);ws.binaryType='arraybuffer';
 ws.onmessage=({data})=>{let raw0=Buffer.from(data);let prefix=raw0.subarray(0,9).toString('hex').startsWith('a80164706c6f74')||raw0.subarray(0,9).toString('hex').startsWith('a50164706c6f74');if(prefix){const j=raw0.indexOf(Buffer.from([0x18,0x1f]));const kl=raw0[j+2]===0x78?raw0[j+3]:raw0[j+2]&31;const ks=raw0[j+2]===0x78?j+4:j+3;frames.push({at:performance.now(),x:{t:'plot',key:raw0.subarray(ks,ks+kl).toString()},raw:raw0});return}
 if(process.env.DBG)console.error(name,'frame',data.byteLength,Buffer.from(data).subarray(0,20).toString('hex'));let x=null;try{x=decode(data)}catch{ const b=Buffer.from(data); const i=b.indexOf(Buffer.from([0x01,0x64,0x70,0x6c,0x6f,0x74,0x18,0x1f])); if(i>=0&&i<4){const o=i+8;const len=b[o]&31;const key=b.subarray(o+1,o+1+len).toString();const cs=null;x={t:'plot',key:b[o]<0x78?key:'',raw:true}}};if(!x)return;if(x.t==='view')greet=x;if(x.t==='sheets'){const {gen,...rest}=x;sheets.push(rest)}if(x.t==='reply')replies.set(x.id,x);if(x.t==='plot'||x.t==='plotrevision'||x.t==='plotnote')frames.push({at:performance.now(),x,raw:Buffer.from(data)})};
 await new Promise((r,j)=>{ws.onopen=r;ws.onerror=()=>j(new Error('ws'))});
 let id=0;const rpc=async(m,p)=>{const i=++id;ws.send(encode({id:i,m,p}));for(let k=0;k<2000&&!replies.has(i);k++)await sleep(5);return replies.get(i)};
 const waitFrames=async(n,ms=15000)=>{for(let k=0;k<ms/20&&frames.length<n;k++)await sleep(20)};
 const r={};const t0=performance.now();
 r.watch=await rpc('kicad.watch',{kind:'board',root:proj,path:'',tree:'',sheet:'',stamps:''});
 await waitFrames(1);r.first=frames.filter(f=>f.x.t==='plot').map(f=>({ms:Math.round(f.at-t0),bytes:f.raw.length,hex:f.raw.subarray(0,48).toString('hex'),sha:(await_sha(f.raw))}));
 function await_sha(b){return require('node:crypto').createHash('sha256').update(b).digest('hex')}
 const nBefore=frames.length;
 // a no-change touch: same bytes, new mtime
 const pf=proj+'/'+pcb;const orig=readFileSync(pf);utimesSync(pf,new Date(),new Date(Date.now()+5000));await sleep(1500);r.touchFrames=frames.length-nBefore;
 // a real edit: nudge one number
 const text=orig.toString('latin1').replace(/\(thickness 1\.6\)/,'(thickness 1.6)').replace(/\(at (\d+)/,(m,a)=>`(at ${Number(a)+1}`);writeFileSync(pf,Buffer.from(text,'latin1'));
 const n1=frames.length;await waitFrames(n1+1,8000);r.edit=frames.slice(n1).filter(f=>f.x.t==='plot').map(f=>({bytes:f.raw.length,sha:await_sha(f.raw),cs:f.x.cs?.length,fresh:f.x.cs?.filter(c=>typeof c==='object').length}));
 // kicad.info for chunk 0 idx 0
 const key=frames.find(f=>f.x.t==='plot')?.x.key??'';r.key=key;const info=await rpc('kicad.info',{key,at:'0',idx:'0'});r.info=info?.result?.info?.slice(0,80)??info;r.infos=[];for(const at of ['0','1','2','3','5','8','999'])for(const idx of ['0','1','2','7']){const q=await rpc('kicad.info',{key,at,idx});r.infos.push(q?.result?.info??q?.error??null)}
 // schematic
 const n2=frames.length;r.watchSch=(await rpc('kicad.watch',{kind:'schematic',root:proj,path:'',tree:'',sheet:'',stamps:''}))?.ok;await waitFrames(n2+1);r.sch=frames.slice(n2).filter(f=>f.x.t==='plot').map(f=>({bytes:f.raw.length,sha:await_sha(f.raw),key:f.x.key,fam:f.x.fam,base:f.x.base}));
 // hostile root
 for(let k=0;k<200&&!sheets.length;k++)await sleep(10);r.sheets=sheets;r.hostile=await rpc('kicad.watch',{kind:'board',root:'/etc',path:'',tree:'',sheet:'',stamps:''});
 // stop
 r.stop=(await rpc('kicad.watch',{kind:'',root:proj,path:'',tree:'',sheet:'',stamps:''}))?.ok;
 {const st=require('node:fs').readFileSync(`/proc/${h.pid}/status`,'utf8');r.mem={rss:/VmRSS:\s+(\d+)/.exec(st)?.[1],hwm:/VmHWM:\s+(\d+)/.exec(st)?.[1]}}
 r.revs=frames.filter(f=>f.x.t==='plotrevision').length;r.order=frames.map(f=>f.x.t).join(',');ws.close();stopHub(h);results[name]=r;
}
const same=JSON.stringify(results.rust)===JSON.stringify(results.js);
const diffs=[];for(const k of Object.keys(results.rust)){const a=JSON.stringify(results.rust[k],(kk,v)=>kk==='ms'||kk==='mem'?undefined:v),b=JSON.stringify(results.js[k],(kk,v)=>kk==='ms'||kk==='mem'?undefined:v);if(a!==b)diffs.push({k,rust:results.rust[k],js:results.js[k]})}
writeFileSync(out,JSON.stringify({results,diffs},null,1));
console.log('first plot ms rust/js',results.rust.first?.[0]?.ms,results.js.first?.[0]?.ms,'bytes',results.rust.first?.[0]?.bytes,results.js.first?.[0]?.bytes);
console.log('hub kB rss/hwm rust',results.rust.mem,'js',results.js.mem);console.log('differences',diffs.length);for(const d of diffs)console.log(JSON.stringify(d).slice(0,600));
process.exit(0);
