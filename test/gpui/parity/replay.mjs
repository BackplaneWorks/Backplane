// Replay a recording's client side against a hub (fresh fixture home each scenario) and compare with the recording.
//   bun replay.mjs --rec DIR --bin BIN --port 3824 [--only a,b] [--out DIR]     (a fresh hub per scenario)
// Outbound frames are re-sent in recorded order; server-issued values (generation, epoch, view ids, thread/project/message ids,
// hub URL, work paths) are substituted from what the target hub actually said. Waits are by reply id, not time.
import {readFileSync,readdirSync,writeFileSync,mkdirSync} from 'node:fs';
import {Rec,connect,httpProbe,attempt,sleep,PROFILES,privateLeaks} from './lib.mjs';
import {startFixtureHub,WORK} from './hubs.mjs';
import {normalise,diff} from './compare.mjs';
import {encode} from '../wire.mjs';
const arg=(k,d)=>{const i=process.argv.indexOf('--'+k);return i>0?process.argv[i+1]:d};
const idRe=/\b([ptgmkbro]\d{10}-\d+(?::\d+)?)\b|\bfake-\d+\b|\b[mt][a-z]?\d{3,8}-\d+-\d+(?::\d+)?\b|(?<![A-Za-z])tu\d+-\d+-\d+\b|\bc[a-z0-9-]*?\d{10}-\d+\b/g;
const ids=s=>[...String(s).matchAll(idRe)].map(m=>m[0]);
const walkStr=(v,f)=>typeof v==='string'?f(v):Array.isArray(v)?v.map(x=>walkStr(x,f)):v&&typeof v==='object'?Object.fromEntries(Object.entries(v).map(([k,x])=>[k,walkStr(x,f)])):v;

