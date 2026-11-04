// Real Bun/fs component A/B. No hub, SQLite, credentials or existing home.
const fs = require('node:fs'), path = require('node:path'), os = require('node:os');
const assert = require('node:assert/strict'), { spawn } = require('node:child_process');
const { hostClock, assertDomain } = require('./real_store_active_clock.cjs');
const { load, invoke, assertDrained, sha } = require('./disk_sync_component.cjs');
const { transfer } = require('./disk_sync_transfer.cjs');
const clock = hostClock(), now = () => clock.nowMs();
const ROUNDS = 32, COMMITS = 128, SEND_BYTES = 262144;
const payload = Buffer.alloc(SEND_BYTES, 0x57);
const chunks = Array.from({ length: COMMITS }, (_, seq) => Buffer.from(JSON.stringify({ seq, text: '日本語 Δ', pad: 'x'.repeat(4000) }) + '\n'));
const expected = Buffer.concat(chunks), expectedHash = sha(expected);
const later = ms => new Promise(resolve => setTimeout(resolve, ms));
function deadline(promise, ms, label) {
  let timer;
  return Promise.race([promise, new Promise((_, reject) => { timer = setTimeout(() => reject(Error(label + ' timeout')), ms); })])
    .finally(() => clearTimeout(timer));
}
function resources() {
  const group = fs.readFileSync('/proc/self/cgroup', 'utf8').split('0::')[1].trim(), root = '/sys/fs/cgroup' + group;
  const read = name => fs.readFileSync(path.join(root, name), 'utf8').trim();
  const events = Object.fromEntries(read('memory.events').split('\n').map(s => s.split(' ')).map(([k, v]) => [k, Number(v)]));
  const result = { memoryMax: Number(read('memory.max')), swapMax: Number(read('memory.swap.max')),
    memoryPeak: Number(read('memory.peak')), swapCurrent: Number(read('memory.swap.current')), events };
  assert(result.memoryMax > 0 && result.memoryMax <= 3700000000 && result.swapMax === 0 && result.swapCurrent === 0);
  for (const key of ['oom', 'oom_kill', 'oom_group_kill']) assert.equal(events[key], 0);
  return result;
}
async function childMain() {
  const admission = resources(), root = fs.mkdtempSync(path.join(os.tmpdir(), 'bp-fsync-ab-')), file = path.join(root, 'events.jsonl');
  const host = load(); let busy = false, closed = false;
  fs.writeFileSync(path.join(root, 'component.js'), payload);
  const reuse = transfer({ web: root, file: 'component.js' }), server = reuse.server();
  const send = value => { assert(process.connected); process.send(value); };
  async function close() {
    if (closed) return; closed = true;
    server.listener.stop(true); await later(0);
    assert.equal(reuse.sockets.size, 0);
    assertDrained(host, (ROUNDS + 2) * COMMITS, (ROUNDS + 2) * 2 * COMMITS);
    for (const name of fs.readdirSync('/proc/self/fd')) {
      let target; try { target = fs.readlinkSync('/proc/self/fd/' + name); } catch { continue; }
      assert(!target.startsWith(root), 'component file descriptor leaked');
    }
    fs.rmSync(root, { recursive: true, force: true }); assert(!fs.existsSync(root));
    send({ event: 'closed', resources: resources(), tempClean: true, remainingSockets: reuse.sockets.size,
      pending: host.context.BEND_IO.pend, stats: host.stats }); process.disconnect();
  }
  process.on('message', message => {
    (async () => {
      assertDomain(clock, message.domain);
      if (message.kind === 'close') { assert(!busy); await close(); return; }
      if (message.kind === 'verify') {
        assert(!busy); const body = fs.readFileSync(file); assert.deepEqual(body, expected);
        assert.equal(sha(body), expectedHash); send({ event: 'verified', round: message.round, bytes: body.length, hash: expectedHash }); return;
      }
      assert.equal(message.kind, 'burst'); assert(!busy); busy = true;
      fs.writeFileSync(file, ''); const ackMs = [], beforeStats = { ...host.stats }, start = now();
      send({ event: 'entered', round: message.round, variant: message.variant, start });
      for (let i = 0; i < COMMITS; i++) {
        const at = now(); fs.appendFileSync(file, chunks[i]);
        assert.equal(await invoke(host, message.variant === 'before', file), 'ok'); ackMs.push(now() - at);
      }
      const end = now(); busy = false;
      assert.equal(host.context.BEND_IO.pend, 0);
      assert.equal(host.stats.duplicates, 0);
      const expectedParks = message.variant === 'before' ? 0 : COMMITS;
      assert.equal(host.stats.parks - beforeStats.parks, expectedParks);
      assert.equal(host.stats.resumes - beforeStats.resumes, expectedParks);
      assert.equal(host.stats.completions - beforeStats.completions, COMMITS);
      send({ event: 'done', round: message.round, start, end, ackMs, parkDelta: expectedParks, resources: resources() });
    })().catch(error => { console.error(error.stack); process.exitCode = 1; process.disconnect(); });
  });
  send({ event: 'ready', httpPort: server.port, pid: process.pid,
    domain: clock.contract.domain, clock: clock.contract, identity: host.identity, transferIdentity: reuse.identity, admission });
}
function stats(values) {
  assert(values.length); const sorted = [...values].sort((a, b) => a - b);
  assert(sorted.every(x => Number.isFinite(x) && x >= 0));
  const q = p => sorted[Math.ceil(p * sorted.length) - 1];
  return { n: sorted.length, min: sorted[0], median: q(.5), p95: q(.95), p99: q(.99), max: sorted.at(-1) };
}
async function parentMain() {
  assert.equal(process.argv[2], '--out'); const out = process.argv[3]; assert(out);
  const admission = resources(), child = spawn(process.execPath, [__filename, '--child'], { stdio: ['ignore', 'pipe', 'pipe', 'ipc'],
    env: { PATH: path.dirname(process.execPath), HOME: os.tmpdir(), BACKPLANE_TELEMETRY: 'off', DISK_SYNC_TRANSFER_HARNESS: process.env.DISK_SYNC_TRANSFER_HARNESS } });
  const waiters = new Map(), queued = new Map(), output = { stdout: '', stderr: '' }; let exitCode, signal, closed;
  const exit = new Promise(resolve => child.once('exit', (code, sig) => { exitCode = code; signal = sig; resolve(); }));
  child.once('error', error => { for (const waiter of waiters.values()) waiter.reject(error); });
  child.on('message', message => {
    const key = message.event + ':' + (message.round ?? '');
    if (waiters.has(key)) { const w = waiters.get(key); waiters.delete(key); w.resolve(message); }
    else { assert(!queued.has(key)); queued.set(key, message); }
  });
  for (const stream of ['stdout', 'stderr']) child[stream].on('data', b => {
    output[stream] += b.toString('utf8'); if (output[stream].length > 16384) child.kill('SIGTERM');
  });
  child.once('exit', () => { for (const waiter of waiters.values()) waiter.reject(Error('component child exited')); });
  const get = (event, round = '') => deadline(new Promise((resolve, reject) => {
    const key = event + ':' + round;
    if (queued.has(key)) { const value = queued.get(key); queued.delete(key); resolve(value); }
    else waiters.set(key, { resolve, reject });
  }), 20000, event).finally(() => waiters.delete(event + ':' + round));
  try {
    const ready = await get('ready'); assert.equal(ready.pid, child.pid); assertDomain(clock, ready.domain);
    const reuse = transfer(); assert.deepEqual(reuse.identity, ready.transferIdentity);
    const probe = async route => {
      const start = now(), pending = reuse.get(ready.httpPort, route), issued = now();
      const response = await pending, end = now();
      assert.equal(response.status, 200); assert.equal(response.coding, 'identity');
      assert.deepEqual(response.body, route === '/hello' ? Buffer.from('ok') : payload);
      return { start, issued, end, ms: response.ms };
    };
    const hello = () => probe('/hello');
    await hello(); const samples = [], warmups = []; let roundNumber = 0;
    for (let pair = -2; pair < ROUNDS; pair++) {
      for (const variant of pair % 2 ? ['after', 'before'] : ['before', 'after']) {
        const round = roundNumber++, idle = await hello();
        child.send({ kind: 'burst', variant, round, domain: clock.contract.domain });
        const entered = await get('entered', round); assert.equal(entered.variant, variant);
        const helloPromise = hello(), socketPromise = probe('/component.js');
        const [h, s, done] = await Promise.all([helloPromise, socketPromise, get('done', round)]);
        assert.equal(done.ackMs.length, COMMITS); assert.equal(done.start, entered.start);
        assert(h.start >= done.start && s.start >= done.start, 'probe preceded burst');
        assert(h.issued < done.end && s.issued < done.end, 'probe missed active fsync burst');
        child.send({ kind: 'verify', round, domain: clock.contract.domain }); const verified = await get('verified', round);
        assert.equal(verified.hash, expectedHash); assert.equal(verified.bytes, expected.length);
        (pair < 0 ? warmups : samples).push({ pair, round, variant, helloMs: h.end - h.start, socketSendMs: s.end - s.start,
          helloStartOffsetMs: h.start - done.start, socketStartOffsetMs: s.start - done.start,
          helloIssuedOffsetMs: h.issued - done.start, socketIssuedOffsetMs: s.issued - done.start,
          helloFinishedDuringBurst: h.end < done.end, socketFinishedDuringBurst: s.end < done.end,
          idleHelloMs: idle.end - idle.start, burstMs: done.end - done.start, ackMs: done.ackMs,
          fileBytes: verified.bytes, fileHash: verified.hash, parkDelta: done.parkDelta, resources: done.resources });
      }
    }
    child.send({ kind: 'close', domain: clock.contract.domain }); closed = await get('closed');
    await deadline(exit, 5000, 'child exit'); assert.equal(exitCode, 0); assert.equal(signal, null);
    assert.equal(closed.pending, 0); assert.equal(closed.remainingSockets, 0); assert(closed.tempClean);
    assert.equal(queued.size, 0); assert.equal(waiters.size, 0); assert.equal(output.stderr, ''); assert.equal(output.stdout, '');
    const summary = {};
    for (const variant of ['before', 'after']) {
      const rows = samples.filter(r => r.variant === variant); assert.equal(rows.length, ROUNDS);
      assert.equal(rows.reduce((n, r) => n + r.parkDelta, 0), variant === 'after' ? ROUNDS * COMMITS : 0);
      summary[variant] = { helloMs: stats(rows.map(r => r.helloMs)), socketSendMs: stats(rows.map(r => r.socketSendMs)),
        commitAckMs: stats(rows.flatMap(r => r.ackMs)), burstMs: stats(rows.map(r => r.burstMs)),
        idleHelloMs: stats(rows.map(r => r.idleHelloMs)),
        helloFinishedDuringBurst: rows.filter(r => r.helloFinishedDuringBurst).length,
        socketFinishedDuringBurst: rows.filter(r => r.socketFinishedDuringBurst).length };
    }
    const report = { state: 'passed', kind: 'real-fsync component only', sourceHead: process.env.DISK_SYNC_SOURCE_HEAD,
      identity: ready.identity, transferIdentity: ready.transferIdentity, clock: ready.clock, roundsPerVariant: ROUNDS, commitsPerBurst: COMMITS,
      socketBytesPerRound: SEND_BYTES, expectedFileBytes: expected.length, expectedFileHash: expectedHash,
      allProbesStartedDuringBurst: true, byteAndOrderChecks: true, artificialFsyncDelay: false,
      summary, samples, warmups, admission, finalResources: resources(), childAdmission: ready.admission, childCleanup: closed,
      childExited: true, fullHub: false, sqliteCommit: false, packagedAcceptance: false,
      limitation: 'real append+Disk.sync ACK only; Hub.loop still awaits current handler before next queued view/RPC' };
    fs.mkdirSync(path.dirname(out), { recursive: true }); fs.writeFileSync(out, JSON.stringify(report, null, 2) + '\n');
    console.log(JSON.stringify({ state: 'passed', summary, roundsPerVariant: ROUNDS, commitsPerBurst: COMMITS }));
    console.log('DISK_SYNC_COMPONENT_AB_PASS rounds=32+32 bytes=checked order=checked child=closed');
  } finally {
    if (exitCode === undefined) { child.kill('SIGTERM'); try { await deadline(exit, 5000, 'cleanup'); } catch { child.kill('SIGKILL'); await deadline(exit, 5000, 'forced cleanup'); } }
  }
}
(process.argv[2] === '--child' ? childMain() : parentMain()).catch(error => { console.error(error.stack); process.exitCode = 1; });
