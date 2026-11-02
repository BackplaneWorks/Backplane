// A thread's Subagents rows shown or hidden at the click, before the hub's
// view comes back (presentation.bend's Node.subs: the hub always sends the
// rows, hidden while shut). Pending visibility only: the click still goes
// once through the ordered path, and the hub keeps the choice.
export class SubsFeedback {
  constructor(changed) { this.changed = changed;this.revision = 0;this.pending = null; }
  choice(action, value) {
    if (action !== 'subs-open' && action !== 'subs-shut') return undefined;
    const tab = String(value).indexOf('\t');
    const thread = tab < 0 ? String(value) : String(value).slice(0, tab);
    const revision = ++this.revision;
    this.pending = { thread, open: action === 'subs-open' };
    this.changed();
    // the hub's answer is the picture from here on
    return () => { if (revision === this.revision) { this.pending = null;this.changed(); } };
  }
  project(session) {
    const p = this.pending;
    return p && session?.sel === p.thread ? { ...session, subsFold: p.open ? '1' : '0' } : session;
  }
  clear() { this.revision++;this.pending = null; }
}
