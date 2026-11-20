// Left sidebar matching GPUI native-chat.png:
// "This computer" header, "Search projects" input, ACTIVE section with folder icons
// and indented thread rows with status dots + relative age, BOTS & ROOMS sections,
// and bottom-left Connected status footer.
import { t } from "./i18n";

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
}

export interface BotItem {
  id: string;
  name: string;
  thread: string;
  color?: string;
  uber?: boolean;
}

export interface RoomItem {
  id: string;
  name: string;
}

export interface SidebarCallbacks {
  onSelectThread: (threadId: string) => void;
  onNewThread: (projectId: string) => void;
  onSelectBot: (bot: BotItem) => void;
  onThreadAct: (threadId: string, action: string) => void;
}

const SVG_NS = "http://www.w3.org/2000/svg";

function makeFolderIcon(): SVGSVGElement {
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("width", "14");
  svg.setAttribute("height", "14");
  svg.setAttribute("fill", "none");
  svg.setAttribute("stroke", "currentColor");
  svg.setAttribute("stroke-width", "1.75");
  svg.setAttribute("aria-hidden", "true");
  svg.classList.add("bp-folder-icon");
  const p = document.createElementNS(SVG_NS, "path");
  p.setAttribute("d", "M3 6h6l2 2h10v11H3z");
  svg.appendChild(p);
  return svg;
}

