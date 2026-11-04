// IO only. Policy, labels and actions live in core/cad-install.bend.
// One bounded streaming download and one captured child tree per tool.
const bp_cad_jobs = new Map();
const bp_cad_terminal = new Set(['ready', 'failed', 'cancelled', 'unavailable']);
const BP_CAD_STATE_BYTES = 4096;
const BP_CAD_FRAME_BYTES = 16384;
function bp_cad_key(home, tool) { return home + '\n' + tool; }
// Clamp at the one state-construction boundary, including catalog, parse,
// timeout, cleanup and effect-wrapper errors. Count JSON's escaped UTF-8,
// not JS string length: one control character occupies six wire bytes.
function bp_cad_error(value) {
  let text = '';
  let readable;
  try { readable = String(value ?? ''); } catch (_) { readable = 'Installation failed without readable error details.'; }
  for (const char of readable.slice(-2048)) {
    const scalar = char.codePointAt(0);
    text += scalar >= 0xd800 && scalar <= 0xdfff ? '\ufffd' : char;
  }
  const scalars = Array.from(text);
  while (Buffer.byteLength(JSON.stringify(scalars.join('')), 'utf8') > 2048) scalars.shift();
  text = scalars.join('');
  return text;
}
function bp_cad_number(value) {
  return Number.isSafeInteger(value) ? Math.max(0, Math.min(8 * 1024 ** 3, value)) : 0;
}
function bp_cad_wire(state) {
  const text = JSON.stringify(state);
  if (Buffer.byteLength(text, 'utf8') > BP_CAD_STATE_BYTES) throw new Error('CAD state exceeds its serialized UTF-8 ceiling');
  return text;
}
function bp_cad_progress(job, phase, fields = {}) {
  const reading = bp_cad_number(fields.done);
  const done = phase === job.state.phase ? Math.max(job.state.done, reading) : reading;
  job.state = { attempt: job.state.attempt, phase, done, total: bp_cad_number(fields.total), speed: bp_cad_number(fields.speed), eta: bp_cad_number(fields.eta), known: fields.known === true, error: bp_cad_error(fields.error) };
  bp_cad_wire(job.state);
}
function bp_cad_check(job) {
  if (job.controller.signal.aborted && !job.committed) throw new Error(job.reason || 'Cancelled');
}
async function bp_cad_download(job, url, file, digest) {
  const fs = require('node:fs'), crypto = require('node:crypto');
  let idle;
  const reset = () => { clearTimeout(idle); idle = setTimeout(() => { job.reason = 'Download stalled for 30 seconds. Check your connection and retry.'; job.controller.abort(); }, 30000); };
  reset();
  let fd, reader;
  try {
    const response = await fetch(url, { signal: job.controller.signal, headers: { 'Accept-Encoding': 'identity' } });
    if (!response.ok) throw new Error('Download returned HTTP ' + response.status);
    if (!response.body) throw new Error('Download returned an empty response');
    const length = response.headers.get('content-length');
    const total = /^\d+$/.test(length || '') ? Number(length) : 0;
    if (!Number.isSafeInteger(total) || total > 8 * 1024 ** 3) throw new Error('Invalid download size');
    const hash = crypto.createHash('sha256');
    fd = await fs.promises.open(file, 'wx', 0o600);
    reader = response.body.getReader();
    let done = 0; const start = performance.now();
    bp_cad_progress(job, 'download', { total });
    while (true) {
      const chunk = await reader.read();
      bp_cad_check(job);
      if (chunk.done) break;
      reset(); done += chunk.value.length;
      if (done > 8 * 1024 ** 3 || (total && done > total)) throw new Error('Download exceeds advertised size');
      // Chunks are bounded by the response stream, never collect the archive.
      let at = 0;
      while (at < chunk.value.length) at += (await fd.write(chunk.value, at, chunk.value.length - at)).bytesWritten;
      hash.update(chunk.value);
      const elapsed = Math.max(1, performance.now() - start), speed = Math.floor(done * 1000 / elapsed);
      bp_cad_progress(job, 'download', { done, total, speed, known: total > 0 && speed > 0, eta: speed && total ? Math.ceil((total - done) / speed) : 0 });
    }
    if (!done || (total && total !== done)) throw new Error('Download ended before the advertised size');
    bp_cad_progress(job, 'verify');
    if (hash.digest('hex') !== digest) throw new Error('Archive checksum mismatch; retry the download');
  } finally {
    clearTimeout(idle);
    if (fd !== undefined) await fd.close();
    if (reader) { try { await reader.cancel(); } catch (_) {} }
  }
}
function bp_cad_stop(job) {
  const child = job.child;
  if (!child || !child.pid) return;
  // Only the tree created for this job, never processes found by name.
  if (process.platform === 'win32') {
    const root = process.env.SystemRoot || 'C:\\Windows';
    require('node:child_process').spawn(require('node:path').join(root, 'System32', 'taskkill.exe'), ['/PID', String(child.pid), '/T', '/F'], { windowsHide: true, stdio: 'ignore' });
  } else {
    try { process.kill(-child.pid, 'SIGTERM'); } catch (_) {}
    job.killTimer = setTimeout(() => { try { process.kill(-child.pid, 'SIGKILL'); } catch (_) {} }, 2000);
  }
}
async function bp_cad_run(job, program, args, cwd) {
  bp_cad_check(job);
  const cp = require('node:child_process');
  await new Promise((resolve, reject) => {
    const child = cp.spawn(program, args, { cwd, detached: process.platform !== 'win32', windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
    job.child = child;
    let tail = '', lines = '';
    const abort = () => bp_cad_stop(job);
    job.controller.signal.addEventListener('abort', abort, { once: true });
    child.stdout.on('data', b => {
      lines = (lines + b.toString()).slice(-8192);
      let at;
      while ((at = lines.indexOf('\n')) >= 0) {
        const line = lines.slice(0, at).trim(); lines = lines.slice(at + 1);
        if (line === 'BP_COMMIT_BEGIN') job.committing = true;
        else if (line === 'BP_COMMITTED') { job.committed = true; job.cancelled = false; }
        else if (/^BP_INSTALL (verify|extract|configure)$/.test(line)) bp_cad_progress(job, line.slice(11));
        else tail = (tail + line + '\n').slice(-4096);
      }
    });
    child.stderr.on('data', b => { tail = (tail + b.toString()).slice(-4096); });
    child.once('error', reject);
    child.once('close', code => {
      job.child = null;
      job.controller.signal.removeEventListener('abort', abort);
      clearTimeout(job.killTimer);
      if (job.controller.signal.aborted && !job.committed) reject(new Error(job.reason || 'Cancelled'));
      else if (code !== 0) reject(new Error(tail.trim() || program + ' exited ' + code));
      else resolve();
    });
  });
  bp_cad_check(job);
}
async function bp_cad_execute(job, home, tool, spec) {
  const fs = require('node:fs'), path = require('node:path');
  let stage, timeout, outcome = ['ready', {}];
  try {
    bp_cad_check(job);
    if (spec.error) { outcome = ['unavailable', { error: spec.error }]; return; }
    if (!['kicad', 'freecad'].includes(tool)) throw new Error('Unknown CAD tool');
    await fs.promises.mkdir(home, { recursive: true });
    bp_cad_check(job);
    stage = await fs.promises.mkdtemp(path.join(home, '.cad-install-'));
    bp_cad_check(job);
    timeout = setTimeout(() => { job.reason = 'Installation timed out after 30 minutes. Retry to start again.'; job.controller.abort(); }, 1800000);
    const archive = path.join(stage, spec.asset);
    await bp_cad_download(job, spec.url, archive, spec.sha256);
    bp_cad_check(job);
    if (tool === 'kicad') {
      // The existing pinned installer retains manifest/layout/header/path checks.
      await bp_cad_run(job, 'sh', ['-c', spec.script, 'sh', ...spec.args, archive], stage);
    } else {
      bp_cad_progress(job, 'extract');
      await bp_cad_run(job, 'sh', ['-c', spec.script, 'sh', home, archive, spec.platform, spec.version], stage);
      bp_cad_progress(job, 'configure');
      const command = (await fs.promises.readFile(path.join(home, 'freecad', 'command'), 'utf8')).trim();
      if (!command) throw new Error('FreeCAD command was not configured');
      process.env.BACKPLANE_FREECAD = command;
    }
    bp_cad_check(job);
  } catch (e) {
    // An explicit cancel differs from a timeout/network error and remains retryable.
    outcome = [job.cancelled ? 'cancelled' : 'failed', { error: job.cancelled ? '' : job.reason || e.message || e }];
  } finally {
    clearTimeout(timeout); clearTimeout(job.killTimer);
    if (stage) {
      bp_cad_progress(job, 'cleanup');
      try { await fs.promises.rm(stage, { recursive: true, force: true }); }
      catch (error) { outcome = ['failed', { error: 'Could not remove installation staging files: ' + String(error.message || error) }]; }
    }
    if (job.cancelled && !job.committed && outcome[0] === 'ready') outcome = ['cancelled', {}];
    // Retry and terminal dismissal are available only after cleanup settles.
    job.settled = true;
    bp_cad_progress(job, ...outcome);
  }
}
function bp_cad_reserve(home, tool, attempt) {
  const key = bp_cad_key(home, tool), previous = bp_cad_jobs.get(key);
  if (!['kicad', 'freecad'].includes(tool) || !/^[1-9][0-9]{0,15}$/.test(attempt)) return { $: 'False' };
  if (previous && !previous.settled) return { $: 'False' };
  const job = { controller: new AbortController(), cancelled: false, started: false, settled: false, state: { attempt, phase: 'starting', done: 0 } };
  bp_cad_progress(job, 'starting');
  bp_cad_jobs.set(key, job);
  return { $: 'True' };
}
function bp_cad_start(home, tool, attempt, text) {
  const job = bp_cad_jobs.get(bp_cad_key(home, tool));
  if (!job || job.state.attempt !== attempt || job.started) return '';
  job.started = true;
  if (job.controller.signal.aborted) return attempt;
  if (globalThis.BEND_ASYNC !== true || typeof io_park_promise !== 'function') {
    job.settled = true;
    bp_cad_progress(job, 'failed', { error: 'CAD installation needs the async hub loop. Remove BACKPLANE_JS_LOOP=sync and restart Backplane, then retry.' });
    return attempt;
  }
  // Parsing and filesystem/network failures become a terminal snapshot too.
  job.promise = Promise.resolve().then(() => bp_cad_execute(job, home, tool, JSON.parse(text))).catch(e => {
    job.settled = true;
    bp_cad_progress(job, 'failed', { error: e.message || e });
  });
  return job.state.attempt;
}
async function bp_cad_next_async(home, tool, attempt) {
  // Exactly one watcher per active job. Its snapshots, including phase edges,
  // are sampled at most 4/s per tool (8/s with both active). Phase edges
  // and the immediate hub reservation are separate control transitions.
  await new Promise(resolve => setTimeout(resolve, 250));
  const state = bp_cad_jobs.get(bp_cad_key(home, tool))?.state;
  if (attempt && state?.attempt !== attempt) return '';
  if (!state) return '';
  return bp_cad_wire(state);
}
// Bend effects park their continuation; returning a Promise is not a String.
function bp_cad_next(home, tool, attempt, k) {
  if (globalThis.BEND_ASYNC === false || typeof io_park_promise !== 'function') {
    const job = bp_cad_jobs.get(bp_cad_key(home, tool));
    const error = 'CAD installation needs the async hub loop. Remove BACKPLANE_JS_LOOP=sync and restart Backplane, then retry.';
    if (job && !job.promise) { job.settled = true; bp_cad_progress(job, 'failed', { error }); }
    return job ? bp_cad_wire(job.state) : '';
  }
  return io_park_promise(bp_cad_next_async(home, tool, attempt), k,
    (value, error) => {
      if (error === undefined) return value;
      const job = bp_cad_jobs.get(bp_cad_key(home, tool));
      if (!job || job.state.attempt !== attempt) return '';
      job.reason = 'Could not read installation progress. Retry.';
      job.controller.abort();
      return io_park_promise(job.promise || Promise.resolve(), k, () => bp_cad_wire(job.state));
    });
}
function bp_cad_cancel(home, tool, attempt) {
  const job = bp_cad_jobs.get(bp_cad_key(home, tool));
  if (job && job.state.attempt === attempt && !job.committing && !bp_cad_terminal.has(job.state.phase)) {
    job.cancelled = true; job.controller.abort();
    if (!job.started) { job.settled = true; bp_cad_progress(job, 'cancelled'); }
  }
  return { $: 'Unit' };
}
function bp_cad_dismiss(home, tool, attempt) {
  const key = bp_cad_key(home, tool), job = bp_cad_jobs.get(key);
  if (job && job.state.attempt === attempt && job.settled && bp_cad_terminal.has(job.state.phase)) bp_cad_jobs.delete(key);
  return { $: 'Unit' };
}
// The argument has ALREADY passed through Info.json/Vault masking. Limit
// the CAD-specific progress envelope, without bypassing secret masking or
// claiming a bound on generic join frames containing unrelated hub info.
function bp_cad_frame(text) {
  if (Buffer.byteLength(text, 'utf8') <= BP_CAD_FRAME_BYTES) return text;
  const frame = JSON.parse(text), states = JSON.parse(frame.info['cad.install']);
  const bounded = {};
  for (const tool of ['kicad', 'freecad']) {
    if (!states[tool]) continue;
    const state = states[tool];
    bounded[tool] = { attempt: String(state.attempt || '').slice(0, 64), phase: String(state.phase || 'failed').slice(0, 32), done: bp_cad_number(state.done), total: bp_cad_number(state.total), speed: bp_cad_number(state.speed), eta: bp_cad_number(state.eta), known: state.known === true, error: 'Installation details exceeded the message size limit.' };
  }
  const result = JSON.stringify({ t: 'info', info: { 'cad.install': JSON.stringify(bounded) } });
  if (Buffer.byteLength(result, 'utf8') > BP_CAD_FRAME_BYTES) throw new Error('CAD envelope exceeds its serialized UTF-8 ceiling');
  return result;
}
io_eff(CID(CadFx.reserve), bp_cad_reserve);
io_eff(CID(CadFx.start), bp_cad_start);
io_eff(CID(CadFx.next), bp_cad_next);
io_eff(CID(CadFx.cancel), bp_cad_cancel);
io_eff(CID(CadFx.dismiss), bp_cad_dismiss);
io_eff(CID(CadFx.frame), bp_cad_frame);
