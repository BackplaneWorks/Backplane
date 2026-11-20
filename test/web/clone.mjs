// Add project's Clone from git in the web client (docs/CLONE.md), driven headless against the disposable browser hub of rig.mjs.
// Run with bun test/web/clone.mjs <hub test executable> <screenshot directory>, after cargo-q test --bin backplane-hub --no-run in
// native/host. Clones a local bare repository (file://), checks a refused destination, a failed clone's Try again, and that a done clone
// selects its thread. Stand-in agents only; HOME is the temp folder, so gh (if installed) is signed out.
import assert from 'node:assert/strict';
import { withBrowser, withTestTls } from './rig.mjs';

async function main(binary, output) {
  const fs = await import('node:fs/promises');
  const path = await import('node:path');
  const { spawn, execFileSync } = await import('node:child_process');
  const { once } = await import('node:events');
  const { createServer } = await import('node:net');
  assert.ok(binary && output && process.env.BP_TMP && process.versions.bun);
  const temp = await fs.mkdtemp(path.join(process.env.BP_TMP, 'web-clone-'));
  const home = path.join(temp, 'hub');
  const agents = path.join(temp, 'agents');
  const project = path.join(temp, 'project');
  await fs.mkdir(agents);
  await fs.mkdir(project);
  await fs.writeFile(path.join(temp, 'registry-clock'), String(Date.now())); // the fixture hub's pairing clock (rig.mjs)
  await fs.mkdir(path.join(temp, 'Documents'));
  await fs.mkdir(output, { recursive: true });
  await fs.writeFile(path.join(project, 'ecc83-pp.kicad_pro'), '{}');
  const standin = new URL('../tools/claude_standin.ts', import.meta.url).pathname;
  await fs.writeFile(path.join(agents, 'claude'), '#!/bin/sh\nexec bun "$BP_WEB_STANDIN" "$@"\n', { mode: 0o700 });

  // a repository on this machine: src (one commit) -> demo.git (bare)
  const gitEnv = { ...process.env, HOME: temp, GIT_CONFIG_GLOBAL: '/dev/null', GIT_CONFIG_NOSYSTEM: '1' };
  const git = (cwd, ...args) => execFileSync('git', ['-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.com', ...args], { cwd, env: gitEnv, stdio: 'pipe' });
  const src = path.join(temp, 'src');
  await fs.mkdir(src);
  git(src, 'init', '-q', '-b', 'main');
  await fs.writeFile(path.join(src, 'README.md'), '# demo\n');
  git(src, 'add', '.');
  git(src, 'commit', '-q', '-m', 'first');
  git(temp, 'clone', '-q', '--bare', src, path.join(temp, 'demo.git'));
  const repoUrl = 'file://' + path.join(temp, 'demo.git');
  // a folder that is there and holds something: refused as a destination
  await fs.mkdir(path.join(temp, 'Documents', 'busy'));
  await fs.writeFile(path.join(temp, 'Documents', 'busy', 'x.txt'), 'x');

  const native = createServer();
  native.listen(0, '127.0.0.1');
  await once(native, 'listening');
  const nativePort = native.address().port;
  await new Promise(resolve => native.close(resolve));
  assert.ok(nativePort >= 3799);

  try {
    await withTestTls(path.join(home, 'browser.sock'), async tls => {
      const env = { PATH: process.env.PATH, HOME: temp, TMPDIR: temp, BP_TMP: temp, BACKPLANE_VAULT_KEYRING: 'file',
        BP_WEB_MASK_FIXTURE: 'fixture-mask-clone', BP_WEB_STANDIN: standin, BP_WEB_FIXTURE: temp, BACKPLANE_AGENTS: agents,
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
          await withBrowser(tls, async (context, origin) => {
            // the hub's web ingress takes only the exact target /ws?data=1&enc=json (net/webingress.rs) and the client asks for
            // &z=deflate too: until the ingress takes it, the page asks without it (the client reads uncompressed frames as well)
            // the race in question: clone.start's reply is held back 1.5 s, so a fast clone's pushes (done) reach the page before it
            await context.addInitScript(() => {
              const Native = window.WebSocket;
              window.cloneLog = [];
              window.WebSocket = class extends Native {
                constructor(url, protocols) {
                  super(String(url).replace('&z=deflate', ''), protocols);
                  this.startIds = new Set();
                }
                send(data) {
                  try { const f = JSON.parse(data); if (f.m === 'clone.start') this.startIds.add(f.id); } catch { /* binary */ }
                  return super.send(data);
                }
                addEventListener(type, fn, opts) {
                  if (type !== 'message') return super.addEventListener(type, fn, opts);
                  return super.addEventListener(type, ev => {
                    let f = null;
                    if (typeof ev.data === 'string') { try { f = JSON.parse(ev.data); } catch { /* not JSON */ } }
                    if (f && f.t === 'reply' && this.startIds.has(f.id)) {
                      setTimeout(() => { window.cloneLog.push('reply:' + (f.result?.state ?? 'error')); fn.call(this, ev); }, 1500);
                      return;
                    }
                    if (f && f.t === 'd' && f.k === 'clone') window.cloneLog.push('push:' + f.clone.state);
                    fn.call(this, ev);
                  }, opts);
                }
              };
            });
            const page = await context.newPage();
            const errors = [];
            page.on('pageerror', e => errors.push(String(e)));
            if (process.env.BP_CLONE_DEBUG) {
              page.on('console', m => console.error('console', m.type(), m.text()));
              page.on('websocket', w => { console.error('ws open', w.url()); w.on('close', () => console.error('ws close')); w.on('socketerror', e => console.error('ws error', e)); });
              page.on('response', r => { if (r.status() >= 400) console.error('http', r.status(), r.url()); });
            }
            const testId = id => page.getByTestId(id);
            const shot = name => page.screenshot({ path: path.join(output, name + '.png') });
            await page.setViewportSize({ width: 1280, height: 800 });
            await page.goto(origin);
            await testId('pair-card').waitFor();
            const name = 'Clone browser';
            await testId('pair-name').fill(name);
            await testId('pair-request-btn').click();
            await testId('pair-code').waitFor();
            const { rows } = await rpc('devices.page', {});
            const row = rows.find(r => r.label === name && r.buttons?.some(b => b.action === 'device-web-approve'));
            assert.ok(row, 'no approval row');
            await rpc('devices.page.act', row.buttons.find(b => b.action === 'device-web-approve'));
            await testId('composer-input').waitFor();
            await testId('timeline').waitFor();
            await shot('clone-0-paired');

            // 1. the sidebar's +: Open folder, a folder browser
            await testId('sidebar-add-project').click();
            await testId('add-project').waitFor();
            await testId('folder-browser').waitFor();
            await page.waitForFunction(() => document.querySelector('[data-testid="folder-path"]')?.value.length > 0);
            const act = await testId('folder-act').innerText();
            assert.match(act, /^Add /);
            await shot('clone-1-open-folder');

            // 2. Clone from git: the repository field, gh's state under it
            await testId('ap-tab-clone').click();
            await testId('clone-repo').waitFor();
            assert.equal(await testId('clone-repo').getAttribute('placeholder'), 'https or ssh URL, or owner/repo');
            await testId('clone-repo').focus();
            await page.waitForTimeout(800);
            await shot('clone-2-repo-field');
            await page.keyboard.press('Escape');
            await page.keyboard.press('Escape');
            await testId('add-project').waitFor({ state: 'detached' });

            // 3. /project <address> <message> in the composer opens Clone from git on it, its destination asked
            await testId('composer-input').fill('/project ' + repoUrl + ' hello from the clone');
            await testId('composer-input').press('Enter');
            await testId('add-project').waitFor();
            assert.equal(await testId('clone-repo').inputValue(), repoUrl);
            await page.waitForFunction(() => /demo$/.test(document.querySelector('[data-testid="clone-dest-shown"]')?.textContent || ''));
            assert.equal(await testId('clone-dest-shown').innerText(), '~/Documents/demo');
            assert.equal(await testId('clone-name').inputValue(), 'demo');
            assert.ok(await testId('clone-thread').isChecked(), 'Start a thread is on by default');
            assert.equal(await testId('clone-go').isDisabled(), false);
            await shot('clone-3-destination');

            // 4. a destination that holds something: the hub's words, Clone disabled; back to the default name
            await testId('clone-name').fill('busy');
            await testId('clone-dest-error').waitFor();
            assert.match(await testId('clone-dest-error').innerText(), /already exists and isn't empty/);
            assert.equal(await testId('clone-go').isDisabled(), true);
            await shot('clone-4-refused');
            await testId('clone-name').fill('demo');
            await testId('clone-dest-error').waitFor({ state: 'detached' });
            await page.waitForFunction(() => !document.querySelector('[data-testid="clone-go"]')?.disabled);

            // 5. Clone: done with nothing to read, the dialog closes and the new thread is selected
            const before = (await rpc('clone.status', {})).jobs?.length ?? 0;
            await testId('clone-go').click();
            await testId('add-project').waitFor({ state: 'detached', timeout: 30000 });
            const jobs = (await rpc('clone.status', {})).jobs;
            assert.equal(jobs.length, before + 1);
            const job = jobs[jobs.length - 1];
            assert.equal(job.state, 'done', JSON.stringify(job));
            assert.match(job.id, /^w\d+-[A-Za-z0-9]+$/, 'the job id is the client\'s');
            await page.waitForFunction(() => window.cloneLog.some(e => e.startsWith('reply:')));
            await page.waitForTimeout(300); // the late reply had its chance to stick
            const log = await page.evaluate(() => window.cloneLog);
            assert.equal(await testId('add-project').count(), 0, 'the late running reply did not bring the dialog back');
            assert.ok(log.indexOf('push:done') >= 0 && log.indexOf('push:done') < log.indexOf('reply:running'),
              'the done push came before the (running) reply: ' + log.join(' '));
            assert.ok(job.thread, 'a thread was made');
            assert.equal(await fs.readFile(path.join(temp, 'Documents', 'demo', 'README.md'), 'utf8'), '# demo\n');
            await page.waitForFunction(id => document.querySelector(`[data-testid="thread-row-${id}"]`)?.classList.contains('active'), job.thread, { timeout: 10000 });
            await page.waitForTimeout(500);
            await shot('clone-5-thread');

            // 6. a clone that fails: its words and Try again (a new job, followed)
            await testId('sidebar-add-project').click();
            await testId('ap-tab-clone').click();
            await testId('clone-repo').fill('file://' + path.join(temp, 'missing.git'));
            await page.waitForFunction(() => /missing$/.test(document.querySelector('[data-testid="clone-dest-shown"]')?.textContent || ''));
            await page.waitForFunction(() => !document.querySelector('[data-testid="clone-go"]')?.disabled);
            await testId('clone-go').click();
            await testId('clone-error').waitFor({ timeout: 30000 });
            await testId('clone-retry').waitFor();
            await shot('clone-6-failed');
            const failedId = (await rpc('clone.status', {})).jobs.at(-1).id;
            await testId('clone-retry').click();
            await page.waitForFunction(() => {
              return document.querySelector('[data-testid="clone-retry"]') && !document.querySelector('[data-testid="clone-retry"]').disabled;
            }, null, { timeout: 30000 });
            assert.equal(await testId('clone-job').count(), 1, 'the retried (failed) job is on show');
            const retried = (await rpc('clone.status', {})).jobs.at(-1);
            assert.notEqual(retried.id, failedId, 'Try again started a new job');
            assert.match(retried.id, /^w\d+-/, 'Try again names its job itself');
            assert.equal(retried.dest, (await rpc('clone.status', { id: failedId })).dest, 'Try again keeps the folder');
            assert.equal(retried.state, 'failed');
            assert.deepEqual(errors, [], 'page errors: ' + errors.join('\n'));
            console.log('web clone: ok', JSON.stringify({ job: job.id, thread: job.thread, retried: retried.id }));
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