function statusClass(th: ThreadItem): string {
  const s = th.status || th.state;
  if (s === "active" || s === "working") return "bp-dot-work";
  if (s === "finished") return "bp-dot-done";
  if (s === "waiting" || s === "asking" || s === "blocked" || s === "failed" || (th.waiting ?? 0) > 0) return "bp-dot-need";
  return "bp-dot-idle";
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

export class SidebarView {
  private el: HTMLElement;
  private hostLabel: HTMLElement;
  private filterInput: HTMLInputElement;
  private listEl: HTMLElement;
  private connEl: HTMLElement;
  private filterText = "";
  private showSettled = new Set<string>();
  private collapsedProjects = new Set<string>();

  private projects: ProjectItem[] = [];
  private threads: ThreadItem[] = [];
  private bots: BotItem[] = [];
  private rooms: RoomItem[] = [];
  private selectedThreadId = "";
  private cb: SidebarCallbacks;

  constructor(container: HTMLElement, cb: SidebarCallbacks) {
    this.el = container;
    this.cb = cb;

    const hostRow = document.createElement("div");
    hostRow.className = "bp-side-host";
    this.hostLabel = document.createElement("span");
    this.hostLabel.textContent = "This computer";
    const chev = document.createElement("span");
    chev.className = "bp-dim";
    chev.textContent = "›";
    hostRow.append(this.hostLabel, chev);

    const head = document.createElement("div");
    head.className = "bp-side-head";

    this.filterInput = document.createElement("input");
    this.filterInput.type = "search";
    this.filterInput.className = "bp-input bp-side-search";
    const ph = t("gpui.filter.projects.and.threads", "Search projects");
    this.filterInput.placeholder = ph;
    this.filterInput.setAttribute("aria-label", ph);
    this.filterInput.setAttribute("data-testid", "sidebar-filter");
    this.filterInput.addEventListener("input", () => {
      this.filterText = this.filterInput.value.trim().toLowerCase();
      this.render();
    });
    head.appendChild(this.filterInput);

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

    this.el.append(hostRow, head, this.listEl, footer);
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

  public setSelected(threadId: string): void {
    this.selectedThreadId = threadId;
    this.render();
  }

  public focusFilter(): void {
    this.filterInput.focus();
  }

  public refreshLocale(): void {
    const ph = t("gpui.filter.projects.and.threads", "Search projects");
    this.filterInput.placeholder = ph;
    this.filterInput.setAttribute("aria-label", ph);
    this.render();
  }

  public render(): void {
    const frag = document.createDocumentFragment();
    const q = this.filterText;

    const byProject = new Map<string, ThreadItem[]>();
    const childrenByParent = new Map<string, ThreadItem[]>();
    for (const th of this.threads) {
      if (th.archived || th.swarm) continue;
      if (q) {
        const hay = `${th.title} ${th.preview ?? ""}`.toLowerCase();
        if (!hay.includes(q)) continue;
      }
      if (th.parent) {
        const list = childrenByParent.get(th.parent) ?? [];
        list.push(th);
        childrenByParent.set(th.parent, list);
      } else {
        const list = byProject.get(th.project) ?? [];
        list.push(th);
        byProject.set(th.project, list);
      }
    }

    const renderThreadRow = (th: ThreadItem, depth: number): HTMLElement => {
      const row = document.createElement("div");
      row.className = "bp-thread-row" +
        (th.id === this.selectedThreadId ? " active" : "") +
        (depth > 0 ? " nested" : "");
      row.setAttribute("data-thread-id", th.id);
      row.setAttribute("data-testid", `thread-row-${th.id}`);

      const branch = document.createElement("span");
      branch.className = "bp-tree-branch";
      branch.setAttribute("aria-hidden", "true");

      const dot = document.createElement("span");
      dot.className = `bp-dot ${statusClass(th)}`;

      const title = document.createElement("div");
      title.className = "bp-thread-title";
      title.textContent = th.title || "Untitled thread";

      row.append(branch, dot, title);

      if ((th.waiting ?? 0) > 0) {
        const badge = document.createElement("span");
        badge.className = "bp-badge-need";
        badge.textContent = String(th.waiting);
        row.appendChild(badge);
      } else {
        const age = document.createElement("span");
        age.className = "bp-thread-age";
        age.textContent = formatAge(th.updatedAt);
        if (th.pinned) age.title = t("tools.pinned", "Pinned");
        row.appendChild(age);
      }

      row.addEventListener("click", () => this.cb.onSelectThread(th.id));
      return row;
    };

    const activeHead = document.createElement("div");
    activeHead.className = "bp-side-section-head";
    activeHead.textContent = "ACTIVE";
    frag.appendChild(activeHead);

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
      toggle.addEventListener("click", () => {
        if (this.collapsedProjects.has(proj.id)) this.collapsedProjects.delete(proj.id);
        else this.collapsedProjects.add(proj.id);
        this.render();
      });

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
        const pinned = pThreads.filter((x) => x.pinned).sort((a, b) => (a.pinOrder ?? 0) - (b.pinOrder ?? 0));
        const active = pThreads.filter((x) => !x.pinned && !x.settled).sort((a, b) => b.updatedAt - a.updatedAt);
        const settled = pThreads.filter((x) => !x.pinned && x.settled).sort((a, b) => b.updatedAt - a.updatedAt);

        const appendWithKids = (list: ThreadItem[]) => {
          for (const th of list) {
            sec.appendChild(renderThreadRow(th, 0));
            const kids = childrenByParent.get(th.id);
            if (kids) {
              for (const k of kids.sort((a, b) => b.updatedAt - a.updatedAt)) {
                sec.appendChild(renderThreadRow(k, 1));
              }
            }
          }
        };

        appendWithKids(pinned);
        appendWithKids(active);

        if (settled.length > 0) {
          const open = this.showSettled.has(proj.id) || Boolean(q);
          const settledToggle = document.createElement("button");
          settledToggle.type = "button";
          settledToggle.className = "bp-settled-toggle";
          const settledLabel = t("notify.settled", "Settled");
          settledToggle.textContent = open ? `${settledLabel} (${settled.length}) ▾` : `${settledLabel} (${settled.length}) ▸`;
          settledToggle.addEventListener("click", () => {
            if (this.showSettled.has(proj.id)) this.showSettled.delete(proj.id);
            else this.showSettled.add(proj.id);
            this.render();
          });
          sec.appendChild(settledToggle);
          if (open) appendWithKids(settled);
        }
      }

      frag.appendChild(sec);
    }

    if (!q) {
      const botSec = document.createElement("section");
      botSec.className = "bp-proj-group bp-bots-group";
      const botHead = document.createElement("div");
      botHead.className = "bp-side-section-head";
      botHead.textContent = t("tour.title.bots", "Bots").toUpperCase();
      botSec.appendChild(botHead);
      for (const bot of this.bots) {
        const row = document.createElement("div");
        row.className = "bp-thread-row" + (bot.thread === this.selectedThreadId ? " active" : "");
        const dot = document.createElement("span");
        dot.className = "bp-dot bp-dot-idle";
        if (bot.color) dot.style.backgroundColor = bot.color;
        const name = document.createElement("div");
        name.className = "bp-thread-title";
        name.textContent = `@${bot.name}`;
        row.append(dot, name);
        row.addEventListener("click", () => this.cb.onSelectBot(bot));
        botSec.appendChild(row);
      }
      frag.appendChild(botSec);

      const roomSec = document.createElement("section");
      roomSec.className = "bp-proj-group";
      const roomHead = document.createElement("div");
      roomHead.className = "bp-side-section-head";
      roomHead.textContent = t("bots.rooms", "Rooms").toUpperCase();
      roomSec.appendChild(roomHead);
      for (const room of this.rooms) {
        const row = document.createElement("div");
        row.className = "bp-thread-row";
        const name = document.createElement("div");
        name.className = "bp-thread-title";
        name.textContent = `#${room.name}`;
        row.appendChild(name);
        roomSec.appendChild(row);
      }
      frag.appendChild(roomSec);
    }

    this.listEl.replaceChildren(frag);
  }
}
