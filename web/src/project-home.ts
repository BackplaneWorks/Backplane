// Project Home: the page a project's name opens (the desktop's project page, native/gpui/src/screens/projects.rs): the project and
// its folder, New thread, its threads, the goal and notes, the mechanical parts, what there is to order (with the last estimate's
// lead time and warnings) and its boards. Everything shown is the hub's (`project.home`, `threads.list`); nothing is priced or bought here.
import { t } from "./i18n";
import { intentIsEmpty, intentLines, sizeText, type ProjectHomeData } from "./project-home-model";

export interface HomeThreadRow {
  id: string;
  title: string;
  /** The dot's class (`bp-dot-work`, `bp-dot-need`, …), as the top bar draws the thread's state. */
  dot: string;
  /** The state's word, for the dot's tooltip. */
  status: string;
  age: string;
  settled?: boolean;
}

export interface ProjectHomeCallbacks {
  onNewThread: (projectId: string) => void;
  onOpenThread: (threadId: string) => void;
  /** A board or model row: its viewer, on the project's newest thread. */
  onOpenViewer: (threadId: string, kind: "board" | "mech") => void;
  onRetry: (projectId: string) => void;
}

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls: string, text?: string): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

export class ProjectHomeView {
  private root: HTMLElement;
  private titleEl: HTMLElement;
  private rootEl: HTMLElement;
  private newBtn: HTMLButtonElement;
  private newLbl: HTMLElement;
  private threadsEl: HTMLElement;
  private bodyEl: HTMLElement;
  private cb: ProjectHomeCallbacks;

  private projectId = "";
  private data: ProjectHomeData | null = null;
  private threads: HomeThreadRow[] | null = null;
  private error = "";

  constructor(container: HTMLElement, cb: ProjectHomeCallbacks) {
    this.cb = cb;
    this.root = el("section", "bp-project-home bp-hidden");
    this.root.setAttribute("data-testid", "project-home");

    const inner = el("div", "bp-home-inner");
    const head = el("header", "bp-home-head");
    this.titleEl = el("h1", "bp-home-title");
    this.titleEl.setAttribute("data-testid", "project-home-title");
    this.rootEl = el("div", "bp-home-root");
    head.append(this.titleEl, this.rootEl);

    const bar = el("div", "bp-home-bar");
    this.newBtn = el("button", "primary bp-home-new");
    this.newBtn.type = "button";
    this.newBtn.setAttribute("data-testid", "project-home-new-thread");
    const plus = el("span", "bp-home-new-plus", "+");
    plus.setAttribute("aria-hidden", "true");
    this.newLbl = el("span", "", t("thread.new", "New thread"));
    this.newBtn.append(plus, this.newLbl);
    this.newBtn.addEventListener("click", () => {
      if (this.projectId) this.cb.onNewThread(this.projectId);
    });
    bar.appendChild(this.newBtn);

    this.threadsEl = el("div", "bp-home-threads");
    this.bodyEl = el("div", "bp-home-body");
    inner.append(head, bar, this.threadsEl, this.bodyEl);
    this.root.appendChild(inner);
    container.appendChild(this.root);
  }

  public isOpen(): boolean {
    return this.projectId !== "";
  }

  /** The page for a project: its name at once, the rest when the hub answers (`setData`, `setThreads`). */
  public show(project: { id: string; title: string }): void {
    if (this.projectId !== project.id) {
      this.data = null;
      this.threads = null;
      this.root.scrollTop = 0;
    }
    this.error = "";
    this.projectId = project.id;
    this.titleEl.textContent = project.title || project.id;
    this.root.classList.remove("bp-hidden");
    this.render();
  }

  public hide(): void {
    this.projectId = "";
    this.root.classList.add("bp-hidden");
  }

  public setTitle(title: string): void {
    if (title) this.titleEl.textContent = title;
  }

  public setData(data: ProjectHomeData): void {
    this.data = data;
    this.error = "";
    this.render();
  }

  public setThreads(rows: HomeThreadRow[]): void {
    this.threads = rows;
    this.render();
  }

  public setError(message: string): void {
    this.error = message;
    this.render();
  }

  public refreshLocale(): void {
    this.newLbl.textContent = t("thread.new", "New thread");
    if (this.isOpen()) this.render();
  }

  private section(title: string): HTMLElement {
    return el("h2", "bp-home-section", title);
  }

  private render(): void {
    this.rootEl.textContent = this.data?.root ?? "";
    this.rootEl.classList.toggle("bp-hidden", !this.data?.root);
    this.renderThreads();
    this.renderBody();
  }

  private renderThreads(): void {
    const frag = document.createDocumentFragment();
    const rows = this.threads;
    if (rows) {
      frag.appendChild(this.section(t("gpui.projects.threads", "THREADS ({n})", { n: rows.length })));
      if (rows.length === 0) {
        frag.appendChild(el("div", "bp-home-empty", t("gpui.no.threads.yet", "No threads yet.")));
      }
      for (const r of rows) {
        const row = el("button", "bp-home-row bp-home-thread" + (r.settled ? " bp-dim" : ""));
        row.type = "button";
        row.setAttribute("data-testid", `project-home-thread-${r.id}`);
        const dot = el("span", `bp-dot ${r.dot}`);
        if (r.status) dot.title = r.status;
        const title = el("span", "bp-home-row-main", r.title || "Untitled thread");
        const age = el("span", "bp-home-row-faint", r.age);
        row.append(dot, title, age);
        row.addEventListener("click", () => this.cb.onOpenThread(r.id));
        frag.appendChild(row);
      }
    }
    this.threadsEl.replaceChildren(frag);
  }

