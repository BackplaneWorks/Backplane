// Synthetic child processes and private temp homes only; no hub, network or accounts.
const {test}=require('node:test'),assert=require('node:assert/strict');
const fs=require('node:fs'),os=require('node:os'),path=require('node:path');
const {spawnSync}=require('node:child_process');
const root=path.resolve(__dirname,'../..');
const source=fs.readFileSync(path.join(root,'src/server/effects/diagnostics.js'),'utf8');
const methods=['start','status','tick','flush','event','control','export'];
const harness=`const fs=require('node:fs'),vm=require('node:vm');
globalThis.require=require;globalThis.Diagnostics=Object.fromEntries(${JSON.stringify(methods)}.map(x=>[x,x]));globalThis.CID=x=>x;globalThis.io_eff=()=>{};
vm.runInThisContext(${JSON.stringify(source)});
diagnostics_start('0.13.1-dev.local.354db14d1');`;
function home(t){const p=fs.mkdtempSync(path.join(os.tmpdir(),'bp-exit-'));t.after(()=>fs.rmSync(p,{recursive:true,force:true}));return p;}
function run(runtime,dir,extra){const script=path.join(dir,'synthetic-child.cjs');fs.writeFileSync(script,harness+'\n'+extra);return spawnSync(runtime,[script],{env:{...process.env,BACKPLANE_HOME:dir},encoding:'utf8',timeout:10000});}
function records(dir){return fs.readdirSync(path.join(dir,'diagnostics')).filter(n=>n.endsWith('.jsonl')).flatMap(n=>fs.readFileSync(path.join(dir,'diagnostics',n),'utf8').trim().split('\n').filter(Boolean).map(JSON.parse));}
const runtimes=[['Node',process.execPath],['Bun',process.env.BACKPLANE_TEST_BUN||'bun']];
for(const [name,runtime] of runtimes){
 test(name+': explicit exit persists one private bounded record with exact exit code',t=>{
  const p=home(t),r=run(runtime,p,`process.exit(37);`);assert.ifError(r.error);assert.equal(r.status,37);
  const rows=records(p);assert.equal(rows.length,1);assert.equal(rows[0].event,'exit');assert.equal(rows[0].exitCode,37);assert(rows[0].pid>0);assert.equal(rows[0].version,'0.13.1');assert.equal(rows[0].exception,'none');assert.deepEqual(rows[0].stack,[]);assert(Math.abs(rows[0].at-Date.now()/1000)<15);
  let allocated=0;for(const n of fs.readdirSync(path.join(p,'diagnostics'))){const st=fs.statSync(path.join(p,'diagnostics',n));allocated+=Math.max(st.size,st.blocks*512);assert.equal(st.mode&0o777,0o600);assert(st.size<=65536);}assert(allocated<=1048576);assert.equal(fs.statSync(path.join(p,'diagnostics')).mode&0o777,0o700);
 });
 test(name+': runtime codes use the same eight-bit process exit value',t=>{
  for(const code of [300,-2]){const p=home(t),r=run(runtime,p,'process.exit('+code+');');assert.ifError(r.error);assert.equal(records(p)[0].exitCode,r.status);}
 });
 test(name+': fatal exception keeps runtime failure and strips message, names and paths',t=>{
  const p=home(t),r=run(runtime,p,`function PRIVATE_prompt_cad_token(){throw new TypeError('SECRET_TOKEN /private/CAD/design.step account@example.invalid 192.0.2.4');} PRIVATE_prompt_cad_token();`);assert.ifError(r.error);assert.equal(r.status,1);
  const rows=records(p);assert.equal(rows.length,1);assert.equal(rows[0].exception,'TypeError');assert.equal(rows[0].exitCode,1);assert(rows[0].stack.length>0);assert(rows[0].stack.length<=8);assert(!/PRIVATE|SECRET|private|CAD|account@|192\.0\.2\.4/.test(JSON.stringify(rows)));
 });
 test(name+': scheduler rejection hook records sanitized stack and retains exit 1',t=>{
  const p=home(t);const loop=fs.readFileSync(path.join(root,'src/server/effects/loop.js'),'utf8');const a=loop.indexOf('function bp_start(main) {'),z=loop.indexOf('\nif (BP_ASYNC)',a);assert(a>0&&z>a);
  const r=run(runtime,p,`function bp_run(){return Promise.reject(Object.assign(new RangeError('SECRET'),{stack:'RangeError: SECRET\\n    at SECRET_TOKEN (/private/CAD:123:456)\\n    at account@example.invalid (https://192.0.2.4/token:234:567)'}));} function io_errs(){};\n${loop.slice(a,z)}\nbp_start(()=>{});`);assert.ifError(r.error);assert.equal(r.status,1);
  const row=records(p)[0];assert.equal(row.exception,'RangeError');assert.deepEqual(row.stack,[[123,456],[234,567]]);assert.equal(row.exitCode,1);assert(!JSON.stringify(row).includes('SECRET'));
 });
 test(name+': disabled collection and failed full-disk write cannot alter exit',t=>{
  for(const mode of ['off','full','partial']){const p=home(t);const code=mode==='off'?`diagnostics_control('off');`:mode==='full'?`fs.writeSync=()=>{throw Object.assign(new Error('SECRET'),{code:'ENOSPC'});};`:`fs.writeSync=()=>0;`;
   const r=run(runtime,p,code+'process.exit(47);');assert.ifError(r.error);assert.equal(r.status,47);assert.deepEqual(records(p),[]);}
 });
 test(name+': private exit rows stay out of optional report export and obey retention',t=>{
  const p=home(t);assert.equal(run(runtime,p,'process.exit(23);').status,23);const r=run(runtime,p,`const text=diagnostics_export();if(JSON.parse(text).events.some(r=>r.event==='exit'))process.exit(91);diagnostics_control('clear');process.exit(0);`);assert.equal(r.status,0);assert.equal(records(p).length,1);
  for(const n of fs.readdirSync(path.join(p,'diagnostics')).filter(n=>n.endsWith('.jsonl'))){const q=path.join(p,'diagnostics',n),rows=fs.readFileSync(q,'utf8').trim().split('\n').map(JSON.parse);for(const x of rows)x.at-=8*86400;fs.writeFileSync(q,rows.map(x=>JSON.stringify(x)+'\n').join(''));}
  assert.equal(run(runtime,p,'process.exit(29);').status,29);assert.deepEqual(records(p).map(x=>x.exitCode),[29]);
 });
}
