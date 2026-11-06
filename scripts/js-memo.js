// Server-only identity memos for pure defs (scripts/js-native.py's MEMO1
// and MEMO2 lists). Bend values never change once built, so a pure def asked
// again about the very same first argument (by identity, weakly) and an equal
// second argument (a String) answers what it answered: the answer is kept,
// never recomputed or changed. Used where one render asks the same question
// in two places (Subs.aggs: the top bar's subagent count and the subagents
// panel, ~4.5 ms each on a thread with many subagents, 2026-11-05).
const __bpMemoTabs = new Map();
function __bpMemo2(tab, a, b, f) {
  let w = __bpMemoTabs.get(tab);
  if (w === undefined) __bpMemoTabs.set(tab, w = new WeakMap());
  let m = w.get(a);
  if (m === undefined) w.set(a, m = new Map());
  if (m.has(b)) return m.get(b);
  const r = f(a, b);
  m.set(b, r);
  return r;
}

// one argument, by identity (SubHistory.own: a read's rows as a state; db.js
// answers an unchanged read with the very same rows; Slot.sidebar / Slot.count:
// the region of the very same rendered page, each a walk of the whole page)
const __bpMemo1Tabs = new Map();
function __bpMemo1(tab, a, f) {
  if (a === null || typeof a !== "object") return f(a);
  let w = __bpMemo1Tabs.get(tab);
  if (w === undefined) __bpMemo1Tabs.set(tab, w = new WeakMap());
  let r = w.get(a);
  if (r === undefined) {
    r = f(a);
    w.set(a, r);
  }
  return r;
}

// two objects by identity (weakly) and two equal Strings or numbers
// (Subs.rows(st, info, thread, now): the top bar's subagent count and the
// subagents panel ask it with the very same arguments in one render, ~7.7 ms
// each on the huge state's long thread, 2026-11-05). Anything else (an
// argument that is no object) is asked as before.
const __bpMemo4Tabs = new Map();
function __bpMemo4(tab, a, b, c, d, f) {
  if (a === null || typeof a !== "object" || b === null || typeof b !== "object") return f(a, b, c, d);
  let w = __bpMemo4Tabs.get(tab);
  if (w === undefined) __bpMemo4Tabs.set(tab, w = new WeakMap());
  let wb = w.get(a);
  if (wb === undefined) w.set(a, wb = new WeakMap());
  let m = wb.get(b);
  if (m === undefined) wb.set(b, m = new Map());
  const key = typeof c + ":" + String(c) + "\0" + typeof d + ":" + String(d);
  if (m.has(key)) return m.get(key);
  const r = f(a, b, c, d);
  m.set(key, r);
  return r;
}
