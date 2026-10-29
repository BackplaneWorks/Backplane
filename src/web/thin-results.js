import { utf8Bytes, viewTarget } from './thin.js';
import { validatePresentation } from './startup.js';
import { mailDraftContext } from './mail-drafts.js';
import { terminalInputReply } from './terminal-input.js';
import { modelMenuReply } from './model-menu-input.js';
const object = x => x && typeof x === 'object' && !Array.isArray(x);
const string = (x, cap = 1024) => typeof x === 'string' && utf8Bytes(x) <= cap;
const decimal = x => string(x, 32) && /^(0|[1-9][0-9]*)$/.test(x);
const flag = x => typeof x === 'boolean';
export function validateBodyPage(m, input) {
  return m?.bodyPage === 1 && input.bodyPage === 1 && string(m.text, 8196)
    && Object.keys(m).every(k => ['id','thread','version','text','bodyPage','start','end','total','hasMore','complete','cursor','previous'].includes(k)) && m.version === input.bodyVersion && m.thread === input.bodyThread
    && Number.isSafeInteger(m.start) && m.start >= 0 && Number.isSafeInteger(m.end) && m.end > m.start
    && Number.isSafeInteger(m.total) && m.total >= m.end && m.total < 2 ** 47
    && string(m.previous) && (m.start === 0 ? m.previous === '' : !!m.previous)
    && m.hasMore === (m.end < m.total);
}

export function validateThinResult(r, input) {
  try {
    if (Object.hasOwn(r, 'modelMenuInput')) return modelMenuReply(r, input);
    if (Object.hasOwn(r, 'terminalInput')) return terminalInputReply(r, input);
    if (Object.hasOwn(input, 'query')) {
      const s = r.search;
      return object(s) && string(s.query, 256) && Array.isArray(s.rows) && s.rows.length <= 25 && string(s.cursor)
        && flag(s.hasMore) && (!s.hasMore || !!s.cursor) && Number.isInteger(s.visited) && s.visited >= 0 && s.visited <= 128
        && utf8Bytes(JSON.stringify(r)) <= 65536 && s.rows.every(row => object(row) && string(row.id) && !!row.id && string(row.thread) && !!row.thread
          && decimal(row.ordinal) && ['message', 'activity'].includes(row.kind) && string(row.text, 1024) && flag(row.partial));
    }
    if (Object.hasOwn(input, 'entryId')) {
      const m = r.message;
      return object(m) && m.id === input.entryId && string(m.id) && string(m.thread) && !!m.thread && decimal(m.version)
        && string(m.text, 65536) && string(m.cursor) && flag(m.hasMore) && flag(m.complete)
        && (m.hasMore ? !!m.cursor && !m.complete : !m.cursor && m.complete)
        && (input.bodyPage === 1 ? validateBodyPage(m, input) : m.bodyPage === undefined);
    }
    if (Object.hasOwn(input, 'requestId')) return flag(r.cancelled);
    if (r.mailDraftContext != null && !mailDraftContext(r.mailDraftContext, r.epoch)) return false;
    // A creation ACK can resolve in the actor's next canonical refresh.
    // Transport identity/currentness still fences that update's navigation.
    if (r.nextTarget !== undefined && (!viewTarget(r.nextTarget) || r.remoteReady !== false || Object.hasOwn(r, 'shown'))) return false;
    if (r.nextTarget && r.nextTarget.kind === input.target?.kind && r.nextTarget.id === input.target?.id) return false;
    validatePresentation({ session: r.session, view: r.nextTarget ? { type: 'text', text: '' } : r.shown, cmds: r.cmds });
    if (r.session.rid >= 2 ** 47) return false;
    if (r.nextTarget) return true;
    const w = r.window;
    return object(w) && Array.isArray(w.ids) && w.ids.length <= 32 && w.ids.every(id => string(id) && !!id)
      && new Set(w.ids).size === w.ids.length && string(w.older) && flag(w.hasMore) && (!w.hasMore || !!w.older)
      && object(w.anchor) && string(w.anchor.id) && Number.isInteger(w.anchor.offset) && w.anchor.offset >= 0 && w.anchor.offset <= 2 ** 32 - 1
      && Array.isArray(w.partial) && w.partial.length <= 33 && w.partial.every(p => object(p) && string(p.id) && !!p.id && string(p.cursor) && !!p.cursor && p.reason === 'byte-limit'
        && (p.bodyPage === undefined || p.bodyPage === 1 && decimal(p.version) && string(p.thread) && !!p.thread && flag(p.automatic)
          && (p.streamGeneration === undefined && p.streamCut === undefined && p.streamBlocks === undefined
            || p.id.startsWith('@live:') && decimal(p.streamGeneration) && flag(p.streamCut) && Number.isSafeInteger(p.streamBlocks) && p.streamBlocks > 8 && p.streamBlocks < 2 ** 47)));
  } catch { return false; }
}

// Chunk assembly is scoped to an explicitly requested message, owner/version
// and its exact preceding cursor. No invalidation downloads message chunks.
export class MessageParts {
  constructor() { this.parts = new Map(); }
  clear() { this.parts.clear(); }
  reset(id) { this.parts.delete(id); }
  next(id) { return this.parts.get(id)?.cursor; }
  accept(message, requestedCursor) {
    const old = this.parts.get(message.id);
    if (old && (old.cursor !== requestedCursor || old.thread !== message.thread || old.version !== message.version)) throw Error('Stale message continuation');
    if (!old && this.parts.size >= 4) this.parts.delete(this.parts.keys().next().value);
    const bytes = utf8Bytes(message.text) + (old?.bytes || 0);
    if (bytes > 8 * 1024 * 1024) throw Error('Message reader memory limit');
    const value = { ...message, text: (old?.text || '') + message.text, bytes,
      // A nonempty first cursor starts partway through a message. It can be
      // read, but the accumulated text must never be called the whole body.
      complete: message.complete && (old ? old.fromStart : requestedCursor === ''), fromStart: old ? old.fromStart : requestedCursor === '' };
    this.parts.set(message.id, value);return value;
  }
}
