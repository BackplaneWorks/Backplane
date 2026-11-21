// A swarm row's need (native/host/src/present/swarm_need.rs through present/sections.rs `swarms_of`): the line saying what happened and the
// actions that resolve it, and what a press of each does here. Pure, so the sidebar only draws it.

export interface NeedAction {
  label: string;
  act: string;
  room: string;
  thread: string;
}

export interface SwarmNeed {
  line: string;
  why: string;
  actions: NeedAction[];
}

export type NeedGo =
  | { kind: "board"; room: string }
  | { kind: "thread"; id: string }
  | { kind: "account"; id: string }
  | { kind: "call"; method: "swarm.act"; params: { room: string; action: string; thread?: string } };

const ASKS = new Set(["resume", "raise", "retry", "move", "signin"]);

export function parseNeed(v: unknown): SwarmNeed | null {
  if (!v || typeof v !== "object") return null;
  const o = v as Record<string, unknown>;
  const s = (x: unknown) => (typeof x === "string" ? x : "");
  const actions = (Array.isArray(o.actions) ? o.actions : [])
    .filter((a): a is Record<string, unknown> => Boolean(a) && typeof a === "object")
    .map((a) => ({ label: s(a.label), act: s(a.act), room: s(a.room), thread: s(a.thread) }));
  const need = { line: s(o.line), why: s(o.why), actions };
  return need.line || actions.length ? need : null;
}

/** What a press does; null: nothing this client can do (`board` without a room view, an act it does not know). */
export function needPress(a: NeedAction, hasRoomView: boolean): NeedGo | null {
  if (a.act === "board") return hasRoomView && a.room ? { kind: "board", room: a.room } : null;
  if (a.act === "open") return a.thread ? { kind: "thread", id: a.thread } : null;
  if (ASKS.has(a.act)) {
    const params: { room: string; action: string; thread?: string } = { room: a.room, action: a.act };
    if (a.thread) params.thread = a.thread;
    return { kind: "call", method: "swarm.act", params };
  }
  return null;
}

/** What `swarm.act`'s reply opens next: "account:<id>" (that account's sign-in) or "thread:<id>" (a terminal running the provider's sign-in). */
export function needAfter(reply: unknown): NeedGo | null {
  const open = reply && typeof reply === "object" ? (reply as Record<string, unknown>).open : undefined;
  if (typeof open !== "string") return null;
  const i = open.indexOf(":");
  const id = i < 0 ? "" : open.slice(i + 1);
  if (!id) return null;
  if (open.startsWith("account:")) return { kind: "account", id };
  if (open.startsWith("thread:")) return { kind: "thread", id };
  return null;
}
