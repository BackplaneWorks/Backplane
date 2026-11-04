// The phone model picker's layout, on a real hub and real Chromium phones.
// A private home with one Claude thread; the stand-in `claude` answers the
// hub's model probe with Claude Code's long list (fixtures/claude/
// models-long.json, BP_STANDIN_MODELS), as a real phone's hub had it when
// its rows ran over the effort meter. On each phone size the picker must be
// a sheet with a head (title, a 44 px close) inside the screen, its agent,
// model and effort sections stacked without overlapping, every model row
// inside the model section (folded or not), the later models under More
// models, the chosen effort lit, no sideways scroll; the desktop keeps its
// three-column picker with none of the phone's parts.
//   bun test/tools/phone_model_picker_e2e.ts BACKPLANE-SERVE WEB-DIR [SHOTS-DIR]
import {mkdtempSync, mkdirSync, writeFileSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {createRequire} from 'node:module';
const require = createRequire(import.meta.url);
const {chromium, devices} = require(process.env.PLAYWRIGHT_MODULE || 'playwright-core');
const [bin, web, shotsArg] = process.argv.slice(2);
if (!bin || !web) throw Error('backplane-serve and web directory required');
const repo = resolve(import.meta.dir, '../..');
const shots = resolve(shotsArg || 'build/phone-model-picker');
mkdirSync(shots, {recursive: true});
const root = mkdtempSync(join(tmpdir(), 'bp-mp-e2e-')), home = join(root, 'home'), agents = join(root, 'agents'), proj = join(root, 'proj');
for (const d of [home, agents, proj]) mkdirSync(d, {recursive: true, mode: 0o700});
writeFileSync(join(agents, 'claude'), `#!/bin/sh\nBP_STANDIN_MODELS='${repo}/test/fixtures/claude/models-long.json' exec bun '${repo}/test/tools/claude_standin.ts' "$@"\n`, {mode: 0o755});
for (const cli of ['codex', 'grok', 'muse', 'opencode', 'pi', 'omp', 'agy', 'tailscale']) writeFileSync(join(agents, cli), '#!/bin/sh\nexit 127\n', {mode: 0o755});
const at = Math.floor(Date.now() / 1000), ev = ($: string, rest: object) => JSON.stringify({$, at, ...rest});
writeFileSync(join(home, 'events.jsonl'), [
  ev('ProjectCreated', {id: 'p1', title: 'Picker fixture', root: proj}),
  ev('ThreadCreated', {id: 't1', project: 'p1', title: 'Model picker', env: 'local', branch: '', worktree: '', provider: 'claude', model: 'opus'}),
  ...['onboard.done', 'onboard.uber'].map(key => ev('SettingSet', {key, value: '1'})),
  ...['restart.continue', 'telemetry.enabled', 'mail.on', 'order.enabled'].map(key => ev('SettingSet', {key, value: 'off'})),
].join('\n') + '\n');
const lease = Bun.listen({hostname: '127.0.0.1', port: 0, socket: {data() {}}}); const port = lease.port; lease.stop(true);
const origin = 'http://127.0.0.1:' + port;
let child: any, browser: any, passed = 0;
const check = (ok: unknown, label: string, detail?: unknown) => { if (!ok) throw Error(label + (detail === undefined ? '' : ': ' + JSON.stringify(detail))); console.log('ok ' + label); passed++; };
const pause = (ms: number) => new Promise(r => setTimeout(r, ms));
async function waitFor(fn: () => Promise<any>, label: string, ms = 30000) {
  const end = Date.now() + ms;
  while (Date.now() < end) { if (child?.exitCode != null) throw Error('hub exited'); try { const v = await fn(); if (v) return v; } catch {} await pause(100); }
  throw Error('timeout: ' + label);
}
// (in the page) every box the picker's layout must keep apart or inside
const MEASURE = () => {
  const menu = document.querySelector('.model-menu.mp') as HTMLElement | null;
  if (!menu || !menu.offsetParent && getComputedStyle(menu).position !== 'fixed') return null;
  const box = (e: Element | null) => { if (!e) return null; const r = e.getBoundingClientRect(); return {t: r.top, b: r.bottom, l: r.left, r: r.right, h: r.height, w: r.width}; };
  const shown = (e: Element) => !e.closest('details:not([open])') && (e as HTMLElement).getClientRects().length > 0;
  const sec = (s: string) => box(menu.querySelector(s));
  const rows = [...menu.querySelectorAll('.mp-models .row')].filter(shown).map(r => ({name: r.querySelector('.name')?.textContent, ...box(r)!,
    lines: Math.round((r.querySelector('.label') as HTMLElement).getBoundingClientRect().height / parseFloat(getComputedStyle(r.querySelector('.label')!).lineHeight || '16'))}));
  const steps = [...menu.querySelectorAll('.mp-effort .step')].map(s => ({on: s.classList.contains('on'), bg: getComputedStyle(s).backgroundColor, ...box(s)!}));
  const body = menu.querySelector('.mp-body') as HTMLElement | null;
  return {vw: innerWidth, vh: innerHeight, menu: box(menu), top: sec('.mp-top'), close: sec('.mp-top .mp-x'), title: menu.querySelector('.mp-title')?.textContent,
    bodyBox: sec('.mp-body'), agents: sec('.mp-agents'), models: sec('.mp-models'), effort: sec('.mp-effort'), meter: sec('.mp-effort .meter'), note: sec('.mp-effort .note'),
    more: sec('.mp-more > summary'), moreOpen: !!menu.querySelector('.mp-more[open]'), rows, steps, display: getComputedStyle(menu).display,
    sideways: (body || menu).scrollWidth > (body || menu).clientWidth + 1, bodyScroll: body ? {h: body.scrollHeight, c: body.clientHeight} : null, bodyOverflow: body ? getComputedStyle(body).overflowY : ''};
};
const scrolls = (m: any) => m.bodyOverflow === 'auto' || m.bodyOverflow === 'scroll';
const apart = (a: any, b: any) => a && b && a.b <= b.t + 0.5;
const inside = (x: any, o: any) => x && o && x.t >= o.t - 0.5 && x.b <= o.b + 0.5 && x.l >= o.l - 0.5 && x.r <= o.r + 0.5;
function layout(m: any, tag: string) {
  check(m, tag + ': picker open');
  check(apart(m.agents, m.models), tag + ': agents and models apart', [m.agents, m.models]);
  check(apart(m.models, m.effort), tag + ': models and effort apart', [m.models, m.effort]);
  const out = m.rows.filter((r: any) => !inside(r, m.models));
  check(out.length === 0, tag + ': every model row inside the model section', out.map((r: any) => r.name));
  const over = m.rows.filter((r: any) => r.b > m.effort.t + 0.5 && r.t < m.effort.b - 0.5);
  check(over.length === 0, tag + ': no model row over the effort', over.map((r: any) => r.name));
  check(m.menu.t >= -0.5 && m.menu.b <= m.vh + 0.5, tag + ': sheet within the screen', m.menu);
  check(m.top && m.top.t >= m.menu.t - 0.5 && m.top.t < m.vh / 2 + m.vh / 2, tag + ': sheet has a head', m.top);
  check(m.close && m.close.w >= 44 && m.close.h >= 44 && inside(m.close, m.top), tag + ': head has a 44 px close', m.close);
  check(!m.sideways, tag + ': no sideways scroll');
  // the body scrolls under the head, clipped by it: it starts where the head ends
  check(apart(m.top, m.bodyBox) && m.bodyBox.b <= m.menu.b + 0.5 && scrolls(m), tag + ': the body scrolls under the head', [m.top, m.bodyBox]);
  check(inside(m.meter, m.effort) && inside(m.note, m.effort) && apart(m.meter, m.note), tag + ': effort meter and note inside the effort section');
  check(m.rows.every((r: any) => r.lines <= 2), tag + ': descriptions two lines at most', m.rows.map((r: any) => r.lines));
  const on = m.steps.filter((s: any) => s.on);
  check(on.length === 1 && m.steps.filter((s: any) => !s.on).every((s: any) => s.bg !== on[0].bg), tag + ': chosen effort lit apart from the rest');
  check(m.steps.every((s: any) => s.h >= 44), tag + ': effort segments 44 px tall');
}
const PHONES: [string, any][] = [
  ['pixel7', devices['Pixel 7']],
  ['android-small', {...devices['Pixel 7'], viewport: {width: 360, height: 640}, screen: {width: 360, height: 740}}],
  ['android-short', {...devices['Pixel 7'], viewport: {width: 360, height: 520}, screen: {width: 360, height: 640}}],
  ['iphone-se', {...devices['iPhone SE'], viewport: {width: 375, height: 553}}],
  ['pixel7-land', devices['Pixel 7 landscape']],
];
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
  browser = await chromium.launch({headless: true, executablePath: process.env.CHROMIUM_PATH || '/usr/bin/chromium', args: ['--no-sandbox', '--disable-dev-shm-usage']});
  const openPicker = async (page: any, phone: boolean) => {
    await page.goto(origin + '/#token=' + token);
    await page.waitForFunction(() => { const r = document.getElementById('root'); return r && !r.hasAttribute('inert') && r.childElementCount > 0; }, null, {timeout: 30000});
    if (phone) { const row = page.locator('div.thread').filter({hasText: 'Model picker'}).filter({visible: true}).first(); await row.waitFor({timeout: 20000}); await row.tap(); }
    else await page.locator('.thread, [data-thread]').filter({hasText: 'Model picker'}).filter({visible: true}).first().click();
    const chip = page.locator('button.model-chip').filter({visible: true}).first();
    await chip.waitFor({timeout: 20000});
    // the probe's list arrives a little after the hub starts: reopen until it shows
    await waitFor(async () => {
      if (!(await page.locator('.model-layer[aria-hidden="false"]').count())) { await (phone ? chip.tap() : chip.click()); await pause(400); }
      const n = await page.locator('.model-menu .mp-models .row').count();
      if (n >= 12) return true;
      await page.keyboard.press('Escape'); await page.evaluate(() => (document.querySelector('.model-layer .shade') as HTMLElement | null)?.click()); await pause(700); return false;
    }, 'the long model list in the picker', 40000);
    await pause(300);
  };
  for (const [name, dev] of PHONES) {
    const ctx = await browser.newContext({...dev, locale: 'en-US'}), page = await ctx.newPage();
    await openPicker(page, true);
    await page.screenshot({path: join(shots, name + '-1-open.png')});
    const m = await page.evaluate(MEASURE);
    layout(m, name);
    check(m.title && m.title.length > 3, name + ': head says what the sheet is', m.title);
    check(m.more && !m.moreOpen, name + ': later models folded under More models', m.more);
    check(m.rows.map((r: any) => r.name).join(',') === 'default,opus,claude-fable-5-1,sonnet,haiku', name + ': the common choices first', m.rows.map((r: any) => r.name));
    await page.locator('.mp-more > summary').tap();
    await pause(300);
    const o = await page.evaluate(MEASURE);
    check(o.moreOpen && o.rows.length === 12, name + ': More models unfolds the rest', o.rows.length);
    layout(o, name + ' unfolded');
    await page.screenshot({path: join(shots, name + '-2-more.png')});
    // the whole body scrolls under the head: at its end the effort is in view, the head still on top
    await page.evaluate(() => { const b = document.querySelector('.model-menu .mp-body') as HTMLElement; b.scrollTop = b.scrollHeight; });
    await pause(200);
    const e = await page.evaluate(MEASURE);
    check(e.effort.b <= e.menu.b + 0.5 && e.effort.t >= e.top.b - 0.5 && e.top.t >= e.menu.t - 0.5, name + ': scrolled, the effort shows whole under the head', [e.top, e.effort, e.menu]);
    await page.screenshot({path: join(shots, name + '-3-effort.png')});
    // a model in the fold, chosen: the fold opens with it the next time
    await page.locator('.mp-models .row[data-value="claude-opus-4-8"]').tap();
    await waitFor(async () => (await page.locator('.mp-models .row.on[data-value="claude-opus-4-8"]').count()) === 1, 'claude-opus-4-8 chosen');
    await page.locator('.mp-top .mp-x').tap();
    await waitFor(async () => (await page.locator('.model-layer[aria-hidden="true"]').count()) === 1, 'close shuts the sheet');
    check(true, name + ': close shuts the sheet');
    await page.locator('button.model-chip').filter({visible: true}).first().tap();
    await waitFor(async () => (await page.locator('.model-layer[aria-hidden="false"] .mp-more[open]').count()) === 1, 'fold open on the chosen model', 10000);
    check(true, name + ': the fold opens on a chosen model in it');
    // back to opus for the next phone
    await page.locator('.mp-models .row[data-value="opus"]').tap();
    await waitFor(async () => (await page.locator('.mp-models .row.on[data-value="opus"]').count()) === 1, 'opus chosen again');
    await ctx.close();
  }
  const desk = await browser.newContext({viewport: {width: 1400, height: 900}}), dp = await desk.newPage();
  await openPicker(dp, false);
  await dp.screenshot({path: join(shots, 'desktop-open.png')});
  const d = await dp.evaluate(MEASURE);
  check(d && d.display === 'grid' && !d.top && !d.more, 'desktop keeps its three-column picker, no phone head or fold', d && {display: d.display, top: d.top, more: d.more});
  check(d.rows.length === 12 && apart({b: d.agents.b}, {t: d.agents.b}) && d.models.l >= d.agents.r - 0.5 && d.effort.l >= d.models.r - 0.5, 'desktop columns side by side, every model listed');
  await desk.close();
  console.log(`phone model picker: ${passed} checks passed`);
} finally {
  await browser?.close().catch(() => {});
  if (child && child.exitCode == null) { child.kill(); await child.exited; }
  rmSync(root, {recursive: true, force: true});
}
