// Add project dialog (docs/CLONE.md): "Open folder" (a folder browser over dirs.list, project.add) and "Clone from git" (clone.repos
// suggestions, clone.dest, clone.start, its progress from the hub's {"t":"d","k":"clone"} pushes, Cancel, Try again, GitHub's sign-in).
// The clone runs in the hub; every decision (where it goes, whether it may, its words) comes from the hub's answers.
import { goesAtOnce, isNewer, newJobId, type CloneJob } from "./clone-model";
import { t, trs } from "./i18n";
import type { DataPush, HubClient } from "./ws";

export interface CloneRepo {
  name: string;
  value: string;
  description?: string;
  private?: boolean;
  account?: string;
}

interface ReposReply {
  repos?: CloneRepo[];
  more?: number;
  state?: string;
  loading?: boolean;
  hint?: string;
}

export interface CloneDest {
  ok?: boolean;
  error?: string;
  same?: boolean;
  repo?: string;
  origin?: string;
  name?: string;
  dest?: string;
  destShown?: string;
  parent?: string;
  folder?: string;
  defaultParent?: string;
  defaultParentShown?: string;
}


export type { CloneJob } from "./clone-model";

/** What a clone was started with, for Try again (the same repository, folder, thread and first message). */
interface CloneSpec {
  repo: string;
  thread: boolean;
  text: string;
}

export interface SignIn {
  state?: string;
  code?: string;
  url?: string;
  error?: string;
}

interface DirsReply {
  path: string;
  parent?: string | null;
  dirs?: string[];
}

export interface AddProjectHost {
  hub: HubClient;
  /** The person added a folder: the app goes to it. */
  onProjectAdded: (project: { id: string; title?: string; [k: string]: unknown }) => void;
  /** A clone is done: the app selects its thread (else shows its project). */
  onCloneDone: (job: CloneJob) => void;
}

export interface OpenOptions {
  mode?: "folder" | "clone";
  repo?: string;
  text?: string;
}

type Mode = "folder" | "clone";

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls = "", text = ""): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text) e.textContent = text;
  return e;
}

function btn(label: string, cls = "", onClick?: () => void): HTMLButtonElement {
  const b = el("button", cls, label);
  b.type = "button";
  if (onClick) b.addEventListener("click", onClick);
  return b;
}

/** The last part of a path, as a folder's name is said ("/" stays "/"). */
export function folderName(path: string): string {
  const trimmed = path.replace(/[\\/]+$/, "");
  if (!trimmed) return path || "/";
  const parts = trimmed.split(/[\\/]/);
  return parts[parts.length - 1] || trimmed;
}

function joinPath(dir: string, name: string): string {
  const sep = dir.includes("\\") && !dir.includes("/") ? "\\" : "/";
  return dir.endsWith(sep) ? dir + name : dir + sep + name;
}

/** A folder browser over dirs.list. `action` names the button under it and runs with the folder on show. */
class FolderBrowser {
  public readonly root: HTMLElement;
  private pathInput: HTMLInputElement;
  private upBtn: HTMLButtonElement;
  private listEl: HTMLElement;
  private errEl: HTMLElement;
  private actBtn: HTMLButtonElement;
  private path = "";
  private parent: string | null = null;
  private gen = 0;
  private busy = false;

