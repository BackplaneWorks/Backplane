// Local structured diagnostics. Effects run on Bend's one IO loop; there
// are no timers, promises, network calls or free-form log inputs. Exit and
// exception-monitor callbacks only save bounded, private process metadata.
// The hub owns its home lock before start(), and calls tick() once a second.

const diagnostics_limits = Object.freeze({
  disk_bytes: 1048576, segment_bytes: 65536, segments: 8,
  retention_seconds: 7 * 86400, flush_ms: 30000, sweep_ms: 60000,
  retry_ms: 60000, record_bytes: 512, export_bytes: 1048576,
});
const diagnostics_operations = new Set([
  "hub.rpc", "hub.mcp", "hub.join", "hub.mobile_join", "hub.join_here",
  "hub.leave", "hub.thin-read", "hub.agent_line", "hub.agent_gone",
  "hub.tick", "hub.heart", "hub.commit", "hub.push", "hub.push_bin",
  "hub.step.busy", "hub.step.far", "hub.step.outs", "hub.step.pend", "hub.step.commit",
  "boot.store", "boot.replay", "boot.files", "boot.kicad", "boot.cfg",
  "boot.thin", "boot.mail", "boot.origin", "boot.helpers", "boot.adopt", "boot.listen",
  "thin.warm", "mail.warm",
]);
const diagnostics_errors = new Set([
  "store_write", "mail_append", "disk_full", "io", "timeout", "protocol", "agent_exit", "log_error", "log_warn",
]);
const diagnostics_connections = new Set([
  "join", "mobile_join", "leave", "drop", "connect", "disconnect", "reconnect", "failed",
]);
const diagnostics_gauges = new Set(["hub.clients", "hub.agents", "hub.busy"]);

function diagnostics_now() { return Date.now(); }
function diagnostics_mono() { return performance.now(); }
function diagnostics_unit() { return { $: "Unit" }; }
function diagnostics_uint(n, max = 0xffffffff) {
  return Number.isSafeInteger(n) && n >= 0 && n <= max;
}
function diagnostics_sum(a, b) { return Math.min(0xffffffff, a + b); }
function diagnostics_fault(code) { const e = new Error(); e.code = code; return e; }
function diagnostics_error_code(e) {
  return e?.code === "ENOSPC" || e?.code === "EDQUOT" ? "full"
    : e?.code === "UNSAFE" ? "unsafe_storage" : e?.code === "CAP" ? "capacity" : "io";
}
function diagnostics_fail(s, e) {
  s.error = diagnostics_error_code(e);
  s.retry = diagnostics_mono() + diagnostics_limits.retry_ms;
  s.dropped = diagnostics_sum(s.dropped, s.pending.size);
  s.pending.clear();
  try { if (s.dir) diagnostics_usage(s); } catch (_) { /* report only the closed error */ }
}
function diagnostics_state() { return globalThis.BACKPLANE_DIAGNOSTICS?.state; }

