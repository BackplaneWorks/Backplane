// Browser context for the disposable HTTPS test hub. Chromium resolves the
// synthetic origin to loopback while retaining the dedicated hostname in Host,
// Origin and its cookie jar. Only the fixture certificate's SPKI is trusted.
import { installVoiceStandin } from './voice.mjs';
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
export async function withTestTls(socketPath, run, canary) {
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
  let leaked = false;
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
        let tail = Buffer.alloc(0);
        upstream.on('data', chunk => {
          if (!canary) return;
          const bytes = Buffer.concat([tail, chunk]);
          if (bytes.includes(canary)) leaked = true;
          tail = bytes.subarray(Math.max(0, bytes.length - canary.length + 1));
        });
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
    const result = await run({ certificate: certificate.toString(), port });
    assert.ok(!leaked, 'raw masking fixture in proxy response');
    return result;
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
  const { spawn, execFileSync } = await import('node:child_process');
  const { once } = await import('node:events');
  const { createServer } = await import('node:net');
  const { checkAccess, checkRequestBoundary } = await import('./security.mjs');
  const canary = 'fixture-mask-' + (await import('node:crypto')).randomUUID();
  const { browserTasks } = await import('./tasks.mjs');
  assert.ok(binary && output && process.env.BP_TMP && process.versions.bun);
  const temp = await fs.mkdtemp(path.join(process.env.BP_TMP, 'web-e2e-'));
  const agents = path.join(temp, 'agents');
  let timestamp = Date.now();
  const clock = path.join(temp, 'registry-clock');
  await fs.writeFile(clock, String(timestamp));
  const advanceDays = async days => {
    timestamp += days * 86400000;
    await fs.writeFile(clock + '.next', String(timestamp));
    await fs.rename(clock + '.next', clock);
  };
  const home = path.join(temp, 'hub');
  const native = createServer();
  native.listen(0, '127.0.0.1');
  await once(native, 'listening');
  const nativePort = native.address().port;
  await new Promise(resolve => native.close(resolve));
  assert.ok(nativePort >= 3799);
  const project = path.join(temp, 'project');
  const models = new URL('../files/fixtures/', import.meta.url);
  await fs.mkdir(path.join(project, 'mech'), {recursive:true});
  for (const extension of ['kicad_pcb','kicad_sch','kicad_prl']) {
    const name = 'ecc83-pp.' + extension;
    await fs.copyFile(new URL('plot/' + name, models), path.join(project, name));
  }
  await fs.writeFile(path.join(project, 'ecc83-pp.kicad_pro'), '{}');
  await fs.copyFile(new URL('mesh/bracket.step', models), path.join(project, 'mech', 'bracket.step'));
  const gerbers = path.join(project, 'mfg', 'gerbers');
  await fs.mkdir(gerbers, {recursive:true});
  const cli = process.env.BACKPLANE_KICAD_CLI;
  assert.ok(cli, 'set BACKPLANE_KICAD_CLI to the selected KiCad runtime');
  const board = path.join(project, 'ecc83-pp.kicad_pcb');
  const exportEnv = { ...process.env, HOME: temp, XDG_CONFIG_HOME: path.join(temp, 'config'),
    XDG_CACHE_HOME: path.join(temp, 'cache'), XDG_DATA_HOME: path.join(temp, 'data') };
  delete exportEnv.DISPLAY;
  delete exportEnv.WAYLAND_DISPLAY;
  for (const args of [
    ['gerbers', '--layers', 'F.Cu,B.Cu,F.Mask,B.Mask,F.Silkscreen,B.Silkscreen,Edge.Cuts', '--no-protel-ext'],
    ['drill', '--format', 'excellon', '--excellon-units', 'mm'],
  ]) {
    execFileSync(cli, ['pcb', 'export', ...args, '--output', gerbers, board], {env:exportEnv});
  }
  await fs.mkdir(agents);
  await fs.mkdir(output, { recursive: true });
  const standin = new URL('../tools/claude_standin.ts', import.meta.url).pathname;
  await fs.writeFile(path.join(agents, 'claude'), '#!/bin/sh\nexec bun "$BP_WEB_STANDIN" "$@"\n', { mode: 0o700 });
  try {
    await withTestTls(path.join(home, 'browser.sock'), async tls => {
      const env = { BACKPLANE_VOICE_BIN: await installVoiceStandin(temp), PATH: process.env.PATH, HOME: temp, TMPDIR: temp, BP_TMP: temp,
        BACKPLANE_VAULT_KEYRING:'file', BP_WEB_MASK_FIXTURE:canary, BP_STREAM_N: "3", BP_STREAM_MS: "600", BP_WEB_STANDIN: standin, BP_WEB_FIXTURE: temp, BACKPLANE_AGENTS: agents,
        BP_WEB_NATIVE_PORT: String(nativePort), BP_WEB_TLS_PORT: String(tls.port),
        BACKPLANE_NO_UPDATE: '1', BACKPLANE_ORDER_SERVER: 'http://127.0.0.1:9' };
      if (process.env.BACKPLANE_STEP2GLB) env.BACKPLANE_STEP2GLB = process.env.BACKPLANE_STEP2GLB;
      const child = spawn(binary, ['--ignored', '--exact', 'net::listener::webingress::isolated_browser_hub'], {
        env, stdio: ['ignore', 'pipe', 'pipe'], detached: true,
      });
      const logs = [];
      for (const stream of [child.stdout, child.stderr]) stream.on('data', chunk => logs.push(chunk));
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
            probe.on("requestfailed", r => console.error("probe", r.method(), new URL(r.url()).pathname, r.failure()?.errorText));
            assert.equal((await probe.goto(origin)).status(), 404);
            const probeUrl = new URL('/__fixture_probe__', origin).href;
            await probe.route(probeUrl, route => route.fulfill({contentType:'text/html', body:'<!doctype html>'}));
            await probe.goto(probeUrl);
            await checkAccess(probe, false);
            await probe.close();
            await rpc('devices.page.act', { action: 'device-web-access', value: '1' });
            await browserTasks({ temp, context, origin, rpc, thread: 't1', canary, advanceDays,
              moveFixtureGerbers: async () => {
                const archive = path.join(temp, "archived-gerbers");
                await fs.rename(gerbers, archive);
                await fs.symlink(archive, gerbers, "dir");
              },
              registeredRoutes: JSON.parse(await fs.readFile(path.join(home, 'fixture-http-routes.json'), 'utf8')),
              checkBoundary: token => checkRequestBoundary(path.join(home, 'browser.sock'), origin, token),
              checkLogs: value => assert.ok(!Buffer.concat(logs).includes(value), 'browser session in hub output'),
              readRegistry: () => fs.readFile(path.join(home, 'devices-v1.json'), 'utf8'),
              assets: [{path:'/files/get?thread=t1&path=sample.txt',body:Buffer.from('Backplane fixture')}],
              replyText:'Browser0 Browser1 Browser2',
              nativeShot: process.env.BP_WEB_GPUI && (async name => {
                const helper = new URL('./native-shot.sh', import.meta.url).pathname;
                const proc = spawn('bash', [helper, temp, String(nativePort), output, name], {stdio:'inherit'});
                const [status] = await once(proc, 'exit');
                assert.equal(status, 0, name + ' capture failed');
              }),
              screenshot: (page, name) => page.screenshot({path:path.join(output, name + '.png')}) });
          });
        } finally { ws.close(); }
      } finally {
        if (child.pid && child.exitCode === null) process.kill(-child.pid, 'SIGTERM');
        await exited;
      }
    }, canary);
  } finally { await fs.rm(temp, {recursive:true, force:true}); }
}

if (import.meta.main) await main(...process.argv.slice(2));