  constructor(
    private hub: HubClient,
    private actionLabel: (dir: string) => string,
    private action: (path: string) => Promise<void>,
    extra: HTMLElement[] = [],
  ) {
    this.root = el("div", "bp-fb");
    this.root.setAttribute("data-testid", "folder-browser");
    const head = el("div", "bp-fb-head");
    this.upBtn = btn("↑", "bp-btn-ghost bp-fb-up", () => { if (this.parent) void this.go(this.parent); });
    this.upBtn.title = t("web.clone.up", "Up");
    this.upBtn.setAttribute("aria-label", t("web.clone.up", "Up"));
    this.pathInput = el("input", "bp-input bp-fb-path");
    this.pathInput.spellcheck = false;
    this.pathInput.setAttribute("data-testid", "folder-path");
    this.pathInput.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        const v = this.pathInput.value.trim();
        if (v) void this.go(v);
      }
    });
    head.append(this.upBtn, this.pathInput);
    this.listEl = el("div", "bp-fb-list");
    this.errEl = el("div", "bp-ap-err bp-hidden");
    const foot = el("div", "bp-ap-foot");
    this.actBtn = btn("", "primary", () => { void this.run(); });
    this.actBtn.setAttribute("data-testid", "folder-act");
    foot.append(...extra, this.actBtn);
    this.root.append(head, this.listEl, this.errEl, foot);
  }

  public currentPath(): string { return this.path; }

  public async go(path: string): Promise<void> {
    const gen = ++this.gen;
    this.setError("");
    this.listEl.replaceChildren(el("div", "bp-fb-empty", t("web.clone.loading", "Loading…")));
    try {
      const r = await this.hub.rpc<DirsReply>("dirs.list", { path });
      if (gen !== this.gen) return;
      this.path = r.path;
      this.parent = r.parent ?? null;
      this.render(r.dirs ?? []);
    } catch (e) {
      if (gen !== this.gen) return;
      this.setError(trs(e instanceof Error ? e.message : String(e)));
      this.render(null);
    }
  }

  public refreshLocale(): void {
    this.upBtn.title = t("web.clone.up", "Up");
    this.updateAct();
  }

  private render(dirs: string[] | null): void {
    this.pathInput.value = this.path;
    this.upBtn.disabled = !this.parent;
    this.updateAct();
    if (dirs === null) {
      this.listEl.replaceChildren();
      return;
    }
    if (dirs.length === 0) {
      this.listEl.replaceChildren(el("div", "bp-fb-empty", t("web.clone.no.folders", "No folders here")));
      return;
    }
    const frag = document.createDocumentFragment();
    for (const name of dirs) {
      const row = btn("", "bp-fb-row", () => { void this.go(joinPath(this.path, name)); });
      row.append(el("span", "bp-fb-icon", "▸"), el("span", "bp-fb-name", name));
      frag.appendChild(row);
    }
    this.listEl.replaceChildren(frag);
    this.listEl.scrollTop = 0;
  }

  private updateAct(): void {
    this.actBtn.textContent = this.path ? this.actionLabel(folderName(this.path)) : this.actionLabel("…");
    this.actBtn.disabled = !this.path || this.busy;
  }

  private async run(): Promise<void> {
    if (!this.path || this.busy) return;
    this.busy = true;
    this.updateAct();
    this.setError("");
    try {
      await this.action(this.path);
    } catch (e) {
      this.setError(trs(e instanceof Error ? e.message : String(e)));
    } finally {
      this.busy = false;
      this.updateAct();
    }
  }

  private setError(msg: string): void {
    this.errEl.textContent = msg;
    this.errEl.classList.toggle("bp-hidden", !msg);
  }
}

export class AddProjectDialog {
  private backdrop: HTMLElement | null = null;
  private bodyEl: HTMLElement | null = null;
  private tabsEl: HTMLElement | null = null;
  private mode: Mode = "folder";

  // clone form
  private repoText = "";
  private repoValue = ""; // a picked suggestion's value; "" when the field was typed
  private firstText = "";
  private wantThread = true;
  private userParent = ""; // "" = the hub's default
  private nameText = "";
  private nameTouched = false;
  private dest: CloneDest | null = null;
  private destGen = 0;
  private destPending = false;
  private destTimer: number | null = null;
  private repos: ReposReply | null = null;
  private reposGen = 0;
  private reposTimer: number | null = null;
  private showSuggest = false;
  private suggestIdx = -1;
  private startErr = "";
  private starting = false;
  private picking = false; // the parent's folder browser on show

  // the clone in progress
  /** The job this dialog follows, known before clone.start answers (the client makes the id); "" when none. */
  private jobId = "";
  private job: CloneJob | null = null;
  private specs = new Map<string, CloneSpec>();
  private signin: SignIn | null = null;
  private copied = false;

  // stable parts of the clone form, rebuilt only on a full render
  private repoInput: HTMLInputElement | null = null;
  private suggestEl: HTMLElement | null = null;
  private destEl: HTMLElement | null = null;
  private signinEl: HTMLElement | null = null;
  private cloneBtn: HTMLButtonElement | null = null;
  private jobEl: HTMLElement | null = null;

  constructor(private container: HTMLElement, private host: AddProjectHost) {}

  public isOpen(): boolean { return this.backdrop !== null; }

  public open(opts: OpenOptions = {}): void {
    const canClone = this.host.hub.hasFeature("clone");
    // a clone still running, or done while the dialog was closed, is shown again (and asked again: pushes may have been missed)
    const keepJob = !opts.repo && Boolean(this.jobId) && (!this.job || this.job.state === "running" || this.job.state === "done");
    if (!this.backdrop) this.build();
    if (!keepJob) {
      this.jobId = "";
      this.job = null;
      this.resetForm();
    }
    if (opts.repo !== undefined || opts.text !== undefined) {
      this.repoText = (opts.repo ?? "").trim();
      this.repoValue = "";
      this.firstText = (opts.text ?? "").trim();
    }
    this.mode = canClone && (opts.mode === "clone" || keepJob) ? "clone" : "folder";
    this.render();
    if (keepJob) {
      if (this.job) this.maybeGo(this.job);
      this.reconcile();
    } else if (this.mode === "clone" && this.repoText) this.askDest();
  }

