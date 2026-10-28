// Isolated production history bench. No user home, browser profile or window.
// node test/tools/upward_history_runtime.cjs SAFE_STAGE WEB OUT [baseline|candidate]
const fs = require('node:fs'), path = require('node:path'), os = require('node:os');
const assert = require('node:assert/strict');
const { chromium } = require(process.env.PLAYWRIGHT_CORE || '/home/h/.bun/install/cache/playwright-core@1.64.1@@@1');
const { Service } = require('../../desktop/service.cjs');
const [stage, web, out, mode = 'baseline'] = process.argv.slice(2);
const snapshotOnly = process.env.SNAPSHOT_ONLY === '1';
if (![stage, web, out].every(p => p && path.isAbsolute(p))) throw Error('Absolute stage, web and output paths required');
fs.mkdirSync(out, { recursive: true });
const result = { mode, stage, web, cases: [], limits: ['Disposable generated history; production server and frontend.', 'Headless Chromium; physical mobile inertia and native IME engines not measured.'] };
const home = fs.mkdtempSync(path.join(os.tmpdir(), 'bp-upward-' + mode + '-'));
const project = path.join(home, 'project'), agents = path.join(home, 'agents');
fs.mkdirSync(project);fs.mkdirSync(agents);
for (const name of ['claude', 'codex', 'grok', 'muse', 'agy', 'opencode', 'pi', 'omp']) fs.writeFileSync(path.join(agents, name), '#!/bin/sh\nexit 127\n', { mode: 0o755 });
let at = Math.floor(Date.now() / 1000) - 40000, events = [];
const event = ($, data) => events.push({ $, ...data, at: at++ });
event('ProjectCreated', { id: 'fixture', title: 'History bench', root: project });
for (const [id, count] of [['long', 4096], ['short', 80]]) {
  event('ThreadCreated', { id, project: 'fixture', title: 'Generated ' + id, env: 'local', branch: '', worktree: project, provider: 'codex', model: '' });
  for (let i = 0; i < count; i++) {
    const msg = id + '-' + String(i).padStart(6, '0');
    // Both long Markdown messages and folded tool runs, without private text.
    if (i % 12 > 3) event('ActivityLogged', { thread: id, act: msg, tone: 'tool', kind: 'shell', summary: ('Generated tool output ' + i + '\n').repeat(35) });
    else event('MessagePosted', { thread: id, msg, role: i % 2 ? 'user' : 'assistant', text: i % 2 ? ('Question ' + i + '. ').repeat(12) : ('Generated answer ' + i + '.\n\n- Detail one\n- Detail two\n\n```js\nconst value = 42;\n```\n\n').repeat(i % 24 ? 6 : 24) });
  }
}
for (const key of ['restart.continue', 'mail.on', 'telemetry.share']) event('SettingSet', { key, value: 'off' });
event('SettingSet', { key: 'onboard.done', value: '1' });
const contents = events.map(JSON.stringify).join('\n') + '\n';events = null;
fs.writeFileSync(path.join(home, 'events.jsonl'), contents);
Object.assign(result, { home, historyRows: 4176, historyBytes: Buffer.byteLength(contents) });
Object.assign(process.env, { BACKPLANE_HOME: home, BACKPLANE_AGENTS: agents, BACKPLANE_COPY: '1', BACKPLANE_NO_UPDATE: '1', BACKPLANE_ORDER_SERVER: 'http://127.0.0.1:9', BACKPLANE_TELEMETRY: 'off', BACKPLANE_PEERS: '', BACKPLANE_LOCK_DIR: home + '/locks' });
let service, browser, page;
const frame = page => page.evaluate(() => new Promise(requestAnimationFrame));
function sample() {
  const tl = document.getElementById('timeline'), box = tl.getBoundingClientRect();
  const rows = [...tl.querySelectorAll('[data-id]')].filter(e => e.getBoundingClientRect().height > 0);
  const visible = rows.filter(e => { const r = e.getBoundingClientRect();return r.bottom > box.top + 1 && r.top < box.bottom; });
  const anchor = visible[0];
  return { top: tl.scrollTop, height: tl.scrollHeight, viewport: tl.clientHeight, rows: rows.map(e => e.dataset.id), nodes: tl.querySelectorAll('*').length,
    anchor: anchor && { id: anchor.dataset.id, offset: anchor.getBoundingClientRect().top - box.top },
    documentHeight: document.documentElement.scrollHeight, innerHeight, memory: performance.memory?.usedJSHeapSize,
    pages: __bpPerf.trace.filter(x => x.phase === 'thin-view').length, sends: __bpPerf.trace.filter(x => x.phase === 'send' && x.method === 'view.page').length,
    ready: document.documentElement.dataset.backplaneRemoteReady,
    composer: document.getElementById('composer')?.value };
}
(async () => {
  try {
    const version = JSON.parse(fs.readFileSync(path.join(stage, 'LOCAL.json'))).version;
    service = new Service({ home, binary: path.join(stage, 'payload/backplane-serve'), web, expectedVersion: version });
    const started = performance.now(), connection = await service.start();result.serviceReadyMs = performance.now() - started;
    browser = await chromium.launch({ headless: true, executablePath: process.env.CHROMIUM || '/home/h/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome', args: ['--no-sandbox', '--disable-dev-shm-usage', '--enable-precise-memory-info'] });
    const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
    await context.request.post(connection.origin + '/auth/pair', { headers: { Authorization: 'Bearer ' + await service.token() } });
    page = await context.newPage();const errors = [];page.setDefaultTimeout(15000);
    let dropReply = false, dropped = null, socketRoute;
    await page.routeWebSocket(/\/ws\?/, ws => {
      socketRoute = ws;const server = ws.connectToServer();
      ws.onMessage(m => server.send(m));
      server.onMessage(m => { if (dropReply && Buffer.isBuffer(m) && m.length > 2048) { dropReply = false;dropped = m;return; }ws.send(m); });
    });
    page.on('pageerror', e => errors.push(String(e)));
    await page.addInitScript(() => {
      localStorage.setItem('backplane-selected', 'long');
      window.__bpPerf = { render: [], act: [], part: [], live: [], trace: [], frames: [] };
      let last;function tick(t) { if (last) __bpPerf.frames.push(t - last);last = t;requestAnimationFrame(tick); }requestAnimationFrame(tick);
    });
    const start = performance.now();await page.goto(connection.origin);
    await page.waitForFunction(() => document.documentElement.dataset.backplaneRemoteReady === 'true' && document.getElementById('composer'));
    result.initialViewMs = performance.now() - start;
    const skip = page.locator('[data-on-click="ob-skip"]');if (await skip.count()) await skip.click();
    await page.waitForFunction(() => !document.querySelector('.app.tour-on'));
    await page.waitForFunction(() => document.getElementById('composer').getAttribute('data-on-input') === 'draft');
    const draft = 'Preserved unsent 日本語 👩‍💻';await page.locator('#composer').fill(draft);
    await page.waitForFunction(d => document.getElementById('composer').value === d, draft);
    await page.waitForTimeout(180);
    await page.evaluate(() => { window.fixtureComposer = document.getElementById('composer');fixtureComposer.setSelectionRange(3, 8);fixtureComposer.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true })); });
    result.initial = await page.evaluate(sample);
    const cdp = await context.newCDPSession(page);await cdp.send('HeapProfiler.collectGarbage');
    result.initialHeap = await cdp.send('Runtime.getHeapUsage');result.initialDom = await cdp.send('Memory.getDOMCounters');
    for (let i = 0; i < (snapshotOnly ? 2 : 12); i++) {
      // Scroll only the disposable test page. A single notch at the boundary.
      const tl = await page.locator('#timeline').boundingBox();
      await page.mouse.move(tl.x + tl.width / 2, tl.y + 10);
      const t = performance.now();
      const before = await page.evaluate(fn => { document.getElementById('timeline').scrollTop = 0;return (0, eval)('(' + fn + ')()'); }, sample.toString());
      await page.mouse.wheel(0, -120);
      await page.waitForFunction(n => __bpPerf.trace.filter(x => x.phase === 'send' && x.method === 'view.page').length > n, before.sends);
      await page.waitForFunction(n => __bpPerf.trace.filter(x => x.phase === 'thin-view').length > n, before.pages);
      await frame(page);await frame(page);
      const after = await page.evaluate(sample);
      const offset = before.anchor && await page.evaluate(id => { const tl = document.getElementById('timeline'), el = tl.querySelector(`[data-id="${CSS.escape(id)}"]`);return el ? el.getBoundingClientRect().top - tl.getBoundingClientRect().top : null; }, before.anchor.id);
      result.cases.push({ page: i, latencyMs: performance.now() - t, before, after, anchorDelta: offset === null ? null : offset - before.anchor.offset });
      assert.equal(new Set(after.rows).size, after.rows.length, 'duplicate rows');
      assert(after.rows.every((id, n, a) => !n || id > a[n - 1]), 'out of order rows');
      assert.equal(after.composer, draft, 'draft changed');assert(after.documentHeight <= after.innerHeight + 1, 'viewport escaped');
      if (mode === 'candidate') assert.equal(result.cases.at(-1).anchorDelta, 0, 'visible message anchor moved');
    }
    await page.evaluate(() => new Promise(r => setTimeout(r, 150)));
    result.final = await page.evaluate(sample);
    result.composerKept = await page.evaluate(() => fixtureComposer === document.getElementById('composer') && fixtureComposer.selectionStart === 3 && fixtureComposer.selectionEnd === 8);
    await cdp.send('HeapProfiler.collectGarbage');result.finalHeap = await cdp.send('Runtime.getHeapUsage');result.finalDom = await cdp.send('Memory.getDOMCounters');
    if (mode === 'candidate' && !snapshotOnly) {
      const box = await page.locator('#timeline').boundingBox();await page.mouse.move(box.x + box.width / 2, box.y + box.height - 30);
      const before = await page.evaluate(fn => { const tl = document.getElementById('timeline');tl.scrollTop = tl.scrollHeight - tl.clientHeight;return (0, eval)('(' + fn + ')()'); }, sample.toString());
      await page.mouse.wheel(0, 120);let loaded = true;
      try { await page.waitForFunction(n => __bpPerf.trace.filter(x => x.phase === 'send' && x.method === 'view.page').length > n, before.sends, { timeout: 1500 }); } catch { loaded = false; }
      if (loaded) await page.waitForFunction(n => __bpPerf.trace.filter(x => x.phase === 'thin-view').length > n, before.pages);
      await frame(page);await frame(page);result.reversal = { before, after: await page.evaluate(sample), loaded };
      assert(loaded, 'downward reversal blocked while reading retained newer rows');
      if (process.env.REVERSAL_ONLY === '1') { result.state = 'passed';return; }
      // A reversal can intentionally continue into the next short page. Let
      // that accepted serial request settle before arming a fault on another
      // request; otherwise the test compares two different valid windows.
      await page.waitForFunction(() => {
        const last = [...__bpPerf.trace].reverse().find(x => x.phase === 'send' && x.method === 'view.page');
        return !document.getElementById('history-status') && (!last || performance.now() - last.ms > 160);
      });
    }
    if (mode === 'candidate' && snapshotOnly) {
      await page.evaluate(() => {
        const tl=document.getElementById('timeline'), rows=[...tl.querySelectorAll('.msg.assistant[data-id]')], row=rows.at(-3);
        if (row) tl.scrollTop+=row.getBoundingClientRect().top-tl.getBoundingClientRect().top+Math.min(700,row.clientHeight-100);
      });await frame(page);
      await page.evaluate(fn => {
        const snapshot=(0,eval)('('+fn+')');document.addEventListener('click',e=>{if(e.target.closest?.('[data-on-click="select"][data-value="short"]'))window.fixtureNavLeaving=snapshot();},true);
      },sample.toString());
      await page.locator('[data-on-click="select"][data-value="short"]:visible').first().click();const before=await page.evaluate(()=>fixtureNavLeaving);
      await page.waitForFunction(()=>document.documentElement.dataset.backplaneRemoteReady==='true'&&document.querySelector('main[data-thread="short"]'));
      await page.locator('[data-on-click="select"][data-value="long"]:visible').first().click();
      await page.waitForFunction(()=>document.documentElement.dataset.backplaneRemoteReady==='true'&&document.querySelector('main[data-thread="long"]'));await frame(page);await frame(page);
      const after=await page.evaluate(sample);assert.equal(after.anchor.id,before.anchor.id);assert(Math.abs(after.anchor.offset-before.anchor.offset)<=1);assert.equal(after.composer,draft);
      result.snapshotNavigation={before,after};await cdp.send('HeapProfiler.collectGarbage');result.scenarioHeap=await cdp.send('Runtime.getHeapUsage');result.scenarioDom=await cdp.send('Memory.getDOMCounters');
    }
    if (mode === 'candidate' && !snapshotOnly) {
      // A late/drop fault on the next large server frame. Only this generated
      // socket is intercepted; the original timed-out reply is released later.
      dropReply = true;dropped = null;
      const before = await page.evaluate(fn => { document.getElementById('timeline').scrollTop = 0;return (0, eval)('(' + fn + ')()'); }, sample.toString());
      await page.mouse.wheel(0, -120);
      await page.locator('#history-status button').waitFor();
      const failed = await page.evaluate(sample);assert(dropped, 'no page reply was dropped');assert.equal(failed.ready, 'true');assert.equal(failed.composer, draft);
      assert.deepEqual(failed.rows, before.rows);assert.equal(failed.top, before.top);
      await page.locator('#history-status button').click();
      await page.waitForFunction(n => __bpPerf.trace.filter(x => x.phase === 'thin-view').length > n, failed.pages);await frame(page);await frame(page);
      const retried = await page.evaluate(sample);socketRoute.send(dropped);await frame(page);await frame(page);
      assert.deepEqual((await page.evaluate(sample)).rows, retried.rows, 'late timed-out reply replaced retry');
      result.recovery = { before, failed, retried, lateIgnored: true };
      for (const [width, height] of [[1024, 560], [390, 740], [390, 420]]) {
        await page.setViewportSize({ width, height });await frame(page);const d = await page.evaluate(sample);
        assert(d.documentHeight <= height + 1);assert.equal(d.composer, draft);result.cases.push({ viewport: [width, height], ...d });
      }
      await page.setViewportSize({ width: 1440, height: 900 });await frame(page);
      const paneBefore = await page.evaluate(sample);
      await page.locator('.workspace-bar [data-on-click="fm"]:visible').first().click();
      await page.waitForFunction(n => __bpPerf.trace.filter(x => x.phase === 'thin-view').length > n, paneBefore.pages);await frame(page);await frame(page);
      const paneAfter = await page.evaluate(sample);assert.equal(paneAfter.composer, draft);assert(paneAfter.documentHeight <= 901);
      result.splitPane = { before: paneBefore, after: paneAfter };
      await page.locator('[data-on-click="panel"]:visible').first().click();await frame(page);await frame(page);
      await page.setViewportSize({ width: 390, height: 740 });await frame(page);
      await cdp.send('Emulation.setTouchEmulationEnabled', { enabled: true });
      const touchBefore = await page.evaluate(fn => { document.getElementById('timeline').scrollTop = 0;return (0, eval)('(' + fn + ')()'); }, sample.toString());
      const touchBox = await page.locator('#timeline').boundingBox(), x = touchBox.x + touchBox.width / 2, y = touchBox.y + 80;
      await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x, y }] });
      await cdp.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [{ x, y: y + 80 }] });
      await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
      await page.waitForFunction(n => __bpPerf.trace.filter(x => x.phase === 'thin-view').length > n, touchBefore.pages);await frame(page);await frame(page);
      result.touch = { before: touchBefore, after: await page.evaluate(sample) };assert.equal(result.touch.after.composer, draft);
      await page.setViewportSize({ width: 1440, height: 900 });await frame(page);await frame(page);
      const navObservedBefore = await page.evaluate(sample);
      // Native touch inertia can still move between a sample and Playwright's
      // click. Capture the actual departure position in that click's capture
      // phase, immediately before the production selection handler runs.
      await page.evaluate(fn => {
        const snapshot = (0, eval)('(' + fn + ')');
        document.addEventListener('click', e => { if (e.target.closest?.('[data-on-click="select"][data-value="short"]')) window.fixtureNavLeaving = snapshot(); }, true);
      }, sample.toString());
      await page.locator('[data-on-click="select"][data-value="short"]:visible').first().click();
      const navBefore = await page.evaluate(() => window.fixtureNavLeaving);
      await page.waitForFunction(() => document.documentElement.dataset.backplaneRemoteReady === 'true' && document.querySelector('main[data-thread="short"]'));
      assert.equal(await page.locator('#composer').inputValue(), '', 'draft crossed conversation');
      await page.locator('[data-on-click="select"][data-value="long"]:visible').first().click();
      await page.waitForFunction(() => document.documentElement.dataset.backplaneRemoteReady === 'true' && document.querySelector('main[data-thread="long"]'));await frame(page);await frame(page);
      const navAfter = await page.evaluate(sample);
      result.navigation = { observedBefore: navObservedBefore, before: navBefore, after: navAfter, anchorKept: navAfter.anchor?.id === navBefore.anchor?.id };
      assert.equal(navAfter.composer, draft);assert(result.navigation.anchorKept, 'earlier page position lost when returning to conversation');
      assert(Math.abs(navAfter.anchor.offset - navBefore.anchor.offset) <= 1, 'restored conversation anchor offset changed');
      // Post from a separate disposable browser identity. The configured
      // provider is a local exit-127 stand-in, never a real provider process.
      const senderContext = await browser.newContext({ viewport: { width: 1440, height: 900 } });
      await senderContext.request.post(connection.origin + '/auth/pair', { headers: { Authorization: 'Bearer ' + await service.token() } });
      const sender = await senderContext.newPage();await sender.addInitScript(() => localStorage.setItem('backplane-selected', 'long'));
      await sender.goto(connection.origin);await sender.waitForFunction(() => document.documentElement.dataset.backplaneRemoteReady === 'true' && document.getElementById('composer'));
      const skipSender = sender.locator('[data-on-click="ob-skip"]');if (await skipSender.count()) await skipSender.click();
      await sender.waitForFunction(() => !document.querySelector('.app.tour-on'));
      const incomingBefore = await page.evaluate(sample), incoming = 'Generated incoming live-tail fixture message';
      await sender.locator('#composer').fill(incoming);await sender.locator('[data-on-click="send"]:visible').first().click();
      await sender.waitForFunction(text => document.getElementById('timeline')?.textContent.includes(text), incoming);
      await page.waitForFunction(n => __bpPerf.trace.filter(x => x.phase === 'thin-view').length > n, incomingBefore.pages);await frame(page);await frame(page);
      const incomingAfter = await page.evaluate(sample), senderAtEnd = await sender.evaluate(() => { const t=document.getElementById('timeline');return t.scrollHeight-t.scrollTop-t.clientHeight; });
      assert.equal(incomingAfter.anchor.id, incomingBefore.anchor.id);assert(Math.abs(incomingAfter.anchor.offset-incomingBefore.anchor.offset)<=1);assert.equal(incomingAfter.composer,draft);assert(senderAtEnd<=2);
      result.incoming = { before:incomingBefore, after:incomingAfter, newestSenderDistancePx:senderAtEnd };await senderContext.close();
      await cdp.send('HeapProfiler.collectGarbage');result.scenarioHeap = await cdp.send('Runtime.getHeapUsage');result.scenarioDom = await cdp.send('Memory.getDOMCounters');
    }
    result.perf = await page.evaluate(() => __bpPerf);result.errors = errors;
    assert(result.composerKept);assert.equal(errors.length, 0, errors.join('\n'));
    result.state = 'passed';
  } catch (e) { result.state = 'failed';result.error = e.stack;process.exitCode = 1;
    if (page) { try { result.failure = await page.evaluate(sample);result.perf = await page.evaluate(() => __bpPerf); } catch {} }
  }
  finally {
    if (browser) await browser.close();if (service) await service.stop();
    fs.writeFileSync(path.join(out, mode + '.json'), JSON.stringify(result, null, 2));
    console.log(JSON.stringify({ state: result.state, pages: result.cases.length, error: result.error }));
  }
})();
