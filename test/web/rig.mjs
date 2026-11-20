// Browser context for the disposable HTTPS test hub. Chromium resolves the
// synthetic origin to loopback while retaining the dedicated hostname in Host,
// Origin and its cookie jar. Only the fixture certificate's SPKI is trusted.
import assert from 'node:assert/strict';
import { createHash, generateKeyPairSync, X509Certificate } from 'node:crypto';
import { createRequire } from 'node:module';

export async function withBrowser({ certificate, port }, run) {
  assert.ok(Number.isInteger(port) && port >= 3799 && port <= 65535);
  const require = createRequire(new URL('../../tools/browser/package.json', import.meta.url));
  const { chromium } = require('playwright-core');
  const publicKey = new X509Certificate(certificate).publicKey;
  const pin = createHash('sha256').update(publicKey.export({ type: 'spki', format: 'der' })).digest('base64');
  const hostname = 'backplane.example.test';
  const env = { ...process.env };
  delete env.DISPLAY;
  delete env.WAYLAND_DISPLAY;
  const browser = await chromium.launch({
    channel: 'chrome',
    headless: true,
    env,
    args: [
      '--no-proxy-server',
      '--disable-background-networking',
      '--host-resolver-rules=MAP *.example.test 127.0.0.1, MAP * ~NOTFOUND',
      '--ignore-certificate-errors-spki-list=' + pin,
    ],
  });
  try {
    const context = await browser.newContext({ serviceWorkers: 'block' });
    return await run(context, 'https://' + hostname + ':' + port);
  } finally {
    await browser.close();
  }
}

// Mimic Serve's authenticated requester on a private backend stream. Keeping
// the temporary certificate key in memory avoids leaving it in test artifacts.
export async function withTestTls(socketPath, run) {
  const { execFileSync } = await import('node:child_process');
  const { createServer } = await import('node:tls');
  const { connect } = await import('node:net');
  const { once } = await import('node:events');
  const { privateKey } = generateKeyPairSync('ec', { namedCurve: 'prime256v1' });
  const key = privateKey.export({ type: 'pkcs8', format: 'pem' });
  const certificate = execFileSync('sh', [
    '-c', 'cat | openssl "$@"', 'openssl',
    'req', '-x509', '-new', '-key', '/dev/stdin', '-days', '1', '-subj', '/CN=backplane.example.test',
    '-addext', 'subjectAltName=DNS:backplane.example.test',
  ], { input: key, stdio: ['pipe', 'pipe', 'ignore'] }).toString();
  const sockets = new Set();
  const server = createServer({ key, cert: certificate }, socket => {
    let head = Buffer.alloc(0);
    const read = chunk => {
      head = Buffer.concat([head, chunk]);
      const end = head.indexOf('\r\n\r\n');
      if (head.length > 65536 || socket.servername !== 'backplane.example.test') return socket.destroy();
      if (end === -1) return;
      socket.pause();
      socket.off('data', read);
      const fields = head.subarray(0, end).toString('latin1').split('\r\n');
      const clean = fields.filter(line => !/^tailscale-user-login:/i.test(line));
      clean.push('Tailscale-User-Login: person@example.com', '', '');
      const upstream = connect(socketPath);
      upstream.on('error', () => socket.destroy());
      socket.once('close', () => upstream.destroy());
      upstream.once('connect', () => {
        upstream.write(clean.join('\r\n'), 'latin1');
        upstream.write(head.subarray(end + 4));
        upstream.pipe(socket);
        socket.pipe(upstream);
      });
    };
    socket.on('error', () => socket.destroy());
    socket.on('data', read);
  });
  server.on('connection', socket => {
    sockets.add(socket);
    socket.once('close', () => sockets.delete(socket));
  });
  server.listen(0, '127.0.0.1');
  try {
    await once(server, 'listening');
    const port = server.address().port;
    assert.ok(port >= 3799);
    return await run({ certificate: certificate.toString(), port });
  } finally {
    for (const socket of sockets) socket.destroy();
    await new Promise(resolve => server.close(resolve));
  }
}

