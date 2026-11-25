// Backplane Web UI main shell matching GPUI native-chat.png:
// Unified top bar (breadcrumb, STATE dot+word, Pin/Settle with icons, command box, gear),
// sidebar (This computer, ACTIVE/BOTS/ROOMS, bottom-left Connected status),
// bottom-anchored chat timeline, tall composer + toolbar row, hub-driven right rail
// with PR #1367 `has` reconciliation + `railGen` staleness guard, and Settings › Devices modal.
import { AddProjectDialog } from "./addproject";
import { ChatView, type EntryItem, type ReviewBar, type UnansweredNotice } from "./chat";
import { ComposerView, type MentionItem, type SlashCommand } from "./composer";
import { onLocaleChange, setLocale, t } from "./i18n";
import { signOut } from "./logout";
import { MgmtStrip } from "./mgmt";
import { CommandPalette, type PaletteAction, type PaletteSearchResult } from "./palette";
import { ProjectHomeView, type HomeThreadRow } from "./project-home";
import { parseProjectHome, rawProjectId } from "./project-home-model";
import { isLocalNoProviderError, parseNoProvider, type NoProviderNotice } from "./provider-notice";
import { parseRingList } from "./ring-model";
import { RailView, type RailEntry } from "./rail";
import { needAfter, needPress } from "./swarm-need";
import { SettingsModal, type SettingsRow } from "./settings";
import {
  parseLocalePref,
  parseThemePref,
  resolveTheme,
  type WebLocalePref,
  type WebThemePref,
} from "./settings-model";
import { ThreadContextDialog } from "./thread-context";
import {
  CHATS,
  SidebarView,
  clearSidebarCache,
  loadSidebarCache,
  moveTargets,
  saveSidebarCache,
  type BotItem,
  type PlanModeInfo,
  type ProjectItem,
  type RoomItem,
  type SectionProjectRow,
  type SectionThreadRow,
  type SidebarSection,
  type ThreadItem,
} from "./sidebar";
import { VoiceController } from "./voice";
import { HubClient, HubRpcError, type DataPush, type HelloFrame, type HomeInfo } from "./ws";
// eslint-disable-next-line @typescript-eslint/ban-ts-comment
// @ts-ignore resolved by web/build.ts to ./viewers/viewer-pane.ts when present
import { mountViewerPane } from "virtual:viewer-pane";

export interface RailHas {
  board?: boolean;
  schematic?: boolean;
  mech?: boolean;
  gerbers?: boolean;
}

export interface ViewerHostApi {
  hub: HubClient;
  getThreadId: () => string;
  closeViewer: () => void;
}

type ViewerMountFn = (el: HTMLElement, api: ViewerHostApi) => {
  open: (kind: string, threadId: string) => void;
  close: () => void;
  reconcileHas?: (has: RailHas) => void;
};

type SubId = number | string;

const SVG_NS = "http://www.w3.org/2000/svg";

function makeMiniIcon(paths: string[]): SVGSVGElement {
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("width", "13");
  svg.setAttribute("height", "13");
  svg.setAttribute("fill", "none");
  svg.setAttribute("stroke", "currentColor");
  svg.setAttribute("stroke-width", "1.8");
  svg.setAttribute("aria-hidden", "true");
  for (const d of paths) {
    const p = document.createElementNS(SVG_NS, "path");
    p.setAttribute("d", d);
    svg.appendChild(p);
  }
  return svg;
}

function stateDotClass(th: ThreadItem | null): string {
  if (!th) return "bp-dot-idle";
  const s = th.status || th.state;
  if (s === "active" || s === "working") return "bp-dot-work";
  if (s === "finished") return "bp-dot-done";
  if (s === "waiting" || s === "asking" || s === "blocked" || s === "failed" || (th.waiting ?? 0) > 0) return "bp-dot-need";
  return "bp-dot-idle";
}

/** A thread's age as the sidebar writes it: now, 5m, 3h, 2d. */
function ageText(ts: number): string {
  if (!ts) return "";
  const ms = ts < 10_000_000_000 ? ts * 1000 : ts;
  const sec = Math.max(0, Math.floor((Date.now() - ms) / 1000));
  if (sec < 60) return "now";
  if (sec < 3600) return `${Math.floor(sec / 60)}m`;
  if (sec < 86400) return `${Math.floor(sec / 3600)}h`;
  return `${Math.floor(sec / 86400)}d`;
}

function hasForKind(has: RailHas | undefined, kind: string): boolean {
  if (!has) return true;
  if (kind === "board" || kind === "3d") return has.board !== false;
  if (kind === "schematic") return has.schematic !== false;
  if (kind === "mech") return has.mech !== false;
  if (kind === "gerbers") return has.gerbers !== false;
  return true;
}

let hubTheme: "dark" | "light" = "dark";
let themePref: WebThemePref = (() => {
  try { return parseThemePref(localStorage.getItem("bp.theme.pref")); } catch { return "auto"; }
})();
let hubLocale: string | null = null;
let localePref: WebLocalePref = (() => {
  try { return parseLocalePref(localStorage.getItem("bp.locale")); } catch { return "auto"; }
})();

function applyEffectiveTheme(): void {
  const resolved = resolveTheme(themePref, hubTheme);
  document.documentElement.classList.toggle("light", resolved === "light");
  try { localStorage.setItem("bp.theme", resolved); } catch { /* ignore */ }
}

function applyHubTheme(theme: string | undefined): void {
  if (!theme) return;
  hubTheme = theme === "light" ? "light" : "dark";
  applyEffectiveTheme();
}

function applyEffectiveLocale(): void {
  if (localePref !== "auto") {
    setLocale(localePref);
  } else if (hubLocale) {
    setLocale(hubLocale);
  } else if (typeof navigator !== "undefined" && navigator.language) {
    if (!setLocale(navigator.language)) setLocale("en");
  }
}

