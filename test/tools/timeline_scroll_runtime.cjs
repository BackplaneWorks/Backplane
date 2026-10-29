// Private copied-history scroll bench. No user browser, home or provider.
// node test/tools/timeline_scroll_runtime.cjs STAGE ENTRY_JSONL OUT
const fs = require('node:fs'), path = require('node:path'), os = require('node:os');
const assert = require('node:assert/strict');
const { chromium } = require(process.env.PLAYWRIGHT_CORE || '/home/h/.bun/install/cache/playwright-core@1.64.1@@@1');
const { Service } = require('../../desktop/service.cjs');
const [stage, entries, out] = process.argv.slice(2);
const accept = process.argv.includes('--accept');
if (![stage, entries, out].every(p => p && path.isAbsolute(p))) throw Error('Absolute paths required');
fs.mkdirSync(out, { recursive: true });
const home = fs.mkdtempSync(path.join(os.tmpdir(), 'bp-scroll-copy-'));
const project = path.join(home, 'project'), agents = path.join(home, 'agents');
fs.mkdirSync(project); fs.mkdirSync(agents);
for (const name of ['claude', 'codex', 'grok', 'muse', 'agy', 'opencode', 'pi', 'omp']) fs.writeFileSync(path.join(agents, name), '#!/bin/sh\nexit 127\n', { mode: 0o755 });
const events = [
  { $: 'ProjectCreated', id: 'fixture', title: 'Private scroll copy', root: project, at: 1 },
  { $: 'ThreadCreated', id: 'history-copy', project: 'fixture', title: 'Copied long conversation', env: 'local', branch: '', worktree: project, provider: 'codex', model: '', at: 2 },
  ...fs.readFileSync(entries, 'utf8').trim().split('\n').map(JSON.parse),
  ...['restart.continue', 'mail.on', 'telemetry.share'].map(key => ({ $: 'SettingSet', key, value: 'off', at: 1 })),
  { $: 'SettingSet', key: 'onboard.done', value: '1', at: 1 },
];
fs.writeFileSync(path.join(home, 'events.jsonl'), events.map(JSON.stringify).join('\n') + '\n');
Object.assign(process.env, { BACKPLANE_HOME: home, BACKPLANE_AGENTS: agents, BACKPLANE_COPY: '1', BACKPLANE_NO_UPDATE: '1', BACKPLANE_ORDER_SERVER: 'http://127.0.0.1:9', BACKPLANE_TELEMETRY: 'off', BACKPLANE_PEERS: '', BACKPLANE_LOCK_DIR: home + '/locks' });
let service, browser, page;
const result = { stage, entries, home, limits: ['Actual copied entry content; isolated thread/project wrappers, no running agents.', 'Headless Chromium compositor/RAF timing; not a physical display or phone app measurement.'], cases: [] };
function installProbe() {
  localStorage.setItem('backplane-selected', 'history-copy');
  window.__bpPerf = { render: [], act: [], part: [], live: [], trace: [] };
  const probe = window.scrollProbe = { transactions: [], frames: [], shifts: [], active: false };
  if (location.search.includes('scroll-debug')) {
    probe.assignments = [];
    const original = Object.getOwnPropertyDescriptor(Element.prototype, 'scrollTop');
    Object.defineProperty(Element.prototype, 'scrollTop', { ...original, set(value) {
      const before = original.get.call(this);original.set.call(this, value);
      if (this.id === 'timeline' && probe.active) probe.assignments.push({ at: performance.now(), before, requested: value, after: original.get.call(this), stack: new Error().stack.split('\n').slice(1, 5) });
    } });
  }
  const key = el => el.dataset.id || (el.matches('button.fold') ? el.__v?.key || 'fold:' + el.dataset.value.slice(1) : '');
  function rows(tl) {
    const box = tl.getBoundingClientRect();
    return [...tl.children].filter(el => key(el)).map(el => ({ el, id: key(el), rect: el.getBoundingClientRect() }))
      .filter(x => x.rect.height > 0 && x.rect.bottom > box.top + 1 && x.rect.top < box.bottom);
  }
  function sourceAnchor(tl) {
    const top = tl.getBoundingClientRect().top;
    const el = [...tl.querySelectorAll('[data-id],button.fold')].find(el => { const r=el.getBoundingClientRect();return r.height>0 && r.bottom>top+1; });
    return el && { id: key(el), top: el.getBoundingClientRect().top, parent: el.parentElement.id };
  }
  let pending, patchRows;
  // The host reads root.__v immediately before the entire keyed patch,
  // after Bend view computation. A compositor scroll may advance during
  // that computation; it is user motion, not prepend drift.
  const vnodes = new WeakMap();
  Object.defineProperty(Node.prototype, '__v', { configurable: true,
    get() {
      if (probe.active && this.id === 'root') {
        const tl = document.getElementById('timeline');patchRows = tl ? rows(tl) : null;
        queueMicrotask(() => { patchRows = null; });
      }
      return vnodes.get(this);
    }, set(v) { vnodes.set(this, v); },
  });
  function before(parent, node, op) {
    const tl = document.getElementById('timeline');
    if (!probe.active || !tl || !(parent === tl || tl.contains(parent))) return;
    if (!pending) {
      pending = { at: performance.now(), top: tl.scrollTop, rows: patchRows || rows(tl), source: sourceAnchor(tl), direct: [...tl.children], operations: [] };
      queueMicrotask(() => {
        const p = pending; pending = null;
        const current = [...tl.children];
        const candidates = p.rows.map(x => ({ ...x, current: current.find(el => key(el) === x.id) })).filter(x => x.current);
        const first = candidates[0];
        probe.transactions.push({ at: p.at, phase: probe.phase, topBefore: p.top, topAfter: tl.scrollTop, sourceBefore: p.source, sourceAfter: sourceAnchor(tl),
          anchor: first?.id, drift: first ? first.current.getBoundingClientRect().top - first.rect.top : null,
          sameNode: first ? first.current === first.el : null,
          lostVisible: p.rows.filter(x => !current.some(el => key(el) === x.id)).map(x => x.id),
          operations: p.operations, rows: tl.children.length });
      });
    }
    if (parent === tl) pending.operations.push({ op, id: node.nodeType === 1 ? key(node) : '', kept: pending.direct.includes(node) });
  }
  for (const name of ['insertBefore', 'appendChild', 'removeChild', 'replaceChild']) {
    const original = Node.prototype[name];
    Node.prototype[name] = function(node, ...args) { before(this, node, name); return original.call(this, node, ...args); };
  }
  let last;
  function tick(t) { if (last && probe.active) probe.frames.push(t - last); last = t; requestAnimationFrame(tick); }
  requestAnimationFrame(tick);
  try { new PerformanceObserver(l => { if (probe.active) probe.shifts.push(...l.getEntries().map(e => ({ value: e.value, input: e.hadRecentInput }))); }).observe({ type: 'layout-shift', buffered: false }); } catch {}
}
function sample() {
  const tl = document.getElementById('timeline'), r = tl.getBoundingClientRect();
  const rows = [...tl.children].filter(e => e.dataset.id || e.matches('button.fold'));
  const anchor = rows.find(e => e.getBoundingClientRect().bottom > r.top + 1);
  return { top: tl.scrollTop, height: tl.scrollHeight, viewport: tl.clientHeight, rows: rows.length,
    anchor: anchor && { id: anchor.dataset.id || anchor.dataset.value, offset: anchor.getBoundingClientRect().top - r.top },
    sends: __bpPerf.trace.filter(x => x.phase === 'send' && x.method === 'view.page').length,
    pages: __bpPerf.trace.filter(x => x.phase === 'thin-view').length,
    draft: document.getElementById('composer')?.value, documentHeight: document.documentElement.scrollHeight, viewportHeight: innerHeight };
}
async function settled(page) {
  await page.waitForFunction(() => {
    const sends = __bpPerf.trace.filter(x => x.phase === 'send' && x.method.startsWith('view.'));
    const views = __bpPerf.trace.filter(x => x.phase === 'thin-view');
    return !document.getElementById('history-status') && (!sends.length || performance.now() - sends.at(-1).ms > 200)
      && (!sends.length || views.at(-1)?.ms > sends.at(-1).ms);
  });
  await page.evaluate(() => new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))));
}
(async () => {
  try {
    browser = await chromium.launch({ headless: true, executablePath: process.env.CHROMIUM || '/home/h/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome', args: ['--no-sandbox', '--disable-dev-shm-usage'] });
    for (const viewport of (process.env.SCROLL_DEBUG ? [{ width: 390, height: 740 }] : [{ width: 1440, height: 900 }, { width: 390, height: 740 }])) {
      // Each viewport starts from its own copied hub as well as browser.
      // Cursor/reading state from the first run is not a second-run input.
      const caseHome = fs.mkdtempSync(path.join(os.tmpdir(), 'bp-scroll-' + viewport.width + '-'));
      fs.mkdirSync(path.join(caseHome, 'project'));
      fs.writeFileSync(path.join(caseHome, 'events.jsonl'), events.map(e => JSON.stringify(e.$ === 'ProjectCreated' ? { ...e, root: path.join(caseHome, 'project') }
        : e.$ === 'ThreadCreated' ? { ...e, worktree: path.join(caseHome, 'project') } : e)).join('\n') + '\n');
      service = new Service({ home: caseHome, binary: path.join(stage, 'backplane-serve'), web: path.join(stage, 'web'), expectedVersion: fs.readFileSync(path.join(stage, 'VERSION'), 'utf8').trim() });
      const connection = await service.start();
      const context = await browser.newContext({ viewport });
      await context.request.post(connection.origin + '/auth/pair', { headers: { Authorization: 'Bearer ' + await service.token() } });
      page = await context.newPage(); page.setDefaultTimeout(18000);
      const errors = []; page.on('pageerror', e => errors.push(String(e)));
      await page.addInitScript(installProbe);
      await page.goto(connection.origin + (process.env.SCROLL_DEBUG ? '/?scroll-debug' : ''));
      await page.waitForFunction(() => document.documentElement.dataset.backplaneRemoteReady === 'true' && document.getElementById('composer'));
      if (await page.locator('[data-on-click="ob-skip"]').count()) await page.locator('[data-on-click="ob-skip"]').click();
      await page.waitForFunction(() => !document.querySelector('.app.tour-on'));
      await page.locator('#composer').fill('Unsent scroll copy 日本語 👩‍💻');
      await settled(page);
      await page.evaluate(() => { window.keptComposer = document.getElementById('composer');keptComposer.setSelectionRange(3, 8);scrollProbe.active = true; });
      const cdp = await context.newCDPSession(page);
      await cdp.send('Tracing.start', { categories: 'devtools.timeline,benchmark', transferMode: 'ReturnAsStream' });
      const start = await page.evaluate(sample);
      const box = await page.locator('#timeline').boundingBox();
      await page.mouse.move(box.x + box.width / 2, box.y + 40);
      // At-rest prepend drift and continuous input are different readings.
      // A trusted notch at the actual top cannot advance the compositor;
      // subsequent flings measure frame delivery without attributing user
      // motion during JS execution to a layout-induced anchor displacement.
      await page.evaluate(() => { scrollProbe.phase = 'prepend'; });
      for (let n = 0; n < 5; n++) {
        await settled(page);
        await page.evaluate(() => { document.getElementById('timeline').scrollTop = 0; });
        await page.evaluate(() => new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))));
        const before = await page.evaluate(sample);
        await page.mouse.wheel(0, -1);
        await page.waitForFunction(n => __bpPerf.trace.filter(x => x.phase === 'thin-view').length > n, before.pages, { timeout: process.env.SCROLL_DEBUG ? 3000 : 18000 });
        await page.evaluate(() => new Promise(requestAnimationFrame));
      }
      await page.evaluate(() => { scrollProbe.phase = 'fling'; });
      for (const yDistance of [18000, -12000, 18000, -18000]) {
        await cdp.send('Input.synthesizeScrollGesture', { x: Math.round(box.x + box.width / 2), y: Math.round(box.y + box.height / 2), yDistance, speed: 3500, gestureSourceType: 'mouse' });
      }
      await page.evaluate(() => new Promise(requestAnimationFrame));
      const end = await page.evaluate(sample);
      const probe = await page.evaluate(() => ({ ...scrollProbe, composerKept: keptComposer === document.getElementById('composer'), selectionKept: keptComposer.selectionStart === 3 && keptComposer.selectionEnd === 8, perf: __bpPerf }));
      if (accept) {
        const prepends = probe.transactions.filter(t => t.phase === 'prepend');
        assert(prepends.length >= 5, 'at least five observed prepend transactions');
        assert(prepends.every(t => t.drift !== null && Math.abs(t.drift) <= 0.5), 'prepend drift exceeds subpixel tolerance');
        assert.equal(end.draft, start.draft);assert(probe.composerKept);assert(probe.selectionKept);
        assert.equal(errors.length, 0);assert(end.documentHeight <= end.viewportHeight);
        assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'horizontal page overflow');
      }
      result.cases.push({ viewport, start, end, errors, ...probe });
      fs.writeFileSync(path.join(out, 'MEASUREMENT.json'), JSON.stringify(result, null, 2));
      const completed = new Promise(resolve => cdp.once('Tracing.tracingComplete', resolve));
      await cdp.send('Tracing.end');const { stream } = await completed;
      const chunks = [];let eof = false;
      while (!eof) { const p = await cdp.send('IO.read', { handle: stream, size: 1048576 });chunks.push(p.base64Encoded ? Buffer.from(p.data, 'base64') : Buffer.from(p.data));eof = p.eof; }
      await cdp.send('IO.close', { handle: stream });
      fs.writeFileSync(path.join(out, `trace-${viewport.width}.json`), Buffer.concat(chunks));
      await page.screenshot({ path: path.join(out, `thread-${viewport.width}.png`) });
      console.log(JSON.stringify({ viewport, pages: end.sends - start.sends, transactions: probe.transactions.length, maxDrift: Math.max(0, ...probe.transactions.filter(t => t.drift != null).map(t => Math.abs(t.drift))), frames: probe.frames.length, longFrames: probe.frames.filter(x => x > 25).length, composerKept: probe.composerKept }));
      await context.close();
      await service.stop();service = null;
    }
    result.state = 'measured';
  } catch (e) { result.state = 'failed';result.error = e.stack;process.exitCode = 1; if (page) { try { result.failure = await page.evaluate(sample);result.diagnostic = await page.evaluate(() => ({ probe: scrollProbe, perf: __bpPerf, mark: document.querySelector('[data-earlier]')?.getAttribute('data-earlier'), ready: document.documentElement.dataset.backplaneRemoteReady, scripts: [...document.scripts].map(x=>x.src) }));await page.screenshot({ path: path.join(out, 'failure.png') }); } catch {} } }
  finally { if (browser) await browser.close();if (service) await service.stop();fs.writeFileSync(path.join(out, 'MEASUREMENT.json'), JSON.stringify(result, null, 2));console.log(JSON.stringify({ state: result.state, error: result.error, out })); }
})();
