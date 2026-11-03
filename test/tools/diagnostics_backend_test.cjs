// Production effects and real temporary directories; no running hub or user
// data. The fault layer injects filesystem failures at the syscall boundary.
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const vm = require('node:vm');
const root = path.resolve(__dirname, '../..');
const diagnostics = fs.readFileSync(path.join(root, 'src/server/effects/diagnostics.js'), 'utf8');
const perf = fs.readFileSync(path.join(root, 'src/server/effects/perf.js'), 'utf8');
const CAP = 1048576, SEGMENT = 65536, AGE = 7 * 86400;
const methods = ['start', 'status', 'tick', 'flush', 'event', 'control', 'export'];
const perfMethods = ['us', 'span', 'add', 'set', 'gauge', 'mark', 'done', 'take', 'emit', 'logs', 'severity'];
function fixture(t) {
  const home = fs.mkdtempSync(path.join(os.tmpdir(), 'bp-diagnostics-'));
  t.after(() => fs.rmSync(home, { recursive: true, force: true }));
  return home;
}
function host(home, io = fs, initial = 1800000000000) {
  const clock = { wall: initial, mono: 0 }, effects = {};
  const context = vm.createContext({
    require: n => n === 'fs' ? io : require(n), Buffer,
    process: { env: home === undefined ? {} : { BACKPLANE_HOME: home }, platform: process.platform,
      memoryUsage: () => ({ rss: 0 }), resourceUsage: () => ({ maxRSS: 0, userCPUTime: 0, systemCPUTime: 0 }) },
    performance: { now: () => clock.mono }, Date: { now: () => clock.wall },
    Diagnostics: Object.fromEntries(methods.map(n => [n, 'diagnostics.' + n])),
    Perf: Object.fromEntries(perfMethods.map(n => [n, 'perf.' + n])),
    CID: x => x, io_eff: (id, fn) => effects[id] = fn,
    io_bytes: s => Buffer.from(s), io_out() {},
  });
  vm.runInContext(diagnostics, context);
  vm.runInContext(perf, context);
  const h = {
    clock, context, call: (n, ...args) => effects['diagnostics.' + n](...args),
    p: (n, ...args) => effects['perf.' + n](...args),
    advance(ms) { clock.wall += ms; clock.mono += ms; },
    json(n, ...args) { return JSON.parse(this.call(n, ...args)); },
  };
  return h;
}
function usage(home) {
  const dir = path.join(home, 'diagnostics'), names = fs.readdirSync(dir);
  const stats = names.map(n => fs.lstatSync(path.join(dir, n)));
  return {
    names, bytes: stats.reduce((n, s) => n + s.size, 0),
    allocated: stats.reduce((n, s) => n + Math.max(s.size, s.blocks * 512), 0),
  };
}
function faultIO() {
  const faults = {}, calls = {}, descriptors = new Map();
  const io = new Proxy(fs, { get(target, key) {
    if (typeof target[key] !== 'function') return target[key];
    return (...args) => {
      calls[key] = (calls[key] || 0) + 1;
      if (faults[key]) return faults[key](...args);
      const result = target[key](...args);
      if (key === 'openSync') descriptors.set(result, args[0]);
      if (key === 'closeSync') descriptors.delete(args[0]);
      return result;
    };
  }});
  return { io, faults, calls, descriptors };
}
function full(code = 'ENOSPC') { return Object.assign(new Error('SECRET /private/home 192.0.2.4 person@example.invalid'), { code }); }
function segment(home, i = 0) { return path.join(home, 'diagnostics', `segment-${i}.jsonl`); }
function row(h, seq = 1, at = Math.floor(h.clock.wall / 1000)) {
  return { v: 1, seq, at, event: 'latency', name: 'hub.rpc', count: 1, us: 12000, maxMs: 12 };
}
function seed(h, home, i, rows, tail = '') {
  fs.writeFileSync(segment(home, i), rows.map(r => JSON.stringify(r) + '\n').join('') + tail);
}