// Export and recovery both reconstruct records through this same closed
// schema. Unknown fields are never copied, even from an edited file.
const diagnostics_exception_kinds = new Set(["Error", "RangeError", "TypeError", "SyntaxError", "ReferenceError", "URIError", "EvalError", "none"]);
function diagnostics_version(version) {
  return typeof version === "string" ? version.match(/^(\d{1,6}\.\d{1,6}\.\d{1,6})(?:[-+].*)?$/)?.[1] || "unknown" : "unknown";
}
function diagnostics_exception(error) {
  const out = { exception: "Error", stack: [] };
  try {
    if (diagnostics_exception_kinds.has(error?.name) && error.name !== "none") out.exception = error.name;
    // Keep only numeric frame coordinates, in order. Never copy the first
    // line (the message), function names, file names, URLs or private paths.
    if (typeof error?.stack === "string") for (const line of error.stack.slice(0, 65536).split("\n").slice(1)) {
      const m = line.match(/^\s*at .+:(\d{1,10}):(\d{1,10})\)?$/);
      if (m && Number(m[1]) <= 0xffffffff && Number(m[2]) <= 0xffffffff) out.stack.push([Number(m[1]), Number(m[2])]);
      if (out.stack.length === 8) break;
    }
  } catch (_) { /* An error accessor must not change the exit path. */ }
  return out;
}
function diagnostics_exit_capture(s, error) {
  if (!s?.ready || !s.enabled) return;
  s.exitException = diagnostics_exception(error);
}
function diagnostics_exit(s, code) {
  try {
    if (!s?.ready || !s.enabled || s.exitRecorded) return;
    s.exitRecorded = true;
    const r = { at: Math.floor(diagnostics_now() / 1000), event: "exit", count: 1,
      pid: diagnostics_uint(process.pid) ? process.pid : 0, version: s.appVersion,
      exitCode: Number.isSafeInteger(code) ? code & 255 : 255,
      ...(s.exitException || { exception: "none", stack: [] }) };
    if (Buffer.byteLength(JSON.stringify({ v: 1, seq: Number.MAX_SAFE_INTEGER, ...r })) > diagnostics_limits.record_bytes) r.stack = [];
    s.pending.set("exit", r);
    diagnostics_flush_value(s);
  } catch (_) { /* Best effort only: never set an exit code or throw. */ }
}

function diagnostics_clean(r) {
  if (!r || r.v !== 1 || !diagnostics_uint(r.seq, Number.MAX_SAFE_INTEGER)
      || !diagnostics_uint(r.at, 0xffffffff) || !diagnostics_uint(r.count) || r.count === 0) return null;
  const out = { v: 1, seq: r.seq, at: r.at, event: r.event };
  if (r.event === "exit" && diagnostics_uint(r.pid) && diagnostics_uint(r.exitCode, 255)
      && (r.version === "unknown" || diagnostics_version(r.version) === r.version)
      && diagnostics_exception_kinds.has(r.exception) && Array.isArray(r.stack) && r.stack.length <= 8
      && r.stack.every(f => Array.isArray(f) && f.length === 2 && f.every(n => diagnostics_uint(n)))) {
    return { ...out, count: 1, pid: r.pid, version: r.version, exitCode: r.exitCode, exception: r.exception, stack: r.stack.map(f => [f[0], f[1]]) };
  }
  if (r.event === "latency" && diagnostics_operations.has(r.name)
      && diagnostics_uint(r.us, Number.MAX_SAFE_INTEGER) && Number.isFinite(r.maxMs) && r.maxMs >= 0 && r.maxMs <= 4294967.295) {
    return { ...out, name: r.name, count: r.count, us: r.us, maxMs: r.maxMs };
  }
  if (r.event === "error" && diagnostics_errors.has(r.code)) return { ...out, code: r.code, count: r.count };
  if (r.event === "connection" && typeof r.name === "string" && r.name.startsWith("connection.")
      && diagnostics_connections.has(r.name.slice(11))) return { ...out, name: r.name, count: r.count };
  if (r.event === "sample" && diagnostics_gauges.has(r.name) && diagnostics_uint(r.value)) {
    return { ...out, name: r.name, count: r.count, value: r.value };
  }
  return null;
}

function diagnostics_note(s, event, detail, value) {
  if (!s?.ready || !s.enabled || s.error !== "" || !diagnostics_uint(value)) return;
  if (!(event === "latency" && diagnostics_operations.has(detail))
      && !(event === "error" && diagnostics_errors.has(detail))
      && !(event === "connection" && diagnostics_connections.has(detail))
      && !(event === "sample" && diagnostics_gauges.has(detail))) return;
  const key = event + ":" + detail;
  let r = s.pending.get(key);
  if (!r) {
    const base = { at: Math.floor(diagnostics_now() / 1000), event, count: 0 };
    if (event === "latency") r = { ...base, name: detail, us: 0, maxMs: 0 };
    else if (event === "error") r = { ...base, code: detail };
    else if (event === "connection") r = { ...base, name: "connection." + detail };
    else r = { ...base, name: detail, value: 0 };
  }
  if (event === "latency") {
    r.count = diagnostics_sum(r.count, 1);
    r.us = Math.min(Number.MAX_SAFE_INTEGER, r.us + value);
    r.maxMs = Math.max(r.maxMs, value / 1000);
  } else if (event === "sample") {
    r.count = diagnostics_sum(r.count, 1);
    r.value = value;
  } else r.count = diagnostics_sum(r.count, value);
  if (r.count > 0) s.pending.set(key, r);
}

