// Exercise actual effect registrations in separate scopes, as Bend loads them.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {createContext,runInContext} from 'node:vm';
import {test} from 'node:test';
const cacheSource=readFileSync('src/server/effects/bounded_cache.js','utf8');
const castSource=readFileSync('src/server/effects/cast.js','utf8');
const reference=readFileSync('test/fixtures/cast_reference.js','utf8');
const policySource=readFileSync('src/server/cacheio.bend','utf8');
const limits=/def Cache.budget\(\) -> CacheBudget:\s*CacheBudget\{(\d+), (\d+), (\d+)\}/.exec(policySource);
assert.ok(limits,'production typed cache policy');
const budget={entries:Number(limits[1]),bytes:Number(limits[2]),age_ms:Number(limits[3])};
const globalBytes=Number(/def Cache.global_bytes\(\) -> U32:\s*(\d+)/.exec(policySource)[1]);
function fixture({old=false,limits=budget,total=globalBytes}={}) {
  const effects=new Map(),maps=[];let clock=0;
  class TrackedMap extends Map {constructor(...args){super(...args);maps.push(this);}}
  const ctx=createContext({Buffer,Map:TrackedMap,performance:{now:()=>clock},io_eff:(name,fn)=>effects.set(name,fn)});
  function load(s){runInContext('(function(){'+s.replace(/CID\(([\w.]+)\)/g,(_,n)=>JSON.stringify(n))+'})();',ctx);}
  if(!old){load(cacheSource);effects.get('CacheFx.ready')(limits,total);}
  load(old?reference:castSource);
  const prefix=old?'Cast.':'CastFx.';
  return {ctx,maps,effects,set time(n){clock=n;},
    seen:(id,page)=>effects.get(prefix+'seen')(id,page),
    to:page=>{const out=[];for(let xs=effects.get(prefix+'to')(page);xs.$==='Con';xs=xs.tail)out.push(xs.head);return out;},
    drop:id=>effects.get('CastFx.drop')(id),
    slot:id=>ctx.BACKPLANE_CACHES.slot(id),
    stats:()=>JSON.parse(effects.get('CacheFx.stats')()),
  };
}
function bounds(f){const s=f.stats();assert.equal(s.chargedBytes,s.slots.reduce((a,c)=>a+c.chargedBytes,0));assert.ok(s.chargedBytes<=s.maxBytes);for(const c of s.slots){assert.ok(c.entries<=c.maxEntries);assert.ok(c.chargedBytes<=c.maxBytes);}return s;}

test('browser subscriptions and helper leases match original production traces',()=>{
  const a=fixture(),b=fixture({old:true});
  for(const [time,id,page] of [[0,2,'p'],[0,1,'p'],[1,2,'p'],[999,7,'q'],[1000,2,'p'],[1001,4,'p'],[3000,5,'p'],[3001,6,'q'],[7000,8,'p']]){
    a.time=time;b.time=time;assert.equal(a.seen(id,page),b.seen(id,page));
    for(const p of ['p','q','missing'])assert.deepEqual(a.to(p),b.to(p));
  }
});
test('exact three-second endpoint, age expiry and disconnect cleanup',()=>{
  const f=fixture();f.seen(1,'p');f.seen(12,'p');f.seen(1,'q');
  f.time=3000;assert.deepEqual(f.to('p'),[1,12]);
  f.drop(1);assert.deepEqual(f.to('p'),[12]);assert.deepEqual(f.to('q'),[]);
  f.time=3001;assert.deepEqual(f.to('p'),[]);assert.equal(bounds(f).chargedBytes,0);
});
test('LRU promotes reads, replacement renews age and preserves insertion order',()=>{
  const f=fixture({limits:{entries:2,bytes:4096,age_ms:100}}),c=f.slot(0);
  c.set('a','first');c.set('b','second');assert.equal(c.get('a'),'first');c.set('c','third');
  assert.equal(c.get('b'),undefined);assert.deepEqual(Array.from(c.entries(),p=>p[0]),['a','c']);
  f.time=99;c.set('a','renewed');f.time=100;assert.equal(c.get('c'),undefined);assert.equal(c.get('a'),'renewed');
  f.time=199;assert.equal(c.get('a'),undefined);bounds(f);
});
test('per-slot bytes and total bytes bound every admission',()=>{
  const f=fixture({limits:{entries:100,bytes:1024,age_ms:3001},total:1024});
  f.slot(0).set('first','x'.repeat(120));f.slot(1).set('second','y'.repeat(120));
  f.slot(0).get('first');f.slot(1).set('third','z'.repeat(120));
  assert.equal(f.slot(1).get('second'),undefined);assert.equal(f.slot(0).get('first'),'x'.repeat(120));
  assert.ok(bounds(f).slots[1].evictions>0);
});
test('rejected replacement drops stale value; unknown slots never allocate',()=>{
  const f=fixture({limits:{entries:4,bytes:1024,age_ms:100}}),c=f.slot(0);
  c.set('k','old');assert.equal(c.set('k','x'.repeat(1024)),false);assert.equal(c.get('k'),undefined);
  c.set('k','old');assert.equal(c.set('k',{}),false);assert.equal(c.get('k'),undefined);
  for(const id of [-1,2,1000,NaN]){assert.equal(f.effects.get('CacheFx.put')(id,'k','v'),false);assert.equal(f.effects.get('CacheFx.get')(id,'k').$, 'None');}
  assert.equal(f.maps.length,5);bounds(f);
});
test('owned UTF16 strings preserve raw surrogate units and empty values',()=>{
  const f=fixture(),c=f.slot(0);
  for(const s of ['',String.fromCharCode(0xd800),String.fromCharCode(0xdc00),'日本語🚀']){assert.equal(c.set(s,s),true);assert.equal(c.get(s),s);}
  assert.equal(f.effects.get('CacheFx.get')(0,'').value,'');bounds(f);
});
test('high page churn and same-key rewrites keep all indexes bounded',()=>{
  const f=fixture();
  for(let i=0;i<30000;i++){f.seen(i,'/private/page-'+i);if(i%1000===0)bounds(f);}
  const before=bounds(f);assert.ok(before.slots.some(c=>c.evictions>0));
  for(let i=0;i<30000;i++)f.seen(1,'same-page');
  const after=bounds(f);assert.ok(f.maps.every(m=>m.size<=budget.entries*2));
  f.time=budget.age_ms;assert.equal(bounds(f).chargedBytes,0);assert.ok(f.maps.every(m=>m.size===0));
});
test('numeric metrics reveal no keys/values and policy cannot mutate in place',()=>{
  const f=fixture();f.seen(1,'/private/sensitive-page');f.slot(1).set('other','private-value');
  const text=f.effects.get('CacheFx.stats')();assert.ok(!text.includes('/private'));assert.ok(!text.includes('private-value'));
  for(const c of f.stats().slots)for(const [k,v]of Object.entries(c))if(k!=='name')assert.equal(typeof v,'number');
  assert.throws(()=>f.effects.get('CacheFx.ready')({...budget,bytes:1},globalBytes),/policy changed/);
  assert.throws(()=>f.effects.get('CacheFx.ready')({...budget,entries:0},globalBytes),/Invalid/);
});
