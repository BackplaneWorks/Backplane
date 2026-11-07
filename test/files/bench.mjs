const BP_ROOT=new URL('../..',import.meta.url).pathname.replace(/\/$/,'');
// bun bench.mjs RUST_BIN JS_BIN PLOT_BIN OUT.json : latency and memory of files.dir, /img and the first plot on both hubs (ports 3858/3859)
import {mkdirSync,rmSync,writeFileSync,cpSync,readFileSync} from 'node:fs';
import {makeStandins,startHub,stopHub,client,rssKb,hwmKb,sleep} from '../perf/lib.mjs';
const [rustBin,jsBin,plotBin,out]=process.argv.slice(2);
const R=`${BP_ROOT}/build/gpui/files-run/bench`;rmSync(R,{recursive:true,force:true});mkdirSync(R,{recursive:true});makeStandins(R+'/bin');
const proj=R+'/ecc83';cpSync('/home/h/.backplane-bend/kicad/10.99.0-bp.4/share/kicad/demos/ecc83',proj,{recursive:true});
const log=[{$:'ProjectCreated',id:'p1',title:'e',root:proj,at:1},{$:'ThreadCreated',id:'t1',project:'p1',title:'T',env:'local',branch:'',worktree:'',provider:'claude',model:'',at:2}].map(x=>JSON.stringify(x)).join('\n')+'\n';
const q=(xs,p)=>{const s=[...xs].sort((a,b)=>a-b);return s[Math.min(s.length-1,Math.floor(p*s.length))]};
const res={};
for(const [name,bin,port] of [['rust',rustBin,3858],['js',jsBin,3859]]){
 const home=`${R}/${name}-home`;mkdirSync(home,{recursive:true});writeFileSync(home+'/events.jsonl',log);
 const t0=performance.now();const h=await startHub({bin,home,port,standins:R+'/bin',extraEnv:{BACKPLANE_PLOT_BIN:plotBin}});const startMs=performance.now()-t0;
 await sleep(500);const rss0=rssKb(h.pid);const c=await client(h,{view:true});
 const dir=[],img=[];
 for(let i=0;i<50;i++){const t=performance.now();await c.rpc('files.dir',{thread:'t1',path:i%2?'':'3d_shapes'},5000);dir.push(performance.now()-t)}
 const url=`${h.url}/img?path=${encodeURIComponent(proj+'/ecc83-pp.kicad_pro')}`; // a text-less path is refused: use a real png below
 cpSync(proj+'/ecc83-pp.kicad_pro',proj+'/n.md');
 for(let i=0;i<50;i++){const t=performance.now();const r=await fetch(`${h.url}/img?path=${encodeURIComponent(proj+'/n.md')}`,{headers:{authorization:'Bearer '+h.token}});await r.arrayBuffer();img.push(performance.now()-t)}
 let first=NaN;{const t=performance.now();c.send('kicad.watch',{kind:'board',root:proj,path:'',tree:'',sheet:'',stamps:''});for(let k=0;k<1500;k++){if(c.seen.length||c.otherItems?.length)break;await sleep(10)}first=performance.now()-t}
 await sleep(1500);
 res[name]={startMs:Math.round(startMs),rssIdleKb:rss0,rssAfterKb:rssKb(h.pid),hwmKb:hwmKb(h.pid),dirMedianMs:q(dir,.5),dirP95Ms:q(dir,.95),imgMedianMs:q(img,.5),imgP95Ms:q(img,.95),n:50};
 c.ws.close();stopHub(h)}
writeFileSync(out,JSON.stringify(res,null,1));console.log(JSON.stringify(res));process.exit(0);
