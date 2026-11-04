// Runtime-only Boat component experiment. Executes the production static sender
// and bp_idle, with real Bun TCP sockets and files. It is NOT a whole hub build.
import * as fs from 'node:fs';
import { resolve, join } from 'node:path';
import { runInNewContext } from 'node:vm';
import { request } from 'node:http';
import { createHash } from 'node:crypto';
import { gunzipSync } from 'node:zlib';
import { cpus, hostname } from 'node:os';
const out = resolve('build/hub-latency/effect-transfer'); fs.mkdirSync(out, { recursive: true });
const group = '/sys/fs/cgroup' + fs.readFileSync('/proc/self/cgroup', 'utf8').trim().split('::')[1];
const cap = Number(fs.readFileSync(group + '/memory.max', 'utf8'));
if (!(cap > 0 && cap <= 3700000000) || fs.readFileSync(group + '/memory.swap.max', 'utf8').trim() !== '0') throw Error('Uncapped experiment refused');
const hostSource = fs.readFileSync('src/server/effects/host.js', 'utf8');
const a = hostSource.indexOf('function sock_send_static('), b = hostSource.indexOf('\nfunction sock_send_preview(', a);
if (a < 0 || b <= a) throw Error('Unexpected static sender shape');
const senderSource = hostSource.slice(a, b);
const baselineFile = fs.existsSync('test/perf/hub-latency/input/baseline-loop.js')
  ? 'test/perf/hub-latency/input/baseline-loop.js' : 'test/perf/hub-latency/input/retained-server.js';
const oldSource = fs.readFileSync(baselineFile, 'utf8');
const newSource = fs.readFileSync('src/server/effects/loop.js', 'utf8');
function idleSource(s: string) { const a = s.indexOf('async function bp_idle(io) {'), b = s.indexOf('\n// A slow synchronous', a); if (a < 0 || b <= a) throw Error('Unexpected idle shape'); return s.slice(a, b); }
const oldIdle = idleSource(oldSource), newIdle = idleSource(newSource);
if (createHash('sha256').update(oldIdle).digest('hex') !== 'cd8a5d6c5d31df1503227983f5d17813d355c4b968358cf735c29a911579bf32') throw Error('Unreviewed baseline scheduler');
const web = resolve('test/perf/hub-latency/input/web');
const file = fs.readdirSync(web).find(x => /^chunk-.*\.js$/.test(x))!;
const expected = createHash('sha256').update(fs.readFileSync(join(web, file))).digest('hex');
const hash = (s: string) => createHash('sha256').update(s).digest('hex');
const results: any[] = [];
const identity = { kind: 'production static sender + bp_idle component experiment; NOT whole-hub acceptance',
  machine: 'Boat default', startedUtc: new Date().toISOString(), hostname: hostname(), cpu: cpus()[0]?.model,
  capBytes: cap, swapBytes: 0, bun: Bun.version, staticSenderSha256: hash(senderSource), oldIdleSha256: hash(oldIdle), newIdleSha256: hash(newIdle),
  concurrentLoad: process.env.LATENCY_CONCURRENT_LOAD || 'runtime-only hub-latency lock; coordinated bend emit may run in another VM; physical host placement unknown',
  file, bytes: fs.statSync(join(web, file)).size, gzipBytes: fs.statSync(join(web, file + '.gz')).size,
  adapter: 'Bun TCP with writable-drain waits; cooperative timer parks execute exact bp_idle; no Bend actor, request parser or renderer included' };