test('production Perf.span/mark/done/add/set and severity feed closed measurements', t => {
  const home = fixture(t), h = host(home);
  assert.equal(h.json('start').enabled, true);
  const t0 = h.p('us'); h.advance(12); h.p('span', 'hub.rpc', t0);
  h.p('mark', 'hub.rpc'); h.advance(20); h.p('done', 'hub.rpc'); h.p('done', 'hub.rpc');
  const j0 = h.p('us'); h.advance(2); h.p('span', 'hub.join', j0);
  h.p('add', 'hub.write_fail', 2); h.p('add', 'mail.append_fail', 1);
  h.p('add', 'hub.client_drop', 3); h.p('set', 'hub.clients', 4);
  h.p('severity', 0); h.p('severity', 1); h.p('severity', 2);
  const bundle = h.json('export'), events = bundle.events;
  const rpc = events.find(r => r.name === 'hub.rpc');
  assert.deepEqual(rpc, { v: 1, at: 1800000000, event: 'latency', name: 'hub.rpc', count: 2, us: 32000, maxMs: 20, meanMs: 16 });
  assert.equal(events.find(r => r.name === 'connection.join').count, 1);
  assert.equal(events.find(r => r.name === 'connection.drop').count, 3);
  assert.equal(events.find(r => r.code === 'store_write').count, 2);
  assert.equal(events.find(r => r.code === 'mail_append').count, 1);
  assert.equal(events.find(r => r.code === 'log_error').count, 1);
  assert.equal(events.find(r => r.code === 'log_warn').count, 1);
  assert.equal(events.find(r => r.event === 'sample').value, 4);
  assert.equal(h.p('take').split('\n').find(r => r.startsWith('hub.rpc\t')).split('\t')[2], '2');
  assert.equal(bundle.status.stored_bytes, usage(home).bytes);
  assert.equal(bundle.status.allocated_bytes, usage(home).allocated);
});

test('unsupported names, text, paths, identity, IPs, invalid numbers and logs never enter the store/export', t => {
  const home = fixture(t), h = host(home); h.call('start');
  const secret = 'SECRET /private/home/alice 192.0.2.1 alice@example.invalid token=abc';
  h.p('emit', secret);
  for (const name of [secret, 'hub.rpc.' + secret, 'hub.out.' + secret, 'hub.rpc\n' + secret]) {
    h.p('span', name, 0); h.p('add', name, 1); h.p('set', name, 1);
    h.call('event', 'latency', name, 1); h.call('event', 'error', name, 1);
  }
  for (const n of [NaN, Infinity, -1, 0.1, 4294967296, '12', { text: secret }]) h.call('event', 'latency', 'hub.rpc', n);
  h.call('event', secret, 'hub.rpc', 1);
  assert.equal(h.json('export').events.length, 0);
  const output = h.call('export') + fs.readFileSync(path.join(home, 'diagnostics/control'));
  assert(!output.includes('SECRET')); assert(!output.includes('192.0.2.1')); assert(!output.includes(home));
  assert.equal(usage(home).names.join(','), 'control');
});

test('event flood aggregates with bounded keys and zero filesystem calls on the measurement path', t => {
  const home = fixture(t), f = faultIO(), h = host(home, f.io); h.call('start');
  const before = JSON.stringify(f.calls);
  for (let i = 0; i < 100000; i++) {
    h.call('event', 'latency', 'hub.rpc', 25000);
    h.call('event', 'connection', 'reconnect', 1);
    h.call('event', 'error', 'arbitrary.' + i, 1);
  }
  assert.equal(JSON.stringify(f.calls), before);
  assert.equal(h.json('status').pending, 2);
  const events = h.json('export').events;
  assert.equal(events.find(r => r.name === 'hub.rpc').count, 100000);
  assert.equal(events.find(r => r.name === 'hub.rpc').us, 2500000000);
  assert.equal(events.find(r => r.name === 'connection.reconnect').count, 100000);
});

test('periodic tick flushes only when due, including idle retention; no timers required', t => {
  const home = fixture(t), f = faultIO(), h = host(home, f.io); h.call('start');
  h.call('event', 'latency', 'hub.rpc', 10);
  const before = JSON.stringify(f.calls);
  for (let i = 0; i < 29; i++) { h.advance(1000); h.call('tick'); }
  assert.equal(JSON.stringify(f.calls), before);
  h.advance(1000); h.call('tick');
  assert.equal(h.json('status').pending, 0); assert(usage(home).bytes > 1);
  h.advance((AGE + 1) * 1000); h.call('tick');
  assert.equal(usage(home).bytes, 1);
  const idle = JSON.stringify(f.calls); h.call('tick'); assert.equal(JSON.stringify(f.calls), idle);
});

