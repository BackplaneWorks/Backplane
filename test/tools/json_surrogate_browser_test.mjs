// Browser codec/render boundary: real emitted Bend parser + production wire.js.
// This fixture does not claim end-to-end app, Swift, Kotlin or network delivery.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import { pathToFileURL } from 'node:url';
const emitted = fs.readFileSync(path.resolve(process.argv[2]), 'utf8');
const out = path.resolve(process.argv[3] || 'build/ui-laws/browser');
fs.mkdirSync(out, { recursive: true });
const wire = fs.readFileSync('src/web/wire.js', 'utf8');
const server = http.createServer((req, res) => {
  res.setHeader('Content-Type', req.url === '/' ? 'text/html; charset=utf-8' : 'text/javascript; charset=utf-8');
  if (req.url === '/parser.mjs') return res.end(emitted);
  if (req.url === '/wire.js') return res.end(wire);
  if (req.url !== '/') { res.statusCode = 404; return res.end(); }
  res.end(`<!doctype html><meta name="viewport" content="width=device-width,initial-scale=1"><title>JSON surrogate regression</title>
    <textarea id="source" aria-label="JSON source"></textarea><button id="decode">Decode</button><pre id="result"></pre>
    <script type="module">
      import F from '/parser.mjs';import {learnJson,json,plain} from '/wire.js';
      learnJson(F.json_tags());window.codec={F,json,plain};
      document.querySelector('#decode').onclick=()=>{
        document.querySelector('#result').textContent=F.show(F.parse(document.querySelector('#source').value));
      };
    </script>`);
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
let browser;
const errors = [], receipts = [];
try {
  const { chromium } = await import(pathToFileURL(path.resolve(process.env.BACKPLANE_PLAYWRIGHT_PATH || 'tools/browser/node_modules/playwright-core/index.mjs')).href);
  browser = await chromium.launch({headless:true, ...(process.env.BACKPLANE_CHROME ? {executablePath:process.env.BACKPLANE_CHROME} : {})});
  for (const width of [390, 1280]) {
    const page = await browser.newPage({viewport:{width,height:800}});
    page.on('pageerror', error => errors.push(String(error)));
    await page.goto(`http://127.0.0.1:${server.address().port}/`);
    await page.waitForFunction(() => !!window.codec);
    const facts = await page.evaluate(() => {
      const { F, json, plain } = window.codec;
      let units = 0;
      for (let unit = 0xd800; unit <= 0xdfff; unit++) {
        const raw = String.raw`"\u` + unit.toString(16) + 'X"';
        if (plain(F.parse(raw)) !== '\ufffdX') throw Error('escaped unit ' + unit);
        if (plain(json(String.fromCharCode(unit) + 'X')) !== '\ufffdX') throw Error('wire unit ' + unit);
        units++;
      }
      const text = '日本語 café 👩🏽‍💻';
      if (plain(F.parse(JSON.stringify(text))) !== text) throw Error('UTF-8 scalar text');
      if (plain(F.parse(String.raw`"\ud83d\udc69"`)) !== '👩') throw Error('pair');
      return { units, text };
    });
    await page.locator('#source').fill(String.raw`"\ud83d\udc69|\ud800|\udfff|日本語"`);
    await page.locator('#decode').click();
    assert.equal(await page.locator('#result').textContent(), '"👩|�|�|日本語"');
    receipts.push({ width, ...facts, rendered: await page.locator('#result').textContent() });
    await page.screenshot({path:path.join(out, `surrogates-${width}.png`)});
    await page.close();
  }
  assert.deepEqual(errors, []);
  fs.writeFileSync(path.join(out,'receipt.json'), JSON.stringify({browser:await browser.version(), receipts, errors},null,2)+'\n');
  console.log('ok browser surrogate decoding: 2,048 units, UTF-8 text, pairs and rendered replacements at 390/1280px');
} finally {
  if (browser) await browser.close();
  await new Promise(resolve => server.close(resolve));
}
