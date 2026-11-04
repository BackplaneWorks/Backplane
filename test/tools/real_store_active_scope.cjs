// Owned test-harness execution scope. No IO runs merely by importing this file.
// Kernel cgroup v2: cgroup.kill covers concurrent forks/migrations; populated
// includes every descendant, independent of which task spawned it.
// https://docs.kernel.org/admin-guide/cgroup-v2.html#core-interface-files
const fs = require('node:fs'), cp = require('node:child_process');
const check = (v, code) => { if (!v) throw Error(code); };
const sleep = ms => new Promise(r => setTimeout(r, ms));
class ScopeProof {
  constructor() { this.launched = false; this.verified = false; this.uncertain = false; this.empty = false; this.killed = false; }
  submitted() { check(!this.launched, 'scope-double-start'); this.launched = true; }
  bind(identity) {
    check(this.launched && !this.verified && identity?.invocation && identity?.inode && identity?.device, 'scope-identity');
    this.identity = { ...identity }; this.verified = true;
  }
  same(identity) {
    const same = this.verified && ['invocation', 'inode', 'device'].every(k => identity?.[k] === this.identity[k]);
    if (!same) this.uncertain = true; return same && !this.uncertain;
  }
  unknown() { this.uncertain = true; }
  killedAll() { check(this.verified && !this.uncertain, 'scope-owner-unknown'); this.killed = true; }
  observedEmpty() { check(this.verified, 'scope-owner-unknown'); this.empty = true; }
  verdict() { return { safeToRemove: !this.launched || this.verified && this.empty && !this.uncertain,
    submitted: this.launched, identityVerified: this.verified, kernelKillIssued: this.killed,
    allDescendantsEmpty: this.empty, uncertainty: this.uncertain }; }
}
class LinuxScope {
  constructor(unit) {
    check(/^bp-real-active-[a-f0-9]{12}-[0-9]+\.scope$/.test(unit), 'scope-name');
    this.unit = unit; this.proof = new ScopeProof(); this.fd = null; this.leaderPid = 0; this.birth = 0;
    this.resourceObservations = []; this.resourceObservationMissing = false; this.failureCode = null;
  }
  command(argv, gate) {
    check(typeof gate === 'string' && gate.startsWith('/') && !fs.existsSync(gate), 'scope-gate');
    this.gate = gate; this.gateReleased = false;
    this.proof.submitted();
    return ['systemd-run', '--user', '--scope', '--quiet', '--unit=' + this.unit,
      '-p', 'MemoryMax=3700000000', '-p', 'MemorySwapMax=0', '--',
      '/bin/sh', '-c', 'while [ ! -f "$1" ]; do sleep 0.01; done; shift; exec "$@"',
      'owned-scope-admission', gate, ...argv];
  }
  manager() {
    const p = cp.spawnSync('systemctl', ['--user', 'show', this.unit,
      '--property=LoadState,ActiveState,ControlGroup,InvocationID'], { encoding: 'utf8', timeout: 2000, maxBuffer: 16384 });
    check(p.status === 0 || p.status === 1, 'scope-manager');
    const rows = Object.fromEntries((p.stdout ?? '').trim().split('\n').map(x => { const i = x.indexOf('='); return [x.slice(0, i), x.slice(i + 1)]; }));
    check(['loaded', 'not-found'].includes(rows.LoadState), 'scope-manager'); return rows;
  }
  statIdentity(invocation) {
    const s = fs.fstatSync(this.fd, { bigint: true }); return { invocation, inode: String(s.ino), device: String(s.dev) };
  }
  read(name) {
    check(['cgroup.events', 'cgroup.procs', 'cgroup.type', 'memory.max', 'memory.swap.max', 'memory.peak',
      'memory.swap.current', 'memory.events'].includes(name), 'scope-file');
    return fs.readFileSync('/proc/self/fd/' + this.fd + '/' + name, 'utf8').trim();
  }
  async attach(matches, timeoutMs = 5000) {
    const deadline = performance.now() + timeoutMs;
    try {
      while (performance.now() < deadline) {
        const m = this.manager();
        if (m.LoadState === 'loaded' && m.ControlGroup) {
          check(/^\/user\.slice\//.test(m.ControlGroup) && m.ControlGroup.endsWith('/' + this.unit) && /^[a-f0-9]{32}$/.test(m.InvocationID), 'scope-identity');
          if (this.fd === null) {
            this.path = '/sys/fs/cgroup' + m.ControlGroup; this.group = m.ControlGroup; this.invocation = m.InvocationID;
            this.fd = fs.openSync(this.path, fs.constants.O_RDONLY | fs.constants.O_DIRECTORY | fs.constants.O_NOFOLLOW);
            check(this.read('cgroup.type') === 'domain', 'scope-type');
            check(Number(this.read('memory.max')) > 0 && Number(this.read('memory.max')) <= 3700000000 && this.read('memory.swap.max') === '0', 'scope-cap');
            this.proof.bind(this.statIdentity(m.InvocationID));
            this.observeResources('admission');
            check(!this.resourceObservationMissing, 'scope-resource-missing');
            // Admit the actual payload only after reading its kernel cap and
            // swap limit. The controller remains outside the kill scope.
            fs.writeFileSync(this.gate, '1', { flag: 'wx', mode: 0o600 }); this.gateReleased = true;
          } else check(this.proof.same(this.statIdentity(m.InvocationID)) && this.group === m.ControlGroup, 'scope-identity');
          const pids = this.read('cgroup.procs').split(/\s+/).filter(Boolean).map(Number); check(pids.length <= 512, 'scope-pid-bound');
          const leaders = pids.filter(pid => { try { return matches(pid); } catch (e) { if (e.code === 'ENOENT' || e.code === 'ESRCH') return false; throw e; } });
          check(leaders.length <= 1, 'scope-multiple-leaders');
          if (leaders.length === 1) {
            this.leaderPid = leaders[0]; this.birth = this.procBirth(this.leaderPid); return this.leaderPid;
          }
        }
        await sleep(10);
      }
      throw Error('scope-admission-deadline');
    } catch (e) { this.recordFailure(e); this.proof.unknown(); throw e; }
  }
  recordFailure(e) {
    const known = new Set(['scope-identity', 'scope-owner-unknown', 'scope-cap', 'scope-type', 'scope-manager',
      'scope-file', 'scope-resource-missing', 'scope-resource-sample', 'scope-resource-bound', 'scope-pid-bound',
      'scope-multiple-leaders', 'scope-admission-deadline', 'scope-process-sample', 'scope-retirement-uncertain',
      'scope-populated-missing', 'scope-kill-write', 'scope-stop-deadline']);
    this.failureCode ??= ['EACCES', 'EPERM', 'ENOENT', 'ENODEV', 'ETIMEDOUT'].includes(e?.code) ?
      'scope-' + e.code.toLowerCase() : known.has(e?.message) ? e.message : 'scope-operation-failed';
  }
  procBirth(pid) {
    const s = fs.readFileSync('/proc/' + pid + '/stat', 'utf8'), fields = s.slice(s.lastIndexOf(') ') + 2).trim().split(/\s+/);
    const n = Number(fields[19]); check(Number.isSafeInteger(n) && n > 0, 'scope-process-sample'); return n;
  }
  contains(pid) {
    const group = fs.readFileSync('/proc/' + pid + '/cgroup', 'utf8').split('0::')[1]?.trim();
    return group === this.group || group?.startsWith(this.group + '/') === true;
  }
  leaderAlive() {
    if (!this.leaderPid) return false;
    try {
      const text = fs.readFileSync('/proc/' + this.leaderPid + '/stat', 'utf8');
      const fields = text.slice(text.lastIndexOf(') ') + 2).trim().split(/\s+/);
      if (Number(fields[19]) !== this.birth || fields[0] === 'Z' || fields[0] === 'X') return false;
      if (!this.contains(this.leaderPid)) { this.proof.unknown(); return false; } return true; }
    catch (e) { if (e.code === 'ENOENT' || e.code === 'ESRCH') return false; this.proof.unknown(); throw e; }
  }
  observeResources(stage) {
    try {
      const events = Object.fromEntries(this.read('memory.events').split('\n').map(row => {
        const [k, v] = row.split(/\s+/); check(/^\d+$/.test(v), 'scope-resource-sample'); return [k, Number(v)];
      }));
      const row = { stage, memoryMaxBytes: Number(this.read('memory.max')), swapMaxBytes: Number(this.read('memory.swap.max')),
        memoryPeakBytes: Number(this.read('memory.peak')), swapCurrentBytes: Number(this.read('memory.swap.current')), events };
      check(row.memoryMaxBytes > 0 && row.memoryMaxBytes <= 3700000000 && row.memoryPeakBytes > 0 &&
        Number.isSafeInteger(row.memoryPeakBytes) && row.swapMaxBytes === 0 && Number.isSafeInteger(row.swapCurrentBytes), 'scope-resource-sample');
      check(this.resourceObservations.length < 3, 'scope-resource-bound'); this.resourceObservations.push(row);
    } catch (e) { this.resourceObservationMissing = true; this.recordFailure(e); }
  }
  resourcesHealthy() {
    return !this.resourceObservationMissing && this.resourceObservations.length === 2 && this.resourceObservations.every(r =>
      r.swapCurrentBytes === 0 && ['oom', 'oom_kill', 'oom_group_kill'].every(k => r.events[k] === 0));
  }
  retired(m) {
    if (!this.proof.verified || !(m.LoadState === 'not-found' || !m.ControlGroup) || m.ActiveState !== 'inactive') return false;
    // existsSync also returns false for access errors. Only a positively
    // observed missing old pathname, plus manager retirement, proves this case.
    try { fs.lstatSync(this.path); return false; }
    catch (e) { if (e.code === 'ENOENT') return true; throw e; }
  }
  populated() {
    const m = this.manager();
    if (m.LoadState === 'not-found' || !m.ControlGroup) {
      // A cgroup directory can retire only after its complete subtree empties.
      check(this.retired(m), 'scope-retirement-uncertain');
      return 0;
    }
    check(m.ControlGroup === this.group && this.proof.same(this.statIdentity(m.InvocationID)), 'scope-owner-unknown');
    let text;
    try { text = this.read('cgroup.events'); } catch (e) {
      if (['ENOENT', 'ENODEV'].includes(e.code)) {
        const retired = this.manager();
        if (this.retired(retired)) return 0;
      }
      throw e;
    }
    const entries = Object.fromEntries(text.split('\n').map(x => x.split(/\s+/)));
    check(entries.populated === '0' || entries.populated === '1', 'scope-populated-missing'); return Number(entries.populated);
  }
  async stop(timeoutMs = 3000) {
    if (!this.proof.launched) return { ...this.proof.verdict(), failureCode: this.failureCode };
    try {
      check(this.proof.verified && !this.proof.uncertain, 'scope-owner-unknown');
      if (this.populated() !== 0) {
        // Captured while the scope still exists. Never invent final zeroes
        // after systemd retires an empty group; these are pre-kill observations.
        this.observeResources('before-kill');
        // Resolve through the opened, identity-checked cgroup directory, never
        // a recycled PID or a replacement directory at the same pathname.
        check(this.proof.same(this.statIdentity(this.invocation)), 'scope-owner-unknown');
        let fd;
        try { fd = fs.openSync('/proc/self/fd/' + this.fd + '/cgroup.kill', fs.constants.O_WRONLY | fs.constants.O_NOFOLLOW); }
        catch (e) { if (!['ENOENT', 'ENODEV'].includes(e.code) || this.populated() !== 0) throw e; }
        if (fd !== undefined) try { check(fs.writeSync(fd, '1') === 1, 'scope-kill-write'); this.proof.killedAll(); } finally { fs.closeSync(fd); }
      }
      const deadline = performance.now() + timeoutMs;
      while (this.populated() !== 0 && performance.now() < deadline) await sleep(10);
      check(this.populated() === 0, 'scope-stop-deadline'); this.proof.observedEmpty();
    } catch (e) { this.recordFailure(e); this.proof.unknown(); }
    return { ...this.proof.verdict(), failureCode: this.failureCode };
  }
  close() { if (this.fd !== null) { fs.closeSync(this.fd); this.fd = null; } }
}
module.exports = { ScopeProof, LinuxScope };