  /** Ask the hub how the followed clone stands (on reconnect, and when the dialog is shown again). */
  public reconcile(): void {
    const id = this.jobId;
    if (!id) return;
    void this.host.hub.rpc<CloneJob>("clone.status", { id })
      .then((j) => this.apply(j))
      .catch(() => { /* gone (a hub restart) or offline: what is held stays */ });
  }

  public close(): void {
    if (this.reposTimer !== null) window.clearTimeout(this.reposTimer);
    if (this.destTimer !== null) window.clearTimeout(this.destTimer);
    this.reposTimer = null;
    this.destTimer = null;
    this.backdrop?.remove();
    this.backdrop = null;
    this.bodyEl = null;
    this.tabsEl = null;
  }

  /** Escape: back out of the parent browser first, else close. */
  public escape(): void {
    if (this.picking) {
      this.picking = false;
      this.render();
      return;
    }
    if (this.showSuggest) {
      this.showSuggest = false;
      this.renderSuggest();
      return;
    }
    this.close();
  }

  public refreshLocale(): void {
    if (this.backdrop) this.render();
  }

  public handleFrame(p: DataPush): void {
    if (p.k === "clone") {
      const job = p.clone as CloneJob | undefined;
      if (job) this.apply(job);
    } else if (p.k === "clone.repos") {
      if (this.isOpen() && this.mode === "clone" && !this.jobId) this.askRepos(0);
    } else if (p.k === "clone.signin") {
      if (!this.signin) return; // only a sign-in asked for here is shown
      const prev = this.signin.state;
      this.signin = (p.signin as SignIn | undefined) ?? null;
      if (this.signin?.state === "cancelled") this.signin = null;
      if (prev !== "done" && this.signin?.state === "done") this.askRepos(0);
      this.renderSignin();
    }
  }

  // ---- building ----

  private build(): void {
    const backdrop = el("div", "bp-modal-backdrop");
    backdrop.setAttribute("data-testid", "add-project");
    const modal = el("div", "bp-modal bp-ap");
    const head = el("div", "bp-modal-head");
    const h2 = el("h2", "", t("sidebar.add.project", "Add project"));
    const closeBtn = btn("×", "icon", () => this.close());
    closeBtn.title = t("common.close", "Close");
    closeBtn.setAttribute("aria-label", t("common.close", "Close"));
    head.append(h2, closeBtn);
    this.tabsEl = el("div", "bp-ap-tabs");
    this.bodyEl = el("div", "bp-modal-body bp-ap-body");
    modal.append(head, this.tabsEl, this.bodyEl);
    backdrop.appendChild(modal);
    backdrop.addEventListener("mousedown", (e) => { if (e.target === backdrop) this.close(); });
    this.container.appendChild(backdrop);
    this.backdrop = backdrop;
  }

  private resetForm(): void {
    this.repoText = "";
    this.repoValue = "";
    this.firstText = "";
    this.wantThread = true;
    this.userParent = "";
    this.nameText = "";
    this.nameTouched = false;
    this.dest = null;
    this.destPending = false;
    this.repos = null;
    this.showSuggest = false;
    this.suggestIdx = -1;
    this.startErr = "";
    this.starting = false;
    this.picking = false;
    this.signin = null;
    this.copied = false;
  }

  private render(): void {
    if (!this.bodyEl || !this.tabsEl || !this.backdrop) return;
    const h2 = this.backdrop.querySelector(".bp-modal-head h2");
    if (h2) h2.textContent = t("sidebar.add.project", "Add project");
    this.renderTabs();
    this.repoInput = null;
    this.suggestEl = null;
    this.destEl = null;
    this.signinEl = null;
    this.cloneBtn = null;
    this.jobEl = null;
    if (this.mode === "folder") this.renderFolder();
    else if (this.jobId && this.job) this.renderJobView();
    else if (this.picking) this.renderPickParent();
    else this.renderCloneForm();
  }

