// Pure Command Palette model: item building, GPUI-matching ranking, urgency ordering,
// remote search merge, and match-range highlighting. Zero DOM or virtual-module imports.
import type { SlashCommand } from "./composer";
import type { RailEntry } from "./rail";
import type { BotItem, ProjectItem, ThreadItem } from "./sidebar";

export type PaletteAction =
  | { type: "thread"; id: string }
  | { type: "rail"; id: string }
  | { type: "thread-act"; action: string }
  | { type: "rename" }
  | { type: "prev-thread" }
  | { type: "next-thread" }
  | { type: "command"; word: string; runDirectly: boolean }
  | { type: "new-thread"; projectId: string }
  | { type: "improvements" }
  | { type: "settings" }
  | { type: "sidebar" };

export type PaletteKind = "thread" | "bot" | "project" | "action" | "command" | "you said" | "agent said";

export interface PaletteItem {
  label: string;
  place: string;
  kind: PaletteKind;
  shortcut?: string;
  urgency?: number;
  dotClass?: string;
  want?: string;
  action: PaletteAction;
}

export interface PaletteSearchMessage {
  thread: string;
  title?: string;
  project?: string;
  role?: string;
  text?: string;
  msg?: string;
}

export interface PaletteSearchResult {
  threads?: ThreadItem[];
  projects?: ProjectItem[];
  messages?: PaletteSearchMessage[];
}

export interface PaletteState {
  selectedThreadId: string;
  threads: ThreadItem[];
  projects: ProjectItem[];
  bots: BotItem[];
  railItems: RailEntry[];
  commands: SlashCommand[];
}

export interface PaletteLabels {
  trs: (s: string) => string;
  t: (key: string, fallback: string) => string;
}

const DEFAULT_LABELS: PaletteLabels = {
  trs: (s) => s,
  t: (_key, fallback) => fallback,
};

// Zero-argument composer commands that execute directly via `composer.run`.
// Commands that take arguments (`/btw <q>`, `/intent <text>`, `/project <name>`, `/give ...`)
// prefill the composer draft and focus it so the user can supply arguments.
const DIRECT_COMMANDS = new Set(["/settle"]);

/**
 * Match ranking identical to GPUI's `screens/palette.rs::rank`:
 * - empty query -> 1
 * - prefix match -> 3
 * - word-boundary prefix match -> 2
 * - ordered subsequence match -> 1
 * - no match -> 0
 */
export function rankLabel(label: string, q: string): number {
  const query = q.trim().toLowerCase();
  if (!query) return 1;
  const low = label.toLowerCase();
  if (low.startsWith(query)) return 3;
  const words = low.split(/[^a-z0-9]+/i);
  if (words.some((w) => w.startsWith(query))) return 2;
  let pos = 0;
  for (let i = 0; i < query.length; i++) {
    const idx = low.indexOf(query[i], pos);
    if (idx < 0) return 0;
    pos = idx + 1;
  }
  return 1;
}

/**
 * Urgency score matching GPUI's `palette.rs::urgency`:
 * threads waiting on the person surface first when the query is empty.
 */
export function urgencyOf(th: ThreadItem): { urgency: number; want: string } {
  if ((th.waiting ?? 0) > 0 || th.status === "waiting") return { urgency: 3, want: "waiting" };
  if (th.status === "asking") return { urgency: 2, want: "question" };
  if (th.status === "failed" || th.state === "failed") return { urgency: 1, want: "failed" };
  return { urgency: 0, want: "" };
}

function dotClassFor(th: ThreadItem): string {
  const s = th.status || th.state;
  if (s === "active" || s === "working") return "bp-dot-work";
  if (s === "finished") return "bp-dot-done";
  if (s === "waiting" || s === "asking" || s === "blocked" || s === "failed" || (th.waiting ?? 0) > 0) return "bp-dot-need";
  return "bp-dot-idle";
}

export function markRange(label: string, q: string): [number, number] | null {
  const firstWord = q.trim().split(/\s+/)[0]?.toLowerCase() ?? "";
  if (!firstWord) return null;
  const low = label.toLowerCase();
  if (low.length !== label.length) return null;
  const idx = low.indexOf(firstWord);
  if (idx < 0) return null;
  return [idx, idx + firstWord.length];
}

