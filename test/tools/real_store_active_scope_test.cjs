// No hub/store fixture. Same LinuxScope used by future actual-hub cleanup.
const fs = require('node:fs'), cp = require('node:child_process'), { join, resolve } = require('node:path');
const { tmpdir } = require('node:os'), { randomBytes } = require('node:crypto');
const assert = require('node:assert/strict'), { ScopeProof, LinuxScope } = require('./real_store_active_scope.cjs');
const sleep = ms => new Promise(r => setTimeout(r, ms));
module.exports.run = async function run() {
  let cases = 0, taskReads = 0;
  const pass = (v, label) => { assert(v, label); cases++; };
  const owner = { invocation: 'a'.repeat(32), inode: '1', device: '1' };
  let p = new ScopeProof(); pass(p.verdict().safeToRemove, 'unsubmitted preparation can be removed');
  p.submitted(); pass(!p.verdict().safeToRemove, 'submitted but unverified cannot be removed');
  p.bind(owner); pass(!p.verdict().safeToRemove, 'leader exit alone never proves emptiness');
  p.observedEmpty(); pass(p.verdict().safeToRemove, 'verified whole-scope empty is required');
  for (const key of ['invocation', 'inode', 'device']) {
    p = new ScopeProof(); p.submitted(); p.bind(owner); p.same({ ...owner, [key]: 'other' }); p.observedEmpty();
    pass(!p.verdict().safeToRemove && p.verdict().uncertainty, 'replacement identity cannot authorize cleanup');
  }
  p = new ScopeProof(); p.submitted(); p.bind(owner); p.unknown(); p.observedEmpty();
  pass(!p.verdict().safeToRemove, 'later emptiness cannot clear prior uncertainty');
  const unknown = new LinuxScope('bp-real-active-012345abcdef-' + process.pid + '.scope');
  unknown.proof.submitted(); let consulted = false;
  unknown.populated = () => { consulted = true; throw Error('must not inspect or signal unknown owner'); };
  const refusal = await unknown.stop();
  pass(!consulted && !refusal.safeToRemove && refusal.uncertainty && !refusal.kernelKillIssued, 'unknown owner never signals or removes home');
  const replaced = new LinuxScope('bp-real-active-abcdef012345-' + process.pid + '.scope');
  replaced.proof.submitted(); replaced.proof.bind(owner); replaced.group = '/user.slice/replaced';
  replaced.manager = () => ({ LoadState: 'loaded', ControlGroup: replaced.group, InvocationID: 'b'.repeat(32) });
  replaced.statIdentity = invocation => ({ ...owner, invocation });
  let resourceRead = false; replaced.read = () => { resourceRead = true; throw Error('must not touch changed scope'); };
  const changed = await replaced.stop();
  pass(!changed.safeToRemove && changed.uncertainty && !changed.kernelKillIssued && !resourceRead,
    'changed live invocation is refused before resources or signal');
  pass(!replaced.resourcesHealthy(), 'missing resource observations never become zero or PASS');
  cases += await require('./real_store_active_scope_mock_test.cjs').run();
  // Pin executable identity without inheriting provider/auth environment.
  const find = cp.spawnSync('which', ['node'], { encoding: 'utf8', env: { PATH: process.env.PATH }, timeout: 2000 });
  assert.equal(find.status, 0); const node = fs.realpathSync(find.stdout.trim());
  const fixture = resolve(__dirname, 'real_store_active_scope_fixture.cjs'), summaries = [];
  for (const mode of ['orphan', 'race']) {
    const root = fs.mkdtempSync(join(tmpdir(), 'bp-real-active-scope-control-')); fs.chmodSync(root, 0o700);
    const scope = new LinuxScope('bp-real-active-' + randomBytes(6).toString('hex') + '-' + process.pid + '.scope');
    let launcher, stopped, waited = false, failure;
    const read = fs.readFileSync;
    try {
      const argv = scope.command(['/usr/bin/env', '-i', 'PATH=/usr/bin:/bin', 'HOME=' + root, node, fixture, root, mode], join(root, 'scope-admitted'));
      launcher = Bun.spawn(argv, { env: { PATH: '/usr/bin:/bin', XDG_RUNTIME_DIR: process.env.XDG_RUNTIME_DIR ?? '',
        DBUS_SESSION_BUS_ADDRESS: process.env.DBUS_SESSION_BUS_ADDRESS ?? '' }, stdin: 'ignore', stdout: 'ignore', stderr: 'ignore' });
      const leader = await scope.attach(pid => {
        const argv = read('/proc/' + pid + '/cmdline', 'utf8').split('\0').filter(Boolean);
        return fs.readlinkSync('/proc/' + pid + '/exe') === node && argv.includes(fixture) && argv.at(-1) === mode;
      });
      pass(scope.gateReleased && scope.resourceObservations[0]?.stage === 'admission', 'payload admitted after actual kernel cap verification');
      const until = async check => {
        const end = performance.now() + 5000;
        while (!check()) { assert(performance.now() < end, 'synthetic process-tree deadline'); await sleep(5); }
      };
      await until(() => fs.existsSync(join(root, 'worker-child')));
      const child = Number(read(join(root, 'worker-child'), 'utf8'));
      await until(() => fs.existsSync(join(root, 'leaf-' + child)));
      const mainChildren = read(`/proc/${leader}/task/${leader}/children`, 'utf8').trim().split(/\s+/).map(Number);
      pass(!mainChildren.includes(child) && scope.contains(child), 'worker task child omitted by main-task list remains scope owned');
      // The cleanup path must remain correct even when task enumeration is
      // missing. Every task-file access is deliberately refused in this control.
      fs.readFileSync = function (path, ...args) {
        if (String(path).includes('/task/')) { taskReads++; const e = Error('missing synthetic task'); e.code = 'ENOENT'; throw e; }
        return read.call(fs, path, ...args);
      };
      fs.writeFileSync(join(root, 'switch'), '1');
      await until(() => fs.existsSync(join(root, 'late-child')));
      if (mode === 'orphan') {
        await until(() => !scope.leaderAlive());
        pass(scope.populated() === 1, 'parent exit does not hide late orphan');
      } else pass(scope.leaderAlive(), 'stop races with a live worker still spawning');
      stopped = await scope.stop();
      pass(stopped.safeToRemove && stopped.allDescendantsEmpty && stopped.kernelKillIssued && !stopped.uncertainty,
        'kernel scope kill proves complete emptiness across late spawns');
      pass(scope.resourcesHealthy(), 'actual synthetic scope caps and pre-kill observations contain no OOM/swap');
      pass(scope.populated() === 0, 'post-stop entire subtree remains empty');
      const code = await Promise.race([launcher.exited, sleep(3000).then(() => 'timeout')]);
      pass(code !== 'timeout', 'owned launcher terminates'); waited = code !== 'timeout';
      summaries.push({ mode, allDescendantsEmpty: stopped.allDescendantsEmpty, kernelKillIssued: stopped.kernelKillIssued,
        uncertainty: stopped.uncertainty, failureCode: stopped.failureCode, launcherExited: waited, resourceObservations: scope.resourceObservations,
        resourceObservationMissing: scope.resourceObservationMissing });
    } catch (e) { failure = e; }
    finally {
      fs.readFileSync = read;
      stopped ??= await scope.stop();
      // Never turn an uncertain scope into a pass or remove its home. The
      // enclosing reviewed Boat packet must clean the VM on a failed control.
      if (stopped.safeToRemove && scope.resourcesHealthy() && waited) fs.rmSync(root, { recursive: true, force: true });
      scope.close();
    }
    if (failure) {
      console.error(JSON.stringify({ scope: 'owned-scope-control-failure', mode, cleanup: stopped,
        resourceObservations: scope.resourceObservations, resourceObservationMissing: scope.resourceObservationMissing }));
      throw failure;
    }
    pass(stopped.safeToRemove && !fs.existsSync(root), 'owned synthetic home removed only after positive proof');
  }
  pass(taskReads === 0, 'cleanup never relies on any task children file');
  assert.equal(cases, 49);
  console.log(JSON.stringify({ scope: 'combined-real-store-owned-execution-scope', ok: true, cases,
    hubStarted: false, realFixtureOpened: false, syntheticScopes: 2, missingTaskReads: taskReads, summaries }));
};
