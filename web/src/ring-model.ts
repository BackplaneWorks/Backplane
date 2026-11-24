// Pure Ring notes model (Settings › Ring): the hub's `ring.list` answer read defensively, the same-origin address that plays a
// recording, and the words each note's line is made of (voice/ringpage.rs draws the same on the desktop and the phones).
// Zero DOM imports so it can be unit tested directly with bun test.

export interface RingNote {
  id: string;
  /** When it was said, unix seconds. */
  said: number;
  gesture: string;
  words: string;
  model: string;
  conf: number | null;
  unsure: string;
  state: string;
  error: string;
  thread: string;
  play: string;
  heard: number;
}

export interface RingList {
  total: number;
  notes: RingNote[];
  /** The note the hub is hearing now ("" for none). */
  busy: string;
}

/** A text the page shows: a catalog key, its English, and what fills its `{}`. */
export interface RingText {
  key: string;
  fallback: string;
  vars?: Record<string, string | number>;
}

function str(v: unknown): string {
  return typeof v === "string" ? v : "";
}

function num(v: unknown): number {
  return typeof v === "number" && Number.isFinite(v) ? v : 0;
}

export function parseRingList(raw: unknown): RingList {
  const r = raw && typeof raw === "object" ? (raw as Record<string, unknown>) : {};
  const notes: RingNote[] = [];
  for (const x of Array.isArray(r.notes) ? r.notes : []) {
    if (!x || typeof x !== "object") continue;
    const n = x as Record<string, unknown>;
    const id = str(n.id);
    if (!id) continue;
    notes.push({
      id,
      said: num(n.said),
      gesture: str(n.gesture),
      words: str(n.words),
      model: str(n.model),
      conf: typeof n.conf === "number" && Number.isFinite(n.conf) ? n.conf : null,
      unsure: str(n.unsure),
      state: str(n.state),
      error: str(n.error),
      thread: str(n.thread),
      play: str(n.play),
      heard: num(n.heard),
    });
  }
  return { total: Math.max(num(r.total), notes.length), notes, busy: str(r.busy) };
}

/**
 * The address this page plays a recording from: the hub's link names the hub as the Pebble app reaches it, the browser asks its own
 * origin for the same path and key (`/ring/audio/<id>?k=<key>`). Anything else plays nothing.
 */
export function ringAudioPath(play: string): string {
  const m = /\/ring\/audio\/(n[0-9a-f]{16})(?:\.m4a)?\?k=([0-9a-fA-F]+)$/.exec(play);
  return m ? `/ring/audio/${m[1]}?k=${m[2]}` : "";
}

/** Who heard the note's words. */
export function ringHeardText(n: Pick<RingNote, "model" | "conf">): RingText {
  if (!n.model) return { key: "web.ring.not.heard", fallback: "Not heard" };
  if (n.model === "ring") return { key: "web.ring.app.words", fallback: "The Pebble app's words" };
  if (n.conf !== null) {
    return { key: "web.ring.heard.by.sure", fallback: "Heard by {model}, {conf}% sure", vars: { model: n.model, conf: Math.round(n.conf) } };
  }
  return { key: "web.ring.heard.by", fallback: "Heard by {model}", vars: { model: n.model } };
}

/** Where the note is on its way to Pebble; `busy`: this page asked for it to be heard again and the answer is not in yet. */
export function ringStateText(state: string, busy: boolean): RingText {
  if (busy) return { key: "web.ring.state.busy", fallback: "being heard now" };
  switch (state) {
    case "sent": return { key: "web.ring.state.sent", fallback: "given to Pebble" };
    case "new": return { key: "web.ring.state.new", fallback: "waiting to be heard" };
    case "heard": return { key: "web.ring.state.heard", fallback: "waiting to go to Pebble" };
    case "failed": return { key: "web.ring.state.failed", fallback: "not given to Pebble" };
    default: return { key: "", fallback: state };
  }
}

/** The line over the list: how many there are, or how many of them the search found. */
export function ringCountText(total: number, found: number, q: string): RingText {
  if (total <= 0) return { key: "ring.no.recordings.yet", fallback: "No recordings yet." };
  if (q.trim()) {
    return { key: "web.ring.found", fallback: "{found} of {total} have “{q}”.", vars: { found, total, q: q.trim() } };
  }
  return total === 1
    ? { key: "web.ring.count.one", fallback: "1 recording" }
    : { key: "web.ring.count", fallback: "{n} recordings", vars: { n: total } };
}