// Perf calls this only with a measurement, never Perf.emit's log text.
function diagnostics_perf(kind, name, value) {
  const s = diagnostics_state();
  if (!s?.enabled || s.error !== "") return;
  if (kind === "span") {
    diagnostics_note(s, "latency", name, value);
    const phase = { "hub.join": "join", "hub.mobile_join": "mobile_join", "hub.leave": "leave" }[name];
    if (phase) diagnostics_note(s, "connection", phase, 1);
  } else if (kind === "add") {
    if (name === "hub.write_fail") diagnostics_note(s, "error", "store_write", value);
    else if (name === "mail.append_fail") diagnostics_note(s, "error", "mail_append", value);
    else if (name === "hub.client_drop") diagnostics_note(s, "connection", "drop", value);
  } else if (kind === "set") diagnostics_note(s, "sample", name, value);
  else if (kind === "severity") {
    if (value === 0) diagnostics_note(s, "error", "log_error", 1);
    else if (value === 1) diagnostics_note(s, "error", "log_warn", 1);
  }
}

function diagnostics_file(s, name) { return require("path").join(s.dir, name); }
function diagnostics_segment(i) { return "segment-" + i + ".jsonl"; }
function diagnostics_names() {
  return ["control", ...Array.from({ length: diagnostics_limits.segments }, (_, i) => diagnostics_segment(i))];
}
function diagnostics_lstat(s, name) {
  const fs = require("fs");
  try {
    const st = fs.lstatSync(diagnostics_file(s, name));
    if (!st.isFile() || st.nlink !== 1) throw diagnostics_fault("UNSAFE");
    return st;
  } catch (e) { if (e.code === "ENOENT") return null; throw e; }
}
function diagnostics_directory(s) {
  const fs = require("fs");
  const st = fs.lstatSync(s.dir);
  if (!st.isDirectory() || st.isSymbolicLink()) throw diagnostics_fault("UNSAFE");
  fs.chmodSync(s.dir, 0o700);
  const names = new Set(diagnostics_names());
  // This is an exclusively owned, fixed-name directory. Unexpected files
  // fail closed; they are never exported, followed or deleted.
  for (const n of fs.readdirSync(s.dir)) if (!names.has(n)) throw diagnostics_fault("UNSAFE");
}
function diagnostics_open_file(s, name, create = false) {
  const fs = require("fs"), p = diagnostics_file(s, name);
  const before = diagnostics_lstat(s, name);
  if (!before && !create) throw diagnostics_fault("UNSAFE");
  const flags = fs.constants.O_RDWR | (fs.constants.O_NOFOLLOW || 0)
    | (!before && create ? fs.constants.O_CREAT | fs.constants.O_EXCL : 0);
  const fd = fs.openSync(p, flags, 0o600);
  try {
    const st = fs.fstatSync(fd);
    if (!st.isFile() || st.nlink !== 1 || (before && (before.ino !== st.ino || before.dev !== st.dev))) throw diagnostics_fault("UNSAFE");
    fs.fchmodSync(fd, 0o600);
    return fd;
  } catch (e) { fs.closeSync(fd); throw e; }
}
function diagnostics_usage(s) {
  let bytes = 0, allocated = 0;
  for (const name of diagnostics_names()) {
    const st = diagnostics_lstat(s, name);
    if (st) { bytes += st.size; allocated += Math.max(st.size, Number(st.blocks || 0) * 512); }
  }
  s.bytes = bytes; s.allocated = allocated;
  return Math.max(bytes, allocated);
}
function diagnostics_remove(s, i) { require("fs").unlinkSync(diagnostics_file(s, diagnostics_segment(i))); }