  private renderTabs(): void {
    if (!this.tabsEl) return;
    if (!this.host.hub.hasFeature("clone")) {
      this.tabsEl.classList.add("bp-hidden");
      return;
    }
    this.tabsEl.classList.remove("bp-hidden");
    const mk = (mode: Mode, label: string): HTMLButtonElement => {
      const b = btn(label, "bp-ap-tab" + (this.mode === mode ? " on" : ""), () => {
        if (this.mode === mode) return;
        this.mode = mode;
        this.render();
        if (mode === "clone" && !this.jobId && this.repos === null) this.askRepos(0);
      });
      b.setAttribute("data-testid", `ap-tab-${mode}`);
      b.setAttribute("aria-pressed", String(this.mode === mode));
      return b;
    };
    this.tabsEl.replaceChildren(
      mk("folder", t("gpui.e.open.folder", "Open folder")),
      mk("clone", t("web.clone.tab", "Clone from git")),
    );
  }

  // ---- Open folder ----

  private renderFolder(): void {
    const fb = new FolderBrowser(
      this.host.hub,
      (dir) => t("web.clone.add.dir", "Add {dir}", { dir }),
      async (path) => {
        const r = await this.host.hub.rpc<{ project?: { id: string; title?: string } }>("project.add", { path }, 30_000);
        this.close();
        if (r.project) this.host.onProjectAdded(r.project);
      },
    );
    this.bodyEl!.replaceChildren(fb.root);
    void fb.go("");
  }

  // ---- Clone from git: the form ----

  private renderCloneForm(): void {
    const body = this.bodyEl!;
    const form = el("div", "bp-ap-form");

    const repoWrap = el("div", "bp-ap-repo");
    const input = el("input", "bp-input");
    input.type = "text";
    input.spellcheck = false;
    input.autocomplete = "off";
    input.placeholder = t("web.clone.repo.placeholder", "https or ssh URL, or owner/repo");
    input.setAttribute("aria-label", t("web.clone.repo.placeholder", "https or ssh URL, or owner/repo"));
    input.setAttribute("data-testid", "clone-repo");
    input.value = this.repoText;
    input.addEventListener("input", () => {
      this.repoText = input.value;
      this.repoValue = "";
      this.showSuggest = true;
      this.suggestIdx = -1;
      this.startErr = "";
      if (!this.nameTouched) this.nameText = "";
      this.askRepos(200);
      this.askDest(250);
    });
    input.addEventListener("focus", () => {
      this.showSuggest = true;
      if (this.repos === null) this.askRepos(0);
      else this.renderSuggest();
    });
    input.addEventListener("blur", () => {
      // a click on a suggestion lands first (mousedown preventDefault keeps focus); a blur elsewhere hides them
      window.setTimeout(() => {
        if (document.activeElement !== input) {
          this.showSuggest = false;
          this.renderSuggest();
        }
      }, 120);
    });
    input.addEventListener("keydown", (e) => this.repoKey(e));
    this.repoInput = input;
    this.suggestEl = el("div", "bp-ap-suggest bp-hidden");
    this.suggestEl.setAttribute("data-testid", "clone-suggest");
    repoWrap.append(input, this.suggestEl);

    this.signinEl = el("div", "bp-ap-signin bp-hidden");
    this.signinEl.setAttribute("data-testid", "clone-signin");

    this.destEl = el("div", "bp-ap-dest");
    this.destEl.setAttribute("data-testid", "clone-dest");

    const threadRow = el("label", "bp-ap-check");
    const cb = el("input");
    cb.type = "checkbox";
    cb.checked = this.wantThread;
    cb.setAttribute("data-testid", "clone-thread");
    cb.addEventListener("change", () => { this.wantThread = cb.checked; });
    threadRow.append(cb, el("span", "", t("web.clone.start.thread", "Start a thread")));

    const rows: HTMLElement[] = [repoWrap, this.signinEl, this.destEl, threadRow];
    if (this.firstText) {
      rows.push(el("div", "bp-ap-first bp-dim", t("web.clone.first", "First message: {text}", { text: this.firstText })));
    }

    const errEl = el("div", "bp-ap-err" + (this.startErr ? "" : " bp-hidden"), this.startErr);
    errEl.setAttribute("data-testid", "clone-start-error");
    const foot = el("div", "bp-ap-foot");
    this.cloneBtn = btn(t("web.clone.go", "Clone"), "primary", () => { void this.startClone(); });
    this.cloneBtn.setAttribute("data-testid", "clone-go");
    foot.append(this.cloneBtn);
    rows.push(errEl, foot);

    form.append(...rows);
    body.replaceChildren(form);
    this.renderSuggest();
    this.renderSignin();
    this.renderDest();
    window.setTimeout(() => { if (this.repoInput === input && !this.repoText) input.focus(); }, 0);
  }

