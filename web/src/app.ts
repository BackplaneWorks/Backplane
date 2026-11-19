// Backplane Web UI main shell: connects over data/1 JSON WS (cookie auth),
// wires sidebar, chat timeline, composer, hub-driven right rail, viewer host slot, and settings.
import { ChatView, type EntryItem, type ReviewBar, type UnansweredNotice } from "./chat";
import { ComposerView, type MentionItem, type SlashCommand } from "./composer";
import { setLocale } from "./i18n";
import { DEFAULT_RAIL_ITEMS, RailView, type RailEntry } from "./rail";
import { SettingsModal, type SettingsRow } from "./settings";
import { SidebarView, type BotItem, type ProjectItem, type RoomItem, type ThreadItem } from "./sidebar";
import { HubClient, type DataPush, type HelloFrame } from "./ws";

export interface ViewerHostApi {
  hub: HubClient;
  getThreadId: () => string;
  closeViewer: () => void;
}

type ViewerMountFn = (el: HTMLElement, api: ViewerHostApi) => {
  open: (kind: string, threadId: string) => void;
  close: () => void;
};

declare global {
  interface Window {
    __bpMountViewerPane?: ViewerMountFn;
  }
}

function applyTheme(theme: string | undefined): void {
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
    const saved = localStorage.getItem("bp.theme");
    if (saved === "light") document.documentElement.classList.add("light");
  } catch { /* ignore */ }

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
  const crumbEl = document.createElement("span");
  crumbEl.className = "bp-crumb";
  crumbEl.textContent = "Backplane";
  brand.append(logo, crumbEl);

  const topRight = document.createElement("div");
  topRight.className = "bp-top-right";

  const connEl = document.createElement("span");
  connEl.className = "bp-conn-status";
  connEl.textContent = "Connecting…";
  connEl.setAttribute("data-testid", "conn-status");

  const settingsBtn = document.createElement("button");
  settingsBtn.type = "button";
  settingsBtn.className = "bp-btn-ghost";
  settingsBtn.textContent = "Devices";
  settingsBtn.setAttribute("data-testid", "settings-btn");

  topRight.append(connEl, settingsBtn);
  topbar.append(sideToggle, brand, topRight);

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
  let projects: ProjectItem[] = [];
  let threads: ThreadItem[] = [];
  let bots: BotItem[] = [];
  let rooms: RoomItem[] = [];
  let selectedThreadId = "";
  let threadSubId = "";
  let olderCursor: string | null = null;
  let activeRailKind = "";
  let viewerController: ReturnType<ViewerMountFn> | null = null;

  const sidebar = new SidebarView(sidebarEl, {
    onSelectThread: (id) => { void openThread(id); },
    onNewThread: (projectId) => { void createThread(projectId); },
    onSelectBot: (bot) => { void openThread(bot.thread); },
    onThreadAct: (id, action) => { void runThreadAct(id, action); },
  });

  const chat = new ChatView(centerEl, {
    onLoadOlder: () => { void loadOlderEntries(); },
    onAnswerAsk: (askId, answer) => {
      void hub.rpc("ask.answer", { ask: askId, answer }).catch(() => {});
    },
    onToggleReview: (nextOn) => {
      if (!selectedThreadId) return;
      void hub.rpc<{ review?: ReviewBar | null }>("thread.review", { thread: selectedThreadId, on: nextOn })
        .then((r) => chat.setReview(r.review ?? null))
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
    onThreadAction: (action) => {
      if (selectedThreadId) void runThreadAct(selectedThreadId, action);
    },
    onLoadImage: async (entryId, path) => {
      try {
        const res = await hub.rpc<{ mime?: string; data?: string }>("message.image", { entry: entryId, path });
        if (res.mime && res.data) return { mime: res.mime, data: res.data };
      } catch { /* ignore */ }
      return null;
    },
  });

  const composer = new ComposerView(centerEl, {
    onSend: (text) => { void sendMessage(text); },
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
  });

  const rail = new RailView(railEl, (id) => {
    if (id === "commands") {
      sidebar.focusFilter();
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
    onSetTheme: (theme) => applyTheme(theme),
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

  settingsBtn.addEventListener("click", () => { void settings.open(); });
  sideToggle.addEventListener("click", () => {
    sidebarEl.classList.toggle("bp-sidebar-open");
    sidebarEl.classList.toggle("bp-sidebar-collapsed");
  });

  function updateCrumb(): void {
    const th = threads.find((x) => x.id === selectedThreadId);
    const proj = th ? projects.find((p) => p.id === th.project) : undefined;
    const hubName = hub.hello?.name ? ` · ${hub.hello.name}` : "";
    crumbEl.textContent = proj ? `Backplane / ${proj.title}${hubName}` : `Backplane${hubName}`;
  }

  function closeViewer(): void {
    activeRailKind = "";
    rail.setActive("");
    viewerEl.classList.add("bp-hidden");
    viewerController?.close();
    if (selectedThreadId && hub.getState() === "connected") {
      void hub.rpc("viewer.watch", { thread: selectedThreadId, kind: "" }).catch(() => {});
    }
  }

  function openViewer(kind: string): void {
    activeRailKind = kind;
    rail.setActive(kind);
    viewerEl.classList.remove("bp-hidden");
    if (!viewerController && typeof window.__bpMountViewerPane === "function") {
      viewerController = window.__bpMountViewerPane(viewerEl, {
        hub,
        getThreadId: () => selectedThreadId,
        closeViewer,
      });
    }
    if (viewerController) {
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
      closeBtn.addEventListener("click", closeViewer);
      head.append(lbl, closeBtn);
      const bodyMsg = document.createElement("div");
      bodyMsg.className = "bp-dim bp-viewer-msg";
      bodyMsg.textContent = `Viewer pane ready (${kind}).`;
      box.append(head, bodyMsg);
      viewerEl.replaceChildren(box);
    }
  }

  async function refreshRail(threadId: string): Promise<void> {
    if (!threadId || !hub.hasFeature("rail")) {
      rail.setItems(DEFAULT_RAIL_ITEMS);
      return;
    }
    try {
      const res = await hub.rpc<{ rail?: RailEntry[]; items?: RailEntry[] }>("viewer.rail", { thread: threadId });
      const items = res.rail ?? res.items ?? [];
      rail.setItems(items.length > 0 ? items : DEFAULT_RAIL_ITEMS);
    } catch {
      rail.setItems(DEFAULT_RAIL_ITEMS);
    }
  }

  async function loadLists(): Promise<void> {
    const [pRes, tRes] = await Promise.all([
      hub.rpc<{ projects?: ProjectItem[] }>("projects.list", { limit: 100 }),
      hub.rpc<{ threads?: ThreadItem[] }>("threads.list", { limit: 200, archived: false }),
    ]);
    projects = pRes.projects ?? [];
    threads = tRes.threads ?? [];

    if (hub.hasFeature("bots")) {
      try {
        const bRes = await hub.rpc<{ bots?: BotItem[]; rooms?: RoomItem[] }>("bots.list", {});
        bots = bRes.bots ?? [];
        rooms = bRes.rooms ?? [];
      } catch { /* ignore */ }
    }

    sidebar.setData(projects, threads, bots, rooms);
    await hub.rpc("subscribe", { topic: "lists" }).catch(() => {});

    if (!selectedThreadId) {
      let saved = "";
      try { saved = localStorage.getItem("bp.lastThread") || ""; } catch { /* ignore */ }
      const candidate =
        threads.find((x) => x.id === saved) ??
        threads.find((x) => !x.archived && !x.swarm && !x.settled) ??
        threads.find((x) => !x.archived && !x.swarm);
      if (candidate) {
        await openThread(candidate.id);
      } else {
        composer.setState(false, false);
        updateCrumb();
      }
    } else {
      updateCrumb();
    }
  }

  async function openThread(threadId: string): Promise<void> {
    if (threadSubId) {
      void hub.rpc("unsubscribe", { sub: threadSubId }).catch(() => {});
      threadSubId = "";
    }
    selectedThreadId = threadId;
    try { localStorage.setItem("bp.lastThread", threadId); } catch { /* ignore */ }
    sidebar.setSelected(threadId);
    sidebarEl.classList.remove("bp-sidebar-open");
    updateCrumb();

    const th = threads.find((x) => x.id === threadId) ?? null;
    composer.setState(true, th?.state === "working");

    const res = await hub.rpc<{
      entries?: EntryItem[];
      next?: string | null;
      unanswered?: UnansweredNotice | null;
    }>("thread.entries", { thread: threadId, limit: 40 });
    olderCursor = res.next ?? null;

    let review: ReviewBar | null = null;
    if (hub.hasFeature("reviewmode")) {
      try {
        const r = await hub.rpc<{ review?: ReviewBar | null }>("thread.review", { thread: threadId });
        review = r.review ?? null;
      } catch { /* ignore */ }
    }

    chat.setThread(th, res.entries ?? [], Boolean(olderCursor), review, res.unanswered ?? null);

    try {
      const sub = await hub.rpc<{ sub?: string }>("subscribe", { topic: "thread", thread: threadId });
      threadSubId = sub.sub ?? "";
    } catch { /* ignore */ }

    void refreshRail(threadId);
    if (activeRailKind) openViewer(activeRailKind);
  }

  async function loadOlderEntries(): Promise<void> {
    if (!selectedThreadId || !olderCursor) return;
    const res = await hub.rpc<{ entries?: EntryItem[]; next?: string | null }>("thread.entries", {
      thread: selectedThreadId,
      limit: 40,
      before: olderCursor,
    });
    olderCursor = res.next ?? null;
    chat.prependEntries(res.entries ?? [], Boolean(olderCursor));
  }

  async function createThread(projectId: string): Promise<void> {
    try {
      const res = await hub.rpc<{ thread?: ThreadItem }>("thread.create", { project: projectId });
      if (res.thread) {
        threads = [res.thread, ...threads.filter((x) => x.id !== res.thread!.id)];
        sidebar.setData(projects, threads, bots, rooms);
        await openThread(res.thread.id);
        composer.focus();
      }
    } catch (e) {
      composer.showError(e instanceof Error ? e.message : "Could not create thread");
    }
  }

  async function sendMessage(text: string): Promise<void> {
    if (!selectedThreadId) return;
    const clientId = makeClientId();
    chat.upsertEntry({
      id: `local-${clientId}`,
      kind: "user",
      text,
      at: Date.now(),
      clientId,
    });
    try {
      const res = await hub.rpc<{ entry?: EntryItem }>("message.send", {
        thread: selectedThreadId,
        text,
        clientId,
      });
      if (res.entry) chat.upsertEntry(res.entry);
    } catch (e) {
      composer.showError(e instanceof Error ? e.message : "Failed to send message");
    }
  }

  async function runSlashCommand(text: string): Promise<void> {
    if (!selectedThreadId) return;
    try {
      const res = await hub.rpc<{ kind?: string; text?: string; error?: string }>("composer.run", {
        thread: selectedThreadId,
        text,
      });
      if (res.error) {
        composer.setDraft(text);
        composer.showError(res.error);
      } else if (res.kind === "edit" && typeof res.text === "string") {
        composer.setDraft(res.text);
      }
    } catch (e) {
      composer.setDraft(text);
      composer.showError(e instanceof Error ? e.message : "Command refused");
    }
  }

  async function runThreadAct(threadId: string, action: string): Promise<void> {
    try {
      const res = await hub.rpc<{ thread?: ThreadItem }>("thread.act", { thread: threadId, action });
      if (res.thread) {
        threads = threads.map((x) => (x.id === res.thread!.id ? res.thread! : x));
        sidebar.setData(projects, threads, bots, rooms);
        chat.updateThreadMeta(res.thread);
      }
    } catch (e) {
      composer.showError(e instanceof Error ? e.message : `Action "${action}" not allowed`);
    }
  }

  hub.onState((state, hello: HelloFrame | null) => {
    if (state === "connected" && hello) {
      connEl.textContent = "Connected";
      connEl.className = "bp-conn-status bp-ok";
      if (hello.theme) applyTheme(hello.theme);
      if (hello.locale) setLocale(hello.locale);
      if (Array.isArray(hello.commands)) composer.setCommands(hello.commands as SlashCommand[]);
      void loadLists();
    } else if (state === "reconnecting") {
      connEl.textContent = "Reconnecting…";
      connEl.className = "bp-conn-status bp-warn";
    } else if (state === "revoked") {
      connEl.textContent = "Signed out";
      connEl.className = "bp-conn-status bp-bad";
    } else {
      connEl.textContent = "Connecting…";
      connEl.className = "bp-conn-status";
    }
  });

  hub.onFrame((frame) => {
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
        updateCrumb();
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
          composer.setState(true, th.state === "working");
        }
        break;
      }
      case "thread.removed": {
        const id = String(p.id ?? "");
        threads = threads.filter((x) => x.id !== id);
        sidebar.setData(projects, threads, bots, rooms);
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
        }
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
        if (typeof p.theme === "string") applyTheme(p.theme);
        break;
      }
      default:
        break;
    }
  });

  window.addEventListener("keydown", (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
      e.preventDefault();
      sidebar.focusFilter();
    } else if (e.key === "Escape" && settings.isOpen()) {
      settings.close();
    }
  });

  hub.connect();
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", mountApp);
} else {
  mountApp();
}