function makeClientId(): string {
  return `web-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
}

function mountApp(): void {
  const root = document.getElementById("root");
  if (!root) return;

  try {
    if (localStorage.getItem("bp.theme") === "light") hubTheme = "light";
    if (resolveTheme(themePref, hubTheme) === "light") document.documentElement.classList.add("light");
    if (localePref !== "auto") setLocale(localePref);
  } catch { /* ignore */ }

  // Unified GPUI-style top bar
  const topbar = document.createElement("header");
  topbar.className = "bp-topbar";
  topbar.setAttribute("data-testid", "topbar");

  const sideToggle = document.createElement("button");
  sideToggle.type = "button";
  sideToggle.className = "icon bp-side-toggle";
  sideToggle.textContent = "☰";
  sideToggle.title = "Toggle sidebar";
  sideToggle.setAttribute("aria-label", "Toggle sidebar");

  const brand = document.createElement("div");
  brand.className = "bp-brand";
  const logo = document.createElement("img");
  logo.src = "/assets/favicon.svg";
  logo.alt = "Backplane";
  logo.className = "bp-logo";
  const brandName = document.createElement("span");
  brandName.className = "bp-brand-name bp-hide-phone";
  brandName.textContent = "Backplane";
  const sep1 = document.createElement("span");
  sep1.className = "bp-crumb-sep bp-hide-phone";
  sep1.textContent = "/";
  const projCrumb = document.createElement("span");
  projCrumb.className = "bp-crumb-proj bp-hide-phone";
  const sep2 = document.createElement("span");
  sep2.className = "bp-crumb-sep bp-hide-phone";
  sep2.textContent = "/";
  const threadCrumb = document.createElement("span");
  threadCrumb.className = "bp-crumb-thread";
  threadCrumb.setAttribute("data-testid", "thread-title");
  const planTopBadge = document.createElement("span");
  planTopBadge.className = "bp-plan-top-badge bp-hidden";
  planTopBadge.setAttribute("data-testid", "plan-top-badge");
  brand.append(logo, brandName, sep1, projCrumb, sep2, threadCrumb, planTopBadge);

  // STATE indicator (desktop)
  const stateBox = document.createElement("div");
  stateBox.className = "bp-top-state bp-hide-phone bp-hidden";
  const stateLbl = document.createElement("span");
  stateLbl.className = "bp-top-state-lbl";
  stateLbl.textContent = "STATE";
  const stateDot = document.createElement("span");
  stateDot.className = "bp-dot bp-dot-idle";
  const stateWord = document.createElement("span");
  stateWord.className = "bp-top-state-word";
  stateBox.append(stateLbl, stateDot, stateWord);

  // Thread action buttons in top bar (Pin, Settle, ▾ menu)
  const topActs = document.createElement("div");
  topActs.className = "bp-top-acts bp-hide-phone bp-hidden";

  const pinBtn = document.createElement("button");
  pinBtn.type = "button";
  pinBtn.className = "bp-btn-ghost bp-top-act";
  const pinLbl = document.createElement("span");
  pinLbl.textContent = t("menu.pin", "Pin");
  pinBtn.append(makeMiniIcon(["M12 17v5", "M9 3h6l1 7 3 3H5l3-3 1-7z"]), pinLbl);

  const settleBtn = document.createElement("button");
  settleBtn.type = "button";
  settleBtn.className = "bp-btn-ghost bp-top-act";
  const settleLbl = document.createElement("span");
  settleLbl.textContent = t("menu.settle", "Settle");
  settleBtn.append(makeMiniIcon(["M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20z", "M9 12l2 2 4-4"]), settleLbl);

  const menuWrap = document.createElement("div");
  menuWrap.className = "bp-menu-wrap";
  const threadMenuBtn = document.createElement("button");
  threadMenuBtn.type = "button";
  threadMenuBtn.className = "bp-btn-ghost bp-top-act";
  threadMenuBtn.textContent = "▾";
  threadMenuBtn.title = "Thread menu";
  threadMenuBtn.setAttribute("aria-label", "Thread menu");
  threadMenuBtn.setAttribute("data-testid", "thread-menu-btn");
  const threadMenuPop = document.createElement("div");
  threadMenuPop.className = "bp-popover bp-hidden";
  threadMenuPop.setAttribute("data-testid", "thread-menu");
  menuWrap.append(threadMenuBtn, threadMenuPop);

  topActs.append(pinBtn, settleBtn, menuWrap);

  // Right controls: Improvements, command box, search icon, settings gear
  const topRight = document.createElement("div");
  topRight.className = "bp-top-right";

  const impBtn = document.createElement("button");
  impBtn.type = "button";
  impBtn.className = "bp-btn-ghost bp-top-act bp-hide-tablet";
  impBtn.setAttribute("data-testid", "improvements-btn");
  const impLbl = document.createElement("span");
  impLbl.textContent = t("gpui.improvements", "Improvements");
  impBtn.append(makeMiniIcon(["M12 2l2.4 7.4H22l-6.2 4.5 2.4 7.4L12 16.8l-6.2 4.5 2.4-7.4L2 9.4h7.6z"]), impLbl);

  const cmdBox = document.createElement("button");
  cmdBox.type = "button";
  cmdBox.className = "bp-cmd-box bp-hide-tablet";
  cmdBox.textContent = "> Jump or run a command";

  const searchBtn = document.createElement("button");
  searchBtn.type = "button";
  searchBtn.className = "icon bp-top-icon";
  searchBtn.title = "Search";
  searchBtn.setAttribute("aria-label", "Search");
  searchBtn.appendChild(makeMiniIcon(["M11 19a8 8 0 1 0 0-16 8 8 0 0 0 0 16z", "M21 21l-4.3-4.3"]));

  const settingsBtn = document.createElement("button");
  settingsBtn.type = "button";
  settingsBtn.className = "icon bp-top-icon";
  settingsBtn.title = t("palette.settings", "Settings");
  settingsBtn.setAttribute("aria-label", t("palette.settings", "Settings"));
  settingsBtn.setAttribute("data-testid", "settings-btn");
  settingsBtn.appendChild(makeMiniIcon([
    "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z",
    "M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9c.26.6.85 1 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z",
  ]));

  topRight.append(impBtn, cmdBox, searchBtn, settingsBtn);
  topbar.append(sideToggle, brand, stateBox, topActs, topRight);

  const body = document.createElement("div");
  body.className = "bp-body";

  const sidebarEl = document.createElement("aside");
  sidebarEl.className = "bp-sidebar";
  sidebarEl.setAttribute("data-testid", "sidebar");

  const centerEl = document.createElement("main");
  centerEl.className = "bp-center";

  const rightAreaEl = document.createElement("div");
  rightAreaEl.className = "bp-right-area bp-hidden";
  rightAreaEl.setAttribute("data-testid", "right-area");

  const rightTopEl = document.createElement("div");
  rightTopEl.className = "bp-right-top";

  const viewerEl = document.createElement("section");
  viewerEl.className = "bp-viewer-pane bp-hidden";
  viewerEl.setAttribute("data-testid", "viewer-pane");

  rightTopEl.appendChild(viewerEl);
  rightAreaEl.appendChild(rightTopEl);

  const railEl = document.createElement("nav");
  railEl.className = "bp-rail";
  railEl.setAttribute("aria-label", "Viewer rail");
  railEl.setAttribute("data-testid", "viewer-rail");

  body.append(sidebarEl, centerEl, rightAreaEl, railEl);
  root.append(topbar, body);

  const hub = new HubClient();
  let sections: SidebarSection[] | null = null;
  const openSectionFolds = new Set<string>();
  let sectionsGen = 0;
  let sectionsRefreshTimer: number | null = null;
  let projects: ProjectItem[] = [];
  let threads: ThreadItem[] = [];
  let bots: BotItem[] = [];
  let rooms: RoomItem[] = [];
  let hostDisplayName = "";
  let favoriteModels: string[] = [];
  let railItems: RailEntry[] = [];
  let helloCommands: SlashCommand[] = [];
  let selectedThreadId = "";
  let threadGen = 0;
  let railGen = 0;
  let threadSubId: SubId | null = null;
  let olderCursor: string | null = null;
  let activeRailKind = "";
  let viewerController: ReturnType<ViewerMountFn> | null = null;
  const THREAD_CACHE_MAX = 20;
  let shellHashes: Record<string, string> = {};
  const threadCache = new Map<string, {
    entries: EntryItem[];
    ids: string[];
    olderCursor: string | null;
    review: ReviewBar | null;
    unanswered: UnansweredNotice | null;
    rail: RailEntry[];
    hashes: Record<string, string>;
  }>();
  function putThreadCache(
    id: string,
    val: {
      entries: EntryItem[];
      ids: string[];
      olderCursor: string | null;
      review: ReviewBar | null;
      unanswered: UnansweredNotice | null;
      rail: RailEntry[];
      hashes: Record<string, string>;
    },
  ): void {
    threadCache.delete(id);
    threadCache.set(id, val);
    while (threadCache.size > THREAD_CACHE_MAX) {
      const oldest = threadCache.keys().next().value;
      if (oldest !== undefined) threadCache.delete(oldest);
      else break;
    }
  }

  // Project Home: the page a project's name opens (feature `projecthome`); it covers the thread and its composer until a thread is opened
  let homeProjectId = "";
  let homeGen = 0;
  const projectHome = new ProjectHomeView(centerEl, {
    onNewThread: (projectId) => { void createThread(projectId); },
    onOpenThread: (id) => { void openThread(id); },
    onOpenViewer: (threadId, kind) => {
      void openThread(threadId).then(() => {
        if (selectedThreadId === threadId) openViewer(kind);
      });
    },
    onRetry: (projectId) => { void openProjectHome(projectId); },
  });

  function closeProjectHome(): void {
    if (!homeProjectId) return;
    homeProjectId = "";
    homeGen++;
    centerEl.classList.remove("bp-home-open");
    projectHome.hide();
    sidebar.setSelectedProject("");
    updateHeader();
  }

  /** A project by the id the hub's requests take: a section row's id may carry its place before a `|`. */
  function projectOf(projectId: string): ProjectItem | undefined {
    return projects.find((p) => p.id === projectId) ?? projects.find((p) => rawProjectId(p.id) === projectId);
  }

  /** The home's thread rows: the hub's own list for the project, in its order. */
  async function loadHomeThreads(projectId: string, gen: number): Promise<void> {
    try {
      const res = await hub.rpc<{ threads?: ThreadItem[] }>("threads.list", { project: projectId, limit: 50, archived: false });
      if (gen !== homeGen || homeProjectId !== projectId) return;
      const rows: HomeThreadRow[] = (res.threads ?? []).filter((th) => !th.swarm).map((th) => ({
        id: th.id,
        title: th.title,
        dot: stateDotClass(th),
        status: th.status || th.state || "",
        age: ageText(th.updatedAt),
        settled: th.settled,
      }));
      projectHome.setThreads(rows);
    } catch {
      if (gen === homeGen && homeProjectId === projectId) projectHome.setThreads([]);
    }
  }

  /** Opens a project's home. False: this hub has no home to show (an older hub), and the caller does what it did before. */
  function openProjectHome(projectId: string): boolean {
    if (!projectId || !hub.hasFeature("projecthome")) return false;
    voice.cancel();
    composer.closeModelPicker();
    homeProjectId = projectId;
    const gen = ++homeGen;
    const proj = projectOf(projectId);
    centerEl.classList.add("bp-home-open");
    sidebarEl.classList.remove("bp-sidebar-open");
    projectHome.show({ id: projectId, title: proj?.title ?? projectId });
    sidebar.setSelectedProject(projectId);
    updateHeader();
    void loadHomeThreads(projectId, gen);
    void hub.rpc("project.home", { project: projectId })
      .then((res) => {
        if (gen === homeGen && homeProjectId === projectId) projectHome.setData(parseProjectHome(res));
      })
      .catch((e) => {
        if (gen !== homeGen || homeProjectId !== projectId) return;
        projectHome.setError(e instanceof Error && e.message ? e.message : t("web.home.error", "Could not load this project."));
      });
    return true;
  }

  const sidebar = new SidebarView(sidebarEl, {
    onSelectThread: (id) => {
      if (id === selectedThreadId && threadSubId !== null) {
        closeProjectHome();
        sidebarEl.classList.remove("bp-sidebar-open");
        return;
      }
      void openThread(id);
    },
    onNewThread: (projectId) => { void createThread(projectId); },
    onOpenProject: (projectId) => openProjectHome(rawProjectId(projectId)),
    canOpenProject: () => hub.hasFeature("projecthome"),
    onSelectBot: (bot) => { void openThread(bot.thread); },
    onQuickOpenViewer: (projectId, kind) => {
      const existing = threads.find((th) => th.project === projectId && !th.archived && !th.swarm);
      const go = async (): Promise<void> => {
        if (existing) await openThread(existing.id);
        else await createThread(projectId);
        if (selectedThreadId) {
          activeRailKind = kind;
          rail.setActive(kind);
          viewerController?.open(kind, selectedThreadId);
        }
      };
      void go();
    },
    onThreadAct: (id, action) => { void runThreadAct(id, action); },
    onSnoozeRow: (value) => {
      void hub.rpc("sidebar.act", { action: "row-snooze", value })
        .then(() => refreshSections())
        .catch(() => {});
    },
    onToggleSection: (toggle) => {
      if (openSectionFolds.has(toggle)) openSectionFolds.delete(toggle);
      else openSectionFolds.add(toggle);
      void refreshSections();
    },
    onToggleProjectFold: (projectKey) => {
      void hub.rpc("sidebar.act", { action: "project-fold", value: projectKey })
        .then(() => refreshSections())
        .catch(() => {});
    },
    onCollapseSidebar: () => {
      sidebarEl.classList.remove("bp-sidebar-open");
      sidebarEl.classList.add("bp-sidebar-collapsed");
    },
    // a swarm need's button: swarm.act, then what its reply opens (this client has no room view or account page: those are left out)
    onNeedAct: async (a) => {
      const go = needPress(a, false);
      if (!go) return;
      if (go.kind === "thread") { void openThread(go.id); return; }
      if (go.kind !== "call") return;
      const next = needAfter(await hub.rpc(go.method, go.params));
      if (next?.kind === "thread") void openThread(next.id);
      void refreshSections();
    },
    hasRoomView: false,
    onAddProject: () => addProject.open({ mode: "folder" }),
    onSettleSwarm: (roomId) => {
      void hub.rpc("sidebar.act", { action: "swarm-settle", value: roomId })
        .then(() => refreshSections())
        .catch(() => {});
    },
    onNewChat: () => { void createThread(CHATS); },
    onMoveToProject: (id) => openMoveToProject(id),
    onDismissHomeMove: () => {
      void hub.rpc<{ home?: HomeInfo }>("home.move.dismiss", {})
        .then((res) => {
          if (res?.home !== undefined) {
            homeInfo = res.home ?? null;
            sidebar.setHome(homeInfo);
            if (settings.isOpen()) void settings.refresh();
          }
        })
        .catch(() => {});
    },
  });

  const chat = new ChatView(centerEl, {
    onLoadOlder: () => { void loadOlderEntries(); },
    onGoToEntry: (id) => { void goToEntry(id); },
    onOpenAccounts: () => { openAccounts(); },
    onAnswerAsk: (askId, answer) => {
      void hub.rpc("ask.answer", { ask: askId, answer }).catch((e) => {
        composer.showError(e instanceof Error ? e.message : "Could not send answer");
      });
    },
    onToggleReview: (nextOn) => {
      if (!selectedThreadId) return;
      const gen = threadGen;
      void hub.rpc<{ review?: ReviewBar | null }>("thread.review", { thread: selectedThreadId, on: nextOn })
        .then((r) => { if (gen === threadGen) chat.setReview(r.review ?? null); })
        .catch(() => {});
    },
    onUnansweredAction: (action, value) => {
      if (!selectedThreadId) return;
      if (action === "again" || action === "thread-again") {
        void hub.rpc("thread.again", { thread: selectedThreadId, value }).catch(() => {});
      } else {
        void hub.rpc("thread.act", { thread: selectedThreadId, action, value }).catch(() => {});
      }
    },
    onReplyEntry: (entry) => {
      // feature `reply`: the hub keeps the desktop's <in_reply_to> block (docs/PHONE-DATA.md); an older hub: the first line quoted in the draft
      if (!hub.hasFeature("reply")) {
        const firstLine = (entry.text || "").split("\n")[0]?.slice(0, 100) ?? "";
        if (firstLine) composer.prependDraft(`> ${firstLine}\n`);
        return;
      }
      if (!entry.text || !entry.text.trim() || entry.live || entry.id.startsWith("local-")) return;
      startReply({ id: entry.id, text: entry.text, role: entry.kind === "user" ? "user" : "assistant" });
    },
    onLoadImage: async (entryId, path) => {
      try {
        const params: Record<string, unknown> = { entry: entryId, path };
        if (hub.hasFeature("blobs")) params.metaOnly = true;
        const res = await hub.rpc<{ mime?: string; data?: string; blob?: string }>("message.image", params);
        if (res.blob && res.blob.startsWith("/blob/")) return { url: res.blob };
        if (res.mime && res.data) return { mime: res.mime, data: res.data };
      } catch { /* ignore */ }
      return null;
    },
  });

  function effectivePlanMode(th: ThreadItem | null): PlanModeInfo | null {
    if (!th) return null;
    if (th.planMode) return th.planMode;
    if (th.provider === "claude") return { phase: "off", interaction: "default", once: false };
    return null;
  }

  function applyPlanModeRpc(method: string, params: Record<string, unknown>): void {
    if (!selectedThreadId) return;
    const gen = threadGen;
    const tid = selectedThreadId;
    void hub.rpc<{ thread?: ThreadItem }>(method, params)
      .then((res) => {
        if (gen !== threadGen || selectedThreadId !== tid) return;
        if (res.thread) {
          const idx = threads.findIndex((x) => x.id === res.thread!.id);
          if (idx >= 0) threads[idx] = res.thread;
          else threads.unshift(res.thread);
          sidebar.setData(projects, threads, bots, rooms);
          chat.updateThreadMeta(res.thread);
          updateHeader();
          composer.setState(true, res.thread.state === "working");
        }
      })
      .catch((e) => {
        if (gen === threadGen && selectedThreadId === tid) {
          composer.showError(e instanceof Error ? e.message : "Could not change Plan mode");
        }
      });
  }

  let voice: VoiceController;
  const composer = new ComposerView(centerEl, {
    onSend: (text, attachments) => { void sendMessage(text, attachments); },
    onRunCommand: (text) => { void runSlashCommand(text); },
    onInterrupt: () => {
      if (selectedThreadId) void hub.rpc("turn.interrupt", { thread: selectedThreadId }).catch(() => {});
    },
    onQueryMentions: async (q, kind) => {
      try {
        const res = await hub.rpc<{ items?: MentionItem[] }>("mentions.list", { q, kind, limit: 10 });
        return res.items ?? [];
      } catch {
        return [];
      }
    },
    onToggleVoice: () => { void voice.toggle(); },
    onCyclePlanMode: () => {
      const th = threads.find((x) => x.id === selectedThreadId) ?? null;
      const pm = effectivePlanMode(th);
      if (!pm || !selectedThreadId) return;
      if (pm.interaction !== "plan") {
        applyPlanModeRpc("thread.planonce", { thread: selectedThreadId, on: true });
      } else if (pm.once) {
        applyPlanModeRpc("thread.pick", { thread: selectedThreadId, action: "interaction", value: "plan" });
      } else {
        applyPlanModeRpc("thread.pick", { thread: selectedThreadId, action: "interaction", value: "default" });
      }
    },
    onSetPlanMode: (mode) => {
      if (!selectedThreadId) return;
      if (mode === "once") {
        applyPlanModeRpc("thread.planonce", { thread: selectedThreadId, on: true });
      } else if (mode === "always") {
        applyPlanModeRpc("thread.pick", { thread: selectedThreadId, action: "interaction", value: "plan" });
      } else {
        applyPlanModeRpc("thread.pick", { thread: selectedThreadId, action: "interaction", value: "default" });
      }
    },
    getFavoriteModels: () => favoriteModels,
    onLoadModelPicker: async () => {
      if (!selectedThreadId) return null;
      return hub.rpc("thread.picker", { thread: selectedThreadId });
    },
    onSelectModel: async (provider, model) => {
      if (!selectedThreadId) return;
      const gen = threadGen;
      const tid = selectedThreadId;
      try {
        const res = await hub.rpc<{ thread?: ThreadItem }>("thread.model", { thread: tid, provider, model });
        if (gen !== threadGen || selectedThreadId !== tid) return;
        if (res.thread) {
          const idx = threads.findIndex((x) => x.id === res.thread!.id);
          if (idx >= 0) threads[idx] = res.thread;
          else threads.unshift(res.thread);
          sidebar.setData(projects, threads, bots, rooms);
          chat.updateThreadMeta(res.thread);
          updateHeader();
          composer.setState(true, res.thread.state === "working");
        }
      } catch (e) {
        if (gen === threadGen && selectedThreadId === tid) {
          composer.showError(e instanceof Error ? e.message : "Could not switch model");
        }
      }
    },
    onSelectEffort: async (effort) => {
      if (!selectedThreadId) return;
      const gen = threadGen;
      const tid = selectedThreadId;
      try {
        const res = await hub.rpc<{ thread?: ThreadItem }>("thread.pick", { thread: tid, action: "effort", value: effort });
        if (gen !== threadGen || selectedThreadId !== tid) return;
        if (res.thread) {
          const idx = threads.findIndex((x) => x.id === res.thread!.id);
          if (idx >= 0) threads[idx] = res.thread;
          else threads.unshift(res.thread);
          sidebar.setData(projects, threads, bots, rooms);
          chat.updateThreadMeta(res.thread);
          updateHeader();
        }
      } catch (e) {
        if (gen === threadGen && selectedThreadId === tid) {
          composer.showError(e instanceof Error ? e.message : "Could not set effort");
        }
      }
    },
    onToggleFavorite: async (provider, model, on) => {
      const res = await hub.rpc<{ favorites?: string[] }>("models.favorite", { provider, model, on });
      if (Array.isArray(res.favorites)) {
        favoriteModels = res.favorites;
        return favoriteModels;
      }
      return null;
    },
  });

  voice = new VoiceController(hub, {
    getThreadId: () => selectedThreadId,
    getDraft: () => composer.getDraft(),
    getCaret: () => composer.getCaret(),
    setDraft: (text) => composer.setDraft(text),
    setRecording: (active) => composer.setVoiceRecording(active),
    setLevel: (level) => composer.setVoiceLevel(level),
    showError: (msg) => composer.showError(msg),
  });

  function syncRightArea(): void {
    const viewerOpen = activeRailKind !== "";
    const panelOpen = mgmt.isOpen();
    rightAreaEl.classList.toggle("bp-hidden", !viewerOpen && !panelOpen);
    rightAreaEl.classList.toggle("bp-has-viewer", viewerOpen);
    rightAreaEl.classList.toggle("bp-has-mgmt", panelOpen);
  }

  const mgmt = new MgmtStrip(rightTopEl, rightAreaEl, {
    hub,
    getThreadId: () => selectedThreadId,
    getThreadGen: () => threadGen,
    onSendText: (text) => { void sendMessage(text); },
    onOpenChange: () => syncRightArea(),
  });

  function renameThread(th: ThreadItem): void {
    const next = window.prompt("Rename thread", th.title || "");
    if (!next || !next.trim()) return;
    void hub.rpc<{ thread?: ThreadItem }>("thread.act", { thread: th.id, action: "rename", title: next.trim() })
      .then((res) => {
        if (res.thread) {
          threads = threads.map((x) => (x.id === res.thread!.id ? res.thread! : x));
          updateHeader();
          void refreshSections();
        }
      })
      .catch((e) => {
        composer.showError(e instanceof Error ? e.message : "Could not rename thread");
      });
  }

  function executePaletteAction(act: PaletteAction): void {
    switch (act.type) {
      case "thread":
        void openThread(act.id);
        break;
      case "rail":
        if (!selectedThreadId && act.id !== "terminal") break;
        if (activeRailKind === act.id) closeViewer();
        else openViewer(act.id);
        break;
      case "thread-act":
        if (selectedThreadId) void runThreadAct(selectedThreadId, act.action);
        break;
      case "new-thread":
        void createThread(act.projectId);
        break;
      case "new-chat":
        void createThread(CHATS);
        break;
      case "move-project":
        if (selectedThreadId) openMoveToProject(selectedThreadId);
        break;
      case "rename": {
        const th = threads.find((x) => x.id === selectedThreadId);
        if (th) renameThread(th);
        break;
      }
      case "prev-thread":
      case "next-thread": {
        const list = threads.filter((x) => !x.archived && !x.swarm);
        if (list.length < 2) break;
        const idx = list.findIndex((x) => x.id === selectedThreadId);
        const step = act.type === "next-thread" ? 1 : -1;
        const next = list[(idx < 0 ? 0 : idx + step + list.length) % list.length];
        if (next) void openThread(next.id);
        break;
      }
      case "command":
        if (act.runDirectly) {
          void runSlashCommand(act.word);
        } else {
          composer.setDraft(`${act.word} `);
          composer.focus();
        }
        break;
      case "improvements":
        void openImprovementsModal();
        break;
      case "settings":
        void settings.open();
        break;
      case "sidebar":
        sidebarEl.classList.toggle("bp-sidebar-open");
        sidebarEl.classList.toggle("bp-sidebar-collapsed");
        break;
    }
  }

  const palette = new CommandPalette(root, {
    getState: () => ({
      selectedThreadId,
      threads,
      projects,
      bots,
      railItems,
      commands: helloCommands,
    }),
    onSearch: async (q) => {
      if (!hub.hasFeature("search")) return { threads: [], projects: [], messages: [] };
      return hub.rpc<PaletteSearchResult>("search", { q, limit: 20 });
    },
    onExecute: executePaletteAction,
  });

  const addProject = new AddProjectDialog(root, {
    hub,
    onProjectAdded: (project) => {
      // as a phone does after project.add (no starter thread from the hub): its first thread, opened
      void refreshSections();
      if (project.id) void createThread(project.id);
    },
    onCloneDone: (job) => {
      void refreshSections();
      if (job.thread) void openThread(job.thread);
    },
  });

  const rail = new RailView(railEl, (id) => {
    if (id === "commands") {
      palette.open();
      return;
    }
    if (!selectedThreadId) return;
    if (activeRailKind === id) closeViewer();
    else openViewer(id);
  });

  let homeInfo: HomeInfo | null = null;
  let noProvider: NoProviderNotice | null = null;
  const settings = new SettingsModal(root, {
    onLoadDevices: async () => {
      const res = await hub.rpc<{ rows?: SettingsRow[] }>("devices.page", {});
      return res.rows ?? [];
    },
    onDevicesAct: async (action, value) => {
      await hub.rpc("devices.page.act", { action, value });
    },
    onGetHome: () => homeInfo,
    onMoveHomeHere: async () => {
      await hub.rpc("home.move", { to: "back" });
    },
    onGetNoProvider: () => noProvider,
    onGetThemePref: () => themePref,
    onSetThemePref: (next) => {
      themePref = next;
      try {
        if (next === "auto") localStorage.removeItem("bp.theme.pref");
        else localStorage.setItem("bp.theme.pref", next);
      } catch { /* ignore */ }
      applyEffectiveTheme();
    },
    onGetHubTheme: () => hubTheme,
    onGetLocalePref: () => localePref,
    onSetLocalePref: (next) => {
      localePref = next;
      try {
        if (next === "auto") localStorage.removeItem("bp.locale");
        else localStorage.setItem("bp.locale", next);
      } catch { /* ignore */ }
      applyEffectiveLocale();
    },
    hasRing: () => hub.hasFeature("ringnotes"),
    onRingList: async (q) => parseRingList(await hub.rpc("ring.list", { q })),
    onRingHear: async (note, engine) => { await hub.rpc("ring.hear", { note, engine }); },
    onOpenThread: (id) => { void openThread(id); },
    onLogout: () => signOut(() => {
      try { clearSidebarCache(window.localStorage); } catch { /* ignore */ }
      hub.disconnect();
      window.location.replace("/");
    }),
  });
  function applyNoProvider(next: NoProviderNotice | null): void {
    noProvider = next;
    chat.setNoProvider(next);
    if (settings.isOpen()) void settings.refresh();
  }
  function openAccounts(): void {
    void settings.openToAccounts();
  }
  function showSendError(err: unknown, fallback: string): void {
    const rawMsg = err instanceof Error ? err.message : fallback;
    const rpcErr = err instanceof HubRpcError ? err : null;
    const code = rpcErr?.code;
    const machine = rpcErr?.machine;
    const msg =
      code === "no_provider_remote" && machine
        ? t(
            "provider.notice.senderr.remote",
            "No AI provider connected on {machine}. Open Settings → Accounts on {machine} to connect Claude, Codex, Antigravity, or another provider.",
          ).replaceAll("{machine}", machine)
        : rawMsg;
    composer.showError(
      msg,
      isLocalNoProviderError(rawMsg, code)
        ? { label: t("provider.notice.button", "Open Settings → Accounts"), onClick: openAccounts }
        : undefined,
    );
  }

  pinBtn.addEventListener("click", () => {
    const th = threads.find((x) => x.id === selectedThreadId);
    if (th) void runThreadAct(th.id, th.pinned ? "unpin" : "pin");
  });
  settleBtn.addEventListener("click", () => {
    const th = threads.find((x) => x.id === selectedThreadId);
    if (th) void runThreadAct(th.id, th.settled ? "unsettle" : "settle");
  });
  const threadContext = new ThreadContextDialog(root, (thread, decision) =>
    decision ? hub.rpc("thread.context.mcp", { thread, ...decision }) : hub.rpc("thread.context", { thread }));
  threadMenuBtn.addEventListener("click", (e) => {
    e.stopPropagation();
    const th = threads.find((x) => x.id === selectedThreadId);
    if (!th) return;
    if (!threadMenuPop.classList.contains("bp-hidden")) {
      threadMenuPop.classList.add("bp-hidden");
      return;
    }
    const items: Array<{ label: string; danger?: boolean; run: () => void }> = [
      {
        label: "Rename…",
        run: () => renameThread(th),
      },
      { label: th.pinned ? t("menu.unpin", "Unpin") : t("menu.pin", "Pin"), run: () => { void runThreadAct(th.id, th.pinned ? "unpin" : "pin"); } },
      { label: th.settled ? t("phone.unsettle", "Unsettle") : t("menu.settle", "Settle"), run: () => { void runThreadAct(th.id, th.settled ? "unsettle" : "settle"); } },
      { label: th.archived ? "Unarchive" : "Archive", run: () => { void runThreadAct(th.id, th.archived ? "unarchive" : "archive"); } },
      {
        label: "Delete",
        danger: true,
        run: () => {
          if (window.confirm("Delete this thread?")) void runThreadAct(th.id, "delete");
        },
      },
    ];
    if (hub.hasFeature("threadContext")) items.unshift({ label: t("thread.context.title", "Context & Prompt"), run: () => threadContext.open(th.id) });
    const frag = document.createDocumentFragment();
    for (const it of items) {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "bp-popover-item" + (it.danger ? " danger" : "");
      btn.textContent = it.label;
      btn.addEventListener("click", () => {
        threadMenuPop.classList.add("bp-hidden");
        it.run();
      });
      frag.appendChild(btn);
    }
    threadMenuPop.replaceChildren(frag);
    threadMenuPop.classList.remove("bp-hidden");
  });
  document.addEventListener("click", () => threadMenuPop.classList.add("bp-hidden"));

  async function openImprovementsModal(): Promise<void> {
    const backdrop = document.createElement("div");
    backdrop.className = "bp-modal-backdrop";
    backdrop.setAttribute("data-testid", "improvements-modal");
    const modal = document.createElement("div");
    modal.className = "bp-modal";
    const head = document.createElement("div");
    head.className = "bp-modal-head";
    const h2 = document.createElement("h2");
    h2.textContent = t("gpui.improvements", "Improvements");
    const closeBtn = document.createElement("button");
    closeBtn.type = "button";
    closeBtn.className = "icon";
    closeBtn.textContent = "×";
    closeBtn.addEventListener("click", () => backdrop.remove());
    head.append(h2, closeBtn);
    const bodyEl = document.createElement("div");
    bodyEl.className = "bp-modal-body";
    bodyEl.textContent = "Loading…";
    modal.append(head, bodyEl);
    backdrop.appendChild(modal);
    backdrop.addEventListener("click", (e) => { if (e.target === backdrop) backdrop.remove(); });
    root.appendChild(backdrop);

    try {
      const res = await hub.rpc<{ project?: string; projects?: Array<{ id: string; title?: string; name?: string }> }>("improvements.projects", {});
      const list = res.projects ?? [];
      if (list.length === 0) {
        bodyEl.textContent = "No projects available for Improvements.";
        return;
      }
      const frag = document.createDocumentFragment();
      for (const p of list) {
        const row = document.createElement("div");
        row.className = "bp-set-row";
        const info = document.createElement("div");
        info.className = "bp-set-info";
        info.textContent = p.title || p.name || p.id;
        const btn = document.createElement("button");
        btn.type = "button";
        btn.className = p.id === res.project ? "primary" : "";
        btn.textContent = p.id === res.project ? "Active" : "Select";
        btn.addEventListener("click", () => {
          void hub.rpc("improvements.project", { project: p.id })
            .then(() => backdrop.remove())
            .catch(() => {});
        });
        row.append(info, btn);
        frag.appendChild(row);
      }
      bodyEl.replaceChildren(frag);
    } catch (e) {
      bodyEl.textContent = e instanceof Error ? e.message : "Could not load Improvements.";
    }
  }

  /** Move to project…: a chat that belongs to no project goes to the project picked, with all it says (`thread.move_project`). */
  function openMoveToProject(threadId: string): void {
    const th = threads.find((x) => x.id === threadId);
    const backdrop = document.createElement("div");
    backdrop.className = "bp-modal-backdrop";
    backdrop.setAttribute("data-testid", "move-project-modal");
    const modal = document.createElement("div");
    modal.className = "bp-modal";
    const head = document.createElement("div");
    head.className = "bp-modal-head";
    const h2 = document.createElement("h2");
    h2.textContent = t("chats.move", "Move to project…");
    const closeBtn = document.createElement("button");
    closeBtn.type = "button";
    closeBtn.className = "icon";
    closeBtn.textContent = "×";
    closeBtn.addEventListener("click", () => backdrop.remove());
    head.append(h2, closeBtn);
    const bodyEl = document.createElement("div");
    bodyEl.className = "bp-modal-body";
    const note = document.createElement("div");
    note.className = "bp-move-note";
    note.textContent = th?.title || "";
    const err = document.createElement("div");
    err.className = "bp-move-err";
    bodyEl.append(note);
    const list = moveTargets(projects);
    if (list.length === 0) {
      const none = document.createElement("div");
      none.textContent = t("chats.move.none", "No project to move it to. Add a project first.");
      bodyEl.appendChild(none);
    }
    for (const p of list) {
      const row = document.createElement("button");
      row.type = "button";
      row.className = "bp-move-row";
      row.textContent = p.title || p.id;
      row.setAttribute("data-testid", `move-project-${p.id}`);
      row.addEventListener("click", () => {
        void hub.rpc<{ thread?: ThreadItem }>("thread.move_project", { thread: threadId, project: p.id })
          .then((res) => {
            if (res.thread) threads = threads.map((x) => (x.id === res.thread!.id ? res.thread! : x));
            else threads = threads.map((x) => (x.id === threadId ? { ...x, project: p.id } : x));
            sidebar.setData(projects, threads, bots, rooms);
            updateHeader();
            void refreshSections();
            backdrop.remove();
          })
          .catch((e) => { err.textContent = e instanceof Error ? e.message : "Could not move the chat"; });
      });
      bodyEl.appendChild(row);
    }
    bodyEl.appendChild(err);
    modal.append(head, bodyEl);
    backdrop.appendChild(modal);
    backdrop.addEventListener("click", (e) => { if (e.target === backdrop) backdrop.remove(); });
    root.appendChild(backdrop);
  }

  impBtn.addEventListener("click", () => { void openImprovementsModal(); });
  cmdBox.addEventListener("click", () => palette.open());
  searchBtn.addEventListener("click", () => palette.open());
  settingsBtn.addEventListener("click", () => { void settings.open(); });
  sideToggle.addEventListener("click", () => {
    sidebarEl.classList.toggle("bp-sidebar-open");
    sidebarEl.classList.toggle("bp-sidebar-collapsed");
  });

  function updateHeader(): void {
    // a project's home is on show: the bar names the project and offers nothing of a thread
    const th = homeProjectId ? null : threads.find((x) => x.id === selectedThreadId) ?? null;
    const proj = homeProjectId
      ? projectOf(homeProjectId) ?? { id: homeProjectId, title: homeProjectId }
      : th ? projectOf(th.project) : undefined;
    if (homeProjectId && proj) projectHome.setTitle(proj.title);
    // a chat that belongs to no project says so where a project's name would be
    const chat = !homeProjectId && th?.project === CHATS;
    projCrumb.textContent = chat ? t("chats.title", "Chats") : (proj?.title ?? "");
    sep1.classList.toggle("bp-hidden", !proj && !chat);
    sep2.classList.toggle("bp-hidden", !th);
    threadCrumb.textContent = th ? (th.title || "Untitled thread") : "";

    const pm = effectivePlanMode(th);
    composer.setPlanMode(pm);
    if (!pm || pm.phase === "off") {
      planTopBadge.classList.add("bp-hidden");
      planTopBadge.classList.remove("is-exec");
      planTopBadge.textContent = "";
    } else {
      planTopBadge.classList.remove("bp-hidden");
      planTopBadge.classList.toggle("is-exec", pm.phase === "executing");
      planTopBadge.title = t("gpui.planmode.tip", "Plan before acting (1 turn, always, or off)");
      planTopBadge.textContent = pm.phase === "armed" && pm.once
        ? t("gpui.planmode.top.once", "Plan (1 turn)")
        : pm.phase === "armed"
          ? t("gpui.mgmt.plan.tab", "Plan")
          : pm.phase === "planning"
            ? t("gpui.planmode.top.planning", "Planning")
            : pm.phase === "pending"
              ? t("gpui.planmode.top.pending", "Review plan")
              : t("gpui.planmode.top.executing", "Building plan");
    }

    stateBox.classList.toggle("bp-hidden", !th);
    topActs.classList.toggle("bp-hidden", !th);
    if (th) {
      stateDot.className = `bp-dot ${stateDotClass(th)}`;
      stateWord.textContent = th.status || th.state || "idle";
      pinLbl.textContent = th.pinned ? t("menu.unpin", "Unpin") : t("menu.pin", "Pin");
      settleLbl.textContent = th.settled ? t("phone.unsettle", "Unsettle") : t("menu.settle", "Settle");
      const extra = th as ThreadItem & { ctxPct?: number; contextPct?: number };
      composer.setModelChip(th.chip || th.model || "default", th.provider, extra.ctxPct ?? extra.contextPct);
      composer.setMemory(th.memory);
    } else {
      composer.setModelChip("default");
      composer.setMemory(null);
    }
  }

  function closeViewer(notifyHub = true): void {
    railGen++;
    activeRailKind = "";
    rail.setActive("");
    viewerEl.classList.add("bp-hidden");
    viewerController?.close();
    syncRightArea();
    if (notifyHub && selectedThreadId && hub.getState() === "connected") {
      void hub.rpc("viewer.watch", { thread: selectedThreadId, kind: "" }).catch(() => {});
    }
  }

  function openViewer(kind: string): void {
    railGen++;
    activeRailKind = kind;
    rail.setActive(kind);
    viewerEl.classList.remove("bp-hidden");
    syncRightArea();
    if (!viewerController && typeof mountViewerPane === "function") {
      viewerController = (mountViewerPane as ViewerMountFn)(viewerEl, {
        hub,
        getThreadId: () => selectedThreadId,
        closeViewer: () => closeViewer(true),
      });
    }
    if (viewerController) {
      // PR #1367 rule (3): direct destination open replaces active watch without a separate stop first.
      viewerController.open(kind, selectedThreadId);
    } else {
      const box = document.createElement("div");
      box.className = "bp-viewer-placeholder";
      const head = document.createElement("div");
      head.className = "bp-viewer-bar";
      const lbl = document.createElement("span");
      lbl.textContent = kind.toUpperCase();
      const closeBtn = document.createElement("button");
      closeBtn.type = "button";
      closeBtn.className = "icon";
      closeBtn.textContent = "×";
      closeBtn.addEventListener("click", () => closeViewer(true));
      head.append(lbl, closeBtn);
      const bodyMsg = document.createElement("div");
      bodyMsg.className = "bp-dim bp-viewer-msg";
      bodyMsg.textContent = `Viewer pane (${kind}).`;
      box.append(head, bodyMsg);
      viewerEl.replaceChildren(box);
    }
  }

  async function refreshRail(threadId: string, gen: number): Promise<void> {
    if (!threadId || !hub.hasFeature("rail")) {
      if (gen === threadGen) {
        railItems = [];
        rail.setItems([]);
      }
      return;
    }
    const rGen = ++railGen;
    try {
      const res = await hub.rpc<{ rail?: RailEntry[]; items?: RailEntry[]; has?: RailHas }>("viewer.rail", {
        thread: threadId,
        page: activeRailKind,
      });
      // PR #1367 rule (2): drop replies older than a newer thread or local rail selection.
      if (gen !== threadGen || rGen !== railGen) return;
      const items = Array.isArray(res.rail) ? res.rail : Array.isArray(res.items) ? res.items : [];
      railItems = items;
      rail.setItems(items);
      // PR #1367 rule (1): reconcile open page against `has`, never against entry state.
      if (res.has) {
        viewerController?.reconcileHas?.(res.has);
        if (activeRailKind && !hasForKind(res.has, activeRailKind)) {
          closeViewer(true);
        }
      }
    } catch {
      if (gen === threadGen && rGen === railGen) {
        railItems = [];
        rail.setItems([]);
      }
    }
  }

  function firstSectionThreadId(list: SidebarSection[]): string {
    for (const sec of list) {
      if (sec.kind === "threads") {
        for (const r of sec.rows as SectionThreadRow[]) {
          if (!r.off && (r.nav || r.id) && !(r.nav || "").startsWith("@room:")) return r.nav || r.id;
        }
      } else if (sec.kind === "projects") {
        for (const p of sec.rows as SectionProjectRow[]) {
          for (const th of p.threads ?? []) {
            if (!th.off && (th.nav || th.id) && !(th.nav || "").startsWith("@room:")) return th.nav || th.id;
          }
        }
      }
    }
    return "";
  }

  function hasSectionThreadId(list: SidebarSection[], id: string): boolean {
    for (const sec of list) {
      if (sec.kind === "threads") {
        if ((sec.rows as SectionThreadRow[]).some((r) => (r.nav || r.id) === id)) return true;
      } else if (sec.kind === "projects") {
        for (const p of sec.rows as SectionProjectRow[]) {
          if ((p.threads ?? []).some((th) => (th.nav || th.id) === id)) return true;
        }
      }
    }
    return false;
  }

  function persistSidebarCache(): void {
    try {
      saveSidebarCache(window.localStorage, {
        hostName: hostDisplayName || undefined,
        sections,
        projects,
        threads,
        bots,
        rooms,
        shellHashes,
        openSectionFolds: Array.from(openSectionFolds),
      });
    } catch { /* ignore */ }
  }

  function syncProjectsFromSections(list: SidebarSection[]): void {
    const out: ProjectItem[] = [];
    for (const sec of list) {
      if (sec.kind !== "projects") continue;
      for (const p of sec.rows as SectionProjectRow[]) {
        out.push({ id: p.id, title: p.title, css: p.css, color: p.color });
      }
    }
    if (out.length > 0) projects = out;
  }

  async function refreshSections(): Promise<boolean> {
    if (!hub.hasFeature("sections")) return false;
    const gen = ++sectionsGen;
    try {
      const res = await hub.rpc<{ sections?: SidebarSection[] }>("sidebar.sections", {
        open: Array.from(openSectionFolds),
      });
      if (gen !== sectionsGen) return true;
      const list = Array.isArray(res.sections) ? res.sections : [];
      sections = list;
      syncProjectsFromSections(list);
      sidebar.setData(projects, threads, bots, rooms);
      sidebar.setSections(list, bots, rooms);
      persistSidebarCache();
      updateHeader();
      if (homeProjectId) void loadHomeThreads(homeProjectId, homeGen);
      return true;
    } catch {
      return false;
    }
  }

  function scheduleSectionsRefresh(): void {
    if (!hub.hasFeature("sections")) return;
    if (sectionsRefreshTimer !== null) window.clearTimeout(sectionsRefreshTimer);
    sectionsRefreshTimer = window.setTimeout(() => {
      sectionsRefreshTimer = null;
      void refreshSections();
    }, 150);
  }

  async function loadLists(): Promise<void> {
    threadSubId = null;

    const botsPromise = hub.hasFeature("bots")
      ? hub.rpc<{ bots?: BotItem[]; rooms?: RoomItem[] }>("bots.list", {}).catch(() => null)
      : Promise.resolve(null);

    if (hub.hasFeature("shell")) {
      const have: Record<string, string> = {};
      for (const [k, v] of Object.entries(shellHashes)) {
        if ((k === "@sections" && (!sections || sections.length === 0)) || (k.startsWith("p:") && projects.length === 0)) continue;
        have[k] = v;
      }
      const [bRes, sRes] = await Promise.all([
        botsPromise,
        hub.rpc<{
          projects?: ProjectItem[];
          threads?: ThreadItem[];
          sections?: SidebarSection[] | { sections?: SidebarSection[] };
          removed?: string[];
          h?: Record<string, string>;
          sub?: SubId;
        }>("shell.sync", { have, settled: 25, open: Array.from(openSectionFolds) }),
      ]);
      if (bRes) {
        bots = bRes.bots ?? [];
        rooms = bRes.rooms ?? [];
      }
      const removed = new Set(sRes.removed ?? []);
      const updatedPids = new Set((sRes.projects ?? []).map((p) => p.id));
      if (removed.size > 0 || updatedPids.size > 0) {
        projects = [
          ...projects.filter((p) => !removed.has(p.id) && !updatedPids.has(p.id)),
          ...(sRes.projects ?? []),
        ];
        threads = [
          ...threads.filter((t) => !removed.has(t.project) && !updatedPids.has(t.project)),
          ...(sRes.threads ?? []),
        ];
      } else if (Array.isArray(sRes.threads) && sRes.threads.length > 0) {
        const byId = new Map(threads.map((t) => [t.id, t]));
        for (const t of sRes.threads) byId.set(t.id, t);
        threads = Array.from(byId.values());
      }
      if (sRes.sections !== undefined) {
        const list = Array.isArray(sRes.sections)
          ? sRes.sections
          : Array.isArray(sRes.sections?.sections)
            ? sRes.sections.sections
            : null;
        if (list) {
          sections = list;
          syncProjectsFromSections(list);
        }
      }
      if (sRes.h) shellHashes = sRes.h;
      sidebar.setData(projects, threads, bots, rooms);
      sidebar.setSections(sections, bots, rooms);
      persistSidebarCache();
    } else if (hub.hasFeature("sections")) {
      const [bRes, secRes, tRes] = await Promise.all([
        botsPromise,
        hub.rpc<{ sections?: SidebarSection[] }>("sidebar.sections", {
          open: Array.from(openSectionFolds),
        }).catch(() => null),
        hub.rpc<{ threads?: ThreadItem[] }>("threads.list", { limit: 200, archived: false }).catch(() => ({ threads: [] })),
      ]);
      if (bRes) {
        bots = bRes.bots ?? [];
        rooms = bRes.rooms ?? [];
      }
      threads = tRes.threads ?? [];
      if (secRes && Array.isArray(secRes.sections)) {
        let list = secRes.sections;
        // If all open project sections are empty but a folded section (e.g. Quiet) has projects,
        // expand it once so the user isn't left with a blank sidebar.
        const hasVisibleProj = list.some((s) => s.kind === "projects" && s.rows.length > 0);
        if (!hasVisibleProj) {
          const folded = list.find((s) => s.kind === "projects" && s.fold && s.toggle && (s.count ?? 0) > 0);
          if (folded?.toggle && !openSectionFolds.has(folded.toggle)) {
            openSectionFolds.add(folded.toggle);
            const reopened = await hub.rpc<{ sections?: SidebarSection[] }>("sidebar.sections", {
              open: Array.from(openSectionFolds),
            }).catch(() => null);
            if (reopened && Array.isArray(reopened.sections)) list = reopened.sections;
          }
        }
        sections = list;
        syncProjectsFromSections(list);
        sidebar.setData(projects, threads, bots, rooms);
        sidebar.setSections(list, bots, rooms);
      } else {
        sections = null;
        const pRes = await hub.rpc<{ projects?: ProjectItem[] }>("projects.list", { limit: 100 });
        projects = pRes.projects ?? [];
        sidebar.setSections(null, bots, rooms);
        sidebar.setData(projects, threads, bots, rooms);
      }
      persistSidebarCache();
      await hub.rpc("subscribe", { topic: "lists" }).catch(() => {});
    } else {
      sections = null;
      const [bRes, pRes, tRes] = await Promise.all([
        botsPromise,
        hub.rpc<{ projects?: ProjectItem[] }>("projects.list", { limit: 100 }),
        hub.rpc<{ threads?: ThreadItem[] }>("threads.list", { limit: 200, archived: false }),
      ]);
      if (bRes) {
        bots = bRes.bots ?? [];
        rooms = bRes.rooms ?? [];
      }
      projects = pRes.projects ?? [];
      threads = tRes.threads ?? [];
      sidebar.setSections(null, bots, rooms);
      sidebar.setData(projects, threads, bots, rooms);
      persistSidebarCache();
      await hub.rpc("subscribe", { topic: "lists" }).catch(() => {});
    }

    let targetId = selectedThreadId;
    if (!targetId) {
      let saved = "";
      try { saved = localStorage.getItem("bp.lastThread") || ""; } catch { /* ignore */ }
      if (saved && ((sections && hasSectionThreadId(sections, saved)) || threads.some((x) => x.id === saved))) {
        targetId = saved;
      } else if (sections) {
        targetId = firstSectionThreadId(sections);
      }
      if (!targetId) {
        const candidate =
          threads.find((x) => !x.archived && !x.swarm && !x.settled) ??
          threads.find((x) => !x.archived && !x.swarm);
        targetId = candidate?.id ?? "";
      }
    }

    if (targetId && ((sections && hasSectionThreadId(sections, targetId)) || threads.some((x) => x.id === targetId))) {
      // a reconnect while a project's home is on show: the thread under it is opened again, the home stays
      await openThread(targetId, homeProjectId !== "");
    } else {
      selectedThreadId = "";
      railItems = [];
      rail.setItems([]);
      composer.setState(false, false);
      updateHeader();
    }
    // still on show (nothing else was opened meanwhile): what it says is read again
    if (homeProjectId) openProjectHome(homeProjectId);
  }

  async function openThread(threadId: string, keepHome = false): Promise<void> {
    if (!keepHome) closeProjectHome();
    if (selectedThreadId && selectedThreadId !== threadId) {
      voice.cancel();
      const held = chat.getEntries();
      if (held.length > 0) {
        const prev = threadCache.get(selectedThreadId);
        putThreadCache(selectedThreadId, {
          entries: held,
          ids: prev?.ids ?? held.map((e) => e.id),
          olderCursor,
          review: prev?.review ?? null,
          unanswered: prev?.unanswered ?? null,
          rail: prev?.rail ?? railItems,
          hashes: prev?.hashes ?? {},
        });
      }
    }

    const gen = ++threadGen;
    railGen++;
    startReply(null);
    const prevSub = threadSubId;
    threadSubId = null;
    if (prevSub !== null) {
      void hub.rpc("unsubscribe", { sub: prevSub }).catch(() => {});
    }

    selectedThreadId = threadId;
    composer.closeModelPicker();
    const cached = threadCache.get(threadId);
    olderCursor = cached?.olderCursor ?? null;
    try { localStorage.setItem("bp.lastThread", threadId); } catch { /* ignore */ }
    sidebar.setSelected(threadId);
    sidebarEl.classList.remove("bp-sidebar-open");
    updateHeader();

    let th = threads.find((x) => x.id === threadId) ?? null;
    if (cached) {
      chat.setThread(th, cached.entries, Boolean(cached.olderCursor), cached.review, cached.unanswered);
      railItems = cached.rail;
      rail.setItems(cached.rail);
    } else {
      chat.setLoading(th);
      railItems = [];
      rail.setItems([]);
    }
    composer.setState(true, th?.state === "working");

    if (hub.hasFeature("open")) {
      const have: Record<string, string> = {};
      if (cached) {
        for (const [k, v] of Object.entries(cached.hashes)) have[k] = v;
        for (const e of cached.entries) {
          if (e.h) have[e.id] = e.h;
        }
      }
      let res: {
        sub?: SubId;
        thread?: ThreadItem;
        ids?: string[];
        entries?: EntryItem[];
        next?: string | null;
        unanswered?: UnansweredNotice | null;
        rail?: { rail?: RailEntry[]; items?: RailEntry[]; has?: RailHas } | RailEntry[];
        h?: Record<string, string>;
      };
      try {
        res = await hub.rpc("thread.open", { thread: threadId, limit: 40, have });
      } catch (e) {
        if (gen === threadGen) {
          const msg = e instanceof Error && e.message ? e.message : "Could not load messages.";
          chat.setError(th, msg, () => { void openThread(threadId); });
        }
        return;
      }
      if (gen !== threadGen) {
        if (res.sub !== undefined && res.sub !== null) {
          void hub.rpc("unsubscribe", { sub: res.sub }).catch(() => {});
        }
        return;
      }
      threadSubId = res.sub ?? null;
      if (res.thread) {
        th = res.thread;
        const idx = threads.findIndex((x) => x.id === res.thread!.id);
        if (idx >= 0) threads[idx] = res.thread;
        else threads.unshift(res.thread);
        updateHeader();
        composer.setState(true, th.state === "working");
      }
      if ("next" in res) olderCursor = res.next ?? null;

      const byId = new Map<string, EntryItem>();
      if (cached) {
        for (const e of cached.entries) byId.set(e.id, e);
      }
      for (const e of res.entries ?? []) byId.set(e.id, e);
      const ids = res.ids ?? cached?.ids ?? (res.entries ?? []).map((e) => e.id);
      const entries = ids.map((id) => byId.get(id)).filter((e): e is EntryItem => Boolean(e));

      let nextRail = cached?.rail ?? [];
      if (res.rail !== undefined) {
        if (Array.isArray(res.rail)) {
          nextRail = res.rail;
        } else {
          nextRail = Array.isArray(res.rail?.rail)
            ? res.rail.rail
            : Array.isArray(res.rail?.items)
              ? res.rail.items
              : [];
          if (res.rail?.has) {
            viewerController?.reconcileHas?.(res.rail.has);
            if (activeRailKind && !hasForKind(res.rail.has, activeRailKind)) {
              closeViewer(true);
            }
          }
        }
      }
      railItems = nextRail;
      rail.setItems(nextRail);

      let review: ReviewBar | null = cached?.review ?? null;
      if (hub.hasFeature("reviewmode")) {
        try {
          const r = await hub.rpc<{ review?: ReviewBar | null }>("thread.review", { thread: threadId });
          if (gen !== threadGen) return;
          review = r.review ?? null;
        } catch { /* ignore */ }
      }
      if (gen !== threadGen) return;

      const unanswered = "unanswered" in res ? (res.unanswered ?? null) : (cached?.unanswered ?? null);
      const hashes = res.h ?? cached?.hashes ?? {};
      putThreadCache(threadId, { entries, ids, olderCursor, review, unanswered, rail: nextRail, hashes });
      chat.setThread(th, entries, Boolean(olderCursor), review, unanswered);
      void watchMgmtForThread(threadId, gen);
      if (activeRailKind) openViewer(activeRailKind);
      return;
    }

    let res: {
      thread?: ThreadItem;
      entries?: EntryItem[];
      next?: string | null;
      unanswered?: UnansweredNotice | null;
    };
    try {
      res = await hub.rpc("thread.entries", { thread: threadId, limit: 40 });
    } catch (e) {
      if (gen === threadGen) {
        const msg = e instanceof Error && e.message ? e.message : "Could not load messages.";
        chat.setError(th, msg, () => { void openThread(threadId); });
      }
      return;
    }
    if (gen !== threadGen) return;
    if (res.thread) {
      const idx = threads.findIndex((x) => x.id === res.thread!.id);
      if (idx >= 0) threads[idx] = res.thread;
      else threads.unshift(res.thread);
      updateHeader();
    }
    olderCursor = res.next ?? null;

    let review: ReviewBar | null = null;
    if (hub.hasFeature("reviewmode")) {
      try {
        const r = await hub.rpc<{ review?: ReviewBar | null }>("thread.review", { thread: threadId });
        if (gen !== threadGen) return;
        review = r.review ?? null;
      } catch { /* ignore */ }
    }
    if (gen !== threadGen) return;

    const entries = res.entries ?? [];
    const unanswered = res.unanswered ?? null;
    putThreadCache(threadId, {
      entries,
      ids: entries.map((e) => e.id),
      olderCursor,
      review,
      unanswered,
      rail: cached?.rail ?? [],
      hashes: cached?.hashes ?? {},
    });
    chat.setThread(th, entries, Boolean(olderCursor), review, unanswered);

    try {
      const sub = await hub.rpc<{ sub?: SubId }>("subscribe", { topic: "thread", thread: threadId });
      const subVal = sub.sub ?? null;
      if (subVal !== null) {
        if (gen !== threadGen) {
          void hub.rpc("unsubscribe", { sub: subVal }).catch(() => {});
          return;
        }
        threadSubId = subVal;
      }
    } catch { /* ignore */ }
    if (gen !== threadGen) return;

    void refreshRail(threadId, gen);
    void watchMgmtForThread(threadId, gen);
    if (activeRailKind) openViewer(activeRailKind);
  }

  async function watchMgmtForThread(threadId: string, gen: number): Promise<void> {
    if (!threadId || !hub.hasFeature("mgmt")) {
      if (gen === threadGen) mgmt.clear();
      return;
    }
    const th = threads.find((x) => x.id === threadId);
    let projectPath = th?.project ? (projects.find((p) => p.id === th.project)?.root ?? "") : "";
    if (!projectPath && hub.hasFeature("viewers")) {
      try {
        const files = await hub.rpc<{ root?: string; pcb?: string; sch?: string }>("viewer.files", { thread: threadId });
        if (gen !== threadGen) return;
        projectPath = files.pcb || files.sch || files.root || "";
      } catch { /* ignore */ }
    }
    if (gen !== threadGen) return;
    if (projectPath) await mgmt.watchProject(projectPath);
    else mgmt.clear();
  }

  /** Go to a message (a reply's quote or strip): older pages are read until it is loaded (at most 25), for this thread only. */
  async function goToEntry(id: string): Promise<void> {
    const gen = threadGen;
    for (let i = 0; i < 25 && gen === threadGen; i++) {
      if (chat.goTo(id)) return;
      if (!olderCursor) return;
      const before = olderCursor;
      await loadOlderEntries();
      if (olderCursor === before) return; // nothing more came
    }
  }

  async function loadOlderEntries(): Promise<void> {
    if (!selectedThreadId || !olderCursor) {
      chat.clearLoadingOlder();
      return;
    }
    const gen = threadGen;
    const threadId = selectedThreadId;
    const cursor = olderCursor;
    try {
      const res = await hub.rpc<{ entries?: EntryItem[]; next?: string | null }>("thread.entries", {
        thread: threadId,
        limit: 40,
        before: cursor,
      });
      if (gen !== threadGen || selectedThreadId !== threadId) return;
      olderCursor = res.next ?? null;
      chat.prependEntries(res.entries ?? [], Boolean(olderCursor));
    } catch {
      if (gen === threadGen) chat.clearLoadingOlder();
    }
  }

  async function createThread(projectId: string): Promise<void> {
    try {
      const res = await hub.rpc<{ thread?: ThreadItem }>("thread.create", { project: projectId });
      if (res.thread) {
        threads = [res.thread, ...threads.filter((x) => x.id !== res.thread!.id)];
        sidebar.setData(projects, threads, bots, rooms);
        void refreshSections();
        await openThread(res.thread.id);
        composer.focus();
      }
    } catch (e) {
      showSendError(e, "Could not create thread");
    }
  }

  /** Reply on a message (feature `reply`): the message the composer's strip answers, for the selected thread only. */
  let replying: { id: string; text: string; role: "user" | "assistant" } | null = null;
  function replyLine(t0: string): string {
    return (t0.split("\n").map((l) => l.trim()).find((l) => l.length > 0) ?? "").slice(0, 200);
  }
  function startReply(r: { id: string; text: string; role: "user" | "assistant" } | null): void {
    replying = r;
    composer.setReply(
      r ? { who: r.role === "user" ? t("secret.you", "You") : t("model.col.agent", "Agent"), line: replyLine(r.text) } : null,
      () => { if (replying) void goToEntry(replying.id); },
      () => startReply(null),
    );
  }

  async function sendMessage(
    text: string,
    attachments: Array<{ name: string; mime: string; data: string }> = [],
  ): Promise<void> {
    if (!selectedThreadId) return;
    const gen = threadGen;
    const threadId = selectedThreadId;
    const clientId = makeClientId();
    const localId = `local-${clientId}`;
    const reply = replying && text.trim() && hub.hasFeature("reply") ? replying : null;
    if (reply) startReply(null);
    chat.upsertEntry({
      id: localId,
      kind: "user",
      text,
      at: Date.now(),
      clientId,
      ...(reply ? { replyTo: { id: reply.id, quote: replyLine(reply.text) } } : {}),
      attachments: attachments.map((a) => ({
        name: a.name,
        mime: a.mime,
        previewUrl: `data:${a.mime};base64,${a.data}`,
      })),
    });
    try {
      const params: Record<string, unknown> = {
        thread: threadId,
        text,
        clientId,
      };
      if (attachments.length > 0) params.attachments = attachments;
      if (reply) params.replyTo = { id: reply.id, text: reply.text };
      const res = await hub.rpc<{ entry?: EntryItem }>("message.send", params);
      if (gen !== threadGen || selectedThreadId !== threadId) return;
      if (res.entry) chat.upsertEntry(res.entry);
    } catch (e) {
      if (gen === threadGen && selectedThreadId === threadId) {
        chat.removeEntry(localId);
        composer.setDraft(text);
        if (reply && !replying) startReply(reply);
        showSendError(e, "Failed to send message");
      }
    }
  }

  async function runSlashCommand(text: string): Promise<void> {
    if (!selectedThreadId) return;
    const gen = threadGen;
    const threadId = selectedThreadId;
    try {
      const res = await hub.rpc<{ kind?: string; q?: string; a?: string; text?: string; repo?: string; error?: string; errorCode?: string; machine?: string }>("composer.run", {
        thread: threadId,
        text,
      });
      if (gen !== threadGen || selectedThreadId !== threadId) return;
      if (res.error) {
        composer.setDraft(text);
        showSendError(new HubRpcError(res.error, { code: res.errorCode, machine: res.machine }), res.error);
      } else if (res.kind === "btw" && typeof res.a === "string") {
        chat.showBtw(res.q ?? "", res.a);
      } else if (res.kind === "edit" && typeof res.text === "string") {
        composer.setDraft(res.text);
      } else if (res.kind === "project") {
        addProject.open({ mode: "folder" });
      } else if (res.kind === "clone") {
        addProject.open({ mode: "clone", repo: res.repo ?? "", text: res.text ?? "" });
      }
    } catch (e) {
      if (gen === threadGen && selectedThreadId === threadId) {
        composer.setDraft(text);
        showSendError(e, "Command refused");
      }
    }
  }

  async function runThreadAct(threadId: string, action: string): Promise<void> {
    try {
      const res = await hub.rpc<{ thread?: ThreadItem }>("thread.act", { thread: threadId, action });
      if (res.thread) {
        threads = threads.map((x) => (x.id === res.thread!.id ? res.thread! : x));
        sidebar.setData(projects, threads, bots, rooms);
        if (res.thread.id === selectedThreadId) {
          chat.updateThreadMeta(res.thread);
          updateHeader();
        }
      }
      void refreshSections();
    } catch (e) {
      composer.showError(e instanceof Error ? e.message : `Action "${action}" not allowed`);
    }
  }

  // While the hub is away after it said it was restarting: its words, for 5 minutes at most (then the usual Reconnecting…)
  function restartWords(r: { reason: string; build: string; text: string; at: number } | null, now: number): string {
    if (!r || !r.text || now - r.at >= 300_000) return "";
    const short = r.build.split("-").pop() ?? "";
    if (r.reason === "build" && short) return t("hub.restarting.build", r.text, { build: short });
    if (r.reason === "stop") return t("hub.stopped", r.text);
    return t("hub.restarting", r.text);
  }

  function renderConnStatus(state: string): void {
    if (state === "connected") {
      sidebar.setConnStatus(t("gpui.acct.connected", "Connected"), "bp-ok");
    } else if (state === "reconnecting" && restartWords(hub.restarting, Date.now())) {
      sidebar.setConnStatus(restartWords(hub.restarting, Date.now()), "");
    } else if (state === "reconnecting") {
      sidebar.setConnStatus("Reconnecting…", "bp-warn");
    } else if (state === "revoked") {
      sidebar.setConnStatus(t("gpui.acct.sign.in.expired", "Sign-in expired"), "bp-bad");
    } else {
      sidebar.setConnStatus("Connecting…", "");
    }
  }

  onLocaleChange(() => {
    renderConnStatus(hub.getState());
    impLbl.textContent = t("gpui.improvements", "Improvements");
    settingsBtn.title = t("palette.settings", "Settings");
    settingsBtn.setAttribute("aria-label", t("palette.settings", "Settings"));
    updateHeader();
    sidebar.refreshLocale();
    chat.refreshLocale();
    composer.refreshLocale();
    rail.render();
    settings.refreshLocale();
    projectHome.refreshLocale();
    addProject.refreshLocale();
  });

  hub.onState((state, hello: HelloFrame | null) => {
    if (state === "connected" && hello) {
      if (hello.locale) {
        hubLocale = hello.locale;
        applyEffectiveLocale();
      }
      if (hello.theme) applyHubTheme(hello.theme);
      if (hello.name) {
        hostDisplayName = hello.name;
        sidebar.setHostName(hello.name);
      }
      if (Array.isArray(hello.favoriteModels)) {
        favoriteModels = hello.favoriteModels;
        composer.updateFavoriteModels(favoriteModels);
      }
      homeInfo = hello.home ?? null;
      sidebar.setHome(homeInfo);
      applyNoProvider(parseNoProvider(hello.noProvider));
      if (settings.isOpen()) void settings.refresh();
      renderConnStatus(state);
      // back after a crash (hello.hubBack): said once, for a few seconds
      const back = typeof (hello as { hubBack?: unknown }).hubBack === "string" ? String((hello as { hubBack?: unknown }).hubBack) : "";
      if (back) { try { const b = JSON.parse(back) as { reason?: string; text?: string }; if (b.reason === "crash") { sidebar.setConnStatus(t("hub.back.crash", b.text ?? ""), "bp-warn"); window.setTimeout(() => renderConnStatus(hub.getState()), 6000); } } catch { /* nothing to say */ } }
      composer.setFeatures(hub.hasFeature("images"), hub.hasFeature("voice"));
      if (Array.isArray(hello.commands)) {
        helloCommands = hello.commands as SlashCommand[];
        composer.setCommands(helloCommands);
      } else {
        helloCommands = [];
      }
      void loadLists();
      // a clone's pushes may have been missed while the socket was down
      addProject.reconcile();
    } else {
      if (state === "revoked") {
        try { clearSidebarCache(window.localStorage); } catch { /* ignore */ }
      }
      voice.cancel();
      renderConnStatus(state);
    }
  });

  hub.onFrame((frame) => {
    if (frame.t === "voice") {
      voice.handleFrame(frame as Record<string, unknown>);
      return;
    }
    if (frame.t === "info" && frame.info && typeof frame.info === "object") {
      const info = frame.info as Record<string, unknown>;
      if (Object.prototype.hasOwnProperty.call(info, "provider.notice")) {
        applyNoProvider(parseNoProvider(info["provider.notice"]));
      }
      mgmt.handleInfo(info);
      return;
    }
    if (frame.t !== "d") return;
    const p = frame as DataPush;
    if (p.k === "clone" || p.k === "clone.repos" || p.k === "clone.signin") {
      addProject.handleFrame(p);
      return;
    }
    switch (p.k) {
      case "project": {
        const proj = p.project as ProjectItem | undefined;
        if (!proj) break;
        const idx = projects.findIndex((x) => x.id === proj.id);
        if (idx >= 0) projects[idx] = proj;
        else projects.unshift(proj);
        sidebar.setData(projects, threads, bots, rooms);
        updateHeader();
        scheduleSectionsRefresh();
        break;
      }
      case "thread": {
        const th = p.thread as ThreadItem | undefined;
        if (!th) break;
        const idx = threads.findIndex((x) => x.id === th.id);
        if (idx >= 0) threads[idx] = th;
        else threads.unshift(th);
        sidebar.setData(projects, threads, bots, rooms);
        if (th.id === selectedThreadId) {
          chat.updateThreadMeta(th);
          updateHeader();
          composer.setState(true, th.state === "working");
        }
        scheduleSectionsRefresh();
        break;
      }
      case "thread.removed": {
        const id = String(p.id ?? "");
        threads = threads.filter((x) => x.id !== id);
        sidebar.setData(projects, threads, bots, rooms);
        scheduleSectionsRefresh();
        break;
      }
      case "entry": {
        if (p.thread === selectedThreadId && p.entry) {
          chat.upsertEntry(p.entry as EntryItem);
        }
        break;
      }
      case "text": {
        if (p.thread === selectedThreadId) {
          chat.appendLiveText(String(p.entry ?? ""), String(p.append ?? ""), Number(p.at ?? Date.now()));
        }
        break;
      }
      case "turn": {
        if (p.thread === selectedThreadId) {
          const st = String(p.state ?? "idle");
          composer.setState(true, st === "working");
          void refreshRail(selectedThreadId, threadGen);
        }
        scheduleSectionsRefresh();
        break;
      }
      case "btw": {
        if (p.thread === selectedThreadId) {
          chat.showBtw(String(p.q ?? ""), String(p.a ?? ""));
        }
        break;
      }
      case "unanswered": {
        if (p.thread === selectedThreadId) {
          chat.setUnanswered((p.unanswered as UnansweredNotice | null) ?? null);
        }
        break;
      }
      case "theme": {
        if (typeof p.theme === "string") applyHubTheme(p.theme);
        break;
      }
      case "home": {
        homeInfo = (p.home as HomeInfo | null | undefined) ?? null;
        sidebar.setHome(homeInfo);
        if (settings.isOpen()) void settings.refresh();
        break;
      }
      case "favorites": {
        if (Array.isArray(p.favorites)) {
          favoriteModels = p.favorites.filter((x): x is string => typeof x === "string");
          composer.updateFavoriteModels(favoriteModels);
        }
        break;
      }
      default:
        break;
    }
  });

  window.addEventListener("keydown", (e) => {
    const key = e.key.toLowerCase();
    if ((e.ctrlKey || e.metaKey) && (key === "k" || (e.shiftKey && key === "p"))) {
      e.preventDefault();
      if (palette.isOpen()) palette.close();
      else palette.open();
    } else if (e.shiftKey && e.code === "KeyN" && (e.altKey || e.ctrlKey || e.metaKey)) {
      // New chat. A browser keeps Ctrl/Cmd+Shift+N for itself (it never arrives here); Alt+Shift+N always does
      e.preventDefault();
      void createThread(CHATS);
    } else if (e.key === "Escape") {
      const moving = document.querySelector('[data-testid="move-project-modal"]');
      if (moving) { moving.remove(); return; }
      if (palette.isOpen()) palette.close();
      else if (composer.isModelPickerOpen()) composer.closeModelPicker();
      else if (addProject.isOpen()) addProject.escape();
      else if (settings.isOpen()) settings.handleEscape();
      else if (chat.closePlanRevise()) { /* handled by plan revise drawer */ }
      else if (mgmt.isOpen()) mgmt.closePanel();
    }
  });

  try {
    const cached = loadSidebarCache(window.localStorage);
    if (cached) {
      if (cached.hostName) {
        hostDisplayName = cached.hostName;
        sidebar.setHostName(cached.hostName);
      }
      projects = cached.projects;
      threads = cached.threads;
      bots = cached.bots;
      rooms = cached.rooms;
      sections = cached.sections;
      shellHashes = cached.shellHashes;
      for (const f of cached.openSectionFolds) openSectionFolds.add(f);
      sidebar.setData(projects, threads, bots, rooms);
      sidebar.setSections(sections, bots, rooms);
    }
  } catch { /* ignore */ }

  hub.connect();
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", mountApp);
} else {
  mountApp();
}