  private repoKey(e: KeyboardEvent): void {
    const list = this.repos?.repos ?? [];
    const open = this.showSuggest && list.length > 0;
    if (e.key === "ArrowDown" && open) {
      e.preventDefault();
      this.suggestIdx = Math.min(list.length - 1, this.suggestIdx + 1);
      this.renderSuggest();
    } else if (e.key === "ArrowUp" && open) {
      e.preventDefault();
      this.suggestIdx = Math.max(-1, this.suggestIdx - 1);
      this.renderSuggest();
    } else if (e.key === "Enter") {
      e.preventDefault();
      const pick = open && this.suggestIdx >= 0 ? list[this.suggestIdx] : undefined;
      if (pick) this.pickRepo(pick);
      else if (this.canStart()) void this.startClone();
    }
  }

  private pickRepo(r: CloneRepo): void {
    this.repoText = r.name;
    this.repoValue = r.value;
    this.showSuggest = false;
    this.suggestIdx = -1;
    this.startErr = "";
    if (!this.nameTouched) this.nameText = "";
    if (this.repoInput) this.repoInput.value = r.name;
    this.renderSuggest();
    this.askDest();
  }

  private repoArg(): string {
    return this.repoValue || this.repoText.trim();
  }

  private askRepos(delay: number): void {
    if (this.reposTimer !== null) window.clearTimeout(this.reposTimer);
    this.reposTimer = window.setTimeout(() => {
      this.reposTimer = null;
      const gen = ++this.reposGen;
      const q = this.repoValue ? "" : this.repoText.trim();
      void this.host.hub.rpc<ReposReply>("clone.repos", { q, limit: 12 }, 30_000)
        .then((r) => {
          if (gen !== this.reposGen) return;
          this.repos = r;
          this.renderSuggest();
          this.renderSignin();
        })
        .catch(() => {
          if (gen !== this.reposGen) return;
          this.repos = { repos: [], state: "error", hint: "Couldn't list your GitHub repositories." };
          this.renderSuggest();
        });
    }, delay);
  }

  private renderSuggest(): void {
    const box = this.suggestEl;
    if (!box) return;
    const r = this.repos;
    const list = r?.repos ?? [];
    const rows: HTMLElement[] = [];
    if (this.showSuggest && !this.repoValue) {
      list.forEach((repo, i) => {
        const row = btn("", "bp-ap-sug" + (i === this.suggestIdx ? " active" : ""));
        row.setAttribute("data-testid", "clone-suggestion");
        row.addEventListener("mousedown", (e) => e.preventDefault());
        row.addEventListener("click", () => this.pickRepo(repo));
        const top = el("div", "bp-ap-sug-top");
        top.append(el("span", "bp-ap-sug-name", repo.name));
        if (repo.private) top.append(el("span", "bp-ap-tag", t("web.clone.private", "private")));
        row.append(top);
        if (repo.description) row.append(el("div", "bp-ap-sug-desc", repo.description));
        rows.push(row);
      });
      if (list.length === 0 && r) {
        if (r.loading) rows.push(el("div", "bp-ap-sug-note", t("web.clone.loading", "Loading…")));
        else if (r.hint) {
          const note = el("div", "bp-ap-sug-note");
          note.append(el("span", "", trs(r.hint)));
          if (r.state === "signedout") {
            const s = btn(t("web.clone.signin", "Sign in to GitHub"), "bp-btn-line", () => { void this.startSignin(); });
            s.addEventListener("mousedown", (e) => e.preventDefault());
            s.setAttribute("data-testid", "clone-signin-btn");
            note.append(s);
          }
          rows.push(note);
        }
      }
    }
    box.replaceChildren(...rows);
    box.classList.toggle("bp-hidden", rows.length === 0);
  }

  // ---- the destination ----

  private askDest(delay = 0): void {
    if (this.destTimer !== null) window.clearTimeout(this.destTimer);
    this.destPending = true;
    this.updateCloneBtn();
    this.destTimer = window.setTimeout(() => {
      this.destTimer = null;
      const gen = ++this.destGen;
      const repo = this.repoArg();
      if (!repo) {
        this.dest = null;
        this.destPending = false;
        this.renderDest();
        return;
      }
      const params: Record<string, unknown> = { repo };
      if (this.userParent) params.parent = this.userParent;
      if (this.nameTouched && this.nameText.trim()) params.name = this.nameText.trim();
      else if (this.userParent) params.name = "";
      void this.host.hub.rpc<CloneDest>("clone.dest", params)
        .then((d) => {
          if (gen !== this.destGen) return;
          this.dest = d;
          this.destPending = false;
          if (!this.nameTouched && d.folder) this.nameText = d.folder;
          this.renderDest();
        })
        .catch((e) => {
          if (gen !== this.destGen) return;
          this.dest = { ok: false, error: e instanceof Error ? e.message : String(e) };
          this.destPending = false;
          this.renderDest();
        });
    }, delay);
  }

