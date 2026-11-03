// Exact production foreign effects, exercised without compiling a whole hub.
// This qualifies the wire-history allocation root, not canonical-state caches,
// agent transport, full application latency or CPU. No private input is accepted.
const fs = require('node:fs'), os = require('node:os'), path = require('node:path');
const vm = require('node:vm'), assert = require('node:assert/strict');
const { createHash } = require('node:crypto'), { Database } = require('bun:sqlite');
const opts = new Map(), allowed = new Set(['--events','--duration-secs','--out']);
for(let i=2;i<process.argv.length;i+=2){const k=process.argv[i],v=process.argv[i+1];assert(allowed.has(k)&&v&&!opts.has(k),'invalid option');opts.set(k,v);}
const events=Number(opts.get('--events')||1370000), seconds=Number(opts.get('--duration-secs')||1800);
assert(Number.isSafeInteger(events)&&events>=1000&&events<=2000000);
assert(Number.isFinite(seconds)&&seconds>=30&&seconds<=3600);
const out=path.resolve(opts.get('--out')||'build/wire-history-soak/report.json');
fs.mkdirSync(path.dirname(out),{recursive:true});
const home=fs.mkdtempSync(path.join(os.tmpdir(),'bp-wire-history-'));
const sourceFiles=['src/server/effects/hotboot.js','src/server/effects/wire_log.js','src/core/wire-retention.bend'];
const root=path.resolve(__dirname,'../..'), hash=p=>createHash('sha256').update(fs.readFileSync(path.join(root,p))).digest('hex');
const hashes=Object.fromEntries(sourceFiles.map(p=>[p,hash(p)]));
const report={schema:1,kind:'production-wire-effects-only',events,seconds,sourceHashes:hashes,forcedGC:false,samples:[],ok:false};
const rows=fs.openSync(out+'.samples.jsonl','w',0o600);
let db,fd,insert,count=0,offset=0,produced=0,replayed=0,highRss=0;
const effects={},context=vm.createContext({require,Buffer,CID:x=>x,io_eff:(id,f)=>effects[id]=f,
  Hot:{lines:'lines',scan:'scan'},WireLog:{page:'page'},WireScan:'WireScan',WirePage:'WirePage',Nil:'Nil',Con:'Con'});
