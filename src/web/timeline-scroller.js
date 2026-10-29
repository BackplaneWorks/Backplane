// DOM effects for the pure Bend timeline planner. Rows remain opaque to this
// adapter; the planner owns layout, ranges, following and scroll anchoring.
export class TimelineScroller {
  constructor(el, { plan, create, patch, key, onChange = () => {} }) {
    this.el = el;
    this.callbacks = { plan, create, patch, key, onChange };
    this.rows = [];
    this.sizes = Object.create(null);
    this.lastPlan = null;
    this.lastMode = null;
    this.mounted = new Map();
    this._keys = [];
    this._byKey = new Map();
    this._follow = true;
    this._destroyed = false;
    this._attached = false;
    this._busy = false;
    this._pending = null;
    this._initialized = false;
    this.track = el.ownerDocument.createElement('div');
    this.track.className = 'timeline-virtual-track';
    Object.assign(this.track.style, { position: 'relative', width: '100%' });
    el.appendChild(this.track);
    this._onScroll = () => this._request('scroll');
  }

  get followEnabled() { return this._follow; }
  set followEnabled(follow) { this.configure({ follow }); }

  configure({ follow } = {}) {
    if (follow !== undefined && this._follow !== Boolean(follow)) {
      this._follow = Boolean(follow);
      this._request('update');
    }
    return this;
  }

  update(rows, { follow } = {}) {
    if (this._destroyed) return this;
    const snapshot = Array.from(rows);
    const keys = snapshot.map(this.callbacks.key);
    const byKey = new Map();
    for (let i = 0; i < keys.length; i++) {
      if (typeof keys[i] !== 'string' || byKey.has(keys[i])) {
        throw new Error('Timeline rows require unique stable string keys: ' + String(keys[i]));
      }
      byKey.set(keys[i], snapshot[i]);
    }
    const changed = snapshot.length !== this.rows.length ||
      snapshot.some((row, i) => row !== this.rows[i] || keys[i] !== this._keys[i]);
    const nextFollow = follow === undefined ? this._follow : Boolean(follow);
    const followChanged = nextFollow !== this._follow;
    this._follow = nextFollow;
    this.rows = snapshot;
    this._keys = keys;
    this._byKey = byKey;
    // A temporarily absent page must not erase measured geometry for its keys.
    // The cache belongs to this list lifetime and is released on destroy.
    // A completed stream is the exception: its next turn reuses the live key.
    for (const item of this.lastPlan?.items || []) {
      if (item.keepHeight && !byKey.has(item.key)) delete this.sizes[item.key];
    }
    if (changed || followChanged) this._request('update');
    return this;
  }

  // Host creation starts detached. Call after the root patch attaches it.
  refresh() {
    if (this._destroyed || !this.el.isConnected) return this;
    if (!this._attached) {
      this._attached = true;
      this.el.addEventListener('scroll', this._onScroll, { passive: true });
      const ResizeObserver = this.el.ownerDocument.defaultView.ResizeObserver;
      if (ResizeObserver) {
        this._observer = new ResizeObserver(() => {
          const changed = this._readSizes();
          if (changed || this.lastPlan?.view !== this.el.clientHeight) this._request('measure');
        });
        this._observer.observe(this.el);
        for (const { wrapper } of this.mounted.values()) this._observer.observe(wrapper);
      }
    }
    const changed = this._readSizes();
    if (!this._initialized || changed || this.lastPlan?.view !== this.el.clientHeight || this._pending) {
      this._request(this._initialized ? 'measure' : 'init', {}, true);
    }
    return this;
  }

  mount() { return this.refresh(); }

  _readSizes() {
    let changed = false;
    for (const [key, { wrapper }] of this.mounted) {
      // A row awaiting final cleanup may already have left the snapshot.
      // In particular, do not restore a completed stream's retired size.
      if (!this._byKey.has(key)) continue;
      const height = wrapper.getBoundingClientRect().height;
      if (this.sizes[key] !== height) {
        this.sizes[key] = height;
        changed = true;
      }
    }
    return changed;
  }

