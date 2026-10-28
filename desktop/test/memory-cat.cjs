// One focused acceptance: real Electron, isolated hub, a store larger than a view.
const fs = require('node:fs'), path = require('node:path'), os = require('node:os'), assert = require('node:assert/strict');
const { _electron } = require('playwright-core');
const { Service } = require('../service.cjs');
const stage = path.resolve(process.env.BACKPLANE_LOCAL_STAGE);
const info = JSON.parse(fs.readFileSync(path.join(stage, 'LOCAL.json')));
const out = process.env.MEMORY_CAT_OUT || path.join(stage, 'memory-cat-' + Date.now());
fs.mkdirSync(out, { recursive: true });
const home = fs.mkdtempSync(path.join(os.tmpdir(), 'backplane-memory-cat-check-'));
const agents = path.join(home, 'agents'); fs.mkdirSync(agents);
for (const name of ['claude', 'codex', 'grok', 'muse', 'agy', 'opencode']) fs.writeFileSync(path.join(agents, name), '#!/bin/sh\nexit 127\n', { mode: 0o755 });
const at = Math.floor(Date.now() / 1000), bot = 'bub', thread = 'bub-chat';
const ev = ($, fields) => ({ $, ...fields, at });
const events = [ev('ProjectCreated', { id: 'fixture', title: 'Memory fixture', root: home }),
  ev('ThreadCreated', { id: thread, title: 'Bub Chat', project: 'fixture', env: 'local', branch: '', worktree: home, provider: 'codex', model: 'gpt-6.1-sol' }),
  ev('BotSet', { id: bot, name: 'Bub', thread, look: 306440, persona: '' }), ev('UberSet', { bot, uid: 'memory-cat-fixture' }),
  ...['restart.continue', 'mail.on', 'telemetry.share'].map(key => ev('SettingSet', { key, value: 'off' })), ev('SettingSet', { key: 'onboard.done', value: '1' }),
  ev('MessagePosted', { thread, msg: 'hello', role: 'assistant', text: 'Memory and cat acceptance fixture.' })];
// Large enough that the former unbounded renderer exceeds 256 KiB even
// without this text. Unicode and all kinds exercise the actual renderer.
for (let i = 0; i < 300; i++) events.push({ ...ev('MemorySaved', { bot, key: 'memory-' + String(i).padStart(3, '0'),
  kind: ['user', 'pref', 'fact', 'howto', 'note'][i % 5], text: ('Fixture memory 日本語 ' + i + '. ').repeat(24), tags: 'fixture' }), at: at - i });
