// The pairing screen's in-page QR scanner, on a real hub and a real Chromium
// phone (Pixel 7) whose camera is a fake: --use-file-for-fake-video-capture
// plays a Y4M video of an invitation's QR code the hub just issued. A fresh
// phone on the hub's own address (a secure context: loopback) taps QR code →
// Scan with the camera, the scanner opens full screen (Close, the aiming
// square, the hint), reads the code and pairs at once, through the same
// /auth/enroll as a pasted link; the hub then lists the device. The same
// page at a plain http:// name (a host-resolver alias, not loopback, so not a
// secure context, as a phone at http://100.x.y.z:57808 is) offers no camera
// button and says why, with Short code one tap away.
//   bun test/tools/pairing_scanner_e2e.ts BACKPLANE-SERVE WEB-DIR [SHOTS-DIR]
import {mkdtempSync, mkdirSync, writeFileSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {createRequire} from 'node:module';
const require = createRequire(import.meta.url);
const {chromium, devices} = require(process.env.PLAYWRIGHT_MODULE || 'playwright-core');
const [bin, web, shotsArg] = process.argv.slice(2);
if (!bin || !web) throw Error('backplane-serve and web directory required');
const shots = resolve(shotsArg || 'build/pairing-scanner');
mkdirSync(shots, {recursive: true});
const root = mkdtempSync(join(tmpdir(), 'bp-scan-e2e-')), home = join(root, 'home'), agents = join(root, 'agents');
for (const d of [home, agents]) mkdirSync(d, {recursive: true, mode: 0o700});
for (const cli of ['claude', 'codex', 'grok', 'muse', 'opencode', 'pi', 'omp', 'agy', 'tailscale']) writeFileSync(join(agents, cli), '#!/bin/sh\nexit 127\n', {mode: 0o755});
const lease = Bun.listen({hostname: '127.0.0.1', port: 0, socket: {data() {}}}); const port = lease.port; lease.stop(true);
const origin = 'http://127.0.0.1:' + port, plain = 'http://hub.example.test:' + port;
let child: any, browser: any, passed = 0;
const check = (ok: unknown, label: string, detail?: unknown) => { if (!ok) throw Error(label + (detail === undefined ? '' : ': ' + JSON.stringify(detail))); console.log('ok ' + label); passed++; };
const pause = (ms: number) => new Promise(r => setTimeout(r, ms));
async function waitFor(fn: () => Promise<any>, label: string, ms = 30000) {
  const end = Date.now() + ms;
  while (Date.now() < end) { if (child?.exitCode != null) throw Error('hub exited'); try { const v = await fn(); if (v) return v; } catch {} await pause(100); }
  throw Error('timeout: ' + label);
}
// a still picture of the QR, black modules on white, centred on a grey
// "desk", as a few frames of 4:2:0 Y4M (luma only matters to a decoder)
function y4m(rows: string[], w = 640, h = 480, scale = 7) {
  const n = rows.length, side = (n + 8) * scale, x0 = (w - side) >> 1, y0 = (h - side) >> 1;
  const Y = new Uint8Array(w * h).fill(90);
  for (let y = 0; y < side; y++) for (let x = 0; x < side; x++) {
    const mx = Math.floor(x / scale) - 4, my = Math.floor(y / scale) - 4;
    const dark = mx >= 0 && my >= 0 && mx < n && my < n && rows[my][mx] === '1';
    Y[(y0 + y) * w + x0 + x] = dark ? 16 : 235;
  }
  const UV = new Uint8Array((w >> 1) * (h >> 1) * 2).fill(128);
  const head = Buffer.from(`YUV4MPEG2 W${w} H${h} F10:1 Ip A1:1 C420jpeg\n`), frame = Buffer.concat([Buffer.from('FRAME\n'), Buffer.from(Y), Buffer.from(UV)]);
  return Buffer.concat([head, frame, frame, frame, frame]);
}
try {
  child = Bun.spawn([resolve(bin), '--home', home, '--web', resolve(web), '--port', String(port), '--host', '127.0.0.1', '--no-tailscale', '--foreground'], {
    cwd: root,
    env: {...process.env, HOME: root, XDG_CACHE_HOME: join(root, '.cache'), XDG_CONFIG_HOME: join(root, '.config'), XDG_DATA_HOME: join(root, '.local/share'), XDG_STATE_HOME: join(root, '.local/state'),
      PATH: agents + ':' + process.env.PATH, BACKPLANE_AGENTS: agents, DISPLAY: '', WAYLAND_DISPLAY: '', BACKPLANE_KISTACK_SYNC: '0', BACKPLANE_NO_UPDATE: '1', BACKPLANE_TELEMETRY: 'off',
      BACKPLANE_PEERS: '', BACKPLANE_VAULT_KEYRING: 'file', BACKPLANE_ORDER_SERVER: 'http://127.0.0.1:9', BACKPLANE_GOOGLE_BAKE: '0'},
    stdout: Bun.file(join(root, 'stdout')), stderr: Bun.file(join(root, 'stderr')),
  });
  await waitFor(async () => (await fetch(origin + '/hello')).status === 200, 'hub ready', 60000);
  const token = (await Bun.file(join(home, 'token')).text()).trim();
  const r = await fetch(origin + '/auth/invitations', {method: 'POST', headers: {Authorization: 'Bearer ' + token}});
  const invite = await r.json();
  check(r.status === 200 && invite.qr && invite.link, 'the hub issues an invitation with a QR code');
  const video = join(root, 'qr.y4m');
  writeFileSync(video, y4m(String(invite.qr).split('\n').filter(Boolean)));
  browser = await chromium.launch({headless: true, executablePath: process.env.CHROMIUM_PATH || '/usr/bin/chromium',
    args: ['--no-sandbox', '--disable-dev-shm-usage', '--use-fake-ui-for-media-stream', '--use-fake-device-for-media-stream', '--use-file-for-fake-video-capture=' + video,
      `--host-resolver-rules=MAP hub.example.test 127.0.0.1`]});
  // 1. the hub's own address (loopback: a secure context): the camera scans and pairs
  const ctx = await browser.newContext({...devices['Pixel 7'], locale: 'en-US', permissions: ['camera']}), page = await ctx.newPage();
  const errors: string[] = [];page.on('pageerror', (e: any) => errors.push(String(e.message)));
  await page.goto(origin + '/');
  await page.getByRole('heading', {name: 'Connect to your Backplane hub'}).waitFor({timeout: 30000});
  const lead = await page.locator('.pairing-lead').innerText();
  check(/Settings → Devices → Add a device/.test(lead) && /QR code/.test(lead), 'the pairing screen says where the QR code is', lead);
  await page.getByRole('tab', {name: 'QR code'}).tap();
  await page.screenshot({path: join(shots, 'scan-1-qr-tab.png')});
  const scan = page.getByRole('button', {name: 'Scan with the camera'});
  check(await scan.isVisible(), 'a secure page offers Scan with the camera');
  check(await page.evaluate(() => isSecureContext), 'loopback is a secure context');
  await scan.tap();
  await page.locator('.pairing-scanner').waitFor({timeout: 10000});
  const box = await page.evaluate(() => { const r = (s: string) => { const e = document.querySelector(s); if (!e) return null; const b = e.getBoundingClientRect(); return {t: b.top, b: b.bottom, l: b.left, r: b.right, w: b.width, h: b.height}; }; return {vw: innerWidth, vh: innerHeight, view: r('.pairing-scanner'), close: r('.pairing-scanner-close'), frame: r('.pairing-scanner-frame'), video: r('.pairing-scanner-video'), say: document.querySelector('.pairing-scanner-say')?.textContent}; });
  check(box.view && box.view.w === box.vw && box.view.h === box.vh, 'the scanner fills the screen', box.view);
  check(box.close && box.close.w >= 44 && box.close.h >= 44, 'Close is a 44 px target', box.close);
  check(/QR code/.test(box.say || ''), 'the scanner says what to aim at', box.say);
  check(await page.evaluate(() => document.activeElement?.classList.contains('pairing-scanner-close') && document.querySelector('.pairing-screen')?.hasAttribute('inert')), 'Close has the focus and the page under it is inert');
  // it reads the fake camera's code, pairs and lands in the app
  await page.waitForURL(origin + '/', {timeout: 30000}).catch(() => {});
  await waitFor(async () => !(await page.locator('.pairing-screen').count()) && !(await page.locator('.pairing-scanner').count()), 'scanned, paired and in the app', 40000);
  check(true, 'the scanner read the code and paired at once');
  const devs = await (await fetch(origin + '/auth/devices', {headers: {Authorization: 'Bearer ' + token}})).json();
  check(devs.devices?.some((d: any) => !d.revoked), 'the hub lists the scanned device', devs.devices?.map((d: any) => d.name));
  const used = devs.invitations?.find((x: any) => x.state === 'used');
  check(!!used, 'the scanned invitation is used');
  await page.waitForTimeout(1500);
  await page.screenshot({path: join(shots, 'scan-3-paired.png')});
  check(errors.length === 0, 'no page errors', errors);
  await ctx.close();
  // 2. the same hub at a plain http:// name: no camera for the page, and it says so
  const ctx2 = await browser.newContext({...devices['Pixel 7'], locale: 'en-US'}), p2 = await ctx2.newPage();
  await p2.goto(plain + '/');
  await p2.getByRole('heading', {name: 'Connect to your Backplane hub'}).waitFor({timeout: 30000});
  check(!(await p2.evaluate(() => isSecureContext)), 'a plain http:// name is not a secure context');
  await p2.getByRole('tab', {name: 'QR code'}).tap();
  check(!(await p2.getByRole('button', {name: 'Scan with the camera'}).count()), 'no camera button where the page has no camera');
  const why = await p2.locator('.pairing-nocam').innerText();
  check(/https:\/\//.test(why) && /camera app/.test(why) && /Short code/.test(why), 'it says why, and the other ways in', why);
  await p2.screenshot({path: join(shots, 'scan-4-http-no-camera.png')});
  await p2.getByRole('button', {name: 'Use Short code'}).tap();
  check(await p2.getByLabel('8-character code').isVisible(), 'Use Short code goes to the code');
  await ctx2.close();
  console.log(`pairing scanner: ${passed} checks passed`);
} finally {
  await browser?.close().catch(() => {});
  if (child && child.exitCode == null) { child.kill(); await child.exited; }
  rmSync(root, {recursive: true, force: true});
}