  private renderDest(): void {
    const box = this.destEl;
    if (!box) return;
    const d = this.dest;
    const rows: HTMLElement[] = [];
    // a refusal of the address itself (nothing parsed): its words, no destination
    if (d && !d.repo) {
      if (d.error && this.repoText.trim()) rows.push(this.errLine(trs(d.error)));
    } else if (d) {
      const where = el("div", "bp-ap-where");
      const shown = el("span", "bp-ap-dest-path", d.destShown || d.dest || d.defaultParentShown || "");
      shown.setAttribute("data-testid", "clone-dest-shown");
      const change = btn(t("web.clone.change.folder", "Change folder"), "bp-btn-line", () => {
        this.picking = true;
        this.render();
      });
      change.setAttribute("data-testid", "clone-change-folder");
      where.append(shown, change);
      rows.push(where);

      const nameRow = el("label", "bp-ap-name");
      nameRow.append(el("span", "bp-ap-lbl", t("web.clone.folder.name", "Folder name")));
      const nameIn = el("input", "bp-input");
      nameIn.type = "text";
      nameIn.spellcheck = false;
      nameIn.value = this.nameText;
      nameIn.setAttribute("data-testid", "clone-name");
      nameIn.addEventListener("input", () => {
        this.nameText = nameIn.value;
        this.nameTouched = nameIn.value.trim() !== "";
        this.startErr = "";
        this.askDest(250);
      });
      nameIn.addEventListener("keydown", (e) => {
        if (e.key === "Enter" && this.canStart()) { e.preventDefault(); void this.startClone(); }
      });
      nameRow.append(nameIn);
      rows.push(nameRow);

      if (!d.ok && d.error) rows.push(this.errLine(trs(d.error)));
      else if (d.same) rows.push(el("div", "bp-ap-note bp-dim", t("web.clone.same", "Already cloned there: it is added as it is.")));
    }
    // keep the name field's focus and caret across a re-ask
    const active = document.activeElement as HTMLInputElement | null;
    const hadName = active && box.contains(active) && active.getAttribute("data-testid") === "clone-name";
    const caret = hadName ? active!.selectionStart : null;
    box.replaceChildren(...rows);
    box.classList.toggle("bp-hidden", rows.length === 0);
    if (hadName) {
      const n = box.querySelector<HTMLInputElement>('[data-testid="clone-name"]');
      if (n) { n.focus(); if (caret !== null) n.setSelectionRange(caret, caret); }
    }
    this.updateCloneBtn();
  }

  private errLine(text: string): HTMLElement {
    const e = el("div", "bp-ap-err", text);
    e.setAttribute("data-testid", "clone-dest-error");
    return e;
  }

  private canStart(): boolean {
    return Boolean(this.dest?.ok && this.dest.repo && !this.destPending && !this.starting && this.repoArg());
  }

  private updateCloneBtn(): void {
    if (this.cloneBtn) this.cloneBtn.disabled = !this.canStart();
  }

  private renderPickParent(): void {
    const start = this.dest?.parent || this.userParent || this.dest?.defaultParent || "";
    const back = btn(t("web.clone.back", "Back"), "bp-btn-line", () => {
      this.picking = false;
      this.render();
    });
    const fb = new FolderBrowser(
      this.host.hub,
      (dir) => t("web.clone.use.dir", "Use {dir}", { dir }),
      async (path) => {
        this.userParent = path;
        this.picking = false;
        this.render();
        this.askDest();
      },
      [back],
    );
    this.bodyEl!.replaceChildren(fb.root);
    void fb.go(start);
  }

  private async startClone(): Promise<void> {
    if (!this.canStart() || !this.dest) return;
    this.starting = true;
    this.startErr = "";
    this.updateCloneBtn();
    const spec: CloneSpec = { repo: this.repoArg(), thread: this.wantThread, text: this.firstText };
    const id = newJobId();
    // followed from now: a push for it may come before the reply
    this.jobId = id;
    this.job = null;
    this.specs.set(id, spec);
    try {
      const job = await this.host.hub.rpc<CloneJob>("clone.start", {
        id,
        repo: spec.repo,
        parent: this.dest.parent ?? "",
        name: this.dest.folder ?? this.nameText.trim(),
        thread: spec.thread,
        text: spec.text,
      });
      this.starting = false;
      this.apply(job);
    } catch (e) {
      this.starting = false;
      const msg = trs(e instanceof Error ? e.message : String(e));
      // a refused start that was given an id leaves a failed job with that id: shown as any failed job
      if (this.jobId === id && !this.job) {
        const j = await this.host.hub.rpc<CloneJob>("clone.status", { id }).catch(() => null);
        if (j && j.id === id) { this.apply(j); return; }
      }
      if (this.jobId === id && !this.job) {
        this.jobId = "";
        this.startErr = msg;
        if (this.mode === "clone") this.render();
      }
    }
  }

