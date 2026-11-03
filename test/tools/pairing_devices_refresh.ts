// Devices (pairing-ui.js): Esc leaves it, and a device that pairs while its
// invitation is on screen shows in "Paired devices" without leaving the page (it showed only
// after Back and Manage devices again). Fixture responses only, no hub.
//   bun test/tools/pairing_devices_refresh.ts   (CHROMIUM_PATH optional)
import {mkdirSync, writeFileSync, readFileSync} from 'node:fs';
import {resolve, join} from 'node:path';
import {tmpdir} from 'node:os';
import {createRequire} from 'node:module';
import assert from 'node:assert/strict';
const require = createRequire(import.meta.url);
const {chromium} = require(process.env.PLAYWRIGHT_MODULE || resolve('tools/browser/node_modules/playwright-core'));
const out = join(tmpdir(), 'bp-pairing-refresh-' + process.pid);mkdirSync(out, {recursive: true});
const entry = join(out, 'fixture.ts');
writeFileSync(entry, `import {installPairingUI} from ${JSON.stringify(resolve('src/web/pairing-ui.js'))};
  let invited = false, asked = 0;
  const fetch = async path => ({ok: true, json: async () => {
    if (path === '/auth/devices') { asked++; return {devices: invited && asked > 2 ? [{id: 'd1', name: 'Phone in the lab', lastSeen: 1793664000000}] : []}; }
    if (path === '/auth/invitations') { invited = true; return {code: 'ABCD2345', link: 'https://hub.example/pair#code=ABCD2345', routes: ['https://hub.example'], expiresAt: Date.now() + 600000, qr: '1'}; }
    return {ok: true};
  }});
  window.ui = installPairingUI({document, location, fetch, onPaired: () => {}});
  window.ui.devices();`);
const built = await Bun.build({entrypoints: [entry], target: 'browser'});
if (!built.success) throw Error(String(built.logs));
const js = await built.outputs[0].text();
const server = Bun.serve({hostname: '127.0.0.1', port: 0, fetch: request => new URL(request.url).pathname === '/f.js'
  ? new Response(js, {headers: {'Content-Type': 'text/javascript'}})
  : new Response('<!doctype html><div id="root"></div><script type="module" src="/f.js"></script>', {headers: {'Content-Type': 'text/html'}})});
const browser = await chromium.launch({headless: true, executablePath: process.env.CHROMIUM_PATH, args: ['--no-sandbox']});
try {
  const page = await browser.newPage();
  await page.goto(`http://127.0.0.1:${server.port}/`);
  await page.waitForSelector('.pairing-screen');
  await page.click('.pairing-screen button:has-text("Add a device")');
  await page.waitForSelector('.pairing-code');
  assert.equal(await page.locator('.pairing-device').count(), 0);
  await page.waitForFunction(() => document.querySelector('.pairing-device')?.innerText.includes('Phone in the lab'), null, {timeout: 8000});
  assert.ok(!(await page.locator('.pairing-screen [role="status"]').first().innerText()).includes('could not'));
  // Esc leaves Devices (as Back does); the connection screen has no Back and stays
  await page.keyboard.press('Escape');
  await page.waitForFunction(() => !document.querySelector('.pairing-screen'), null, {timeout: 3000});
  await page.evaluate(() => (window as any).ui.show());
  await page.keyboard.press('Escape');
  assert.equal(await page.locator('.pairing-screen').count(), 1);
  console.log('pairing_devices_refresh: ok');
} finally { await browser.close();server.stop(true); }
