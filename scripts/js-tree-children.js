// Server-only core/subthreads.bend Tree.children(s, id): the threads whose
// display parent is id, sorted by Tree.before. The pure def scans every
// thread for each id it is asked about, and the sidebar asks it for every
// row (O(threads²) a render: ~5 ms of a 10 ms sidebar on a real home,
// 2026-11-05). A state never changes once built, so the parent of every
// thread in it is worked out once per state (weakly, by identity), with the
// very Tree.display_parent and Thread.id the pure walk calls, kept in the
// order Tree.all gives; each id's children are that list sorted by the pure
// sort, as the pure def sorts its filtered walk of the same order.
const __bpTreeKids = new WeakMap();
function __bpTreeChildren(s, id, all, parent, tid, sort) {
  let ix = __bpTreeKids.get(s);
  if (ix === undefined) {
    const by = new Map();
    for (let x = all(s); x.$ === "Con"; x = x.tail) {
      const t = x.head, p = parent(s, tid(t));
      let b = by.get(p);
      if (b === undefined) by.set(p, b = []);
      b.push(t);
    }
    ix = { by, sorted: new Map() };
    __bpTreeKids.set(s, ix);
  }
  let r = ix.sorted.get(id);
  if (r === undefined) {
    const b = ix.by.get(id) || [];
    let xs = { $: "Nil" };
    for (let i = b.length - 1; i >= 0; i--) xs = { $: "Con", head: b[i], tail: xs };
    r = sort(xs);
    ix.sorted.set(id, r);
  }
  return r;
}