// Run with bun test/web/rig.mjs <hub test executable> <screenshot directory>
// after building cargo-q test --bin backplane-hub --no-run in native/host.
async function main(binary, output) {
  const fs = await import('node:fs/promises');
  const path = await import('node:path');
  const { spawn } = await import('node:child_process');
  const { once } = await import('node:events');
  const { createServer } = await import('node:net');
  const { browserTasks } = await import('./tasks.mjs');
  assert.ok(binary && output && process.env.BP_TMP && process.versions.bun);
  const temp = await fs.mkdtemp(path.join(process.env.BP_TMP, 'web-e2e-'));
  const agents = path.join(temp, 'agents');
  const home = path.join(temp, 'hub');
  const native = createServer();
  native.listen(0, '127.0.0.1');
  await once(native, 'listening');
  const nativePort = native.address().port;
  await new Promise(resolve => native.close(resolve));
  assert.ok(nativePort >= 3799);
  await fs.mkdir(agents);
  await fs.mkdir(output, { recursive: true });
  const standin = new URL('../tools/claude_standin.ts', import.meta.url).pathname;
  await fs.writeFile(path.join(agents, 'claude'), '#!/bin/sh\nexec bun "$BP_WEB_STANDIN" "$@"\n', { mode: 0o700 });
  try {
    await withTestTls(path.join(home, 'browser.sock'), async tls => {
      const env = { PATH: process.env.PATH, HOME: temp, TMPDIR: temp, BP_TMP: temp,
        BP_WEB_STANDIN: standin, BP_WEB_FIXTURE: temp, BACKPLANE_AGENTS: agents,
        BP_WEB_NATIVE_PORT: String(nativePort), BP_WEB_TLS_PORT: String(tls.port),
        BACKPLANE_NO_UPDATE: '1', BACKPLANE_ORDER_SERVER: 'http://127.0.0.1:9' };
      const child = spawn(binary, ['--ignored', '--exact', 'net::listener::webingress::isolated_browser_hub'], {
        env, stdio: 'ignore', detached: true,
      });
      const exited = once(child, 'exit');
      try {
        for (let i = 0; ; ++i) {
          if (await fs.stat(path.join(home, 'browser.sock')).then(s => s.isSocket(), () => false)) break;
          assert.ok(child.exitCode === null && i < 200, 'fixture failed to start');
          await Bun.sleep(50);
        }
        const credential = (await fs.readFile(path.join(home, 'token'), 'utf8')).trim();
        const ws = new WebSocket('ws://127.0.0.1:' + nativePort + '/ws?data=1&enc=json', {
          headers: { Authorization: 'Bearer ' + credential },
        });
        const replies = new Map();
        let next = 0;
        ws.onmessage = event => {
          if (typeof event.data !== 'string') return;
          const frame = JSON.parse(event.data);
          if (frame.t === 'reply') replies.get(frame.id)?.(frame);
        };
        await new Promise((resolve, reject) => { ws.onopen = resolve; ws.onerror = () => reject(Error('native fixture connection failed')); });
        const rpc = async (method, params) => {
          const id = ++next;
          try {
            const frame = await new Promise((resolve, reject) => {
              const timer = setTimeout(() => reject(Error(method + ' timed out')), 5000);
              replies.set(id, frame => { clearTimeout(timer); resolve(frame); });
              ws.send(JSON.stringify({ id, m: method, p: params }));
            });
            assert.equal(frame.ok, true, method + ' failed');
            return frame.result;
          } finally { replies.delete(id); }
        };
        try {
          await withBrowser(tls, async (context, origin) => {
            const probe = await context.newPage();
            assert.equal((await probe.goto(origin)).status(), 404);
            await probe.close();
            await rpc('devices.page.act', { action: 'device-web-access', value: '1' });
            await browserTasks({ context, origin, rpc, thread: 't1',
              assets: [{path:'/files/get?thread=t1&path=sample.txt',body:Buffer.from('Backplane fixture')}],
              replyText:'Browser stand-in reply',
              screenshot: (page, name) => page.screenshot({path:path.join(output, name + '.png')}) });
          });
        } finally { ws.close(); }
      } finally {
        if (child.pid && child.exitCode === null) process.kill(-child.pid, 'SIGTERM');
        await exited;
      }
    });
  } finally { await fs.rm(temp, {recursive:true, force:true}); }
}

if (import.meta.main) await main(...process.argv.slice(2));
