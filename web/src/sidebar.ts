// Left sidebar: Crew / Inbox / Bench layout matching GPUI:
// - Top header ("This computer" + search + quick add)
// - Sticky Crew band (never scrolls): Uberbot (Bub), compact bot avatar strip, wrapping room chips
// - Scrollable Inbox (Needs you + Running + foldable Snoozed) with unmistakable icon+word status pills,
//   project context, and hover 1-click Snooze (1h / Tomorrow / 1w) & Settle
// - Scrollable Bench (Pinned + Recent MRU projects with hw/sw icons, live badges, hover quick-open)
// - Foldable All projects (alphabetical, optional All/Boards/Code filter) & Quiet
// - Bottom Connected status footer
import { t } from "./i18n";
import { needPress, parseNeed, type NeedAction } from "./swarm-need";
import type { HomeInfo } from "./ws";

/** The place of the chats that belong to no project (the hub's `sidebar::CHATS`): `thread.create {project: CHATS}` starts one. */
export const CHATS = "chats";

export const SIDEBAR_CACHE_KEY = "bp.sidebar.cache.v1";

export interface SidebarCache {
  v: 1;
  hostName?: string;
  sections: SidebarSection[] | null;
  projects: ProjectItem[];
  threads: ThreadItem[];
  bots: BotItem[];
  rooms: RoomItem[];
  shellHashes: Record<string, string>;
  openSectionFolds: string[];
}

