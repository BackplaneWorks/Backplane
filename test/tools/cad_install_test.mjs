// Run only on Boat, in the enclosing verified 3.7GB/no-swap scope.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import http from 'node:http';
import vm from 'node:vm';
import cp from 'node:child_process';
import { createHash } from 'node:crypto';
import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
const source = fs.readFileSync('src/server/effects/cad-install.js', 'utf8');
const api = vm.runInNewContext(source + '\n({start:bp_cad_start,next:bp_cad_next_async,cancel:bp_cad_cancel,jobs:bp_cad_jobs})', {
  require, process, Buffer, fetch, AbortController, performance, setTimeout, clearTimeout,
  CadFx: {start:1,next:2,cancel:3}, CID: x=>x, io_eff() {},
});
const root = fs.mkdtempSync(path.join(os.tmpdir(), 'bp-cad-progress-'));
const compiled=path.join(root,'cad-io.js');
const emit=cp.spawnSync('bend',['test/native/cad_install_io.bend','-o',compiled],{encoding:'utf8',timeout:90000});
assert.equal(emit.status,0,emit.stdout+emit.stderr);
async function compiledRun(h,spec,wanted) {
  const result=await new Promise((resolve,reject)=>{
    const child=cp.spawn('bun',[compiled],{env:{...process.env,TEST_CAD_HOME:h,TEST_CAD_SPEC:spec,TEST_CAD_WANTED:wanted}});
    let out='';child.stdout.on('data',b=>out+=b);child.stderr.on('data',b=>out+=b);
    const timeout=setTimeout(()=>{child.kill();reject(Error('compiled effect hung: '+out));},15000);
    child.on('error',reject);child.on('exit',code=>{clearTimeout(timeout);resolve({code,out});});
  });
  assert.equal(result.code,0,result.out);assert.match(result.out,/ok compiled install terminated/);assert.doesNotMatch(result.out,/FAIL|Error:/);
}
const bytes = Buffer.alloc(128 * 1024, 42), count = 24;
const digest = createHash('sha256').update(Buffer.concat(Array(count).fill(bytes))).digest('hex');
const server = http.createServer((req,res) => {
  if (req.url === '/error') { res.writeHead(503); res.end('fixture failure'); return; }
  if (req.url === '/stall') return;
  const known = req.url !== '/unknown';
  res.writeHead(200, known ? {'Content-Length':bytes.length*count} : {});
  let n=0;
  const timer = setInterval(() => { res.write(bytes); if (++n === count) { clearInterval(timer); res.end(); } }, 45);
  res.on('close',()=>clearInterval(timer));
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const url = `http://127.0.0.1:${server.address().port}`;
const make = (endpoint='/slow', script='echo "BP_INSTALL extract"; sleep 0.35; echo "BP_INSTALL configure"; sleep 0.35') => JSON.stringify({
  asset:'fixture.tar.gz',url:url+endpoint,sha256:digest,args:[],script,
});
const home = n => path.join(root,n);
async function collect(h, tool='kicad') {
  const samples=[];
  for(let i=0;i<40;i++) {
    const at=performance.now(),p=JSON.parse(await api.next(h,tool)); samples.push(p);
    assert(performance.now()-at>=230,'watcher must throttle snapshots');
    if(['ready','failed','cancelled','unavailable'].includes(p.phase)) return samples;
  }
  assert.fail('installation silently hung');
}
try {
  assert(api.start(home('slow'),'kicad',make()));
  assert.equal(api.start(home('slow'),'kicad',make()), '','duplicate clients must share a job');
  const progress=await collect(home('slow'));
  assert.equal(progress.at(-1).phase,'ready');
  const downloads=progress.filter(p=>p.phase==='download');
  assert(downloads.length>=2);
  assert(downloads.every(p=>p.total===bytes.length*count && p.speed>0 && p.known && p.eta>=0));
  assert(downloads.every((p,i)=>i===0 || p.done>=downloads[i-1].done));
  assert(downloads.some(p=>p.done>0 && p.done<p.total));
  assert(progress.some(p=>p.phase==='extract') && progress.some(p=>p.phase==='configure'));
  // A second client/reload gets the current job, with its same attempt id.
  assert.equal(JSON.parse(await api.next(home('slow'),'kicad')).attempt, progress[0].attempt);
  console.log('ok slow Content-Length download, rate/ETA, phases, duplicate/reload snapshot and throttle');
  api.start(home('error'),'kicad',make('/error'));
  const error=await collect(home('error')); assert.equal(error.at(-1).phase,'failed'); assert.match(error.at(-1).error,/HTTP 503/);
  const second = api.start(home('error'),'kicad',make());
  assert.equal(await api.next(home('error'),'kicad',error[0].attempt), '', 'old watcher is fenced from a retry');
  assert(second);
  const retry=await collect(home('error')); assert.equal(retry.at(-1).phase,'ready'); assert.notEqual(error[0].attempt,retry[0].attempt);
  console.log('ok HTTP failure is visible and Retry creates a new attempt');
  api.start(home('cancel'),'kicad',make());
  const before=JSON.parse(await api.next(home('cancel'),'kicad')); assert.equal(before.phase,'download');
  api.cancel(home('cancel'),'kicad'); const cancelled=await collect(home('cancel')); assert.equal(cancelled.at(-1).phase,'cancelled');
  assert(!fs.readdirSync(home('cancel')).some(n=>n.startsWith('.cad-install-')));
  api.start(home('cancel'),'kicad',make()); assert.equal((await collect(home('cancel'))).at(-1).phase,'ready');
  console.log('ok download Cancel aborts request, cleans staging and permits Retry');
  api.start(home('extract-cancel'),'kicad',make('/slow','echo "BP_INSTALL extract"; sleep 10; echo unexpected > escaped-child'));
  let phase=''; for(let i=0;i<10 && phase!=='extract';i++) phase=JSON.parse(await api.next(home('extract-cancel'),'kicad')).phase;
  assert.equal(phase,'extract'); api.cancel(home('extract-cancel'),'kicad');
  assert.equal((await collect(home('extract-cancel'))).at(-1).phase,'cancelled');
  assert(!fs.readdirSync(home('extract-cancel')).some(n=>n.startsWith('.cad-install-')));
  console.log('ok Cancel during extraction reaps the captured child tree');
  api.start(home('unknown'),'kicad',make('/unknown'));
  const unknown=await collect(home('unknown')); assert.equal(unknown.at(-1).phase,'ready');
  assert(unknown.filter(p=>p.phase==='download').every(p=>p.total===0 && !p.known));
  console.log('ok missing Content-Length stays indeterminate');
  const bad=JSON.parse(make());bad.sha256='0'.repeat(64);api.start(home('digest'),'kicad',JSON.stringify(bad));
  assert.match((await collect(home('digest'))).at(-1).error,/checksum mismatch/);
  console.log('ok invalid checksum cannot install');
  await compiledRun(home('compiled-ready'),make(),'ready');
  await compiledRun(home('compiled-error'),make('/error'),'failed');
  await compiledRun(home('compiled-cancel'),make(),'cancelled');
  console.log('ok compiled Bend async effect start/snapshot/error/cancel ABI');
  const timed=vm.runInNewContext(source+'\n({start:bp_cad_start,next:bp_cad_next_async})',{
    require,process,Buffer,fetch,AbortController,performance,clearTimeout,
    setTimeout:(fn,ms)=>setTimeout(fn,ms===30000?100:ms),
    CadFx:{start:1,next:2,cancel:3},CID:x=>x,io_eff(){},
  });
  timed.start(home('stalled'),'kicad',make('/stall'));
  const stalled=JSON.parse(await timed.next(home('stalled'),'kicad'));
  assert.equal(stalled.phase,'failed');assert.match(stalled.error,/stalled/);
  console.log('ok stalled HTTP headers terminate with actionable error');
} finally {
  server.closeAllConnections(); await new Promise(resolve=>server.close(resolve));
  fs.rmSync(root,{recursive:true,force:true});
}
