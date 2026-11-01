// Real desktop and Bend hub, isolated homes and a test-only fault effect.
// Build test/native/hub_diagnostics.bend as backplane-serve, then set
// BACKPLANE_SERVICE and BACKPLANE_WEB. No live history or agent is used.
'use strict';
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const os = require('node:os');
const { createHash } = require('node:crypto');
const execFile = require('node:util').promisify(require('node:child_process').execFile);
const { _electron } = require('playwright-core');
const { Service } = require('../service.cjs');
const root = path.resolve(__dirname, '../..');
const binary = process.env.BACKPLANE_SERVICE, web = process.env.BACKPLANE_WEB;
assert(binary && path.isAbsolute(binary));assert(web && path.isAbsolute(web));
const out = process.env.BACKPLANE_DIAGNOSTICS_EVIDENCE || path.join(root, 'build/hub-diagnostics/practical');
assert(path.isAbsolute(out));
let app;
const owned = new Set();
const receipts = [];
let binaryHash;
const sourceFiles = ['desktop/service.cjs', 'desktop/service-diagnostics.cjs', 'src/server/effects/loop.js',
  'test/native/hub_diagnostics.bend', 'test/native/hub-diagnostics.js', 'desktop/test/hub-diagnostics.cjs'];
const hash = async file => createHash('sha256').update(await fs.readFile(file)).digest('hex');

async function until(fn, label, ms = 15000) {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    const answer = await fn();if (answer) return answer;
    await new Promise(resolve => setTimeout(resolve, 50));
  }
  throw Error('Timed out: ' + label);
}
async function records(home) {
  const dir = path.join(home, 'diagnostics');
  const names = await fs.readdir(dir).catch(() => []);
  return Promise.all(names.filter(n => n.endsWith('.json')).map(async name => ({ name, ...JSON.parse(await fs.readFile(path.join(dir, name), 'utf8')) })));
}
async function launch(home, fault, trigger) {
  const previous = new Set((await records(home)).map(r => r.name));
  app = await _electron.launch({ executablePath: require('electron'), args: [path.join(root, 'desktop')],
    env: { ...process.env, BACKPLANE_HOME: home, BACKPLANE_SERVICE: binary, BACKPLANE_WEB: web,
      BACKPLANE_COPY: '1', BACKPLANE_AGENTS: path.join(home, 'agents'), BACKPLANE_NO_UPDATE: '1',
      BACKPLANE_TELEMETRY: '0', BACKPLANE_PEERS: '', BACKPLANE_TEST_FAULT: fault,
      BACKPLANE_TEST_FAULT_FILE: trigger }, timeout: 45000 });
  const page = await app.firstWindow();
  const run = await until(async () => (await records(home)).find(r => !previous.has(r.name) && r.state === 'running' && r.pid), 'captured hub PID');
  owned.add(run.pid);
  assert.equal(run.binarySha256, binaryHash);
  await page.waitForURL(/^http:\/\/127\.0\.0\.1:\d+\/?$/, { timeout: 45000 });
  await page.getByPlaceholder('Search projects').waitFor({ timeout: 45000 });
  const versions = await page.evaluate(() => window.backplaneDesktop.version());
  assert(versions.service, 'authenticated desktop reports the hub version');
  assert.equal(new URL(page.url()).hostname, '127.0.0.1');
  await page.screenshot({ path: path.join(out, `${fault}-connected.png`) });
  return { page, run, origin: new URL(page.url()).origin };
}
async function homeFor(name) {
  const home = await fs.mkdtemp(path.join(os.tmpdir(), 'bp-diagnostics-' + name + '-'));
  await fs.mkdir(path.join(home, 'agents'));
  const at = Math.floor(Date.now() / 1000);
  await fs.writeFile(path.join(home, 'events.jsonl'), ['onboard.done', 'mail.on', 'restart.continue'].map((key, i) =>
    JSON.stringify({ $: 'SettingSet', key, value: i === 0 ? '1' : 'off', at })).join('\n') + '\n');
  return home;
}