export function buildPaletteItems(state: PaletteState, labels: PaletteLabels = DEFAULT_LABELS): PaletteItem[] {
  const { t, trs } = labels;
  const out: PaletteItem[] = [];
  const projById = new Map<string, string>();
  for (const p of state.projects) projById.set(p.id, p.title || p.id);

  const cur = state.threads.find((x) => x.id === state.selectedThreadId) ?? null;

  // 1. Enabled rail pages and tools (excluding `commands` itself, matching GPUI palette.rs:95)
  for (const row of state.railItems) {
    if (row.state === "disabled" || row.id === "commands") continue;
    if (!cur && row.id !== "terminal") continue;
    out.push({
      label: trs(row.label),
      place: row.id === "terminal" ? "" : "viewer",
      kind: "action",
      shortcut: row.key ? row.key.toUpperCase() : undefined,
      action: { type: "rail", id: row.id },
    });
  }

  // 2. Active thread actions (only actions supported by data/1 `thread.act`)
  if (cur) {
    out.push({
      label: cur.pinned ? t("menu.unpin", "Unpin") : t("menu.pin", "Pin"),
      place: "",
      kind: "action",
      action: { type: "thread-act", action: cur.pinned ? "unpin" : "pin" },
    });
    out.push({
      label: cur.settled ? t("phone.unsettle", "Unsettle") : t("menu.settle", "Settle"),
      place: "",
      kind: "action",
      action: { type: "thread-act", action: cur.settled ? "unsettle" : "settle" },
    });
    out.push({
      label: cur.archived ? t("palette.unarchive", "Unarchive") : t("palette.archive", "Archive"),
      place: "",
      kind: "action",
      action: { type: "thread-act", action: cur.archived ? "unarchive" : "archive" },
    });
    out.push({
      label: "Rename thread",
      place: "",
      kind: "action",
      action: { type: "rename" },
    });
    out.push({
      label: "Previous thread",
      place: "",
      kind: "action",
      action: { type: "prev-thread" },
    });
    out.push({
      label: "Next thread",
      place: "",
      kind: "action",
      action: { type: "next-thread" },
    });
  }

  // 3. Hub composer commands (`hello.commands`)
  if (cur) {
    for (const cmd of state.commands) {
      if (!cmd.word) continue;
      const runDirectly = DIRECT_COMMANDS.has(cmd.word.toLowerCase());
      out.push({
        label: cmd.label ? `${cmd.word} — ${cmd.label}` : cmd.word,
        place: cmd.desc || "command",
        kind: "command",
        action: { type: "command", word: cmd.word, runDirectly },
      });
    }
  }

  // 4. Global actions
  out.push({
    label: t("gpui.improvements", "Improvements"),
    place: "",
    kind: "action",
    action: { type: "improvements" },
  });
  out.push({
    label: t("palette.settings", "Settings"),
    place: "",
    kind: "action",
    action: { type: "settings" },
  });
  out.push({
    label: "Fold or unfold the sidebar",
    place: "",
    kind: "action",
    action: { type: "sidebar" },
  });

  // 5. New thread in each project
  for (const p of state.projects) {
    out.push({
      label: `New thread in ${p.title || p.id}`,
      place: "",
      kind: "action",
      action: { type: "new-thread", projectId: p.id },
    });
  }

  // 6. Threads
  for (const th of state.threads) {
    const { urgency, want } = urgencyOf(th);
    out.push({
      label: th.title || "Untitled thread",
      place: th.project ? (projById.get(th.project) ?? "") : "",
      kind: "thread",
      urgency,
      want,
      dotClass: dotClassFor(th),
      action: { type: "thread", id: th.id },
    });
  }

  // 7. Bots
  for (const b of state.bots) {
    if (!b.name || !b.thread) continue;
    out.push({
      label: b.name,
      place: "",
      kind: "bot",
      action: { type: "thread", id: b.thread },
    });
  }

  return out;
}

export function filterPaletteItems(
  all: PaletteItem[],
  q: string,
  search: PaletteSearchResult | null,
  projects: ProjectItem[] = [],
  maxLocal = 10,
  maxTotal = 14,
): PaletteItem[] {
  const trimmed = q.trim();
  const empty = trimmed.length === 0;

  // Merge remote thread title hits not already present in local list
  const combined = all.slice();
  if (search?.threads && search.threads.length > 0) {
    const projById = new Map<string, string>();
    for (const p of projects) projById.set(p.id, p.title || p.id);
    const heldThreads = new Set<string>();
    for (const it of combined) {
      if (it.action.type === "thread") heldThreads.add(it.action.id);
    }
    for (const th of search.threads) {
      if (!th.id || heldThreads.has(th.id)) continue;
      heldThreads.add(th.id);
      const { urgency, want } = urgencyOf(th);
      combined.push({
        label: th.title || "Untitled thread",
        place: th.project ? (projById.get(th.project) ?? "") : "",
        kind: "thread",
        urgency,
        want,
        dotClass: dotClassFor(th),
        action: { type: "thread", id: th.id },
      });
    }
  }

  const scored: Array<{ score: number; index: number; item: PaletteItem }> = [];
  for (let i = 0; i < combined.length; i++) {
    const it = combined[i];
    const labelRank = rankLabel(it.label, trimmed);
    const placeRank = it.place ? Math.min(1, rankLabel(it.place, trimmed)) : 0;
    const score = Math.max(labelRank, placeRank);
    if (score > 0) scored.push({ score, index: i, item: it });
  }

  scored.sort((a, b) => {
    if (empty) {
      const urgDiff = (b.item.urgency ?? 0) - (a.item.urgency ?? 0);
      if (urgDiff !== 0) return urgDiff;
    }
    if (b.score !== a.score) return b.score - a.score;
    const aIsThread = a.item.kind === "thread" ? 0 : 1;
    const bIsThread = b.item.kind === "thread" ? 0 : 1;
    if (aIsThread !== bIsThread) return aIsThread - bIsThread;
    return a.index - b.index;
  });

  const seen = new Set<string>();
  const local: PaletteItem[] = [];
  for (const s of scored) {
    const key = `${s.item.label}\0${s.item.place}\0${s.item.kind}`;
    if (seen.has(key)) continue;
    seen.add(key);
    local.push(s.item);
    if (local.length >= maxLocal) break;
  }

  if (search?.messages && search.messages.length > 0 && local.length < maxTotal) {
    const room = maxTotal - local.length;
    for (const m of search.messages.slice(0, room)) {
      if (!m.thread || !m.text) continue;
      const place = m.project ? `${m.title || ""} · ${m.project}` : (m.title || "");
      local.push({
        label: m.text,
        place,
        kind: m.role === "user" ? "you said" : "agent said",
        action: { type: "thread", id: m.thread },
      });
    }
  }

  return local;
}
