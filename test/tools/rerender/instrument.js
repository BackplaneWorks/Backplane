// The re-render harness's in-page recorder (test/tools/rerender/README.md).
// Installed before the page's own scripts (Playwright's addInitScript), so
// it sees every socket and every DOM write. Sets window.__bpRender, which
// host.js and timeline-scroller.js read for their own counters (off in
// normal use: nothing here is loaded then).
//
// Each scenario is one window: begin(name) ... end() → a plain object:
//   dom:     mutation records by region and kind, nodes added/removed,
//            attributes by name, net no-op writes, nodes recreated identical
//   rows:    timeline rows touched, really changed, touched but unchanged,
//            mounted/unmounted
//   ws:      frames and bytes in/out by message kind, request→reply ms
//   frames:  rAF frames, long frames (>25 ms), dropped frames, LoAF entries
//            with their scripts (the code path)
//   events:  Event Timing entries (input → next paint) by type
//   scroll:  wheel → next frame after its scroll, blank frames (rows not
//            covering the timeline), loading placeholders seen
//   counts:  the page's own counters (render.full, dom.patch, tl.*, ...)
(() => {
  if (window.__bpRR) return;
  const now = () => performance.now();
  const W = { on: false };
  const counts = Object.create(null), times = Object.create(null);
  // the page's counters
  // the hub's messages as the page decoded them (host.js): by kind, with
  // their bytes and decode time; requests by method, with their reply's
  const msgs = Object.create(null), sentAt = new Map();
  window.__bpRender = {
    sent(id, method) { if (W.on) sentAt.set(id, { method, at: now() }); },
    got(env, b, ms) {
      if (!W.on) return;
      let kind = env?.t || "?";
      if (kind === "reply" && sentAt.has(env.id)) { const s = sentAt.get(env.id); sentAt.delete(env.id); kind = "reply:" + s.method;
        const r = msgs["rtt:" + s.method] || (msgs["rtt:" + s.method] = { n: 0, bytes: 0, decodeMs: 0, ms: [] }); r.n++; r.ms.push(now() - s.at); }
      else if (kind === "view.changed") kind = "view.changed";
      const x = msgs[kind] || (msgs[kind] = { n: 0, bytes: 0, decodeMs: 0 }); x.n++; x.bytes += b; x.decodeMs += ms;
    },
    n(name, k = 1) { if (W.on) counts[name] = (counts[name] || 0) + k; },
    t(name, ms) { if (!W.on) return; const x = times[name] || (times[name] = { n: 0, total: 0, max: 0 }); x.n++;x.total += ms;if (ms > x.max) x.max = ms; },
  };

  // Regions: which part of the page a node is in
  // --------------------------------------------
  const REGIONS = [
    ["composer", "#composer, .composer"],
    ["asks", "#asks"],
    ["workbar", ".thread-workbar"],
    ["timeline-row", ".timeline-virtual-row"],
    ["timeline", "#timeline"],
    ["subagents", ".subs, .subagents"],
    ["todos", ".todos, .todo-dock"],
    ["thread-head", ".thread-head, .thread-header, .thread-top, header"],
    ["sidebar", ".sidebar, #sidebar, .side, aside, nav"],
    ["viewer", "#viewer"],
    ["status", "#history-status, #view-status, .view-status"],
  ];
  const elOf = (n) => (n && n.nodeType === 1 ? n : n?.parentElement) || null;
  function region(n) {
    const el = elOf(n);
    if (!el || !el.isConnected) return "detached";
    for (const [name, sel] of REGIONS) if (el.closest(sel)) return name;
    return "page";
  }
  const rowKey = (n) => elOf(n)?.closest?.(".timeline-virtual-row")?.dataset.virtualKey || "";
  const size = (n) => (n.nodeType === 1 ? 1 + n.getElementsByTagName("*").length : 1);
  // a cheap fingerprint of a subtree (FNV-1a over its markup, capped)
  function hash(s) { let h = 2166136261; for (let i = 0; i < s.length; i++) { h ^= s.charCodeAt(i);h = Math.imul(h, 16777619); } return (h >>> 0).toString(36) + ":" + s.length; }
  const markup = (n) => (n.nodeType === 1 ? n.outerHTML : n.nodeType === 3 ? "#t" + n.nodeValue : "");
  const bump = (o, k, by = 1) => { o[k] = (o[k] || 0) + by; };

  // DOM: a MutationObserver over the whole document
  // -----------------------------------------------
  let D, rowsTouched, attrFirst, textFirst, removedHashes, addedHashes, loadingSeen;
  function resetDom() {
    D = { records: 0, byRegion: {}, attrs: {}, added: 0, removed: 0, text: 0, styleOnly: 0, recreatedIdentical: 0, netNoop: 0, regionNodes: {} };
    rowsTouched = new Map(); attrFirst = new Map(); textFirst = new Map(); removedHashes = new Map(); addedHashes = new Map(); loadingSeen = [];
  }
  resetDom();
  const LOADING = /Loading (earlier|newer)|Loading…|Loading\.\.\./;
  const mo = new MutationObserver((records) => {
    if (!W.on) return;
    for (const r of records) {
      D.records++;
      const reg = region(r.target);
      const byr = D.byRegion[reg] || (D.byRegion[reg] = { records: 0, childList: 0, attributes: 0, characterData: 0, added: 0, removed: 0 });
      byr.records++; byr[r.type]++;
      const key = rowKey(r.target);
      if (key) rowsTouched.set(key, (rowsTouched.get(key) || 0) + 1);
      if (r.type === "attributes") {
        bump(D.attrs, (reg === "timeline-row" || reg === "timeline") && r.attributeName === "style" && r.target.classList?.contains("timeline-virtual-row") ? "row-style" : r.attributeName);
        if (r.attributeName === "style" && (r.target.classList?.contains("timeline-virtual-row") || r.target.classList?.contains("timeline-virtual-track"))) D.styleOnly++;
        let m = attrFirst.get(r.target);
        if (!m) attrFirst.set(r.target, (m = new Map()));
        if (!m.has(r.attributeName)) m.set(r.attributeName, r.oldValue);
      } else if (r.type === "characterData") {
        D.text++;
        if (!textFirst.has(r.target)) textFirst.set(r.target, r.oldValue);
      } else {
        for (const n of r.addedNodes) {
          const k = size(n); D.added += k; byr.added += k;
          if (n.nodeType === 1 || n.nodeType === 3) { const h = hash(markup(n)); addedHashes.set(h, (addedHashes.get(h) || 0) + 1); }
          const t = n.textContent || "";
          if (n.nodeType === 1 && LOADING.test(t.slice(0, 200))) loadingSeen.push({ at: now(), text: t.slice(0, 80), region: reg });
        }
        for (const n of r.removedNodes) {
          const k = size(n); D.removed += k; byr.removed += k;
          if (n.nodeType === 1 || n.nodeType === 3) { const h = hash(markup(n)); removedHashes.set(h, (removedHashes.get(h) || 0) + 1); }
        }
      }
    }
  });
  const startMo = () => mo.observe(document, { subtree: true, childList: true, attributes: true, characterData: true, attributeOldValue: true, characterDataOldValue: true });
  if (document.documentElement) startMo(); else document.addEventListener("DOMContentLoaded", startMo);
  // a text-node status ("Loading earlier messages…") written in place
  function checkLoadingText() {
    const s = document.getElementById("history-status");
    if (s && LOADING.test(s.textContent || "")) loadingSeen.push({ at: now(), text: s.textContent.slice(0, 80), region: "status" });
  }

  // Timeline rows: what each mounted row looks like (begin vs end)
  // --------------------------------------------------------------
  function rowShots() {
    const m = new Map();
    for (const w of document.querySelectorAll(".timeline-virtual-row")) m.set(w.dataset.virtualKey || "", hash(w.innerHTML));
    return m;
  }

  // Sockets: frames and bytes by kind, request → reply times
  // --------------------------------------------------------
  let S, pend;
  const resetWs = () => { S = { in: {}, out: {}, inBytes: 0, outBytes: 0, inFrames: 0, outFrames: 0, rpc: {} }; pend = new Map(); };
  resetWs();
  const dec = new TextDecoder();
  // CBOR text keys are written as plain UTF-8 inside the frame: enough to
  // name a message by its "t" and "method" without decoding it.
  function kindOf(data) {
    let s;
    if (typeof data === "string") s = data.slice(0, 600);
    else if (data instanceof ArrayBuffer) s = dec.decode(new Uint8Array(data, 0, Math.min(600, data.byteLength)));
    else return { kind: "blob", id: null };
    const t = /"t":"([\w.]+)"/.exec(s)?.[1] ?? /\x61t[\x60-\x77]([\w.]+)/.exec(s)?.[1] ?? "";
    const method = /"method":"([\w.]+)"/.exec(s)?.[1] ?? /method[\x60-\x77]([\w.]+)/.exec(s)?.[1] ?? "";
    const id = /"id":(\d+)/.exec(s)?.[1] ?? null;
    return { kind: [t, method].filter(Boolean).join(":") || "other", id, method };
  }
  const bytes = (d) => (typeof d === "string" ? new TextEncoder().encode(d).length : d?.byteLength ?? d?.size ?? 0);
  const NativeWS = window.WebSocket;
  class RecWS extends NativeWS {
    constructor(...a) {
      super(...a);
      this.addEventListener("message", (e) => {
        W.lastWs = now();
        if (!W.on) return;
        const b = bytes(e.data), k = kindOf(e.data);
        S.inBytes += b; S.inFrames++;
        const x = S.in[k.kind] || (S.in[k.kind] = { frames: 0, bytes: 0 }); x.frames++; x.bytes += b;
        if (k.id !== null && pend.has(k.id)) {
          const p = pend.get(k.id); pend.delete(k.id);
          const r = S.rpc[p.method] || (S.rpc[p.method] = { n: 0, ms: [], replyBytes: 0 });
          r.n++; r.ms.push(now() - p.at); r.replyBytes += b;
        }
      });
    }
    send(d) {
      W.lastWs = now();
      if (W.on) {
        const b = bytes(d), k = kindOf(d);
        S.outBytes += b; S.outFrames++;
        const x = S.out[k.kind] || (S.out[k.kind] = { frames: 0, bytes: 0 }); x.frames++; x.bytes += b;
        if (k.id !== null) pend.set(k.id, { method: k.method || k.kind, at: now() });
      }
      return super.send(d);
    }
  }
  window.WebSocket = RecWS;

  // Frames: rAF gaps, Long Animation Frames, Event Timing
  // -----------------------------------------------------
  let F, E, loaf, lastFrame = 0, scrollInfo;
  const resetFrames = () => { F = { frames: 0, long: 0, dropped: 0, worst: 0, gaps: [] }; E = {}; loaf = []; lastFrame = 0; scrollInfo = { wheel: [], blank: 0, checked: 0, minCover: 1 }; };
  resetFrames();
  function frame(t) {
    if (W.on) {
      if (lastFrame) {
        const d = t - lastFrame; F.frames++; if (d > F.worst) F.worst = d;
        if (d > 25) { F.long++; F.dropped += Math.max(0, Math.round(d / 16.7) - 1); F.gaps.push(Math.round(d)); }
      }
      lastFrame = t;
      if (W.watchBlank) blankCheck();
      checkLoadingText();
    }
    requestAnimationFrame(frame);
  }
  requestAnimationFrame(frame);
  try {
    new PerformanceObserver((l) => {
      if (!W.on) return;
      for (const e of l.getEntries()) {
        if (e.startTime < W.t0) continue;
        loaf.push({ ms: Math.round(e.duration), blocking: Math.round(e.blockingDuration || 0), render: Math.round(e.renderStart ? e.startTime + e.duration - e.renderStart : 0),
          style: Math.round(e.styleAndLayoutStart ? e.startTime + e.duration - e.styleAndLayoutStart : 0),
          scripts: (e.scripts || []).map((s) => ({ ms: Math.round(s.duration), invoker: s.invoker, fn: s.sourceFunctionName, src: (s.sourceURL || "").split("/").pop() + ":" + (s.sourceCharPosition ?? ""), forced: Math.round(s.forcedStyleAndLayoutDuration || 0) })) });
      }
    }).observe({ type: "long-animation-frame", buffered: false });
  } catch {}
  try {
    new PerformanceObserver((l) => {
      if (!W.on) return;
      for (const e of l.getEntries()) {
        if (e.startTime < W.t0) continue;
        const x = E[e.name] || (E[e.name] = []);
        x.push({ ms: Math.round(e.duration), input: Math.round(e.processingStart - e.startTime), handler: Math.round(e.processingEnd - e.processingStart) });
      }
    }).observe({ type: "event", durationThreshold: 16, buffered: false });
  } catch {}
  // all key events (Event Timing only reports those over 16 ms): input → the frame after
  for (const type of ["keydown", "input"]) {
    addEventListener(type, (e) => {
      if (!W.on) return;
      const t0 = e.timeStamp;
      requestAnimationFrame(() => setTimeout(() => { const x = E["paint:" + type] || (E["paint:" + type] = []); x.push({ ms: Math.round(now() - t0) }); }, 0));
    }, true);
  }
  // wheel → the frame after its scroll
  let wheelAt = 0;
  addEventListener("wheel", (e) => { if (W.on) wheelAt = e.timeStamp; }, { capture: true, passive: true });
  addEventListener("scroll", () => {
    if (!W.on || !wheelAt) return;
    const t0 = wheelAt; wheelAt = 0;
    requestAnimationFrame(() => scrollInfo.wheel.push(Math.round(now() - t0)));
  }, { capture: true, passive: true });
  // a frame where the rows on screen leave part of the timeline empty
  function blankCheck() {
    const tl = document.getElementById("timeline");
    if (!tl) return;
    const box = tl.getBoundingClientRect();
    if (box.height < 50) return;
    const spans = [];
    for (const w of tl.querySelectorAll(".timeline-virtual-row")) {
      const r = w.getBoundingClientRect();
      if (r.bottom > box.top && r.top < box.bottom && r.height > 0) spans.push([Math.max(r.top, box.top), Math.min(r.bottom, box.bottom)]);
    }
    spans.sort((a, b) => a[0] - b[0]);
    let covered = 0, end = box.top;
    for (const [a, b] of spans) { if (b > end) { covered += b - Math.max(a, end); end = b; } }
    // the content may be shorter than the box (a short thread): judge against it
    const track = tl.querySelector(".timeline-virtual-track");
    const want = Math.min(box.height, track ? track.getBoundingClientRect().height : box.height);
    const cover = want > 0 ? Math.min(1, covered / want) : 1;
    // judged against the cover at the window's start (padding under the
    // composer or above the first row is no gap)
    if (scrollInfo.base === undefined) scrollInfo.base = cover;
    scrollInfo.checked++;
    if (cover < scrollInfo.minCover) scrollInfo.minCover = cover;
    if (cover < scrollInfo.base - 0.1) scrollInfo.blank++;
  }

  // The window
  // ----------
  let name = "", rows0 = null;
  window.__bpRR = {
    begin(n, opts = {}) {
      name = n; resetDom(); resetWs(); resetFrames();
      for (const k in counts) delete counts[k];
      for (const k in msgs) delete msgs[k];
      sentAt.clear();
      for (const k in times) delete times[k];
      wheelAt = 0; rows0 = rowShots(); W.watchBlank = !!opts.blank; W.t0 = now(); W.on = true;
      return W.t0;
    },
    // ms since the last DOM mutation or socket frame (the harness waits for quiet)
    quiet() { return now() - Math.max(W.lastDom || 0, W.lastWs || 0, W.t0 || 0); },
    pending() { let n = 0; for (const v of sentAt.values()) if (now() - v.at < 5000) n++; return n; },
    async end() {
      await new Promise((r) => setTimeout(r, 0)); // flush the observers
      for (const r of mo.takeRecords()) {} // already counted by the callback when on
      W.on = false;
      const ms = now() - W.t0;
      // net no-op writes: an attribute or a text that ends where it began
      for (const [el, m] of attrFirst) for (const [a, v] of m) if (el.isConnected && el.getAttribute(a) === v) D.netNoop++;
      for (const [t, v] of textFirst) if (t.isConnected && t.nodeValue === v) D.netNoop++;
      // a node removed and an identical one added: recreated for nothing
      for (const [h, k] of removedHashes) { const a = addedHashes.get(h) || 0; D.recreatedIdentical += Math.min(k, a); }
      const rows1 = rowShots();
      let changed = 0, unchanged = 0;
      for (const [k] of rowsTouched) { if (!rows0.has(k) || !rows1.has(k)) continue; if (rows0.get(k) === rows1.get(k)) unchanged++; else changed++; }
      let mounted = 0, unmounted = 0;
      for (const k of rows1.keys()) if (!rows0.has(k)) mounted++;
      for (const k of rows0.keys()) if (!rows1.has(k)) unmounted++;
      const pctl = (xs, p) => { if (!xs.length) return 0; const s = [...xs].sort((a, b) => a - b); return s[Math.min(s.length - 1, Math.floor(p * s.length))]; };
      const rpc = {}; for (const [m, r] of Object.entries(S.rpc)) rpc[m] = { n: r.n, p50: Math.round(pctl(r.ms, 0.5)), p95: Math.round(pctl(r.ms, 0.95)), max: Math.round(Math.max(...r.ms)), replyBytes: r.replyBytes };
      const ev = {}; for (const [k, xs] of Object.entries(E)) { const v = xs.map((x) => x.ms); ev[k] = { n: v.length, p50: pctl(v, 0.5), p95: pctl(v, 0.95), max: Math.max(...v) }; }
      return {
        name, ms: Math.round(ms), activeMs: Math.round(Math.max(W.lastDom || 0, W.lastWs || 0, W.t0) - W.t0),
        dom: { ...D, rowsTouched: rowsTouched.size },
        rows: { touched: [...rowsTouched.keys()].filter((k) => rows0.has(k) && rows1.has(k)).length, changed, touchedUnchanged: unchanged, mounted, unmounted, mountedNow: rows1.size },
        ws: { ...S, rpc },
        frames: { ...F, gaps: F.gaps.slice(0, 40), loaf: loaf.sort((a, b) => b.ms - a.ms).slice(0, 12), loafCount: loaf.length, loafMs: loaf.reduce((s, x) => s + x.ms, 0) },
        events: ev,
        scroll: { wheelToFrame: { n: scrollInfo.wheel.length, p50: pctl(scrollInfo.wheel, 0.5), p95: pctl(scrollInfo.wheel, 0.95), max: scrollInfo.wheel.length ? Math.max(...scrollInfo.wheel) : 0 },
          blankFrames: scrollInfo.blank, checkedFrames: scrollInfo.checked, minCover: Math.round(scrollInfo.minCover * 100) / 100, loadingSeen: loadingSeen.slice(0, 10), loadingCount: loadingSeen.length },
        hubMsgs: Object.fromEntries(Object.entries(msgs).map(([k, v]) => [k, v.ms ? { n: v.n, p50: Math.round(pctl(v.ms, 0.5)), max: Math.round(Math.max(...v.ms)) } : { n: v.n, bytes: v.bytes, decodeMs: Math.round(v.decodeMs) }])),
        counts: { ...counts }, times: Object.fromEntries(Object.entries(times).map(([k, v]) => [k, { n: v.n, total: Math.round(v.total), max: Math.round(v.max) }])),
      };
    },
  };
  // last activity, for quiet()
  new MutationObserver(() => { W.lastDom = now(); }).observe(document, { subtree: true, childList: true, attributes: true, characterData: true });
})();
