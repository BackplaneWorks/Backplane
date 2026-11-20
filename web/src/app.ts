// Backplane Web UI main shell matching GPUI native-chat.png:
// Unified top bar (breadcrumb, STATE dot+word, Pin/Settle with icons, command box, gear),
// sidebar (This computer, ACTIVE/BOTS/ROOMS, bottom-left Connected status),
// bottom-anchored chat timeline, tall composer + toolbar row, hub-driven right rail
// with PR #1367 `has` reconciliation + `railGen` staleness guard, and Settings › Devices modal.
import { ChatView, type EntryItem, type ReviewBar, type UnansweredNotice } from "./chat";
import { ComposerView, type MentionItem, type SlashCommand } from "./composer";
import { onLocaleChange, setLocale, t } from "./i18n";
import { CommandPalette, type PaletteAction, type PaletteSearchResult } from "./palette";
import { RailView, type RailEntry } from "./rail";
import { SettingsModal, type SettingsRow } from "./settings";
import {
  SidebarView,
  type BotItem,
  type ProjectItem,
  type RoomItem,
  type SectionProjectRow,
  type SectionThreadRow,
  type SidebarSection,
  type ThreadItem,
} from "./sidebar";
import { VoiceController } from "./voice";
import { HubClient, type DataPush, type HelloFrame } from "./ws";
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

function hasForKind(has: RailHas | undefined, kind: string): boolean {
  if (!has) return true;
  if (kind === "board" || kind === "3d") return has.board !== false;
  if (kind === "schematic") return has.schematic !== false;
  if (kind === "mech") return has.mech !== false;
  if (kind === "gerbers") return has.gerbers !== false;
  return true;
}

function applyHubTheme(theme: string | undefined): void {
  if (!theme) return;
  const light = theme === "light";
  document.documentElement.classList.toggle("light", light);
  try { localStorage.setItem("bp.theme", light ? "light" : "dark"); } catch { /* ignore */ }
}

