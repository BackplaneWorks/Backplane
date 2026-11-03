'use strict';
// Real Bend planner + Chromium geometry. Run after emitting timeline-plan.bend:
// node desktop/test/timeline-follow.cjs /absolute/planner.mjs /absolute/output
const fs = require('node:fs'), path = require('node:path'), http = require('node:http');
const assert = require('node:assert/strict');
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright-core');
const [planner, output] = process.argv.slice(2);
assert(planner && output && path.isAbsolute(planner) && path.isAbsolute(output));
fs.mkdirSync(output, { recursive: true });
const source = path.resolve(__dirname, '../../src/web');
const files = { '/planner.mjs': planner, '/scroller.js': source + '/timeline-scroller.js', '/wire.js': source + '/wire.js' };
const server = http.createServer((req, res) => {
  res.setHeader('content-type', files[req.url] ? 'text/javascript' : 'text/html');
  res.end(files[req.url] ? fs.readFileSync(files[req.url]) : '<style>#timeline{height:200px;width:360px;overflow:auto;overflow-anchor:none}.timeline-virtual-row{display:flex;flex-direction:column}</style><div id="timeline"></div>');
});
let browser;
(async () => { try {
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  browser = await chromium.launch({ headless: true, ...(process.env.BACKPLANE_CHROME ? { executablePath: process.env.BACKPLANE_CHROME } : {}) });
  const page = await browser.newPage({ viewport: { width: 390, height: 844 } });
  const errors = [];page.on('pageerror', e => errors.push(e.message));
  await page.goto(`http://127.0.0.1:${server.address().port}`);
  await page.evaluate(async () => {
    const { default: Planner } = await import('/planner.mjs');
    const Wire = await import('/wire.js');
    const { TimelineScroller } = await import('/scroller.js');
    Wire.learnJson(Planner['../core/json.Json.parse']('[null,true,1,"",{"x":null}]').value);
    window.gestures = [];window.ends = [];
    window.list = new TimelineScroller(document.getElementById('timeline'), {
      key: row => row.key,
      plan: input => Wire.plain(Planner.timeline_plan(Wire.json({ ...input, bounded: true, bottomAlign: true }))),
      create(row) { const e = document.createElement('div');e.textContent = row.text;e.style.height = row.height + 'px';return e; },
      patch(e, row) { e.textContent = row.text;e.style.height = row.height + 'px'; },
      onScroll(_, delta) { gestures.push(delta); },
      onChange(s) { ends.push(s.atEnd()); },
    });
    window.rows = Array.from({ length: 60 }, (_, i) => ({ key: 'm' + i, text: 'Message ' + i, height: 40 }));
    list.update(rows).mount();
    window.settle = () => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    window.append = (key, text) => { rows = [...rows, { key, text, height: 100 }];list.update(rows); };
    window.visible = key => {
      const row = list.mounted.get(key)?.wrapper, box = list.el.getBoundingClientRect(), r = row?.getBoundingClientRect();
      return !!r && r.top >= box.top && r.bottom <= box.bottom;
    };
    await settle();gestures.length = ends.length = 0;
  });
  // Same task as an arriving saved message: the browser has clamped top but
  // the ResizeObserver still holds the height from the multiline composer.
  const sent = await page.evaluate(async () => {
    list.el.style.height = '400px';void list.el.clientHeight;
    append('own', 'The message I just sent');await settle();
    return { atEnd: list.atEnd(), visible: visible('own'), gestures, ends };
  });
  console.log(JSON.stringify({ sent }));
  await page.screenshot({ path: path.join(output, 'following.png') });
  assert(sent.atEnd && sent.visible, 'Send clearing a multiline composer must reveal the saved message');
  assert.deepEqual(sent.gestures, [], 'browser resize clamping is not an upward reading gesture');
  assert(sent.ends.every(Boolean), 'no intermediate layout may mark the follower away');
  const shrink = await page.evaluate(async () => {
    list.el.style.height = '200px';append('reply', 'The next agent reply');await settle();
    return { atEnd: list.atEnd(), visible: visible('reply') };
  });
  assert(shrink.atEnd && shrink.visible, 'a smaller viewport also retains following');
  // Force the scroll event ahead of ResizeObserver, as browsers may do.
  const resize = await page.evaluate(async () => {
    gestures.length = 0;list.el.style.height = '400px';void list.el.clientHeight;
    list.el.dispatchEvent(new Event('scroll'));await settle();
    return { atEnd: list.atEnd(), gestures };
  });
  assert(resize.atEnd);assert.deepEqual(resize.gestures, [], 'early resize scroll is a geometry correction');
  await page.locator('#timeline').evaluate(el => { el.scrollTop -= 900; });
  await page.waitForFunction(() => gestures.length > 0 && !list.atEnd());
  const away = await page.evaluate(async () => {
    const anchor = list.position(), before = list.mounted.get(anchor.anchor).wrapper.getBoundingClientRect().top;
    list.el.style.height = '500px';append('unread', 'New output while reading history');await settle();
    return { atEnd: list.atEnd(), anchor: list.position().anchor, expected: anchor.anchor,
      drift: list.mounted.get(anchor.anchor).wrapper.getBoundingClientRect().top - before };
  });
  assert.equal(away.atEnd, false, 'deliberate upward scrolling still holds older messages');
  assert.equal(away.anchor, away.expected);assert(Math.abs(away.drift) < 1, 'older reading anchor stays put');
  await page.evaluate(async () => { list.scrollToEnd();await settle(); });
  assert(await page.evaluate(() => list.atEnd() && visible('unread')), 'explicit newest navigation still reveals unread output');
  assert.deepEqual(errors, []);
  fs.writeFileSync(path.join(output, 'result.json'), JSON.stringify({ sent, shrink, resize, away }, null, 2) + '\n');
  console.log('ok real planner keeps sent messages visible through viewport changes and preserves older reading');
} finally { if (browser) await browser.close();await new Promise(resolve => server.close(resolve)); } })().catch(error => { console.error(error);process.exitCode = 1; });