  /** Try again: a new job with the same repository, folder, thread and first message, followed under a new id of ours. */
  private async retry(j: CloneJob, button: HTMLButtonElement): Promise<void> {
    const spec = this.specs.get(j.id) ?? { repo: j.origin || j.repo || "", thread: true, text: "" };
    const id = newJobId();
    const prev = this.jobId;
    this.jobId = id;
    this.specs.set(id, spec);
    button.disabled = true;
    try {
      const nj = await this.host.hub.rpc<CloneJob>("clone.start", { id, repo: spec.repo, dest: j.dest ?? "", thread: spec.thread, text: spec.text });
      this.apply(nj);
    } catch (e) {
      const msg = trs(e instanceof Error ? e.message : String(e));
      if (this.jobId !== id) return;
      if (!this.job || this.job.id !== id) {
        const got = await this.host.hub.rpc<CloneJob>("clone.status", { id }).catch(() => null);
        if (got && got.id === id) { this.apply(got); return; }
      }
      if (this.jobId === id && (!this.job || this.job.id !== id)) {
        this.jobId = prev;
        button.disabled = false;
        this.showJobError(msg);
      }
    }
  }

  /** A push or reply for the followed job: taken only when it is not older than what is held (clone-model.ts `isNewer`). */
  private apply(incoming: CloneJob): void {
    if (!incoming || incoming.id !== this.jobId) return;
    if (!isNewer(this.job, incoming)) return;
    this.job = incoming;
    this.copied = false;
    if (this.isOpen() && this.mode === "clone") {
      if (this.jobEl && this.jobEl.isConnected) this.renderJob();
      else this.render();
    }
    this.maybeGo(incoming);
  }

  /** Done with nothing to read, while the dialog is on show: go to it. */
  private maybeGo(j: CloneJob): void {
    if (this.isOpen() && j.id === this.jobId && goesAtOnce(j)) this.goTo(j);
  }

  // ---- the clone's progress ----

  private renderJobView(): void {
    this.jobEl = el("div", "bp-ap-job");
    this.jobEl.setAttribute("data-testid", "clone-job");
    this.signinEl = el("div", "bp-ap-signin bp-hidden");
    this.signinEl.setAttribute("data-testid", "clone-signin");
    this.bodyEl!.replaceChildren(this.jobEl, this.signinEl);
    this.renderJob();
    this.renderSignin();
  }

