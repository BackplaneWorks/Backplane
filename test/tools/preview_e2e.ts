// Real preview HTTP delivery, isolated home and stand-in providers.
// bun test/tools/preview_e2e.ts HUB [WEB WIRE [PLAYWRIGHT_MODULE]]
// The optional browser gate opens Mech's Renders page and its lightbox.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync, openSync, closeSync, ftruncateSync, symlinkSync, readlinkSync, realpathSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {deflateSync} from 'node:zlib';
import {createRequire} from 'node:module';
const hubToken = (dir: string): string => require('node:fs').readFileSync(dir + '/token', 'utf8').trim();

const [binary, web, wire, playwright] = process.argv.slice(2);
assert.ok(binary, 'usage: bun test/tools/preview_e2e.ts HUB [WEB WIRE [PLAYWRIGHT_MODULE]]');
const root = mkdtempSync(join(tmpdir(), 'bp-preview-e2e-'));
const home = join(root, 'home'), fake = join(root, 'agents'), project = join(root, 'project');
for (const dir of [home, fake, project, join(project, 'mech'), join(project, 'mech/renders')]) mkdirSync(dir);
for (const cli of ['claude', 'codex', 'grok', 'muse', 'gh', 'tailscale', 'freecad'])
  writeFileSync(join(fake, cli), '#!/bin/sh\nexit 127\n', {mode: 0o755});
const ok = (message: string) => console.log('ok ' + message);
const digest = (b: Uint8Array) => createHash('sha256').update(b).digest('hex');

