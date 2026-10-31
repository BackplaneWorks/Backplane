// Pixel coordinates belong to this browser. Bend names the thread, board,
// comparison and report; a drawing detour changes none of those identities.
export class ReviewPosition {
  constructor(storage) { this.storage = storage;this.current = null;this.pending = new Map(); }
  read(thread) {
    try { const value = JSON.parse(this.storage.getItem('backplane-review:' + thread) || '{}');return value && !Array.isArray(value) && typeof value === 'object' ? value : {}; } catch { return {}; }
  }
  write(thread, positions) {
    try { this.storage.setItem('backplane-review:' + thread, JSON.stringify(positions)); } catch {}
  }
  capture() {
    const c = this.current;
    if (!c || !c.ready || !c.el.isConnected) return;
    if (c.top === c.el.scrollTop && c.left === c.el.scrollLeft) return;
    c.top = c.el.scrollTop;c.left = c.el.scrollLeft;
    const positions = this.read(c.thread);
    positions[c.place] = [c.el.scrollTop, c.el.scrollLeft];
    const keys = Object.keys(positions);
    for (const key of keys.slice(0, Math.max(0, keys.length - 64))) delete positions[key];
    this.write(c.thread, positions);
  }
  fork(source) { this.capture();this.pending.set(source, this.read(source)); }
  accepted(session) {
    const source = session.drafts?.['@viewer.fork:' + session.sel];
    if (!source) return;
    let existing = null;
    try { existing = this.storage.getItem('backplane-review:' + session.sel); } catch {}
    if (existing === null) this.write(session.sel, this.pending.get(source) || this.read(source));
    this.pending.delete(source);
  }
  mount(el) {
    const thread = el?.dataset.reviewThread, place = el?.dataset.reviewPlace;
    if (!thread || !place) { this.current = null;return; }
    const ready = el.dataset.reviewReady === '1';
    if (this.current?.el === el && this.current.thread === thread && this.current.place === place && this.current.ready === ready) return;
    this.current = { el, thread, place, ready };
    if (!ready) return;
    const saved = this.read(thread)[place];
    if (!Array.isArray(saved) || saved.length !== 2 || !saved.every(n => Number.isFinite(n) && n >= 0)) return;
    el.scrollTop = Math.min(saved[0], Math.max(0, el.scrollHeight - el.clientHeight));
    el.scrollLeft = Math.min(saved[1], Math.max(0, el.scrollWidth - el.clientWidth));
  }
}
