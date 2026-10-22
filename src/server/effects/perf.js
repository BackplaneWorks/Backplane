// Performance counters: the JS twin of perf.c (the same rows, the
// BACKPLANE_TRACE Chrome trace and the BACKPLANE_LOG_FILE copy of the log).

function perf_state() {
  if (globalThis.BACKPLANE_PERF === void 0) {
    globalThis.BACKPLANE_PERF = { t0: performance.now(), tab: new Map(), ring: [] };
  }
  return globalThis.BACKPLANE_PERF;
}

function perf_now_us() {
  return Math.floor((performance.now() - perf_state().t0) * 1000);
}

// the slot for name (made on first use; names cut to 63 bytes); past 1023
// names the overflow slot takes the rest, as perf.c's table
function perf_slot(name) {
  const tab = perf_state().tab;
  if (name.length > 63) {
    name = Buffer.from(name, "utf8").subarray(0, 63).toString("utf8");
  }
  let s = tab.get(name);
  if (s === void 0) {
    if (tab.size >= 1023) {
      name = "perf.overflow";
      s = tab.get(name);
    }
    if (s === void 0) {
      s = { name, kind: 0, n: 0, sum: 0, max: 0, last: 0, mark: 0, b: [] };
      tab.set(name, s);
    }
  }
  return s;
}

// Tracing: events go into a buffer and reach the file whole (at most every
// 200 ms, or when it fills), so a process killed mid-run leaves a file
// that ends between events; the format allows the missing ].
function perf_tracer() {
  const st = perf_state();
  if (st.trace === void 0) {
    st.trace = null;
    const p = typeof process !== "undefined" ? process.env.BACKPLANE_TRACE : "";
    if (p) {
      try {
        const fs = require("fs");
        st.trace = { fd: fs.openSync(p, "w"), buf: [], len: 0, n: 0, flushed: 0 };
        fs.writeSync(st.trace.fd, "[\n");
        process.on("exit", () => {
          perf_trace_flush();
          try {
            fs.writeSync(st.trace.fd, "\n]\n");
            fs.closeSync(st.trace.fd);
          } catch (_) {
          }
        });
      } catch (_) {
        st.trace = null;
      }
    }
  }
  return st.trace;
}

function perf_trace_flush() {
  const t = perf_state().trace;
  if (t) {
    try {
      require("fs").writeSync(t.fd, t.buf.join(""));
    } catch (_) {
    }
    t.buf = [];
    t.len = 0;
  }
}

// one timeline row per area (the name's first part): ui, hub, http, ...
function perf_tid(name) {
  let h = 7;
  for (const c of Buffer.from(name.split(".")[0], "utf8")) {
    h = (Math.imul(h, 31) + c) >>> 0;
  }
  return 1 + h % 997;
}

function perf_trace_put(name, fields, now) {
  const t = perf_tracer();
  if (t === null) {
    return;
  }
  const esc = JSON.stringify(name).slice(1, -1);
  const ev = (t.n === 0 ? "" : ",\n") + "{\"name\":\"" + esc + "\"," + fields + "}";
  t.n += 1;
  if (t.len + ev.length > 65536) {
    perf_trace_flush();
  }
  t.buf.push(ev);
  t.len += ev.length;
  if (now - t.flushed > 200000) {
    t.flushed = now;
    perf_trace_flush();
  }
}

function perf_trace_span(name, now, dur) {
  perf_trace_put(name, `"ph":"X","ts":${now - dur},"dur":${dur},"pid":${process.pid},"tid":${perf_tid(name)}`, now);
}

function perf_trace_count(name, now, v) {
  perf_trace_put(name, `"ph":"C","ts":${now},"pid":${process.pid},"args":{"v":${v}}`, now);
}

function perf_trace_note(line, now) {
  perf_trace_put(line, `"ph":"i","s":"g","ts":${now},"pid":${process.pid}`, now);
}

