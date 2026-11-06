// bun e2e_solid.mjs RUST_HUB_BIN JS_HUB_BIN OUT.json RUNTIME FIXTURES [STEP2GLB]    (env SOLID_DEMO: a demo folder, default ecc83; SOLID_PORTS: two ports, default 3890,3891)
// Both hubs (stand-in agents, temp HOME and cache) on the same project: a 3D watch of the board with parts and bare, a GLB of the project's own, a model too big, a board KiCad cannot
// read, mech.render of a GLB with the same refusals as before, and a STEP when STEP2GLB names the tool both hubs then mesh with. Frames and PNG files compared by sha256.
// The Rust hub's BACKPLANE_PLOT_BIN names nothing, which proves it spawns no child. Without STEP2GLB it has BACKPLANE_STEP2GLB=off, and then (env SOLID_FREECAD=1) it also meshes a STEP through
// FreeCAD alone, a run the JS hub has no twin of: `rust.freecad`. Its whole process tree is sampled every 10 ms; a Bun, or step2glb process in it is reported, and it is reported when sampling saw none of what it must see.
// SOLID_CHILD=1 is the fallback: BACKPLANE_PLOT_NATIVE=0 and the JS hub's binary as BACKPLANE_PLOT_BIN (the tree then holds it, as it should).
// SOLID_SETTLE: how long a watch's frames are collected after its model came (ms; the JS hub's child takes over a second on a board like cm5_minima, so use 3000 there).
// Compared: every plot and plotnote frame in order. plotrevision only by count (the JS hub repeats one per read: build/gpui/results/files/README.md). A GLB by bytes less KiCad's `generated_at`.
import {mkdirSync,rmSync,writeFileSync,readFileSync,cpSync,readdirSync,existsSync,symlinkSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {execSync} from 'node:child_process';
import {makeStandins,startHub,stopHub,sleep} from '../perf/lib.mjs';
import {decode,encode} from '../wire.mjs';
const [rustBin,jsBin,out,runtime,fixtures,step2glb]=process.argv.slice(2);
const sha=b=>createHash('sha256').update(b).digest('hex');
const R='/home/h/Documents/i2cjak/backplane-gpui/build/gpui/files-run/solid';rmSync(R,{recursive:true,force:true});mkdirSync(R,{recursive:true});makeStandins(R+'/bin');
const demoName=process.env.SOLID_DEMO||'ecc83';const [portA,portB]=(process.env.SOLID_PORTS||'3890,3891').split(',').map(Number);
const demo=`${runtime}/share/kicad/demos/${demoName}`;const proj=R+'/'+demoName;
const setup=()=>{rmSync(proj,{recursive:true,force:true});cpSync(demo,proj,{recursive:true});execSync(`chmod -R u+w ${JSON.stringify(proj)}`);mkdirSync(proj+'/mech',{recursive:true});
 for(const f of ['cube-u16','glass','sphere'])cpSync(`${fixtures}/${f}.glb`,`${proj}/mech/${f}.glb`);
 writeFileSync(proj+'/mech/huge.glb',Buffer.alloc(12582913));writeFileSync(proj+'/mech/broken.kicad_pcb','(kicad_pcb (version');writeFileSync(proj+'/mech/empty.glb','glTF');
 cpSync('/home/h/Documents/i2cjak/backplane-gpui/test/files/fixtures/mesh/bracket.step',proj+'/mech/bracket.step');
 writeFileSync(R+'/outside.glb',readFileSync(`${fixtures}/tri.glb`));symlinkSync(R+'/outside.glb',proj+'/mech/link.glb')};
setup();
const log=[{$:'ProjectCreated',id:'p1',title:'p',root:proj,at:1},{$:'ThreadCreated',id:'t1',project:'p1',title:'T',env:'local',branch:'',worktree:'',provider:'claude',model:'',at:2}].map(x=>JSON.stringify(x)).join('\n')+'\n';
const results={};let freecad=null;
const pcb=readdirSync(demo).find(n=>n.endsWith('.kicad_pcb'));
// every process under the hub: its name, for the "no Bun" check
const tree=pid=>{try{const ps=execSync('ps -e -o pid=,ppid=,comm=',{encoding:'utf8',stdio:['ignore','pipe','ignore']}).split('\n').map(l=>l.trim().match(/^(\d+)\s+(\d+)\s+(.*)$/)).filter(Boolean);const under=new Set([String(pid)]);const o=[];for(let more=true;more;){more=false;for(const [,p,pp,c] of ps)if(under.has(pp)&&!under.has(p)){under.add(p);o.push(c);more=true}}return o}catch{return[]}};
for(const [name,bin,port] of [['rust',rustBin,portA],['js',jsBin,portB]]){
 const home=`${R}/${name}-home`;const uh=`${R}/${name}-user`;mkdirSync(home,{recursive:true});mkdirSync(uh,{recursive:true});writeFileSync(home+'/events.jsonl',log);
 setup();
 const paths=step2glb?{PATH:`${step2glb}:${R}/bin:${process.env.PATH}`}:{};
 const extraEnv={HOME:uh,XDG_CACHE_HOME:uh+'/.cache',XDG_CONFIG_HOME:uh+'/.config',XDG_DATA_HOME:uh+'/.local/share',KICAD_CONFIG_HOME:uh+'/kicad',BACKPLANE_KICAD_CLI:`${runtime}/bin/kicad-cli`,...paths,
  ...(name==='rust'?{BACKPLANE_PLOT_BIN:process.env.SOLID_CHILD?jsBin:'/nonexistent/backplane-serve',...(process.env.SOLID_CHILD?{BACKPLANE_PLOT_NATIVE:'0'}:{}),...(step2glb?{BACKPLANE_STEP2GLB:step2glb+'/backplane-step2glb'}:{BACKPLANE_STEP2GLB:'off'})}:{})};
 const h=await startHub({bin,home,port,standins:R+'/bin',extraEnv});
 const frames=[];const replies=new Map();const seen=new Set();let watching=true;
 (async()=>{while(watching){for(const c of tree(h.pid))seen.add(c);await sleep(10)}})();
 const ws=new WebSocket(`ws://127.0.0.1:${port}/ws?view=1&mobile=1&enc=cbor&token=${h.token}`);ws.binaryType='arraybuffer';
 ws.onmessage=({data})=>{const raw=Buffer.from(data);const head=raw.subarray(0,9).toString('hex');
  if(head.startsWith('a50164706c6f74')||head.startsWith('a80164706c6f74')){const j=raw.indexOf(Buffer.from([0x18,0x1f]));const l=raw[j+2];const kl=l===0x78?raw[j+3]:l===0x79?raw.readUInt16BE(j+3):l&31;const ks=l===0x78?j+4:l===0x79?j+5:j+3;frames.push({t:'plot',key:raw.subarray(ks,ks+kl).toString(),raw,at:performance.now()});return}
  let x=null;try{x=decode(data)}catch{return}if(!x)return;if(x.t==='reply')replies.set(x.id,x);if(['plot','plotnote','plotrevision'].includes(x.t))frames.push({t:x.t,key:x.key,x,raw,at:performance.now()})};
 await new Promise((r,j)=>{ws.onopen=r;ws.onerror=()=>j(new Error('ws'))});
 let id=0;const rpc=async(m,p,ms=240000)=>{const i=++id;ws.send(encode({id:i,m,p}));for(let k=0;k<ms/5&&!replies.has(i);k++)await sleep(5);return replies.get(i)};
 const until=async(f,ms=240000)=>{for(let k=0;k<ms/20&&!f();k++)await sleep(20)};
 const strip=k=>String(k).replaceAll(R,'');
 // what a watch produced: each key's frames in order, by what they are; a revision's words name inodes and times, so it counts only
 const view=fs=>fs.filter(f=>f.t!=='plotrevision').map(f=>f.t==='plotnote'?{t:f.t,key:strip(f.key),text:f.x.text}:{t:f.t,key:strip(f.key),none:f.x?.none?strip(f.x.none):undefined,bytes:f.raw.length,sha:f.x?.none?undefined:sha(Buffer.from(f.raw.toString('latin1').replaceAll(R,''),'latin1'))});
 const watch=async(kind,path,want)=>{const n=frames.length;const t0=performance.now();const reply=await rpc('kicad.watch',{kind,root:proj,path,tree:'',sheet:'',stamps:''});await until(()=>want(frames.slice(n)));await sleep(Number(process.env.SOLID_SETTLE||500));
  const fs=frames.slice(n);return {ok:reply?.ok,error:reply?.error,revs:fs.filter(f=>f.t==='plotrevision').length,ms:Math.round((fs.find(f=>f.t==='plot'&&f.key.startsWith(kind==='3d'?'2|':kind==='3d-bare'?'3|':'4|'))?.at??t0)-t0),frames:view(fs)}};
 const solid=k=>fs=>fs.some(f=>f.t==='plot'&&f.key.startsWith(k+'|'));
 const r={};
 r.board=await watch('3d','',solid(2));
 r.again=await watch('3d','',solid(2)); // a view that starts on a model already made
 r.bare=await watch('3d-bare','',solid(3));
 r.full=await watch('3d-full','',solid(4));
 for(const f of ['cube-u16','glass','sphere','empty'])r['glb_'+f]=await watch('3d',`${proj}/mech/${f}.glb`,solid(2));
 r.huge=await watch('3d',`${proj}/mech/huge.glb`,solid(2));
 r.broken=await watch('3d',`${proj}/mech/broken.kicad_pcb`,solid(2));
 if(step2glb)r.step=await watch('3d',`${proj}/mech/bracket.step`,solid(2));
 await rpc('kicad.watch',{kind:'',root:proj,path:'',tree:'',sheet:'',stamps:''});
 // the GLBs each hub exported: identical input for the frames above?
 const models=uh+'/.cache/backplane-bend/models';r.glbs=existsSync(models)?readdirSync(models).filter(f=>f.endsWith('.glb')).map(f=>({kind:f.replace(/[.-]\d.*$/,''),bytes:readFileSync(`${models}/${f}`).length})).sort((a,b)=>a.kind.localeCompare(b.kind)||a.bytes-b.bytes):[];
 r.glbSha=existsSync(models)?readdirSync(models).filter(f=>f.endsWith('.glb')).map(f=>sha(Buffer.from(readFileSync(`${models}/${f}`).toString('latin1').replace(/"generated_at":"[^"]*"/,''),'latin1'))).sort():[];
 // mech.render and its refusals
 const render=async(thread,path)=>{const t0=performance.now();const x=await rpc('mech.render',{thread,path});const res=x?.result;return {ok:x?.ok,error:strip(x?.error??''),rerr:strip(res?.error??''),text:strip(res?.text??''),rendered:strip(res?.rendered??''),ms:Math.round(performance.now()-t0),mech:res?.mech?.split('\n').filter(l=>l.endsWith('.png')).map(l=>l.split(' ').slice(2).join(' ')).filter(l=>!l.includes('/link.'))}};// (the JS hub draws the link that leaves the project; the refusal is compared in renderLink)
 const shas=m=>Object.fromEntries(['iso','front','top','right'].map(v=>{const f=`${proj}/mech/renders/${m}.${v}.png`;return [v,existsSync(f)?sha(readFileSync(f)):null]}));
 r.renderCube=await render('t1','mech/cube-u16.glb');r.cubePng=shas('cube-u16');
 r.renderAbs=await render('t1',`${proj}/mech/glass.glb`);r.glassPng=shas('glass');
 r.renderSphere=await render('t1','mech/sphere.glb');r.spherePng=shas('sphere');
 r.renderEmpty=await render('t1','mech/empty.glb');
 r.renderMissing=await render('t1','mech/none.glb');
 r.renderEscape=await render('t1','../outside.glb');
 r.renderLink=await render('t1','mech/link.glb');
 r.renderOutside=await render('t1','/etc/passwd');
 r.renderUnknown=await render('nope','mech/cube-u16.glb');
 if(step2glb){r.renderStep=await render('t1','mech/bracket.step');r.stepPng=shas('bracket')}
 if(!step2glb&&name==='rust'&&process.env.SOLID_FREECAD){const fw=await watch('3d',`${proj}/mech/bracket.step`,solid(2));const fr=await render('t1','mech/bracket.step');freecad={ms:fw.ms,bytes:fw.frames.find(f=>f.t==='plot')?.bytes,renderMs:fr.ms,renderErr:fr.rerr,pngs:shas('bracket')}}
 watching=false;await sleep(60);
 r.tree=[...seen].sort();
 {const st=readFileSync(`/proc/${h.pid}/status`,'utf8');r.mem={rss:/VmRSS:\s+(\d+)/.exec(st)?.[1],hwm:/VmHWM:\s+(\d+)/.exec(st)?.[1]}}
 ws.close();stopHub(h);results[name]=r;
}
const drop=(k,v)=>['ms','mem','tree','revs'].includes(k)?undefined:v;
const diffs=[];for(const k of Object.keys(results.rust)){if(k==='mem'||k==='tree')continue;const a=JSON.stringify(results.rust[k],drop),b=JSON.stringify(results.js[k],drop);if(a!==b)diffs.push({k,rust:results.rust[k],js:results.js[k]})}
const bad=results.rust.tree.filter(c=>/bun|backplane-s|node/i.test(c));
writeFileSync(out,JSON.stringify({results,diffs,freecad,rustTree:results.rust.tree,jsTree:results.js.tree},null,1));
console.log('first 3D frame ms rust/js',results.rust.board.ms,results.js.board.ms,'(export included)','again',results.rust.again.ms,results.js.again.ms);
console.log('render ms rust/js',results.rust.renderCube.ms,results.js.renderCube.ms,'sphere',results.rust.renderSphere.ms,results.js.renderSphere.ms);
console.log('hub kB rss/hwm rust',results.rust.mem,'js',results.js.mem);console.log('rust process tree',results.rust.tree.join(' '),'js',results.js.tree.join(' '));
console.log('Bun or under the Rust hub:',bad.length?bad.join(' '):'none','(sampled',results.rust.tree.length,'names)');if(freecad)console.log('STEP by FreeCAD alone',JSON.stringify(freecad));console.log('differences',diffs.length);for(const d of diffs)console.log(JSON.stringify(d).slice(0,900));
process.exit(0);
