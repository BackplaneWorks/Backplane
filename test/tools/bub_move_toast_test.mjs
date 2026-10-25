// Source-only presentation check: no hub, lease move, Bend emit or user profile.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import { pathToFileURL } from 'node:url';

const out = path.resolve(process.argv[2]);
fs.mkdirSync(out, { recursive: true });
const moduleText = fs.readFileSync('src/web/bub-move.js', 'utf8');
const styles = fs.readFileSync('src/web/style.css', 'utf8');
const server = http.createServer((req, res) => {
  if (req.url === '/move.js') { res.setHeader('Content-Type', 'text/javascript');return res.end(moduleText); }
  if (req.url === '/style.css') { res.setHeader('Content-Type', 'text/css');return res.end(styles); }
  res.setHeader('Content-Type', 'text/html');
  res.end(`<!doctype html><meta name="viewport" content="width=device-width,initial-scale=1"><link rel="stylesheet" href="/style.css">
    <div class="app tour-on" style="display:block"><textarea id="draft">Unsent draft</textarea>
      <div class="tour-back"></div><div class="tour" role="dialog" aria-modal="true"><b>Welcome to Backplane</b><button id="next">Next</button></div>
    </div><script type="module">import {showBubMove} from '/move.js';window.show=showBubMove;</script>`);
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const { chromium } = await import(pathToFileURL(process.env.BACKPLANE_PLAYWRIGHT_PATH || '/home/h/.bun/install/cache/playwright-core@1.64.1@@@1/index.mjs').href);
let browser;
const checks = [], errors = [], geometry = [];
const check = (name, ok) => { checks.push({ name, ok: !!ok });assert.ok(ok, name);console.log('ok ' + name); };
const move = { id: 'fixture', title: 'Tub’s moving', from: 'desktop', to: 'the Boat', duration: 2000,
  cat: 'data:image/svg+xml,%3Csvg xmlns="http://www.w3.org/2000/svg" width="48" height="48"%3E%3Ctext y="34" font-size="32"%3E🐈%3C/text%3E%3C/svg%3E' };
try {
  browser = await chromium.launch({ headless: true, executablePath: process.env.BACKPLANE_CHROME || '/home/h/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome', args: ['--disable-dev-shm-usage'] });
  for (const [width, height, light] of [[1280,800,false], [360,740,true]]) {
    const page = await browser.newPage({ viewport: { width, height } });
    page.on('pageerror', e => errors.push(String(e)));await page.goto(`http://127.0.0.1:${server.address().port}/`);
    await page.waitForFunction(() => !!window.show);
    await page.evaluate(light => document.querySelector('.app').classList.toggle('light', light), light);
    await page.locator('#next').focus();
    await page.evaluate(move => window.show(move), move);
    check(width + ' named title uses plain text', await page.locator('.bub-move-title').textContent() === 'Tub’s moving');
    check(width + ' tour focus retained', await page.evaluate(() => document.activeElement.id === 'next'));
    // A stable animation frame separates stacking from the initial fade-in.
    await page.locator('#bub-move').evaluate(t => t.getAnimations({ subtree: true }).forEach(a => { a.pause();a.currentTime = 750; }));
    const receipt = await page.locator('#bub-move').evaluate(t => {
      const b = t.getBoundingClientRect(), style = getComputedStyle(t);
      // Only the test probe enables hit-testing; product pointer behavior stays none.
      t.style.pointerEvents = 'auto';const top = document.elementFromPoint(b.left + 4, b.top + 4);t.style.removeProperty('pointer-events');
      return { opacity: style.opacity, z: style.zIndex, tourZ: getComputedStyle(document.querySelector('.tour-back')).zIndex,
        above: top === t || t.contains(top), pointer: getComputedStyle(t).pointerEvents,
        bounded: b.left >= 0 && b.right <= innerWidth && b.bottom <= innerHeight && document.documentElement.scrollWidth <= innerWidth };
    });
    geometry.push({ width, ...receipt });
    check(width + ' above tour at full opacity', receipt.above && receipt.opacity === '1' && Number(receipt.z) > Number(receipt.tourZ));
    check(width + ' no pointer interception or horizontal escape', receipt.pointer === 'none' && receipt.bounded);
    await page.screenshot({ path: path.join(out, `tour-${width}.png`) });
    await page.evaluate(move => {
      document.querySelector('.tour-back').className = 'draw-back';
      window.show(move);
    }, { ...move, title: '<Tub> & friends’s moving' });
    check(width + ' name markup remains literal', await page.locator('.bub-move-title').textContent() === '<Tub> & friends’s moving' && await page.locator('.bub-move-title > *').count() === 0);
    check(width + ' above highest existing overlay', await page.locator('#bub-move').evaluate(t => Number(getComputedStyle(t).zIndex) > Number(getComputedStyle(document.querySelector('.draw-back')).zIndex)));
    await page.evaluate(move => { const { title, ...older } = move;window.show(older); }, move);
    check(width + ' older move falls back to Bub', await page.locator('.bub-move-title').textContent() === 'Bub’s moving');
    await page.locator('#bub-move').waitFor({ state: 'detached', timeout: 3000 });
    check(width + ' dismisses and preserves draft', await page.locator('#draft').inputValue() === 'Unsent draft');
    await page.close();
  }
  check('no browser errors', errors.length === 0);
} finally {
  fs.writeFileSync(path.join(out, 'RECEIPT.json'), JSON.stringify({ kind: 'isolated source JS/CSS; synthetic move, not Bend or SDK qualification', checks, geometry, errors }, null, 2));
  await browser?.close();await new Promise(resolve => server.close(resolve));
}
