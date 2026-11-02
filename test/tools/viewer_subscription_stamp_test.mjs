// bun test/tools/viewer_subscription_stamp_test.mjs BUILD/viewer_subscription_stamp_probe
import assert from 'node:assert/strict';
import {mkdtempSync,writeFileSync,rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {resolve,join} from 'node:path';
import {spawnSync} from 'node:child_process';
const dir=mkdtempSync(join(tmpdir(),'bp-subscription-stamp-'));
try {
 const path=join(dir,'board.kicad_pcb');writeFileSync(path,'saved source');
 const binary=resolve(process.argv[2]);
 function run(file){
  const r=spawnSync(binary,[file],{encoding:'utf8',timeout:15000,env:{...process.env,BACKPLANE_HOME:join(dir,'home'),XDG_CACHE_HOME:join(dir,'cache')}});
  assert.equal(r.status,0,r.stderr||r.stdout);return r.stdout.trim().split('\n').filter(Boolean).map(JSON.parse);
 }
 const rows=run(path);
 for(const stage of ['join-busy','repeat-watch']){
  const messages=rows.filter(r=>r.stage===stage&&r.kind==='text');
  assert.equal(messages.length,2,'a subscriber gets both stamps before any compact mesh completes');
  assert.deepEqual(messages.map(r=>r.client),[7,7],'no replay to other clients');
  const stamps=messages.map(r=>JSON.parse(r.text));
  assert(stamps.every(r=>r.t==='plotstamp'&&r.revision&&r.revision!=='earlier-save'));
  assert.deepEqual(stamps.map(r=>r.key).sort(),[`0|/fixture|${path}`,`2|/fixture|${path}`]);
  assert.equal(stamps[0].revision,stamps[1].revision);
  const state=rows.find(r=>r.stage===stage&&r.kind==='state');
  assert.equal(state.busy,true);assert.equal(state.good,'');assert.equal(state.box,null,'stamp must not assert plot readiness');
 }
 assert.equal(rows.filter(r=>r.stage==='legacy'&&r.kind==='text').length,0,'legacy client did not opt in');
 assert.equal(rows.filter(r=>r.kind==='binary').length,0,'no geometry was published');
 assert.equal(run(join(dir,'missing.kicad_pcb')).filter(r=>r.kind==='text').length,0,'no revision invented for missing files');
 console.log('ok subscription stamps: busy join, repeated watch, exact source, client isolation, legacy opt-out, missing source; no false plot readiness');
} finally {rmSync(dir,{recursive:true,force:true});}
