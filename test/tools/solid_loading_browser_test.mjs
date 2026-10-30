// Isolated headless production-renderer check and loading benchmark.
// bun test/tools/solid_loading_browser_test.mjs /path/to/read-only-board.glb
// Requires playwright-core from tools/browser; no hub or installed app runs.
import assert from 'node:assert/strict';
import {readFileSync, writeFileSync, mkdirSync, existsSync} from 'node:fs';
import {resolve, dirname, join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {createHash} from 'node:crypto';
const source = resolve(process.env.SOLID_RENDER_SOURCE || 'src/web/solid.js');
const adapter = resolve(process.env.SOLID_GLB_ADAPTER || 'test/tools/solid_glb_fixture.mjs');
const {glbFixture} = await import(pathToFileURL(adapter).href);
const {chromium} = await import(pathToFileURL(resolve(process.env.PLAYWRIGHT_CORE_PATH || 'tools/browser/node_modules/playwright-core/index.mjs')).href);
const path = resolve(process.argv[2]);
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const out = resolve(process.env.SOLID_LOADING_OUT || 'build/solid-loading');mkdirSync(out, {recursive: true});
const fixtures = {};
for (const [key, selection] of [['board', undefined], ['component', 'C5']]) {
  const began = performance.now(), f = glbFixture(path, selection);
  fixtures[key] = {...f, adapterMs: performance.now() - began, selection: selection || 'all', sha256: sha(f.mesh)};
}
const solid = readFileSync(source, 'utf8') + '\nexport {view as testView, assets as testAssets};';
const assets = readFileSync('src/web/solid-assets.js');
const html = '<style>html,body{margin:0;background:#202128}canvas{width:100%;height:100%;touch-action:none;display:block}.viewer-note{position:absolute;top:0;color:white}</style><div style="width:100vw;height:100vh"><canvas id="solid" data-key="2|board" tabindex="0"></canvas><div class="viewer-note" role="status"></div><div class="viewer-stat"></div></div>';
const server = Bun.serve({hostname: '127.0.0.1', port: 0, fetch(req) {
  const name = new URL(req.url).pathname.slice(1);
  if (name === 'solid.js') return new Response(solid, {headers: {'Content-Type': 'application/javascript'}});
  if (name === 'solid-assets.js') return new Response(assets, {headers: {'Content-Type': 'application/javascript'}});
  if (['solid-materials.js', 'solid-dof.js'].includes(name)) return new Response(readFileSync(join(dirname(source), name)), {headers: {'Content-Type': 'application/javascript'}});
  if (name === 'app.bend') return new Response('export default {solid_waiting:()=>"Preparing model…",solid_stats:()=>"Ready"};', {headers: {'Content-Type': 'application/javascript'}});
  if (name.endsWith('.mesh')) return new Response(fixtures[name.slice(0, -5)].mesh);
  return new Response(html, {headers: {'Content-Type': 'text/html'}});
}});
const executablePath = process.env.CHROME_BIN || ['/usr/bin/google-chrome-stable', '/usr/bin/chromium'].find(existsSync);
assert.ok(executablePath, 'CHROME_BIN must identify Chromium');
const browser = await chromium.launch({executablePath, headless: true, args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader']});
const results = [];
try {
  for (const mode of ['desktop', 'touch-reduced-motion']) {
    const context = await browser.newContext({viewport: mode === 'desktop' ? {width: 1000, height: 700} : {width: 390, height: 844}, hasTouch: mode !== 'desktop', reducedMotion: mode === 'desktop' ? 'no-preference' : 'reduce'});
    const page = await context.newPage(), errors = [];
    page.on('pageerror', error => errors.push(String(error)));
    await page.goto(`http://127.0.0.1:${server.port}/`);
    await page.evaluate(async () => { window.S = await import('/solid.js'); });
    for (const key of ['board', 'component']) {
      const f = fixtures[key];
      const result = await page.evaluate(async ({key, n, box3}) => {
        const S = window.S, canvas = document.querySelector('canvas');S.reset();canvas.dataset.key = `2|${key}`;
        const start = performance.now();S.mount(canvas);
        const mountMs = performance.now() - start;
        const beforeFetch = performance.now(), mesh = new Uint8Array(await (await fetch(`/${key}.mesh`)).arrayBuffer());
        const fetchMs = performance.now() - beforeFetch;
        const baselineStart = performance.now();S.mesh({n, mesh});const synchronousDecodeMs = performance.now() - baselineStart;
        let firstDrawMs = null, firstVisibleMs = null, firstFramePixels = 0, draws = 0;
        const v = S.testView, originalDraw = v.draw.bind(v), started = performance.now();
        v.draw = () => {
          originalDraw();if (!v.count) return;draws++;
          if (firstDrawMs !== null) return;
          firstDrawMs = performance.now() - started;
          // A submission alone may still be blank or waiting on the GPU. Read
          // the first frame back and verify actual visible geometry separately.
          const pixels = new Uint8Array(4 * canvas.width * canvas.height), gl = v.gl;
          gl.readPixels(0, 0, canvas.width, canvas.height, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
          for (let i = 3; i < pixels.length; i += 4) if (pixels[i]) firstFramePixels++;
          if (firstFramePixels > 50) firstVisibleMs = performance.now() - started;
        };
        const inputDelays = [];let tick = performance.now();
        const input = setInterval(() => {
          const at = performance.now();inputDelays.push(Math.max(0, at - tick - 8));tick = at;
          canvas.dispatchEvent(new KeyboardEvent('keydown', {key: 'ArrowLeft', bubbles: true}));
        }, 8);
        await S.got({key: `2|${key}`, n, box3, mesh}, {bytes: mesh.length, t: started});
        const completeMs = performance.now() - started;
        await new Promise(requestAnimationFrame);await new Promise(requestAnimationFrame);clearInterval(input);
        const record = S.testAssets.get(`2|${key}`), model = record.model;
        if (firstVisibleMs === null || !v.count) throw Error('First geometry submission has no visible pixels');
        const beforeWarm = performance.now();
        v.cam.spin(37, -19);v.cam.pan(12, 8, v.cam.unit(canvas.clientHeight));
        S.cameraAction('projection', 'orthographic');S.cameraAction('exposure', 1.2);
        const saved = JSON.stringify(v.snapshot());
        S.mount(null);const next = canvas.cloneNode();canvas.replaceWith(next);S.mount(next);
        await new Promise(requestAnimationFrame);
        const warmMs = performance.now() - beforeWarm;
        if (saved !== JSON.stringify(S.testView.snapshot())) throw Error('Camera/presentation state lost on remount');
        if (S.testAssets.get(`2|${key}`).model !== model) throw Error('Warm reopen decoded model again');
        const poseBefore = JSON.stringify(S.testView.cam);
        next.dispatchEvent(new KeyboardEvent('keydown', {key: 'ArrowRight', bubbles: true}));
        if (poseBefore === JSON.stringify(S.testView.cam)) throw Error('Keyboard orbit did not respond');
        // Pointer events exercise the same production one/two-finger handlers.
        const touchBefore = JSON.stringify(S.testView.cam);
        next.dispatchEvent(new PointerEvent('pointerdown', {pointerId: 1, pointerType: 'touch', clientX: 100, clientY: 100}));
        next.dispatchEvent(new PointerEvent('pointerdown', {pointerId: 2, pointerType: 'touch', clientX: 180, clientY: 100}));
        next.dispatchEvent(new PointerEvent('pointermove', {pointerId: 2, pointerType: 'touch', clientX: 210, clientY: 130}));
        next.dispatchEvent(new PointerEvent('pointerup', {pointerId: 1, pointerType: 'touch'}));
        next.dispatchEvent(new PointerEvent('pointerup', {pointerId: 2, pointerType: 'touch'}));
        if (touchBefore === JSON.stringify(S.testView.cam)) throw Error('Touch camera did not respond');
        // Reduced motion has no ongoing repaint once input and decode settle.
        let idleDraws = 0;const warmDraw = S.testView.draw.bind(S.testView);
        S.testView.draw = () => { idleDraws++;warmDraw(); };
        await new Promise(requestAnimationFrame);await new Promise(requestAnimationFrame);
        const settled = idleDraws;await new Promise(resolve => setTimeout(resolve, 70));
        if (settled !== idleDraws) throw Error('Viewer repaints continuously at rest');
        const gl = S.testView.gl, pixels = new Uint8Array(4 * next.width * next.height);
        S.testView.draw();gl.readPixels(0, 0, next.width, next.height, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
        let visible = 0;for (let i = 3; i < pixels.length; i += 4) if (pixels[i]) visible++;
        if (visible < 50) throw Error('Draw submitted no visible geometry');
        return {mountMs, fetchMs, synchronousDecodeMs, completeMs, firstDrawMs, firstVisibleMs, firstFramePixels,
          coldOpenToVisibleMs: mountMs + fetchMs + firstVisibleMs, firstGeometryMs: record.stats.firstGeometry,
          warmMs, maxInputDelayMs: Math.max(0, ...inputDelays), inputSamples: inputDelays.length, visiblePixels: visible,
          triangles: n, wireBytes: mesh.length, cachedModelReused: true, cameraPreserved: true, keyboard: true, touch: true, idleRepaints: 0};
      }, {key, n: f.n, box3: f.box3});
      assert.ok(result.maxInputDelayMs < 150, `input stall: ${JSON.stringify(result)}`);
      await page.locator('canvas').screenshot({path: join(out, `${mode}-${key}.png`)});
      results.push({mode, key, ...result});
    }
    assert.deepEqual(errors, []);await context.close();
  }
  const receipt = {browser: await browser.version(), source, sourceSha256: sha(solid), helperSha256: sha(assets), sourceGlb: path, sourceGlbSha256: sha(readFileSync(path)),
    scope: 'Existing saved GLB → test-only wire adapter → production browser progressive decode/GPU draw and cache. Export/network/hub parse cold paths are not measured.',
    fixtures: Object.fromEntries(Object.entries(fixtures).map(([key, f]) => [key, {selection: f.selection, triangles: f.n, bytes: f.mesh.length, adapterMs: f.adapterMs, wireSha256: f.sha256}])), results};
  writeFileSync(join(out, 'receipt.json'), JSON.stringify(receipt, null, 2) + '\n');
  console.log(JSON.stringify(receipt, null, 2));
} finally { await browser.close();server.stop(true); }
