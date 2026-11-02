// Exact service IO effect, published bytes, disposable homes; no hub/live data.
// bun test/tools/kicad_library_test.ts [verified-header.js]
import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { tmpdir } from 'node:os';
import { gzipSync, gunzipSync } from 'node:zlib';
import { createHash } from 'node:crypto';
const dir = fs.mkdtempSync(path.join(tmpdir(), 'bp-library-test-'));
const header = process.argv[2] || path.join(dir, 'header.js');
if (!process.argv[2]) execFileSync('python3', ['scripts/kicad-library-header.py', header]);
new Function(fs.readFileSync(header, 'utf8'))();
const originalBundle = (globalThis as any).BACKPLANE_KICAD_BUNDLE;
const source = fs.readFileSync('src/server/effects/kicad_library.js', 'utf8').replace(/CID\(([A-Za-z0-9_.]+)\)/g, '"$1"');
const load = () => new Function('io_eff', 'require', source + ';return {prepare:bp_kl_prepare,unpack:bp_kl_unpack,table:bp_kl_table,lock:bp_kl_lock,child:bp_kl_child_env};')(() => {}, require);
const originalEnv = {...process.env};
let passed = 0;
function test(name: string, f: () => void) {
  process.env = {...originalEnv}; (globalThis as any).BACKPLANE_KICAD_BUNDLE = originalBundle;
  f(); console.log('ok ' + name); passed++;
}
function fixture(label: string) {
  const root = path.join(dir, label), home = path.join(root, 'hub 日本'), custom = path.join(root, 'custom (profile)');
  fs.mkdirSync(path.join(custom, '10.99'), {recursive:true});
  const text = '(fp_lib_table\n ; keep comment (verbatim)\n (lib (name "Custom (lab)")(type "KiCad")(uri "${LAB_PARTS}/odd.pretty")(options "x=y")(descr "custom"))\n (lib (name "Relative")(type "KiCad")(uri "relative.pretty")(options "")(descr ""))\n (lib (name "Config path")(type "KiCad")(uri "${KICAD_CONFIG_HOME}/parts.pretty")(options "")(descr ""))\n)\n';
  const table = path.join(custom, '10.99/fp-lib-table'); fs.writeFileSync(table, text);
  fs.writeFileSync(path.join(custom, '10.99/sym-lib-table'), '(sym_lib_table (lib (name "My symbols")))\n');
  fs.writeFileSync(path.join(custom, '10.99/kicad_common.json'), '{"environment":{"vars":{"LAB_PARTS":"/opt/my parts"}},"keep":true}');
  fs.mkdirSync(path.join(custom, '10.99/prefs')); fs.writeFileSync(path.join(custom, '10.99/prefs/sentinel'), 'keep');
  return {root,home,custom,text,table,runtime:path.join(root,'runtime')};
}
try {
test('absent bake/no KiCad does no IO or env change', () => {
  const f=fixture('none'); delete (globalThis as any).BACKPLANE_KICAD_BUNDLE;
  assert.equal(load().prepare(f.runtime,f.home,f.custom),''); assert(!fs.existsSync(f.home));
  (globalThis as any).BACKPLANE_KICAD_BUNDLE=originalBundle;
  assert.equal(load().prepare('',f.home,f.custom),''); assert(!fs.existsSync(f.home));
});
test('offline cold + warm selection, exact inventory and custom profile semantics', () => {
  const f=fixture('cold'), api=load(), start=performance.now();
  assert.equal(api.prepare(f.runtime,f.home,f.custom),''); const cold=performance.now()-start;
  const library=process.env.BACKPLANE_KICAD_LIBRARY!,config=process.env.KICAD_CONFIG_HOME!;
  assert(path.isAbsolute(library)); assert(library.includes(originalBundle.pin.source_commit));
  const {files}=api.unpack(originalBundle); assert.equal(files.size,270);
  assert.equal([...files.keys()].filter(n=>n.endsWith('.kicad_mod')).length,199);
  assert.equal([...files.keys()].filter(n=>n.endsWith('.step')).length,42);
  const merged=fs.readFileSync(path.join(config,'10.99/fp-lib-table'),'utf8');
  for(const n of originalBundle.pin.library_nicknames) assert(merged.includes('(name "'+n+'")'));
  assert(merged.includes('${LAB_PARTS}/odd.pretty')); assert(merged.includes('"relative.pretty"'));
  assert(merged.includes(f.custom+'/parts.pretty')); assert(merged.includes('; keep comment (verbatim)'));
  assert.equal(fs.readFileSync(f.table,'utf8'),f.text);
  for(const n of ['sym-lib-table','kicad_common.json','prefs/sentinel']) assert.equal(fs.readFileSync(path.join(config,'10.99',n),'utf8'),fs.readFileSync(path.join(f.custom,'10.99',n),'utf8'));
  const at=performance.now(); assert.equal(api.prepare(f.runtime,f.home,f.custom),''); const warm=performance.now()-at;
  assert.equal(fs.readFileSync(path.join(config,'10.99/fp-lib-table'),'utf8'),merged);
  // Actual child inheritance, the same env transport used by CLI/GUI/agents.
  const child=spawnSync(process.execPath,['-e','console.log(JSON.stringify([process.env.BACKPLANE_KICAD_LIBRARY,process.env.KICAD_CONFIG_HOME]))'],{encoding:'utf8',env:{...process.env}});
  assert.equal(child.status,0); assert.deepEqual(JSON.parse(child.stdout),[library,config]);
  console.log(JSON.stringify({coldMs:cold,warmMs:warm,files:files.size}));
});
test('custom nickname conflict fails without changing global or prior env', () => {
  const f=fixture('conflict'); fs.writeFileSync(f.table,'(fp_lib_table (lib (name "Backplane_Shoggoth")(type "KiCad")(uri "/my/custom.pretty")))');
  process.env.BACKPLANE_KICAD_LIBRARY='/prior/library'; process.env.KICAD_CONFIG_HOME='/prior/config';
  assert.match(load().prepare(f.runtime,f.home,f.custom),/conflicts/);
  assert.equal(process.env.BACKPLANE_KICAD_LIBRARY,'/prior/library'); assert.equal(process.env.KICAD_CONFIG_HOME,'/prior/config');
  assert(fs.readFileSync(f.table,'utf8').includes('/my/custom.pretty')); assert(!fs.existsSync(path.join(f.home,'kicad-library/profiles')));
});
test('new export/agent child reuses original profile and does not decode archive again', () => {
  const f=fixture('child');assert.equal(load().prepare(f.runtime,f.home,f.custom),'');
  const config=process.env.KICAD_CONFIG_HOME,root=process.env.BACKPLANE_KICAD_LIBRARY;
  // No archive decoder is needed for the validated existing extraction.
  (globalThis as any).BACKPLANE_KICAD_BUNDLE={...originalBundle,base64:'not decoded'};
  const at=performance.now();assert.equal(load().prepare(f.runtime,f.home,config),'');
  assert.equal(process.env.KICAD_CONFIG_HOME,config);assert.equal(process.env.BACKPLANE_KICAD_LIBRARY,root);
  console.log(JSON.stringify({newChildValidationMs:performance.now()-at}));
});
test('runtime rejects unsafe tar paths, links, duplicates and invalid PAX before activation', () => {
  function member(name:string,type='0',body=Buffer.alloc(0)) {
    const h=Buffer.alloc(512);h.write(name,0,100,'utf8');h.write('0000644\0',100);h.write('0000000\0',108);h.write('0000000\0',116);
    h.write(body.length.toString(8).padStart(11,'0')+'\0',124);h.write('00000000000\0',136);h.fill(32,148,156);h.write(type,156);
    const sum=h.reduce((a,b)=>a+b,0);h.write(sum.toString(8).padStart(6,'0')+'\0 ',148);
    return Buffer.concat([h,body,Buffer.alloc((512-body.length%512)%512)]);
  }
  const tar=gunzipSync(Buffer.from(originalBundle.base64,'base64'));
  const cases=[member('../escape'),member('/absolute'),member('Backplane_KiCad_Library/../escape'),member('Backplane_KiCad_Library/a\\b'),
    member('Backplane_KiCad_Library/link','2'),member('Backplane_KiCad_Library/link','1'),member('Backplane_KiCad_Library/BUNDLE_MANIFEST.json'),
    member('././@PaxHeader','x',Buffer.from('99 path=../escape\n')),member('Backplane_KiCad_Library/device','3')];
  for(let i=0;i<cases.length;i++){
    const f=fixture('unsafe-'+i),archive=gzipSync(Buffer.concat([cases[i],tar]));
    const bad={...originalBundle,base64:archive.toString('base64'),pin:{...originalBundle.pin,bytes:archive.length,sha256:createHash('sha256').update(archive).digest('hex')}};
    (globalThis as any).BACKPLANE_KICAD_BUNDLE=bad;assert.match(load().prepare(f.runtime,f.home,f.custom),/unsafe|duplicate|invalid/i);
    assert(!fs.existsSync(path.join(f.home,'kicad-library/profiles')));
  }
});
test('archive corruption and wrong manifest pin fail before any activation', () => {
  const f=fixture('bad-archive'); const bad=structuredClone(originalBundle); bad.base64='AAAA';
  (globalThis as any).BACKPLANE_KICAD_BUNDLE=bad; assert.match(load().prepare(f.runtime,f.home,f.custom),/checksum/); assert(!fs.existsSync(path.join(f.home,'kicad-library/profiles')));
  bad.base64=originalBundle.base64; bad.pin.source_commit='0'.repeat(40);
  assert.match(load().prepare(f.runtime,f.home,f.custom),/manifest mismatch/); assert(!fs.existsSync(path.join(f.home,'kicad-library/profiles')));
});
test('partial/corrupt extracted cache repairs from pinned bytes; damaged copy retained', () => {
  const f=fixture('repair'),api=load(); assert.equal(api.prepare(f.runtime,f.home,f.custom),'');
  const installed=process.env.BACKPLANE_KICAD_LIBRARY!, file=path.join(installed,'3dmodels/Resistor_SMD.3dshapes/R_0402_1005Metric.step');
  const expected=fs.readFileSync(file); fs.writeFileSync(file,'partial');
  assert.equal(api.prepare(f.runtime,f.home,f.custom),''); assert.deepEqual(fs.readFileSync(file),expected);
  assert(fs.readdirSync(path.dirname(installed)).some(n=>n.includes('.damaged-')));
  fs.unlinkSync(file); assert.equal(load().prepare(f.runtime,f.home,f.custom),''); assert.deepEqual(fs.readFileSync(file),expected);
});
test('malformed managed table preserves bytes and prior env', () => {
  const f=fixture('bad-table'),api=load(); assert.equal(api.prepare(f.runtime,f.home,f.custom),'');
  const managed=path.join(process.env.KICAD_CONFIG_HOME!,'10.99/fp-lib-table'); fs.writeFileSync(managed,'(broken');
  const before=process.env.BACKPLANE_KICAD_LIBRARY; assert.match(api.prepare(f.runtime,f.home,f.custom),/table/);
  assert.equal(fs.readFileSync(managed,'utf8'),'(broken'); assert.equal(process.env.BACKPLANE_KICAD_LIBRARY,before);
});
test('new/resumed provider children get explicit env; a repair child never inherits an invalid bundled path', () => {
  const f=fixture('provider'),api=load();assert.equal(api.prepare(f.runtime,f.home,f.custom),'');
  const installed=process.env.BACKPLANE_KICAD_LIBRARY,profile=process.env.KICAD_CONFIG_HOME;
  function child(){const env=[];for(let x=api.child();x.$==='Con';x=x.tail)env.push(x.head);
    const r=spawnSync('env',[...env,process.execPath,'-e','console.log(JSON.stringify([process.env.BACKPLANE_KICAD_LIBRARY,process.env.KICAD_CONFIG_HOME]))'],{encoding:'utf8',env:{...process.env,BACKPLANE_KICAD_LIBRARY:'unvalidated'}});
    assert.equal(r.status,0,r.stderr);return JSON.parse(r.stdout);}
  assert.deepEqual(child(),[installed,profile]);
  const at=performance.now();for(let i=0;i<5;i++)assert.equal(api.prepare(f.runtime,f.home,f.custom),'');
  console.log(JSON.stringify({meanWarmTurnMs:(performance.now()-at)/5}));
  fs.writeFileSync(path.join(profile!,'10.99/fp-lib-table'),'(bad');assert.match(api.prepare(f.runtime,f.home,f.custom),/table/);
  assert.deepEqual(child(),[null,f.custom]);
});
test('managed symlink refused without writing its target', () => {
  const f=fixture('link'),outside=path.join(f.root,'outside'); fs.mkdirSync(f.home,{recursive:true});fs.mkdirSync(outside);
  fs.symlinkSync(outside,path.join(f.home,'kicad-library'),'dir'); assert.match(load().prepare(f.runtime,f.home,f.custom),/symlink/); assert.deepEqual(fs.readdirSync(outside),[]);
});
test('a setup held past the wait is refused, and the lock releases for retry', () => {
  const f=fixture('lock'),api=load(),base=path.join(f.home,'kicad-library');fs.mkdirSync(base,{recursive:true});
  const release=api.lock(path.join(base,'.setup.lock'));
  try { const t=Date.now(); assert.throws(() => load().lock(path.join(base,'.setup.lock'),200),/active/); assert(Date.now()-t>=150); }
  finally { release(); }
  assert.equal(api.prepare(f.runtime,f.home,f.custom),'');
});
test('a setup another process holds briefly is waited for', () => {
  const f=fixture('lockwait'),base=path.join(f.home,'kicad-library'),lock=path.join(base,'.setup.lock'),held=path.join(f.root,'held');
  fs.mkdirSync(base,{recursive:true});
  // another process (a plot or render child) holds the lock for 400 ms
  const holder=Bun.spawn([process.execPath,'-e',`const f=require('node:fs'),{dlopen}=require('bun:ffi');const c=dlopen(process.platform==='darwin'?'/usr/lib/libSystem.B.dylib':'libc.so.6',{flock:{args:['i32','i32'],returns:'i32'}}).symbols;const fd=f.openSync(${JSON.stringify(lock)},'a+');if(c.flock(fd,6)!==0)process.exit(3);f.writeFileSync(${JSON.stringify(held)},'');Bun.sleepSync(400);`]);
  const until=Date.now()+5000; while(!fs.existsSync(held)&&Date.now()<until) Bun.sleepSync(10);
  assert(fs.existsSync(held),'holder never took the lock');
  const t=Date.now(); assert.equal(load().prepare(f.runtime,f.home,f.custom),''); assert(Date.now()-t>=200);
  holder.kill();
});
test('interrupted lock file is reusable without stale-owner cleanup', () => {
  const f=fixture('interrupted'),lock=path.join(f.home,'kicad-library/.setup.lock');fs.mkdirSync(path.dirname(lock),{recursive:true});fs.writeFileSync(lock,'old owner');
  assert.equal(load().prepare(f.runtime,f.home,f.custom),'');
});
console.log(`KiCad library: ${passed} checks passed`);
} finally { process.env=originalEnv; (globalThis as any).BACKPLANE_KICAD_BUNDLE=originalBundle;fs.rmSync(dir,{recursive:true,force:true}); }
