// What a Send took from the composer while its acknowledgement is on its
// way. A send captures the whole draft; the person may type on before the
// hub's Keep of "" comes back. The captured text is still the first
// characters of the field then, unless the person edited them: selected
// them and typed over them, deleted into them, pasted over them. Each claim
// knows whether its text is untouched (`intact`), from the field's own edit
// events (touch: where an edit starts) and from each draft value (edited:
// a draft that no longer starts with it). An accepted send takes its text
// off the front of the field only while it is intact; otherwise the field
// stays exactly as the person made it. Nothing is guessed from what the new
// text happens to start with.
//
// A claim's `text` is in the field's terms: what it covers from the start
// of the field (a second Send before the first's acknowledgement covers the
// first's text too; `over` names the claims it covered). While an IME
// composition is under way the field is not touched: an accepted claim is
// `due`, and its text comes off when the composition ends (settle).
export class SentClaims {
  constructor() { this.list = []; }
  // a send captured `text`, the whole draft of `thread`
  capture(thread, text) {
    if (!thread || typeof text !== 'string' || !text) return null;
    const claim = { thread, text, intact: true, due: false, over: this.list.filter(c => c.thread === thread && c.intact) };
    this.list.push(claim);
    // (sends that never settle leave theirs; keep the list short)
    if (this.list.length > 32) this.list.splice(0, this.list.length - 32);
    return claim;
  }
  // an edit in the thread's field starting at `start`: every claim that
  // reaches past it is the person's text again
  touch(thread, start) {
    for (const c of this.list) if (c.thread === thread && start < c.text.length) c.intact = false;
  }
  // the thread's draft after an edit: a claim it no longer starts with too
  edited(thread, value) {
    for (const c of this.list) if (c.thread === thread && c.intact && !(typeof value === 'string' && value.startsWith(c.text))) c.intact = false;
  }
  drop(claim) {
    this.list = this.list.filter(c => c !== claim);
    for (const c of this.list) c.over = c.over.filter(x => x !== claim);
  }
  // its send was accepted while newer text was typed: the text to take off
  // the front of the field now ("": nothing, or not yet while composing)
  accept(claim, composing) {
    if (!claim || !this.list.includes(claim)) return '';
    if (!claim.intact) { this.drop(claim);return ''; }
    claim.due = true;
    return composing ? '' : this.settle(claim.thread);
  }
  // the accepted claims of the thread, oldest first, taken off together
  settle(thread) {
    let cut = '';
    for (const claim of this.list.filter(c => c.thread === thread)) {
      if (!claim.due) break;
      if (claim.intact) {
        cut += claim.text;
        // the claims that covered it now start where it ended
        for (const c of this.list) if (c !== claim && c.over.includes(claim) && c.text.startsWith(claim.text)) c.text = c.text.slice(claim.text.length);
      }
      this.drop(claim);
    }
    return cut;
  }
}

// Where an edit about to change the field starts (an input event's type and
// the field before it): a selection's start, or the character before the
// caret for a Backspace. An edit whose reach is not known here (a word or a
// line deleted backwards, a drop, an autocorrection, undo) counts from the
// start of the field, so the person's text is never taken off with it.
export function editStart(type, value, start, end) {
  if (start !== end) return Math.min(start, end);
  if (type === 'deleteContentBackward') return start - (/[\udc00-\udfff]/.test(value?.[start - 1] || '') ? 2 : 1);
  if (type?.startsWith('delete')) return type.includes('Backward') || type === 'deleteByDrag' ? 0 : start;
  if (/^(insertFromDrop|insertReplacementText|history)/.test(type || '')) return 0;
  return start;
}