  private renderJob(): void {
    const box = this.jobEl;
    const j = this.job;
    if (!box || !j) return;
    const dest = j.destShown || j.dest || "";
    const repo = j.repo || "";
    const title = j.state === "done"
      ? t("web.clone.cloned", "Cloned {repo} into {dest}", { repo, dest })
      : j.state === "cancelled"
        ? t("web.clone.stopped", "Stopped cloning {repo}", { repo })
        : t("web.clone.cloning", "Cloning {repo} into {dest}", { repo, dest });
    const rows: HTMLElement[] = [el("div", "bp-ap-job-title", title)];

    if (j.state === "running") {
      const phase = el("div", "bp-ap-phase", trs(j.phase || ""));
      phase.setAttribute("data-testid", "clone-phase");
      const bar = el("div", "bp-ap-bar" + (typeof j.pct === "number" ? "" : " indeterminate"));
      const fill = el("div", "bp-ap-bar-fill");
      if (typeof j.pct === "number") fill.style.width = `${Math.max(0, Math.min(100, j.pct))}%`;
      bar.appendChild(fill);
      bar.setAttribute("role", "progressbar");
      if (typeof j.pct === "number") bar.setAttribute("aria-valuenow", String(j.pct));
      rows.push(phase, bar);
      if (j.detail) rows.push(el("div", "bp-ap-detail bp-dim", j.detail));
      const foot = el("div", "bp-ap-foot");
      const cancel = btn(t("common.cancel", "Cancel"), "bp-btn-line", () => {
        void this.host.hub.rpc<CloneJob>("clone.cancel", { id: j.id })
          .then((nj) => this.apply(nj))
          .catch(() => {});
      });
      cancel.setAttribute("data-testid", "clone-cancel");
      foot.append(cancel);
      rows.push(foot);
    } else if (j.state === "failed" || j.state === "cancelled") {
      if (j.state === "failed" && j.error) {
        const err = el("div", "bp-ap-err", trs(j.error));
        err.setAttribute("data-testid", "clone-error");
        rows.push(err);
      }
      const foot = el("div", "bp-ap-foot");
      const back = btn(t("web.clone.back", "Back"), "bp-btn-line", () => {
        this.jobId = "";
        this.job = null;
        this.signin = null;
        this.render();
        this.askDest();
      });
      foot.append(back);
      if (j.signin) {
        const s = btn(t("web.clone.signin", "Sign in to GitHub"), "bp-btn-line", () => { void this.startSignin(); });
        s.setAttribute("data-testid", "clone-signin-btn");
        foot.append(s);
      }
      const retry = btn(t("web.clone.retry", "Try again"), "primary", () => { void this.retry(j, retry); });
      retry.setAttribute("data-testid", "clone-retry");
      foot.append(retry);
      rows.push(foot);
    } else if (j.state === "done") {
      const notes = j.notes ?? [];
      if (notes.length > 0) {
        const ul = el("ul", "bp-ap-notes");
        ul.setAttribute("data-testid", "clone-notes");
        for (const n of notes) ul.appendChild(el("li", "", trs(n)));
        rows.push(ul);
      }
      const foot = el("div", "bp-ap-foot");
      const label = j.thread
        ? t("web.clone.open.thread", "Open the thread")
        : t("web.clone.open.project", "Open the project");
      const go = btn(label, "primary", () => this.goTo(j));
      go.setAttribute("data-testid", "clone-open");
      foot.append(go);
      rows.push(foot);
    }
    box.replaceChildren(...rows);
  }

  private showJobError(msg: string): void {
    if (!this.jobEl) return;
    this.jobEl.querySelector(".bp-ap-job-err")?.remove();
    const e = el("div", "bp-ap-err bp-ap-job-err", msg);
    this.jobEl.appendChild(e);
  }

  private goTo(j: CloneJob): void {
    this.jobId = "";
    this.job = null;
    this.close();
    this.host.onCloneDone(j);
  }

  // ---- GitHub's sign-in ----

  private async startSignin(): Promise<void> {
    this.signin = { state: "starting" };
    this.copied = false;
    this.renderSignin();
    try {
      const s = await this.host.hub.rpc<SignIn>("clone.signin", { restart: true }, 30_000);
      // a push may have moved it on meanwhile; keep the later state
      if (!this.signin || this.signin.state === "starting") this.signin = s;
    } catch (e) {
      this.signin = { state: "failed", error: e instanceof Error ? e.message : String(e) };
    }
    if (this.signin?.state === "done") this.askRepos(0);
    this.renderSignin();
  }

  private renderSignin(): void {
    const box = this.signinEl;
    if (!box) return;
    const s = this.signin;
    const rows: HTMLElement[] = [];
    if (s && s.state) {
      if (s.state === "starting" || (s.state === "waiting" && !s.code)) {
        rows.push(el("div", "bp-dim", t("web.clone.signin.wait", "Starting GitHub sign-in…")));
      } else if (s.state === "waiting") {
        rows.push(el("div", "", t("web.clone.signin.enter", "Enter this code on GitHub:")));
        const codeRow = el("div", "bp-ap-code-row");
        const code = el("span", "bp-ap-code", s.code || "");
        code.setAttribute("data-testid", "clone-signin-code");
        const copy = btn(this.copied ? t("web.clone.copied", "Copied") : t("common.copy", "Copy"), "bp-btn-line", () => {
          void navigator.clipboard?.writeText(s.code || "").then(() => {
            this.copied = true;
            this.renderSignin();
          }).catch(() => {});
        });
        codeRow.append(code, copy);
        rows.push(codeRow);
        if (s.url) {
          const a = el("a", "bp-ap-link", s.url);
          a.href = s.url;
          a.target = "_blank";
          a.rel = "noopener noreferrer";
          a.setAttribute("data-testid", "clone-signin-url");
          rows.push(a);
        }
      } else if (s.state === "done") {
        rows.push(el("div", "bp-ok", t("web.clone.signin.done", "Signed in to GitHub")));
      } else if (s.state === "failed") {
        rows.push(el("div", "bp-ap-err", trs(s.error || "")));
      }
    }
    box.replaceChildren(...rows);
    box.classList.toggle("bp-hidden", rows.length === 0);
  }
}
