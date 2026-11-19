// Browser context for the disposable HTTPS test hub. Chromium resolves the
// synthetic origin to loopback while retaining the dedicated hostname in Host,
// Origin and its cookie jar. Only the fixture certificate's SPKI is trusted.
import assert from 'node:assert/strict';
import { createHash, X509Certificate } from 'node:crypto';
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