test('fixed slots rotate oldest data and keep the measured TOTAL byte/block budget across restart', t => {
  const home = fixture(t); let h = host(home); h.call('start');
  // Seed all eight real files up to their normal boundary, then rotate via
  // real effect appends. Avoid thousands of redundant full-store scans.
  let seq = 1;
  for (let i = 0; i < 8; i++) {
    const rows = []; let bytes = 0;
    for (;;) {
      const r = { ...row(h, seq), us: seq === 1 ? 1234567 : 99, maxMs: seq === 1 ? 1234.567 : 0.099 };
      const n = Buffer.byteLength(JSON.stringify(r) + '\n');
      if (bytes + n > SEGMENT - 16) break;
      rows.push(r); bytes += n; seq++;
    }
    seed(h, home, i, rows);
  }
  h = host(home); assert.equal(h.json('start').ok, true);
  const oldestBefore = fs.readFileSync(segment(home));
  for (let i = 0; i < 240; i++) {
    for (const name of ['hub.rpc', 'hub.mcp', 'hub.step.commit', 'hub.agent_line']) h.call('event', 'latency', name, 99);
    assert.equal(h.json('flush').ok, true);
    const u = usage(home);
    assert(u.names.length <= 9); assert(u.bytes <= CAP); assert(u.allocated <= CAP);
    if (i === 120) { h = host(home, fs, h.clock.wall); assert.equal(h.json('start').ok, true); }
  }
  const u = usage(home), events = h.json('export').events;
  assert.equal(u.names.length, 9);
  assert(!fs.readFileSync(segment(home)).equals(oldestBefore));
  assert(!events.some(r => r.us === 1234567));
  assert(events.every(r => r.us === 99));
  assert.equal(h.json('status').stored_bytes, usage(home).bytes);
  for (const n of u.names.filter(n => n !== 'control')) assert(fs.statSync(path.join(home, 'diagnostics', n)).size <= SEGMENT);
});

test('restart repairs a torn tail, retains earlier measurements and appends a fresh sequence', t => {
  const home = fixture(t); let h = host(home); h.call('start');
  seed(h, home, 0, [row(h)], '{"prompt":"SECRET');
  h = host(home); assert.equal(h.json('start').ok, true);
  h.call('event', 'latency', 'hub.rpc', 10); h.call('flush');
  const disk = fs.readFileSync(segment(home), 'utf8').trim().split('\n').map(JSON.parse);
  assert.deepEqual(disk.map(r => r.seq), [1, 2]);
  assert(!h.call('export').includes('SECRET'));
});

test('recovery/export revalidate disk records and prune old/future/corrupt/oversized segments', t => {
  const home = fixture(t); let h = host(home); h.call('start');
  seed(h, home, 0, [{ ...row(h, 1), prompt: 'SECRET', path: '/private/home', ip: '192.0.2.1' }]);
  seed(h, home, 1, [row(h, 2, Math.floor(h.clock.wall / 1000) - AGE - 1)]);
  seed(h, home, 2, [row(h, 3, Math.floor(h.clock.wall / 1000) + 1)]);
  seed(h, home, 3, [{ ...row(h, 4), name: '/private/SECRET' }]);
  fs.writeFileSync(segment(home, 4), 'X'.repeat(SEGMENT + 1));
  h = host(home); h.call('start');
  const bundle = h.call('export'), events = JSON.parse(bundle).events;
  assert.equal(events.length, 1); assert.equal(events[0].name, 'hub.rpc');
  assert(!/SECRET|private|192\.0\.2/.test(bundle));
  assert.deepEqual(usage(home).names.sort(), ['control', 'segment-0.jsonl']);
  // Tamper AFTER start too: export must revalidate, not reuse a trusted cache.
  seed(h, home, 0, [{ ...row(h), event: 'prompt', text: 'SECRET' }]);
  assert.equal(h.json('export').events.length, 0);
});

test('pending measurements expire too and wall-clock rollback never exports future records', t => {
  const home = fixture(t), h = host(home); h.call('start');
  h.call('event', 'latency', 'hub.rpc', 10); h.advance((AGE + 1) * 1000);
  assert.equal(h.json('export').events.length, 0);
  h.call('event', 'latency', 'hub.rpc', 10); h.call('flush');
  h.clock.wall -= 86400000;
  assert.equal(h.json('export').events.length, 0);
});

test('off/on/clear are closed durable controls; off and clear discard pending data', t => {
  const home = fixture(t); let h = host(home); h.call('start');
  h.call('event', 'latency', 'hub.rpc', 12); h.call('flush');
  h.call('event', 'latency', 'hub.mcp', 12);
  assert.equal(h.json('control', 'off').enabled, false);
  h.call('event', 'latency', 'hub.mcp', 12);
  assert.equal(h.json('status').pending, 0);
  h = host(home); assert.equal(h.json('start').enabled, false);
  assert.equal(h.json('export').events.length, 1);
  assert.equal(h.json('control', 'clear').enabled, false);
  assert.equal(h.json('export').events.length, 0); assert.equal(usage(home).bytes, 1);
  assert.equal(h.json('control', 'on').enabled, true);
  h.call('event', 'connection', 'reconnect', 1);
  assert.equal(h.json('export').events[0].name, 'connection.reconnect');
  assert.equal(h.json('control', '/private/SECRET').error, 'invalid_action');
  assert(!h.call('control', '/private/SECRET').includes('SECRET'));
  h.call('event', 'error', 'timeout', 1); h.call('control', 'clear');
  assert.equal(h.json('export').events.length, 0);
});

