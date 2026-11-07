const BP_ROOT=new URL('../..',import.meta.url).pathname.replace(/\/$/,'');
// bun e2e_plot_big.mjs RUST_HUB_BIN OUT.json: the Rust hub alone (its port 3866, stand-in agents, no plot child: BACKPLANE_PLOT_BIN names nothing) reading the 85 MB demo board:
// time to the first plot frame after kicad.watch, frame size, hub RSS and peak before, right after and 3 s after (malloc_trim). The JS hub's child cannot read this board under the 3.7 GiB cap.
import {mkdirSync,rmSync,writeFileSync,copyFileSync,readFileSync} from 'node:fs';
import {makeStandins,startHub,stopHub,sleep} from '../perf/lib.mjs';
import {encode,decode} from '../wire.mjs';
const [bin,out]=process.argv.slice(2);
const R=`${BP_ROOT}/build/gpui/files-run/plotbig`;rmSync(R,{recursive:true,force:true});mkdirSync(R+'/proj',{recursive:true});makeStandins(R+'/bin');
const name='jetson-agx-thor-baseboard';copyFileSync(`/home/h/.backplane-bend/kicad/10.99.0-bp.4/share/kicad/demos/${name}/${name}.kicad_pcb`,`${R}/proj/${name}.kicad_pcb`);writeFileSync(`${R}/proj/${name}.kicad_pro`,'{}');
const home=R+'/home';mkdirSync(home);writeFileSync(home+'/events.jsonl',[{$:'ProjectCreated',id:'p1',title:'b',root:R+'/proj',at:1},{$:'ThreadCreated',id:'t1',project:'p1',title:'T',env:'local',branch:'',worktree:'',provider:'claude',model:'',at:2}].map(x=>JSON.stringify(x)).join('\n')+'\n');
const h=await startHub({bin,home,port:3866,standins:R+'/bin',extraEnv:{BACKPLANE_PLOT_BIN:'/nonexistent/backplane-serve'}});
const mem=()=>{const s=readFileSync(`/proc/${h.pid}/status`,'utf8');return {rss_kb:+/VmRSS:\s+(\d+)/.exec(s)[1],hwm_kb:+/VmHWM:\s+(\d+)/.exec(s)[1]}};
const ws=new WebSocket(`ws://127.0.0.1:3866/ws?view=1&mobile=1&enc=cbor&token=${h.token}`);ws.binaryType='arraybuffer';let got=null,t0=0;
ws.onmessage=({data})=>{const b=Buffer.from(data);if(b.subarray(0,7).toString('hex')==='a80164706c6f74'&&!got)got={ms:Math.round(performance.now()-t0),bytes:b.length}};
await new Promise(r=>ws.onopen=r);await sleep(500);const r={idle:mem()};t0=performance.now();ws.send(encode({id:1,m:'kicad.watch',p:{kind:'board',root:R+'/proj',path:'',tree:'',sheet:'',stamps:''}}));
for(let i=0;i<3000&&!got;i++)await sleep(10);r.first=got;r.after=mem();await sleep(3000);r.later=mem();ws.close();await sleep(300);stopHub(h);writeFileSync(out,JSON.stringify(r,null,1));console.log(JSON.stringify(r));await sleep(500);rmSync(R,{recursive:true,force:true});process.exit(0);
