'use strict';
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const http = require('node:http');
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright-core');

// These exercise the DOM boundary with explicit planner outputs, not a second
// implementation of the Bend anchoring/range algorithm. Layout is Chromium's.
test('timeline DOM effects preserve nodes, measure, forward commands and detach', async t => {
  const source = fs.readFileSync(path.resolve(__dirname, '../../src/web/timeline-scroller.js'));
  const server = http.createServer((req, res) => {
    if (req.url === '/timeline-scroller.js') {
      res.setHeader('content-type', 'text/javascript');
      res.end(source);
    } else {
      res.setHeader('content-type', 'text/html');
      res.end('<style>#timeline{height:200px;width:400px;overflow:auto;overflow-anchor:none}.timeline-virtual-row{display:flex;flex-direction:column}</style><body>');
    }
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  t.after(() => new Promise(resolve => server.close(resolve)));
  const executablePath = process.env.BACKPLANE_CHROME || [
    '/home/h/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome',
    '/home/h/.cache/ms-playwright/chromium-1217/chrome-linux64/chrome',
  ].find(p => fs.existsSync(p));
  const browser = await chromium.launch({ executablePath, headless: true });
  t.after(() => browser.close());
  const page = await browser.newPage();
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.goto(`http://127.0.0.1:${server.address().port}`);
  const initial = await page.evaluate(async () => {
    const { TimelineScroller } = await import('/timeline-scroller.js');
    const el = document.createElement('div');
    el.id = 'timeline';
    const rows = [ { key: 'a', height: 40.5 }, { key: 'b', height: 50 } ];
    const items = rows.map((r, index) => ({ key: r.key, index, start: 700 + index * 100, size: r.height }));
    window.calls = [];
    window.patches = 0;
    window.notifications = 0;
    window.output = { items, visible: items, total: 1000, top: 800, view: 200, atEnd: true, anchor: 'b', offset: 0 };
    const scroller = new TimelineScroller(el, {
      plan(input) {
        calls.push(input);
        // Explicit effect fixture: Bend supplies the normalized correction.
        if (input.mode === 'readback') output = { ...output, top: 801, logicalTop: 800.5, trackOffset: 0.5 };
        return output;
      },
      key: row => row.key,
      create(row) { const node = document.createElement('div'); node.textContent = row.key; node.style.height = row.height + 'px'; return node; },
      patch(node, row) { patches++; node.textContent = row.key; node.style.height = row.height + 'px'; },
      onChange(current) { notifications++; current.refresh(); },
    });
    window.scroller = scroller;
    window.rows = rows;
    scroller.update(rows);
    const detachedCalls = calls.length;
    document.body.append(el);
    scroller.refresh();
    window.kept = scroller.mounted.get('b').node;
    return { detachedCalls, mode: calls[0].mode, count: el.querySelectorAll('.timeline-virtual-row').length,
      height: scroller.track.style.height, top: el.scrollTop, measured: scroller.sizes.a,
      transform: scroller.mounted.get('b').wrapper.style.transform, position: scroller.position(), notifications };
  });
  assert.equal(initial.detachedCalls, 0);
  assert.equal(initial.mode, 'init');
  assert.equal(initial.count, 2);
  assert.equal(initial.height, '1000px');
  assert.equal(initial.top, 800);
  assert.equal(initial.measured, 40.5);
  assert.equal(initial.transform, 'translateY(800px)');
  assert.deepEqual(initial.position, { top: 800, atEnd: true, anchor: 'b', offset: 0 });
  assert.equal(initial.notifications, 1, 'reentrant refresh settles without a feedback loop');

  const update = await page.evaluate(() => {
    const before = patches;
    scroller.update(rows.slice());
    const unchanged = patches - before;
    const next = [rows[0], { ...rows[1], height: 75.25 }];
    scroller.update(next, { follow: false });
    next.push({ key: 'outside-snapshot', height: 10 });
    return { unchanged, patches: patches - before, retained: kept === scroller.mounted.get('b').node,
      measured: scroller.sizes.b, snapshot: scroller.rows.length, follow: calls.at(-1).follow };
  });
  assert.deepEqual(update, { unchanged: 0, patches: 1, retained: true, measured: 75.25, snapshot: 2, follow: false });

  await page.evaluate(() => { scroller.mounted.get('b').node.style.height = '125.5px'; });
  await page.waitForFunction(() => calls.some(call => call.mode === 'measure' && call.sizes.b === 125.5));
  const readback = await page.evaluate(() => {
    output = { ...output, top: 800.5, logicalTop: 800.5, trackOffset: 0 };
    scroller.restore({ top: 800.5 });
    return { readback: calls.some(call => call.mode === 'readback'), top: scroller.el.scrollTop,
      transform: scroller.track.style.transform, logicalTop: scroller.position().top };
  });
  assert.deepEqual(readback, { readback: true, top: 801, transform: 'translateY(0.5px)', logicalTop: 800.5 });

  const intermediate = await page.evaluate(() => {
    const keptWrapper = scroller.mounted.get('b').wrapper;
    const keptNode = scroller.mounted.get('b').node;
    const original = scroller.callbacks.plan;
    let first = true;
    scroller.callbacks.plan = input => {
      const plan = original(input);
      if (first) {
        first = false;
        return { ...plan, visible: [plan.items[0]] };
      }
      return plan;
    };
    // Measuring the newly patched a causes a second plan that includes b.
    scroller.update([{ key: 'a', height: 65 }, { key: 'b', height: 125.5 }]);
    scroller.callbacks.plan = original;
    return { wrapper: keptWrapper === scroller.mounted.get('b').wrapper,
      node: keptNode === scroller.mounted.get('b').node };
  });
  assert.deepEqual(intermediate, { wrapper: true, node: true }, 'measurement plans keep overlapping DOM until the range settles');

  const commands = await page.evaluate(() => {
    const missing = scroller.scrollToKey('missing');
    output = { ...output, visible: [output.items[0]] };
    scroller.configure({ follow: true });
    // b is now unmounted, but its key can still be sent to the planner.
    if (scroller.mounted.has('b')) throw new Error('Fixture must search an unmounted row');
    const found = scroller.scrollToKey('b', { align: 'center', offset: 3 });
    const key = calls.at(-1);
    scroller.scrollToEnd();
    const bottom = calls.at(-1).mode;
    const bookmark = { anchor: 'a', offset: -16, top: 0, atEnd: false };
    scroller.restore(bookmark);
    return { missing, found, key: { mode: key.mode, key: key.key, align: key.align, offset: key.offset },
      bottom, restored: calls.at(-1).bookmark };
  });
  assert.deepEqual(commands, { missing: false, found: true, key: { mode: 'key', key: 'b', align: 'center', offset: 3 },
    bottom: 'bottom', restored: { anchor: 'a', offset: -16, top: 0, atEnd: false } });

  const cache = await page.evaluate(() => {
    const measured = scroller.sizes.b;
    output = { ...output, visible: [output.items[0]] };
    scroller.update([rows[0]]);
    return { measured, retained: scroller.sizes.b };
  });
  assert.equal(cache.retained, cache.measured, 'absent page keys keep their measured size');

  const stream = await page.evaluate(() => {
    const live = { key: '@live:t', height: 150 };
    const item = { key: live.key, index: 0, start: 0, size: 150, keepHeight: true };
    output = { ...output, items: [item], visible: [item] };
    scroller.update([live]);
    const grown = scroller.sizes[live.key];
    scroller.update([{ ...live, height: 80 }]);
    const wrapper = scroller.mounted.get(live.key).wrapper;
    const retained = scroller.sizes[live.key], height = wrapper.getBoundingClientRect().height;
    const alignment = wrapper.style.justifyContent;
    output = { ...output, items: [], visible: [] };
    scroller.update([]);
    return { grown, retained, height, alignment, cleared: scroller.sizes[live.key] === undefined };
  });
  assert.deepEqual(stream, { grown: 150, retained: 150, height: 150, alignment: 'flex-end', cleared: true }, 'live tail shrink retains space until the stream ends');

  const destroyed = await page.evaluate(async () => {
    scroller.destroy();
    const count = calls.length;
    scroller.el.dispatchEvent(new Event('scroll'));
    scroller.el.style.height = '300px';
    scroller.refresh();
    scroller.update(rows);
    await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    return { extraCalls: calls.length - count, wrappers: scroller.el.children.length };
  });
  assert.deepEqual(destroyed, { extraCalls: 0, wrappers: 0 });
  assert.deepEqual(errors, []);
});
