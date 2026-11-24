// The web client's Project Home and Settings › Ring, driven headless against the disposable browser hub of rig.mjs.
// Run with bun test/web/home.mjs <hub test executable> <screenshot directory>, after cargo-q test --bin backplane-hub --no-run in
// native/host. A project with a goal, a fab file, a board and two models: its name opens the home (goal and notes, To order, Boards,
// Mechanical, threads, New thread), a thread row and New thread leave it. One ring note is delivered through the ring's own webhook;
// Settings › Ring lists it, finds it, plays it from the page's origin and asks for it to be heard again, while the ring's setup stays
// refused to a browser. Stand-in agents only; no order server is reachable, so the estimate's lead time and warnings are put into the
// hub's `project.home` answer on its way to the page (a drawing check: the hub's own test covers where they come from).
import assert from 'node:assert/strict';
import { withBrowser, withTestTls } from './rig.mjs';

async function main(binary, output) {
  const fs = await import('node:fs/promises');
  const path = await import('node:path');
  const { spawn } = await import('node:child_process');
  const { once } = await import('node:events');
  const { createServer } = await import('node:net');
  assert.ok(binary && output && process.env.BP_TMP && process.versions.bun);
  const temp = await fs.mkdtemp(path.join(process.env.BP_TMP, 'web-home-'));
  const home = path.join(temp, 'hub');
  const agents = path.join(temp, 'agents');
  const project = path.join(temp, 'project');
  await fs.mkdir(agents);
  await fs.mkdir(path.join(project, '.backplane'), { recursive: true });
  await fs.mkdir(path.join(project, 'mech'));
  await fs.writeFile(path.join(temp, 'registry-clock'), String(Date.now())); // the fixture hub's pairing clock (rig.mjs)
  await fs.mkdir(output, { recursive: true });
  await fs.writeFile(path.join(project, 'ecc83-pp.kicad_pro'), '{}');
  await fs.writeFile(path.join(project, 'ecc83-pp.kicad_pcb'), '(kicad_pcb (version 20240108) (generator "fixture"))\n');
  await fs.writeFile(path.join(project, '.backplane', 'intent.md'), '# Goal\n\nA push-pull amplifier for the bench.\n\n# Parts\n\n- Amp board: the PCB, to order\n- Lid: printed\n\n# Notes\n\nKeep it under 100 mm wide.\n');
  await fs.writeFile(path.join(project, 'amp.fab.json'), JSON.stringify({ name: 'Amp board', kind: 'pcb', source: 'ecc83-pp.kicad_pcb', qty: 5, options: {}, features: {} }));
  await fs.writeFile(path.join(project, 'mech', 'lid.step'), 'ISO-10303-21;\n');
  await fs.writeFile(path.join(project, 'mech', 'amp-assembly.step'), 'ISO-10303-21;\n' + 'x'.repeat(4000));
  const standin = new URL('../tools/claude_standin.ts', import.meta.url).pathname;
  await fs.writeFile(path.join(agents, 'claude'), '#!/bin/sh\nexec bun "$BP_WEB_STANDIN" "$@"\n', { mode: 0o700 });

  const native = createServer();
  native.listen(0, '127.0.0.1');
  await once(native, 'listening');
  const nativePort = native.address().port;
  await new Promise(resolve => native.close(resolve));
  assert.ok(nativePort >= 3799);

  try {
    await withTestTls(path.join(home, 'browser.sock'), async tls => {
      const env = { PATH: process.env.PATH, HOME: temp, TMPDIR: temp, BP_TMP: temp, BACKPLANE_VAULT_KEYRING: 'file',
        BP_WEB_MASK_FIXTURE: 'fixture-mask-home', BP_WEB_STANDIN: standin, BP_WEB_FIXTURE: temp, BACKPLANE_AGENTS: agents,
        BP_WEB_NATIVE_PORT: String(nativePort), BP_WEB_TLS_PORT: String(tls.port),
        BACKPLANE_NO_UPDATE: '1', BACKPLANE_ORDER_SERVER: 'http://127.0.0.1:9' };
      const child = spawn(binary, ['--ignored', '--exact', 'net::listener::webingress::isolated_browser_hub'], {
        env, stdio: ['ignore', 'pipe', 'pipe'], detached: true,
      });
      const logs = [];
      for (const stream of [child.stdout, child.stderr]) stream.on('data', chunk => logs.push(chunk));
      const exited = once(child, 'exit');
      try {
        for (let i = 0; ; ++i) {
          if (await fs.stat(path.join(home, 'browser.sock')).then(s => s.isSocket(), () => false)) break;
          assert.ok(child.exitCode === null && i < 400, 'fixture failed to start: ' + Buffer.concat(logs).toString().slice(-2000));
          await Bun.sleep(50);
        }
        const credential = (await fs.readFile(path.join(home, 'token'), 'utf8')).trim();
        const ws = new WebSocket('ws://127.0.0.1:' + nativePort + '/ws?data=1&enc=json', { headers: { Authorization: 'Bearer ' + credential } });
        const replies = new Map();
        let next = 0;
        ws.onmessage = event => {
          if (typeof event.data !== 'string') return;
          const frame = JSON.parse(event.data);
          if (frame.t === 'reply') replies.get(frame.id)?.(frame);
        };
        await new Promise((resolve, reject) => { ws.onopen = resolve; ws.onerror = () => reject(Error('native fixture connection failed')); });
        const rpc = async (method, params) => {
          const id = ++next;
          try {
            const frame = await new Promise((resolve, reject) => {
              const timer = setTimeout(() => reject(Error(method + ' timed out')), 10000);
              replies.set(id, frame => { clearTimeout(timer); resolve(frame); });
              ws.send(JSON.stringify({ id, m: method, p: params }));
            });
            assert.equal(frame.ok, true, method + ' failed: ' + JSON.stringify(frame.error));
            return frame.result;
          } finally { replies.delete(id); }
        };
        try {
          await rpc('devices.page.act', { action: 'device-web-access', value: '1' });

          // one ring note, through the ring's own webhook: set up natively (a browser may not), then the Pebble app's form
          const ring = await rpc('ring.setup', {});
          assert.ok(ring.hook && ring.token, 'ring.setup: ' + JSON.stringify(ring));
          const audio = Buffer.from('\x00\x00\x00\x18ftypM4A fixture-recording', 'latin1');
          const said = Date.now() - 3600_000;
          const form = Buffer.concat([
            Buffer.from('--bnd\r\nContent-Disposition: form-data; name="audio"; filename="rec-1.m4a"\r\nContent-Type: audio/mp4\r\n\r\n'), audio,
            Buffer.from('\r\n--bnd\r\nContent-Disposition: form-data; name="transcription"\r\n\r\nRemind me to order the amp boards\r\n'
              + '--bnd\r\nContent-Disposition: form-data; name="recordedAt"\r\n\r\n' + said + '\r\n'
              + '--bnd\r\nContent-Disposition: form-data; name="client"\r\n\r\nring\r\n--bnd--\r\n'),
          ]);
          const posted = await fetch('http://127.0.0.1:' + nativePort + '/hook/' + ring.hook, { method: 'POST', body: form, headers: {
            'Content-Type': 'multipart/form-data; boundary=bnd', Authorization: 'Bearer ' + ring.token,
            'X-Index-Webhook-Version': '1', 'X-Index-Delivery': 'd-home-1', 'X-Index-Trigger': 'single-click-hold' } });
          assert.equal(posted.status, 200, 'the ring webhook: ' + await posted.text());
          let noteId = '';
          for (let i = 0; i < 100 && !noteId; ++i) {
            noteId = (await rpc('ring.list', {})).notes?.[0]?.id ?? '';
            if (!noteId) await Bun.sleep(100);
          }
          assert.match(noteId, /^n[0-9a-f]{16}$/, 'the note is kept');

          await withBrowser(tls, async (context, origin) => {
            // the estimate's lead time and warnings, put into `project.home`'s answer (no order server here); the page's own requests,
            // by method, for the checks below. The page asks for plain frames here (no &z=deflate), so they can be read
            await context.addInitScript(() => {
              const Native = window.WebSocket;
              window.bpCalls = [];
              window.WebSocket = class extends Native {
                constructor(url, protocols) { super(String(url).replace('&z=deflate', ''), protocols); this.homeIds = new Set(); window.bpSocket = this; }
                send(data) {
                  try { const f = JSON.parse(data); if (f.m) window.bpCalls.push(f.m); if (f.m === 'project.home') this.homeIds.add(f.id); } catch { /* binary */ }
                  return super.send(data);
                }
                addEventListener(type, fn, opts) {
                  if (type !== 'message') return super.addEventListener(type, fn, opts);
                  return super.addEventListener(type, ev => {
                    let f = null;
                    if (typeof ev.data === 'string') { try { f = JSON.parse(ev.data); } catch { /* not JSON */ } }
                    if (f && f.t === 'reply' && f.ok && this.homeIds.has(f.id) && window.bpEstimate) {
                      window.bpHomeRaw = JSON.parse(JSON.stringify(f.result));
                      Object.assign(f.result, window.bpEstimate);
                      fn.call(this, new MessageEvent('message', { data: JSON.stringify(f) }));
                      return;
                    }
                    if (f && f.t === 'reply' && f.ok && this.homeIds.has(f.id)) window.bpHomeRaw = f.result;
                    fn.call(this, ev);
                  }, opts);
                }
              };
            });
            const page = await context.newPage();
            const errors = [];
            page.on('pageerror', e => errors.push(String(e)));
            if (process.env.BP_HOME_DEBUG) {
              page.on('console', m => console.error('console', m.type(), m.text()));
              page.on('response', r => { if (r.status() >= 400) console.error('http', r.status(), r.url()); });
            }
            const testId = id => page.getByTestId(id);
            const shot = name => page.screenshot({ path: path.join(output, name + '.png') });
            await page.setViewportSize({ width: 1280, height: 860 });
            await page.goto(origin);
            await testId('pair-card').waitFor();
            const name = 'Home browser';
            await testId('pair-name').fill(name);
            await testId('pair-request-btn').click();
            await testId('pair-code').waitFor();
            const { rows } = await rpc('devices.page', {});
            const row = rows.find(r => r.label === name && r.buttons?.some(b => b.action === 'device-web-approve'));
            assert.ok(row, 'no approval row');
            await rpc('devices.page.act', row.buttons.find(b => b.action === 'device-web-approve'));
            await testId('composer-input').waitFor();
            await testId('timeline').waitFor();

            // 1. the project's name opens its home: the hub's own answer first, as it is
            // (the fixture's project has had no turn: it is under the Quiet fold until that is opened)
            await testId('sidebar-list').waitFor();
            if (await page.locator('[data-testid^="project-open-"]').count() === 0) await testId('section-toggle-quiet').click();
            const open = page.locator('[data-testid^="project-open-"]').first();
            await open.waitFor();
            await open.click();
            await testId('project-home').waitFor();
            await testId('project-home-intent').waitFor();
            assert.equal(await testId('project-home-title').innerText(), 'Browser fixture');
            const intent = await testId('project-home-intent').innerText();
            assert.match(intent, /Goal\s+A push-pull amplifier for the bench\./);
            assert.match(intent, /Amp board: the PCB, to order/);
            assert.match(intent, /Keep it under 100 mm wide\./);
            assert.match(await testId('project-home-part-0').innerText(), /Amp board\s+pcb\s+amp\.fab\.json/);
            assert.match(await testId('project-home-board-0').innerText(), /ecc83-pp\.kicad_pcb/);
            assert.match(await testId('project-home-mech-0').innerText(), /amp-assembly\.step\s+Assembly/);
            assert.match(await testId('project-home-mech-1').innerText(), /lid\.step/);
            await testId('project-home-thread-t1').waitFor();
            assert.equal(await testId('project-home-lead').count(), 0, 'no estimate, no lead time');
            assert.equal(await testId('project-home-warning').count(), 0);
            const raw = await page.evaluate(() => window.bpHomeRaw);
            assert.deepEqual(Object.keys(raw).sort(), ['boards', 'intent', 'leadDays', 'mech', 'parts', 'project', 'root', 'warnings']);
            assert.equal(raw.leadDays, 0);
            assert.equal(await testId('composer-input').isVisible(), false, 'the home covers the thread and its composer');
            assert.equal(await testId('thread-title').innerText(), '', 'the bar names the project, no thread');
            assert.equal(await page.locator('.bp-thread-row.active').count(), 0, 'no thread row is marked while the home is on show');
            await shot('home-1-project');

            // 2. an estimate's lead time and warnings are drawn on the To order card
            await page.evaluate(() => { window.bpEstimate = { leadDays: 9, warnings: ['thin board', 'fragile'] }; });
            await testId('thread-row-t1').click();
            await testId('project-home').waitFor({ state: 'hidden' });
            await open.click();
            await testId('project-home-lead').waitFor();
            assert.equal(await testId('project-home-lead').innerText(), 'Lead time: 9 days');
            assert.deepEqual(await testId('project-home-warning').allInnerTexts(), ['thin board', 'fragile']);
            await shot('home-2-lead-and-warnings');
            await page.evaluate(() => { window.bpEstimate = null; });

            // 3. the chevron folds the project's threads and leaves the home on show; again unfolds
            const fold = page.locator('[data-testid^="project-fold-"]').first();
            if (await fold.isEnabled()) {
              await fold.click();
              await page.waitForFunction(() => document.querySelector('[data-testid^="project-fold-"]')?.getAttribute('aria-expanded') === 'false');
              assert.ok(await testId('project-home').isVisible());
              await page.locator('[data-testid^="project-fold-"]').first().click();
              await page.waitForFunction(() => document.querySelector('[data-testid^="project-fold-"]')?.getAttribute('aria-expanded') === 'true');
            }

            // 4. a thread row opens the thread and the home is gone
            await testId('project-home-thread-t1').click();
            await testId('project-home').waitFor({ state: 'hidden' });
            await testId('composer-input').waitFor();
            await page.waitForFunction(() => document.querySelector('[data-testid="thread-row-t1"]')?.classList.contains('active'));
            assert.equal(await testId('thread-title').innerText(), 'Browser test');

            // 5. New thread on the home makes a thread in the project and opens it
            const before = (await rpc('threads.list', { project: 'p1' })).threads.length;
            await open.click();
            await testId('project-home').waitFor();
            await testId('project-home-new-thread').click();
            await testId('project-home').waitFor({ state: 'hidden' });
            await testId('composer-input').waitFor();
            const after = (await rpc('threads.list', { project: 'p1' })).threads;
            assert.equal(after.length, before + 1, 'New thread made one thread');
            await shot('home-3-new-thread');

            // 6. a narrow screen: the home still reads
            await page.setViewportSize({ width: 390, height: 780 });
            await page.evaluate(() => document.querySelector('.bp-side-toggle')?.click());
            await open.click();
            await testId('project-home-intent').waitFor();
            const wide = await page.evaluate(() => document.documentElement.scrollWidth > document.documentElement.clientWidth);
            assert.equal(wide, false, 'nothing overflows sideways at 390 px');
            await shot('home-4-phone');
            await page.setViewportSize({ width: 1280, height: 860 });

            // 7. Settings › Ring: the note, found by its words, played from this origin, heard again; the ring's setup stays native
            await testId('settings-btn').click();
            await testId('settings-nav-ring').click();
            await testId('ring-note-' + noteId).waitFor();
            assert.match(await testId('ring-note-' + noteId).innerText(), /Remind me to order the amp boards/);
            assert.equal(await testId('ring-count').innerText(), '1 recording');
            await shot('ring-1-list');
            await testId('ring-search').fill('zebra');
            await testId('ring-search').press('Enter');
            await page.waitForFunction(() => /0 of 1 have “zebra”\./.test(document.querySelector('[data-testid="ring-count"]')?.textContent || ''));
            assert.equal(await testId('ring-note-' + noteId).count(), 0);
            await testId('ring-search').fill('AMP boards');
            await testId('ring-search').press('Enter');
            await testId('ring-note-' + noteId).waitFor();
            const src = await testId('ring-audio-' + noteId).getAttribute('src');
            assert.match(src, new RegExp('^/ring/audio/' + noteId + '\\?k=[0-9a-f]+$'));
            const played = await page.evaluate(async s => {
              const r = await fetch(s);
              return { status: r.status, type: r.headers.get('content-type'), bytes: Array.from(new Uint8Array(await r.arrayBuffer())) };
            }, src);
            assert.equal(played.status, 200);
            assert.equal(played.type, 'audio/mp4');
            assert.deepEqual(Buffer.from(played.bytes), audio, 'the recording as it came');
            const refused = await page.evaluate(async s => [
              (await fetch(s.replace(/k=.*/, 'k=00'))).status,
              (await fetch('/ring/audio/nope?k=00')).status,
            ], src);
            assert.deepEqual(refused, [404, 404], 'a wrong key and a name that is no recording');
            const part = await page.evaluate(async s => {
              const r = await fetch(s, { headers: { Range: 'bytes=4-7' } });
              return { status: r.status, range: r.headers.get('content-range'), bytes: Array.from(new Uint8Array(await r.arrayBuffer())) };
            }, src);
            assert.deepEqual(part, { status: 206, range: 'bytes 4-7/' + audio.length, bytes: Array.from(audio.subarray(4, 8)) }, 'a player that asks in parts gets the part');
            const blocked = await page.evaluate(() => {
              const violations = [];
              document.addEventListener('securitypolicyviolation', e => violations.push(e.violatedDirective));
              return new Promise(resolve => {
                const a = document.querySelector('audio.bp-ring-audio');
                a.load();
                setTimeout(() => resolve(violations), 600);
              });
            });
            assert.deepEqual(blocked, [], 'the page may load the recording');
            // hearing again changes the row in place: the player on show is the same element (a recording that plays keeps playing)
            await page.evaluate(() => { document.querySelector('audio.bp-ring-audio').dataset.kept = '1'; });
            await testId('ring-hear-local-' + noteId).click();
            await page.waitForFunction(() => window.bpCalls.includes('ring.hear'));
            await page.waitForFunction(() => window.bpCalls.filter(m => m === 'ring.list').length >= 4, null, { timeout: 15000 });
            assert.equal(await page.evaluate(() => document.querySelector('audio.bp-ring-audio')?.dataset.kept), '1', 'the list read again did not rebuild the player');
            await shot('ring-2-heard-again');
            const calls = await page.evaluate(() => window.bpCalls);
            assert.ok(calls.includes('ring.list') && calls.includes('ring.hear') && calls.includes('project.home'));
            for (const m of ['ring.page', 'ring.setup', 'ring.send', 'ring.get', 'ring.search', 'ring.page.act']) assert.ok(!calls.includes(m), 'the page never asks ' + m);
            const denied = await page.evaluate(() => new Promise((resolve, reject) => {
              const out = {};
              const methods = ['ring.page', 'ring.setup', 'ring.send', 'ring.get', 'ring.search', 'ring.page.act', 'thread.buy', 'order.buy'];
              const socket = new WebSocket('/ws?data=1&enc=json');
              const timer = setTimeout(() => { socket.close(); reject(Error('browser RPC timeout')); }, 5000);
              socket.addEventListener('open', () => methods.forEach((m, i) => socket.send(JSON.stringify({ id: i + 1, m, p: { note: 'n0000000000000000', thread: 't1' } }))));
              socket.addEventListener('message', ev => {
                if (typeof ev.data !== 'string') return;
                const f = JSON.parse(ev.data);
                if (f.t !== 'reply') return;
                out[methods[f.id - 1]] = f.ok ? 'ok' : f.error;
                if (Object.keys(out).length === methods.length) { clearTimeout(timer); socket.close(); resolve(out); }
              });
            }));
            for (const [m, said] of Object.entries(denied)) assert.equal(said, 'not allowed for browser', m);

            assert.deepEqual(errors, [], 'page errors: ' + errors.join('\n'));
            console.log('web home: ok', JSON.stringify({ note: noteId, threads: after.length }));
          });
        } catch (e) {
          await fs.writeFile(path.join(output, 'hub.log'), Buffer.concat(logs));
          throw e;
        } finally { ws.close(); }
      } finally {
        if (child.pid && child.exitCode === null) { try { process.kill(-child.pid, 'SIGTERM'); } catch { /* already gone */ } }
        await exited;
      }
    });
  } finally { await fs.rm(temp, { recursive: true, force: true }); }
}

if (import.meta.main) await main(...process.argv.slice(2));