test('only BACKPLANE_HOME is used, binding survives environment changes and double start is idempotent', t => {
  const home = fixture(t), other = fixture(t), h = host(home);
  h.call('start'); h.call('event', 'latency', 'hub.rpc', 1);
  h.context.process.env.BACKPLANE_HOME = other;
  assert.equal(h.json('start').pending, 1); h.call('flush');
  assert(!fs.existsSync(path.join(other, 'diagnostics')));
  assert.equal(h.json('export').events.length, 1);
  for (const value of [undefined, '', 'relative', '/bad\0path']) {
    const bad = host(value); assert.equal(bad.json('start').enabled, false);
    assert.equal(bad.json('export').events.length, 0);
  }
});

test('ENOSPC/EDQUOT and short append roll back, drop pending data, back off and resume without crashing', t => {
  for (const mode of ['ENOSPC', 'EDQUOT', 'short']) {
    const home = fixture(t), f = faultIO(), h = host(home, f.io); h.call('start');
    h.call('event', 'latency', 'hub.rpc', 10); h.call('flush');
    const before = fs.readFileSync(segment(home));
    f.faults.writeSync = (...args) => {
      if (mode !== 'short') throw full(mode);
      return fs.writeSync(args[0], args[1], args[2], 3, args[4]);
    };
    h.call('event', 'latency', 'hub.mcp', 30);
    const result = h.json('flush');
    assert.equal(result.ok, false); assert.equal(result.error, mode === 'short' ? 'io' : 'full');
    assert.equal(result.pending, 0); assert.equal(result.dropped, 1);
    assert(fs.readFileSync(segment(home)).equals(before));
    assert(!JSON.stringify(result).includes('SECRET'));
    const calls = JSON.stringify(f.calls);
    for (let i = 0; i < 50; i++) { h.call('event', 'latency', 'hub.rpc', 10); h.call('tick'); }
    assert.equal(JSON.stringify(f.calls), calls);
    delete f.faults.writeSync; h.advance(60001); h.call('tick');
    assert.equal(h.json('status').error, '');
    h.call('event', 'latency', 'hub.mcp', 30);
    assert.equal(h.json('export').events.length, 2);
  }
});

test('failed rollback removes the damaged segment; export returns only whole sanitized records', t => {
  const home = fixture(t), f = faultIO(), h = host(home, f.io); h.call('start');
  f.faults.writeSync = (...args) => fs.writeSync(args[0], args[1], args[2], 2, args[4]);
  f.faults.ftruncateSync = () => { throw full(); };
  h.call('event', 'latency', 'hub.rpc', 10);
  assert.equal(h.json('flush').ok, false); assert(!fs.existsSync(segment(home)));
  assert.equal(h.json('export').events.length, 0);
});

test('disk-full disable stops immediately, fails visibly and restarts disabled after a torn control write', t => {
  const home = fixture(t), f = faultIO(), h = host(home, f.io); h.call('start');
  h.call('event', 'latency', 'hub.rpc', 1);
  f.faults.writeSync = () => { throw full(); };
  const result = h.json('control', 'off');
  assert.equal(result.enabled, false); assert.equal(result.ok, false); assert.equal(result.error, 'full');
  assert.equal(result.pending, 0);
  const restarted = host(home); assert.equal(restarted.json('start').enabled, false);
  assert.equal(restarted.json('export').events.length, 0);
});

test('every storage-operation failure is contained and returns fixed codes without paths or exception text', t => {
  for (const method of ['mkdirSync', 'lstatSync', 'chmodSync', 'readdirSync', 'openSync', 'fstatSync', 'fchmodSync', 'ftruncateSync', 'writeSync', 'fsyncSync', 'readSync']) {
    const home = fixture(t), f = faultIO(); f.faults[method] = () => { throw full(); };
    const h = host(home, f.io);
    assert.doesNotThrow(() => h.call('start'), method);
    for (const action of ['off', 'on', 'clear']) assert.doesNotThrow(() => h.call('control', action), method);
    assert.doesNotThrow(() => h.call('tick'), method);
    const out = h.call('export'); assert.equal(JSON.parse(out).status.ok, false, method);
    assert(!/SECRET|private|192\.0\.2|example\.invalid/.test(out), method);
    assert.equal(f.descriptors.size, 0, method + ' closes its descriptors');
  }
  for (const action of ['off', 'on', 'clear']) {
    const home = fixture(t), f = faultIO(), h = host(home, f.io); h.call('start');
    h.call('event', 'latency', 'hub.rpc', 10); h.call('flush');
    f.faults[action === 'clear' ? 'unlinkSync' : 'fsyncSync'] = () => { throw full(); };
    assert.equal(h.json('control', action).ok, false);
  }
});

