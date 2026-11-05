// run.mjs : measure interactions -> OUT/<id>.json (every rep) + OUT/summary.json
//   node run.mjs --kind web|desktop [--origin URL --token T | --cdp PORT] --state R [--cpu 4] [--reps 10]
//        [--only a,b] [--skip a,b] [--out DIR] [--label L] [--device iphone15]
import { open, sleep } from './driver.mjs';
import { INTERACTIONS, PHONE, STATES } from './interactions.mjs';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
const a = process.argv.slice(2);
const opt = (n, d) => { const i = a.indexOf('--' + n); return i >= 0 ? a[i + 1] : d; };
const kind = opt('kind', 'web'), state = opt('state', 'R'), cpu = Number(opt('cpu', '1')), reps = Number(opt('reps', '10'));
const only = opt('only', ''), skip = opt('skip', ''), out = opt('out', 'click-to-render-out'), device = opt('device', '');
mkdirSync(out, { recursive: true });
if (kind === 'web' && !opt('origin')) throw new Error('--origin http://127.0.0.1:PORT of a hub on a COPY (never the live home)');
if (/:(3787|57808)\b/.test(opt('origin', '')) || ['4787', '58808'].includes(opt('cdp', ''))) throw new Error('refusing the live hub / app ports');
const fx = opt('fixture', new URL(`./fixture-${state}.json`, import.meta.url).pathname);
const S = STATES[state] || JSON.parse(readFileSync(fx, 'utf8'));
const list = (device === 'iphone15' ? PHONE : INTERACTIONS).filter((x) => (!only || only.split(',').includes(x.id)) && !(skip && skip.split(',').includes(x.id)) && !(kind === 'desktop' && x.webOnly));
const pct = (xs, p) => { const s = xs.filter((x) => x != null).sort((x, y) => x - y); if (!s.length) return null; return s[Math.min(s.length - 1, Math.floor(p * (s.length - 1) + 0.5))]; };
const log = (...m) => console.error(`[${new Date().toISOString().slice(11, 19)}]`, ...m);
import { existsSync } from 'node:fs';
const prev = existsSync(`${out}/summary.json`) ? JSON.parse(readFileSync(`${out}/summary.json`, 'utf8')) : null; // re-runs of some interactions merge
const summary = { kind, state, cpu, reps, device, press: Number(opt('press', '0')), rtt: Number(opt('rtt', '0')), started: new Date().toISOString(), loadStart: (await import('node:os')).loadavg(), label: opt('label', ''), rows: prev ? prev.rows.filter((r) => !list.some((x) => x.id === r.id)) : [] };
let h = await open({ kind, origin: opt('origin'), token: opt('token'), cdp: opt('cdp'), device });
h.S = S; h.press = Number(opt('press', '0')); // ms between press and release (a person: ~80)
for (const it of list) {
  const runs = []; const st0 = {};
  try {
    await h.page.reload(); // (a goto to the same URL#hash would not reload: cold = really after a load)
    await h.throttle(1);
    await h.ready();
    await it.prep(h, S);
    await h.throttle(cpu);
    await h.idle(600);
    for (let i = 0; i <= reps; i++) {
      if (cpu > 1) await h.throttle(1);
      const st = await it.step(h, S, i); // setup unthrottled; only the measured part runs at --cpu
      if (!st.cpred) { st.cpred = st.pred; if (!st.cpredSame) st0.same = true; } st0.cpred = true; // no separate content: content = the frame's predicate
      if (cpu > 1) { await h.throttle(cpu); await h.idle(300); }
      const r = await h.measure(it.id, { pred: st.pred, go: st.go, quiet: st.quiet || 300, cap: st.cap || 6000, sample: st.sample, content: st.content, confirm: st.confirm, cpred: st.cpred, csig: st.csig, timeout: st.timeout || 15000 });
      r.rep = i; r.cold = i === 0; runs.push(r);
      if (r.error) log(it.id, i, 'ERROR', r.error);
      if (st.undo) { await h.throttle(1); await st.undo(h); await h.throttle(cpu); }
      await h.idle(st.settleAfter || 400);
    }
  } catch (e) { log(it.id, 'FAILED', String(e).slice(0, 300)); runs.push({ error: String(e).slice(0, 300) }); }
  writeFileSync(`${out}/${it.id}.json`, JSON.stringify(runs, null, 1));
  const warm = runs.filter((r) => !r.cold && !r.error && r.paint != null), cold = runs.find((r) => r.cold && !r.error);
  const m = (k, p = 0.5) => pct(warm.map((r) => r[k]), p);
  const row = { id: it.id, what: it.what, n: warm.length, errors: runs.filter((r) => r.error).length,
    cnt50: m('cnt'), cnt95: m('cnt', 0.95), cnt1_50: m('cnt1'), nC50: m('nC'), cntMissing: warm.filter((r) => st0.cpred && r.cnt == null).length, cntPre: warm.filter((r) => r.cPre).length, contentIsFrame: !!st0.same, cHub: warm.filter((r) => (r.cRecv || 0) > 0).length, cHubLast50: m('cRecvLast'),
    paint50: m('paint'), paint95: m('paint', 0.95), px50: m('px'), px95: m('px', 0.95), content50: m('content'), content95: m('content', 0.95), confirm50: m('confirm'), confirm95: m('confirm', 0.95), dom50: m('dom'), settle50: m('settle'), settle95: m('settle', 0.95),
    cold: cold?.paint ?? null, coldSettle: cold?.settle ?? null,
    send50: m('send'), recv50: m('recv1'), recvN50: m('recvN'), nRecv50: m('nRecv'), recvKB50: Math.round((m('recvBytes') || 0) / 1024),
    shift50: m('shift'), shiftCls50: m('shiftCls'), shiftClsMax: Math.max(0, ...warm.map((r) => r.shiftCls || 0)), shiftMax: Math.max(0, ...warm.map((r) => r.shift || 0)), late50: m('nLate'), lateMax50: m('lateMax'),
    proc50: pct(warm.map((r) => Math.max(0, ...(r.events || []).map((e) => e.proc))), 0.5),
    loaf50: pct(warm.map((r) => (r.loafs || []).filter((l) => l.t < (r.paint || 0)).reduce((s, l) => s + l.dur, 0)), 0.5),
    local: warm.filter((r) => r.send == null || r.send > r.dom).length,
    roundTrip: warm.filter((r) => r.recv1 != null && r.recv1 <= r.dom).length,
    hubWait50: pct(warm.map((r) => r.recvN != null && r.send != null && r.recvN <= r.dom ? r.recvN - r.send : 0), 0.5) };
  // late after the press frame: what appeared after the frame predicate's paint, grouped over the warm reps
  const lg = new Map();
  for (const r of warm) { const seen = new Set(); for (const l of r.late || []) { const k = l.el + ' "' + (l.text || '').replace(/\d+/g, '#').slice(0, 40) + '"'; if (seen.has(k)) continue; seen.add(k); const e = lg.get(k) || { k, n: 0, dts: [] }; e.n++; e.dts.push(l.dt); lg.set(k, e); } }
  row.lateTop = [...lg.values()].filter((e) => e.n * 2 >= warm.length).map((e) => ({ what: e.k, reps: e.n, dt50: pct(e.dts, 0.5) })).sort((x, y) => x.dt50 - y.dt50).slice(0, 14);
  summary.rows = summary.rows.filter((r) => r.id !== row.id); summary.rows.push(row);
  log(it.id.padEnd(16), `px p50 ${row.px50} CONTENT p50 ${row.cnt50} p95 ${row.cnt95} (missing ${row.cntMissing}) | paint p50 ${row.paint50} p95 ${row.paint95} | dom ${row.dom50} | settle ${row.settle50} | cold ${row.cold} | send ${row.send50} recv ${row.recv50}..${row.recvN50} (${row.nRecv50} msg ${row.recvKB50} KB) | shift ${row.shift50} late ${row.late50} | err ${row.errors}`);
  writeFileSync(`${out}/summary.json`, JSON.stringify(summary, null, 1));
}
summary.errorsPage = h.errors; summary.loadEnd = (await import('node:os')).loadavg();
writeFileSync(`${out}/summary.json`, JSON.stringify(summary, null, 1));
await h.close();
