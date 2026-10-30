// bun test/tools/viewer_revision_state_test.mjs BUILD/viewer_revision_state_probe
import assert from 'node:assert/strict';
import {mkdtempSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {resolve, join} from 'node:path';
import {spawnSync} from 'node:child_process';
const binary = resolve(process.argv[2] || 'build/viewer_revision_state_probe');
function cbor(bytes) {
  let i = 0;
  const item = () => {
    const h = bytes[i++], major = h >> 5;let n = h & 31;
    if (n >= 24) { const width = 2 ** (n - 24);n = 0;for (let k = 0; k < width; k++) n = n * 256 + bytes[i++]; }
    if (major === 0) return n;
    if (major === 1) return -1 - n;
    if (major === 2 || major === 3) { const b = bytes.subarray(i, i += n);return major === 2 ? b : b.toString(); }
    if (major === 4) return Array.from({length:n}, item);
    if (major === 5) { const out = {};for (let k = 0; k < n; k++) { const key = item();out[key === 1 ? 't' : key === 31 ? 'key' : key] = item(); }return out; }
    throw Error(`unsupported CBOR ${major}`);
  };
  const result = item();assert.equal(i, bytes.length);return result;
}
const dir = mkdtempSync(join(tmpdir(), 'bp-viewer-revision-state-'));
try {
  const ran = spawnSync(binary, [], {encoding:'utf8', timeout:15000, env:{...process.env, BACKPLANE_HOME:join(dir,'home'), XDG_CACHE_HOME:join(dir,'cache'), BACKPLANE_TELEMETRY:'off'}});
  assert.equal(ran.status, 0, ran.stderr || ran.stdout);
  const rows = ran.stdout.trim().split('\n').filter(Boolean).map(line => JSON.parse(line));
  const stage = name => rows.filter(row => row.stage === name);
  const state = name => stage(name).find(row => row.kind === 'state');
  const revisions = name => stage(name).filter(row => row.kind === 'text').map(row => JSON.parse(row.text)).filter(row => row.t === 'plotrevision');
  const geometry = name => stage(name).filter(row => row.kind === 'binary').map(row => cbor(Buffer.from(row.hex, 'hex')));
  const A = [11,22,33,44], D = [55,66,77,88];
  assert.equal(state('success-A').good, 'revision-A');
  for (const name of ['first-subscriber','cached-after-failure','cached-after-half-written']) {
    assert.deepEqual(geometry(name).map(row => row.box), [A], `${name}: cached geometry changed`);
    assert.equal(geometry(name)[0].cs.length, 1);
    assert.ok(geometry(name)[0].cs[0].d.length > 0, 'fixture must contain actual encoded geometry');
    assert.deepEqual(revisions(name), [{t:'plotrevision', key:'0|review|board', revision:'revision-A'}], `${name}: reported the failed/observed revision instead of the good geometry revision`);
    assert.equal(state(name).good, 'revision-A');
  }
  assert.equal(state('failure-B').observed, 'revision-B');assert.equal(state('failure-B').good, 'revision-A');
  assert.equal(state('half-written-C').observed, 'revision-C');assert.equal(state('half-written-C').good, 'revision-A');
  assert.deepEqual(revisions('failure-B'), []);assert.deepEqual(revisions('half-written-C'), []);
  assert.deepEqual(geometry('failure-B'), []);assert.deepEqual(geometry('half-written-C'), []);
  for (const [name, clients] of [['success-D',[7,8,9]],['cached-after-new-success',[10]]]) {
    assert.deepEqual(geometry(name).map(row => row.box), clients.map(() => D));
    assert.deepEqual(revisions(name), clients.map(() => ({t:'plotrevision', key:'0|review|board', revision:'revision-D'})));
    assert.deepEqual(stage(name).filter(row => row.kind === 'binary').map(row => row.client).sort(), clients);
    assert.equal(state(name).good, 'revision-D');
  }
  for (const name of ['changed-revision','changed-size']) {
    const row = stage(name)[0];assert.equal(row.kind, 'checked');assert.equal(row.box, null, `${name}: cross-save result published`);assert.equal(row.error, '');
  }
  assert.deepEqual(stage('stable-success')[0].box, D);
  assert.equal(stage('stable-failure')[0].box, null);assert.match(stage('stable-failure')[0].error, /export failed/);
  assert.equal(stage('stable-half-written')[0].box, null);assert.equal(stage('stable-half-written')[0].error, '');
  console.log('ok actual plot lifecycle: A success → B failure/partial keeps A geometry and cached-subscriber revision; success-only readiness; new success advances both; before/after size or revision change discards exported geometry');
} finally { rmSync(dir, {recursive:true, force:true}); }
