// What the hub replies to the requests the iOS journeys depend on (run on the hub machine next to the fixture): files.dir, thread read.
// usage: bun probe.mjs ROOT PORT   (any hub: PROBE_TOKEN_FILE=<its token file> PROBE_HUB=<the hub key the app would use, host:port>; ROOT is then unused)
// older line: bun probe.mjs ROOT PORT   (ROOT: the fixture's root, PORT: the hub's own port, 4850 for fixture port 3850)
import {readFileSync} from 'node:fs';
import {client} from '../../perf/lib.mjs';
const [root,port]=[process.argv[2],Number(process.argv[3])];
const token=readFileSync(process.env.PROBE_TOKEN_FILE||root+'/home/token','utf8').trim();
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
// what the phone's discovery asks first: /client/capabilities with the hub token (the app accepts only version 1, views.version 1, websocketQuery "view=1", renderer "mobile", compatibility "explicit-only")
{
  const r=await fetch(`http://127.0.0.1:${port}/client/capabilities`,{headers:{Authorization:'Bearer '+token}});
  const t=await r.text();console.log('probe capabilities:',r.status,t.slice(0,400));
}
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
// the first view as the app asks for it, run through the bridge's own validators (the app says "invalid view presentation" when they refuse it)
try {
  const hubKey=process.env.PROBE_HUB||'100.96.192.111:3850';
  let greeting=null;const v=await client({host:'127.0.0.1',port,token},{view:true,onMsg:(o)=>{if(o.t==='view'&&!greeting)greeting=o}});await new Promise(r=>setTimeout(r,1500));
  console.log('probe greeting:',greeting?JSON.stringify({renderer:greeting.renderer,version:greeting.version,generation:greeting.generation,epoch:String(greeting.epoch).slice(0,12),limits:Object.keys(greeting.limits||{}).length}):'NONE');
  const session={rid:2**47+7,sel:'',drafts:{},open:{},adding:false,panel:false,wantNew:false,error:'',note:'',info:{},outbox:{},route:{tab:'threads'}};
  v.id=2**47+10; // the app's view request ids start at VIEW_ID_BASE (src/web/thin.js)
  const r=await v.rpc('view.open',{session,cid:'probe-cid',version:1,hub:hubKey,generation:greeting?.generation,epoch:greeting?.epoch,viewControl:1,windowProjection:0,requestRevision:1,target:{kind:'recent',id:''},viewId:''},8000);
  const res=r?.o?.result;
  console.log('probe view.open:',r?(r.o.ok?'ok':'error '+JSON.stringify(r.o).slice(0,200)):'NO REPLY', res?('keys '+Object.keys(res).join(',')):'');
  if(res){
    const input={hub:hubKey,target:{kind:'recent',id:''},session};
    const m=await import('../../../../src/mobile/thin-native-host-307265.js');
    console.log('probe validateNativeResult307265:',m.validateNativeResult307265(res,input),' validateNativeSidebar307265:',m.validateNativeSidebar307265(res,input,true));
    const sh=res.shown||{},ov=res.overlay||{};
    const t=x=>x===undefined?'undefined':x===null?'null':Array.isArray(x)?'array'+x.length:typeof x;
    console.log('probe fields: remoteReady',res.remoteReady,'| shown.hub',sh.hub,'| overlay.key',ov.key,'| shown.online',t(sh.online),'| version/error/note/empty',['version','error','note','empty'].map(k=>t(sh[k])).join('/'),'| hubs/found/projects/bots/rooms',['hubs','found','projects','bots','rooms'].map(k=>t(sh[k])).join('/'),'| active/legacy',t(sh.active),t(sh.legacy),'| nativeSidebar',sh.nativeSidebar,ov.j?.nativeSidebar,'| window',res.window?Object.keys(res.window).join(','):t(res.window),'| session.rid',typeof res.session?.rid,'| nextTarget',t(res.nextTarget),'| cmds',t(res.cmds));
    console.log('probe sidebarPages:',JSON.stringify(sh.sidebarPages)?.slice(0,300));
  }
} catch(e){console.log('probe view validation error:',String(e).slice(0,300))}
process.exit(0);
