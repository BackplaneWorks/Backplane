// Split and cross-probing, end to end (skills/backplane-e2e/stories.md S33):
// the real web client in headless Chromium against an isolated hub whose
// thread has a KiCad project (skills/backplane-e2e/hub.sh with
// BP_E2E_FIXTURE, or a copy of real data). Never a person's own hub.
//   node test/tools/split_e2e.mjs --url http://127.0.0.1:PORT --token TOKEN --thread THREAD [--out DIR]
//   (--playwright PATH: playwright-core's index.mjs; default tools/browser's)
// A desktop window (1600x1000): Split draws the schematic, the board and the
// 3D at once; × puts one away and the bar's toggle brings it back; a part
// clicked on the board is the probe of all three panes, with no panel
// opened over the 3D; Esc clears it. A phone (iPhone 15): two panes, the
// schematic and the board, and 3D takes the board's place.
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
const args = process.argv.slice(2), option = (k, d = '') => { const i = args.indexOf(`--${k}`); return i < 0 ? d : args[i + 1]; };
const url = option('url'), token = option('token'), thread = option('thread'), out = option('out', '');
assert.ok(url && token && thread, '--url, --token and --thread are required');
assert.ok(!/:(3787|57808)\b/.test(url), 'refusing a person\'s own hub port');
const pw = await import(pathToFileURL(resolve(option('playwright', process.env.PLAYWRIGHT_CORE_PATH || 'tools/browser/node_modules/playwright-core/index.mjs'))).href);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const browser = await pw.chromium.launch({ executablePath: process.env.BACKPLANE_CHROME || '/usr/bin/chromium', headless: true, args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader'] });
const failures = [];
const check = (ok, what) => { console.log(`${ok ? 'ok' : 'FAIL'} ${what}`); if (!ok) failures.push(what); };
const waitFor = async (page, fn, arg, ms = 20000) => { try { await page.waitForFunction(fn, arg, { timeout: ms }); return true; } catch { return false; } };
// the panes on show, by their head's pane word, with their size
const panes = (page) => page.evaluate(() => [...document.querySelectorAll('.combo .combo-pane')].map((e) => { const r = e.getBoundingClientRect(); return { id: e.id, w: Math.round(r.width), h: Math.round(r.height) }; }).filter((p) => p.w > 0 && p.h > 0));
const shown = async (page) => (await panes(page)).map((p) => p.id).sort().join(' ');
const probes = (page) => page.evaluate(() => Object.fromEntries(['plot-sch', 'plot-pcb', 'solid'].map((id) => [id, document.getElementById(id)?.dataset.probe ?? null])));
const snap = async (page, name) => { if (out) { fs.mkdirSync(out, { recursive: true }); await page.screenshot({ path: `${out}/${name}.png` }); } };

async function openSplit(page, phone) {
  await page.goto(`${url}/#token=${encodeURIComponent(token)}`);
  await waitFor(page, () => !!document.querySelector('.sidebar, .app.phone'));
  // the thread, from its sidebar row (on a phone: in Projects), opening the
  // shelves and projects it may be folded under
  if (phone) await page.getByRole('button', { name: 'Projects' }).first().tap().catch(() => {});
  await waitFor(page, () => document.querySelectorAll('.thread[data-value]').length > 0, null, 30000);
  await sleep(1000);
  const row = page.locator(`.thread[data-value="${thread}"]`).filter({ visible: true });
  for (let i = 0; i < 60 && !(await row.count()); i++) {
    let shut = page.locator('button.shelf:not(.open)').filter({ visible: true });
    if (!(await shut.count())) shut = page.locator('button.project-disclosure:has(svg[data-icon="chevron-right"])').filter({ visible: true });
    if (!(await shut.count())) break;
    await shut.first().click().catch(() => {}); await sleep(400);
  }
  assert.ok(await row.count(), `no sidebar row for thread ${thread}`);
  await (phone ? row.first().tap() : row.first().click());
  await waitFor(page, (t) => !!document.querySelector(`main.thread-view[data-thread="${t}"]`), thread);
  if (phone) {
    await page.locator('summary[aria-label="Views"]').filter({ visible: true }).first().tap();
    await waitFor(page, () => !!document.querySelector('details[open] button.tab[data-value="split"]'), null, 8000);
    await page.locator('details[open] button.tab[data-value="split"]').first().tap();
  }
  else await page.locator('nav.workspace-bar button.tab[aria-label="Split"]').first().click();
  return waitFor(page, () => !!document.querySelector('.viewer.split .combo .combo-pane'));
}

// ---- a desktop window
{
  const page = await (await browser.newContext({ viewport: { width: 1600, height: 1000 } })).newPage();
  check(await openSplit(page, false), 'desktop: Split opens');
  // (a setting left by an earlier run: every pane brought back first)
  for (const p of ['sch', 'pcb', '3d']) if (!(await shown(page)).includes('combo-' + p)) { await page.locator(`.combo-pick[data-value="${p}"]`).first().click(); await sleep(1500); }
  await waitFor(page, () => document.querySelectorAll('.combo .combo-pane').length === 3, null, 15000);
  const three = await panes(page);
  check(three.length === 3 && three.every((p) => p.w >= 140), `desktop: the schematic, the board and the 3D at once (${JSON.stringify(three)})`);
  await snap(page, 'desktop-split');
  await page.locator('#combo-3d .combo-hide').first().click();
  check(await waitFor(page, () => !document.getElementById('combo-3d')), 'desktop: × puts the 3D away');
  check((await shown(page)) === 'combo-pcb combo-sch', 'desktop: the schematic and the board stay');
  await page.locator('.combo-pick[data-value="3d"]').first().click();
  check(await waitFor(page, () => !!document.getElementById('combo-3d') && !!document.getElementById('combo-pcb')), 'desktop: the bar\'s 3D brings it back beside the board');
  // cross-probing: a part clicked on the board is every pane's probe
  // (a probe left by an earlier run cleared first; Esc with none shuts the viewer)
  if (Object.values(await probes(page)).some(Boolean)) { await page.keyboard.press('Escape'); await sleep(800); }
  await waitFor(page, () => document.getElementById('plot-pcb')?.dataset.probe !== undefined, null, 20000);
  const b = await page.evaluate(() => { const r = document.getElementById('plot-pcb').getBoundingClientRect(); return [r.x, r.y, r.width, r.height]; });
  let probe = '';
  for (let i = 0; i < 49 && !probe; i++) {
    const fx = 0.2 + 0.1 * (i % 7), fy = 0.2 + 0.1 * Math.floor(i / 7);
    await page.mouse.click(b[0] + b[2] * fx, b[1] + b[3] * fy); await sleep(700);
    probe = (await probes(page))['plot-pcb'] || '';
  }
  check(!!probe, `desktop: a click on the board probes something (${JSON.stringify(probe)})`);
  await waitFor(page, (p) => ['plot-sch', 'solid'].every((id) => document.getElementById(id)?.dataset.probe === p), probe, 10000);
  const all = await probes(page);
  check(!!probe && all['plot-sch'] === probe && all.solid === probe, `desktop: the schematic and the 3D probe it too (${JSON.stringify(all)})`);
  await sleep(1500);
  const covered = await page.evaluate(() => {
    const pane = document.getElementById('combo-3d')?.getBoundingClientRect();
    const card = document.querySelector('.ph-inspect');
    const inspector = [...document.querySelectorAll('#combo-3d .solid-panel-head')].find((e) => e.getClientRects().length && /Components/.test(e.textContent));
    return { card: !!card, components: !!inspector, pane: pane ? Math.round(pane.height) : 0 };
  });
  check(!covered.card && !covered.components, `desktop: no panel opens over the panes (${JSON.stringify(covered)})`);
  await snap(page, 'desktop-probe');
  await page.keyboard.press('Escape');
  check(!!probe && await waitFor(page, () => !document.getElementById('plot-pcb')?.dataset.probe, null, 8000), 'desktop: Esc clears the probe');
  // a part clicked in the 3D: the schematic and the board probe it, and the
  // Components table stays shut (it took half the 3D pane)
  const d = await page.evaluate(() => { const r = document.getElementById('solid').getBoundingClientRect(); return [r.x, r.y, r.width, r.height]; });
  let part = '';
  for (let i = 0; i < 25 && !part; i++) {
    const fx = 0.3 + 0.1 * (i % 5), fy = 0.3 + 0.1 * Math.floor(i / 5);
    await page.mouse.click(d[0] + d[2] * fx, d[1] + d[3] * fy); await sleep(900);
    part = (await probes(page)).solid || '';
  }
  check(!!part, `desktop: a click on the 3D probes a part (${JSON.stringify(part)})`);
  await waitFor(page, (p) => ['plot-sch', 'plot-pcb'].every((id) => document.getElementById(id)?.dataset.probe === p), part, 10000);
  const both = await probes(page);
  check(!!part && both['plot-sch'] === part && both['plot-pcb'] === part, `desktop: the schematic and the board probe it too (${JSON.stringify(both)})`);
  await sleep(1500);
  const table = await page.evaluate(() => [...document.querySelectorAll('#combo-3d .solid-panel-head')].some((e) => e.getClientRects().length && /Components/.test(e.textContent)));
  check(!table, 'desktop: the 3D keeps its whole pane (no Components table)');
  await snap(page, 'desktop-probe-3d');
  await page.keyboard.press('Escape');
  await page.context().close();
}

// ---- a phone
{
  const context = await browser.newContext({ ...pw.devices['iPhone 15'] });
  const page = await context.newPage();
  check(await openSplit(page, true), 'phone: Split opens');
  await waitFor(page, () => document.documentElement.classList.contains('phone'), null, 5000);
  await waitFor(page, () => document.querySelectorAll('.combo .combo-pane').length >= 2, null, 15000);
  const two = await shown(page);
  check(two === 'combo-pcb combo-sch', `phone: the schematic and the board (${two})`);
  await page.locator('.combo-pick[data-value="3d"]').first().tap();
  check(await waitFor(page, () => !!document.getElementById('combo-3d') && !document.getElementById('combo-pcb')), 'phone: 3D takes the board\'s place');
  await snap(page, 'phone-3d');
  await page.locator('.combo-pick[data-value="pcb"]').first().tap();
  check(await waitFor(page, () => !!document.getElementById('combo-pcb') && !document.getElementById('combo-3d')), 'phone: and the board takes it back');
  await context.close();
}
await browser.close();
if (failures.length) { console.log(`${failures.length} failed`); process.exit(1); }
console.log('split_e2e: ok');
