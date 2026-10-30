// Sample file writes and owned output growth independently of a blocked tool.
// Metadata work is bounded per sample; directory reads are incremental, and
// a rotating plan cursor gives threads beyond the first global budget a turn.
const bp_progress = new Map();
let bp_progress_cursor = 0;
const bp_progress_files_limit = 32768;
function progress_sample(plan, home) {
  const fs = require('node:fs'), path = require('node:path');
  let rows;
  try { rows = JSON.parse(plan); } catch { return '[]'; }
  if (!Array.isArray(rows)) return '[]';
  const now = Date.now(), changed = [], keep = new Set();
  const ignored = new Set(['.git', '.backplane', 'node_modules', '__pycache__']);
  const within = (root, file) => file === root || file.startsWith(root + path.sep);
  const hub = path.resolve(home);
  const real = p => { try { return fs.realpathSync(p); } catch { return p; } };
  const realHub = real(hub);
  rows = rows.filter(row => row && typeof row.thread === 'string' && typeof row.cwd === 'string' && path.isAbsolute(row.cwd));
  for (const row of rows) keep.add(row.thread);
  const close = state => {
    for (const dir of state.stack) try { dir.handle.closeSync(); } catch {}
    state.stack = [];
  };
  for (const [id, state] of bp_progress) if (!keep.has(id)) { close(state); bp_progress.delete(id); }
  let budget = 4096, visited = 0;
  const start = rows.length ? bp_progress_cursor % rows.length : 0;
  while (visited < rows.length && budget > 0) {
    const row = rows[(start + visited++) % rows.length], id = row.thread, cwd = path.resolve(row.cwd);
    let state = bp_progress.get(id);
    if (!state || state.cwd !== cwd) {
      if (state) close(state);
      // A restarted hub can observe writes newer than its durable progress
      // watermark. Without one, existing files establish a quiet baseline.
      const since = Number.isSafeInteger(row.since) && row.since > 0 && row.since <= now ? row.since : now;
      state = { cwd, realCwd: real(cwd), since, files: new Map(), stack: [], started: false,
        overflow: [0, 0], previousOverflow: null, output: new Map(), workIndex: 0 };
      bp_progress.set(id, state);
    }
    let at = 0, reason = '', share = Math.min(256, budget);
    const signal = why => { at = Math.floor(now / 1000); reason = why; };
    const allowed = file => {
      const actual = real(file);
      return !within(hub, file) && !within(realHub, actual) && within(state.realCwd, actual);
    };
    const observe = file => {
      let st;
      try { st = fs.lstatSync(file); } catch { return; }
      if (!st.isFile() || !allowed(file)) return;
      // ctime and inode also catch same-size edits and atomic replacement with
      // a deliberately preserved mtime. atime never participates.
      const fingerprint = `${st.dev}:${st.ino}:${st.mtimeMs}:${st.ctimeMs}:${st.size}`;
      const previous = state.files.get(file);
      if (previous !== undefined || state.files.size < bp_progress_files_limit) {
        state.files.set(file, fingerprint);
        if ((previous !== undefined && previous !== fingerprint) || (previous === undefined && Math.max(st.mtimeMs, st.ctimeMs) > state.since))
          signal('file write: ' + path.relative(cwd, file));
      } else {
        // Keep fingerprints bounded. Overflow files contribute to a constant
        // size scan digest; a completed changed scan is evidence of progress,
        // while rescanning an unchanged large tree never is.
        const hash = require('node:crypto').createHash('sha256').update(file).update('\0').update(fingerprint).digest();
        state.overflow[0] ^= hash.readUInt32LE(0);
        state.overflow[1] ^= hash.readUInt32LE(4);
      }
    };
    const work = typeof row.work === 'string' ? row.work.split('\n').filter(Boolean) : [];
    const activeOutput = new Set();
    for (const line of work) {
      const workId = line.split('\t', 1)[0];
      if (/^w-[A-Za-z0-9_-]+$/.test(workId)) activeOutput.add(path.join(hub, 'work', workId, 'out'));
    }
    for (const file of state.output.keys()) if (!activeOutput.has(file)) state.output.delete(file);
    const outputs = Array.from(activeOutput), workShare = Math.min(64, share, outputs.length);
    for (let i = 0; i < workShare; i++) {
      const file = outputs[(state.workIndex + i) % outputs.length];
      share--; budget--;
      let st;
      try { st = fs.lstatSync(file); } catch { continue; }
      if (!st.isFile() || !within(realHub, real(file))) continue;
      const previous = state.output.get(file), identity = `${st.dev}:${st.ino}`;
      state.output.set(file, { size: st.size, identity });
      if ((previous !== undefined && st.size > (previous.identity === identity ? previous.size : 0)) ||
          (previous === undefined && st.size > 0 && Math.max(st.mtimeMs, st.ctimeMs) > state.since))
        signal('work output grew: ' + path.basename(path.dirname(file)));
    }
    if (outputs.length) state.workIndex = (state.workIndex + workShare) % outputs.length;
    while (share > 0 && budget > 0) {
      share--; budget--;
      if (!state.started) {
        state.started = true;
        if (!within(hub, cwd) && !within(realHub, state.realCwd)) {
          try { state.stack.push({ file: cwd, handle: fs.opendirSync(cwd) }); } catch {}
        }
      }
      const dir = state.stack[state.stack.length - 1];
      if (!dir) {
        if (state.previousOverflow && (state.previousOverflow[0] !== state.overflow[0] || state.previousOverflow[1] !== state.overflow[1]))
          signal('file writes in large working tree');
        state.previousOverflow = state.overflow;
        state.overflow = [0, 0]; state.started = false;
        break;
      }
      let entry;
      try { entry = dir.handle.readSync(); } catch { entry = null; }
      if (!entry) { try { dir.handle.closeSync(); } catch {} state.stack.pop(); continue; }
      const file = path.join(dir.file, entry.name);
      if (entry.isSymbolicLink() || ignored.has(entry.name) || entry.name.startsWith('.bend-check') || entry.name.endsWith('.backplane.json') || within(hub, file)) continue;
      if (entry.isDirectory()) {
        if (state.stack.length < 64 && allowed(file)) {
          try { state.stack.push({ file, handle: fs.opendirSync(file) }); } catch {}
        }
      } else observe(file);
    }
    if (at) changed.push({ thread: id, at, reason });
  }
  bp_progress_cursor = rows.length ? (start + visited) % rows.length : 0;
  return JSON.stringify(changed);
}
io_eff(CID(Progress.sample), progress_sample);