export async function replayOne(recording,{bin,port,work}){
  const {meta,frames}=recording;
  // the recording's server-issued values, in order of first appearance
  const recIds=[],seen=new Set();
  const greet={},viewIdsRec={},tokRec={},tokRep={};
  const volIds=new Set(frames.filter(f=>f.dir==='out'&&f.vol&&f.json&&f.json.id!==undefined).map(f=>f.c+'|'+f.json.id));
  // ids the client made itself (first seen in something it sent) are not the hub's and are never substituted
  for(const f of frames){if(f.dir==='out'&&f.json&&typeof f.json==='object'&&!f.vol)for(const s of ids(JSON.stringify(f.json)))seen.add(s);
    if(f.dir!=='in'||!f.json||typeof f.json!=='object')continue;
    if(f.json.t==='reply'&&volIds.has(f.c+'|'+f.json.id))continue;
    if(f.json.t==='view')greet[f.c]=f.json;
    const vid=f.json.result?.viewId;if(vid&&!(viewIdsRec[f.c]??=[]).includes(vid))viewIdsRec[f.c].push(vid);
    const tk=f.json.result?.viewDelta?.token;if(tk&&!(tokRec[f.c]??=[]).includes(tk))tokRec[f.c].push(tk);
    for(const s of ids(JSON.stringify(f.json)))if(!seen.has(s)){seen.add(s);recIds.push(s)}}
  // a client sends some requests only because a hub reply told it to (cmds of type send): the replay sends the command its own hub issued
  const cmdRec={},outCmd=new Map();
  for(const f of frames){
    if(f.dir==='in'&&f.json?.t==='reply'&&!(f.vol&&volIds.has(f.c+'|'+f.json.id)))for(const cm of f.json.result?.cmds||[])if(cm.type==='send'){try{(cmdRec[f.c]??=[]).push({q:JSON.parse(cm.text),used:false})}catch{}}
    if(f.dir==='out'&&f.kind==='bin'&&!f.vol&&f.json&&!f.json.raw){const k=(cmdRec[f.c]||[]).findIndex(x=>!x.used&&JSON.stringify(x.q)===JSON.stringify(f.json));if(k>=0){cmdRec[f.c][k].used=true;outCmd.set(f.i,k)}}}
  const cmdRep={};
  const h=await startFixtureHub({bin,port,work,deleted:meta.scenario==='unresolved',privateLines:meta.scenario==='private'});
  const rec=new Rec();const clients={};const repIds=[],repSeen=new Set();const viewIdsRep={};
  const observe=(c)=>{for(const j of c.inbox.slice(c.obs||0)){
      if(j?.t==='reply'&&!(j.ok===false))for(const cm of j.result?.cmds||[])if(cm.type==='send'){try{(cmdRep[c.name]??=[]).push(JSON.parse(cm.text))}catch{}}const vid=j?.result?.viewId;if(vid&&!(viewIdsRep[c.name]??=[]).includes(vid))viewIdsRep[c.name].push(vid);
      const tk=j?.result?.viewDelta?.token;if(tk&&!(tokRep[c.name]??=[]).includes(tk))tokRep[c.name].push(tk);
      for(const s of ids(JSON.stringify(j)))if(!repSeen.has(s)){repSeen.add(s);repIds.push(s)}}c.obs=c.inbox.length};
  const sub=(c,v)=>walkStr(v,s=>{
    if(s===meta.hubUrl)return h.url;
    s=s.split(meta.hubUrl).join(h.url).split(meta.work).join(h.work);
    const g=greet[c.name],gg=c.greet;
    if(g&&gg){if(s===g.generation)return gg.generation;if(s===g.epoch)return gg.epoch}
    const vi=(viewIdsRec[c.name]||[]).indexOf(s);if(vi>=0&&viewIdsRep[c.name]?.[vi])return viewIdsRep[c.name][vi];
    // an id the replay hub has already said (the fixture's own) is valid as it is; only ids the recorded hub made up are mapped
    return s.replace(idRe,m=>{if(repSeen.has(m))return m;const k=recIds.indexOf(m);return k>=0&&repIds[k]?repIds[k]:m})});
  // reply ids owed before each non-volatile outbound: replies to earlier non-volatile requests
  // a reply is waited for only if the recorded client had it before sending the next frame
  const replyAt=new Map();for(const f of frames)if(f.dir==='in'&&f.json?.t==='reply'&&!replyAt.has(f.c+'|'+f.json.id))replyAt.set(f.c+'|'+f.json.id,f.i);
  const outId=f=>f.json&&typeof f.json==='object'&&f.json.id;
  const owed=[];
  // a poll run (volatile requests) in the recording waited for something; the replay waits for the same observable: thread state, entry count, thread count
  const sigOf=x=>{const th=x?.result?.shown?.thread;return {state:th?.state??null,entries:(th?.entries||[]).length,threads:(x?.result?.shown?.projects||[]).reduce((a,p)=>a+(p.threads?.length||0),0)}};
  const replyOf=new Map();for(const f of frames)if(f.dir==='in'&&f.json?.t==='reply'&&f.json.result?.viewRevision)replyOf.set(f.c+'|'+f.json.id,f.json);
  const done=new Set();
  // the recording's own timing is replayed: a frame goes out no earlier than its recorded offset (the hub's counters and housekeeping depend on elapsed time)
  const first=frames.find(f=>f.dir==='out');const T0=Date.now()-(first?first.ms:0);
  for(let fi=0;fi<frames.length;fi++){
    const f=frames[fi];
    if(f.dir==='out'&&f.vol){
      if(done.has(f.i))continue;
      // the whole run of volatile requests that starts here
      const run=[];for(let k=fi;k<frames.length;k++){const g=frames[k];if(g.dir==='out'&&g.vol&&g.kind==='bin')run.push(g);else if(g.dir==='out'&&!g.vol&&g.kind==='bin')break}
      run.forEach(g=>done.add(g.i));
      const c0=clients[run[0].c];if(!c0)continue;
      const want=sigOf(replyOf.get(run[run.length-1].c+'|'+run[run.length-1].json.id));
      const d=T0+run[0].ms-Date.now();if(d>0)await sleep(d);
      const ok=x=>{const s=sigOf(x);return s.state===want.state&&s.entries>=want.entries&&s.threads>=want.threads};
      rec.vol=true;
      try{
        let last=null,n=0;
        for(const g of run){const cc=clients[g.c];for(const x of Object.values(clients))observe(x);
          const obj=sub(cc,g.json);cc.send(obj);last=obj;n++;
          const rep=await cc.until(x=>x.t==='reply'&&String(x.id)===String(obj.id),20000);if(rep&&ok(rep))break}
        // not there yet: more polls, ids and revisions counting on from the last (the next real request is a thousand further on)
        for(let extra=1;extra<=200&&last;extra++){
          const rep0=await c0.until(x=>x.t==='reply'&&String(x.id)===String(last.id),20000);if(rep0&&ok(rep0))break;
          await sleep(150);last=JSON.parse(JSON.stringify(last));last.id=Number(last.id)+1;last.p.requestRevision=Number(last.p.requestRevision)+1;c0.send(last)}
      }finally{rec.vol=false}
      continue}
    if(f.dir==='out'&&f.vol)continue;
    const c=clients[f.c];
    if(f.dir==='out'){
      if(f.kind!=='settle'){const d=T0+f.ms-Date.now();if(d>0)await sleep(d)}
      if(f.kind==='http-get'){await httpProbe(h,f.json.path,rec,f.c,{auth:f.json.auth});continue}
      if(f.kind==='connect'){
        if(f.json.mode==='good'){let q=f.json.query||'';
          // a resume names the log position and origin it was given: the origin is the target's own
          q=sub({name:f.c,greet:null},q);
          const m=/origin=([^&]*)/.exec(q);if(m){const o=Object.values(clients).map(x=>x.inbox.find(y=>y?.t==='log')?.origin).filter(Boolean).pop();if(o)q=q.replace(/origin=[^&]*/,'origin='+o)}
          clients[f.c]=await connect({hub:h,rec,name:f.c,profile:f.json.profile,query:q});clients[f.c].obs=0}
        else await attempt(h,rec,f.c,f.json.mode);
        continue}
      if(!c)continue;
      if(f.kind==='settle'||f.kind==='check'){continue}
      if(f.kind==='end'){await c.end(f.json.quiet);continue}
      // wait for every reply owed to an earlier non-volatile request of this client
      // every reply, from any client, that the recording had before this frame (one client's view update depends on another's request)
      for(const oid of owed.filter(o=>o.i<f.i&&(replyAt.get(o.c+'|'+o.id)??Infinity)<f.i)){const oc=clients[oid.c];if(oc&&!(await oc.until(x=>x.t==='reply'&&String(x.id)===String(oid.id),20000)))break}
      for(const x of Object.values(clients))observe(x);
      if(f.json.raw){c.sendRaw(Uint8Array.from(Buffer.from(f.raw,'base64')))}
      else{const own=outCmd.has(f.i)?cmdRep[f.c]?.[outCmd.get(f.i)]:undefined;const obj=own?JSON.parse(JSON.stringify(own)):sub(c,f.json);const t0=obj?.p?.viewDelta?.token;if(t0!==undefined){const i=(tokRec[f.c]||[]).indexOf(f.json.p.viewDelta.token);if(i>=0&&tokRep[f.c]?.[i]!==undefined)obj.p.viewDelta.token=tokRep[f.c][i]}for(const s of ids(JSON.stringify(obj)))repSeen.add(s);c.send(obj)}
      if(outId(f)!==undefined&&outId(f)!==null)owed.push({c:f.c,i:f.i,id:outId(f)});
      continue}
  }
  for(const c of Object.values(clients))if(!c.closed)await c.end(500);
  await h.finish();
  return {meta:{replay:true,scenario:meta.scenario,frames:rec.frames.length},frames:rec.frames};
}
if(import.meta.main){
  const dir=arg('rec'),bin=arg('bin'),port=Number(arg('port',3824)),out=arg('out','/tmp/parity-replay');
  const only=(arg('only','')||'').split(',').filter(Boolean);mkdirSync(out,{recursive:true});
  let total=0;let replayed=0;
  if(!readdirSync(dir).some(x=>x.endsWith('.json'))){console.error('replay: no recordings in '+dir+' (an empty replay proves nothing)');process.exit(2)}
  for(const f of readdirSync(dir).filter(x=>x.endsWith('.json')).sort()){
    const name=f.replace('.json','');if(only.length&&!only.includes(name))continue;replayed++;
    const recd=JSON.parse(readFileSync(dir+'/'+f));
    const r=await replayOne(recd,{bin,port,work:`${WORK}/work-R`});
    writeFileSync(`${out}/${f}`,JSON.stringify(r));
    const a=normalise(recd.frames.filter(f=>f.kind!=='check')),b=normalise(r.frames);const d=[];
    for(const c of new Set([...Object.keys(a),...Object.keys(b)]))for(const s of ['out','events','hk','deltas','replies','views','other'])diff(a[c]?.[s],b[c]?.[s],`${c}.${s}`,d);
    console.log(name,d.length?`${d.length} differences`:'identical');for(const x of (process.argv.includes('--all')?d:d.slice(0,12)))console.log('   ',x);total+=d.length;
    {const lk=privateLeaks(r.frames);if(lk.length){console.log(name,'PRIVATE LINES REACHED A CLIENT:',lk.length,JSON.stringify(lk.slice(0,5)));total+=lk.length}}
  }
  if(!replayed){console.error('replay: nothing replayed (--only matched no recording)');process.exit(2)}
  console.log('TOTAL differences:',total);process.exit(total?1:0);
}