export function loadSidebarCache(storage: Pick<Storage, "getItem"> | null | undefined): SidebarCache | null {
  try {
    const raw = storage?.getItem(SIDEBAR_CACHE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as Partial<SidebarCache> | null;
    if (!parsed || parsed.v !== 1) return null;
    const projects = Array.isArray(parsed.projects) ? parsed.projects : [];
    const threads = Array.isArray(parsed.threads) ? parsed.threads : [];
    const sections = parsed.sections === null ? null : Array.isArray(parsed.sections) ? parsed.sections : null;
    const bots = Array.isArray(parsed.bots) ? parsed.bots : [];
    const rooms = Array.isArray(parsed.rooms) ? parsed.rooms : [];
    const shellHashes = parsed.shellHashes && typeof parsed.shellHashes === "object" && !Array.isArray(parsed.shellHashes)
      ? parsed.shellHashes
      : {};
    const openSectionFolds = Array.isArray(parsed.openSectionFolds)
      ? parsed.openSectionFolds.filter((x): x is string => typeof x === "string")
      : [];
    if (projects.length === 0 && threads.length === 0 && (!sections || sections.length === 0)) return null;
    return {
      v: 1,
      hostName: typeof parsed.hostName === "string" ? parsed.hostName : undefined,
      sections,
      projects,
      threads,
      bots,
      rooms,
      shellHashes,
      openSectionFolds,
    };
  } catch {
    return null;
  }
}

export function saveSidebarCache(
  storage: Pick<Storage, "setItem"> | null | undefined,
  cache: Omit<SidebarCache, "v">,
): void {
  try {
    storage?.setItem(SIDEBAR_CACHE_KEY, JSON.stringify({ v: 1, ...cache }));
  } catch { /* ignore quota / private mode */ }
}

export function clearSidebarCache(storage: Pick<Storage, "removeItem"> | null | undefined): void {
  try {
    storage?.removeItem(SIDEBAR_CACHE_KEY);
  } catch { /* ignore */ }
}

/** The projects a chat may move to: this hub's own, by name (a linked machine's copy, `<link>~<id>`, is not offered). */
export function moveTargets(projects: ProjectItem[]): ProjectItem[] {
  return projects
    .filter((p) => p.id && p.id !== CHATS && p.id !== "scratch" && !p.id.includes("~"))
    .slice()
    .sort((a, b) => (a.title || a.id).toLowerCase().localeCompare((b.title || b.id).toLowerCase()));
}

export interface ProjectItem {
  id: string;
  title: string;
  root?: string;
  threadCount?: number;
  settledCount?: number;
  updatedAt?: number;
  waiting?: number;
  color?: string;
  css?: string;
  icon?: string;
}

export interface PlanModeInfo {
  phase: "off" | "armed" | "planning" | "pending" | "executing";
  interaction: string;
  once: boolean;
  pending?: boolean;
}

export interface ThreadItem {
  id: string;
  project: string;
  title: string;
  state: string;
  status?: string;
  updatedAt: number;
  pinned?: boolean;
  pinOrder?: number;
  settled?: boolean;
  archived?: boolean;
  provider?: string;
  model?: string;
  chip?: string;
  preview?: string;
  waiting?: number;
  parent?: string;
  swarm?: string;
  memory?: import("./composer").MemoryProgress | null;
  planMode?: PlanModeInfo | null;
}

export interface BotItem {
  id: string;
  name: string;
  thread: string;
  color?: string;
  uber?: boolean;
  mood?: string;
  note?: string;
}

export interface RoomItem {
  id: string;
  name: string;
  members?: number;
}

export interface SnoozeOption {
  label: string;
  value: string;
}

export interface SectionThreadRow {
  id: string;
  title: string;
  word?: string;
  say?: string;
  want?: string;
  /** Explicit status pill word from hub (Approve, Input, Sign in, Failed, Stopped, Working, Waiting, Done). */
  label?: string;
  /** Explicit status pill tone from hub (bad | run | wait | ok | ""). */
  tone?: string;
  status?: string;
  state?: string;
  ago?: string;
  activity?: number;
  pinned?: boolean;
  project?: string;
  projectKey?: string;
  off?: boolean;
  nav?: string;
  kind?: string;
  /** A chat that belongs to no project (hub `chat` field): it can move to a project. */
  chat?: boolean;
  /** Thread id for 1-click settle when eligible (hub `settle` field). */
  settle?: string;
  /** Snooze duration choices when eligible (hub `snooze` field; send `row-snooze` with `value`). */
  snooze?: SnoozeOption[];
  /** Wake value for a snoozed row (`<qid>|off`, send `row-snooze`). */
  wake?: string;
  /** a swarm row's need (swarm-need.ts): null when nothing waits on the person */
  need?: unknown;
  working?: number;
  members?: number;
  monitoring?: boolean;
  settled?: boolean;
}

export interface SectionProjectRow {
  id: string;
  title: string;
  key?: string;
  nav?: string;
  css?: string;
  color?: string;
  pinned?: boolean;
  hasBoard?: boolean;
  hasSchematic?: boolean;
  threads?: SectionThreadRow[];
  swarms?: SectionThreadRow[];
  more?: number;
  running?: number;
  needsCount?: number;
  fold?: string;
  folded?: boolean;
  under?: number;
  off?: boolean;
}

export interface SidebarSection {
  id: string;
  kind: string;
  title: string;
  count?: number;
  fold?: boolean;
  open?: boolean;
  toggle?: string;
  hint?: string;
  rows: Array<SectionProjectRow | SectionThreadRow>;
}

export interface SidebarCallbacks {
  onSelectThread: (threadId: string) => void;
  onNewThread: (projectId: string) => void;
  /** A project's name: its home. False (or no handler): this hub has none, and the name folds the project as before. */
  onOpenProject?: (projectId: string) => boolean;
  /** Whether a project's name opens a home at all (the name's tooltip says so only then). */
  canOpenProject?: () => boolean;
  onSelectBot: (bot: BotItem) => void;
  onSelectRoom?: (room: RoomItem) => void;
  onNewBot?: () => void;
  onNewRoom?: () => void;
  onQuickOpenViewer?: (projectId: string, kind: "board" | "schematic") => void;
  onThreadAct: (threadId: string, action: string) => void;
  /** Snooze or wake a thread row (`row-snooze` action with hub-provided value). */
  onSnoozeRow?: (value: string) => void;
  onToggleSection?: (toggle: string) => void;
  onToggleProjectFold?: (projectKey: string) => void;
  onCollapseSidebar?: () => void;
  /** a press of a swarm need's button; it rejects with the hub's words when the hub refuses */
  onNeedAct?: (a: NeedAction) => Promise<void>;
  /** this client has a room view: a need's `board` opens it */
  hasRoomView?: boolean;
  /** The + beside the projects' heading: Add project. */
  onAddProject?: () => void;
  /** Settle/dismiss a swarm row (`sidebar.act` `swarm-settle`). */
  onSettleSwarm?: (roomId: string) => void;
  /** New chat: a thread in no project, started at once. */
  onNewChat?: () => void;
  /** A chat row's Move to project…: the app asks which project. */
  onMoveToProject?: (threadId: string) => void;
  /** Dismiss a failed home move banner (`home.move.dismiss`). */
  onDismissHomeMove?: () => void;
}

export interface HomeBannerState {
  text: string;
  isError: boolean;
  dismissible: boolean;
}

/** Pure presentation state for the sidebar home status banner. */
export function homeBannerState(home: HomeInfo | null): HomeBannerState | null {
  if (!home) return null;
  if (home.moving?.state === "moving") {
    const text = home.moving.step
      ? t("home.moving.step", "Moving home to {to}: {step}").replace("{to}", home.moving.to).replace("{step}", home.moving.step)
      : t("home.moving", "Moving home to {to}…").replace("{to}", home.moving.to);
    return { text, isError: false, dismissible: false };
  }
  if (home.moving?.state === "failed") {
    const text = home.moving.note
      ? t("home.move.failed.note", "Home move failed: {note}").replace("{note}", home.moving.note)
      : t("home.move.failed", "Home move failed");
    return { text, isError: true, dismissible: true };
  }
  if (!home.online) {
    const name = home.name || "home";
    let time = "";
    if (home.awaySince && home.awaySince > 0) {
      const d = new Date(home.awaySince);
      time = `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
    }
    let text = "";
    if (time && home.queuedCount > 0) {
      text = t("home.away.since.queued", "Home ({name}) is away since {time} · {n} queued")
        .replace("{name}", name).replace("{time}", time).replace("{n}", String(home.queuedCount));
    } else if (time) {
      text = t("home.away.since", "Home ({name}) is away since {time}")
        .replace("{name}", name).replace("{time}", time);
    } else if (home.queuedCount > 0) {
      text = t("home.away.queued", "Home ({name}) is away · {n} queued")
        .replace("{name}", name).replace("{n}", String(home.queuedCount));
    } else {
      text = t("home.away", "Home ({name}) is away").replace("{name}", name);
    }
    return { text, isError: false, dismissible: false };
  }
  return null;
}

/** Whether a swarm row under a project should be shown in the sidebar. */
export function isSwarmVisibleUnderProject(sw: SectionThreadRow, folded: boolean): boolean {
  if (sw.settled) return false;
  if (sw.state === "done" || sw.state === "stopped") return false;
  if (!folded) return true;
  const working = sw.working ?? 0;
  const monitoring = Boolean(sw.monitoring);
  const needs = Boolean(parseNeed(sw.need)) || sw.word === "needs";
  return ((working > 0 || monitoring) && sw.state !== "paused") || needs;
}

export interface StatusPill {
  icon: string;
  label: string;
  tone: "bad" | "bad-out" | "run" | "wait" | "ok" | "dim";
}

/** Resolve an explicit, scannable icon+word status pill from hub `label`/`tone` or `word`/`want`. */
export function statusPillOf(row: Pick<SectionThreadRow, "label" | "tone" | "word" | "want" | "status" | "state" | "wake">): StatusPill | null {
  if (row.wake) return { icon: "◷", label: "Snoozed", tone: "dim" };

  const rawLabel = (row.label ?? "").trim();
  const rawTone = (row.tone ?? "").trim();
  if (rawLabel) {
    const low = rawLabel.toLowerCase();
    const icon =
      low === "approve" || low === "sign in" ? "!" :
      low === "input" ? "?" :
      low === "failed" || low === "stopped" ? "×" :
      low === "working" || low === "running" ? "▸" :
      low === "waiting" ? "◌" :
      low === "done" ? "✓" : "•";
    const tone: StatusPill["tone"] =
      rawTone === "bad" ? (low === "failed" || low === "stopped" ? "bad-out" : "bad") :
      rawTone === "run" ? "run" :
      rawTone === "wait" ? "wait" :
      rawTone === "ok" ? "ok" : "dim";
    return { icon, label: rawLabel, tone };
  }

  const w = row.word || "";
  if (w === "needs") {
    if (row.want === "approval") return { icon: "!", label: "Approve", tone: "bad" };
    if (row.want === "sign-in") return { icon: "!", label: "Sign in", tone: "bad" };
    return { icon: "?", label: "Input", tone: "bad" };
  }
  if (w === "failed" || row.status === "failed" || row.state === "fail") return { icon: "×", label: "Failed", tone: "bad-out" };
  if (w === "cut" || w === "interrupted") return { icon: "×", label: "Stopped", tone: "bad-out" };
  if (w === "running" || row.status === "active" || row.status === "working" || row.state === "run") return { icon: "▸", label: "Working", tone: "run" };
  if (w === "monitoring" || w === "limit" || row.status === "waiting") return { icon: "◌", label: "Waiting", tone: "wait" };
  if (w === "done" || row.status === "finished") return { icon: "✓", label: "Done", tone: "ok" };
  return null;
}

const SVG_NS = "http://www.w3.org/2000/svg";

function makeSvgIcon(paths: string[], size = 14, className = ""): SVGSVGElement {
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("width", String(size));
  svg.setAttribute("height", String(size));
  svg.setAttribute("fill", "none");
  svg.setAttribute("stroke", "currentColor");
  svg.setAttribute("stroke-width", "1.75");
  svg.setAttribute("stroke-linecap", "square");
  svg.setAttribute("stroke-linejoin", "miter");
  svg.setAttribute("aria-hidden", "true");
  if (className) svg.classList.add(...className.split(" "));
  for (const d of paths) {
    const p = document.createElementNS(SVG_NS, "path");
    p.setAttribute("d", d);
    svg.appendChild(p);
  }
  return svg;
}

function makeFolderIcon(): SVGSVGElement {
  return makeSvgIcon(["M3 6h6l2 2h10v11H3z"], 14, "bp-folder-icon");
}

function makeBoardIcon(size = 14, className = "bp-folder-icon bp-hw-icon"): SVGSVGElement {
  return makeSvgIcon(
    [
      "M3 3h18v18H3z",
      "M9 3v6h6",
      "M9 21v-6",
      "M15 9a2 2 0 1 0 0-4 2 2 0 0 0 0 4z",
      "M9 15a2 2 0 1 0 0-4 2 2 0 0 0 0 4z",
    ],
    size,
    className,
  );
}

function makeSchematicIcon(size = 13): SVGSVGElement {
  return makeSvgIcon(
    [
      "M6 6h12v12H6z",
      "M9 2v4",
      "M15 2v4",
      "M9 18v4",
      "M15 18v4",
      "M2 9h4",
      "M2 15h4",
      "M18 9h4",
      "M18 15h4",
    ],
    size,
  );
}

function makeCatGlyph(size = 15): SVGSVGElement {
  return makeSvgIcon(
    [
      "M4 10V4l4 3h8l4-3v6l-2 9H6z",
      "M9 13h1",
      "M14 13h1",
      "M11 16h2",
    ],
    size,
    "bp-cat-glyph",
  );
}

function makeCheckIcon(size = 11): SVGSVGElement {
  return makeSvgIcon(["M4 12l5 5 11-11"], size);
}

/** Lucide square-pen: the New chat glyph (as GPUI's). */
function makeChatIcon(size = 13): SVGSVGElement {
  return makeSvgIcon(["M12 3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7", "M18.4 2.6a2.1 2.1 0 0 1 3 3L12 15l-4 1 1-4z"], size);
}

/** Lucide folder-input: Move to project. */
function makeMoveIcon(size = 11): SVGSVGElement {
  return makeSvgIcon(["M2 9V5a2 2 0 0 1 2-2h4l2 3h10a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H2", "M2 13h10", "M9 16l3-3-3-3"], size);
}

/** New chat's key as the person reads it. A browser keeps Ctrl+Shift+N for itself, so the web's is Alt+Shift+N. */
export function newChatKeyHint(): string {
  const mac = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform || "");
  return mac ? "⌥⇧N" : "Alt+Shift+N";
}

function makeClockIcon(size = 11): SVGSVGElement {
  return makeSvgIcon(["M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20z", "M12 6v6l4 2"], size);
}

function makePinIcon(size = 11): SVGSVGElement {
  return makeSvgIcon(["M12 17v5", "M8 4h8l-1 5 3 3v2H6v-2l3-3z"], size, "bp-pin-icon");
}

function formatAge(ts: number): string {
  if (!ts) return "";
  const ms = ts < 10_000_000_000 ? ts * 1000 : ts;
  const diffSec = Math.max(0, Math.floor((Date.now() - ms) / 1000));
  if (diffSec < 60) return "now";
  if (diffSec < 3600) return `${Math.floor(diffSec / 60)}m`;
  if (diffSec < 86400) return `${Math.floor(diffSec / 3600)}h`;
  return `${Math.floor(diffSec / 86400)}d`;
}

function botInitials(name: string): string {
  const clean = name.replace(/^@/, "").trim();
  if (!clean) return "?";
  const parts = clean.split(/[\s_-]+/).filter(Boolean);
  if (parts.length >= 2) return (parts[0][0] + parts[1][0]).toUpperCase();
  return clean.slice(0, 2).toUpperCase();
}

function moodClass(mood?: string): string {
  if (mood === "wait" || mood === "error") return "bp-mood-wait";
  if (mood === "work" || mood === "talk") return "bp-mood-work";
  return "bp-mood-idle";
}

function rawThreadId(qid: string): string {
  const idx = qid.lastIndexOf("|");
  return idx >= 0 ? qid.slice(idx + 1) : qid;
}

export class SidebarView {
  private el: HTMLElement;
  private hostLabel: HTMLElement;
  private homeBanner: HTMLElement;
  private crewEl: HTMLElement;
  private filterInput: HTMLInputElement;
  private listEl: HTMLElement;
  private connEl: HTMLElement;
  private filterText = "";
  private allFilter: "all" | "hw" | "sw" = "all";
  private snoozeMenuRowId = "";
  private collapsedProjects = new Set<string>();
  private addShown = false;

  private sections: SidebarSection[] | null = null;
  private projects: ProjectItem[] = [];
  private selectedProjectId = "";
  private threads: ThreadItem[] = [];
  private bots: BotItem[] = [];
  private rooms: RoomItem[] = [];
  private selectedThreadId = "";
  private cb: SidebarCallbacks;
  /** a need's refusal, by row, until its next press */
  private needErr = new Map<string, string>();
  private needBusy = new Set<string>();

  constructor(container: HTMLElement, cb: SidebarCallbacks) {
    this.el = container;
    this.cb = cb;

    const hostRow = document.createElement("div");
    hostRow.className = "bp-side-host";
    const hostLeft = document.createElement("div");
    hostLeft.className = "bp-side-host-left";
    this.hostLabel = document.createElement("span");
    this.hostLabel.textContent = "This computer";
    const chev = document.createElement("span");
    chev.className = "bp-dim";
    chev.textContent = "›";
    hostLeft.append(this.hostLabel, chev);

    const collapseBtn = document.createElement("button");
    collapseBtn.type = "button";
    collapseBtn.className = "icon bp-side-collapse";
    collapseBtn.textContent = "‹";
    collapseBtn.title = "Collapse sidebar";
    collapseBtn.setAttribute("aria-label", "Collapse sidebar");
    collapseBtn.setAttribute("data-testid", "sidebar-collapse");
    collapseBtn.addEventListener("click", () => this.cb.onCollapseSidebar?.());

    hostRow.append(hostLeft, collapseBtn);

    const head = document.createElement("div");
    head.className = "bp-side-head";

    this.filterInput = document.createElement("input");
    this.filterInput.type = "search";
    this.filterInput.className = "bp-input bp-side-search";
    const ph = t("sidebar.search.projects", "Search projects, threads…");
    this.filterInput.placeholder = ph;
    this.filterInput.setAttribute("aria-label", ph);
    this.filterInput.setAttribute("data-testid", "sidebar-filter");
    this.filterInput.addEventListener("input", () => {
      this.filterText = this.filterInput.value.trim().toLowerCase();
      this.render();
    });

    const quickAdd = document.createElement("button");
    quickAdd.type = "button";
    quickAdd.className = "icon bp-side-add";
    quickAdd.textContent = "+";
    quickAdd.title = t("thread.new", "New thread");
    quickAdd.setAttribute("aria-label", t("thread.new", "New thread"));
    quickAdd.setAttribute("data-testid", "sidebar-new-thread");
    quickAdd.addEventListener("click", () => {
      const firstProj = this.firstProjectId();
      if (firstProj) this.cb.onNewThread(firstProj);
    });

    // New chat: a thread in no project, in one press
    const newChat = document.createElement("button");
    newChat.type = "button";
    newChat.className = "icon bp-side-add bp-side-new-chat";
    newChat.appendChild(makeChatIcon(13));
    const chatTip = `${t("chats.new.noproject", "New chat (no project)")} (${newChatKeyHint()})`;
    newChat.title = chatTip;
    newChat.setAttribute("aria-label", t("chats.new.noproject", "New chat (no project)"));
    newChat.setAttribute("data-testid", "sidebar-new-chat");
    newChat.addEventListener("click", () => this.cb.onNewChat?.());

    head.append(this.filterInput, newChat, quickAdd);

    this.homeBanner = document.createElement("div");
    this.homeBanner.className = "bp-side-home bp-hidden";
    this.homeBanner.setAttribute("data-testid", "home-away-banner");

    // Sticky Crew band: Bub + Bots strip + Rooms strip (never scrolls away)
    this.crewEl = document.createElement("div");
    this.crewEl.className = "bp-crew-band bp-hidden";
    this.crewEl.setAttribute("data-testid", "sidebar-crew");

    this.listEl = document.createElement("div");
    this.listEl.className = "bp-side-list";
    this.listEl.setAttribute("data-testid", "sidebar-list");

    const footer = document.createElement("div");
    footer.className = "bp-side-footer";
    this.connEl = document.createElement("span");
    this.connEl.className = "bp-conn-status";
    this.connEl.textContent = "Connecting…";
    this.connEl.setAttribute("data-testid", "conn-status");
    footer.appendChild(this.connEl);

    this.el.append(hostRow, head, this.homeBanner, this.crewEl, this.listEl, footer);
  }

  public setHome(home: HomeInfo | null): void {
    const st = homeBannerState(home);
    if (!st) {
      this.homeBanner.className = "bp-side-home bp-hidden";
      this.homeBanner.replaceChildren();
      return;
    }
    this.homeBanner.className = st.isError ? "bp-side-home bp-bad" : "bp-side-home bp-dim";
    const textEl = document.createElement("span");
    textEl.className = "bp-side-home-text";
    textEl.textContent = st.text;
    if (!st.dismissible) {
      this.homeBanner.replaceChildren(textEl);
      return;
    }
    const dismissBtn = document.createElement("button");
    dismissBtn.type = "button";
    dismissBtn.className = "icon bp-side-home-dismiss";
    dismissBtn.textContent = "×";
    dismissBtn.setAttribute("data-testid", "home-move-dismiss");
    const label = t("approval.dismiss", "Dismiss");
    dismissBtn.title = label;
    dismissBtn.setAttribute("aria-label", label);
    dismissBtn.addEventListener("click", () => this.cb.onDismissHomeMove?.());
    this.homeBanner.replaceChildren(textEl, dismissBtn);
  }

  public setHostName(name: string): void {
    this.hostLabel.textContent = name || "This computer";
  }

  public setConnStatus(text: string, toneClass: string): void {
    this.connEl.textContent = text;
    this.connEl.className = `bp-conn-status ${toneClass}`.trim();
  }

  public setData(projects: ProjectItem[], threads: ThreadItem[], bots: BotItem[], rooms: RoomItem[]): void {
    this.projects = projects;
    this.threads = threads;
    this.bots = bots;
    this.rooms = rooms;
    this.render();
  }

  public setSections(sections: SidebarSection[] | null, bots: BotItem[], rooms: RoomItem[]): void {
    this.sections = sections;
    this.bots = bots;
    this.rooms = rooms;
    this.render();
  }

  public setSelected(threadId: string): void {
    this.selectedThreadId = threadId;
    this.render();
  }

  /** The project whose home is on show ("" for none): its head is marked, and no thread row is. */
  public setSelectedProject(projectId: string): void {
    if (this.selectedProjectId === projectId) return;
    this.selectedProjectId = projectId;
    this.render();
  }

  /** The chevron beside a project's name: it folds and unfolds the project's threads. */
  private makeFoldChevron(folded: boolean, projectId: string): HTMLButtonElement {
    const chev = document.createElement("button");
    chev.type = "button";
    chev.className = "bp-proj-chev" + (folded ? " folded" : "");
    chev.textContent = folded ? "▸" : "▾";
    const label = folded ? t("web.home.unfold", "Show threads") : t("web.home.fold", "Hide threads");
    chev.title = label;
    chev.setAttribute("aria-label", label);
    chev.setAttribute("aria-expanded", folded ? "false" : "true");
    chev.setAttribute("data-testid", `project-fold-${projectId}`);
    return chev;
  }

  public focusFilter(): void {
    this.filterInput.focus();
  }

  private firstProjectId(): string {
    if (this.sections) {
      for (const sec of this.sections) {
        if (sec.kind === "projects" && sec.rows.length > 0) {
          const p = sec.rows[0] as SectionProjectRow;
          if (p.id && !p.off) return p.id;
        }
      }
    }
    return this.projects[0]?.id ?? "";
  }

  public refreshLocale(): void {
    const ph = t("sidebar.search.projects", "Search projects, threads…");
    this.filterInput.placeholder = ph;
    this.filterInput.setAttribute("aria-label", ph);
    this.render();
  }

  private renderCrewBand(q: string): void {
    const uber = this.bots.find((b) => b.uber);
    const otherBots = this.bots.filter((b) => !b.uber && (!q || b.name.toLowerCase().includes(q)));
    const rooms = this.rooms.filter((r) => !q || r.name.toLowerCase().includes(q));
    const showUber = Boolean(uber && (!q || uber.name.toLowerCase().includes(q)));
    const hasBotsRow = otherBots.length > 0 || (Boolean(this.cb.onNewBot) && !q);
    const hasRoomsRow = rooms.length > 0 || (Boolean(this.cb.onNewRoom) && !q);

    if (!showUber && !hasBotsRow && !hasRoomsRow) {
      this.crewEl.classList.add("bp-hidden");
      this.crewEl.replaceChildren();
      return;
    }

    const frag = document.createDocumentFragment();

    if (showUber && uber) {
      const row = document.createElement("button");
      row.type = "button";
      row.className = "bp-crew-uber" + (uber.thread === this.selectedThreadId ? " active" : "");
      row.setAttribute("data-testid", `crew-uber-${uber.id}`);
      const avatar = document.createElement("span");
      avatar.className = `bp-crew-uber-avatar ${moodClass(uber.mood)}`;
      if (uber.color) avatar.style.color = uber.color;
      avatar.appendChild(makeCatGlyph(14));

      const meta = document.createElement("span");
      meta.className = "bp-crew-uber-meta";
      const name = document.createElement("span");
      name.className = "bp-crew-uber-name";
      name.textContent = uber.name;
      meta.appendChild(name);
      if (uber.note) {
        const note = document.createElement("span");
        note.className = `bp-crew-uber-note ${moodClass(uber.mood)}`;
        note.textContent = uber.note;
        meta.appendChild(note);
      }

      row.append(avatar, meta);
      if (uber.mood === "wait" || uber.mood === "error") {
        const badge = document.createElement("span");
        badge.className = "bp-status-pill bp-pill-bad";
        badge.textContent = "? Input";
        row.appendChild(badge);
      }
      row.addEventListener("click", () => this.cb.onSelectBot(uber));
      frag.appendChild(row);
    }

    if (hasBotsRow) {
      const strip = document.createElement("div");
      strip.className = "bp-crew-bots";
      strip.setAttribute("data-testid", "crew-bots");

      const lbl = document.createElement("span");
      lbl.className = "bp-crew-lbl";
      lbl.textContent = t("tour.title.bots", "Bots").toUpperCase();
      strip.appendChild(lbl);

      const tiles = document.createElement("div");
      tiles.className = "bp-crew-bot-tiles";
      for (const bot of otherBots) {
        const tile = document.createElement("button");
        tile.type = "button";
        tile.className = `bp-crew-bot-tile ${moodClass(bot.mood)}` + (bot.thread === this.selectedThreadId ? " active" : "");
        tile.title = bot.note ? `@${bot.name} — ${bot.note}` : `@${bot.name}`;
        tile.setAttribute("aria-label", `@${bot.name}`);
        tile.setAttribute("data-testid", `crew-bot-${bot.id}`);
        if (bot.color) tile.style.setProperty("--bot-c", bot.color);

        const init = document.createElement("span");
        init.className = "bp-crew-bot-init";
        init.textContent = botInitials(bot.name);
        tile.appendChild(init);

        if (bot.mood === "wait" || bot.mood === "error") {
          const waitDot = document.createElement("span");
          waitDot.className = "bp-crew-bot-wait";
          tile.appendChild(waitDot);
        }
        tile.addEventListener("click", () => this.cb.onSelectBot(bot));
        tiles.appendChild(tile);
      }
      if (this.cb.onNewBot && !q) {
        const add = document.createElement("button");
        add.type = "button";
        add.className = "bp-crew-bot-tile bp-crew-add";
        add.textContent = "+";
        add.title = t("bots.new", "New bot");
        add.setAttribute("aria-label", t("bots.new", "New bot"));
        add.setAttribute("data-testid", "crew-new-bot");
        add.addEventListener("click", () => this.cb.onNewBot?.());
        tiles.appendChild(add);
      }
      strip.appendChild(tiles);
      frag.appendChild(strip);
    }

    if (hasRoomsRow) {
      const strip = document.createElement("div");
      strip.className = "bp-crew-rooms";
      strip.setAttribute("data-testid", "crew-rooms");
      for (const room of rooms) {
        const chip = document.createElement("button");
        chip.type = "button";
        chip.className = "bp-crew-room-chip";
        chip.setAttribute("data-testid", `crew-room-${room.id}`);
        const hash = document.createElement("span");
        hash.className = "bp-dim";
        hash.textContent = "#";
        const name = document.createElement("span");
        name.textContent = room.name;
        chip.append(hash, name);
        if (room.members !== undefined && room.members > 0) {
          const cnt = document.createElement("span");
          cnt.className = "bp-crew-room-cnt";
          cnt.textContent = String(room.members);
          chip.appendChild(cnt);
        }
        chip.title = room.members ? `#${room.name} (${room.members})` : `#${room.name}`;
        chip.addEventListener("click", () => this.cb.onSelectRoom?.(room));
        strip.appendChild(chip);
      }
      if (this.cb.onNewRoom && !q) {
        const add = document.createElement("button");
        add.type = "button";
        add.className = "bp-crew-room-chip bp-crew-add";
        add.textContent = "+";
        add.title = t("rooms.new", "New room");
        add.setAttribute("aria-label", t("rooms.new", "New room"));
        add.setAttribute("data-testid", "crew-new-room");
        add.addEventListener("click", () => this.cb.onNewRoom?.());
        strip.appendChild(add);
      }
      frag.appendChild(strip);
    }

    this.crewEl.classList.remove("bp-hidden");
    this.crewEl.replaceChildren(frag);
  }

  /** A projects heading with Add project's + at its right (the first projects heading only). */
  private projectsHead(title: string): HTMLElement {
    const head = document.createElement("div");
    head.className = "bp-side-section-head";
    if (!this.cb.onAddProject || this.addShown) {
      head.textContent = title;
      return head;
    }
    this.addShown = true;
    head.classList.add("bp-side-section-row");
    const lbl = document.createElement("span");
    lbl.textContent = title;
    const add = document.createElement("button");
    add.type = "button";
    add.className = "bp-side-sec-plus bp-side-sec-btn";
    add.textContent = "+";
    const tip = t("sidebar.add.project", "Add project");
    add.title = tip;
    add.setAttribute("aria-label", tip);
    add.setAttribute("data-testid", "sidebar-add-project");
    add.addEventListener("click", (e) => {
      e.stopPropagation();
      this.cb.onAddProject?.();
    });
    head.append(lbl, add);
    return head;
  }

  private renderStatusPill(pill: StatusPill, say?: string): HTMLElement {
    const el = document.createElement("span");
    el.className = `bp-status-pill bp-pill-${pill.tone}`;
    el.textContent = `${pill.icon} ${pill.label}`;
    if (say) el.title = say;
    return el;
  }

  private renderRowActions(row: SectionThreadRow): HTMLElement | null {
    if (row.off) return null;
    const hasSnooze = Array.isArray(row.snooze) && row.snooze.length > 0 && Boolean(this.cb.onSnoozeRow);
    const hasWake = Boolean(row.wake) && Boolean(this.cb.onSnoozeRow);
    const isSwarm = row.kind === "swarm";
    const hasSettle = Boolean(row.settle) && (!isSwarm || Boolean(this.cb.onSettleSwarm));
    const hasMove = Boolean(row.chat) && Boolean(this.cb.onMoveToProject);
    if (!hasSnooze && !hasWake && !hasSettle && !hasMove) return null;

    const acts = document.createElement("span");
    acts.className = "bp-row-acts";

    if (hasWake && row.wake) {
      const wakeBtn = document.createElement("button");
      wakeBtn.type = "button";
      wakeBtn.className = "bp-row-act-btn bp-wake-btn";
      wakeBtn.textContent = t("menu.wake", "Wake");
      wakeBtn.title = t("menu.wake", "Wake snoozed thread");
      wakeBtn.setAttribute("data-testid", `wake-row-${row.id}`);
      wakeBtn.addEventListener("click", (e) => {
        e.stopPropagation();
        this.cb.onSnoozeRow?.(row.wake!);
      });
      acts.appendChild(wakeBtn);
    }

    if (hasSnooze && row.snooze) {
      const snBtn = document.createElement("button");
      snBtn.type = "button";
      snBtn.className = "bp-row-act-btn bp-snooze-btn";
      snBtn.title = t("menu.snooze", "Snooze thread");
      snBtn.setAttribute("aria-label", t("menu.snooze", "Snooze thread"));
      snBtn.setAttribute("data-testid", `snooze-row-${row.id}`);
      snBtn.appendChild(makeClockIcon(11));
      snBtn.addEventListener("click", (e) => {
        e.stopPropagation();
        this.snoozeMenuRowId = this.snoozeMenuRowId === row.id ? "" : row.id;
        this.render();
      });
      acts.appendChild(snBtn);
    }

    if (hasMove) {
      const moveBtn = document.createElement("button");
      moveBtn.type = "button";
      moveBtn.className = "bp-row-act-btn bp-move-btn";
      moveBtn.title = t("chats.move", "Move to project…");
      moveBtn.setAttribute("aria-label", t("chats.move", "Move to project…"));
      moveBtn.setAttribute("data-testid", `move-row-${row.id}`);
      moveBtn.appendChild(makeMoveIcon(11));
      moveBtn.addEventListener("click", (e) => {
        e.stopPropagation();
        this.cb.onMoveToProject?.(rawThreadId(row.id));
      });
      acts.appendChild(moveBtn);
    }

    if (hasSettle && row.settle) {
      const settleBtn = document.createElement("button");
      settleBtn.type = "button";
      settleBtn.className = "bp-row-act-btn bp-settle-btn";
      settleBtn.title = t("menu.settle", "Settle");
      settleBtn.setAttribute("aria-label", t("menu.settle", "Settle"));
      settleBtn.setAttribute("data-testid", `settle-row-${row.id}`);
      settleBtn.appendChild(makeCheckIcon(11));
      settleBtn.addEventListener("click", (e) => {
        e.stopPropagation();
        if (isSwarm) this.cb.onSettleSwarm?.(rawThreadId(row.settle!));
        else this.cb.onThreadAct(rawThreadId(row.settle!), "settle");
      });
      acts.appendChild(settleBtn);
    }

    return acts;
  }

  private renderSectionThreadRow(row: SectionThreadRow, showTreeBranch: boolean, isInbox = false): HTMLElement {
    const el = document.createElement("div");
    el.className = "bp-thread-row" +
      (isInbox ? " bp-inbox-row" : "") +
      (!this.selectedProjectId && (row.id === this.selectedThreadId || rawThreadId(row.id) === this.selectedThreadId) ? " active" : "") +
      (row.off ? " bp-dim" : "");
    el.setAttribute("data-thread-id", row.id);
    el.setAttribute("data-testid", `thread-row-${row.id}`);

    const pill = statusPillOf(row);
    const acts = this.renderRowActions(row);

    if (isInbox) {
      // 2-line Inbox row:
      // Line 1: Full-width Title + hover Snooze/Wake/Settle actions
      // Line 2: Explicit [Status Pill] + Project name + right-aligned age
      const main = document.createElement("div");
      main.className = "bp-inbox-main";

      const line1 = document.createElement("div");
      line1.className = "bp-inbox-top";
      const title = document.createElement("div");
      title.className = "bp-thread-title";
      title.textContent = row.title || "Untitled thread";
      line1.appendChild(title);
      if (acts) line1.appendChild(acts);

      const line2 = document.createElement("div");
      line2.className = "bp-inbox-meta";
      if (pill) line2.appendChild(this.renderStatusPill(pill, row.say));
      if (row.project && !row.chat) {
        const proj = document.createElement("span");
        proj.className = "bp-inbox-proj";
        proj.textContent = row.project;
        line2.appendChild(proj);
      }
      if (row.ago) {
        const age = document.createElement("span");
        age.className = "bp-thread-age";
        age.textContent = row.ago;
        if (row.pinned) age.title = t("tools.pinned", "Pinned");
        line2.appendChild(age);
      }

      main.append(line1, line2);
      el.appendChild(main);
    } else {
      // Nested Bench project row: optional tree branch + explicit compact status pill + title + hover actions + age
      if (showTreeBranch) {
        const branch = document.createElement("span");
        branch.className = "bp-tree-branch";
        branch.setAttribute("aria-hidden", "true");
        el.appendChild(branch);
      }
      if (pill) el.appendChild(this.renderStatusPill(pill, row.say));

      const title = document.createElement("div");
      title.className = "bp-thread-title";
      title.textContent = row.title || "Untitled thread";
      el.appendChild(title);

      if (acts) el.appendChild(acts);

      if (row.ago) {
        const age = document.createElement("span");
        age.className = "bp-thread-age";
        age.textContent = row.ago;
        if (row.pinned) age.title = t("tools.pinned", "Pinned");
        el.appendChild(age);
      }
    }

    const target = row.nav || row.id;
    if (!row.off && target && !target.startsWith("@room:")) {
      el.addEventListener("click", () => this.cb.onSelectThread(rawThreadId(target)));
    }

    const showSnoozeBar = this.snoozeMenuRowId === row.id && Array.isArray(row.snooze) && row.snooze.length > 0;
    const need = row.kind === "swarm" ? parseNeed(row.need) : null;
    if (!need && !showSnoozeBar) return el;

    const wrap = document.createElement("div");
    wrap.className = "bp-need-wrap";
    wrap.appendChild(el);

    // Inline Snooze duration picker (1 hour · Tomorrow · 1 week)
    if (showSnoozeBar && row.snooze) {
      const bar = document.createElement("div");
      bar.className = "bp-snooze-bar" + (showTreeBranch ? " nested" : "");
      bar.setAttribute("data-testid", `snooze-bar-${row.id}`);
      const lbl = document.createElement("span");
      lbl.className = "bp-dim";
      lbl.textContent = "Snooze:";
      bar.appendChild(lbl);
      for (const opt of row.snooze) {
        const b = document.createElement("button");
        b.type = "button";
        b.className = "bp-snooze-opt";
        b.textContent = opt.label;
        b.addEventListener("click", (e) => {
          e.stopPropagation();
          this.snoozeMenuRowId = "";
          this.cb.onSnoozeRow?.(opt.value);
          this.render();
        });
        bar.appendChild(b);
      }
      wrap.appendChild(bar);
    }

    if (need) {
      const line = document.createElement("div");
      line.className = "bp-need" + (showTreeBranch ? " nested" : "") + (row.off ? " bp-dim" : "");
      line.setAttribute("data-testid", `need-${row.id}`);
      const text = document.createElement("span");
      text.className = "bp-need-line";
      text.textContent = need.line;
      text.title = need.line;
      line.appendChild(text);
      for (const a of need.actions) {
        if (!needPress(a, Boolean(this.cb.hasRoomView))) continue;
        const b = document.createElement("button");
        b.type = "button";
        b.className = "bp-need-btn";
        b.textContent = a.label;
        b.setAttribute("data-testid", `need-${a.act}-${row.id}`);
        b.disabled = Boolean(row.off) || this.needBusy.has(row.id) || !this.cb.onNeedAct;
        b.addEventListener("click", (e) => {
          e.stopPropagation();
          const act = this.cb.onNeedAct;
          if (!act) return;
          this.needErr.delete(row.id);
          this.needBusy.add(row.id);
          this.render();
          act(a)
            .catch((err: unknown) => { this.needErr.set(row.id, err instanceof Error ? err.message : String(err)); })
            .finally(() => { this.needBusy.delete(row.id); this.render(); });
        });
        line.appendChild(b);
      }
      wrap.appendChild(line);
      const err = this.needErr.get(row.id);
      if (err) {
        const e = document.createElement("div");
        e.className = "bp-need-err" + (showTreeBranch ? " nested" : "");
        e.textContent = err;
        wrap.appendChild(e);
      }
    }
    return wrap;
  }

  private renderHubSections(frag: DocumentFragment, q: string): void {
    const sections = this.sections ?? [];
    const listedThreadIds = new Set<string>();
    for (const sec of sections) {
      if (sec.kind === "threads") {
        for (const r of sec.rows as SectionThreadRow[]) {
          listedThreadIds.add(r.id);
          listedThreadIds.add(rawThreadId(r.id));
        }
      } else if (sec.kind === "projects") {
        for (const p of sec.rows as SectionProjectRow[]) {
          for (const th of p.threads ?? []) {
            listedThreadIds.add(th.id);
            listedThreadIds.add(rawThreadId(th.id));
          }
        }
      }
    }
    const selectedMeta = this.selectedThreadId
      ? this.threads.find((x) => x.id === this.selectedThreadId) ?? null
      : null;

    let anyProjectRows = false;
    for (const sec of sections) {
      if (sec.kind === "projects" && (sec.rows.length > 0 || (sec.count ?? 0) > 0)) anyProjectRows = true;
    }

    for (const sec of sections) {
      if (sec.kind === "bub") continue;

      if (sec.kind === "threads") {
        const isFoldable = Boolean(sec.fold && sec.toggle);
        const rows = (sec.rows as SectionThreadRow[]).filter((r) => {
          if (!q) return true;
          return `${r.title} ${r.project ?? ""}`.toLowerCase().includes(q);
        });
        if (!isFoldable && rows.length === 0) continue;
        if (isFoldable && q && rows.length === 0) continue;
        if (isFoldable && !q && (sec.count ?? rows.length) === 0) continue;

        const secWrap = document.createElement("section");
        secWrap.className = `bp-side-section bp-side-inbox bp-sec-${sec.id}`;
        secWrap.setAttribute("data-section-id", sec.id);

        if (isFoldable) {
          const headBtn = document.createElement("button");
          headBtn.type = "button";
          headBtn.className = "bp-side-section-head bp-side-section-toggle";
          const count = q ? rows.length : (sec.count ?? rows.length);
          const arrow = sec.open || Boolean(q) ? "▾" : "▸";
          headBtn.textContent = `${arrow} ${sec.title.toUpperCase()} (${count})`;
          if (sec.hint) headBtn.title = sec.hint;
          headBtn.setAttribute("data-testid", `section-toggle-${sec.id}`);
          headBtn.addEventListener("click", () => {
            if (sec.toggle) this.cb.onToggleSection?.(sec.toggle);
          });
          secWrap.appendChild(headBtn);
        } else {
          const head = document.createElement("div");
          head.className = "bp-side-section-head" +
            (sec.id === "needs" ? " bp-head-needs" : sec.id === "running" ? " bp-head-running" : "");
          if (sec.id === CHATS && this.cb.onNewChat) {
            // Chats (the threads in no project): its + starts one
            head.classList.add("bp-side-section-row");
            const lbl = document.createElement("span");
            lbl.textContent = `${t("chats.title", sec.title).toUpperCase()} ${rows.length}`;
            const add = document.createElement("button");
            add.type = "button";
            add.className = "bp-side-sec-plus bp-side-sec-btn";
            add.textContent = "+";
            add.title = t("chats.new.noproject", "New chat (no project)");
            add.setAttribute("aria-label", add.title);
            add.setAttribute("data-testid", "section-new-chat");
            add.addEventListener("click", (e) => { e.stopPropagation(); this.cb.onNewChat?.(); });
            head.append(lbl, add);
          } else {
            head.textContent = `${sec.title.toUpperCase()} ${rows.length}`;
          }
          secWrap.appendChild(head);
        }

        if (!isFoldable || Boolean(sec.open) || Boolean(q)) {
          for (const r of rows) {
            secWrap.appendChild(this.renderSectionThreadRow(r, false, true));
          }
        }
        frag.appendChild(secWrap);
        continue;
      }

      if (sec.kind === "projects") {
        const rawRows = sec.rows as SectionProjectRow[];
        let projRows = rawRows.filter((p) => {
          if (!q) return true;
          if ((p.title || p.id).toLowerCase().includes(q)) return true;
          if ((p.threads ?? []).some((th) => th.title.toLowerCase().includes(q))) return true;
          if ((p.swarms ?? []).some((sw) => sw.title.toLowerCase().includes(q))) return true;
          return false;
        });

        const isFoldable = Boolean(sec.fold && sec.toggle);
        if (q && projRows.length === 0) continue;
        if (!isFoldable && sec.id !== "active" && projRows.length === 0) continue;

        const secWrap = document.createElement("section");
        secWrap.className = `bp-side-section bp-sec-${sec.id}`;
        secWrap.setAttribute("data-section-id", sec.id);

        if (isFoldable) {
          const headBtn = document.createElement("button");
          headBtn.type = "button";
          headBtn.className = "bp-side-section-head bp-side-section-toggle";
          const count = q ? projRows.length : (sec.count ?? projRows.length);
          const arrow = sec.open || Boolean(q) ? "▾" : "▸";
          headBtn.textContent = `${arrow} ${sec.title.toUpperCase()} (${count})`;
          if (sec.hint) headBtn.title = sec.hint;
          headBtn.setAttribute("data-testid", `section-toggle-${sec.id}`);
          headBtn.addEventListener("click", () => {
            if (sec.toggle) this.cb.onToggleSection?.(sec.toggle);
          });
          secWrap.appendChild(headBtn);
        } else {
          secWrap.appendChild(this.projectsHead(sec.title.toUpperCase()));
        }

        const isOpen = !isFoldable || Boolean(sec.open) || Boolean(q);
        if (isOpen) {
          if (sec.id === "all" && !q) {
            const hasHw = rawRows.some((p) => p.hasBoard || p.hasSchematic);
            const hasSw = rawRows.some((p) => !p.hasBoard && !p.hasSchematic);
            if (hasHw && hasSw) {
              const bar = document.createElement("div");
              bar.className = "bp-proj-filter";
              bar.setAttribute("data-testid", "all-projects-filter");
              const opts: Array<{ id: "all" | "hw" | "sw"; label: string }> = [
                { id: "all", label: "All" },
                { id: "hw", label: "Boards" },
                { id: "sw", label: "Code" },
              ];
              for (const opt of opts) {
                const btn = document.createElement("button");
                btn.type = "button";
                btn.className = "bp-proj-filter-btn" + (this.allFilter === opt.id ? " active" : "");
                btn.textContent = opt.label;
                btn.addEventListener("click", () => {
                  this.allFilter = opt.id;
                  this.render();
                });
                bar.appendChild(btn);
              }
              secWrap.appendChild(bar);
              if (this.allFilter === "hw") projRows = projRows.filter((p) => p.hasBoard || p.hasSchematic);
              else if (this.allFilter === "sw") projRows = projRows.filter((p) => !p.hasBoard && !p.hasSchematic);
            }
          }

          if (sec.id === "active" && !anyProjectRows && !q) {
            const empty = document.createElement("div");
            empty.className = "bp-side-empty";
            empty.textContent = "No projects yet.";
            secWrap.appendChild(empty);
          }

          for (const proj of projRows) {
            const group = document.createElement("div");
            group.className = "bp-proj-group" + (proj.off || sec.id === "quiet" ? " bp-dim" : "");
            group.setAttribute("data-project-id", proj.id);
            if (proj.nav || proj.key) group.setAttribute("data-project-key", proj.nav || proj.key || "");

            const header = document.createElement("div");
            header.className = "bp-proj-head";

            const toggle = document.createElement("button");
            toggle.type = "button";
            toggle.className = "bp-proj-toggle";
            const isHw = Boolean(proj.hasBoard || proj.hasSchematic);
            const icon = isHw ? makeBoardIcon(14) : makeFolderIcon();
            if (proj.css) icon.style.color = proj.css;
            const nameSpan = document.createElement("span");
            nameSpan.className = "bp-proj-name";
            nameSpan.textContent = proj.title || proj.id;
            toggle.append(icon, nameSpan);
            const canFold = proj.fold === "project-fold" && Boolean(proj.nav);
            const fold = (): void => { if (canFold) this.cb.onToggleProjectFold?.(proj.nav!); };
            // the name opens the project's home; the chevron beside it folds its threads (an older hub has no home: the name folds)
            if (!proj.off && this.cb.canOpenProject?.()) toggle.title = t("web.home.open", "Open project home");
            toggle.setAttribute("data-testid", `project-open-${proj.id}`);
            if (this.selectedProjectId && rawThreadId(proj.id) === this.selectedProjectId) header.classList.add("active");
            toggle.addEventListener("click", () => {
              if (proj.off || !this.cb.onOpenProject?.(proj.id)) fold();
            });
            const chev = this.makeFoldChevron(Boolean(proj.folded), proj.id);
            if (canFold) chev.addEventListener("click", (e) => { e.stopPropagation(); fold(); });
            else chev.disabled = true;
            header.appendChild(chev);

            const meta = document.createElement("div");
            meta.className = "bp-proj-meta";
            if ((proj.needsCount ?? 0) > 0) {
              const nb = document.createElement("span");
              nb.className = "bp-proj-badge-need";
              nb.textContent = String(proj.needsCount);
              nb.title = `${proj.needsCount} needing you`;
              meta.appendChild(nb);
            }
            if ((proj.running ?? 0) > 0) {
              const rb = document.createElement("span");
              rb.className = "bp-proj-badge-run";
              rb.textContent = String(proj.running);
              rb.title = `${proj.running} running`;
              meta.appendChild(rb);
            }
            if (proj.pinned && sec.id !== "pinned") {
              meta.appendChild(makePinIcon(11));
            }

            const acts = document.createElement("div");
            acts.className = "bp-proj-acts";
            if (!proj.off && this.cb.onQuickOpenViewer) {
              if (proj.hasBoard) {
                const bBtn = document.createElement("button");
                bBtn.type = "button";
                bBtn.className = "bp-proj-quick";
                bBtn.title = t("viewer.board", "Open Board");
                bBtn.setAttribute("aria-label", `Open Board (${proj.title})`);
                bBtn.setAttribute("data-testid", `quick-board-${proj.id}`);
                bBtn.appendChild(makeBoardIcon(13, ""));
                bBtn.addEventListener("click", (e) => {
                  e.stopPropagation();
                  this.cb.onQuickOpenViewer?.(rawThreadId(proj.id), "board");
                });
                acts.appendChild(bBtn);
              }
              if (proj.hasSchematic) {
                const sBtn = document.createElement("button");
                sBtn.type = "button";
                sBtn.className = "bp-proj-quick";
                sBtn.title = t("viewer.schematic", "Open Schematic");
                sBtn.setAttribute("aria-label", `Open Schematic (${proj.title})`);
                sBtn.setAttribute("data-testid", `quick-sch-${proj.id}`);
                sBtn.appendChild(makeSchematicIcon(13));
                sBtn.addEventListener("click", (e) => {
                  e.stopPropagation();
                  this.cb.onQuickOpenViewer?.(rawThreadId(proj.id), "schematic");
                });
                acts.appendChild(sBtn);
              }
            }

            const addBtn = document.createElement("button");
            addBtn.type = "button";
            addBtn.className = "icon bp-proj-add";
            addBtn.title = t("thread.new", "New thread");
            addBtn.textContent = "+";
            addBtn.setAttribute("data-testid", `new-thread-${proj.id}`);
            if (proj.off) addBtn.disabled = true;
            addBtn.addEventListener("click", (e) => {
              e.stopPropagation();
              this.cb.onNewThread(rawThreadId(proj.id));
            });
            acts.appendChild(addBtn);

            header.append(toggle, meta, acts);
            group.appendChild(header);

            const projNameMatches = Boolean(q) && (proj.title || proj.id).toLowerCase().includes(q);
            const isProjFolded = Boolean(proj.folded && !q);
            for (const sw of proj.swarms ?? []) {
              if (!isSwarmVisibleUnderProject(sw, isProjFolded)) continue;
              if (q && !projNameMatches && !sw.title.toLowerCase().includes(q)) continue;
              group.appendChild(this.renderSectionThreadRow(sw, true));
            }
            if (!isProjFolded) {
              for (const th of proj.threads ?? []) {
                if (q && !projNameMatches && !th.title.toLowerCase().includes(q)) continue;
                group.appendChild(this.renderSectionThreadRow(th, true));
              }
              if (
                selectedMeta &&
                selectedMeta.project === rawThreadId(proj.id) &&
                !listedThreadIds.has(selectedMeta.id) &&
                (!q || projNameMatches || selectedMeta.title.toLowerCase().includes(q))
              ) {
                group.appendChild(this.renderSectionThreadRow({
                  id: selectedMeta.id,
                  title: selectedMeta.title,
                  status: selectedMeta.status,
                  state: selectedMeta.state,
                  ago: formatAge(selectedMeta.updatedAt),
                  pinned: selectedMeta.pinned,
                  nav: selectedMeta.id,
                }, true));
              }
            }

            secWrap.appendChild(group);
          }
        }

        frag.appendChild(secWrap);
      }
    }
  }

  public render(): void {
    this.addShown = false;
    const q = this.filterText;

    this.renderCrewBand(q);

    const frag = document.createDocumentFragment();

    if (this.sections !== null) {
      this.renderHubSections(frag, q);
    } else {
      const byProject = new Map<string, ThreadItem[]>();
      for (const th of this.threads) {
        if (th.archived || th.swarm) continue;
        if (q) {
          const hay = `${th.title} ${th.preview ?? ""}`.toLowerCase();
          if (!hay.includes(q)) continue;
        }
        const list = byProject.get(th.project) ?? [];
        list.push(th);
        byProject.set(th.project, list);
      }

      frag.appendChild(this.projectsHead("RECENT"));

      if (this.projects.length === 0 && this.threads.length === 0) {
        const empty = document.createElement("div");
        empty.className = "bp-side-empty";
        empty.textContent = "No projects yet.";
        frag.appendChild(empty);
      }

      for (const proj of this.projects) {
        const pThreads = byProject.get(proj.id) ?? [];
        if (q && pThreads.length === 0 && !proj.title.toLowerCase().includes(q)) continue;

        const sec = document.createElement("section");
        sec.className = "bp-proj-group";
        sec.setAttribute("data-project-id", proj.id);

        const header = document.createElement("div");
        header.className = "bp-proj-head";

        const toggle = document.createElement("button");
        toggle.type = "button";
        toggle.className = "bp-proj-toggle";
        const collapsed = this.collapsedProjects.has(proj.id) && !q;
        const folder = makeFolderIcon();
        if (proj.css) folder.style.color = proj.css;
        const nameSpan = document.createElement("span");
        nameSpan.className = "bp-proj-name";
        nameSpan.textContent = proj.title || proj.id;
        toggle.append(folder, nameSpan);
        const fold = (): void => {
          if (this.collapsedProjects.has(proj.id)) this.collapsedProjects.delete(proj.id);
          else this.collapsedProjects.add(proj.id);
          this.render();
        };
        if (this.cb.canOpenProject?.()) toggle.title = t("web.home.open", "Open project home");
        toggle.setAttribute("data-testid", `project-open-${proj.id}`);
        if (this.selectedProjectId && proj.id === this.selectedProjectId) header.classList.add("active");
        toggle.addEventListener("click", () => {
          if (!this.cb.onOpenProject?.(proj.id)) fold();
        });
        const chev = this.makeFoldChevron(collapsed, proj.id);
        chev.addEventListener("click", (e) => { e.stopPropagation(); fold(); });
        header.appendChild(chev);

        const addBtn = document.createElement("button");
        addBtn.type = "button";
        addBtn.className = "icon bp-proj-add";
        addBtn.title = t("thread.new", "New thread");
        addBtn.textContent = "+";
        addBtn.setAttribute("data-testid", `new-thread-${proj.id}`);
        addBtn.addEventListener("click", (e) => {
          e.stopPropagation();
          this.cb.onNewThread(proj.id);
        });

        header.append(toggle, addBtn);
        sec.appendChild(header);

        if (!collapsed) {
          for (const th of pThreads) {
            sec.appendChild(this.renderSectionThreadRow({
              id: th.id,
              title: th.title,
              status: th.status,
              state: th.state,
              ago: formatAge(th.updatedAt),
              pinned: th.pinned,
              nav: th.id,
            }, true));
          }
        }

        frag.appendChild(sec);
      }
    }

    this.listEl.replaceChildren(frag);
  }
}