function perf_put(s, v) {
  s.n += 1;
  s.sum += v;
  s.last = v;
  s.max = Math.max(s.max, v);
  let i = 0;
  for (let x = v; x > 0 && i < 32; x = Math.floor(x / 2)) {
    i += 1;
  }
  while (s.b.length <= i) {
    s.b.push(0);
  }
  s.b[i] += 1;
}

function perf_us() {
  return perf_now_us() >>> 0;
}

function perf_span(name, t0) {
  const full = perf_now_us();
  const now = full >>> 0;
  const dur = (now - t0) >>> 0;
  const s = perf_slot(name);
  perf_put(s, dur);
  perf_trace_span(s.name, full, dur);
  return now;
}

function perf_add(name, v) {
  const s = perf_slot(name);
  s.kind = 2;
  perf_put(s, v);
  perf_trace_count(s.name, perf_now_us(), v);
  return { $: "Unit" };
}

function perf_set(name, v) {
  const s = perf_slot(name);
  if (s.last !== v || s.n === 0) {
    perf_trace_count(s.name, perf_now_us(), v);
  }
  s.kind = 1;
  perf_put(s, v);
  return { $: "Unit" };
}

function perf_mark(name) {
  const s = perf_slot(name);
  if (s.mark === 0) {
    s.mark = perf_now_us() + 1;
  }
  return { $: "Unit" };
}

function perf_done(name) {
  const s = perf_slot(name);
  if (s.mark !== 0) {
    const now = perf_now_us();
    const dur = now - (s.mark - 1);
    s.mark = 0;
    perf_put(s, Math.min(dur, 0xffffffff));
    perf_trace_span(s.name, now, dur);
  }
  return { $: "Unit" };
}

function perf_take() {
  const rows = [];
  const row = (name, s) => rows.push([name, s.kind, s.n, s.sum, s.max, s.last, (s.b.length ? s.b : [0]).join(",")].join("\t"));
  for (const [name, s] of perf_state().tab) {
    if (s.n > 0) {
      row(name, s);
    }
  }
  const g = (name, v) => {
    const s = { kind: 1, n: 0, sum: 0, max: 0, last: 0, b: [] };
    perf_put(s, v);
    row(name, s);
  };
  const mem = typeof process !== "undefined" ? process.memoryUsage().rss : 0;
  const ru = typeof process !== "undefined" && process.resourceUsage ? process.resourceUsage() : null;
  g("proc.rss_kb", Math.floor(mem / 1024));
  g("proc.peak_kb", ru ? ru.maxRSS : 0);
  g("proc.cpu_ms", ru ? Math.floor((ru.userCPUTime + ru.systemCPUTime) / 1000) : 0);
  g("proc.up_ms", Math.floor(perf_now_us() / 1000));
  return rows.map((r) => r + "\n").join("");
}

function perf_emit(line) {
  io_out(2, io_bytes(line + "\n"));
  const st = perf_state();
  if (st.logf === void 0) {
    st.logf = null;
    const p = typeof process !== "undefined" ? process.env.BACKPLANE_LOG_FILE : "";
    if (p) {
      try {
        st.logf = require("fs").openSync(p, "a");
      } catch (_) {
      }
    }
  }
  if (st.logf !== null) {
    try {
      require("fs").writeSync(st.logf, line + "\n");
    } catch (_) {
    }
  }
  perf_trace_note(line, perf_now_us());
  const ring = perf_state().ring;
  ring.push(line);
  if (ring.length > 512) {
    ring.shift();
  }
  return { $: "Unit" };
}

function perf_logs() {
  return perf_state().ring.map((l) => l + "\n").join("");
}

io_eff(CID(Perf.us), perf_us);
io_eff(CID(Perf.span), perf_span);
io_eff(CID(Perf.add), perf_add);
io_eff(CID(Perf.set), perf_set);
io_eff(CID(Perf.mark), perf_mark);
io_eff(CID(Perf.done), perf_done);
io_eff(CID(Perf.take), perf_take);
io_eff(CID(Perf.emit), perf_emit);
io_eff(CID(Perf.logs), perf_logs);
