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
// Add a device with every kind of address: Tailscale, two local network
// addresses (the hub answers on neither yet), this computer
const entry2 = join(out, 'addresses.ts');
writeFileSync(entry2, `import {installPairingUI} from ${JSON.stringify(resolve('src/web/pairing-ui.js'))};
  const at = (kind, url, listening, qr) => ({kind, url, listening, qr, link: url + '/pair#invite=0123456789abcdef0123456789abcdef&routes=' + encodeURIComponent(JSON.stringify([url]))});
  const addresses = [at('tailnet', 'http://100.101.102.103:3787', true, '101\\n010\\n101'), at('lan', 'http://192.168.1.23:3787', false, '11\\n11'), at('lan', 'http://10.0.0.7:3787', false, '1'), at('local', 'http://127.0.0.1:3787', true, '1001\\n0110')];
  let lan = [];
  window.calls = [];
  const fetch = async (path, options = {}) => { window.calls.push({path, headers: options.headers || {}}); return {ok: true, json: async () => {
    if (path === '/auth/devices') return {devices: [], invitations: [], lan};
    if (path === '/auth/invitations') return {code: 'ABCD2345', link: addresses[0].link, routes: [addresses[0].url], localOnly: false, expiresAt: Date.now() + 600000, qr: addresses[0].qr, addresses};
    if (path === '/auth/lan') { const ip = options.headers['X-Backplane-Lan'], on = options.headers['X-Backplane-Lan-State'] === 'on';
      lan = on ? [{ip, url: 'http://' + ip + ':3787', listening: true}] : [];return {ok: true, ip, listening: on}; }
    return {ok: true};
  }}; };
  window.ui = installPairingUI({document, location, fetch, onPaired: () => {}});
  window.ui.devices();`);
const built2 = await Bun.build({entrypoints: [entry2], target: 'browser'});
if (!built2.success) throw Error(String(built2.logs));
const js2 = await built2.outputs[0].text();
const built = await Bun.build({entrypoints: [entry], target: 'browser'});
if (!built.success) throw Error(String(built.logs));
const js = await built.outputs[0].text();
const css = readFileSync(resolve('src/web/style.css'), 'utf8') + readFileSync(resolve('src/web/pairing.css'), 'utf8');
const server = Bun.serve({hostname: '127.0.0.1', port: 0, fetch: request => {
  const p = new URL(request.url).pathname;
  if (p === '/f.js' || p === '/a.js') return new Response(p === '/f.js' ? js : js2, {headers: {'Content-Type': 'text/javascript'}});
  if (p === '/s.css') return new Response(css, {headers: {'Content-Type': 'text/css'}});
  return new Response('<!doctype html><link rel="stylesheet" href="/s.css"><div id="root"></div><script type="module" src="/' + (p === '/addresses' ? 'a' : 'f') + '.js"></script>', {headers: {'Content-Type': 'text/html'}});
}});
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
  // the address choice: Tailscale by default; each choice swaps the hub
  // address, the link and the QR in place, with no second invitation
  const p2 = await browser.newPage();
  await p2.goto(`http://127.0.0.1:${server.port}/addresses`);
  await p2.click('.pairing-screen button:has-text("Add a device")');
  await p2.waitForSelector('.pairing-where');
  const view = () => p2.evaluate(() => ({
    picked: [...document.querySelectorAll('.pairing-where [role=radio]')].filter(b => b.getAttribute('aria-checked') === 'true').map(b => b.textContent),
    hub: document.querySelector('.pairing-hub').textContent, link: (document.querySelector('.pairing-shares input') as HTMLInputElement).value,
    shares: !(document.querySelector('.pairing-shares') as HTMLElement).hidden, qr: document.querySelector('.pairing-qr path').getAttribute('d'),
    note: (document.querySelector('.pairing-note') as HTMLElement).hidden ? '' : document.querySelector('.pairing-note').textContent,
    offer: [...document.querySelectorAll('.pairing-offer button')].filter(b => !(b as HTMLElement).hidden && !(b.parentElement as HTMLElement).hidden).map(b => b.textContent),
    status: document.querySelector('.pairing-status').textContent, picker: !(document.querySelector('.pairing-lanpick') as HTMLElement).hidden}));
  assert.deepEqual(await p2.locator('.pairing-where [role=radio]').allTextContents(), ['Tailscale', 'Local network', 'This computer']);
  let v = await view();
  assert.deepEqual(v.picked, ['Tailscale']);assert.equal(v.hub, 'Hub address: http://100.101.102.103:3787');assert.ok(v.link.startsWith('http://100.101.102.103:3787/pair#invite='));
  assert.ok(v.shares);assert.ok(v.note.includes('Tailscale'));assert.deepEqual(v.offer, []);assert.ok(!v.picker);
  const qrTail = v.qr;
  await p2.click('.pairing-where button:has-text("This computer")');v = await view();
  assert.equal(v.hub, 'Hub address: http://127.0.0.1:3787');assert.ok(v.link.startsWith('http://127.0.0.1:3787/pair#'));assert.notEqual(v.qr, qrTail);
  assert.ok(v.status.startsWith('This address works only on this computer.'));assert.ok(v.shares);
  // a local network address the hub does not answer on: no link to hand
  // out, an offer to let it answer there, honest about plain HTTP
  await p2.click('.pairing-where button:has-text("Local network")');v = await view();
  assert.equal(v.hub, 'Hub address: http://192.168.1.23:3787');assert.ok(!v.shares);assert.ok(v.picker);
  assert.deepEqual(v.offer, ['Let the hub answer on the local network']);assert.ok(v.note.includes('plain HTTP'));
  await p2.selectOption('.pairing-lanpick select', '1');v = await view();assert.equal(v.hub, 'Hub address: http://10.0.0.7:3787');
  await p2.selectOption('.pairing-lanpick select', '0');
  await p2.click('.pairing-offer button:has-text("Let the hub answer")');
  await p2.waitForFunction(() => !(document.querySelector('.pairing-shares') as HTMLElement).hidden);v = await view();
  assert.ok(v.link.startsWith('http://192.168.1.23:3787/pair#'));assert.deepEqual(v.offer, ['Stop answering on the local network']);assert.ok(v.note.includes('not encrypted'));
  await p2.waitForSelector('.pairing-lan .pairing-device');
  assert.ok((await p2.locator('.pairing-lan').innerText()).includes('http://192.168.1.23:3787'));
  const lanCall = await p2.evaluate(() => (window as any).calls.find((c: any) => c.path === '/auth/lan'));
  assert.deepEqual(lanCall.headers, {'X-Backplane-Lan': '192.168.1.23', 'X-Backplane-Lan-State': 'on'});
  // keys move the choice too
  await p2.focus('.pairing-where button.on');await p2.keyboard.press('ArrowRight');v = await view();assert.deepEqual(v.picked, ['This computer']);
  await p2.keyboard.press('ArrowLeft');await p2.keyboard.press('ArrowLeft');v = await view();assert.deepEqual(v.picked, ['Tailscale']);
  // turned off beside the choice: the link goes again
  await p2.click('.pairing-where button:has-text("Local network")');
  await p2.click('.pairing-offer button:has-text("Stop answering")');
  await p2.waitForFunction(() => (document.querySelector('.pairing-shares') as HTMLElement).hidden);
  await p2.waitForFunction(() => !document.querySelector('.pairing-lan .pairing-device'));
  assert.equal(await p2.evaluate(() => (window as any).calls.filter((c: any) => c.path === '/auth/invitations').length), 1);
  console.log('pairing_devices_refresh: ok');
} finally { await browser.close();server.stop(true); }
