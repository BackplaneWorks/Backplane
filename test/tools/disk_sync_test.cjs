const assert = require('node:assert/strict'), fs = require('node:fs'), os = require('node:os'), path = require('node:path');
const { load, invoke, assertDrained } = require('./disk_sync_component.cjs');
const turn = () => new Promise(resolve => setImmediate(resolve));
let cases = 0;
async function test(name, fn) { await fn(); cases++; console.log('ok ' + name); }
function fake(failure = '') {
  const order = [], open = new Set(); let flags;
  const io = { promises: {
    async open(_, mode) {
      order.push('open'); flags = mode; if (failure === 'open') throw Error('open refused');
      const handle = { async sync() { order.push('fsync'); if (failure === 'sync') throw Error('sync refused'); },
        async close() { order.push('close'); assert(open.delete(handle)); if (failure === 'close') throw Error('close reported failure'); } };
      open.add(handle); return handle;
    }
  } };
  for (const key of ['openSync', 'fsyncSync', 'closeSync']) io[key] = () => { throw Error('unexpected blocking fallback'); };
  return { io, order, open, flags: () => flags };
}
(async () => {
  for (const delayed of [false, true]) await test('duplicate completion negative control delayed=' + delayed, async () => {
    const h = load(); h.effects['Disk.sync'] = (_, finish) => {
      finish('ok'); if (delayed) setImmediate(() => finish('again')); else finish('again');
    };
    assert.equal(await invoke(h, false, 'component'), 'ok'); await turn();
    assert.equal(h.stats.duplicates, 1); assert.throws(() => assertDrained(h, 0, 1), /duplicate completion/);
  });
  await test('waits for async open fsync close before exactly one resume', async () => {
    const gates = {}, order = [], io = { promises: { open: () => new Promise(resolve => { gates.open = () => resolve({
      sync: () => new Promise(r => { order.push('sync'); gates.sync = r; }),
      close: () => new Promise(r => { order.push('close'); gates.close = r; }) }); }) } };
    const h = load({ io }); let done = 0, result;
    assert.equal(h.effects['Disk.sync']('component', v => { done++; result = v; }), undefined);
    assert.equal(done, 0); gates.open(); await turn(); assert.deepEqual(order, ['sync']); assert.equal(done, 0);
    gates.sync(); await turn(); assert.deepEqual(order, ['sync', 'close']); assert.equal(done, 0);
    gates.close(); await turn(); await turn(); assert.equal(done, 1); assert.equal(result, 'ok');
    assert.equal(h.stats.resumes, 1, 'resume count'); assert.equal(h.stats.parks, 1, 'park count'); assert.equal(h.context.BEND_IO.pend, 0, 'pending count');
  });
  for (const failure of ['', 'open', 'sync', 'close']) await test('async lifecycle ' + (failure || 'success'), async () => {
    const f = fake(failure), h = load({ io: f.io }), answer = await invoke(h, false, 'component');
    assert.equal(answer, failure ? 'fail ' + (failure === 'close' ? 'close reported failure' : failure + ' refused') : 'ok');
    assert.deepEqual(f.order, failure === 'open' ? ['open'] : ['open', 'fsync', 'close']);
    assert.equal(f.open.size, 0); assert.equal(h.stats.resumes, 1); assert.equal(h.context.BEND_IO.pend, 0);
  });
  for (const platform of ['linux', 'darwin', 'win32']) await test('async handle policy ' + platform, async () => {
    const f = fake(), h = load({ io: f.io, platform }); assert.equal(await invoke(h, false, 'component'), 'ok');
    assert.equal(f.flags(), platform === 'win32' ? 'r+' : 'r');
  });
  for (const options of [{ asyncMode: false }, { mode: 'sync' }]) {
    for (const platform of ['linux', 'win32']) await test('explicit sync mode ' + JSON.stringify(options) + ' ' + platform, async () => {
      const order = [], io = { openSync: (_, flags) => { order.push(flags); return 7; },
        fsyncSync: fd => { assert.equal(fd, 7); order.push('sync'); }, closeSync: fd => { assert.equal(fd, 7); order.push('close'); },
        promises: { open() { throw Error('async path in sync mode'); } } };
      const h = load({ io, platform, ...options }); assert.equal(await invoke(h, false, 'component'), 'ok');
      assert.deepEqual(order, [platform === 'win32' ? 'r+' : 'r', 'sync', 'close']); assert.equal(h.stats.parks, 0);
    });
  }
  await test('missing async bridge refuses before open with no blocking fallback', async () => {
    const f = fake(), h = load({ io: f.io, bridge: false });
    assert.equal(await invoke(h, false, 'component'), 'fail async IO loop unavailable'); assert.deepEqual(f.order, []);
  });
  await test('explicit sync fsync error closes and returns failure', async () => {
    let closed = 0; const h = load({ asyncMode: false, io: { openSync: () => 9,
      fsyncSync: () => { throw Error('sync refused'); }, closeSync: () => closed++ } });
    assert.equal(await invoke(h, false, 'component'), 'fail sync refused'); assert.equal(closed, 1);
  });
  await test('explicit sync close exception returns failure', async () => {
    const h = load({ asyncMode: false, io: { openSync: () => 9, fsyncSync() {}, closeSync() { throw Error('close refused'); } } });
    assert.equal(await invoke(h, false, 'component'), 'fail close refused');
  });
  await test('internal direct sync helper remains blocking even in async mode', () => {
    const order = [], h = load({ io: { openSync: () => 8, fsyncSync: () => order.push('sync'), closeSync: () => order.push('close') } });
    assert.equal(h.context.store_sync('component'), 'ok'); assert.deepEqual(order, ['sync', 'close']); assert.equal(h.stats.parks, 0);
  });
  await test('real sequential durable prefixes preserve Unicode bytes order and descriptors', async () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'bp-disk-sync-')), file = path.join(root, 'events.jsonl');
    try {
      fs.writeFileSync(file, ''); const h = load(); let expected = '';
      for (let i = 0; i < 32; i++) {
        const line = JSON.stringify({ seq: i, text: '日本語 Δ ' + i }) + '\n'; expected += line;
        fs.appendFileSync(file, line); assert.equal(await invoke(h, false, file), 'ok');
        assert.deepEqual(fs.readFileSync(file), Buffer.from(expected));
      }
      assert.equal(h.stats.resumes, 32); assert.equal(h.context.BEND_IO.pend, 0);
      const f = await fs.promises.open(file, 'r+'); await f.close();
      assert.match(await invoke(h, false, path.join(root, 'missing')), /^fail /);
      assertDrained(h, 33, 33);
      if (process.platform === 'linux') for (const name of fs.readdirSync('/proc/self/fd')) {
        let target; try { target = fs.readlinkSync('/proc/self/fd/' + name); } catch { continue; }
        assert(!target.startsWith(root), 'owned descriptor leaked');
      }
    } finally { fs.rmSync(root, { recursive: true, force: true }); }
  });
  assert.equal(cases, 19); console.log('DISK_SYNC_BOUNDARY_PASS cases=' + cases);
})().catch(e => { console.error(e.stack); process.exitCode = 1; });
