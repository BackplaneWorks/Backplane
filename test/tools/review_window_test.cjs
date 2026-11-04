// Boat/Bun only: exercise the complete production effect closure with SQLite.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const {test} = require('node:test');
const {Database} = require('bun:sqlite');
const source = fs.readFileSync('src/server/effects/db.js', 'utf8');
const array = xs => { const out=[]; for (; xs.$==='Con'; xs=xs.tail) out.push(xs.head); return out; };
const rows = p => array(p.rows).map(array);
const budget = {rows:128, bytes:1048576};

function fixture() {
  const effects={}, stats={active:0, reads:0, savepoints:0, scans:0, failScan:0, failRead:0, failFinalize:false};
  let actual;
  class OwnedDatabase {
    constructor(path, options) { actual=new Database(path,options); }
    exec(sql) {
      if (sql==='SAVEPOINT bp_review_page') stats.savepoints++;
      if (sql==='RELEASE bp_review_page') stats.savepoints--;
      return actual.exec(sql);
    }
    prepare(sql) {
      if (sql.startsWith('SELECT seq,256+')) {
        stats.scans++;
        if (stats.scans===stats.failScan) throw Error('injected later page failure');
      }
      const st=actual.prepare(sql); stats.active++;
      return {
        *iterate(...ps) { yield* st.iterate(...ps); },
        get(...ps) { stats.reads++; if(stats.reads===stats.failRead) throw Error('injected payload failure'); return st.get(...ps); },
        values() { assert.fail('eager rows are forbidden'); },
        finalize() { stats.active--; st.finalize(); if(stats.failFinalize) throw Error('injected cleanup failure'); },
      };
    }
    close() { actual.close(); }
  }
  const Db={};
  for(const name of ['open','exec','run','batch','script','query','close','review_page']) {
    const tag=()=>{}; tag.label=name; tag.t=name+'.t'; Db[name]=tag;
  }
  const context={require:n=>n==='bun:sqlite'?{Database:OwnedDatabase}:require(n), Buffer,
    CID:x=>typeof x==='function'?x.label:x, io_eff:(k,f)=>effects[k]=f, Db,
    Nil:'Nil',Con:'Con',HistoryPage:'HistoryPage',True:'True',False:'False',DbText:'DbText',DbInt:'DbInt',DbNull:'DbNull'};
  vm.runInNewContext(source,context);
  const opened=effects.open(':memory:'); assert.match(opened,/^ok /); const h=opened.slice(3);
  actual.exec('CREATE TABLE entry(seq INTEGER PRIMARY KEY,ord INTEGER,id TEXT,thread TEXT,src TEXT,kind TEXT,role TEXT,akind TEXT,text TEXT,at INTEGER); CREATE INDEX entry_thread ON entry(thread,seq)');
  const insert=actual.prepare('INSERT INTO entry VALUES(?,?,?,?,?,?,?,?,?,?)');
  return {stats,db:actual,
    add(seq,{ord=10000-seq,id='id-'+seq,thread='a',src='',kind='act',role='review',akind='add',text='text '+seq,at=seq}={}) {
      insert.run(seq,ord,id,thread,src,kind,role,akind,text,at);
    },
    page(after=0,through=10000,b=budget,thread='a') {
      const p=effects.review_page(h,thread,after,through,b);
      assert.equal(stats.active,0,'every prepared statement finalized');
      assert.equal(stats.savepoints,0,'page savepoint released');
      return p;
    },
    close() { insert.finalize(); assert.equal(effects.close(h),'ok'); },
  };
}

test('indexed thread/seq windows preserve every column and ascending seq rather than ordinal',()=>{
  const f=fixture(); try {
    for(let n=1;n<=301;n++)f.add(n,{text:n%2?'Unicode 🛶 日本語\n'+n:'',akind:n%2?'add':'resolve'});
    f.add(302,{thread:'private'});f.add(303,{kind:'msg'});f.add(304,{role:'tool'});f.add(305,{thread:'elsewhere',src:'a'});
    const expected=f.db.query("SELECT seq,ord,id,thread,src,kind,role,akind,text,at FROM entry WHERE thread='a' AND kind='act' AND role='review' AND seq>0 AND seq<=305 ORDER BY seq").values().map(r=>r.map(x=>String(x??'')));
    let at=0,got=[],pages=0;
    for (;;) { const p=f.page(at,305); assert.equal(p.error,'');const xs=rows(p);assert.ok(xs.length<=128);got.push(...xs);pages++; if(p.done.$==='True')break;assert.ok(p.next>at);at=p.next; }
    assert.equal(pages,3);assert.deepEqual(got,expected);
    const plan=f.db.query("EXPLAIN QUERY PLAN SELECT seq FROM entry INDEXED BY entry_thread WHERE thread='a' AND kind='act' AND role='review' AND seq>0 AND seq<=305 ORDER BY seq LIMIT 129").values();
    assert.match(JSON.stringify(plan),/entry_thread.*thread=.*seq>/);
  } finally { f.close(); }
});

