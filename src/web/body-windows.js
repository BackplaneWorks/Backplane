import { T, is } from './wire.js';

const list = xs => xs.reduceRight((tail, head) => ({ $: 'Con', head, tail }), { $: 'Nil' });
const items = xs => { const out = [];for (; xs && is(xs, 'Con'); xs = xs.tail) out.push(xs.head);return out; };
const attr = (n, key) => items(n.attrs).find(a => is(a, 'At') && a.name === key)?.value;
const text = value => ({ $: T.Txt, text: value });
const node = (tag, key, attrs, kids) => ({ $: T.El, tag, key,
  attrs: list(Object.entries(attrs).map(([name, value]) => ({ $: T.At, name, value: String(value) }))), kids: list(kids) });
const bytes = s => new TextEncoder().encode(s).length;
const metadata = p => ({id:p.id,thread:p.thread,version:p.version,cursor:p.cursor,bodyPage:1,automatic:p.automatic, streamGeneration:p.streamGeneration, streamCut:p.streamCut, streamBlocks:p.streamBlocks});

// Read-only body windows, independent of action/session authority. Six small
// pages per body and four bodies per parked scope; at most eight scopes and
// 1MiB of serialized UTF-8 page envelopes; other metadata has fixed count/field bounds. There are no hidden mounted conversation trees.
export class BodyWindows {
  constructor({ request, changed, renderBody, maxBytes = 1024 * 1024 } = {}) {
    this.request = request;this.changed = changed;this.renderBody = renderBody;this.maxBytes = maxBytes;
    this.scopes = new Map();this.scope = '';this.fence = '';this.parts = new Map();this.pending = null;
  }
  bind(hub, detail, owner, currentNode, holding = false) {
    const scope = JSON.stringify([hub, owner, detail.epoch, detail.target]);
    const fence = JSON.stringify([scope, detail.generation, detail.viewId]);
    const changedFence = fence !== this.fence;
    if (changedFence) this.pending = null;
    if (scope !== this.scope) this.parts.clear();
    this.scope = scope;this.fence = fence;this.generation = detail.generation;this.viewId = detail.viewId;this.owner = owner;
    if (!this.scopes.has(scope)) this.scopes.set(scope, new Map());
    const saved = this.scopes.get(scope);this.scopes.delete(scope);this.scopes.set(scope, saved);
    // A new authoritative connection/view may retry once. Same-fence failures
    // remain parked; stale callbacks cannot poison the new request.
    if (changedFence) for (const r of saved.values()) r.error = "";
    for (const p of detail.window.partial) if (p.bodyPage === 1) {
      this.parts.delete(p.id);this.parts.set(p.id, metadata(p));
      const retained = saved.get(p.id);
      if (retained?.version !== p.version) {
        const sameStream = holding && !changedFence && p.id.startsWith('@live:') && retained?.pages.length
          && retained.streamGeneration === p.streamGeneration && typeof p.streamGeneration === 'string'
          && p.streamCut === false && retained.streamCut === false && p.streamBlocks >= retained.streamBlocks;
        if (sameStream) {
          retained.version = p.version;retained.error = '';
          for (const page of retained.pages) {
            page.cursor = this.liveCursor(p, page.cursor);page.previous = this.liveCursor(p, page.previous);
            page.bytes = bytes(JSON.stringify(Object.fromEntries(Object.entries(page).filter(([key]) => key !== 'bytes' && key !== 'rendered')))) + 64;
          }
          retained.refreshTail ||= retained.pages.at(-1)?.complete === true;
          retained.streamBlocks = p.streamBlocks;
        } else saved.delete(p.id);
      }
      if (this.pending?.id === p.id && this.pending.version !== p.version) this.pending = null;
    }
    const issued = new Set(detail.window.partial.filter(p => p.bodyPage === 1).map(p => p.id));
    const currentMessages = new Set();
    const collect = n => { if (!is(n, 'El')) return;
      if (n.tag === 'bp-memo' && ['msg','row','live'].includes(attr(n, 'fn'))) currentMessages.add(n.key);
      else if ((attr(n, 'class') || '').split(/\s+/).some(c => c === 'msg' || c === 'act')) currentMessages.add(attr(n, 'data-id'));
      for (const child of items(n.kids)) collect(child);
    };
    if (currentNode) collect(currentNode);
    for (const id of currentMessages) if (!issued.has(id)) { this.parts.delete(id);saved.delete(id); }
    while (this.parts.size > 128) this.parts.delete(this.parts.keys().next().value);
    this.trim();
  }
  // Prefetch metadata is read-only and can never establish a new scope or
  // picture. It may describe rows in the bounded adjacent reading bridge.
  offer(hub, detail, owner) {
    const scope = JSON.stringify([hub, owner, detail.epoch, detail.target]);
    if (scope !== this.scope || JSON.stringify([scope, detail.generation, detail.viewId]) !== this.fence) return false;
    for (const p of detail.window.partial) if (p.bodyPage === 1) {
      // Lookahead cannot replace the stream incarnation already painted.
      if (p.id.startsWith('@live:') && this.parts.has(p.id)) continue;
      this.parts.delete(p.id);this.parts.set(p.id, metadata(p));
      const saved = this.scopes.get(scope);
      if (saved?.get(p.id)?.version !== p.version) saved?.delete(p.id);
    }
    while (this.parts.size > 128) this.parts.delete(this.parts.keys().next().value);
    return true;
  }
  liveCursor(part, oldCursor, start) {
    if (!oldCursor && start === undefined) return '';
    try {
      const fresh = JSON.parse(part.cursor), old = oldCursor ? JSON.parse(oldCursor) : fresh;
      if (!/^body:[1-9][0-9]*:0$/.test(fresh.query) || !/^body:[1-9][0-9]*:[0-9]+$/.test(old.query)
        || fresh.epoch !== old.epoch || JSON.stringify(fresh.target) !== JSON.stringify(old.target)) return '';
      fresh.query = fresh.query.slice(0, fresh.query.lastIndexOf(':') + 1) + (start === undefined ? old.query.split(':')[2] : '0');
      fresh.before = start === undefined ? old.before : String(start);
      return JSON.stringify(fresh);
    } catch { return ''; }
  }
  disconnect() { this.pending = null;this.fence = ''; }
  clear() { this.disconnect();this.parts.clear();this.scope = '';this.scopes.clear(); }
  reader(id, create = false) {
    const saved = this.scopes.get(this.scope), part = this.parts.get(id);
    if (!saved || !part) return null;
    if (!saved.has(id) && create) {
      if (saved.size >= 4) saved.delete(saved.keys().next().value);
      saved.set(id, { version: part.version, pages: [], heights: new Map(), before: 0, after: 0, error: '', explicit: false, streamGeneration:part.streamGeneration, streamCut:part.streamCut, streamBlocks:part.streamBlocks });
    }
    return saved.get(id) || null;
  }
  open(id) {
    const part = this.parts.get(id), r = this.reader(id, true);
    if (!part || !r) return false;
    r.explicit = !r.explicit;if (r.explicit) r.error = "";this.changed?.();return true;
  }
  retainedBytes() {
    let used = 0;for (const saved of this.scopes.values()) for (const r of saved.values()) for (const p of r.pages) used += p.bytes;
    return used;
  }
  trim(protectedId = "") {
    while (this.scopes.size > 8 || this.retainedBytes() > this.maxBytes) {
      const oldest = [...this.scopes.keys()].find(k => k !== this.scope);
      if (oldest !== undefined) this.scopes.delete(oldest);
      else {
        const saved = this.scopes.get(this.scope);
        if (!saved?.size) break;
        const victim = [...saved.keys()].find(id => id !== protectedId);
        if (victim === undefined) break;
        saved.delete(victim);
      }
    }
  }
  accept(message, input) {
    const pending = this.pending;
    if (!pending || pending.fence !== this.fence || pending.id !== message.id || pending.cursor !== input.cursor
      || pending.version !== message.version || pending.thread !== message.thread || input.bodyVersion !== message.version
      || input.generation !== pending.generation || input.viewId !== pending.viewId || input.accountBinding !== pending.owner) return false;
    this.pending = null;
    const r = this.reader(message.id, true);
    if (!r || r.version !== message.version) return false;
    const first = r.pages[0], last = r.pages.at(-1), previous = pending.direction === 'previous', replacing = pending.direction === 'replace';
    if (first && (replacing ? message.start !== last.start || !message.text.startsWith(last.text) : previous ? message.end !== first.start : message.start !== last.end)) return false;
    const page = { ...message, bytes: bytes(JSON.stringify(message)) + 64 };
    r.pages = replacing ? [...r.pages.slice(0,-1),page] : previous ? [page, ...r.pages] : [...r.pages, page];
    if (replacing) r.refreshTail = false;
    const remembered = r.heights.get(message.start) || 0;
    if (previous) r.before = Math.max(0, r.before - remembered);
    else r.after = Math.max(0, r.after - remembered);
    while (r.pages.length > 6) {
      const drop = previous ? r.pages.pop() : r.pages.shift();
      const height = r.heights.get(drop.start) || Math.max(24, drop.text.split('\n').length * 20);
      if (previous) r.after += height;else r.before += height;
    }
    r.error = '';this.trim(message.id);this.changed?.();return true;
  }
  failed(input, reason) {
    if (!this.pending || this.pending.fence !== this.fence || this.pending.id !== input.entryId
      || this.pending.cursor !== input.cursor || this.pending.version !== input.bodyVersion) return;
    if (input.generation !== this.pending.generation || input.viewId !== this.pending.viewId || input.accountBinding !== this.pending.owner) return;
    const r = this.reader(input.entryId);this.pending = null;
    if (r && reason !== 'overtaken') r.error = reason;
    this.changed?.();
  }
  project(root) {
    const walk = n => {
      if (!is(n, 'El')) return n;
      const memo = n.tag === 'bp-memo' && attr(n, 'fn') === 'msg';
      const activity = (attr(n, 'class') || '').split(/\s+/).includes('act');
      const id = memo ? n.key : attr(n, 'data-id'), part = this.parts.get(id);
      if (part && (memo || (attr(n, 'class') || '').split(/\s+/).includes('msg') || activity && this.reader(id)?.explicit)) {
        const r = this.reader(id), pages = r?.pages || [];
        // Activity pages retain their source formatting. Message pages use
        // the same Bend Markdown renderer, once per retained small page.
        const body = [];
        if (r?.before) body.push(node('div', 'body-before-space:' + id, { style: 'height:' + r.before + 'px', 'aria-hidden': 'true' }, []));
        if (pages[0]?.previous) body.push(node('span', 'body-previous:' + id, { 'data-body-edge': 'previous', 'data-body-id': id }, []));
        for (const p of pages) {
          if (!p.rendered) {
            const key = 'body-part:' + id + ':' + p.start;
            const made = !activity && this.renderBody?.(node('bp-memo', key, {fn:'msg', who:attr(n,'who') || ((attr(n,'class') || '').split(/\s+/).includes('user') ? 'u' : 'a'), stamp:''}, [text(p.text)]));
            const content = made ? items(made.kids).filter(k => !(is(k,'El') && ((attr(k,'class') || '').split(/\s+/).some(c => c === 'stamp' || c === 'copy')))) : [text(p.text)];
            p.rendered = node(made ? 'div' : 'pre', key, {class:'body-message-part', 'data-body-id':id, 'data-body-start':p.start,
              style:made ? 'overflow-wrap:anywhere;max-width:100%' : 'white-space:pre-wrap;overflow-wrap:anywhere;max-width:100%;margin:0'}, content);
          }
          body.push(p.rendered);
        }
        if (!pages.length) {
          const collectText = x => is(x, 'Txt') ? x.text : items(x.kids).map(collectText).join('');
          const source = memo ? items(n.kids).filter(k => is(k, 'Txt')).map(k => k.text).join('') : items(n.kids).filter(k => is(k, 'El') && (attr(k, 'class') || '').split(/\s+/).includes('body')).map(collectText).join('');
          body.push(node('pre', 'body-part:' + id + ':0', { class: 'body-message-part', 'data-body-id': id, 'data-body-start': 0,
            style: 'white-space:pre-wrap;overflow-wrap:anywhere;max-width:100%;margin:0' }, [text(source)]));
        }
        if (!pages.length || pages.at(-1).hasMore || r?.refreshTail) body.push(node('span', 'body-next:' + id, {
          class: 'muted', 'data-body-edge': 'next', 'data-body-id': id, role: 'status'
        }, [text(r?.error ? 'Message text is unavailable while the connection refreshes.' : id.startsWith('@live:') && !pages.length ? 'Earlier text loads as you scroll.' : 'Loading message text…')]));
        if (r?.after) body.push(node('div', 'body-after-space:' + id, { style: 'height:' + r.after + 'px', 'aria-hidden': 'true' }, []));
        if (pages.length) body.push(node('span', 'body-range:' + id, { class: 'muted', 'aria-label': 'Message reading range' },
          [text(pages[0].start === 0 && !pages.at(-1).hasMore ? '' : 'Message text continues as you scroll.')]));
        const stamp = memo ? [node('span', 'body-stamp:' + id, {class:'stamp'}, [text(attr(n, 'stamp') || '')])]
          : items(n.kids).filter(k => is(k, 'El') && (attr(k, 'class') || '').split(/\s+/).includes('stamp'));
        const copy = pages.length ? [node('button', 'body-copy:' + id, {class:'copy', 'data-on-click':'copy',
          'data-value':pages.map(p=>p.text).join(''), title:'Copy displayed part'}, [text('Copy displayed part')])] : [];
        if (activity) return { ...n, kids: list([...items(n.kids).filter(k => !(is(k, 'El') && k.tag === 'button' && k.key === 'thin-more:' + id)), node('div', 'body-window:' + id, {class:'body body-window'}, body)]) };
        return node('div', id, {class:'msg ' + (attr(n, 'who') === 'u' ? 'user' : 'assistant'), 'data-id':id},
          [node('div', 'body-window:' + id, { class: 'body body-window', 'data-body-id': id }, body), ...stamp, ...copy]);
      }
      return { ...n, kids: list(items(n.kids).map(walk)) };
    };
    return walk(root);
  }
  anchor(timeline) {
    if (!timeline) return null;
    const box = timeline.getBoundingClientRect();
    const el = [...timeline.querySelectorAll('[data-body-start]')].find(e => {const r=e.getBoundingClientRect();return r.bottom > box.top + 1 && r.top < box.bottom;});
    return el && { id: el.dataset.bodyId, start: el.dataset.bodyStart, offset: el.getBoundingClientRect().top };
  }
  restore(timeline, anchor) {
    if (!timeline || !anchor) return;
    const el = [...timeline.querySelectorAll('[data-body-start]')].find(e => e.dataset.bodyId === anchor.id && e.dataset.bodyStart === anchor.start);
    if (el) timeline.scrollTop += el.getBoundingClientRect().top - anchor.offset;
  }
  observe(timeline, ready) {
    if (!timeline) return;
    for (const el of timeline.querySelectorAll('[data-body-start]')) {
      const r = this.reader(el.dataset.bodyId), start = Number(el.dataset.bodyStart);
      if (r) {
        r.heights.delete(start);r.heights.set(start, el.getBoundingClientRect().height);
        while (r.heights.size > 64) r.heights.delete(r.heights.keys().next().value);
      }
    }
    if (!ready || this.pending || !this.fence) return;
    const bounds = timeline.getBoundingClientRect(), distance = timeline.clientHeight;
    for (const el of timeline.querySelectorAll('[data-body-edge]')) {
      const box = el.getBoundingClientRect();
      if (box.bottom < bounds.top - distance || box.top > bounds.bottom + distance) continue;
      const id = el.dataset.bodyId, part = this.parts.get(id), r = this.reader(id, true);
      if (!part || !r || r.error || !part.automatic && !r.explicit) continue;
      const previous = el.dataset.bodyEdge === 'previous';
      const live = id.startsWith('@live:');
      if (live && !r.pages.length && timeline.scrollHeight - timeline.scrollTop - timeline.clientHeight <= 16) continue;
      const replacing = !previous && r.refreshTail && r.pages.length > 0;
      const cursor = replacing ? this.liveCursor(part, part.cursor, r.pages.at(-1).start) : previous ? r.pages[0]?.previous : r.pages.at(-1)?.cursor || (!r.pages.length ? part.cursor : '');
      if (!cursor) continue;
      if (this.request?.(part, cursor, part.automatic && !r.pages.length && (part.id.startsWith('@live:') || timeline.scrollHeight - timeline.scrollTop - timeline.clientHeight <= 16))) this.pending = { id, thread: part.thread, version: part.version, cursor, fence: this.fence, direction: replacing ? 'replace' : previous ? 'previous' : 'next', generation: this.generation, viewId: this.viewId, owner: this.owner };
      return;
    }
  }
}