async function main() {
  await fs.mkdir(out, { recursive: true });
  await fs.rm(path.join(out, 'receipt.json'), { force: true });
  binaryHash = await hash(binary);
  const sourceHashes = Object.fromEntries(await Promise.all(sourceFiles.map(async file => [file, await hash(path.join(root, file))])));
  for (const fault of ['range', 'exit', 'signal', 'after-desktop-quit']) {
    const home = await homeFor(fault), trigger = path.join(home, 'trigger-fault');
    const actualFault = fault === 'after-desktop-quit' ? 'range' : fault;
    let current = await launch(home, actualFault, trigger);
    if (fault === 'after-desktop-quit') {
      await app.close();app = null;
      const response = await fetch(current.origin + '/hello');assert.equal(response.status, 200, 'hub survives desktop quit');
    }
    if (fault === 'signal') process.kill(current.run.pid, 'SIGTERM');
    else await fs.writeFile(trigger, 'go');
    const failed = await until(async () => {
      const rs = await records(home);
      return fault === 'range' || fault === 'after-desktop-quit'
        ? rs.find(r => r.exception === 'RangeError') : rs.find(r => r.state === 'exited');
    }, 'durable failure record');
    if (fault === 'range' || fault === 'after-desktop-quit') {
      assert(failed.stackAvailable);
      assert(failed.frames.some(f => /diagnostic_fault_origin/.test(f.function || '')));
    } else {
      assert.equal(failed.exitCode, fault === 'exit' ? 23 : null);
      assert.equal(failed.signal, fault === 'signal' ? 'SIGTERM' : null);
    }
    if (app) {
      await until(async () => (await records(home)).some(r => r.state === 'exited'), 'desktop observes hub exit');
      await app.close();app = null;
    }
    owned.delete(current.run.pid);
    const before = await records(home);
    const prior = before.find(r => r.name === current.run.name);
    assert(prior.version && /^[a-f0-9]{64}$/.test(prior.binarySha256));
    if (fault === 'after-desktop-quit') {
      assert.equal(prior.state, 'running', 'only the last observation is retained');
      assert(!('exitCode' in prior), 'an absent observer cannot invent an exit code');
    }
    assert(!JSON.stringify(before).includes('PRIVATE_PROMPT'));assert(!JSON.stringify(before).includes('SECRET'));
    await fs.unlink(trigger).catch(() => {});
    current = await launch(home, 'none', trigger);
    const after = await records(home);
    assert(after.some(r => r.name === failed.name), 'failure survives a fresh desktop and hub');
    await app.close();app = null;
    assert.equal((await fetch(current.origin + '/hello')).status, 200, 'restarted hub survives desktop quit');
    receipts.push({ fault, home, failed, restartPid: current.run.pid, previousRecordRetained: true, residentAfterQuit: true });
    process.kill(current.run.pid, 'SIGTERM');owned.delete(current.run.pid);
  }
  {
    const home = receipts.find(r => r.fault === 'range').home, trigger = path.join(home, 'trigger-fault');
    const dir = path.join(home, 'diagnostics'), file = path.join(dir, 'parent.exception.json');
    // The helper runs only after the hub has initialized its store.
    await fs.mkdir(dir, { recursive: true, mode: 0o700 });
    await fs.writeFile(file, 'parent report\n', { mode: 0o600 });
    await fs.writeFile(trigger, 'go');
    await execFile(binary, ['--store-terms', home], { timeout: 15000, env: {
      ...process.env, BACKPLANE_COPY: '1', BACKPLANE_AGENTS: path.join(home, 'agents'),
      BACKPLANE_TEST_FAULT: 'range', BACKPLANE_TEST_FAULT_FILE: trigger,
      BACKPLANE_DIAGNOSTICS_FILE: file,
    } });
    assert.equal(await fs.readFile(file, 'utf8'), 'parent report\n');
    receipts.push({ fault: 'store-terms-child', exitedNormally: true, reportUnchanged: true });
  }
  for (const fault of ['startup-range', 'missing-binary']) {
    const home = await homeFor(fault), trigger = path.join(home, 'trigger-fault');
    if (fault === 'startup-range') await fs.writeFile(trigger, 'go');
    app = await _electron.launch({ executablePath: require('electron'), args: [path.join(root, 'desktop')],
      env: { ...process.env, BACKPLANE_HOME: home,
        BACKPLANE_SERVICE: fault === 'missing-binary' ? path.join(home, 'missing-service') : binary,
        BACKPLANE_WEB: web, BACKPLANE_COPY: '1', BACKPLANE_AGENTS: path.join(home, 'agents'),
        BACKPLANE_NO_UPDATE: '1', BACKPLANE_TELEMETRY: '0', BACKPLANE_PEERS: '',
        BACKPLANE_TEST_FAULT: 'range', BACKPLANE_TEST_FAULT_FILE: trigger } });
    const page = await app.firstWindow();
    await page.getByText(fault === 'missing-binary' ? 'Cannot start the packaged service' : 'Service exited (1)', { exact: true }).waitFor();
    const failed = await until(async () => (await records(home)).find(r =>
      fault === 'missing-binary' ? r.state === 'launch-failed' : r.exception === 'RangeError'), 'startup failure');
    if (fault === 'missing-binary') assert.equal(failed.errorCode, 'ENOENT');
    else assert(failed.frames.some(f => /diagnostic_fault_origin/.test(f.function || '')));
    await page.screenshot({ path: path.join(out, fault + '.png') });
    await app.close();app = null;
    assert((await records(home)).some(r => r.name === failed.name));
    receipts.push({ fault, home, failed, recordRetainedAfterQuit: true });
  }
  {
    const home = await homeFor('blocked-diagnostics');
    await fs.writeFile(path.join(home, 'diagnostics'), 'leave this obstruction alone');
    const env = { BACKPLANE_COPY: '1', BACKPLANE_AGENTS: path.join(home, 'agents'),
      BACKPLANE_NO_UPDATE: '1', BACKPLANE_TELEMETRY: '0' };
    const old = Object.fromEntries(Object.keys(env).map(key => [key, process.env[key]]));
    const manager = new Service({ home, binary, web });
    try {
      Object.assign(process.env, env);
      const connection = await manager.start(), child = manager.child;
      assert(child?.pid, 'the launcher captured its native child');
      child.ref(); // A pending Promise alone cannot keep this test process alive.
      assert.equal((await fetch(connection.origin + '/hello')).status, 200);
      assert.equal(await fs.readFile(path.join(home, 'diagnostics'), 'utf8'), 'leave this obstruction alone');
      const done = new Promise(resolve => child.once('exit', (code, signal) => resolve({ code, signal })));
      manager.stop();assert.deepEqual(await done, { code: null, signal: 'SIGTERM' });
      receipts.push({ fault: 'blocked-diagnostics', home, serviceStarted: true, exitUnchanged: true });
    } finally {
      if (manager.child) manager.stop();
      for (const [key, value] of Object.entries(old)) {
        if (value === undefined) delete process.env[key];else process.env[key] = value;
      }
    }
  }
  for (const [file, digest] of Object.entries(sourceHashes)) assert.equal(await hash(path.join(root, file)), digest, 'source changed during test: ' + file);
  await fs.writeFile(path.join(out, 'receipt.json'), JSON.stringify({ status: 'passed', binary, binaryHash, web, sourceHashes, receipts }, null, 2));
  console.log('Hub diagnostics practical checks passed: ' + out);
}
main().catch(async error => {
  console.error(error);process.exitCode = 1;
  if (app) {
    const page = await app.firstWindow().catch(() => null);
    if (page) await page.screenshot({ path: path.join(out, 'failure.png') }).catch(() => {});
  }
}).finally(async () => {
  if (app) await app.close().catch(() => {});
  for (const pid of owned) { try { process.kill(pid, 'SIGTERM'); } catch {} }
});
