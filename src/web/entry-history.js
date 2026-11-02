// Initial viewport fill is separate from scrolling intent. It is armed by an
// accepted fresh entry, never by a render, resize, live append or a saved anchor.
import { HELD_IDS } from './thin-host.js';

// One page at a time, bounded by the source's remaining count and the existing
// retained-history capacity. Reserve one complete newest page for the return.
// Unknown counts from older hubs retain the small two-page fallback.
const PAGE_BUDGET = Math.floor((HELD_IDS - 32) / 32);
export class EntryHistory {
  constructor() { this.cancel(); }
  cancel() { this.owner = '';this.left = 0;this.phase = '';this.cursors = new Set(); }
  arm(owner, remaining) {
    this.cancel();
    if (owner) {
      this.owner = owner;
      this.left = Number.isSafeInteger(remaining) && remaining >= 0 ? Math.min(remaining, PAGE_BUDGET) : 2;
      this.phase = 'fill';
    }
  }
  active(owner) { return !!owner && this.owner === owner; }
  check(owner, thin, usable) {
    if (!this.active(owner)) { this.cancel();return false; }
    if (!thin.ready || thin.pageError || thin.pendingNewest) { this.cancel();return false; }
    if (thin.paging || thin.pagingNext || thin.pageActionPending()) return true;
    if (this.phase === 'fill' && !usable && this.left && thin.hasAhead('older')) {
      // Only an issued opaque cursor can advance the fill. A repeated cursor
      // cannot spend the second page or create a request loop.
      const cursor = (thin.held || thin.heldHere()).pages[0]?.older;
      if (cursor && !this.cursors.has(cursor) && thin.ahead('older')) {
        this.cursors.add(cursor);this.left--;return true;
      }
    }
    // Return to the newest source using the normal merged-newer path. Unlike
    // newest(), this keeps the older rows that made the viewport scrollable.
    this.phase = 'return';
    if (!thin.atNewest()) return thin.ahead('newer');
    this.cancel();return false;
  }
}
