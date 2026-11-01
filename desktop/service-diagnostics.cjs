'use strict';
// One pair of local files per launch. No output pipes: the resident service
// must keep running after Electron quits. Error text and argv are not retained.
const fs = require('node:fs/promises');
const path = require('node:path');
const { randomUUID, createHash } = require('node:crypto');
const KEEP = 8;
const REPORT = /^hub-\d{13}-[a-f0-9-]{36}(?:\.exception)?\.json$/;
const SIGNAL = new Set(Object.keys(require('node:os').constants.signals));

async function serviceDiagnostics({ home, binary, version, files = fs }) {
  try {
    const dir = path.join(home, 'diagnostics');
    await files.mkdir(dir, { recursive: true, mode: 0o700 });
    const stat = await files.lstat(dir);
    if (!stat.isDirectory() || stat.isSymbolicLink() ||
        (process.getuid && stat.uid !== process.getuid())) return null;
    await files.chmod(dir, 0o700);
    const base = `hub-${Date.now()}-${randomUUID()}`;
    const file = path.join(dir, base + '.json');
    const exception = path.join(dir, base + '.exception.json');
    const names = (await files.readdir(dir)).filter(name => REPORT.test(name));
    const runs = [...new Set(names.map(name => name.replace(/(?:\.exception)?\.json$/, ''))) ].sort().reverse();
    const keep = new Set(runs.slice(0, KEEP - 1));
    for (const name of names) {
      if (!keep.has(name.replace(/(?:\.exception)?\.json$/, ''))) await files.unlink(path.join(dir, name));
    }
    const record = { schema: 1, startedAt: new Date().toISOString(), observerPid: process.pid, state: 'starting' };
    if (typeof version === 'string' && /^[\w.+-]{1,128}$/.test(version)) record.version = version;
    // Hash in chunks rather than loading another copy of the service into RAM.
    try {
      if (!(await files.stat(binary)).isFile()) throw new Error('Not a regular executable');
      const input = await files.open(binary, 'r');
      try {
        const hash = createHash('sha256'), chunk = Buffer.alloc(1024 * 1024);
        for (;;) {
          const { bytesRead } = await input.read(chunk, 0, chunk.length, null);
          if (!bytesRead) break;
          hash.update(chunk.subarray(0, bytesRead));
        }
        record.binarySha256 = hash.digest('hex');
      } finally { await input.close(); }
    } catch { /* An unreadable executable still deserves a launch-failure record. */ }
    let pending = Promise.resolve();
    function save(update) {
      pending = pending.then(async () => {
        Object.assign(record, update);
        const temp = file + '.' + randomUUID() + '.tmp';
        try {
          await files.writeFile(temp, JSON.stringify(record) + '\n', { flag: 'wx', mode: 0o600 });
          await files.rename(temp, file);
        } finally { await files.unlink(temp).catch(() => {}); }
      }).catch(() => {});
      return pending;
    }
    await save({});
    return {
      env: { BACKPLANE_DIAGNOSTICS_FILE: exception },
      started(pid) {
        return save({ state: 'running', ...(Number.isSafeInteger(pid) && pid > 0 ? { pid } : {}) });
      },
      ready(actualVersion) {
        return save(typeof actualVersion === 'string' && /^[\w.+-]{1,128}$/.test(actualVersion)
          ? { version: actualVersion } : {});
      },
      exited(code, signal) {
        return save({ state: 'exited', endedAt: new Date().toISOString(),
          exitCode: Number.isSafeInteger(code) ? code : null,
          signal: typeof signal === 'string' && SIGNAL.has(signal) ? signal : null });
      },
      failed(code) {
        return save({ state: 'launch-failed', endedAt: new Date().toISOString(),
          ...(typeof code === 'string' && /^E[A-Z0-9_]{1,31}$/.test(code) ? { errorCode: code } : {}) });
      },
    };
  } catch { return null; }
}

module.exports = { serviceDiagnostics, KEEP };