// At most eight 64 KiB reads. A segment crossing the age limit is deleted
// whole (possibly sacrificing newer rows). Torn final records are cut back
// to the last newline. Invalid complete rows discard the segment.
function diagnostics_scan(s) {
  const fs = require("fs"), now = Math.floor(diagnostics_now() / 1000);
  diagnostics_directory(s);
  const files = [];
  for (let i = 0; i < diagnostics_limits.segments; i++) {
    const name = diagnostics_segment(i), st = diagnostics_lstat(s, name);
    if (!st) continue;
    if (st.size > diagnostics_limits.segment_bytes) { diagnostics_remove(s, i); continue; }
    const fd = diagnostics_open_file(s, name);
    let rows = [], invalid = false, size = 0;
    try {
      const buf = Buffer.alloc(st.size);
      let got = 0;
      while (got < buf.length) {
        const n = fs.readSync(fd, buf, got, buf.length - got, got);
        if (n <= 0) throw diagnostics_fault("IO");
        got += n;
      }
      size = buf.lastIndexOf(10) + 1;
      if (size !== st.size) fs.ftruncateSync(fd, size);
      for (const line of buf.subarray(0, size).toString("utf8").split("\n")) {
        if (!line) continue;
        if (Buffer.byteLength(line) > diagnostics_limits.record_bytes) { invalid = true; break; }
        let r;
        try { r = diagnostics_clean(JSON.parse(line)); } catch (_) { r = null; }
        if (!r || r.at < now - diagnostics_limits.retention_seconds || r.at > now) { invalid = true; break; }
        rows.push(r);
      }
    } finally { fs.closeSync(fd); }
    if (invalid || rows.length === 0) { diagnostics_remove(s, i); continue; }
    const seq = Math.max(...rows.map(r => r.seq));
    s.seq = Math.max(s.seq, seq);
    files.push({ i, seq, size, rows });
  }
  files.sort((a, b) => a.seq - b.seq);
  // Reserve one entire segment plus allocation slack BEFORE writing.
  while (diagnostics_usage(s) > diagnostics_limits.disk_bytes - diagnostics_limits.segment_bytes - 8192 && files.length) {
    diagnostics_remove(s, files.shift().i);
  }
  if (diagnostics_usage(s) > diagnostics_limits.disk_bytes - diagnostics_limits.segment_bytes - 8192) throw diagnostics_fault("CAP");
  s.swept = diagnostics_mono();
  return files;
}

function diagnostics_write_control(s, enabled) {
  const fs = require("fs"), fd = diagnostics_open_file(s, "control", true);
  try {
    // An interrupted or failed control write leaves an empty/invalid marker,
    // which recovery treats as disabled. Never report a durable success early.
    fs.ftruncateSync(fd, 0);
    const b = Buffer.from(enabled ? "1" : "0");
    if (fs.writeSync(fd, b, 0, 1, 0) !== 1) throw diagnostics_fault("IO");
    fs.fsyncSync(fd);
    if (diagnostics_usage(s) > diagnostics_limits.disk_bytes) throw diagnostics_fault("CAP");
  } finally { fs.closeSync(fd); }
}

function diagnostics_status_value(s, ok = !!s?.ready && s.error === "") {
  return {
    version: 1, ok, started: !!s?.ready, enabled: !!s?.enabled,
    error: s?.error || "", stored_bytes: s?.bytes || 0, allocated_bytes: s?.allocated || 0,
    cap_bytes: diagnostics_limits.disk_bytes, retention_seconds: diagnostics_limits.retention_seconds,
    pending: s?.pending.size || 0, dropped: s?.dropped || 0,
  };
}
function diagnostics_status() { return JSON.stringify(diagnostics_status_value(diagnostics_state())); }

