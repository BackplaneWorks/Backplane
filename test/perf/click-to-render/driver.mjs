// driver.mjs : click-to-render driver (Playwright over CDP: headless Chromium web, or the desktop app).
//   import { open } from './c2r.mjs';
//   const h = await open({ kind: 'web'|'desktop', origin, token, cdp, device, cpu });
//   const r = await h.measure('label', { pred: 'expr', go: async () => h.click(sel) });
import { createRequire } from 'node:module';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
const require = createRequire(import.meta.url);
// playwright-core: $PLAYWRIGHT_CORE, else the desktop app's dev dependency (desktop/node_modules)
const pw = require(process.env.PLAYWRIGHT_CORE || new URL('../../../desktop/node_modules/playwright-core', import.meta.url).pathname);
const CHROMIUM = process.env.CHROMIUM || '/usr/bin/chromium';
const HERE = dirname(fileURLToPath(import.meta.url));
const REC = readFileSync(join(HERE, 'rec.js'), 'utf8');
export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

export async function open(o) {
  let browser, context, page; const h0 = {};
  if (o.kind === 'desktop') {
    browser = await pw.chromium.connectOverCDP(`http://127.0.0.1:${o.cdp}`);
    context = browser.contexts()[0];
    for (let i = 0; i < 100 && !page; i++) {
      page = context.pages().find((p) => /^https?:/.test(p.url()) && !/devtools/.test(p.url()));
      if (!page) await sleep(300);
    }
    if (!page) throw new Error('no app page over CDP: ' + context.pages().map((p) => p.url()).join(' '));
    await context.addInitScript(REC);
    // the app's CSP has no 'unsafe-eval'; the recorder's predicates are evaluated strings
    const pre = await context.newCDPSession(page); await pre.send('Page.enable'); await pre.send('Page.setBypassCSP', { enabled: true });
    h0.pre = pre;
    await page.reload();
  } else {
    const dev = o.device === 'iphone15' ? { ...pw.devices['iPhone 15'] } : { viewport: { width: o.width || 1400, height: o.height || 900 }, deviceScaleFactor: 1 };
    browser = await pw.chromium.launch({ executablePath: CHROMIUM, headless: true, args: ['--disable-gpu-vsync-off', '--enable-features=LongAnimationFrameTiming', ...(process.env.C2R_GL === 'rtx' ? ['--use-gl=angle', '--use-angle=gl-egl', '--ignore-gpu-blocklist', '--enable-gpu'] : [])] });
    context = await browser.newContext(dev);
    await context.addInitScript(REC);
    page = await context.newPage();
    await page.goto(`${o.origin}/${o.query || ''}#token=${o.token}`);
  }
  const cdp = await context.newCDPSession(page);
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e).slice(0, 200)));
  const h = {
    browser, context, page, cdp, errors, kind: o.kind, touch: o.device === 'iphone15',
    async ready(ms = 60000) {
      await page.waitForFunction(() => document.documentElement.dataset.backplaneReady === 'true' || document.querySelector('.app .sidebar, .app .thread'), null, { timeout: ms });
      await sleep(1500);
      // a fresh workspace's first-run tour covers the page: skip it as a person would
      for (let i = 0; i < 3; i++) {
        const skip = page.locator('[data-on-click="ob-skip"]').filter({ visible: true });
        if (!(await skip.count())) break;
        await skip.first().click().catch(() => {}); await sleep(600);
      }
    },
    async throttle(rate) { await cdp.send('Emulation.setCPUThrottlingRate', { rate: rate || 1 }); h.cpu = rate || 1; },
    box: async (sel) => {
      const loc = typeof sel === 'string' ? page.locator(sel).filter({ visible: true }).first() : sel;
      await loc.scrollIntoViewIfNeeded({ timeout: 10000 }).catch(() => {});
      const b = await loc.boundingBox({ timeout: 15000 });
      if (!b) throw new Error('no box for ' + sel);
      return { x: b.x + b.width / 2, y: b.y + Math.min(b.height / 2, 12) };
    },
    // a person's click: the pointer arrives, rests ~150 ms (hover warming is real), press, release
    async click(sel, { hover = 150 } = {}) {
      const { x, y } = await h.box(sel);
      if (h.touch) { if (h.press) { await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x, y }] }); await sleep(h.press); await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] }); } else await page.touchscreen.tap(x, y); return; }
      await page.mouse.move(x - 30, y); await page.mouse.move(x, y, { steps: 2 });
      if (hover) await sleep(hover);
      await page.mouse.down(); if (h.press) await sleep(h.press); await page.mouse.up();
    },
    async key(k) { await page.keyboard.press(k); },
    async type(text, delay = 0) { await page.keyboard.type(text, { delay }); },
    async idle(ms = 400) { await sleep(ms); await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => setTimeout(r, 0)))); },
    // one measurement: arm the recorder with the predicate, run go(), wait for the predicate's frame, settle
    async measure(label, { pred, sample, content, confirm, cpred, csig, go, timeout = 15000, quiet = 300, cap = 6000 }) {
      const armed = await page.evaluate(([p, s, c, f, cp, cs]) => window.__c2r.arm(p, s, c, f, cp, cs), [pred, sample || null, content || null, confirm || null, cpred || null, csig || null]);
      if (!armed) { const e = await page.evaluate(() => window.__c2r.error); return { label, error: e }; }
      const wall0 = Date.now();
      await go();
      try {
        await page.waitForFunction(() => window.__c2r.tPaint != null, null, { timeout, polling: 50 });
      } catch {
        const st = await page.evaluate(() => ({ t0: window.__c2r.t0, inputType: window.__c2r.inputType }));
        await page.evaluate(() => { window.__c2r.armed = false; });
        return { label, error: st.t0 == null ? 'input never arrived' : 'predicate never true', wall: Date.now() - wall0 };
      }
      if (confirm) await page.waitForFunction(() => window.__c2r.tDom2 != null, null, { timeout: cap, polling: 50 }).catch(() => {});
      // content: wait until it is on screen (then finish's quiet period makes sure it stayed final)
      if (cpred) await page.waitForFunction(() => window.__c2r.cOk, null, { timeout: cap, polling: 50 }).catch(() => {});
      const r = await page.evaluate(([q, c]) => window.__c2r.finish(q, c), [quiet, cap]);
      return { label, ...r };
    },
    async shot(path) { await page.screenshot({ path }); },
    async close() { if (o.kind === 'desktop') await browser.close().catch(() => {}); else await browser.close(); },
  };
  return h;
}
