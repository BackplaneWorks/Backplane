// Replay a recording's client side against a hub (fresh fixture home each scenario) and compare with the recording.
//   bun replay.mjs --rec DIR --bin BIN --port 3824 [--only a,b] [--out DIR]     (a fresh hub per scenario)
// Outbound frames are re-sent in recorded order; server-issued values (generation, epoch, view ids, thread/project/message ids,
// hub URL, work paths) are substituted from what the target hub actually said. Waits are by reply id, not time.
import {readFileSync,readdirSync,writeFileSync,mkdirSync} from 'node:fs';
import {Rec,connect,httpProbe,attempt,sleep,PROFILES} from './lib.mjs';
import {startFixtureHub} from './hubs.mjs';
import {normalise,diff} from './compare.mjs';
import {encode} from '../wire.mjs';
const arg=(k,d)=>{const i=process.argv.indexOf('--'+k);return i>0?process.argv[i+1]:d};
const idRe=/\\b([ptgm]\\d{10}-\\d+(?::\\d+)?)\\b|\\bfake-\\d+\\b|\\b[mt][a-z]?\\d{3,8}-\\d+-\\d+(?::\\d+)?\\b|\\btu\\d+-\\d+-\\d+\\b|\\bc[a-z0-9-]*?\\d{10}-\\d+\\b/g;
const ids=s=>[...String(s).matchAll(idRe)].map(m=>m[0]);
const walkStr=(v,f)=>typeof v==='string'?f(v):Array.isArray(v)?v.map(x=>walkStr(x,f)):v&&typeof v==='object'?Object.fromEntries(Object.entries(v).map(([k,x])=>[k,walkStr(x,f)])):v;

export async function replayOne(recording,{bin,port,work}){
  const {meta,frames}=recording;
  // the recording's server-issued values, in order of first appearance
  const recIds=[],seen=new Set();
  const greet={},viewIdsRec={};
  const volIds=new Set(frames.filter(f=>f.dir==='out'&&f.vol&&f.json&&f.json.id!==undefined).map(f=>f.c+'|'+f.json.id));
  // ids the client made itself (first seen in something it sent) are not the hub's and are never substituted
  for(const f of frames){if(f.dir==='out'&&f.json&&typeof f.json==='object'&&!f.vol)for(const s of ids(JSON.stringify(f.json)))seen.add(s);
    if(f.dir!=='in'||(f.vol&&f.json?.t!=='reply')||!f.json||typeof f.json!=='object')continue;
    if(f.json.t==='reply'&&volIds.has(f.c+'|'+f.json.id))continue;
    if(f.json.t==='view')greet[f.c]=f.json;
    const vid=f.json.result?.viewId;if(vid&&!(viewIdsRec[f.c]??=[]).includes(vid))viewIdsRec[f.c].push(vid);
    for(const s of ids(JSON.stringify(f.json)))if(!seen.has(s)){seen.add(s);recIds.push(s)}}
  const h=await startFixtureHub({bin,port,work});
  const rec=new Rec();const clients={};const repIds=[],repSeen=new Set();const viewIdsRep={};
  const observe=(c)=>{for(const j of c.inbox.slice(c.obs||0)){const vid=j?.result?.viewId;if(vid&&!(viewIdsRep[c.name]??=[]).includes(vid))viewIdsRep[c.name].push(vid);
      for(const s of ids(JSON.stringify(j)))if(!repSeen.has(s)){repSeen.add(s);repIds.push(s)}}c.obs=c.inbox.length};
  const sub=(c,v)=>walkStr(v,s=>{
    if(s===meta.hubUrl)return h.url;
    s=s.split(meta.hubUrl).join(h.url).split(meta.work).join(h.work);
    const g=greet[c.name],gg=c.greet;
    if(g&&gg){if(s===g.generation)return gg.generation;if(s===g.epoch)return gg.epoch}
    const vi=(viewIdsRec[c.name]||[]).indexOf(s);if(vi>=0&&viewIdsRep[c.name]?.[vi])return viewIdsRep[c.name][vi];
    return s.replace(idRe,m=>{const k=recIds.indexOf(m);return k>=0&&repIds[k]?repIds[k]:m})});
  // reply ids owed before each non-volatile outbound: replies to earlier non-volatile requests
  // a reply is waited for only if the recorded client had it before sending the next frame
  const replyAt=new Map();for(const f of frames)if(f.dir==='in'&&f.json?.t==='reply'&&!replyAt.has(f.c+'|'+f.json.id))replyAt.set(f.c+'|'+f.json.id,f.i);
  const outId=f=>f.json&&typeof f.json==='object'&&f.json.id;
  const owed=[];
  // the recording's own timing is replayed: a frame goes out no earlier than its recorded offset (the hub's counters and housekeeping depend on elapsed time)
  const first=frames.find(f=>f.dir==='out');const T0=Date.now()-(first?first.ms:0);
  for(const f of frames){
    if(f.dir==='out'&&f.vol)continue;
    const c=clients[f.c];
    if(f.dir==='out'){
      if(f.kind!=='settle'){const d=T0+f.ms-Date.now();if(d>0)await sleep(d)}
      if(f.kind==='http-get'){await httpProbe(h,f.json.path,rec,f.c);continue}
      if(f.kind==='connect'){
        if(f.json.mode==='good'){let q=f.json.query||'';
          // a resume names the log position and origin it was given: the origin is the target's own
          q=sub({name:f.c,greet:null},q);
          const m=/origin=([^&]*)/.exec(q);if(m){const o=Object.values(clients).map(x=>x.inbox.find(y=>y?.t==='log')?.origin).filter(Boolean).pop();if(o)q=q.replace(/origin=[^&]*/,'origin='+o)}
          clients[f.c]=await connect({hub:h,rec,name:f.c,profile:f.json.profile,query:q});clients[f.c].obs=0}
        else await attempt(h,rec,f.c,f.json.mode);
        continue}
      if(!c)continue;
      if(f.kind==='settle'){continue}
      if(f.kind==='end'){await c.end(f.json.quiet);continue}
      // wait for every reply owed to an earlier non-volatile request of this client
      for(const oid of owed.filter(o=>o.c===f.c&&o.i<f.i&&(replyAt.get(o.c+'|'+o.id)??Infinity)<f.i)){if(!(await c.until(x=>x.t==='reply'&&String(x.id)===String(oid.id),20000)))break}
      observe(c);
      if(f.json.raw){c.sendRaw(Uint8Array.from(Buffer.from(f.raw,'base64')))}
      else{const obj=sub(c,f.json);for(const s of ids(JSON.stringify(obj)))repSeen.add(s);c.send(obj)}
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
  let total=0;
  for(const f of readdirSync(dir).filter(x=>x.endsWith('.json')).sort()){
    const name=f.replace('.json','');if(only.length&&!only.includes(name))continue;
    const recd=JSON.parse(readFileSync(dir+'/'+f));
    const r=await replayOne(recd,{bin,port,work:`/home/h/Documents/i2cjak/backplane-gpui/build/gpui/parity/work-R`});
    writeFileSync(`${out}/${f}`,JSON.stringify(r));
    const a=normalise(recd.frames),b=normalise(r.frames);const d=[];
    for(const c of new Set([...Object.keys(a),...Object.keys(b)]))for(const s of ['out','events','deltas','replies','views','other'])diff(a[c]?.[s],b[c]?.[s],`${c}.${s}`,d);
    console.log(name,d.length?`${d.length} differences`:'identical');for(const x of d.slice(0,12))console.log('   ',x);total+=d.length;
  }
  console.log('TOTAL differences:',total);process.exit(total?1:0);
}
