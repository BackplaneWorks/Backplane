// A thread's Subagents rows shown or hidden at the click, before the hub's
// view comes back (presentation.bend's Node.subs: the hub always sends the
// rows, hidden while shut). Pending visibility only: the click still goes
// once through the ordered path, and the hub keeps the choice.
export class SubsFeedback {
  constructor(changed) { this.changed = changed;this.revision = 0;this.pending = null;this.rows = new Map(); }
  choice(action, value) {
    if (action === 'fold') return this.row(String(value));
    if (action !== 'subs-open' && action !== 'subs-shut') return undefined;
    const tab = String(value).indexOf('\t');
    const thread = tab < 0 ? String(value) : String(value).slice(0, tab);
    const revision = ++this.revision;
    // (drawn by the caller at once: host.js's subsNow; a whole page in the
    // same frame would hold that frame back)
    this.pending = { thread, open: action === 'subs-open' };
    // the hub's answer is the picture from here on
    return () => { if (revision === this.revision) { this.pending = null;this.changed(); } };
  }
  // A subagent's row ("fold" with fold.bend's Fold.ask value: "1sub:<call>"
  // opens, "0sub:<call>" shuts): its steps are drawn shut too, so the row
  // shows them at once (presentation.bend's Node.rows). Other folds (tool
  // runs, the finished group) wait for the hub as before.
  row(value) {
    if (!/^[01]sub:./.test(value)) return undefined;
    const key = value.slice(5), revision = ++this.revision;
    this.rows.set(key, { open: value[0] === '1', revision });
    return () => { if (this.rows.get(key)?.revision === revision) { this.rows.delete(key);this.changed(); } };
  }
  project(session) {
    const p = this.pending;
    let out = p && session?.sel === p.thread ? { ...session, subsFold: p.open ? '1' : '0' } : session;
    if (this.rows.size) out = { ...out, subsRows: [...this.rows].map(([key, r]) => (r.open ? '1' : '0') + key).join('\n') };
    return out;
  }
  clear() { this.revision++;this.pending = null;this.rows.clear(); }
}