// A valid PNG with known pixels. Deflate level 0 keeps the encoded file
// above the old limit without relying on a random fixture.
function png() {
  const w = 2048, h = 2048, stride = w * 4 + 1;
  const raw = Buffer.alloc(stride * h);
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
    const at = y * stride + 1 + x * 4;
    raw[at] = x % 256; raw[at + 1] = y % 256; raw[at + 2] = 73; raw[at + 3] = 255;
  }
  const crc = (b: Buffer) => {
    let c = 0xffffffff;
    for (const byte of b) { c ^= byte; for (let i = 0; i < 8; i++) c = (c >>> 1) ^ (0xedb88320 & -(c & 1)); }
    return (c ^ 0xffffffff) >>> 0;
  };
  const chunk = (type: string, data: Buffer) => {
    const b = Buffer.concat([Buffer.alloc(4), Buffer.from(type), data, Buffer.alloc(4)]);
    b.writeUInt32BE(data.length, 0); b.writeUInt32BE(crc(b.subarray(4, -4)), b.length - 4); return b;
  };
  const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(w, 0); ihdr.writeUInt32BE(h, 4); ihdr[8] = 8; ihdr[9] = 6;
  return Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk('IHDR', ihdr), chunk('IDAT', deflateSync(raw, {level: 0})), chunk('IEND', Buffer.alloc(0))]);
}
function pdf() {
  const content = 'BT /F1 24 Tf 40 100 Td (Large PDF preview) Tj ET\n' + ('% preview fixture\n').repeat(800000);
  const objects = ['<< /Type /Catalog /Pages 2 0 R >>', '<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
    '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>',
    '<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>', `<< /Length ${content.length} >>\nstream\n${content}endstream`];
  let text = '%PDF-1.7\n'; const offsets = [0];
  objects.forEach((o, i) => { offsets.push(text.length); text += `${i + 1} 0 obj\n${o}\nendobj\n`; });
  const xref = text.length;
  text += `xref\n0 6\n0000000000 65535 f \n` + offsets.slice(1).map(n => `${String(n).padStart(10, '0')} 00000 n \n`).join('');
  return Buffer.from(text + `trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`);
}
const image = png(), document = pdf();
assert.ok(image.length > 12 * 1024 * 1024 && document.length > 12 * 1024 * 1024);
const imagePath = join(project, 'mech/renders/part.iso.png'), pdfPath = join(project, 'large.pdf');
writeFileSync(imagePath, image); writeFileSync(pdfPath, document);
// Listing a real part needs no CAD conversion for its saved renders.
writeFileSync(join(project, 'mech/part.step'), 'ISO-10303-21;\nHEADER;\nENDSEC;\nDATA;\nENDSEC;\nEND-ISO-10303-21;\n');
writeFileSync(join(project, 'small.txt'), '日本語 preview\n'); writeFileSync(join(project, 'empty.txt'), '');
writeFileSync(join(project, 'blocked.js'), 'not a preview'); symlinkSync(imagePath, join(project, 'linked.png'));
const hugePath = join(project, 'large.txt'), fd = openSync(hugePath, 'w'); ftruncateSync(fd, 256 * 1024 * 1024); closeSync(fd);
const listener = Bun.listen({hostname: '127.0.0.1', port: 0, socket: {data() {}}});
const port = listener.port; listener.stop(true);
const base = `http://127.0.0.1:${port}`;
const proc = Bun.spawn([resolve(binary), '--home', home, '--host', '127.0.0.1', '--port', String(port), '--foreground', '--headless', '--no-tailscale', ...(web ? ['--web', resolve(web)] : [])], {
  env: {...process.env, HOME: root, BACKPLANE_AGENTS: fake, BACKPLANE_COPY: '1', BACKPLANE_NO_UPDATE: '1',
    BACKPLANE_VAULT_KEYRING: 'file', BACKPLANE_LOCK_DIR: join(root, 'locks'), BACKPLANE_PEERS: '', BACKPLANE_ORDER_SERVER: 'http://127.0.0.1:9'},
  stdout: Bun.file(join(root, 'stdout')), stderr: Bun.file(join(root, 'stderr')),
});
let browser: any, uiPage: any, ws: WebSocket | undefined;
async function until<T>(f: () => T | Promise<T>, message: string, ms = 20000): Promise<NonNullable<T>> {
  const end = Date.now() + ms;
  while (Date.now() < end) { const v = await f(); if (v) return v as NonNullable<T>; await Bun.sleep(50); }
  throw new Error(message);
}
const url = (path: string) => `${base}/img?path=${encodeURIComponent(path)}`;
const rss = () => {
  if (process.platform === 'linux') return Number(readFileSync(`/proc/${proc.pid}/status`, 'utf8').match(/^VmRSS:\s+(\d+)/m)![1]) * 1024;
  return Number(Bun.spawnSync(['ps', '-o', 'rss=', '-p', String(proc.pid)]).stdout.toString().trim()) * 1024;
};
try {
  await until(async () => { try { return (await fetch(base + '/hello')).ok; } catch { return false; } }, 'hub did not start');
  for (const [path, bytes, type] of [[imagePath, image, 'image/png'], [pdfPath, document, 'application/pdf']] as const) {
    const r = await fetch(url(path)); assert.equal(r.status, 200);
    assert.equal(r.headers.get('content-type'), type); assert.equal(r.headers.get('content-length'), String(bytes.length));
    assert.equal(digest(new Uint8Array(await r.arrayBuffer())), digest(bytes));
    ok(`${type} above 12 MiB arrives byte-for-byte (${bytes.length} bytes)`);
  }
  for (const [range, expected, contentRange] of [
    ['bytes=0-15', document.subarray(0, 16), `bytes 0-15/${document.length}`],
    ['bytes=-16', document.subarray(-16), `bytes ${document.length - 16}-${document.length - 1}/${document.length}`],
  ] as const) {
    const r = await fetch(url(pdfPath), {headers: {Range: range}}); assert.equal(r.status, 206);
    assert.equal(r.headers.get('content-range'), contentRange); assert.deepEqual(Buffer.from(await r.arrayBuffer()), expected);
  }
  const badRange = await fetch(url(pdfPath), {headers: {Range: `bytes=${document.length}-`}});
  assert.equal(badRange.status, 416); assert.equal(badRange.headers.get('content-range'), `bytes */${document.length}`); await badRange.text();
  const ifRange = await fetch(url(pdfPath), {headers: {Range: 'bytes=0-15', 'If-Range': '"unverified"'}});
  assert.equal(ifRange.status, 200); assert.equal((await ifRange.arrayBuffer()).byteLength, document.length);
  ok('PDF ranges return 206/416 and unverified If-Range falls back to the full response');
  for (const name of ['small.txt', 'empty.txt']) {
    const r = await fetch(url(join(project, name))); assert.equal(r.status, 200);
    assert.equal(await r.text(), readFileSync(join(project, name), 'utf8'));
  }
  const linked = await fetch(url(join(project, 'linked.png')), {headers: {Range: 'bytes=0-7'}});
  assert.equal(linked.status, 206); assert.deepEqual(Buffer.from(await linked.arrayBuffer()), image.subarray(0, 8));
  for (const path of [join(project, 'missing.png'), join(project, 'blocked.js'), project]) {
    const r = await fetch(url(path)); assert.equal(r.status, 404); await r.text();
  }
  // A non-local Host removes the legacy loopback exemption; a token still
  // works, while a cross-site browser request cannot inherit it.
  const noToken = await fetch(url(imagePath), {headers: {Host: 'preview.invalid'}}); assert.equal(noToken.status, 401); await noToken.text();
  const token = readFileSync(join(home, 'token'), 'utf8').trim();
  const paired = await fetch(url(imagePath), {headers: {Host: 'preview.invalid', Authorization: `Bearer ${token}`, Range: 'bytes=0-7'}});
  assert.equal(paired.status, 206); await paired.arrayBuffer();
  const crossSite = await fetch(url(imagePath), {headers: {Origin: 'https://other.invalid', Authorization: `Bearer ${token}`}});
  assert.equal(crossSite.status, 401); await crossSite.text();
  ok('small/empty text, symlinks, missing files, file-type refusal and pairing/origin gates');

  // Warm the sender before measuring. Four paused 256 MiB responses must
  // not retain four whole files, and health requests must still get a turn.
  const before = rss(), downloads = await Promise.all(Array.from({length: 4}, () => fetch(url(hugePath))));
  downloads.forEach(r => assert.equal(r.status, 200));
  const latencies: number[] = []; let peak = rss();
  for (let i = 0; i < 30; i++) {
    const began = performance.now(), r = await fetch(base + '/hello'); assert.ok(r.ok); await r.text();
    latencies.push(performance.now() - began); peak = Math.max(peak, rss()); await Bun.sleep(20);
  }
  await Promise.all(downloads.map(r => r.body!.cancel()));
  if (process.platform === 'linux') await until(() =>
    readdirSync(`/proc/${proc.pid}/fd`).every(fd => {
      try { return readlinkSync(`/proc/${proc.pid}/fd/${fd}`) !== hugePath; } catch { return true; }
    }), 'cancelled downloads retained open preview descriptors', 5000);
  assert.ok(peak - before < 64 * 1024 * 1024, `paused downloads grew hub RSS by ${peak - before} bytes`);
  assert.ok(Math.max(...latencies) < 1000, `health request took ${Math.max(...latencies)}ms`);
  // Read a complete 256 MiB response without retaining it in the test client.
  const large = await fetch(url(hugePath)), reader = large.body!.getReader(); let received = 0;
  while (true) { const next = await reader.read(); if (next.done) break; received += next.value.byteLength; }
  assert.equal(received, 256 * 1024 * 1024);
  ok(`four paused 256 MiB downloads: RSS growth ${Math.max(0, peak - before)} bytes, max health latency ${Math.max(...latencies).toFixed(1)} ms; full 256 MiB download completes`);

  if (playwright) {
    assert.ok(web && wire, 'browser gate requires matching WEB and WIRE');
    for (const name of readdirSync(wire).filter(f => f.endsWith('.js'))) (0, eval)(readFileSync(join(wire, name), 'utf8'));
    const W = (globalThis as any).Wire, messages: any[] = []; let id = 0;
    ws = new WebSocket(`ws://127.0.0.1:${port}/ws?token=${hubToken(home)}`); ws.binaryType = 'arraybuffer';
    ws.onmessage = event => { try { messages.push(JSON.parse(W.decode(new Uint8Array(event.data as ArrayBuffer)))); } catch {} };
    await new Promise<void>(resolve => { ws!.onopen = () => resolve(); });
    const request = async (m: string, p: any) => {
      const n = ++id; ws!.send(W.encode(JSON.stringify({id: n, m, p})));
      return until(() => messages.find(o => o.id === n), `no answer to ${m}`);
    };
    await request('project.add', {path: project});
    const createdProject = await until(() => messages.flatMap(o => o.items ?? []).find(o => o.$ === 'ProjectCreated' && o.root === realpathSync(project)), 'no project');
    const created = await until(() => messages.flatMap(o => o.items ?? []).find(o => o.$ === 'ThreadCreated' && o.project === createdProject.id), 'no project thread');
    const {chromium, _electron} = await import(resolve(playwright));
    const desktop = process.env.BACKPLANE_PREVIEW_ELECTRON === '1';
    let page;
    if (desktop) {
      const requireDesktop = createRequire(resolve('desktop/package.json'));
      browser = await _electron.launch({executablePath: requireDesktop('electron'), args: [resolve('desktop')],
        env: {...process.env, BACKPLANE_HOME: home, BACKPLANE_SERVICE: resolve(binary), BACKPLANE_WEB: resolve(web),
          BACKPLANE_AGENTS: fake, BACKPLANE_COPY: '1', BACKPLANE_NO_UPDATE: '1', BACKPLANE_PEERS: ''}});
      page = await browser.firstWindow();
      await page.waitForURL(base + '/**');
    } else {
      browser = await chromium.launch({headless: true});
      page = await browser.newPage({viewport: {width: 1440, height: 1000}});
      await page.goto(base + '/#token=' + encodeURIComponent(token));
    }
    uiPage = page;
    page.setDefaultTimeout(10000);
    await page.locator('[data-on-click="ob-skip"]').click();
    // Use the real thread and Mech controls, then decode the served render.
    await page.locator(`[data-on-click="select"][data-value="${created.id}"]`).first().click();
    await page.locator('[data-on-click="mech"]:visible').first().click();
    await page.locator('[data-on-click="mech-renders"]').first().click();
    const shot = page.locator('.mech-grid img').first(); await shot.waitFor();
    await until(() => shot.evaluate((img: HTMLImageElement) => img.complete && img.naturalWidth === 2048), 'Mech render did not decode');
    assert.deepEqual(await shot.evaluate((img: HTMLImageElement) => {
      const c = document.createElement('canvas'); c.width = 2048; c.height = 2048;
      const ctx = c.getContext('2d')!; ctx.drawImage(img, 0, 0);
      return Array.from(ctx.getImageData(321, 456, 1, 1).data);
    }), [65, 200, 73, 255]);
    await shot.click();
    await page.locator('.lightbox img').waitFor();
    await page.screenshot({path: resolve(`build/media-preview/mech-${desktop ? 'electron' : 'browser'}-lightbox.png`)});
    ok(`real ${desktop ? 'Electron' : 'browser'}: Mech render decodes at 2048×2048 with exact sampled pixels; lightbox opens`);
  }
} catch (e) {
  if (uiPage) {
    await uiPage.screenshot({path: resolve('build/media-preview/ui-failure.png')}).catch(() => {});
    writeFileSync('build/media-preview/ui-failure.html', await uiPage.content());
  }
  console.error(readFileSync(join(root, 'stderr'), 'utf8').slice(-8000)); throw e;
} finally {
  ws?.close(); await browser?.close();
  if (proc.exitCode === null) proc.kill('SIGTERM'); await proc.exited;
  if (process.env.BACKPLANE_KEEP_PREVIEW_TEST) console.log('test files: ' + root);
  else rmSync(root, {recursive: true, force: true});
}