test('maintenance/export faults after startup stay bounded, close descriptors and never expose partial raw data', t => {
  for (const method of ['lstatSync', 'chmodSync', 'readdirSync', 'openSync', 'fstatSync', 'fchmodSync', 'readSync']) {
    const home = fixture(t), f = faultIO(), h = host(home, f.io); h.call('start');
    h.call('event', 'latency', 'hub.rpc', 10); h.call('flush');
    h.call('event', 'error', 'timeout', 1);
    f.faults[method] = () => { throw full(); };
    const result = h.json('export');
    assert.equal(result.status.ok, false, method);
    assert.equal(result.status.error, 'full', method);
    assert.equal(result.status.pending, 0, method);
    assert(!JSON.stringify(result).includes('SECRET'));
    assert.equal(f.descriptors.size, 0, method);
    delete f.faults[method];
    assert(usage(home).bytes <= CAP); assert(usage(home).allocated <= CAP);
  }
});

test('recovery deletes oversized owned files before reading; there are no forgotten rotated/temp files in cap accounting', t => {
  const home = fixture(t); let h = host(home); h.call('start');
  for (let i = 0; i < 8; i++) fs.writeFileSync(segment(home, i), Buffer.alloc(SEGMENT * 4));
  assert(usage(home).bytes > CAP);
  const f = faultIO();
  f.faults.readSync = (fd, ...args) => {
    assert.equal(path.basename(f.descriptors.get(fd)), 'control');
    return fs.readSync(fd, ...args);
  };
  h = host(home, f.io); assert.equal(h.json('start').ok, true);
  assert.equal(usage(home).bytes, 1); assert.equal(h.json('export').events.length, 0);
});

test('filesystem block usage is measured separately, including control, and a cap excess rolls back', t => {
  const home = fixture(t), f = faultIO(), h = host(home, f.io); h.call('start');
  const orig = fs.lstatSync;
  f.faults.lstatSync = p => {
    const st = orig(p);
    if (p.endsWith('.jsonl') && st.size > 0) st.blocks = 4096;
    return st;
  };
  h.call('event', 'latency', 'hub.rpc', 10);
  assert.equal(h.json('flush').error, 'capacity');
  assert.equal(fs.statSync(segment(home)).size, 0);
  assert(h.json('status').stored_bytes <= CAP); assert(h.json('status').allocated_bytes <= CAP);
});

test('symlinks/hardlinks and unexpected files fail closed; legacy logs are never touched', t => {
  const legacy = path.join(fixture(t), 'legacy.log'); fs.writeFileSync(legacy, 'SECRET');
  for (const kind of ['directory-link', 'file-link', 'hardlink', 'unknown']) {
    const home = fixture(t), dir = path.join(home, 'diagnostics');
    if (kind === 'directory-link') fs.symlinkSync(path.dirname(legacy), dir, 'dir');
    else {
      fs.mkdirSync(dir);
      if (kind === 'file-link') fs.symlinkSync(legacy, path.join(dir, 'control'));
      if (kind === 'hardlink') fs.linkSync(legacy, path.join(dir, 'control'));
      if (kind === 'unknown') fs.writeFileSync(path.join(dir, 'old.log'), 'SECRET');
    }
    const h = host(home), result = h.json('start');
    assert.equal(result.enabled, false); assert.equal(result.error, 'unsafe_storage');
    assert.equal(h.json('export').events.length, 0); assert.equal(fs.readFileSync(legacy, 'utf8'), 'SECRET');
  }
});

test('actual kernel ENOSPC is contained (Linux /dev/full)', { skip: process.platform !== 'linux' }, t => {
  const home = fixture(t), f = faultIO(), h = host(home, f.io); h.call('start');
  const fd = fs.openSync('/dev/full', 'w'); t.after(() => fs.closeSync(fd));
  f.faults.writeSync = (_fd, ...args) => fs.writeSync(fd, ...args);
  h.call('event', 'latency', 'hub.rpc', 10);
  assert.equal(h.json('flush').error, 'full'); assert.equal(h.json('export').events.length, 0);
});
