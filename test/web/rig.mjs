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