function makeClientId(): string {
  return `web-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
}

function mountApp(): void {
  const root = document.getElementById("root");
  if (!root) return;

  try {
    if (localStorage.getItem("bp.theme") === "light") document.documentElement.classList.add("light");
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
  brand.append(logo, brandName, sep1, projCrumb, sep2, threadCrumb);

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

  const viewerEl = document.createElement("section");
  viewerEl.className = "bp-viewer-pane bp-hidden";
  viewerEl.setAttribute("data-testid", "viewer-pane");

  const railEl = document.createElement("nav");
  railEl.className = "bp-rail";
  railEl.setAttribute("aria-label", "Viewer rail");
  railEl.setAttribute("data-testid", "viewer-rail");

  body.append(sidebarEl, centerEl, viewerEl, railEl);
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
  let railItems: RailEntry[] = [];
  let helloCommands: SlashCommand[] = [];
  let selectedThreadId = "";
  let threadGen = 0;
  let railGen = 0;
  let threadSubId: SubId | null = null;
  let olderCursor: string | null = null;
  let activeRailKind = "";
  let viewerController: ReturnType<ViewerMountFn> | null = null;
  const threadCache = new Map<string, {
    entries: EntryItem[];
    olderCursor: string | null;
    review: ReviewBar | null;
    unanswered: UnansweredNotice | null;
  }>();

  const sidebar = new SidebarView(sidebarEl, {
    onSelectThread: (id) => {
      if (id === selectedThreadId && threadSubId !== null) {
        sidebarEl.classList.remove("bp-sidebar-open");
        return;
      }
      void openThread(id);
    },
    onNewThread: (projectId) => { void createThread(projectId); },
    onSelectBot: (bot) => { void openThread(bot.thread); },
    onThreadAct: (id, action) => { void runThreadAct(id, action); },
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
  });

  const chat = new ChatView(centerEl, {
    onLoadOlder: () => { void loadOlderEntries(); },
    onAnswerAsk: (askId, answer) => {
      void hub.rpc("ask.answer", { ask: askId, answer }).catch(() => {});
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
      const firstLine = (entry.text || "").split("\n")[0]?.slice(0, 100) ?? "";
      if (firstLine) composer.prependDraft(`> ${firstLine}\n`);
    },
    onLoadImage: async (entryId, path) => {
      try {
        const res = await hub.rpc<{ mime?: string; data?: string }>("message.image", { entry: entryId, path });
        if (res.mime && res.data) return { mime: res.mime, data: res.data };
      } catch { /* ignore */ }
      return null;
    },
  });

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

  const rail = new RailView(railEl, (id) => {
    if (id === "commands") {
      palette.open();
      return;
    }
    if (!selectedThreadId) return;
    if (activeRailKind === id) closeViewer();
    else openViewer(id);
  });

  const settings = new SettingsModal(root, {
    onLoadDevices: async () => {
      const res = await hub.rpc<{ rows?: SettingsRow[] }>("devices.page", {});
      return res.rows ?? [];
    },
    onDevicesAct: async (action, value) => {
      await hub.rpc("devices.page.act", { action, value });
    },
    onLogout: async () => {
      try {
        await fetch("/auth/web/logout", {
          method: "POST",
          credentials: "same-origin",
          headers: { "Content-Type": "application/json" },
          body: "{}",
        });
      } finally {
        hub.disconnect();
        window.location.replace("/");
      }
    },
  });

  pinBtn.addEventListener("click", () => {
    const th = threads.find((x) => x.id === selectedThreadId);
    if (th) void runThreadAct(th.id, th.pinned ? "unpin" : "pin");
  });
  settleBtn.addEventListener("click", () => {
    const th = threads.find((x) => x.id === selectedThreadId);
    if (th) void runThreadAct(th.id, th.settled ? "unsettle" : "settle");
  });
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

  impBtn.addEventListener("click", () => { void openImprovementsModal(); });
  cmdBox.addEventListener("click", () => palette.open());
  searchBtn.addEventListener("click", () => palette.open());
  settingsBtn.addEventListener("click", () => { void settings.open(); });
  sideToggle.addEventListener("click", () => {
    sidebarEl.classList.toggle("bp-sidebar-open");
    sidebarEl.classList.toggle("bp-sidebar-collapsed");
  });

  function updateHeader(): void {
    const th = threads.find((x) => x.id === selectedThreadId) ?? null;
    const proj = th ? projects.find((p) => p.id === th.project) : undefined;
    projCrumb.textContent = proj?.title ?? "";
    sep1.classList.toggle("bp-hidden", !proj);
    sep2.classList.toggle("bp-hidden", !th);
    threadCrumb.textContent = th ? (th.title || "Untitled thread") : "";

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
    if (notifyHub && selectedThreadId && hub.getState() === "connected") {
      void hub.rpc("viewer.watch", { thread: selectedThreadId, kind: "" }).catch(() => {});
    }
  }

  function openViewer(kind: string): void {
    railGen++;
    activeRailKind = kind;
    rail.setActive(kind);
    viewerEl.classList.remove("bp-hidden");
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
      updateHeader();
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

    if (hub.hasFeature("bots")) {
      try {
        const bRes = await hub.rpc<{ bots?: BotItem[]; rooms?: RoomItem[] }>("bots.list", {});
        bots = bRes.bots ?? [];
        rooms = bRes.rooms ?? [];
      } catch { /* ignore */ }
    }

    if (hub.hasFeature("sections")) {
      const [secRes, tRes] = await Promise.all([
        hub.rpc<{ sections?: SidebarSection[] }>("sidebar.sections", {
          open: Array.from(openSectionFolds),
        }).catch(() => null),
        hub.rpc<{ threads?: ThreadItem[] }>("threads.list", { limit: 200, archived: false }).catch(() => ({ threads: [] })),
      ]);
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
    } else {
      sections = null;
      const [pRes, tRes] = await Promise.all([
        hub.rpc<{ projects?: ProjectItem[] }>("projects.list", { limit: 100 }),
        hub.rpc<{ threads?: ThreadItem[] }>("threads.list", { limit: 200, archived: false }),
      ]);
      projects = pRes.projects ?? [];
      threads = tRes.threads ?? [];
      sidebar.setSections(null, bots, rooms);
      sidebar.setData(projects, threads, bots, rooms);
    }

    await hub.rpc("subscribe", { topic: "lists" }).catch(() => {});

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
      await openThread(targetId);
    } else {
      selectedThreadId = "";
      railItems = [];
      rail.setItems([]);
      composer.setState(false, false);
      updateHeader();
    }
  }

  async function openThread(threadId: string): Promise<void> {
    if (selectedThreadId && selectedThreadId !== threadId) {
      voice.cancel();
      const held = chat.getEntries();
      if (held.length > 0) {
        const prev = threadCache.get(selectedThreadId);
        threadCache.set(selectedThreadId, {
          entries: held,
          olderCursor,
          review: prev?.review ?? null,
          unanswered: prev?.unanswered ?? null,
        });
      }
    }

    const gen = ++threadGen;
    railGen++;
    const prevSub = threadSubId;
    threadSubId = null;
    if (prevSub !== null) {
      void hub.rpc("unsubscribe", { sub: prevSub }).catch(() => {});
    }

    selectedThreadId = threadId;
    const cached = threadCache.get(threadId);
    olderCursor = cached?.olderCursor ?? null;
    try { localStorage.setItem("bp.lastThread", threadId); } catch { /* ignore */ }
    sidebar.setSelected(threadId);
    sidebarEl.classList.remove("bp-sidebar-open");
    updateHeader();

    const th = threads.find((x) => x.id === threadId) ?? null;
    if (cached) {
      chat.setThread(th, cached.entries, Boolean(cached.olderCursor), cached.review, cached.unanswered);
    } else {
      chat.setLoading(th);
    }
    composer.setState(true, th?.state === "working");
    railItems = [];
    rail.setItems([]);

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
    threadCache.set(threadId, { entries, olderCursor, review, unanswered });
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
    if (activeRailKind) openViewer(activeRailKind);
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
      composer.showError(e instanceof Error ? e.message : "Could not create thread");
    }
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
    chat.upsertEntry({
      id: localId,
      kind: "user",
      text,
      at: Date.now(),
      clientId,
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
      const res = await hub.rpc<{ entry?: EntryItem }>("message.send", params);
      if (gen !== threadGen || selectedThreadId !== threadId) return;
      if (res.entry) chat.upsertEntry(res.entry);
    } catch (e) {
      if (gen === threadGen && selectedThreadId === threadId) {
        chat.removeEntry(localId);
        composer.setDraft(text);
        composer.showError(e instanceof Error ? e.message : "Failed to send message");
      }
    }
  }

  async function runSlashCommand(text: string): Promise<void> {
    if (!selectedThreadId) return;
    const gen = threadGen;
    const threadId = selectedThreadId;
    try {
      const res = await hub.rpc<{ kind?: string; text?: string; error?: string }>("composer.run", {
        thread: threadId,
        text,
      });
      if (gen !== threadGen || selectedThreadId !== threadId) return;
      if (res.error) {
        composer.setDraft(text);
        composer.showError(res.error);
      } else if (res.kind === "edit" && typeof res.text === "string") {
        composer.setDraft(res.text);
      }
    } catch (e) {
      if (gen === threadGen && selectedThreadId === threadId) {
        composer.setDraft(text);
        composer.showError(e instanceof Error ? e.message : "Command refused");
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

  function renderConnStatus(state: string): void {
    if (state === "connected") {
      sidebar.setConnStatus(t("gpui.acct.connected", "Connected"), "bp-ok");
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
  });

  hub.onState((state, hello: HelloFrame | null) => {
    if (state === "connected" && hello) {
      if (hello.locale) setLocale(hello.locale);
      if (hello.theme) applyHubTheme(hello.theme);
      if (hello.name) sidebar.setHostName(hello.name);
      renderConnStatus(state);
      composer.setFeatures(hub.hasFeature("images"), hub.hasFeature("voice"));
      if (Array.isArray(hello.commands)) {
        helloCommands = hello.commands as SlashCommand[];
        composer.setCommands(helloCommands);
      } else {
        helloCommands = [];
      }
      void loadLists();
    } else {
      voice.cancel();
      renderConnStatus(state);
    }
  });

  hub.onFrame((frame) => {
    if (frame.t === "voice") {
      voice.handleFrame(frame as Record<string, unknown>);
      return;
    }
    if (frame.t !== "d") return;
    const p = frame as DataPush;
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
    } else if (e.key === "Escape") {
      if (palette.isOpen()) palette.close();
      else if (settings.isOpen()) settings.close();
    }
  });

  hub.connect();
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", mountApp);
} else {
  mountApp();
}