function diagnostics_start(version) {
  const old = diagnostics_state();
  // One hub, one home. A second start does not lose pending measurements.
  if (old) return diagnostics_status();
  const s = { dir: "", ready: false, enabled: false, error: "", pending: new Map(),
    bytes: 0, allocated: 0, seq: 0, dropped: 0, retry: 0, flushed: diagnostics_mono(), swept: 0 };
  s.appVersion = diagnostics_version(version);
  globalThis.BACKPLANE_DIAGNOSTICS = { state: s, recordPerf: diagnostics_perf,
    captureException: error => diagnostics_exit_capture(s, error) };
  // Monitor does not handle or suppress an uncaught exception. In particular,
  // do not register uncaughtException or unhandledRejection handlers. The
  // scheduler also captures its rejected run immediately before its existing
  // process.exit(1). All writes happen synchronously in the exit listener.
  try {
    process.on?.("uncaughtExceptionMonitor", error => diagnostics_exit_capture(s, error));
    process.once?.("exit", code => diagnostics_exit(s, code));
  } catch (_) { /* Unsupported monitor APIs do not affect hub startup. */ }
  try {
    const home = typeof process !== "undefined" ? process.env.BACKPLANE_HOME : "";
    if (typeof home !== "string" || !require("path").isAbsolute(home) || home.includes("\0")) throw diagnostics_fault("UNSAFE");
    s.dir = require("path").join(home, "diagnostics");
    const fs = require("fs");
    try { fs.mkdirSync(s.dir, { mode: 0o700 }); } catch (e) { if (e.code !== "EEXIST") throw e; }
    diagnostics_directory(s);
    const st = diagnostics_lstat(s, "control");
    if (!st) diagnostics_write_control(s, true);
    // Bound this read too; a malformed marker always means off.
    const control = diagnostics_lstat(s, "control");
    if (control.size > 1) {
      diagnostics_write_control(s, false);
    } else {
      const fd = diagnostics_open_file(s, "control");
      try { const b = Buffer.alloc(1); s.enabled = fs.readSync(fd, b, 0, 1, 0) === 1 && b[0] === 49; }
      finally { fs.closeSync(fd); }
    }
    diagnostics_scan(s);
    s.ready = true;
  } catch (e) { s.enabled = false; diagnostics_fail(s, e); }
  return JSON.stringify(diagnostics_status_value(s, s.ready));
}

