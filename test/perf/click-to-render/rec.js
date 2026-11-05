// rec.js : in-page click-to-render recorder, installed before any page script runs
// (Playwright addInitScript / CDP Page.addScriptToEvaluateOnNewDocument).
//
// One measurement = arm(predicateSource) -> the driver injects real input over CDP ->
//   t0     the first trusted input event's timeStamp (pointerdown / keydown / wheel): when the
//          browser received it, so main-thread queueing is included
//   tSend  first WebSocket send after t0          (client handler before the request leaves)
//   tRecv  first / last WebSocket message after t0 and before the predicate turned true
//   tDom   the predicate (a JS expression over the DOM) first true, checked on every DOM
//          mutation batch and every animation frame
//   tRaf   the next animation frame's callback after tDom
//   tPaint a task posted from that rAF: runs after the frame's style/layout/paint was produced
//          ("input-to-pixels" = tPaint - t0; excludes the GPU/compositor's few ms)
//   settle the last DOM mutation / layout shift, once quiet for 300 ms (cap 6 s)
// plus Event Timing entries (processing = handler time, duration = input -> next paint of the
// sync frame), long tasks, long animation frames with script attribution, layout-shift entries
// with the elements that moved, and "late" additions: visible text / elements added after tPaint.
(() => {
  if (window.__c2r) return;
  window.__bpPerf = { trace: [], render: [], part: [], live: [], act: [], msg: [] };
  // host.js's render-phase counters (globalThis.__bpRender): summed per measurement window
  const RT = {}, RN = {};
  window.__bpRender = { t(k, ms) { if (window.__c2r?.armed && window.__c2r.t0 != null && window.__c2r.tPaint == null) RT[k] = (RT[k] || 0) + ms; }, n(k) { if (window.__c2r?.armed && window.__c2r.t0 != null && window.__c2r.tPaint == null) RN[k] = (RN[k] || 0) + 1; } };
  const now = () => performance.now();
  const R = window.__c2r = { armed: false };
  const reset = () => Object.assign(R, {
    armed: false, t0: null, inputType: null, pred: null, tDom: null, tRaf: null, tPaint: null,
    lastMut: null, nMut: 0, nMutBefore: 0, sends: [], recvs: [], shifts: [], events: [], longs: [], loafs: [],
    late: [], lateNodes: [], done: false, sample: null, film: [], contentSel: null, lastContent: null, settled: null, tArm: null, error: null, hitHow: null, perfMark: 0,
  });
  reset();
  const desc = (n) => {
    if (!n) return '?';
    if (n.nodeType === 3) n = n.parentElement;
    if (!n || !n.tagName) return '?';
    let s = n.tagName.toLowerCase();
    if (n.id) s += '#' + n.id;
    if (n.classList && n.classList.length) s += '.' + [...n.classList].slice(0, 3).join('.');
    return s;
  };
  const check = (how) => {
    if (!R.armed || R.tDom != null || R.t0 == null) return;
    let ok = false;
    try { ok = !!R.pred(); } catch (e) { ok = false; }
    if (!ok) return;
    R.tDom = now(); R.hitHow = how;
    requestAnimationFrame(() => {
      R.tRaf = now();
      const ch = new MessageChannel();
      ch.port1.onmessage = () => { R.tPaint = now(); R.lastMut = Math.max(R.lastMut || 0, R.tPaint); };
      ch.port2.postMessage(0);
    });
  };
  // input timestamp: the first trusted event of the gesture
  for (const t of ['pointerdown', 'mousedown', 'keydown', 'wheel', 'click'])
    addEventListener(t, (e) => {
      if (R.armed && R.t0 == null && e.isTrusted) { R.t0 = e.timeStamp; R.inputType = t; queueMicrotask(() => check('input')); }
    }, true);
  // sockets: time every send / receive
  const WS = window.WebSocket;
  if (WS) {
    const send = WS.prototype.send;
    WS.prototype.send = function (d) {
      if (R.armed && R.t0 != null) R.sends.push([now(), d?.byteLength ?? d?.length ?? d?.size ?? 0]);
      return send.call(this, d);
    };
    window.WebSocket = class extends WS {
      constructor(...a) {
        super(...a);
        this.addEventListener('message', (e) => {
          if (R.armed && R.t0 != null && R.tDom == null) R.recvs.push([now(), e.data?.byteLength ?? e.data?.length ?? e.data?.size ?? 0]);
        });
      }
    };
  }
  const startMO = () => {
    const mo = new MutationObserver((recs) => {
      if (!R.armed) return;
      const t = now();
      if (R.t0 == null) { R.nMutBefore += recs.length; return; }
      R.nMut += recs.length; R.lastMut = t;
      if (R.contentSel) for (const r of recs) { const el = r.target.nodeType === 1 ? r.target : r.target.parentElement; if (el?.closest?.(R.contentSel)) { R.lastContent = t; break; } }
      if (R.tDom == null) { check('mutation'); return; }
      if (R.tPaint != null && R.lateNodes.length < 400) {
        for (const r of recs) {
          if (r.type === 'childList') for (const n of r.addedNodes) {
            if (n.nodeType === 1 || (n.nodeType === 3 && n.data.trim())) R.lateNodes.push([t - R.t0, n, 'add']);
          } else if (r.type === 'characterData' && r.target.data.trim()) R.lateNodes.push([t - R.t0, r.target, 'text']);
        }
      }
    });
    mo.observe(document, { subtree: true, childList: true, attributes: true, characterData: true });
  };
  if (document.documentElement) startMO(); else addEventListener('DOMContentLoaded', startMO);
  // canvases inside the content area: a 16x16 downscale per frame; a change = content still arriving (plots, images)
  let probe = null, lastSig = '';
  const canvasSig = () => {
    const box = document.querySelector(R.contentSel); if (!box) return '';
    const cs = box.querySelectorAll('canvas'); if (!cs.length) return '';
    probe = probe || Object.assign(document.createElement('canvas'), { width: 16, height: 16 });
    const g = probe.getContext('2d', { willReadFrequently: true }); let sig = '';
    for (const c of [...cs].slice(0, 3)) {
      if (!c.width || !c.height) continue;
      try { g.clearRect(0, 0, 16, 16); g.drawImage(c, 0, 0, 16, 16); const d = g.getImageData(0, 0, 16, 16).data; let hsh = 0; for (let i = 0; i < d.length; i += 4) hsh = (hsh * 31 + d[i] + d[i + 1] * 3 + d[i + 2] * 7) | 0; sig += hsh + ','; } catch { sig += 'x,'; }
    }
    return sig;
  };
  const frame = () => {
    check('frame');
    if (R.armed && R.t0 != null && R.contentSel && R.tDom != null) { const sg = canvasSig(); if (sg !== lastSig) { lastSig = sg; R.lastContent = R.lastMut = now(); } }
    if (R.armed && R.t0 != null && R.sample && R.film.length < 300) {
      let v; try { v = JSON.stringify(R.sample()); } catch (e) { v = 'ERR ' + e.message; }
      if (!R.film.length || R.film[R.film.length - 1][1] !== v) R.film.push([+(now() - R.t0).toFixed(1), v]);
    }
    requestAnimationFrame(frame);
  };
  requestAnimationFrame(frame);
  const po = (type, f, extra = {}) => { try { new PerformanceObserver((l) => { if (R.armed && R.t0 != null) for (const e of l.getEntries()) f(e); }).observe({ type, ...extra }); } catch {} };
  po('layout-shift', (e) => {
    if (e.startTime < R.t0) return;
    R.lastMut = Math.max(R.lastMut || 0, now());
    R.shifts.push({ t: e.startTime - R.t0, v: e.value, src: (e.sources || []).slice(0, 5).map((s) => ({ n: desc(s.node), from: [s.previousRect.x, s.previousRect.y, s.previousRect.width, s.previousRect.height].map(Math.round), to: [s.currentRect.x, s.currentRect.y, s.currentRect.width, s.currentRect.height].map(Math.round) })) });
  });
  po('event', (e) => {
    if (e.startTime < R.t0 - 1) return;
    R.events.push({ name: e.name, start: e.startTime - R.t0, proc: e.processingEnd - e.processingStart, wait: e.processingStart - e.startTime, dur: e.duration });
  }, { durationThreshold: 16 });
  po('longtask', (e) => { if (e.startTime + e.duration >= R.t0) R.longs.push([e.startTime - R.t0, e.duration]); });
  po('long-animation-frame', (e) => {
    if (e.startTime + e.duration < R.t0) return;
    R.loafs.push({ t: e.startTime - R.t0, dur: e.duration, block: e.blockingDuration, render: e.renderStart ? e.startTime + e.duration - e.renderStart : 0, style: e.styleAndLayoutStart ? e.startTime + e.duration - e.styleAndLayoutStart : 0,
      scripts: (e.scripts || []).slice(0, 4).map((s) => ({ dur: Math.round(s.duration), inv: s.invoker, fn: s.sourceFunctionName, src: (s.sourceURL || '').split('/').pop() + ':' + s.sourceCharPosition, fsl: Math.round(s.forcedStyleAndLayoutDuration || 0) })) });
  });
  R.arm = (src, sample, contentSel) => {
    reset(); for (const k in RT) delete RT[k]; for (const k in RN) delete RN[k];
    if (sample) R.sample = new Function('return (' + sample + ')');
    R.contentSel = contentSel || null; lastSig = '';
    R.pred = new Function('return (' + src + ')');
    let pre = false; try { pre = !!R.pred(); } catch {}
    if (pre) { R.error = 'predicate already true before input'; return false; }
    R.perfMark = window.__bpPerf.trace.length;
    R.tArm = now(); R.armed = true; return true;
  };
  // after the driver saw tPaint: wait for 300 ms of quiet (cap 6 s), then summarise
  R.finish = (quietMs = 300, capMs = 6000) => new Promise((res) => {
    const tick = () => {
      const t = now();
      const last = Math.max(R.lastMut || 0, R.tPaint || 0);
      if (t - last >= quietMs || t - R.t0 >= capMs) {
        R.armed = false;
        R.settled = { at: last - R.t0, capped: t - R.t0 >= capMs };
        const vw = innerWidth, vh = innerHeight;
        const late = [];
        for (const [dt, n, kind] of R.lateNodes) {
          const el = n.nodeType === 3 ? n.parentElement : n;
          if (!el || !el.isConnected) continue;
          const r = el.getBoundingClientRect();
          if (r.width < 1 || r.height < 1 || r.bottom < 0 || r.top > vh || r.right < 0 || r.left > vw) continue;
          const text = (n.nodeType === 3 ? n.data : (el.innerText || '')).trim().replace(/\s+/g, ' ').slice(0, 60);
          const media = el.matches?.('img,canvas,svg,video,iframe') || !!el.querySelector?.('img,canvas,svg,iframe');
          if (!text && !media) continue;
          late.push({ dt: Math.round(dt), kind, el: desc(el), text });
        }
        R.late = late;
        const tr = window.__bpPerf.trace.slice(R.perfMark).filter((e) => e.ms >= R.t0 - 1).map((e) => ({ ...e, ms: +(e.ms - R.t0).toFixed(1), epochMs: undefined }));
        const rel = (x) => x == null ? null : +(x - R.t0).toFixed(1);
        res({
          inputType: R.inputType, hitHow: R.hitHow, queue: rel(R.tArm) == null ? null : null,
          dom: rel(R.tDom), raf: rel(R.tRaf), paint: rel(R.tPaint),
          content: R.contentSel ? +((Math.max(R.lastContent || 0, R.tPaint) - R.t0).toFixed(1)) : null, settle: +R.settled.at.toFixed(1), capped: R.settled.capped,
          send: R.sends.length ? rel(R.sends[0][0]) : null, nSend: R.sends.length, sendBytes: R.sends.reduce((a, s) => a + s[1], 0),
          recv1: R.recvs.length ? rel(R.recvs[0][0]) : null, recvN: R.recvs.length ? rel(R.recvs[R.recvs.length - 1][0]) : null, nRecv: R.recvs.length, recvBytes: R.recvs.reduce((a, s) => a + s[1], 0),
          nMut: R.nMut, nMutBefore: R.nMutBefore,
          shift: +R.shifts.reduce((a, s) => a + s.v, 0).toFixed(4), shifts: R.shifts.slice(0, 8),
          events: R.events.slice(0, 8), longs: R.longs, loafs: R.loafs.slice(0, 6),
          late: late.slice(0, 20), nLate: late.length, lateMax: late.reduce((a, l) => Math.max(a, l.dt), 0),
          trace: tr.slice(0, 40), film: R.film, rr: Object.fromEntries(Object.entries(RT).map(([k, v]) => [k, +v.toFixed(1)])), rn: { ...RN },
        });
      } else setTimeout(tick, 50);
    };
    tick();
  });
})();
