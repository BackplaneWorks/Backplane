// What the Add project dialog decides about a clone job (docs/CLONE.md), kept apart from the DOM so it is tested (test/clone-model.test.ts).

/** A clone job as the hub sends it (clone.start, clone.status, the {"t":"d","k":"clone"} push). */
export interface CloneJob {
  id: string;
  repo?: string;
  origin?: string;
  dest?: string;
  destShown?: string;
  name?: string;
  state: "running" | "done" | "failed" | "cancelled" | string;
  phase?: string;
  pct?: number | null;
  detail?: string;
  error?: string;
  auth?: boolean;
  signin?: boolean;
  notes?: string[];
  /** a newer hub: notes to read before going to the clone; absent: any note holds */
  hold?: boolean;
  project?: string;
  thread?: string;
  started?: number;
  ended?: number;
}

/** A job id the client makes before it asks, so pushes are recognised before clone.start's reply: [A-Za-z0-9_-], at most 64. */
export function newJobId(now: number = Date.now(), rand: () => number = Math.random): string {
  const r = Math.floor(rand() * 2 ** 40).toString(36);
  return `w${now}-${r}`.replace(/[^A-Za-z0-9_-]/g, "").slice(0, 64);
}

export function isTerminal(state: string): boolean {
  return state === "done" || state === "failed" || state === "cancelled";
}

/**
 * Whether `incoming` (a push or a reply) replaces `held`: never an older picture over a newer one. Another job (or none held) is always
 * taken; a terminal state beats running and running never beats a terminal one; within the same kind the later `ended` wins, and on a tie
 * the later frame (`incoming`).
 */
export function isNewer(held: CloneJob | null | undefined, incoming: CloneJob): boolean {
  if (!held || held.id !== incoming.id) return true;
  const ht = isTerminal(held.state);
  const it = isTerminal(incoming.state);
  if (ht !== it) return it;
  return (incoming.ended ?? 0) >= (held.ended ?? 0);
}

/** A done clone goes to its thread at once unless it has something to read first. */
export function goesAtOnce(job: CloneJob): boolean {
  if (job.state !== "done") return false;
  if (typeof job.hold === "boolean") return !job.hold;
  return (job.notes ?? []).length === 0;
}
