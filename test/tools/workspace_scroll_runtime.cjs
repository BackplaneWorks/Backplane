// Narrow packaged-runtime regression for the deployed DOM/CSS contract.
// Run from the repo root: node test/tools/workspace_scroll_runtime.cjs STAGE OUT OLD_STAGE
// Uses fresh generated homes and headless Chromium; never uses a user profile.
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const assert = require('node:assert/strict');
const { chromium } = require(process.env.PLAYWRIGHT_CORE || '/home/h/.bun/install/cache/playwright-core@1.64.1@@@1');
const { Service } = require(path.join(process.cwd(), 'desktop/service.cjs'));
const [stage, out, oldStage] = process.argv.slice(2);
if (![stage, out, oldStage].every(x => x && path.isAbsolute(x))) throw Error('Require absolute candidate stage, output, old stage');
fs.mkdirSync(out, { recursive: true });
const version = JSON.parse(fs.readFileSync(path.join(stage, 'LOCAL.json'))).version;
const results = { stage, oldStage, version, cases: [], limits: ['Generated histories and disposable PTY, actual packaged web/server, headless only.', 'Viewport resizing models available address-bar/keyboard height; no physical phone or native IME engine exercised.'] };
let browser, service;
function dimensions() {
  const rect = selector => {
    const e = document.querySelector(selector); if (!e) return null;
    const b = e.getBoundingClientRect(), s = getComputedStyle(e);
    return { top: b.top, bottom: b.bottom, height: b.height, display: s.display, client: e.clientHeight, scroll: e.scrollHeight, scrollTop: e.scrollTop, overflow: s.overflow, minHeight: s.minHeight };
  };
  const ancestors = [];
  for (let e = document.getElementById('timeline'); e; e = e.parentElement) {
    const r = e.getBoundingClientRect(), s = getComputedStyle(e);
    ancestors.push({ tag: e.tagName, class: e.className, id: e.id, height: r.height, client: e.clientHeight, scroll: e.scrollHeight, overflow: s.overflow });
  }
  return { width: innerWidth, height: innerHeight, documentHeight: document.documentElement.scrollHeight, pageY: scrollY, rail: rect('.workspace-bar'), header: rect('.thread-head'), composer: rect('.composer'), timeline: rect('#timeline'), sidebar: rect('.sidebar'), ancestors, scripts: [...document.scripts].map(x => x.src), styles: [...document.styleSheets].map(x => x.href) };
}
function bounded(d) {
  results.lastDimensions = d;
  assert(d.documentHeight <= d.height + 1, `document escapes: ${d.documentHeight}/${d.height}`);
  assert.equal(d.pageY, 0, 'document must not scroll');
  for (const name of ['rail', 'header', 'composer']) {
    const r = d[name]; assert(r && r.top >= -1 && r.bottom <= d.height + 1, `${name} outside viewport: ${JSON.stringify(r)}`);
  }
  if (d.width > 760) assert(d.sidebar?.top >= -1 && d.sidebar.bottom <= d.height + 1, 'sidebar outside viewport');
}
function createHome(mode) {
  const home = fs.mkdtempSync(path.join(os.tmpdir(), 'bp-scroll-' + mode + '-'));
  const project = path.join(home, 'project'), agents = path.join(home, 'agents');
  fs.mkdirSync(project); fs.mkdirSync(agents);
  for (const name of ['claude', 'codex', 'grok', 'muse', 'agy', 'opencode', 'pi', 'omp']) fs.writeFileSync(path.join(agents, name), '#!/bin/sh\nexit 127\n', { mode: 0o755 });
  const shell = path.join(home, 'fixture-shell');
  fs.writeFileSync(shell, '#!/usr/bin/python3\nimport os,tty\ntty.setraw(0)\nos.write(1,b"SCROLL_FIXTURE_READY\\r\\n")\nwhile os.read(0,1024): pass\n', { mode: 0o755 });
  let at = Math.floor(Date.now() / 1000), changes = [];
  const event = ($, data) => changes.push({ $, ...data, at: at++ });
  event('ProjectCreated', { id: 'fixture', title: 'Scroll fixture', root: project });
  for (const id of ['thread', 'bot-thread']) {
    event('ThreadCreated', { id, project: 'fixture', title: 'Long production scroll fixture ' + id, env: 'local', branch: '', worktree: project, provider: 'codex', model: '' });
    for (let i = 0; i < 40; i++) event('MessagePosted', { thread: id, msg: id + '-m' + i, role: i % 2 ? 'user' : 'assistant', text: ('Generated scroll content ' + i + '. ').repeat(12) });
  }
  event('BotSet', { id: 'fixture-bot', name: 'Fixture agent', thread: 'bot-thread', look: 1, persona: '' });
  for (const key of ['restart.continue', 'mail.on', 'telemetry.share']) event('SettingSet', { key, value: 'off' });
  event('SettingSet', { key: 'onboard.done', value: '1' });
  fs.writeFileSync(path.join(home, 'events.jsonl'), changes.map(JSON.stringify).join('\n') + '\n');
  Object.assign(process.env, { BACKPLANE_HOME: home, BACKPLANE_AGENTS: agents, BACKPLANE_COPY: '1', BACKPLANE_NO_UPDATE: '1', BACKPLANE_ORDER_SERVER: 'http://127.0.0.1:9', BACKPLANE_TELEMETRY: 'off', BACKPLANE_PEERS: '', BACKPLANE_LOCK_DIR: home + '/locks', SHELL: shell });
  return home;
}
async function clickDestination(page, action) {
  let button = page.locator(`.workspace-bar [data-on-click="${action}"]:visible`).first();
  if (!await button.count()) { await page.locator('.workspace-menu summary:visible').first().click(); button = page.locator(`.workspace-menu [data-on-click="${action}"]:visible`).first(); }
  await button.click();
}
(async () => {
  try {
    browser = await chromium.launch({ headless: true, executablePath: process.env.CHROMIUM || '/home/h/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome', args: ['--no-sandbox', '--disable-dev-shm-usage'] });
    for (const mode of (process.env.BACKPLANE_SCROLL_MODES || 'old,current').split(',')) {
      const home = createHome(mode), web = path.join(mode === 'old' ? oldStage : stage, 'payload/web');
      assert(fs.existsSync(path.join(web, 'index.html')), 'immutable web assets missing');
      service = new Service({ home, binary: path.join(stage, 'payload/backplane-serve'), web, expectedVersion: version });
      const connection = await service.start();
      const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
      await context.request.post(connection.origin + '/auth/pair', { headers: { Authorization: 'Bearer ' + await service.token() } });
      const page = await context.newPage(), errors = []; page.setDefaultTimeout(12000);
      page.on('pageerror', e => errors.push(String(e)));
      await page.addInitScript(() => { localStorage.setItem('backplane-selected', 'thread'); window.backplaneDesktop = { terminalFocus() {} }; });
      await page.goto(connection.origin);
      await page.waitForFunction(() => document.documentElement.dataset.backplaneRemoteReady === 'true' && document.getElementById('composer'));
      const skip = page.locator('[data-on-click="ob-skip"]'); if (await skip.count()) await skip.click();
      await page.waitForFunction(() => !document.querySelector('.app.tour-on'));
      for (const bot of [false, true]) {
        if ((process.env.BACKPLANE_SCROLL_SKIP || '').split(',').includes(mode + ':' + (bot ? 'bot' : 'thread'))) continue;
        await page.setViewportSize({ width: 1440, height: 900 });
        if (bot) { await page.locator('[data-on-click="bot"][data-value="fixture-bot"]').first().click(); await page.waitForFunction(() => document.querySelector('.bot-head')); }
        for (const [width, height] of [[1440, 900], [1024, 560], [360, 740], [360, 420]]) {
          await page.setViewportSize({ width, height });
          const draft = 'Unsent scroll fixture ' + mode + bot;
          const composer = page.locator('#composer'); await composer.fill(draft);
          const field = await composer.elementHandle();
          await field.evaluate(e => { e.focus(); e.setSelectionRange(3, 8); e.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true })); });
          const item = { mode, bot, width, height, states: [] }; results.cases.push(item);
          for (const fraction of [0, 0.5, 1]) {
            await page.evaluate(f => { const t = document.getElementById('timeline'); t.scrollTop = (t.scrollHeight - t.clientHeight) * f; }, fraction);
            await page.evaluate(() => new Promise(requestAnimationFrame));
            const d = await page.evaluate(dimensions); bounded(d); item.states.push({ fraction, ...d });
          }
          await page.evaluate(() => window.scrollTo(0, document.documentElement.scrollHeight));
          bounded(await page.evaluate(dimensions));
          assert(await field.evaluate(e => e === document.getElementById('composer')), 'composer node replaced during scroll/resize');
          assert.equal(await composer.inputValue(), draft, 'draft lost');
          assert.deepEqual(await field.evaluate(e => [e.selectionStart, e.selectionEnd]), [3, 8], 'keyboard selection lost');
          await field.evaluate(e => e.dispatchEvent(new CompositionEvent('compositionend', { bubbles: true })));
          await page.screenshot({ path: path.join(out, `${mode}-${bot ? 'bot' : 'thread'}-${width}-${height}.png`) });
        }
      }
      await page.setViewportSize({ width: 1440, height: 600 });
      await clickDestination(page, 'fm'); await page.waitForFunction(() => document.querySelector('.fm-note')?.textContent.toLowerCase().includes('empty'));
      bounded(await page.evaluate(dimensions));
      await page.locator('.workspace-bar [data-on-click="panel"]:visible').first().click();
      await clickDestination(page, 'term-toggle'); await page.waitForFunction(() => document.querySelector('.screen')?.textContent.includes('SCROLL_FIXTURE_READY'));
      bounded(await page.evaluate(dimensions));
      await page.locator('[data-on-click="ws-term-hide"]').click();
      assert.equal(errors.length, 0, errors.join('\n'));
      results.cases.push({ mode, splitViewer: true, terminal: true, errors });
      await context.close(); await service.stop(); service = null;
    }
    results.state = 'passed';
  } catch (error) { results.state = 'failed'; results.error = error.stack; process.exitCode = 1; }
  finally { if (browser) await browser.close(); if (service) await service.stop(); fs.writeFileSync(path.join(out, 'SCROLL-REGRESSION.json'), JSON.stringify(results, null, 2)); console.log(JSON.stringify({ state: results.state, cases: results.cases.length, error: results.error })); }
})();
