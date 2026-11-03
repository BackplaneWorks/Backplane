// Settings > Devices > Pairing links (pairing-ui.js) lists hub-to-hub invites
// (/auth/devices invitations of kind "machine") as machine invites: the
// machine that used one, unlinked once revoked, and Revoke on an open one or
// a used one whose link is live, sent with X-Backplane-Invitation. Fixture
// responses only, no hub.
//   bun test/tools/pairing_machine_invites_ui.ts   (CHROMIUM_PATH, PLAYWRIGHT_MODULE optional)
import {mkdirSync, writeFileSync} from 'node:fs';
import {resolve, join} from 'node:path';
import {tmpdir} from 'node:os';
import {createRequire} from 'node:module';
import assert from 'node:assert/strict';
const require = createRequire(import.meta.url);
const {chromium} = require(process.env.PLAYWRIGHT_MODULE || resolve('tools/browser/node_modules/playwright-core'));
const out = join(tmpdir(), 'bp-pairing-machines-' + process.pid);mkdirSync(out, {recursive: true});
const entry = join(out, 'fixture.ts');
writeFileSync(entry, `import {installPairingUI} from ${JSON.stringify(resolve('src/web/pairing-ui.js'))};
  const at = 1793664000000;
  window.revoked = [];
  const invitations = [
    {id: 'a'.repeat(32), kind: 'machine', createdAt: at, expiresAt: at + 600000, state: 'open'},
    {id: 'b'.repeat(32), kind: 'machine', createdAt: at, expiresAt: at + 600000, state: 'used', usedAt: at + 1000, device: 'p' + '1'.repeat(16), deviceName: 'desk', deviceRevoked: false},
    {id: 'c'.repeat(32), kind: 'machine', createdAt: at, expiresAt: at + 600000, state: 'revoked', usedAt: at + 1000, device: 'p' + '2'.repeat(16), deviceName: 'lab', deviceRevoked: true},
    {id: 'd'.repeat(32), kind: 'device', createdAt: at, expiresAt: at + 600000, state: 'expired'},
  ];
  const fetch = async (path, init) => ({ok: true, json: async () => {
    if (path === '/auth/devices') return {devices: [], invitations};
    if (path === '/auth/revoke') { window.revoked.push(init.headers['X-Backplane-Invitation']); return {ok: true, clients: []}; }
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
  page.on('dialog', d => d.accept());
  await page.goto(`http://127.0.0.1:${server.port}/`);
  await page.waitForFunction(() => document.querySelectorAll('.pairing-device').length === 4, null, {timeout: 8000});
  const rows = await page.locator('.pairing-device').allInnerTexts();
  assert.ok(rows[0].startsWith('Machine invite · ') && rows[0].includes('Not used yet') && rows[0].includes('Revoke'), rows[0]);
  assert.ok(rows[1].startsWith('Machine invite · ') && rows[1].includes('by desk') && !rows[1].includes('unlinked') && rows[1].includes('Revoke'), rows[1]);
  assert.ok(rows[2].includes('Revoked with lab') && !rows[2].includes('Revoke\n') && !/Revoke$/.test(rows[2].trim()), rows[2]);
  assert.ok(rows[3].startsWith('Device invitation · ') && rows[3].includes('Expired'), rows[3]);
  if (process.env.SHOT) await page.screenshot({path: process.env.SHOT, fullPage: true});
  await page.locator('.pairing-device').nth(1).locator('button:has-text("Revoke")').click();
  await page.waitForFunction(() => (window as any).revoked.length === 1, null, {timeout: 3000});
  assert.deepEqual(await page.evaluate(() => (window as any).revoked), ['b'.repeat(32)]);
  console.log('pairing_machine_invites_ui: ok');
} finally { await browser.close();server.stop(true); }
