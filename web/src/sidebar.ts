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
  memory?: import("./composer").MemoryProgress | null;
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

export interface SectionThreadRow {
  id: string;
  title: string;
  word?: string;
  say?: string;
  want?: string;
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
}

export interface SectionProjectRow {
  id: string;
  title: string;
  key?: string;
  nav?: string;
  css?: string;
  color?: string;
  pinned?: boolean;
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
  onSelectBot: (bot: BotItem) => void;
  onThreadAct: (threadId: string, action: string) => void;
  onToggleSection?: (toggle: string) => void;
  onToggleProjectFold?: (projectKey: string) => void;
  onCollapseSidebar?: () => void;
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

function statusClassFromWords(status?: string, word?: string, state?: string, waiting?: number): string {
  const s = status || word || state || "";
  if (s === "active" || s === "working" || s === "running" || s === "run") return "bp-dot-work";
  if (s === "finished" || s === "done") return "bp-dot-done";
  if (s === "waiting" || s === "asking" || s === "blocked" || s === "needs" || s === "failed" || s === "fail" || s === "cut" || s === "interrupted" || s === "limit" || s === "monitoring" || (waiting ?? 0) > 0) return "bp-dot-need";
  return "bp-dot-idle";
}

function statusClass(th: ThreadItem): string {
  return statusClassFromWords(th.status, undefined, th.state, th.waiting);
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

  private sections: SidebarSection[] | null = null;
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
    const ph = t("sidebar.search.projects", "Search projects");
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

    head.append(this.filterInput, quickAdd);

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
    const ph = t("sidebar.search.projects", "Search projects");
    this.filterInput.placeholder = ph;
    this.filterInput.setAttribute("aria-label", ph);
    this.render();
  }

  private renderSectionThreadRow(row: SectionThreadRow, showTreeBranch: boolean): HTMLElement {
    const el = document.createElement("div");
    el.className = "bp-thread-row" +
      (row.id === this.selectedThreadId ? " active" : "") +
      (row.off ? " bp-dim" : "");
    el.setAttribute("data-thread-id", row.id);
    el.setAttribute("data-testid", `thread-row-${row.id}`);

    if (showTreeBranch) {
      const branch = document.createElement("span");
      branch.className = "bp-tree-branch";
      branch.setAttribute("aria-hidden", "true");
      el.appendChild(branch);
    }

    const dot = document.createElement("span");
    dot.className = `bp-dot ${statusClassFromWords(row.status, row.word, row.state)}`;

    const title = document.createElement("div");
    title.className = "bp-thread-title";
    title.textContent = row.title || "Untitled thread";

    el.append(dot, title);

    if (row.word === "needs") {
      const badge = document.createElement("span");
      badge.className = "bp-badge-need";
      badge.textContent = row.want === "approval" ? "!" : "•";
      if (row.say) badge.title = row.say;
      el.appendChild(badge);
    } else if (row.ago) {
      const age = document.createElement("span");
      age.className = "bp-thread-age";
      age.textContent = row.ago;
      if (row.pinned) age.title = t("tools.pinned", "Pinned");
      el.appendChild(age);
    }

    const target = row.nav || row.id;
    if (!row.off && target && !target.startsWith("@room:")) {
      el.addEventListener("click", () => this.cb.onSelectThread(target));
    }
    return el;
  }

