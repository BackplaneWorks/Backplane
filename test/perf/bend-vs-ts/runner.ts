import { readFileSync, writeSync } from 'node:fs';
import { gunzipSync } from 'node:zlib';
import { createHash } from 'node:crypto';
import { Database } from 'bun:sqlite';
import { heapStats } from 'bun:jsc';
import * as ts from './equivalents';

const [mode, path, sizeArg, repeatsArg = '1'] = process.argv.slice(2);
const size = Number(sizeArg), repeats = Number(repeatsArg);
const input = 'test/perf/bend-vs-ts/input';
// Import only Bend jobs: TypeScript does not pay hub parse/startup memory.
const b = mode.startsWith('bend') || mode==='ts-trie' || mode==='verify' ? (await import(`./input/hub-${mode==='bend-raw'?'raw':'native'}.mjs`)).bench : null;
const Nil = {$:'Nil'};
function list(xs:any[]) { let out:any=Nil; for(let i=xs.length-1;i>=0;i--) out={$:'Con',head:xs[i],tail:out};return out; }
function array(xs:any) { const out=[];for(;xs.$==='Con';xs=xs.tail)out.push(xs.head);return out; }
function ctor(name:string,fields={}) { return {$:'../core/model.'+name,...fields}; }
function io(op:any):any {
  while(true) {
    if(op.$==='$JMP') {op=op.f(op.x);continue;}
    if(op.$==='bench.done') return op.value;
    if(!['db.Db.open','db.Db.close','db.Db.query.t'].includes(op.$)) throw Error('Unapproved IO: '+op.$);
    const v=b.effects[op.$](...op.args,op.kont);
    op=op.kont(v);
  }
}
const done=(value:any)=>({$:'bench.done',value});
function meta(n:number):ts.Metadata {
  const s=JSON.parse(readFileSync(input+'/metadata.json','utf8'));
  s.threads=s.threads.slice(0,n);
  return s;
}
function bendState(s:ts.Metadata) {
  const st=b.empty();
  for(const t of s.threads) st.threads=b.mapSet(st.threads,t.id,ctor('Thread',{...t,title:'thread',created:0,activity:0,turn:ctor(t.turn[0].toUpperCase()+t.turn.slice(1)),override:ctor('NoOverride'),settled_at:0,unsettled_at:0,pinned:false,env:ctor(t.env==='worktree'?'Worktree':'Local'),branch:'',worktree:t.worktree?'/isolated':'',provider:'claude',model:'',runtime:ctor('FullAccess'),interaction:ctor('DefaultMode'),session:''}));
  for(const a of s.asks) st.asks=b.mapSet(st.asks,a.id,ctor('Ask',{...a,tool:'Bash',detail:'',options:'',answer:''}));
  for(const t of s.tasks) st.tasks=b.mapSet(st.tasks,t.id,ctor('Task',{...t,depth:1,state:ctor('Task'+t.state[0].toUpperCase()+t.state.slice(1)),summary:'',delivered:false,provider:'claude',model:''}));
  for(const [key,value] of Object.entries(s.settings)) st.settings=b.mapSet(st.settings,key,value);
  for(const [key,values] of Object.entries(s.queued)) st.queued=b.mapSet(st.queued,key,list(values.map(msg=>ctor('QItem',{msg,text:''}))));
  return st;
}
function statuses(s:ts.Metadata,st:any,scan=false,prebuilt?:any) {
  const ix=scan?null:prebuilt??b.sideIndex(st);
  return s.threads.map(t=>{
    // Input conversion and Map lookup are outside the measured path.
    const bt=bThreads.get(t.id);
    const p=scan?b.phaseScan(st,bt):b.phase(ix,st,bt);
    return [t.id,b.phaseName(p),p.n??0];
  });
}
let bThreads=new Map();
let run:()=>any, warm:()=>any, canonical=(value:any)=>value;
if(path==='status'||path==='phase') {
  const s=meta(size),st=b?bendState(s):null;
  if(b) bThreads=new Map(array(b.mapValues(st.threads)).map((t:any)=>[t.id,t]));
  if(path==='phase') {
    const bix=b?.sideIndex(st), ix=ts.sideIndex(s);
    run=b?()=>statuses(s,st,false,bix):()=>s.threads.map(t=>[t.id,...ts.phase(ix,s,t)]);
    if(mode==='ts-trie') run=()=>s.threads.map(t=>{
      const get=(m:any,d:any)=>b.mapGet(d,m,t.id).snd;
      const asks=array(get(bix.asks,Nil)), kids=get(bix.kids,0),work=get(bix.work,0);
      const qs=array(get(st.queued,Nil)),thread=get(st.threads,ctor('Thread'));
      const starting=qs.some((q:any)=>!q.msg.startsWith('handoff:'))&&thread.env.$.endsWith('.Worktree')&&!thread.worktree;
      const approval=t.pending>0||asks.some((a:any)=>a.kind==='approval');
      const question=asks.some((a:any)=>['input','plan','browser'].includes(a.kind));
      const n=kids+work;
      const name=approval?'approval':question?'input':t.turn==='running'?'working':t.turn==='failed'?'failed':n>0?'monitoring':starting?'working':t.turn==='completed'?'complete':t.turn==='interrupted'?'stopped':'idle';
      return [t.id,name,name==='monitoring'?n:0];
    });
  } else run=b?()=>statuses(s,st,mode==='bend-scan'):()=>ts.status(s);
  warm=run;
  if(mode==='verify') {
    const expected=ts.status(s),actual=statuses(s,st);
    if(JSON.stringify(actual)!==JSON.stringify(expected)) throw Error('Status mismatch '+JSON.stringify(actual.filter((r,i)=>JSON.stringify(r)!==JSON.stringify(expected[i])).slice(0,4)));
    if(JSON.stringify(statuses(s,st,true))!==JSON.stringify(expected)) throw Error('Scan mismatch');
    // The copied store is mostly settled. Independently exercise the other
    // phase branches so a degenerate real snapshot cannot bless a bad oracle.
    for(const turn of ['idle','running','failed','completed','interrupted'])
    for(const kind of ['','approval','input','plan','browser'])
    for(const work of [0,2]) {
      const t={...s.threads[0],turn,pending:0,env:'worktree',worktree:false};
      const c:ts.Metadata={threads:[t],tasks:[],asks:kind?[{id:'a',thread:t.id,kind,open:true}]:[],settings:{['bg.'+t.id]:work?'b\twork\nc\twork':''},queued:{[t.id]:['start']}};
      const bs=bendState(c);bThreads=new Map(array(b.mapValues(bs.threads)).map((t:any)=>[t.id,t]));
      if(JSON.stringify(statuses(c,bs))!==JSON.stringify(ts.status(c))) throw Error('Phase branch mismatch');
    }
    for(const text of ['', 'x'.repeat(512), 'x'.repeat(511)+'😀é'+'x'.repeat(510), 'αβ\n中😀'.repeat(100)]) {
      const blocks=b.blocks(text);
      const actual=Array.from({length:blocks.count},(_,i)=>b.mapGet('',blocks.parts,String(i)).snd);
      if(JSON.stringify(actual)!==JSON.stringify(ts.blocks(text))) throw Error('Unicode block mismatch');
    }
  }
} else if(path==='broadcast') {
  const events=gunzipSync(readFileSync(input+'/events.jsonl.gz')).toString().trimEnd().split('\n').map(l=>{const e=JSON.parse(l);return {...e,$:'../core/model.'+e.$};});
  const th=meta(4).threads.map(t=>t.id);
  const output=new Uint32Array(size*th.length*5);
  function decisions(n:number, bend:boolean, verify=false) {
    let ps=th.map(()=>({$:'../core/thin-pace.Pace',last:0,owed:0}));
    let other=th.map(()=>({last:0,owed:0}));
    let cursor=0;
    for(let i=0;i<n;i++) {
      const e=events[i%events.length],now=i;
      let ids:any, touch:any;
      if(bend) {ids=e.$.endsWith('.CtxNoted')?Nil:b.touch(e);touch=b.touchAdd(ids,{$:'../core/thin-pace.Touch',all:false,threads:Nil});}
      else ids=ts.touched(e);
      if(verify && JSON.stringify(array(ids))!==JSON.stringify(ts.touched(e))) throw Error('Touch mismatch '+e.$);
      for(let v=0;v<th.length;v++) {
        const level=bend?b.touchLevel(touch,th[v],String(v+1)):ts.level(ids,th[v],String(v+1));
        const p=bend?b.pace(ps[v],now,level):ts.pace(ps[v],now,level);
        ps[v]=p.pace;
        if(verify) {
          const l=ts.level(ts.touched(e),th[v],String(v+1));
          const q=ts.pace(other[v],now,l);other[v]=q.pace;
          if(level!==l||p.send!==q.send||p.wait!==q.wait||p.pace.last!==q.pace.last||p.pace.owed!==q.pace.owed) throw Error('Pace mismatch '+i+' '+v);
        }
        output[cursor++]=level;output[cursor++]=Number(p.send);output[cursor++]=p.wait;
        output[cursor++]=p.pace.last;output[cursor++]=p.pace.owed;
      }
    }
    return output;
  }
  run=()=>decisions(size,!!b);
  warm=()=>decisions(10000,!!b);
  if(mode==='verify') decisions(size,true,true);
} else if(path==='history') {
  const dbPath=`build/bend-vs-ts/runtime/history-${size}.db`;
  const queries=JSON.parse(readFileSync(`build/bend-vs-ts/runtime/queries-${size}.json`,'utf8'));
  let handle='',db:any;
  const base=b?.indexEmpty();
  if(b) {const opened=io(b.dbOpen(dbPath,done));if(!opened.startsWith('ok '))throw Error(opened);handle=opened.slice(3);}
  if(!b||mode==='verify') db=new Database(dbPath,{readonly:true,safeIntegers:true});
  function normalize(p:any) {
    const rows=array(b.mapValues(p.index.rows)).map((r:any)=>{
      const e=r.entry, msg=e.$.endsWith('.Msg');
      const parts=Array.from({length:r.blocks.count},(_,i)=>b.mapGet('',r.blocks.parts,String(i)).snd);
      return [r.ordinal,e.id,e.thread,msg?'msg':'act',msg?e.role.$.split('.').at(-1).toLowerCase():e.tone,msg?'':e.kind,msg?e.text:e.summary,e.at,parts];
    }).sort((a:any,b:any)=>b[0]-a[0]);
    const lookup=(m:any)=>array(b.mapValues(m))[0]??0;
    return {ids:array(b.pageIds(p)),below:lookup(p.index.below),above:lookup(p.index.above),rows};
  }
  const plain=(p:ts.Page)=>({ids:p.ids,below:p.below,above:p.above,rows:p.rows.map(r=>[Number(r[1]),r[2],r[3],r[5],r[6],r[7],r[8],Number(r[9]),p.blocks?.get(Number(r[1]))??ts.blocks(String(r[8]))]).sort((a,b)=>Number(b[0])-Number(a[0]))});
  const page=mode==='ts-query'?ts.page:ts.fullPage;
  run=b?()=>queries.map((q:any)=>io(b.page(handle,q.thread,q.before,32,base)(done))):()=>queries.map((q:any)=>page(db,q.thread,q.before));
  canonical=pages=>pages.map(b?normalize:plain);
  warm=run;
  if(mode==='verify') for(const q of queries) {
    const actual=normalize(io(b.page(handle,q.thread,q.before,32,base)(done)));
    const expected=plain(ts.fullPage(db,q.thread,q.before));
    if(JSON.stringify(actual)!==JSON.stringify(expected)) {
      console.error(JSON.stringify({q,actual:{...actual,rows:actual.rows.map(r=>r.slice(0,6))},expected:{...expected,rows:expected.rows.map(r=>r.slice(0,6))}}));
      throw Error('Page mismatch');
    }
  }
} else throw Error('Unknown path');

if(mode==='verify') {console.log(JSON.stringify({verified:path,size}));process.exit(0);}
for(let i=0;i<4;i++) warm!();
Bun.gc(true);
const before=heapStats();
writeSync(2,'BENCH_BEGIN\n');
let result:any;
const start=performance.now();
for(let i=0;i<repeats;i++) result=run!();
const wallMs=performance.now()-start;
const after=heapStats();
writeSync(2,'BENCH_WORK_END\n');
const gcStart=performance.now();
Bun.gc(true);
const forcedGcMs=performance.now()-gcStart;
writeSync(2,'BENCH_END\n');
const digest=createHash('sha256').update(path==='broadcast'?Buffer.from(result.buffer):JSON.stringify(canonical(result))).digest('hex');
console.log(JSON.stringify({mode,path,size,repeats,wallMs,msPerOp:wallMs/repeats,heapBefore:before.heapSize,heapAfter:after.heapSize,heapCapacity:after.heapCapacity,forcedGcMs,digest,maxRssKiB:process.resourceUsage().maxRSS}));