function server(idleText: string) {
  const sockets = new Map<number, any>(), drain = new Map<number, () => void>(); let next = 1;
  let parks = 0;
  const idle = runInNewContext(`(${idleText})`, { performance, setImmediate, setTimeout, clearTimeout,
    bp_due() {}, bp_poll() { throw Error('Unexpected POSIX descriptor poll'); } });
  const handles = { EAGAIN: 11, is: (s: number) => sockets.has(s), get: (s: number) => {
    const socket = sockets.get(s); return socket ? { write: (bytes: Uint8Array) => socket.write(bytes) } : undefined;
  } };
  let complete = new Map<number, (x: any) => void>();
  const settle = (id: number, result: any) => { if (result !== undefined) { complete.get(id)?.(result); complete.delete(id); } };
  const send = runInNewContext(`(${senderSource})`, { require: (n: string) => { if (n !== 'fs') throw Error('Unexpected require'); return fs; },
    Buffer, process, performance, BP_H: handles, io_sys() { throw Error('Unexpected POSIX path'); },
    io_tup: (socket: number, result: any) => ({ socket, result }), io_done: () => ({ ok: true }), io_fail: (errno: number) => ({ ok: false, errno }), host_errno: () => 5,
    io_park_on(id: number | undefined, writable: boolean, k: any, more: () => any, at: number) {
      parks++;
      if (id === undefined) {
        // The sender parks at now after each bounded piece. Execute the exact
        // production idle boundary before its retry; Bun gets one event turn.
        idle({ waits: [{ at }], pend: 1, tick: 1, wake: null }).then(() => settle(k.id, more()));
      } else {
        if (!writable) throw Error('Unexpected reader wait');
        const timer = setTimeout(() => { drain.delete(id); settle(k.id, more()); }, Math.max(0, Math.ceil(at - performance.now())));
        drain.set(id, () => { clearTimeout(timer); drain.delete(id); settle(k.id, more()); });
      }
    } });
  const listener = Bun.listen({ hostname: '127.0.0.1', port: 0, socket: {
    open(socket: any) { socket.data = { id: next++, request: '' }; sockets.set(socket.data.id, socket); },
    data(socket: any, bytes: Uint8Array) {
      socket.data.request += Buffer.from(bytes).toString();
      if (!socket.data.request.includes('\r\n\r\n') || socket.data.started) return;
      socket.data.started = true;
      if (/^GET \/hello /.test(socket.data.request)) { socket.end('HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok'); return; }
      const id = socket.data.id, k: any = () => {}; k.id = id;
      const spec = { path: join(web, file), ctype: 'text/javascript', cache: 'no-cache', extra: '', piece: 65536,
        coding: /accept-encoding:\s*gzip/i.test(socket.data.request) ? 1 : 0 };
      new Promise<any>(r => { complete.set(id, r); settle(id, send(id, spec, k)); }).then(x => { if (!x.result.ok) console.error('sender errno', x.result.errno); socket.end(); });
    },
    drain(socket: any) { drain.get(socket.data.id)?.(); },
    close(socket: any) { sockets.delete(socket.data.id); },
    error(socket: any) { sockets.delete(socket.data.id); },
  } });
  return { listener, port: listener.port, parks: () => parks };
}
function get(port: number, path: string, coding = 'identity'): Promise<any> {
  const start = performance.now();
  return new Promise((resolve, reject) => {
    const req = request({ hostname: '127.0.0.1', port, path, agent: false, headers: { 'Accept-Encoding': coding } }, res => {
      const parts: Buffer[] = []; res.on('data', c => parts.push(c));
      res.on('end', () => resolve({ ms: performance.now() - start, status: res.statusCode, body: Buffer.concat(parts), coding: res.headers['content-encoding'] || 'identity' })); res.on('error', reject);
    }); req.setTimeout(15000, () => req.destroy(Error('HTTP timeout'))); req.on('error', reject); req.end();
  });
}
for (let round = 0; round < 3; round++) {
  for (const variant of round % 2 ? ['after', 'before'] : ['before', 'after']) {
    const s = server(variant === 'before' ? oldIdle : newIdle);
    try {
      for (const coding of ['identity', 'gzip']) for (let n = -2; n < 10; n++) {
        const r = await get(s.port, '/' + file, coding);
        if (r.status !== 200 || r.coding !== coding || hash(coding === 'gzip' ? gunzipSync(r.body) : r.body) !== expected) throw Error('Response bytes changed');
        if (n >= 0) results.push({ round, variant, coding, ms: r.ms, bytes: r.body.length });
      }
      for (let n = 0; n < 5; n++) {
        const bodies = Array.from({ length: 4 }, () => get(s.port, '/' + file));
        await new Promise(r => setTimeout(r, 1)); const ping = await get(s.port, '/hello');
        const all = await Promise.all(bodies); if (ping.status !== 200 || all.some(r => hash(r.body) !== expected)) throw Error('Concurrent response bytes changed');
        results.push({ round, variant, coding: 'hello-during-four', ms: ping.ms, transferMaxMs: Math.max(...all.map(r => r.ms)) });
      }
      results.push({ round, variant, coding: 'cooperative-parks', n: s.parks() });
    } finally { s.listener.stop(true); }
    fs.writeFileSync(join(out, 'results.json'), JSON.stringify({ identity, results }, null, 2));
    console.log(JSON.stringify({ round, variant, complete: true }));
  }
}