  private renderHubSections(frag: DocumentFragment, q: string): void {
    const sections = this.sections ?? [];
    const listedThreadIds = new Set<string>();
    for (const sec of sections) {
      if (sec.kind === "threads") {
        for (const r of sec.rows as SectionThreadRow[]) listedThreadIds.add(r.id);
      } else if (sec.kind === "projects") {
        for (const p of sec.rows as SectionProjectRow[]) {
          for (const th of p.threads ?? []) listedThreadIds.add(th.id);
        }
      }
    }
    const selectedMeta = this.selectedThreadId
      ? this.threads.find((x) => x.id === this.selectedThreadId) ?? null
      : null;

    let anyProjectRows = false;
    for (const sec of sections) {
      if (sec.kind === "projects" && sec.rows.length > 0) anyProjectRows = true;
    }

    for (const sec of sections) {
      if (sec.kind === "bub") continue;

      if (sec.kind === "threads") {
        const rows = (sec.rows as SectionThreadRow[]).filter((r) => {
          if (!q) return true;
          return `${r.title} ${r.project ?? ""}`.toLowerCase().includes(q);
        });
        if (rows.length === 0) continue;

        const secWrap = document.createElement("section");
        secWrap.className = "bp-side-section";
        secWrap.setAttribute("data-section-id", sec.id);

        const head = document.createElement("div");
        head.className = "bp-side-section-head";
        head.textContent = sec.title.toUpperCase();
        secWrap.appendChild(head);

        for (const r of rows) {
          secWrap.appendChild(this.renderSectionThreadRow(r, false));
        }
        frag.appendChild(secWrap);
        continue;
      }

      if (sec.kind === "projects") {
        const projRows = (sec.rows as SectionProjectRow[]).filter((p) => {
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
        secWrap.className = "bp-side-section";
        secWrap.setAttribute("data-section-id", sec.id);

        if (isFoldable) {
          const headBtn = document.createElement("button");
          headBtn.type = "button";
          headBtn.className = "bp-side-section-head bp-side-section-toggle";
          const count = sec.count ?? projRows.length;
          const arrow = sec.open ? "▾" : "▸";
          headBtn.textContent = `${sec.title.toUpperCase()} (${count}) ${arrow}`;
          if (sec.hint) headBtn.title = sec.hint;
          headBtn.addEventListener("click", () => {
            if (sec.toggle) this.cb.onToggleSection?.(sec.toggle);
          });
          secWrap.appendChild(headBtn);
        } else {
          const head = document.createElement("div");
          head.className = "bp-side-section-head";
          head.textContent = sec.title.toUpperCase();
          secWrap.appendChild(head);
        }

        if (!isFoldable || sec.open || Boolean(q)) {
          if (sec.id === "active" && !anyProjectRows && !q) {
            const empty = document.createElement("div");
            empty.className = "bp-side-empty";
            empty.textContent = "No projects yet.";
            secWrap.appendChild(empty);
          }

          for (const proj of projRows) {
            const group = document.createElement("div");
            group.className = "bp-proj-group" + (proj.off ? " bp-dim" : "");
            group.setAttribute("data-project-id", proj.id);
            if (proj.nav || proj.key) group.setAttribute("data-project-key", proj.nav || proj.key || "");

            const header = document.createElement("div");
            header.className = "bp-proj-head";

            const toggle = document.createElement("button");
            toggle.type = "button";
            toggle.className = "bp-proj-toggle";
            const folder = makeFolderIcon();
            if (proj.css) folder.style.color = proj.css;
            const nameSpan = document.createElement("span");
            nameSpan.className = "bp-proj-name";
            nameSpan.textContent = proj.title || proj.id;
            toggle.append(folder, nameSpan);
            if (proj.fold === "project-fold" && proj.nav) {
              toggle.addEventListener("click", () => this.cb.onToggleProjectFold?.(proj.nav!));
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
              this.cb.onNewThread(proj.id);
            });

            header.append(toggle, addBtn);
            group.appendChild(header);

            if (!proj.folded || Boolean(q)) {
              const projNameMatches = Boolean(q) && (proj.title || proj.id).toLowerCase().includes(q);
              for (const sw of proj.swarms ?? []) {
                if (q && !projNameMatches && !sw.title.toLowerCase().includes(q)) continue;
                group.appendChild(this.renderSectionThreadRow(sw, true));
              }
              for (const th of proj.threads ?? []) {
                if (q && !projNameMatches && !th.title.toLowerCase().includes(q)) continue;
                group.appendChild(this.renderSectionThreadRow(th, true));
              }
              // Mirror GPUI's `on_show` rule (`sections.rs:340-341`): since `data/1` `sidebar.sections`
              // passes `selected: ""`, keep the currently selected thread visible under its project
              // without altering the order of hub-returned rows.
              if (
                selectedMeta &&
                selectedMeta.project === proj.id &&
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
    const frag = document.createDocumentFragment();
    const q = this.filterText;

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

    if (!q) {
      const botSec = document.createElement("section");
      botSec.className = "bp-proj-group bp-bots-group";
      const botHead = document.createElement("div");
      botHead.className = "bp-side-section-head bp-side-section-row";
      const botLbl = document.createElement("span");
      botLbl.textContent = t("tour.title.bots", "Bots").toUpperCase();
      const botAdd = document.createElement("span");
      botAdd.className = "bp-side-sec-plus";
      botAdd.textContent = "+";
      botAdd.setAttribute("aria-hidden", "true");
      botHead.append(botLbl, botAdd);
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
      roomHead.className = "bp-side-section-head bp-side-section-row";
      const roomLbl = document.createElement("span");
      roomLbl.textContent = t("bots.rooms", "Rooms").toUpperCase();
      const roomAdd = document.createElement("span");
      roomAdd.className = "bp-side-sec-plus";
      roomAdd.textContent = "+";
      roomAdd.setAttribute("aria-hidden", "true");
      roomHead.append(roomLbl, roomAdd);
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
