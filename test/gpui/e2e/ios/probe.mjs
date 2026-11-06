// What the hub replies to the requests the iOS journeys depend on (run on the hub machine next to the fixture): files.dir, thread read.
// usage: bun probe.mjs ROOT PORT   (ROOT: the fixture's root, PORT: the hub's own port, 4850 for fixture port 3850)
import {readFileSync} from 'node:fs';
import {client} from '../../perf/lib.mjs';
const [root,port]=[process.argv[2],Number(process.argv[3])];
const token=readFileSync(root+'/home/token','utf8').trim();
const c=await client({host:'127.0.0.1',port,token});
const show=(m,r)=>console.log(`probe ${m}:`,r?JSON.stringify(r.o).slice(0,500):'NO REPLY in 8 s');
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
// pairing as the phone does it: an invitation, then /auth/enroll with the invite as the bearer (status and keys only: no secret is printed)
{
  const base=`http://127.0.0.1:${port}`;
  const inv=await fetch(base+'/auth/invitations',{method:'POST',headers:{Authorization:'Bearer '+token,'Content-Type':'application/json'},body:'{}'});
  const ij=await inv.json().catch(()=>({}));
  console.log('probe invitations:',inv.status,Object.keys(ij).join(','),'routes',JSON.stringify(ij.routes),'error',ij.error??'');
  const secret=(ij.link??'').match(/invite=([a-f0-9]{32})/)?.[1];
  if(secret){
    const en=await fetch(base+'/auth/enroll',{method:'POST',headers:{Authorization:'Bearer '+secret,'X-Backplane-Client':'native','X-Backplane-Name':'probe'}});
    const ej=await en.json().catch(()=>({}));
    console.log('probe enroll:',en.status,Object.keys(ej).join(','),'tokenShape',/^[a-f0-9]{32}$/.test(ej.token??''),'error',ej.error??'');
    // the device credential works on the view lane
    if(ej.token){const d=await client({host:'127.0.0.1',port,token:ej.token},{view:true});await new Promise(r=>setTimeout(r,1500));console.log('probe device credential: view lane connected, frames',d.msgs);
      show('device credential files.dir',await d.rpc('files.dir',{thread,path:''},6000));
      show('device credential boards.list',await d.rpc('boards.list',{thread},6000));
      const seen2=[];const d2=await client({host:'127.0.0.1',port,token:ej.token},{view:false,onMsg:(o)=>seen2.push(o.t)});await new Promise(r=>setTimeout(r,2500));
      console.log('probe device credential, plain lane: frames',seen2.slice(0,8).join(' '));}
  } else console.log('probe: no invite secret in the link');
}
process.exit(0);