for(const p of sourceFiles.slice(0,2))vm.runInContext(fs.readFileSync(path.join(root,p),'utf8'),context,{filename:p});
const mono=()=>Number(process.hrtime.bigint())/1e6;
function sample(stage){
  const p=Object.fromEntries(fs.readFileSync('/proc/self/status','utf8').split('\n').filter(x=>/^Vm(RSS|HWM|Swap):/.test(x)).map(x=>[x.split(':')[0],Number(x.match(/\d+/)[0])]));
  const m=process.memoryUsage(),h=require('bun:jsc').heapStats();highRss=Math.max(highRss,p.VmRSS);
  const s={stage,atMs:mono(),rssKiB:p.VmRSS,hwmKiB:p.VmHWM,swapKiB:p.VmSwap,heapBytes:h.heapSize,heapCapacity:h.heapCapacity,externalBytes:m.external,events:count,logBytes:offset,replayed};
  assert.equal(p.VmSwap,0,'swap invalidates memory result');assert(p.VmRSS<768*1024,'wire-effect RSS exceeds768MiB');
  fs.writeSync(rows,JSON.stringify(s)+'\n');console.log('MEMORY',JSON.stringify(s));return s;
}
function append(n){
  db.exec('BEGIN');let chunk='',startOffset=offset;
  try{for(let i=0;i<n;i++){
    const line=JSON.stringify({$: 'SettingSet',key:'synthetic.overwrite',value:`${count}:`+'x'.repeat(900),at:count+1});
    const len=Buffer.byteLength(line);insert.run(++count,offset,len,'');offset+=len+1;chunk+=line+'\n';
    if(Buffer.byteLength(chunk)>=65536){fs.writeSync(fd,chunk);chunk='';}
  }if(chunk)fs.writeSync(fd,chunk);db.exec('COMMIT');}
  catch(e){db.exec('ROLLBACK');throw e;}
}
function page(since,through){
  const p=effects.page('owned',home,since,through);assert.equal(p.error,'');let n=0,bytes=0;
  for(let xs=p.lines;xs.$==='Con';xs=xs.tail){n++;bytes+=Buffer.byteLength(xs.head);const e=JSON.parse(xs.head);assert.equal(e.at,since+n);assert.equal(e.key,'synthetic.overwrite');}
  assert(n>0&&n<=1024);assert(bytes<=1048576);replayed+=n;return n;
}
async function main(){
  try{
    db=new Database(path.join(home,'store.db'));db.exec('PRAGMA journal_mode=WAL;PRAGMA cache_size=-2048;CREATE TABLE line(seq INTEGER PRIMARY KEY,off INTEGER,len INTEGER,wire TEXT)');
    insert=db.prepare('INSERT INTO line VALUES(?,?,?,?)');fd=fs.openSync(path.join(home,'events.jsonl'),'wx',0o600);
    report.empty=sample('empty');
    while(count<events){append(Math.min(5000,events-count));if(count===100000)report.at100k=sample('100k');}
    const scan=effects.scan(path.join(home,'events.jsonl'),0,offset,{$:'Nil'});assert.equal(scan.error,'');assert.equal(scan.count,events);
    assert.equal(JSON.parse(scan.first).at,1);assert(Buffer.byteLength(JSON.stringify(scan))<2048,'scan retained full history');
    report.scan={count:scan.count,retainedBytes:Buffer.byteLength(JSON.stringify(scan))};report.seeded=sample('1370k');
    // Exercise every historical cursor once before warmup; retain no prior page.
    for(let at=0;at<events;)at+=page(at,events);
    const warmEnd=mono()+60000;let cursor=0;
    const cycle=()=>{append(20);produced+=20;cursor+=page(cursor,count);if(cursor>=count)cursor=0;};
    while(mono()<warmEnd){cycle();await new Promise(r=>setTimeout(r,25));}
    const start=sample('warm'),endAt=mono()+seconds*1000;let next=mono()+60000;
    // Fixed cadence:20 owners x40 events/s, bounded disk pages, no event array.
    let n=0,mx=0,my=0,xx=0,xy=0,last=start;
    function add(s){const x=(s.atMs-start.atMs)/3600000,dx=x-mx;n++;mx+=dx/n;my+=(s.rssKiB-my)/n;xx+=dx*(x-mx);xy+=dx*(s.rssKiB-my);last=s;}
    add(start);
    while(mono()<endAt){cycle();if(mono()>=next){add(sample('measured'));next+=60000;}await new Promise(r=>setTimeout(r,25));}
    add(sample('end'));const hours=(last.atMs-start.atMs)/3600000,slope=xy/xx,endpoint=(last.rssKiB-start.rssKiB)/start.rssKiB/hours;
    report.growth={samples:n,hours,warmRssKiB:start.rssKiB,endRssKiB:last.rssKiB,slopeKiBPerHour:slope,slopeFractionPerHour:slope/start.rssKiB,endpointFractionPerHour:endpoint};
    assert(n>=3&&hours>=seconds/3600*0.99);assert(slope/start.rssKiB<=0.10&&endpoint<=0.10,'RSS slope/endpoint exceeds10%/hour');
    for(const p of sourceFiles)assert.equal(hash(p),hashes[p],'source changed');
    report.ok=true;
  }catch(e){report.error=String(e.stack||e);try{report.failureMemory=sample('failure');}catch{}}
  finally{
    report.produced=produced;report.replayed=replayed;report.totalEvents=count;report.highRssKiB=highRss;
    try{if(insert)insert.finalize();if(db)db.close();if(fd!==undefined)fs.closeSync(fd);}finally{fs.rmSync(home,{recursive:true,force:true});fs.closeSync(rows);}
    fs.writeFileSync(out,JSON.stringify(report,null,2)+'\n');console.log(report.ok?'PASS':'FAIL',out);process.exitCode=report.ok?0:1;
  }
}
main();