function diagnostics_flush_value(s) {
  if (!s?.ready) return false;
  if (s.error && diagnostics_mono() < s.retry) return false;
  let attempted = 0;
  try {
    const files = diagnostics_scan(s);
    s.error = "";
    if (!s.enabled || s.pending.size === 0) return true;
    const at = Math.floor(diagnostics_now() / 1000);
    const rows = [...s.pending.values()]
      .filter(r => r.at >= at - diagnostics_limits.retention_seconds && r.at <= at)
      .map(r => ({ v: 1, seq: ++s.seq, ...r }));
    attempted = rows.length;
    const data = Buffer.from(rows.map(r => JSON.stringify(r) + "\n").join(""));
    s.pending.clear();
    if (rows.length === 0) return true;
    if (data.length > diagnostics_limits.segment_bytes) throw diagnostics_fault("CAP");
    let file = files[files.length - 1];
    if (!file || file.size + data.length > diagnostics_limits.segment_bytes) {
      const used = new Set(files.map(f => f.i));
      const i = Array.from({ length: diagnostics_limits.segments }, (_, k) => k).find(k => !used.has(k)) ?? files[0].i;
      if (used.has(i)) diagnostics_remove(s, i);
      file = { i, size: 0 };
    }
    if (diagnostics_usage(s) + data.length + 8192 > diagnostics_limits.disk_bytes) throw diagnostics_fault("CAP");
    const fs = require("fs"), fd = diagnostics_open_file(s, diagnostics_segment(file.i), true);
    try {
      if (fs.fstatSync(fd).size !== file.size) throw diagnostics_fault("UNSAFE");
      // One bounded append per interval. A partial write is rolled back; no
      // queued retries and no growing in-memory backlog on a full disk.
      if (fs.writeSync(fd, data, 0, data.length, file.size) !== data.length) throw diagnostics_fault("IO");
      if (diagnostics_usage(s) > diagnostics_limits.disk_bytes) throw diagnostics_fault("CAP");
    } catch (e) {
      try { fs.ftruncateSync(fd, file.size); } catch (_) { diagnostics_remove(s, file.i); }
      throw e;
    } finally { fs.closeSync(fd); }
    diagnostics_usage(s);
    s.flushed = diagnostics_mono();
    return true;
  } catch (e) {
    s.dropped = diagnostics_sum(s.dropped, attempted);
    diagnostics_fail(s, e);
    return false;
  }
}
function diagnostics_flush() {
  const s = diagnostics_state(), ok = diagnostics_flush_value(s);
  return JSON.stringify(diagnostics_status_value(s, ok));
}
function diagnostics_tick() {
  const s = diagnostics_state(), now = diagnostics_mono();
  if (s?.ready && (now - s.flushed >= diagnostics_limits.flush_ms || now - s.swept >= diagnostics_limits.sweep_ms)) {
    diagnostics_flush_value(s);
    // Also pace an idle or disabled hub; no rescan on every message.
    s.flushed = now;
  }
  return diagnostics_unit();
}
function diagnostics_event(event, detail, value) {
  diagnostics_note(diagnostics_state(), event, detail, value);
  return diagnostics_unit();
}
function diagnostics_control(action) {
  const s = diagnostics_state();
  if (!s?.ready) return JSON.stringify(diagnostics_status_value(s, false));
  if (!["on", "off", "clear"].includes(action)) {
    return JSON.stringify({ ...diagnostics_status_value(s, false), error: "invalid_action" });
  }
  // Disable and clear discard pending data BEFORE touching disk.
  if (action !== "on") s.pending.clear();
  if (action === "off") s.enabled = false;
  try {
    diagnostics_directory(s);
    if (action === "clear") {
      for (let i = 0; i < diagnostics_limits.segments; i++) {
        if (diagnostics_lstat(s, diagnostics_segment(i))) diagnostics_remove(s, i);
      }
      s.seq = 0; s.dropped = 0;
    } else {
      diagnostics_write_control(s, action === "on");
      s.enabled = action === "on";
    }
    diagnostics_usage(s);
    s.error = ""; s.retry = 0;
    return JSON.stringify(diagnostics_status_value(s));
  } catch (e) {
    if (action === "on") s.enabled = false;
    diagnostics_fail(s, e);
    return JSON.stringify(diagnostics_status_value(s, false));
  }
}
function diagnostics_export() {
  const s = diagnostics_state();
  let events = [], ok = false;
  if (s?.ready) {
    ok = diagnostics_flush_value(s);
    try {
      // Re-read and re-allowlist persisted bytes. Never attach perf logs,
      // traces, exception messages, configuration or a source filename.
      events = diagnostics_scan(s).flatMap(f => f.rows).filter(r => r.event !== "exit").sort((a, b) => a.seq - b.seq)
        .map(({ seq, ...r }) => r.event === "latency" ? { ...r, meanMs: r.us / r.count / 1000 } : r);
    } catch (e) { diagnostics_fail(s, e); ok = false; }
  }
  const bundle = { version: 1, status: diagnostics_status_value(s, ok), events };
  const text = JSON.stringify(bundle);
  if (Buffer.byteLength(text) > diagnostics_limits.export_bytes) {
    return JSON.stringify({ version: 1, status: { ...diagnostics_status_value(s, false), error: "capacity" }, events: [] });
  }
  return text;
}

io_eff(CID(Diagnostics.start), diagnostics_start);
io_eff(CID(Diagnostics.status), diagnostics_status);
io_eff(CID(Diagnostics.tick), diagnostics_tick);
io_eff(CID(Diagnostics.flush), diagnostics_flush);
io_eff(CID(Diagnostics.event), diagnostics_event);
io_eff(CID(Diagnostics.control), diagnostics_control);
io_eff(CID(Diagnostics.export), diagnostics_export);
