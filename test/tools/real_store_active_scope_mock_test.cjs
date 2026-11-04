// Actual LinuxScope.stop/observeResources branches with mocked kernel/manager
// boundaries only. No process, systemd command, cgroup or fixture is opened.
const fs = require('node:fs'), assert = require('node:assert/strict');
const { LinuxScope } = require('./real_store_active_scope.cjs');
module.exports.run = async function run() {
  let cases = 0;
  const pass = (v, label) => { assert(v, label); cases++; };
  const owner = { invocation: 'a'.repeat(32), inode: '1', device: '1' };
  const healthy = stage => ({ stage, memoryMaxBytes: 3699998720, swapMaxBytes: 0,
    memoryPeakBytes: 4096, swapCurrentBytes: 0, events: { oom: 0, oom_kill: 0, oom_group_kill: 0 } });
  const make = () => {
    const scope = new LinuxScope('bp-real-active-012345abcdef-' + process.pid + '.scope');
    scope.proof.submitted(); scope.proof.bind(owner); scope.invocation = owner.invocation;
    scope.group = '/user.slice/mock-scope'; scope.path = '/mock-owned-cgroup'; scope.fd = 123;
    scope.statIdentity = invocation => ({ ...owner, invocation });
    scope.resourceObservations = [healthy('admission'), healthy('before-kill')]; return scope;
  };
  const active = { LoadState: 'loaded', ActiveState: 'active', ControlGroup: '/user.slice/mock-scope', InvocationID: owner.invocation };
  const saved = { lstatSync: fs.lstatSync, openSync: fs.openSync, writeSync: fs.writeSync, closeSync: fs.closeSync };
  let pathnameReads = 0, removals = 0, kills = 0;
  const consume = verdict => { if (verdict.safeToRemove) removals++; };
  try {
    fs.lstatSync = path => { assert.equal(path, '/mock-owned-cgroup'); pathnameReads++; const e = Error('mock missing'); e.code = 'ENOENT'; throw e; };
    fs.openSync = path => { assert.equal(path, '/proc/self/fd/123/cgroup.kill'); return 777; };
    let killed = false;
    fs.writeSync = (fd, text) => { assert.equal(fd, 777); assert.equal(text, '1'); kills++; killed = true; return 1; };
    fs.closeSync = fd => { assert.equal(fd, 777); };
    // Both direct retirement forms and both cgroup.events missing-file fallback
    // codes must reject a nonempty replacement ID before looking at the path.
    for (const route of ['loaded-retired', 'not-found', 'ENOENT', 'ENODEV']) {
      const scope = make(); let reads = 0, managers = 0; const pathsBefore = pathnameReads;
      const retired = { LoadState: route === 'not-found' ? 'not-found' : 'loaded', ActiveState: 'inactive',
        ControlGroup: '', InvocationID: 'b'.repeat(32) };
      scope.manager = () => { managers++; return route === 'ENOENT' || route === 'ENODEV' ? managers === 1 ? active : retired : retired; };
      scope.read = name => { assert.equal(name, 'cgroup.events'); reads++; const e = Error('mock missing'); e.code = route; throw e; };
      let result = await scope.stop(); consume(result);
      pass(!result.safeToRemove && result.ownershipUncertainty && result.uncertainty && !result.kernelKillIssued &&
        !result.allDescendantsEmpty && pathnameReads === pathsBefore && removals === 0 && kills === 0 &&
        reads === (route === 'ENOENT' || route === 'ENODEV' ? 1 : 0), 'retired replacement cannot authorize deletion');
      // Even a later valid, empty retirement cannot erase the observed mismatch.
      scope.manager = () => ({ ...retired, InvocationID: owner.invocation });
      scope.read = () => 'populated 0'; result = await scope.stop(); consume(result);
      pass(!result.safeToRemove && result.ownershipUncertainty && result.failureCode === 'scope-owner-unknown' && removals === 0,
        'prior retired replacement uncertainty remains sticky');
    }
    // Valid retirement still works with the original ID or a genuinely erased
    // ID, and with the valid-ID missing-events fallback.
    for (const route of ['same-id', 'erased-id', 'same-id-fallback']) {
      const scope = make(); let managers = 0;
      const retired = { LoadState: 'loaded', ActiveState: 'inactive', ControlGroup: '',
        InvocationID: route === 'erased-id' ? '' : owner.invocation };
      scope.manager = () => ++managers === 1 && route === 'same-id-fallback' ? active : retired;
      scope.read = () => { const e = Error('mock missing'); e.code = 'ENOENT'; throw e; };
      const result = await scope.stop();
      pass(result.safeToRemove && result.allDescendantsEmpty && !result.uncertainty && !result.kernelKillIssued,
        'verified original or erased retirement identity remains supported');
    }
    for (const fault of ['ENOENT', 'EACCES', 'malformed']) {
      const scope = make(); scope.resourceObservations = []; killed = false; let failResource = false;
      scope.manager = () => active;
      scope.read = name => {
        if (name === 'cgroup.events') return 'populated ' + (killed ? '0' : '1');
        if (name === 'memory.peak' && failResource) {
          if (fault === 'malformed') return 'not-a-number';
          const e = Error('mock resource unavailable'); e.code = fault; throw e;
        }
        return { 'memory.events': 'oom 0\noom_kill 0\noom_group_kill 0', 'memory.max': '3699998720',
          'memory.swap.max': '0', 'memory.peak': '4096', 'memory.swap.current': '0' }[name];
      };
      scope.observeResources('admission'); failResource = true;
      let result = await scope.stop(); consume(result);
      pass(result.kernelKillIssued && result.allDescendantsEmpty && !result.ownershipUncertainty && result.resourceUncertainty &&
        result.uncertainty && !result.safeToRemove && removals === 0,
        'verified kill and empty subtree cannot authorize deletion with missing resource evidence');
      pass(scope.resourceObservationMissing && !scope.resourcesHealthy() && result.failureCode ===
        (fault === 'malformed' ? 'scope-resource-sample' : 'scope-' + fault.toLowerCase()), 'resource fault remains explicit');
      failResource = false; scope.observeResources('before-kill'); result = await scope.stop(); consume(result);
      pass(scope.resourceObservations.length === 2 && result.allDescendantsEmpty && result.resourceUncertainty &&
        !scope.resourcesHealthy() && !result.safeToRemove && removals === 0, 'later good reads and emptiness cannot clear resource uncertainty');
    }
    const missing = make(); missing.resourceObservations = []; missing.manager = () => active;
    missing.read = name => { assert.equal(name, 'cgroup.events'); return 'populated 0'; };
    const result = await missing.stop(); consume(result);
    pass(!result.safeToRemove && result.resourceUncertainty && result.allDescendantsEmpty && removals === 0,
      'empty scope with absent observations never authorizes deletion');
  } finally { Object.assign(fs, saved); }
  assert.equal(cases, 21); return cases;
};
