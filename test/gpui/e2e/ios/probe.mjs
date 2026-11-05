// What the hub replies to the requests the iOS journeys depend on (run on the hub machine next to the fixture): files.dir, thread read.
// usage: bun probe.mjs ROOT PORT   (ROOT: the fixture's root, PORT: the hub's own port, 4850 for fixture port 3850)
import {readFileSync} from 'node:fs';
import {client} from '../../perf/lib.mjs';
const [root,port]=[process.argv[2],Number(process.argv[3])];
const token=readFileSync(root+'/home/token','utf8').trim();
const c=await client({host:'127.0.0.1',port,token});
const show=(m,r)=>console.log(`probe ${m}:`,r?JSON.stringify(r.o).slice(0,500):'NO REPLY in 8 s');
const state=JSON.parse(readFileSync(root+'/fixture.json','utf8'));
let thread=null;await new Promise(r=>setTimeout(r,1500));
for(const it of c.logItems){if(it.$==='ThreadCreated'||it.t==='ThreadCreated'){thread=it.id;break}}
console.log('probe thread:',thread);
for(const p of ['','.'])show('files.dir path='+JSON.stringify(p),await c.rpc('files.dir',{thread,path:p},8000));
show('boards.list',await c.rpc('boards.list',{thread},8000));
// the same on the phones' view lane (view=1&mobile=1): the reply, and which frames arrive around it
const seen=[];const v=await client({host:'127.0.0.1',port,token},{view:true,onMsg:(o)=>{seen.push(o.t+(o.id!==undefined?'#'+o.id:''))}});
await new Promise(r=>setTimeout(r,2500));seen.length=0;
show('view-lane files.dir',await v.rpc('files.dir',{thread,path:''},8000));
await new Promise(r=>setTimeout(r,1500));
console.log('probe view-lane frames after files.dir:',seen.slice(0,12).join(' '));
for(const m of ['bots.list','rooms.list','thread.read']){const r=await v.rpc(m,{thread},4000);show('view-lane '+m,r)}
process.exit(0);