  _request(mode, extra = {}, preservePending = false) {
    if (this._destroyed) return;
    if (!preservePending || !this._pending) this._pending = { mode, ...extra };
    if (!this._attached || !this.el.isConnected || this._busy) return;
    this._busy = true;
    let changeMode = null;
    try {
      while (this._pending && !this._destroyed) {
        const request = this._pending;
        this._pending = null;
        // An explicit bookmark/search/bottom command also initializes a list.
        if (!this._initialized && !['restore', 'bottom', 'key'].includes(request.mode)) request.mode = 'init';
        // Report the triggering operation, not a subsequent measurement pass.
        changeMode ??= request.mode;
        const plan = this.callbacks.plan({
          keys: this._keys.slice(), sizes: { ...this.sizes },
          previous: this.lastPlan, top: this.el.scrollTop,
          view: this.el.clientHeight, follow: this._follow, ...request,
        });
        this.lastPlan = plan;
        this._initialized = true;
        this._render(plan);
        // Mount and measure before applying the scroll command. An estimated
        // track can be much shorter than the measured rows, especially during
        // streaming. Feeding that temporary browser clamp into readback used
        // to translate the entire track by hundreds of pixels.
        if (this._readSizes()) {
          this._pending = { mode: 'measure', top: plan.top };
          continue;
        }
        if (this.el.scrollTop !== plan.top) this.el.scrollTop = plan.top;
        // Browser scroll readback can quantize a fractional command. Bend
        // decides the track correction; the adapter only returns the reading.
        // Resize measurements are collected once the command has settled.
        if (this.el.scrollTop !== plan.top) {
          const readbackAttempt = (request.readbackAttempt || 0) + 1;
          if (readbackAttempt > 4) throw new Error('Timeline scroll readback did not settle');
          this._pending = { mode: 'readback', readbackAttempt };
        }
      }
      if (!this._destroyed) {
        // Intermediate measurement plans can briefly omit a row that the
        // settled range includes again. Keep its DOM through this synchronous
        // transaction and retire only rows outside the final range.
        this._prune(this.lastPlan);
        this.lastMode = changeMode;
        this.callbacks.onChange(this);
      }
    } finally {
      this._busy = false;
    }
    // A host callback can synchronously deliver a different immutable snapshot.
    if (this._pending) this._request(this._pending.mode, this._pending);
  }

  _prune(plan) {
    const wanted = new Set(plan.visible.map(item => item.key));
    for (const [key, row] of this.mounted) {
      if (!wanted.has(key)) {
        this._observer?.unobserve(row.wrapper);
        row.wrapper.remove();
        this.mounted.delete(key);
      }
    }
  }

  _render(plan) {
    for (let i = 0; i < plan.visible.length; i++) {
      const item = plan.visible[i], vnode = this._byKey.get(item.key);
      let row = this.mounted.get(item.key);
      if (!row) {
        const wrapper = this.el.ownerDocument.createElement('div');
        wrapper.className = 'timeline-virtual-row';
        Object.assign(wrapper.style, { position: 'absolute', top: '0', left: '0', width: '100%' });
        const node = this.callbacks.create(vnode);
        wrapper.appendChild(node);
        row = { wrapper, node, vnode };
        this.mounted.set(item.key, row);
        this._observer?.observe(wrapper);
      } else if (row.vnode !== vnode) {
        const node = this.callbacks.patch(row.node, vnode);
        if (node && node !== row.node) {
          row.node.replaceWith(node);
          row.node = node;
        }
        row.vnode = vnode;
      }
      row.wrapper.dataset.index = String(item.index);
      row.wrapper.dataset.virtualKey = item.key;
      // Bend marks bounded live previews. Their tail window can shrink while
      // the reply grows; retain measured space and keep the newest line at its
      // foot instead of repeatedly shrinking/growing the entire scroll track.
      row.wrapper.style.minHeight = item.keepHeight && this.sizes[item.key] !== undefined ? `${this.sizes[item.key]}px` : '';
      row.wrapper.style.justifyContent = item.keepHeight ? 'flex-end' : '';
      row.wrapper.style.transform = `translateY(${item.start}px)`;
      const at = this.track.childNodes[i];
      if (at !== row.wrapper) this.track.insertBefore(row.wrapper, at ?? null);
    }
    this.track.style.height = `${plan.physicalTotal ?? plan.total}px`;
    this.track.style.transform = `translateY(${plan.trackOffset ?? 0}px)`;
  }

  atEnd() { return this.lastPlan?.atEnd ?? true; }

  scrollToEnd() { this._request('bottom'); }

  scrollToKey(key, { align = 'start', offset = 0 } = {}) {
    if (this._destroyed || !this._byKey.has(key)) return false;
    this._request('key', { key, align, offset });
    return true;
  }

  position() {
    const p = this.lastPlan;
    return p ? { top: p.logicalTop ?? p.top, atEnd: p.atEnd, anchor: p.anchor, offset: p.offset }
      : { top: this.el.scrollTop, atEnd: true, anchor: null, offset: 0 };
  }

  restore(bookmark) { this._request('restore', { bookmark: { ...bookmark } }); }

  destroy() {
    if (this._destroyed) return;
    this._destroyed = true;
    this._observer?.disconnect();
    this.el.removeEventListener('scroll', this._onScroll);
    this.mounted.clear();
    this.track.remove();
    this._pending = null;
  }
}
