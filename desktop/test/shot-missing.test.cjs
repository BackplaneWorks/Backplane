'use strict';
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const http = require('node:http');
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright-core');

// A reply's picture whose file is gone folds to one line naming it; one that
// loads keeps its 4:3 box (src/web/shot-missing.js, style.css .shot.missing).
test('a reply picture that cannot load folds to a line naming its file', async t => {
  const web = path.resolve(__dirname, '../../src/web');
  const png = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==', 'base64');
  const server = http.createServer((req, res) => {
    if (req.url === '/message-images.js') { res.setHeader('content-type', 'text/javascript');res.end(fs.readFileSync(path.join(web, 'message-images.js')));return; }
    if (req.url === '/shot-missing.js') { res.setHeader('content-type', 'text/javascript');res.end(fs.readFileSync(path.join(web, 'shot-missing.js')));return; }
    if (req.url === '/style.css') { res.setHeader('content-type', 'text/css');res.end(fs.readFileSync(path.join(web, 'style.css')));return; }
    if (req.url.startsWith('/img?path=/ok.png')) { res.setHeader('content-type', 'image/png');res.end(png);return; }
    if (req.url.startsWith('/img')) { res.statusCode = 404;res.end('no such file');return; }
    res.setHeader('content-type', 'text/html');
    res.end(`<link rel="stylesheet" href="/style.css"><div class="msg assistant" style="width:800px">
      <a class="shot" id="gone" title="/p/mech/renders/terminal_closeup.png" href="/img?path=/p/gone.png"><img src="/img?path=/p/gone.png" alt="x"></a>
      <a class="shot" id="message" title="/tmp/missing.png" href="/message-image?entry=m&path=/tmp/missing.png"><img data-message-image="1" src="/img?path=/tmp/missing.png" alt="missing"><span class="message-image-unavailable">Image unavailable</span></a>
      <a class="shot" id="ok" title="/ok.png" href="/img?path=/ok.png"><img src="/img?path=/ok.png" alt="y"></a></div>`);
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));t.after(() => new Promise(resolve => server.close(resolve)));
  const browser = await chromium.launch({ executablePath: process.env.BACKPLANE_CHROME || '/home/h/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome', headless: true });t.after(() => browser.close());
  const page = await browser.newPage();
  await page.goto(`http://127.0.0.1:${server.address().port}/`);
  await page.evaluate(async () => {
    const m = await import('/shot-missing.js');window.m = m;m.installShotMissing(document);
    const mi=await import('/message-images.js');window.mi=mi;mi.installMessageImages(document);
    const image=document.querySelector('#message img');image.dispatchEvent(new Event('error'));
    await Promise.all([...document.images].map(i => i.complete ? 0 : new Promise(r => { i.onload = i.onerror = r; })));
    m.shotsMissing(document);
  });
  const box = id => page.evaluate(id => { const a = document.getElementById(id), r = a.getBoundingClientRect();return { missing: a.classList.contains('missing'), h: Math.round(r.height), text: getComputedStyle(a, '::before').content }; }, id);
  const gone = await box('gone'), ok = await box('ok'), message=await box('message');
  assert(message.h<40,'entry-bound missing image keeps main’s compact failure geometry');
  assert.equal(message.text,'none','entry-bound placeholder suppresses duplicate generic pseudo-text');
  assert.equal(await page.locator('#message .message-image-unavailable').innerText(),'Image unavailable');
  assert.equal(gone.missing, true, 'the picture that failed is marked');
  assert(gone.h < 40, `its box folds to a line (height ${gone.h})`);
  assert.match(gone.text, /terminal_closeup\.png/, 'the line names the file');
  assert.equal(ok.missing, false, 'a picture that loaded is not marked');
  assert(ok.h > 300, `a picture that loaded keeps its box (height ${ok.h})`);
  // a later frame patches the link's class back; the next render marks it again
  await page.evaluate(() => { document.getElementById('gone').className = 'shot';m.shotsMissing(document); });
  assert.equal((await box('gone')).missing, true, 'marked again after a patch');
  await page.evaluate(()=>{document.getElementById('message').className='shot';m.shotsMissing(document);mi.messageImages(document);});
  assert(await page.locator('#message .message-image-unavailable').isVisible(),'clear text placeholder survives a hub DOM patch');
  assert.equal((await box('message')).text,'none','no duplicate pseudo-text after patch');
});
