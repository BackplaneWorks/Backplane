// Normalise and compare two recordings (or a recording and a replay). usage: bun compare.mjs A_DIR B_DIR
// Output: differences per scenario/client; exit 1 when any. NORMALISATIONS lists every rewrite, with its reason.
import {readFileSync,readdirSync} from 'node:fs';
export const NORMALISATIONS=[
 'N1 clocks: numeric fields at, rid, now, clock, last, since-time keys -> <T>; 10-digit (17xxxxxxxx/18xxxxxxxx) and 13-digit epoch numbers inside strings -> <T>; hh:mm and durations (e.g. "Finished 18:26 · 2s") -> <time>. Reason: wall clock.',
 'N13 ordinals are per client: each client numbers the ids it sees by first appearance in its own stream, so what another client saw first cannot shift them.',
 'N14 delta tokens: viewDelta token and base strings (the hub revision a body was kept under) -> ordinals, like ids; an empty base (a whole reply) stays empty.',
 'N2 ids: p/t/g/m-ids with a 10-digit time part (p1793834744-7, t..., g..., m...), stand-in session/message ids (fake-<pid>, m<pid>-n-n, tu<pid>-..), view ids (<gen>:<n>), and the greeting generation and epoch -> ordinals by first appearance per recording (ID1, ID2...). Reason: ids embed time, pids and counters.',
 'N3 viewRevision -> <R> (and checked non-decreasing per client). Reason: counts hub-internal changes, which coalesce with timing.',
 'N4 delta frames: consecutive delta frames of one thread joined into one item with "@<ms>@" stamps stripped; frame boundaries are not compared. Reason: coalescing follows read timing (200 ms pace).',
 'N5 changes frames: expanded to one item per event; batch boundaries are not compared. Reason: batching follows timing.',
 'N12 late volatile replies: a reply is volatile exactly when its request was a volatile poll (matched by client and request id), whenever it arrives; a reply to a real request that arrives during a poll window is kept.',
 'N6 dropped: hb frames, view.changed frames (their number and position follow timing; each reply carries the revision it saw), volatile frames (polls made while waiting for a turn). Reason: timing-driven counts.',
 'N11 harness steps: `settle` and `end` pseudo-frames (the client program waiting or closing) are control, not wire traffic, and are not compared. Reason: a replay waits by reply id, not by the recorded quiet times.',
 'N7 streams: per client the sequence of events, deltas, replies and view replies are compared as separate ordered streams; relative order between streams is not compared. Reason: network timing.',
 'N9 info frames: all info frames of a client are merged into one item (keys unioned, later values win; environment keys dropped); position and repeat count are not compared. The `other` stream (http, upgrade, close, info) is compared as a sorted set. Reason: the hub repeats info on its own timer.',
 'N10 hub start-up settings: a contiguous run of SettingSet events whose key is machine.id, accounts.inventory, allowance.codex, projects.missing or tz.* is sorted by key. Reason: background start-up jobs finish in varying order.',
 'N8 info frames: home paths, ports, machine ids, tokens, pids -> placeholders. Reason: environment.',
];
const CLOCK_KEYS=new Set(['at','rid','now','clock','last','nudged','attempted_at','eligibleAt','sentAt','checked']);
function mkNorm(){
  const ids=new Map();const ord=v=>{if(!ids.has(v))ids.set(v,'ID'+(ids.size+1));return ids.get(v)};
  const idRe=/\b([ptgm]\d{10}-\d+(?::\d+)?)\b|\bfake-\d+\b|\b[mt][a-z]?\d{3,8}-\d+-\d+(?::\d+)?\b|\btu\d+-\d+-\d+\b|\b\d+:\d{12,15}\b/g;
  const str=(s,key)=>{
    s=s.replace(/@\d{13}@/g,'@<T>@').replace(/(?:\/[^"\s]*)?\/work-[A-Za-z0-9]+/g,'<work>').replace(/backplane[-\/][0-9a-f]{8}\b/g,'backplane-<wt>').replace(/\bc[a-z0-9-]*?\d{10}-\d+\b/g,m=>ord(m));
    s=s.replace(/\b1[78]\d{8}\b(?!-)/g,'<T>').replace(/\b1[78]\d{11}\b/g,'<T>');
    s=s.replace(idRe,m=>ord(m));
    s=s.replace(/\b\d{1,2}:\d\d\b/g,'<time>').replace(/(Finished <time> · )[0-9hms ]+/,'$1<dur>');
    s=s.replace(/\/home\/[^"\s]*?\/parity\/work-[A-Za-z0-9]+/g,'<work>').replace(/\b127\.0\.0\.1:\d+\b/g,'127.0.0.1:<port>').replace(/\bm[0-9a-f]{48}\b/g,'<machine>');
    return s;};
  const go=(v,key)=>{
    if(v===undefined||v===null||typeof v==='boolean')return v;
    if(typeof v==='number')return CLOCK_KEYS.has(key)?'<T>':(v>=1.7e9&&v<1.9e9?'<T>':(v>=1.7e12&&v<1.9e12?'<T>':v));
    if(typeof v==='string'){if(v!==''&&(key==='epoch'||key==='generation'||key==='origin'||key==='hub'||key==='viewId'||key==='token'||key==='base'))return ord(v);if(key==='viewRevision')return '<R>';return str(v,key)}
    if(Array.isArray(v))return v.map(x=>go(x,key));
    const o={};for(const k of Object.keys(v).sort())o[k]=go(v[k],k);return o;};
  return go;
}
export function normalise(frames){
  const G={};const gof=cl=>G[cl]??(G[cl]=mkNorm());let go=null;const per={};
  const S=(c)=>per[c]??(per[c]={out:[],events:[],deltas:[],replies:[],views:[],other:[]});
  const open={}; // per client+thread delta accumulation
  const flush=(c,th)=>{const k=c+'|'+th;if(open[k]!==undefined){S(c).deltas.push({delta:th,text:gof(c)(open[k].replace(/@\d{13}@/g,''))});delete open[k]}};
  const lastRev={};
  // replies to volatile polls can arrive after the poll window closed: they are volatile too (matched by client and request id)
  const volIds=new Set(frames.filter(f=>f.dir==='out'&&f.vol&&f.json&&f.json.id!==undefined).map(f=>f.c+'|'+f.json.id));
  for(const f of frames){
    if(f.dir==='in'&&f.json&&f.json.t==='reply'&&volIds.has(f.c+'|'+f.json.id))continue;
    go=gof(f.c);const isReply=f.dir==='in'&&f.json&&f.json.t==='reply';
    if(f.vol&&!isReply)continue;const c=f.c,j=f.json,s=S(c);
    if(f.dir==='out'&&(f.kind==='settle'||f.kind==='end'))continue;
    if(f.dir==='out'&&f.kind==='check'){s.other.push(go(j));continue}
    if(f.dir==='out'){s.out.push(go(j));continue}
    if(f.kind==='http'){s.other.push(go({http:f.status,type:f.ctype,json:j}));continue}
    if(f.kind==='upgrade'||f.kind==='close'||f.kind==='connect-failed'){s.other.push(go({[f.kind]:j}));continue}
    const t=j?.t;
    if(t==='hb')continue;
    if(t==='view.changed')continue;
    if(t==='delta'){const k=c+'|'+j.thread;open[k]=(open[k]??'')+j.text;continue}
    if(t==='delta_end'){flush(c,j.thread);s.deltas.push({delta_end:go(j.thread)});continue}
    for(const th of Object.keys(open).filter(k=>k.startsWith(c+'|')).map(k=>k.split('|')[1]))flush(c,th);
    if(t==='changes'||t==='log'){
      if(t==='log'){const {items,...rest}=j;s.events.push({log:go({...rest,info:undefined})});s.other.push({logInfoKeys:Object.keys(j.info||{}).sort()});for(const it of items||[])s.events.push(go(it))}
      else for(const it of j.items||[])s.events.push(go(it));continue}
    if(t==='reply'){const r=j.result;if(r&&r.viewRevision!==undefined){const n=Number(r.viewRevision);if(lastRev[c]!==undefined&&n<lastRev[c])s.views.push({revisionWentBackwards:true});lastRev[c]=n;s.views.push(go(j))}else s.replies.push(go(j));continue}
    if(t==='info'){s.infoSeen=Object.assign(s.infoSeen||{},j.info||{});continue}
    if(false){s.other.push(go({info:j.info&&Object.fromEntries(Object.entries(j.info).filter(([k])=>!/^(home|listen|machines|models|update|accounts\.live|kicad\.|freecad|browser\.|telemetry\.|tailnet|tailscale|bots\.remote|machine\.id)/.test(k)))}));continue}
    s.other.push(go(j));
  }
  for(const k of Object.keys(open)){const [c,th]=k.split('|');flush(c,th)}
  const STARTUP=/^(machine\.id|accounts\.inventory|allowance\.codex|projects\.missing|tz\..*)$/;
  for(const s of Object.values(per)){
    if(s.infoSeen){s.other.push({info:gof(Object.keys(per).find(k=>per[k]===s))(Object.fromEntries(Object.entries(s.infoSeen).filter(([k])=>!/^(home|listen|machines|models|update|accounts\.live|kicad\.|freecad|browser\.|telemetry\.|tailnet|tailscale|bots\.remote|machine\.id|os|arch)/.test(k))))});delete s.infoSeen}
    s.other.sort((a,b)=>JSON.stringify(a)<JSON.stringify(b)?-1:1);
    const ev=s.events,out=[];let i=0;
    while(i<ev.length){if(ev[i].$==='SettingSet'&&STARTUP.test(ev[i].key)){let j=i;while(j<ev.length&&ev[j].$==='SettingSet'&&STARTUP.test(ev[j].key))j++;out.push(...ev.slice(i,j).sort((a,b)=>a.key<b.key?-1:1));i=j}else out.push(ev[i++])}
    s.events=out}
  return per;
}
export function diff(a,b,path='',out=[]){
  if(JSON.stringify(a)===JSON.stringify(b))return out;
  if(Array.isArray(a)&&Array.isArray(b)){const n=Math.max(a.length,b.length);if(a.length!==b.length)out.push(`${path}: length ${a.length} vs ${b.length}`);for(let i=0;i<Math.min(a.length,b.length)&&out.length<40;i++)diff(a[i],b[i],`${path}[${i}]`,out);return out}
  if(a&&b&&typeof a==='object'&&typeof b==='object'&&!Array.isArray(a)&&!Array.isArray(b)){for(const k of new Set([...Object.keys(a),...Object.keys(b)]))if(out.length<40)diff(a[k],b[k],`${path}.${k}`,out);return out}
  out.push(`${path}: ${JSON.stringify(a)?.slice(0,160)} vs ${JSON.stringify(b)?.slice(0,160)}`);return out;
}
export function compareDirs(A,B){
  let total=0;const rep={};
  for(const f of readdirSync(A).filter(x=>x.endsWith('.json')).sort()){
    const a=normalise(JSON.parse(readFileSync(A+'/'+f)).frames),b=normalise(JSON.parse(readFileSync(B+'/'+f)).frames);
    const d=[];for(const c of new Set([...Object.keys(a),...Object.keys(b)]))for(const s of ['out','events','deltas','replies','views','other'])diff(a[c]?.[s],b[c]?.[s],`${c}.${s}`,d);
    rep[f]=d;total+=d.length;}
  return {total,rep};
}
if(import.meta.main){const {total,rep}=compareDirs(process.argv[2],process.argv[3]);
  for(const [f,d] of Object.entries(rep)){console.log(f,d.length?`${d.length} differences`:'identical');for(const x of d.slice(0,15))console.log('   ',x)}
  console.log('TOTAL differences:',total);process.exit(total?1:0)}
