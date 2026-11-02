const { test } = require('node:test'), assert = require('node:assert/strict');
const fs = require('node:fs'), path = require('node:path');
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright-core');

// Production CSS and divider pointer listeners against a real native Chromium
// scrollbar. Only the pure Bend width-clamp boundary is supplied by the fixture.
// A pointer at the scrollbar's right edge must never start a viewer resize.
test('viewer divider leaves the conversation native scrollbar draggable', async t => {
  const css = fs.readFileSync(process.env.VIEWER_SCROLLBAR_STYLE || path.resolve(__dirname, '../../src/web/style.css'), 'utf8');
  const host = fs.readFileSync(path.resolve(__dirname, '../../src/web/host.js'), 'utf8');
  const panes = host.slice(host.indexOf('const VIEWER_W ='), host.indexOf('// Socket\n'));
  assert.ok(panes.includes('edge.setPointerCapture(e.pointerId)'));
  const executablePath = process.env.BACKPLANE_CHROME || [
    '/home/h/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome',
    '/home/h/.cache/ms-playwright/chromium-1217/chrome-linux64/chrome',
  ].find(fs.existsSync);
  const browser = await chromium.launch({ executablePath, headless: true, ignoreDefaultArgs: ['--hide-scrollbars'] });
  t.after(() => browser.close());
  for (const viewport of [{ width: 2556, height: 1398 }, { width: 1440, height: 900 }]) {
    const page = await browser.newPage({ viewport });
    await page.setContent(`<style>${css}\n:root{--viewer-w:600px}</style><div id="root"><div class="app top panel picked"><header class="topbar">Private scrollbar fixture</header><aside class="sidebar"></aside><main class="thread-view workspace-thread"><div class="thread-content"><div id="timeline" class="timeline"><div style="height:8000px">Private history geometry</div></div><div class="composer"><textarea id="composer">Unsent draft</textarea></div></div><nav class="workspace-bar"></nav></main><aside class="viewer" id="viewer"><div class="viewer-split" role="separator" tabindex="0" aria-label="Resize viewer"></div><div class="viewer-stage"></div></aside></div></div>`);
    await page.addScriptTag({ content: `const App={panes_viewer:(w,side,strip,rail,drag)=>BigInt(Math.max(320,Math.min(Number(w)-Number(side)-Number(rail)-320,Number(drag)||600))),panes_fold:()=>false,panes_drawer:()=>false,sel:()=>"fixture"};let thin=null,ui={};function composerGeometry(){}function composerTop(){}\n${panes}\nwindow.dividerPresses=0;document.addEventListener('pointerdown',e=>{if(e.target.closest?.('.viewer-split'))window.dividerPresses++},true);` });
    const before = await page.evaluate(() => {
      const tl = document.getElementById('timeline'), r = tl.getBoundingClientRect(), edge = document.querySelector('.viewer-split');
      const bar = tl.offsetWidth - tl.clientWidth, thumb = tl.clientHeight ** 2 / tl.scrollHeight;
      return { bar, x: r.right - 1, y: r.top + thumb / 2, top: tl.scrollTop, divider: edge.getBoundingClientRect().toJSON(), hit: document.elementFromPoint(r.right - 1, r.top + thumb / 2)?.className, viewerWidth: document.getElementById('viewer').getBoundingClientRect().width };
    });
    assert.equal(before.bar, 8, 'test must retain the real 8px native scrollbar');
    await page.mouse.move(before.x, before.y);await page.mouse.down();await page.mouse.move(before.x, before.y + 250, { steps: 12 });await page.mouse.up();
    const after = await page.evaluate(() => ({ top: document.getElementById('timeline').scrollTop, dividerPresses: window.dividerPresses, viewerWidth: document.getElementById('viewer').getBoundingClientRect().width, draft: document.getElementById('composer').value }));
    t.diagnostic(JSON.stringify({ viewport, before, after }));
    assert.ok(after.top - before.top > 500, 'right-edge native thumb drag was intercepted by viewer divider');
    assert.equal(after.dividerPresses, 0);assert.equal(after.viewerWidth, before.viewerWidth);assert.equal(after.draft, 'Unsent draft');
    // Moving the divider itself still uses the production captured pointer.
    const box = await page.locator('.viewer-split').boundingBox();
    await page.mouse.move(box.x + box.width / 2, box.y + 100);await page.mouse.down();await page.mouse.move(box.x + box.width / 2 - 60, box.y + 100, { steps: 5 });await page.mouse.up();
    const resized = await page.evaluate(() => ({ presses: window.dividerPresses, width: document.getElementById('viewer').getBoundingClientRect().width, draft: document.getElementById('composer').value }));
    assert.equal(resized.presses, 1);assert.ok(resized.width > after.viewerWidth + 40);assert.equal(resized.draft, 'Unsent draft');
    await page.setViewportSize({ width: 390, height: 844 });assert.equal(await page.locator('.viewer-split').isVisible(), false);
    await page.close();
  }
});
