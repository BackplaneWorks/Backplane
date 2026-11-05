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
// answers an unchanged read with the very same rows)
const __bpMemo1Tabs = new Map();
function __bpMemo1(tab, a, f) {
  let w = __bpMemo1Tabs.get(tab);
  if (w === undefined) __bpMemo1Tabs.set(tab, w = new WeakMap());
  let r = w.get(a);
  if (r === undefined) {
    r = f(a);
    w.set(a, r);
  }
  return r;
}