  private renderBody(): void {
    const frag = document.createDocumentFragment();
    const d = this.data;

    if (this.error) {
      const box = el("div", "bp-home-error");
      box.setAttribute("data-testid", "project-home-error");
      box.appendChild(el("span", "", this.error));
      const retry = el("button", "bp-btn-ghost", t("tools.retry", "Retry"));
      retry.type = "button";
      const pid = this.projectId;
      retry.addEventListener("click", () => this.cb.onRetry(pid));
      box.appendChild(retry);
      frag.appendChild(box);
    }
    if (!d) {
      if (!this.error) frag.appendChild(el("div", "bp-home-empty", t("web.home.loading", "Loading project…")));
      this.bodyEl.replaceChildren(frag);
      return;
    }

    // the goal and notes
    frag.appendChild(this.section(t("gpui.goal.notes", "GOAL & NOTES")));
    const intent = el("div", "bp-home-intent");
    intent.setAttribute("data-testid", "project-home-intent");
    if (intentIsEmpty(d.intent)) {
      frag.appendChild(el(
        "div",
        "bp-home-empty",
        t("gpui.no.goal.yet.ask.an", "No goal yet. Ask an agent to write what this project is for, or edit .backplane/intent.md."),
      ));
    } else {
      for (const l of intentLines(d.intent)) {
        if (l.kind === "h") {
          intent.appendChild(el("div", "bp-home-intent-h", l.text));
        } else if (l.kind === "li") {
          const li = el("div", "bp-home-intent-li");
          const bullet = el("span", "bp-home-bullet", "•");
          bullet.setAttribute("aria-hidden", "true");
          li.append(bullet, el("span", "", l.text));
          intent.appendChild(li);
        } else {
          intent.appendChild(el("div", "", l.text));
        }
      }
      frag.appendChild(intent);
    }

    const top = this.threads?.[0]?.id ?? "";

    // the mechanical parts: the assembly (the file with the most in it) first
    if (d.mech.length > 0) {
      frag.appendChild(this.section(t("gpui.projects.mech", "MECHANICAL ({n})", { n: d.mech.length })));
      d.mech.forEach((m, i) => {
        const first = i === 0 && d.mech.length > 1;
        const row = this.fileRow(top, "mech", `project-home-mech-${i}`);
        row.title = m.path;
        row.appendChild(el("span", "bp-home-row-main", m.name || m.path));
        if (first) row.appendChild(el("span", "bp-home-row-accent", t("gpui.assembly", "Assembly")));
        row.appendChild(el("span", "bp-home-row-faint", sizeText(m.size)));
        frag.appendChild(row);
      });
    }

    // what there is to order, and what the last estimate said beyond a price
    if (d.parts.length > 0) {
      frag.appendChild(this.section(t("gpui.projects.to.order", "TO ORDER ({n})", { n: d.parts.length })));
      const card = el("div", "bp-home-order");
      card.setAttribute("data-testid", "project-home-order");
      d.parts.forEach((p, i) => {
        const row = el("div", "bp-home-row");
        row.setAttribute("data-testid", `project-home-part-${i}`);
        row.append(
          el("span", "bp-home-row-main", p.name || p.path),
          el("span", "bp-home-row-dim", p.kind),
          el("span", "bp-home-row-faint", p.path),
        );
        card.appendChild(row);
      });
      if (d.leadDays > 0) {
        const lead = el(
          "div",
          "bp-home-order-note",
          d.leadDays === 1
            ? t("web.home.lead.one", "Lead time: 1 day")
            : t("web.home.lead", "Lead time: {n} days", { n: d.leadDays }),
        );
        lead.setAttribute("data-testid", "project-home-lead");
        card.appendChild(lead);
      }
      for (const w of d.warnings) {
        const warn = el("div", "bp-home-order-note warn", w);
        warn.setAttribute("data-testid", "project-home-warning");
        card.appendChild(warn);
      }
      frag.appendChild(card);
    }

    // the boards: a row opens Board on the project's newest thread
    if (d.boards.length > 0) {
      frag.appendChild(this.section(t("gpui.projects.boards", "BOARDS ({n})", { n: d.boards.length })));
      d.boards.forEach((b, i) => {
        const row = this.fileRow(top, "board", `project-home-board-${i}`);
        row.appendChild(el("span", "bp-home-row-main", b));
        frag.appendChild(row);
      });
    }

    this.bodyEl.replaceChildren(frag);
  }

  /** A file's row: a button that opens its viewer on `thread`, or plain text when the project has no thread to open it on. */
  private fileRow(thread: string, kind: "board" | "mech", testId: string): HTMLElement {
    if (!thread) {
      const row = el("div", "bp-home-row");
      row.setAttribute("data-testid", testId);
      return row;
    }
    const row = el("button", "bp-home-row bp-home-open");
    row.type = "button";
    row.setAttribute("data-testid", testId);
    row.addEventListener("click", () => this.cb.onOpenViewer(thread, kind));
    return row;
  }
}
