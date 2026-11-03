const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const vm = require('node:vm');
function reader(t) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'bp-hot-stream-')); t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const effects = {}, requests = [];
  const io = new Proxy(fs, { get(o, k) { if(k === 'readSync') return (...a) => {requests.push(a[3]); return o.readSync(...a);}; return o[k]; } });
  const c = vm.createContext({ require: n => n === 'fs' ? io : require(n), Buffer, Hot:{lines:'lines',scan:'scan'}, WireScan:'WireScan', Nil:'Nil', Con:'Con', CID:x=>x, io_eff:(id, fn)=>effects[id]=fn });
  vm.runInContext(fs.readFileSync(path.join(__dirname,'../../src/server/effects/hotboot.js'),'utf8'),c);
  const list = a => a.reduceRight((tail,head)=>({$:'Con',head,tail}),{$:'Nil'});
  const array = a => {const r=[]; for(;a.$==='Con';a=a.tail) r.push(a.head);return r;};
  return {dir,requests,list,array,...effects};
}
test('streamed validation matches whole-line reference across UTF8/chunk boundaries, skipped and empty lines, and unterminated tails', t => {
  const r=reader(t), first='{"$":"SettingSet","key":"launch.input.x","value":"'+ '🧰'.repeat(18000) +'","at":9}';
  const lines=[first,'','abc','é'.repeat(70000),'skip-me','tail'], data=Buffer.from(lines.join('\n'));
  const file=path.join(r.dir,'events');fs.writeFileSync(file,data);
  const skipStart=Buffer.byteLength(lines.slice(0,4).join('\n'))+1, skips=r.list([String(skipStart),String(skipStart+8)]);
  const scan=r.scan(file,0,data.length,skips), full=r.array(r.lines(file,0,data.length,skips));
  assert.equal(scan.count,4);assert.equal(scan.error,'');
  const publicFirst='{"$":"SettingSet","key":"launch.input.x","value":"","at":9}';
  assert.equal(scan.first,publicFirst);assert.deepEqual(full,['tail',lines[3],'abc',publicFirst]);
  assert.ok(r.requests.every(n=>n<=65536));
});
test('a growing log has a constant-size retained scan result, and short/invalid ranges fail closed', t => {
  const r=reader(t),file=path.join(r.dir,'events');fs.writeFileSync(file,'first\n'+'later\n'.repeat(100000));
  const size=fs.statSync(file).size, a=r.scan(file,0,size,r.list([]));
  assert.equal(a.count,100001);assert.equal(a.first,'first');assert.ok(JSON.stringify(a).length<100);
  assert.notEqual(r.scan(file,0,size+5,r.list([])).error,'');
  assert.deepEqual(r.array(r.lines(file,0,size+5,r.list([]))),[]);
  assert.equal(r.scan(file,2,2,r.list([])).count,0);
});
