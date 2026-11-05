// Normalise and compare two recordings (or a recording and a replay). usage: bun compare.mjs A_DIR B_DIR
// Output: differences per scenario/client; exit 1 when any. NORMALISATIONS lists every rewrite, with its reason.
import {readFileSync,readdirSync} from 'node:fs';
export const NORMALISATIONS=[
 'N1 clocks: numeric fields at, rid, now, clock, last, since-time keys -> <T>; 10-digit (17xxxxxxxx/18xxxxxxxx) and 13-digit epoch numbers inside strings -> <T>; hh:mm and durations (e.g. "Finished 18:26 · 2s") -> <time>. Reason: wall clock.',
 'N16 housekeeping events read the id table but never add to it, so their number and position cannot shift the ordinals of other events (an id first seen only there is <id>).',
 'N13 ordinals are per client: each client numbers the ids it sees by first appearance in its own stream, so what another client saw first cannot shift them.',
 'N14 delta tokens: viewDelta token and base strings (the hub revision a body was kept under) -> ordinals, like ids; an empty base (a whole reply) stays empty.',
 'N2 ids: p/t/g/m-ids with a 10-digit time part (p1793834744-7, t..., g..., m...), stand-in session/message ids (fake-<pid>, m<pid>-n-n, tu<pid>-..), view ids (<gen>:<n>), and the greeting generation and epoch -> ordinals by first appearance per recording (ID1, ID2...). Reason: ids embed time, pids and counters.',
 'N3 viewRevision -> <R> (and checked non-decreasing per client). Reason: counts hub-internal changes, which coalesce with timing.',
 'N4 delta frames: consecutive delta frames of one thread joined into one item with "@<ms>@" stamps stripped; frame boundaries are not compared. Reason: coalescing follows read timing (200 ms pace).',
 'N5 changes frames: expanded to one item per event; batch boundaries are not compared. Reason: batching follows timing.',
  'N18 http probe auth flag: an http-get probe with auth:false is compared as {path} only. Reason: recordings made before the harness wrote the flag omit it; false is its default.',
 'N19 origin in a query string (the since/origin of a reconnect): the value is replaced by its FORM only: <origin:decimal> (Bend: the decimal `at` of the log\'s first change), <origin:hex>, <origin:empty> or <origin:other>. Reason: the value differs per log, but a hub that answers in another form is a real difference (a client resuming across hubs reads it), so the form is compared.',
 'N20 sets: the housekeeping events (hk) and the greeting\'s info keys (logInfoKeys) are compared as multisets, one difference per element found on one side only. Reason: the hub appends them in varying order; positional comparison turned one missing key into a difference at every later position.',
 'N21 arrays of different length are aligned by key (an object\'s id or key, else its JSON) with a longest common subsequence; an inserted or removed element is one difference. Reason: positional comparison made one insertion a difference at every later position. Equal-length arrays are still compared position by position.',
 'N22 liveness.<thread> housekeeping events are left out of the hk set. Reason: the hub\'s periodic tick writes them once agent output was seen, and whether a tick fell inside the recording differs between two runs of the JS hub itself (self-diff 1). The state lane\'s liveness policy has its own tests.',
 'N23 a bot\'s `look` (a random number the hub draws) -> <N>; WorktreeSet events join the housekeeping set (N10). Reason: the hub draws the look at creation, and a bot\'s folder is made by a job that lands before or after the bot\'s thread.',
 'N24 a thread\'s text deltas are joined until its delta_end (or the end of the recording), whatever other frames arrive in between. Reason: the split points of a stream depend on timing; their joined text does not.',
 'N25 `<T>.<digits>` (a time and a sub-second count, as in the alert record) -> `<T>.<n>`. Reason: the count differs between two runs of the JS hub.',
 'N26 object keys are normalised like values (ids inside keys such as drafts @sch:<thread> -> ID n), then ordered. Reason: a thread a scenario creates has a different id in every run, and so does its key.',
 'N12 late volatile replies: volatile means a poll request and its reply, matched by client and request id, whenever the reply arrives; every other inbound frame is kept even if it arrived during a poll window (the recorder flags by arrival time, which is not a property of the frame).',
 'N6 dropped: hb frames, view.changed frames (their number and position follow timing; each reply carries the revision it saw), volatile frames (polls made while waiting for a turn). Reason: timing-driven counts.',
 'N11 harness steps: `settle` and `end` pseudo-frames (the client program waiting or closing) are control, not wire traffic, and are not compared. Reason: a replay waits by reply id, not by the recorded quiet times.',
 'N7 streams: per client the sequence of events, deltas, replies and view replies are compared as separate ordered streams; relative order between streams is not compared. Reason: network timing.',
 'N9 info frames: all info frames of a client are merged into one item (keys unioned, later values win; environment keys dropped); position and repeat count are not compared. The `other` stream (http, upgrade, close, info) is compared as a sorted set. Reason: the hub repeats info on its own timer.',
 'N10 housekeeping events: a contiguous run of events that concurrent hub jobs append (SettingSet keys machine.id, accounts.*, allowance.*, projects.missing, tz.*, autosettle.*, liveness.*, turn.from/speed.*, restart.*, provider.*, fallback.*, tok.*, summary.*, told.*, alert.*, account.*, star.*; ActivityLogged of kind git or spawn; WorktreeSkipped) is taken out of the event stream and compared as a set (distinct events, canonical order; no order, no repeat count, and not their position among the other events); every other event stays in order. Reason: start-up, git probes and settle/liveness bookkeeping finish in varying order and repeat with elapsed time.',
 'N15 counters: numeric seq and changes (the hub change count at the instant a view was drawn) -> <N>. Reason: housekeeping events land between draws.',
 'N8 info frames: home paths, ports, machine ids, tokens, pids -> placeholders. Reason: environment.',
];
const CLOCK_KEYS=new Set(['at','rid','now','clock','last','nudged','attempted_at','eligibleAt','sentAt','checked']);
function mkNorm(){
  const ids=new Map();let ro=false;const ord=v=>{if(!ids.has(v)){if(ro)return '<id>';ids.set(v,'ID'+(ids.size+1))}return ids.get(v)};
  const idRe=/\b([ptgmkbro]\d{10}-\d+(?::\d+)?)\b|\bfake-\d+\b|\b[mt][a-z]?\d{3,8}-\d+-\d+(?::\d+)?\b|(?<![A-Za-z])tu\d+-\d+-\d+\b|\b\d+:\d{12,15}\b/g;
  const str=(s,key)=>{
    s=s.replace(/([?&]origin=)([^&"\s]*)/g,(m,a,v)=>a+(/^\d+$/.test(v)?'<origin:decimal>':/^[0-9a-f]{8,}$/.test(v)?'<origin:hex>':v===''?'<origin:empty>':'<origin:other>'));
    s=s.replace(/@\d{13,19}@/g,'@<T>@').replace(/(?:\/[^"\s]*)?\/work-[A-Za-z0-9]+/g,'<work>').replace(/backplane[-\/][0-9a-f]{8}\b/g,'backplane-<wt>').replace(/\bc[a-z0-9-]*?\d{10}-\d+\b/g,m=>ord(m));
    s=s.replace(/\b1[78]\d{8}\b(?!-)/g,'<T>').replace(/\b1[78]\d{11}\b/g,'<T>');
    s=s.replace(/<T>\.\d+\b/g,'<T>.<n>');
    s=s.replace(idRe,m=>ord(m));
    s=s.replace(/\b\d{1,2}:\d\d\b/g,'<time>').replace(/(Finished <time> · )[0-9hms ]+/,'$1<dur>');
    s=s.replace(/\/home\/[^"\s]*?\/parity\/work-[A-Za-z0-9]+/g,'<work>').replace(/\b127\.0\.0\.1:\d+\b/g,'127.0.0.1:<port>').replace(/\bm[0-9a-f]{48}\b/g,'<machine>');
    return s;};
  const go=(v,key)=>{
    if(v===undefined||v===null||typeof v==='boolean')return v;
    if((key==='seq'||key==='changes')&&typeof v==='number')return '<N>';
    if(typeof v==='number')return key==='look'?'<N>':CLOCK_KEYS.has(key)?'<T>':(v>=1.7e9&&v<1.9e9?'<T>':(v>=1.7e12&&v<1.9e12?'<T>':v));
    if(typeof v==='string'){if(v!==''&&(key==='epoch'||key==='generation'||key==='origin'||key==='hub'||key==='viewId'||key==='token'||key==='base'))return ord(v);if(key==='viewRevision')return '<R>';return str(v,key)}
    if(Array.isArray(v))return v.map(x=>go(x,key));
    // keys can carry ids too (drafts "@sch:<thread>"): normalised like values, then ordered
    const o={};for(const k of Object.keys(v).sort()){const nk=str(k,'');o[nk]=go(v[k],k)}return Object.fromEntries(Object.entries(o).sort((a,b)=>a[0]<b[0]?-1:a[0]>b[0]?1:0));};
  go.readonly=f=>{ro=true;try{return f()}finally{ro=false}};
  return go;
}
export function normalise(frames){
  const G={};const gof=cl=>G[cl]??(G[cl]=mkNorm());let go=null;const per={};
  const S=(c)=>per[c]??(per[c]={out:[],events:[],hkEv:[],deltas:[],replies:[],views:[],other:[],hk:[]});
  const open={}; // per client+thread delta accumulation
  const flush=(c,th)=>{const k=c+'|'+th;if(open[k]!==undefined){S(c).deltas.push({delta:gof(c)(th),text:gof(c)(open[k].replace(/@\d{13,19}@/g,''))});delete open[k]}};
  const HK_RAW=/^(machine\.id|accounts\.inventory|allowance\.codex|projects\.missing|tz\..*|autosettle\..*|liveness\..*|turn\.(from|speed)\..*|restart\..*|provider\..*|fallback\..*|tok\..*|summary\..*|told\..*|alert\..*|account\..*|star\..*)$/;
  const hkRaw=e=>(e.$==='SettingSet'&&HK_RAW.test(e.key))||(e.$==='ActivityLogged'&&(e.kind==='git'||e.kind==='spawn'))||e.$==='WorktreeSkipped'||e.$==='WorktreeSet';
  const evNorm=(c,it)=>hkRaw(it)?{hk:true,v:gof(c).readonly(()=>gof(c)(it))}:{hk:false,v:gof(c)(it)};
  const lastRev={};
  // replies to volatile polls can arrive after the poll window closed: they are volatile too (matched by client and request id)
  const volIds=new Set(frames.filter(f=>f.dir==='out'&&f.vol&&f.json&&f.json.id!==undefined).map(f=>f.c+'|'+f.json.id));
  for(const f of frames){
    if(f.dir==='in'&&f.json&&f.json.t==='reply'&&volIds.has(f.c+'|'+f.json.id))continue;
    go=gof(f.c);const isReply=f.dir==='in'&&f.json&&f.json.t==='reply';
    if(f.dir==='out'&&f.vol)continue;const c=f.c,j=f.json,s=S(c);
    if(f.dir==='out'&&(f.kind==='settle'||f.kind==='end'))continue;
    if(f.dir==='out'&&f.kind==='check'){s.other.push(go(j));continue}
    if(f.dir==='out'){s.out.push(go(f.kind==='http-get'&&j&&j.auth===false?{path:j.path}:j));continue}
    if(f.kind==='http'){s.other.push(go({http:f.status,type:f.ctype,json:j}));continue}
    if(f.kind==='upgrade'||f.kind==='close'||f.kind==='connect-failed'){s.other.push(go({[f.kind]:j}));continue}
    const t=j?.t;
    if(t==='hb')continue;
    if(t==='view.changed')continue;
    if(t==='delta'){const k=c+'|'+j.thread;open[k]=(open[k]??'')+j.text;continue}
    if(t==='delta_end'){flush(c,j.thread);s.deltas.push({delta_end:go(j.thread)});continue}
    if(t==='changes'||t==='log'){
      if(t==='log'){const {items,...rest}=j;s.events.push({log:go({...rest,info:undefined})});s.other.push({logInfoKeys:Object.keys(j.info||{}).sort()});for(const it of items||[]){const r=evNorm(c,it);(r.hk?s.hkEv:s.events).push(r.v)}}
      else for(const it of j.items||[]){const r=evNorm(c,it);(r.hk?s.hkEv:s.events).push(r.v)};continue}
    if(t==='reply'){const r=j.result;if(r&&r.viewRevision!==undefined){const n=Number(r.viewRevision);if(lastRev[c]!==undefined&&n<lastRev[c])s.views.push({revisionWentBackwards:true});lastRev[c]=n;s.views.push(go(j))}else s.replies.push(go(j));continue}
    if(t==='info'){s.infoSeen=Object.assign(s.infoSeen||{},j.info||{});continue}
    if(false){s.other.push(go({info:j.info&&Object.fromEntries(Object.entries(j.info).filter(([k])=>!/^(home|listen|machines|models|update|accounts\.live|kicad\.|freecad|browser\.|telemetry\.|tailnet|tailscale|bots\.remote|machine\.id)/.test(k)))}));continue}
    s.other.push(go(j));
  }
  for(const k of Object.keys(open)){const [c,th]=k.split('|');flush(c,th)}
  // housekeeping events are appended by concurrent hub jobs (start-up, git probe, settle/liveness bookkeeping): the set matters, their order does not
  const HK_SET_UNUSED=/^(machine\.id|accounts\.inventory|allowance\.codex|projects\.missing|tz\..*|autosettle\..*|liveness\..*|turn\.(from|speed)\..*|restart\..*|provider\..*|fallback\..*|tok\..*|summary\..*|told\..*|alert\..*|account\..*|star\..*)$/;
  for(const s of Object.values(per)){
    if(s.infoSeen){s.other.push({info:gof(Object.keys(per).find(k=>per[k]===s))(Object.fromEntries(Object.entries(s.infoSeen).filter(([k])=>!/^(home|listen|machines|models|update|accounts\.live|kicad\.|freecad|browser\.|telemetry\.|tailnet|tailscale|bots\.remote|machine\.id|os|arch)/.test(k))))});delete s.infoSeen}
    s.other.sort((a,b)=>JSON.stringify(a)<JSON.stringify(b)?-1:1);
    s.hk=[...new Set(s.hkEv.map(e=>JSON.stringify(e)))].sort().map(x=>JSON.parse(x));delete s.hkEv}
  // deltas still open when the recording ends (no delta_end seen): one run each, in the order they began
  for(const k of Object.keys(open)){const [c,th]=[k.slice(0,k.indexOf('|')),k.slice(k.indexOf('|')+1)];flush(c,th)}
  return per;
}
// Arrays of different lengths are aligned by key (an object's `id` or `key`, else its JSON) with a longest common subsequence: an element
// inserted or removed is one difference, and the elements after it are still compared with their own counterparts, not shifted ones.
const keyOf=x=>x&&typeof x==='object'&&!Array.isArray(x)?((typeof x.id==='string'||typeof x.id==='number')?'id:'+x.id:typeof x.key==='string'?'key:'+x.key+(x.value!==undefined?'='+JSON.stringify(x.value):''):JSON.stringify(x)):JSON.stringify(x);
function keyed(a,b,path,out){
  const ka=a.map(keyOf),kb=b.map(keyOf),n=a.length,m=b.length;
  const L=Array.from({length:n+1},()=>new Int32Array(m+1));
  for(let i=n-1;i>=0;i--)for(let j=m-1;j>=0;j--)L[i][j]=ka[i]===kb[j]?L[i+1][j+1]+1:Math.max(L[i+1][j],L[i][j+1]);
  let i=0,j=0;
  while(i<n||j<m){
    if(i<n&&j<m&&ka[i]===kb[j]){diff(a[i],b[j],`${path}[${i}]`,out);i++;j++}
    else if(j>=m||(i<n&&L[i+1][j]>=L[i][j+1])){out.push(`${path}[${i}]: only in A: ${JSON.stringify(a[i])?.slice(0,160)}`);i++}
    else{out.push(`${path}[${j}]: only in B: ${JSON.stringify(b[j])?.slice(0,160)}`);j++}
  }
  return out;
}
// housekeeping events (hk) and the greeting's info keys (logInfoKeys) are sets: the hub appends them in varying order, and one missing or
// extra element must not shift (and so count) every later position. Compared as multisets: each element on one side only is one difference.
const SETS=/\.(hk|logInfoKeys)$/;
function multiset(a0,b0,path,out){
  // liveness.<thread> is written by the hub's periodic tick once agent output was seen: whether a tick fell inside the recording varies run to run on the JS hub itself
  const drop=x=>!(path.endsWith('.hk')&&x&&typeof x.key==='string'&&x.key.startsWith('liveness.'));const a=a0.filter(drop),b=b0.filter(drop);
  const cnt=new Map();for(const x of a){const k=JSON.stringify(x);cnt.set(k,(cnt.get(k)||0)+1)}
  for(const x of b){const k=JSON.stringify(x);cnt.set(k,(cnt.get(k)||0)-1)}
  for(const [k,n] of cnt)if(n!==0)out.push(`${path}: ${n>0?'only in A':'only in B'}${Math.abs(n)>1?' x'+Math.abs(n):''}: ${k.slice(0,160)}`);
  return out;
}
export function diff(a,b,path='',out=[]){
  if(JSON.stringify(a)===JSON.stringify(b))return out;
  if(SETS.test(path)&&Array.isArray(a)&&Array.isArray(b))return multiset(a,b,path,out);
  if(Array.isArray(a)&&Array.isArray(b)&&a.length!==b.length&&a.length*b.length<=4e6)return keyed(a,b,path,out);
  if(Array.isArray(a)&&Array.isArray(b)){const n=Math.max(a.length,b.length);if(a.length!==b.length)out.push(`${path}: length ${a.length} vs ${b.length}`);for(let i=0;i<Math.min(a.length,b.length);i++)diff(a[i],b[i],`${path}[${i}]`,out);for(let i=Math.min(a.length,b.length);i<n;i++)out.push(`${path}[${i}]: ${a.length>b.length?'only in A':'only in B'}: ${JSON.stringify(a.length>b.length?a[i]:b[i])?.slice(0,160)}`);return out}
  if(a&&b&&typeof a==='object'&&typeof b==='object'&&!Array.isArray(a)&&!Array.isArray(b)){for(const k of new Set([...Object.keys(a),...Object.keys(b)]))diff(a[k],b[k],`${path}.${k}`,out);return out}
  out.push(`${path}: ${JSON.stringify(a)?.slice(0,160)} vs ${JSON.stringify(b)?.slice(0,160)}`);return out;
}
export function compareDirs(A,B){
  let total=0;const rep={};
  if(!readdirSync(A).some(x=>x.endsWith('.json'))){console.error('compare: no recordings in '+A+' (an empty comparison proves nothing)');process.exit(2)}
  for(const f of readdirSync(A).filter(x=>x.endsWith('.json')).sort()){
    const a=normalise(JSON.parse(readFileSync(A+'/'+f)).frames),b=normalise(JSON.parse(readFileSync(B+'/'+f)).frames);
    const d=[];for(const c of new Set([...Object.keys(a),...Object.keys(b)]))for(const s of ['out','events','hk','deltas','replies','views','other'])diff(a[c]?.[s],b[c]?.[s],`${c}.${s}`,d);
    rep[f]=d;total+=d.length;}
  return {total,rep};
}
if(import.meta.main){const {total,rep}=compareDirs(process.argv[2],process.argv[3]);
  for(const [f,d] of Object.entries(rep)){console.log(f,d.length?`${d.length} differences`:'identical');for(const x of (process.argv.includes('--all')?d:d.slice(0,15)))console.log('   ',x)}
  console.log('TOTAL differences:',total);process.exit(total?1:0)}