fs.writeFileSync(path.join(home, 'events.jsonl'), events.map(JSON.stringify).join('\n') + '\n');
Object.assign(process.env, { BACKPLANE_AGENTS: agents, BACKPLANE_COPY: '1', BACKPLANE_NO_UPDATE: '1', BACKPLANE_ORDER_SERVER: 'http://127.0.0.1:9', BACKPLANE_TELEMETRY: 'off', BACKPLANE_PEERS: '', BACKPLANE_LOCK_DIR: path.join(home, 'locks') });
const binary = path.join(stage, 'payload/backplane-serve'), web = path.join(stage, 'payload/web');
const service = new Service({ home, binary, web, expectedVersion: info.version });
let app, page; const errors = [], frames = [], start = performance.now();
(async () => { try {
  await service.start();
  app = await _electron.launch({ executablePath: path.join(stage, 'desktop/runtime/backplane-desktop'), args: [],
    env: { ...process.env, BACKPLANE_HOME: home, BACKPLANE_SERVICE: binary, BACKPLANE_WEB: web, BACKPLANE_SERVICE_VERSION: info.version } });
  page = await app.firstWindow();
  page.on('pageerror', e => errors.push(e.stack)); page.on('crash', () => errors.push('renderer crashed'));
  page.on('websocket', ws => ws.on('framereceived', f => frames.push(Buffer.byteLength(f.payload))));
  await page.waitForFunction(() => document.documentElement.dataset.backplaneRemoteReady === 'true');
  await page.addInitScript(() => {
    window.__bpPerf = { trace: [], act: [], live: [], part: [], render: [] };
    window.__memorySockets = [];
    const Native = window.WebSocket;
    window.WebSocket = class extends Native { constructor(...args) { super(...args); if (String(args[0]).includes('view=1')) window.__memorySockets.push(this); } };
  });
  await page.reload();
  const ready = () => page.waitForFunction(() => document.documentElement.dataset.backplaneRemoteReady === 'true');
  const cats = async () => {
    await page.waitForFunction(() => { const xs = [...document.querySelectorAll('.uber-row img.cat')]; return xs.length === 1 && xs.every(x => x.complete && x.naturalWidth === 28); });
  };
  const tab = async name => { await page.locator(`[data-on-click="bot-tab"][data-value="${name}"]`).first().click(); await ready(); await cats(); };
  await ready(); await cats(); await page.locator('.uber-row').click();
  await page.locator('.bot-tabs').waitFor();
  let mem = page.locator('[data-on-click="bot-tab"][data-value="memory"]');
  if (!await mem.count()) await tab('settings');
  await tab('memory');
  await page.waitForFunction(() => document.querySelector('.memory-range')?.textContent === '1–24 / 300');
  await cats(); await page.screenshot({ path: path.join(out, 'memory-desktop.png') });
  const keys = new Set();
  const reused = process.env.MEMORY_CAT_RESUME ? JSON.parse(fs.readFileSync(process.env.MEMORY_CAT_RESUME)) : null;
  if (reused) { assert.equal(reused.source, info.commit); assert.equal(reused.stage, stage); }
  if (!reused) {
  for (let p = 0; p < 13; p++) {
    const end = Math.min((p + 1) * 24, 300), range = `${p * 24 + 1}–${end} / 300`;
    await page.waitForFunction(range => document.querySelector('.memory-range')?.textContent === range, range);
    const shown = await page.locator('.item-key').allTextContents(); assert.equal(shown.length, end - p * 24);
    for (const key of shown) { assert(!keys.has(key), 'no memory repeated between pages'); keys.add(key); }
    await cats();
    if (p < 12) await page.locator('nav.memory-pages button').filter({ hasText: /^More$/ }).click();
  }
  assert.equal(keys.size, 300, 'every saved memory is accessible');
  await page.locator('nav.memory-pages button').filter({ hasText: /^Prev$/ }).click();
  await page.waitForFunction(() => document.querySelector('.memory-range')?.textContent === '265–288 / 300');
  // A healthy quiet link must survive returning from a long-hidden tab.
  const socketsBeforeWake = await page.evaluate(() => window.__memorySockets.length);
  await page.evaluate(() => {
    const originalNow = Date.now, at = originalNow();
    try {
      Date.now = () => at - 65000;
      Object.defineProperty(document, 'visibilityState', { configurable: true, value: 'hidden' });
      document.dispatchEvent(new Event('visibilitychange'));
      Date.now = () => at + 5000;
      Object.defineProperty(document, 'visibilityState', { configurable: true, value: 'visible' });
      document.dispatchEvent(new Event('visibilitychange'));
    } finally { Date.now = originalNow; delete document.visibilityState; }
  });
  assert.equal(await page.evaluate(() => window.__memorySockets.length), socketsBeforeWake);
  assert.equal(await page.evaluate(() => window.__memorySockets.at(-1).readyState), 1, 'healthy wake did not close socket');
  // A real lost transport keeps the known cat/sidebar and reconnects.
  await page.evaluate(() => window.__memorySockets.at(-1).close(1000, 'isolated acceptance reconnect'));
  await page.waitForFunction(n => window.__memorySockets.length > n && document.documentElement.dataset.backplaneRemoteReady === 'true', socketsBeforeWake);
  await cats();
  if (!await mem.count()) await tab('settings');
  await tab('memory');
  await page.waitForFunction(() => document.querySelectorAll('.item-key').length === 24);
  }
  // Refuse a single read-only paging request locally, exercising fail(state).
  await page.evaluate(() => {
    const socket = window.__memorySockets.at(-1), send = socket.send;
    socket.send = function () { socket.send = send; throw Error('isolated fixture transport refusal'); };
  });
  await page.locator('nav.memory-pages button').filter({ hasText: /^More$/ }).click();
  await page.waitForFunction(() => document.documentElement.dataset.backplaneRemoteReady === 'false');
  await cats(); assert.equal(await page.locator('.sidebar section.project[data-drag-to="fixture"]').count(), 1);
  await page.screenshot({ path: path.join(out, 'failure-keeps-cat.png') });
  await page.getByRole('button', { name: 'Retry now', exact: true }).click(); await ready(); await cats();
  if (!await mem.count()) await tab('settings');
  await tab('memory');
  await page.setViewportSize({ width: 390, height: 844 }); await cats();
  assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'mobile width fits');
  await page.screenshot({ path: path.join(out, 'memory-mobile.png') });
  await tab('chat'); await page.locator('[data-id="hello"]').waitFor(); await cats();
  await tab('settings'); await tab('memory'); await cats();
  assert.deepEqual(errors, []); assert(Math.max(...frames) <= 262144);
  const trace = await page.evaluate(() => window.__bpPerf.trace.filter(x => ['thin-disconnect', 'thin-error'].includes(x.phase)));
  const result = { state: 'passed', seconds: (performance.now() - start) / 1000, source: info.commit, version: info.version,
    memories: reused ? reused.passedBeforeFailure.memories : keys.size, reusedReceipt: process.env.MEMORY_CAT_RESUME || null, pageSize: 24, maxFrameBytes: Math.max(...frames), healthyWakeKeptSocket: true, actualDisconnectRecovered: true,
    failureKeptSidebarCat: true, transitions: ['memory pages', 'reconnect', 'request failure', 'retry', 'mobile', 'chat', 'settings', 'memory'], errors, trace, home, out };
  fs.writeFileSync(path.join(out, 'RESULT.json'), JSON.stringify(result, null, 2)); console.log(JSON.stringify(result));
} catch (e) {
  console.error(e); process.exitCode = 1;
  if (page) { fs.writeFileSync(path.join(out, 'failure.html'), await page.content().catch(() => '')); await page.screenshot({ path: path.join(out, 'failure.png'), timeout: 3000 }).catch(() => {}); }
} finally { if (app) await app.close(); service.stop(); } })();
