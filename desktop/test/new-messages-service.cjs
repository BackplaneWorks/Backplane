// Private service/browser regression. Never opens the person's profile or home.
const fs = require('node:fs'), path = require('node:path'), os = require('node:os'), assert = require('node:assert/strict');
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright-core');
const { Service } = require('../service.cjs');
const [binary, web, output] = process.argv.slice(2);
assert([binary, web, output].every(path.isAbsolute));
const home = fs.mkdtempSync(path.join(os.tmpdir(), 'bp-new-messages-')), agents = home + '/agents';
fs.mkdirSync(agents);
for (const name of ['claude', 'codex', 'grok', 'muse', 'agy', 'opencode', 'pi', 'omp']) fs.writeFileSync(agents + '/' + name, '#!/bin/sh\nexit 127\n', { mode: 0o755 });
const thread = 'chat', bot = 'fixture-bot', events = [
  { $: 'ProjectCreated', id: 'p', title: 'Isolated unread regression', root: home, at: 1 },
  { $: 'ThreadCreated', id: thread, project: 'p', title: 'Fixture chat', env: 'local', branch: '', worktree: home, provider: 'codex', model: '', at: 1 },
  { $: 'ThreadCreated', id: 'control', project: 'p', title: 'Fixture control', env: 'local', branch: '', worktree: home, provider: 'codex', model: '', at: 1 },
  { $: 'BotSet', id: bot, name: 'Fixture bot', thread, look: 0, persona: '', at: 1 },
];
for (let i = 0; i < 180; i++) events.push({ $: 'MessagePosted', thread, msg: 'm' + i, role: 'assistant', text: 'Visible history ' + i + '\n\n' + 'Reading context. '.repeat(20), at: 2 + i });
if (process.env.UNREAD_ENTRIES) events.push(...fs.readFileSync(process.env.UNREAD_ENTRIES, 'utf8').trim().split('\n').map(JSON.parse));
for (const [key, value] of [['restart.continue', 'off'], ['mail.on', 'off'], ['telemetry.share', 'off'], ['onboard.done', '1'], ['onboard.uber', '1']]) events.push({ $: 'SettingSet', key, value, at: 1 });
fs.writeFileSync(home + '/events.jsonl', events.map(JSON.stringify).join('\n') + '\n', { mode: 0o600 });
Object.assign(process.env, { BACKPLANE_HOME: home, BACKPLANE_COPY: '1', BACKPLANE_AGENTS: agents, BACKPLANE_KISTACK_SYNC: '0', BACKPLANE_NO_UPDATE: '1', BACKPLANE_TELEMETRY: 'off', BACKPLANE_PEERS: '', BACKPLANE_LOCK_DIR: home + '/locks', SHELL: '/bin/sh' });
const service = new Service({ home, binary, web });
let browser, page, step = 'start';
const result = { home, binary, web, state: 'running', snapshots: [], errors: [], wire: [] };
async function snapshot(label) {
  const value = await page.evaluate(() => {
    const el = document.getElementById('timeline'), box = el.getBoundingClientRect();
    return { ready: document.documentElement.dataset.backplaneRemoteReady, top: el.scrollTop, height: el.scrollHeight, view: el.clientHeight,
      pill: el.querySelector('.hold-pill')?.textContent || '', draft: document.getElementById('composer')?.value,
      rows: [...el.querySelectorAll('.timeline-virtual-row')].map(e => ({ key: e.dataset.virtualKey, y: e.getBoundingClientRect().top - box.top, h: e.getBoundingClientRect().height, text: e.textContent.slice(0, 90) })) };
  });
  result.snapshots.push({ label, ...value }); return value;
}
(async () => { try {
  const connection = await service.start(); assert(service.child, 'Own isolated child');
  browser = await chromium.launch({ executablePath: process.env.BACKPLANE_CHROME || '/home/h/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome', headless: true });
  const context = await browser.newContext({ viewport: process.env.UNREAD_MOBILE === '1' ? { width: 390, height: 844 } : { width: 1280, height: 900 } });
  const paired = await context.request.post(connection.origin + '/auth/pair', { headers: { Authorization: 'Bearer ' + await service.token() } }); assert(paired.ok());
  page = await context.newPage(); page.setDefaultTimeout(12000); page.on('pageerror', e => result.errors.push(e.message));
  const Wire = await import('../../src/web/wire.js'), cbor = fs.readFileSync(path.join(__dirname, '../../src/core/cbor.bend'), 'utf8');
  const dictionary = name => JSON.parse(new RegExp('def Cbor\\.' + name + '\\(\\) -> List<&2, String>:\\s*(\\[[\\s\\S]*?\\])').exec(cbor)[1]);
  const keys = dictionary('keys'), words = dictionary('words');
  page.on('websocket', socket => {
    for (const event of ['framesent', 'framereceived']) socket.on(event, frame => {
      try {
        const data = frame.payload, o = typeof data === 'string' ? JSON.parse(data) : Wire.plain(Wire.cbor(data, keys, words));
        result.wire.push({ event, type: o.t, method: o.m, id: o.id, away: o.p?.away, cursor: o.p?.cursor, selected: o.p?.session?.sel, target: o.p?.target,
          newer: o.result?.window?.newer, resultSelected: o.result?.session?.sel, resultAway: o.result?.session?.drafts?.['@away.chat'] });
      } catch {}
    });
  });
  await page.addInitScript(() => {
    localStorage.setItem('backplane-selected', 'chat');
    window.__bpRender = { n() {}, t() {}, sent() {}, got() {} };
    window.testSockets = []; const Socket = window.WebSocket;
    window.WebSocket = class extends Socket { constructor(...args) { super(...args); testSockets.push(this); } };
  });
  await page.goto(connection.origin);
  await page.waitForFunction(() => document.documentElement.dataset.backplaneRemoteReady === 'true' && document.getElementById('composer'));
  for (let i = 0; i < 5 && await page.locator('[data-on-click="ob-skip"]').count(); i++) await page.locator('[data-on-click="ob-skip"]').click();
  if (process.env.UNREAD_BOT === '1') {
    const button = page.locator('[data-on-click="bot"][data-value="' + bot + '"]');
    result.botButtons = await page.locator('[data-value="' + bot + '"]').evaluateAll(es => es.map(e => ({ action: e.getAttribute('data-on-click'), text: e.textContent })));
    await button.first().click();
    await page.waitForTimeout(250);
  }
  await page.locator('#composer').fill('Unsent 日本語 draft');
  await snapshot('initial');
  const otherContext = await browser.newContext();
  const otherPair = await otherContext.request.post(connection.origin + '/auth/pair', { headers: { Authorization: 'Bearer ' + await service.token() } }); assert(otherPair.ok());
  const other = await otherContext.newPage();
  await other.addInitScript(() => localStorage.setItem('backplane-selected', 'chat'));
  await other.goto(connection.origin);
  await other.waitForFunction(() => document.documentElement.dataset.backplaneRemoteReady === 'true' && document.getElementById('composer'));
  await other.locator('#composer').fill('Other client draft');
  // Test-only source messages through the real hub; providers are blocked.
  let rpc = 1;
  async function append(text) {
    const response = await context.request.post(connection.origin + '/mcp/control', { headers: { Authorization: 'Bearer ' + await service.token() },
      data: { jsonrpc: '2.0', id: rpc++, method: 'tools/call', params: { name: 'thread_send', arguments: { threadId: thread, text } } } });
    assert(response.ok()); assert(!((await response.text()).includes('"isError":true')));
  }
  if (process.env.UNREAD_PAGING === '1') {
    step = 'paging-click';
    await append('Reachable during delayed paging');
    await page.waitForTimeout(200);
    // Delay only this isolated browser's replies. An explicit newest intent
    // must survive the actual host's in-flight history request.
    await page.routeWebSocket(/\/ws\?view=1/, ws => {
      const remote = ws.connectToServer();
      remote.onMessage(message => setTimeout(() => ws.send(message), 600));
    });
    await page.evaluate(() => testSockets.at(-1).close());
    await page.waitForFunction(() => testSockets.length > 1 && document.documentElement.dataset.backplaneRemoteReady === 'true');
    await page.waitForTimeout(100);
    const box = await page.locator('#timeline').boundingBox();
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.wheel(0, -10000);
    await page.waitForFunction(() => __bpRender.paging?.().paging);
    await snapshot('page-in-flight');
    // The virtualized pill can be outside the mounted range at the top.
    // Exercise its exact delegated action using a fixed fixture button so
    // Playwright does not implicitly scroll it into view before the click.
    await page.evaluate(() => {
      const button = document.createElement('button'); button.id = 'fixture-newest'; button.dataset.onClick = 'hold-bottom'; button.textContent = 'Fixture jump';
      button.style.cssText = 'position:fixed;bottom:20px;right:20px;z-index:1000'; document.body.append(button);
    });
    await page.locator('#fixture-newest').click();
    await page.locator('#fixture-newest').evaluate(el => el.remove());
    await page.waitForTimeout(2000);
    const reached = await snapshot('delayed-page-button-bottom');
    assert(reached.rows.some(r => r.text.includes('Reachable during delayed paging') && r.y < reached.view && r.y + r.h > 0), 'Button issued during paging reveals latest content');
    assert.equal(reached.pill, '');
  }
  step = 'away'; await page.locator('#timeline').evaluate(el => { el.scrollTop -= 450; }); await page.waitForTimeout(200);
  await append('Reachable new one'); await append('Reachable new two');
  await page.waitForTimeout(500); await snapshot('away-with-new');
  await other.waitForFunction(() => document.getElementById('timeline')?.textContent.includes('Reachable new two'));
  assert.equal(await other.locator('#timeline .msg').filter({ hasText: 'Reachable new two' }).count(), 1, 'Other client receives the new message exactly once');
  assert.equal(await other.locator('#timeline .hold-pill').count(), 0, 'Following client does not inherit the other client read boundary');
  assert.equal(await other.locator('#composer').inputValue(), 'Other client draft');
  result.crossClient = { deliveredExactlyOnce: true, independentReadPositions: true, draftsPreserved: true };
  await otherContext.close();
  step = 'button'; const pill = page.locator('.hold-pill'); assert(await pill.count(), 'New badge exists'); await pill.click({ force: true });
  await page.waitForFunction(() => !document.querySelector('#timeline .hold-pill'));
  const bottom = await snapshot('button-bottom');
  assert(bottom.rows.some(r => r.text.includes('Reachable new two') && r.y < bottom.view && r.y + r.h > 0), 'Button reveals latest message'); assert.equal(bottom.pill, '', 'Button clears badge');
  step = 'reconnect'; await page.locator('#timeline').evaluate(el => { el.scrollTop -= 450; }); await page.waitForTimeout(100);
  const beforeDisconnect = await snapshot('before-disconnect');
  const anchor = beforeDisconnect.rows.find(r => !r.key.startsWith('control:') && r.y + r.h > 0 && r.y < beforeDisconnect.view);
  const socketCount = await page.evaluate(() => testSockets.length);
  await page.evaluate(() => testSockets.at(-1).close()); await append('Visible after reconnect');
  await page.waitForFunction(count => testSockets.length > count && document.documentElement.dataset.backplaneRemoteReady === 'true', socketCount);
  await page.waitForTimeout(300); const reconnected = await snapshot('reconnected-away');
  if (process.env.UNREAD_EXPECT_RESTORE === '1') {
    const row = reconnected.rows.find(r => r.key === anchor.key);
    assert(row && Math.abs(row.y - anchor.y) <= 2, 'Reconnect preserves the visible reading anchor and pixel offset');
    assert.equal(reconnected.pill, '1 new ↓', 'Reconnect keeps the first boundary and counts the genuinely unseen message until reached');
  }
  step = 'manual-bottom'; await page.locator('#timeline').evaluate(el => { el.scrollTop = el.scrollHeight; });
  await page.waitForFunction(() => !document.querySelector('#timeline .hold-pill'));
  const manual = await snapshot('manual-bottom');
  assert(manual.rows.some(r => r.text.includes('Visible after reconnect') && r.y < manual.view && r.y + r.h > 0), 'Manual scroll reveals reconnect message');
  assert.equal(manual.pill, '', 'Manual bottom clears badge'); assert.equal(manual.draft, 'Unsent 日本語 draft'); assert.deepEqual(result.errors, []);
  result.state = 'passed';
} catch (e) { result.state = 'failed'; result.step = step; result.error = e.stack; if (page) await snapshot('failure').catch(() => {}); process.exitCode = 1;
} finally { if (browser) await browser.close(); await service.stop(); fs.writeFileSync(output, JSON.stringify(result, null, 2) + '\n'); console.log(JSON.stringify({ state: result.state, step: result.step, error: result.error, output })); } })();