test('fixed inclusive through excludes later appends between pages',()=>{
  const f=fixture();try {
    for(let n=1;n<=3;n++)f.add(n);
    const first=f.page(0,3,{rows:2,bytes:1048576});assert.equal(first.next,2);assert.equal(first.done.$,'False');
    f.add(4);const last=f.page(first.next,3,{rows:2,bytes:1048576});assert.deepEqual(rows(last).map(r=>r[0]),['3']);assert.equal(last.done.$,'True');
    const empty=f.page(3,3);assert.equal(empty.next,3);assert.equal(empty.done.$,'True');assert.deepEqual(rows(empty),[]);
  } finally {f.close();}
});

test('byte-limited short page is a continuation, with payload lookahead kept in SQLite',()=>{
  const f=fixture();try {
    f.add(1,{text:'🛶'.repeat(100)});f.add(2,{text:'y'.repeat(800)});f.add(3,{text:'z'});
    const p=f.page(0,3,{rows:128,bytes:1024});assert.equal(p.done.$,'False');assert.equal(p.next,1);assert.equal(rows(p).length,1);assert.equal(f.stats.reads,1);
    const q=f.page(p.next,3,{rows:128,bytes:1024});assert.equal(q.next,2);assert.equal(rows(q).length,1);assert.equal(q.done.$,'False');
    const r=f.page(q.next,3,{rows:128,bytes:1024});assert.equal(r.next,3);assert.equal(r.done.$,'True');
  } finally {f.close();}
});

test('one indivisible oversized action advances once, without caching or losing its successor',()=>{
  const f=fixture();try {
    const text='🛶'.repeat(300000);f.add(1,{text});f.add(2,{text:'later'});
    const p=f.page(0,2);assert.equal(rows(p).length,1);assert.equal(rows(p)[0][8],text);assert.equal(p.next,1);assert.equal(p.done.$,'False');
    const q=f.page(p.next,2);assert.equal(rows(q).length,1);assert.equal(q.next,2);assert.equal(q.done.$,'True');assert.equal(f.stats.reads,2);
  } finally {f.close();}
});

test('zero/invalid budgets and invalid range/thread fail without SQL or cursor advancement',()=>{
  const f=fixture();try {
    for(const [a,z,b,th] of [[0,4,{rows:0,bytes:1024},'a'],[0,4,{rows:129,bytes:1024},'a'],[0,4,{rows:1,bytes:255},'a'],[0,4,{rows:1,bytes:1048577},'a'],[4,3,budget,'a'],[-1,3,budget,'a'],[0,3,budget,''],[0,3,budget,'a\0'],[0,Infinity,budget,'a']]) {
      const p=f.page(a,z,b,th);assert.notEqual(p.error,'');assert.deepEqual(rows(p),[]);assert.equal(p.next,a);assert.equal(p.done.$,'False');
    }
    assert.equal(f.stats.scans,0);
  } finally {f.close();}
});

test('read, later-page and cleanup failures return no partial page and release resources',()=>{
  const f=fixture();try {
    for(let n=1;n<=3;n++)f.add(n);
    f.stats.failRead=2;const bad=f.page();assert.notEqual(bad.error,'');assert.deepEqual(rows(bad),[]);assert.equal(bad.next,0);
    f.stats.failRead=0;const first=f.page(0,3,{rows:1,bytes:1024});assert.equal(first.next,1);
    f.stats.failScan=f.stats.scans+1;const late=f.page(first.next,3);assert.notEqual(late.error,'');assert.deepEqual(rows(late),[]);assert.equal(late.next,1);
    f.stats.failFinalize=true;const cleanup=f.page();assert.notEqual(cleanup.error,'');assert.deepEqual(rows(cleanup),[]);
    f.stats.failFinalize=false;assert.equal(f.page().error,'');
  } finally {f.close();}
});

test('missing thread index fails closed and the handle remains usable',()=>{
  const f=fixture();try {
    f.add(1);f.db.exec('DROP INDEX entry_thread');const p=f.page();assert.notEqual(p.error,'');assert.deepEqual(rows(p),[]);
    f.db.exec('CREATE INDEX entry_thread ON entry(thread,seq)');assert.equal(f.page().error,'');
  } finally {f.close();}
});
