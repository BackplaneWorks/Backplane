import { t, trs } from "./i18n";

interface Instruction {
  path: string;
  bytes: number;
  estimated_tokens: number;
  status: string;
  preview: string;
  note: string;
}

interface Skill {
  scope: string;
  status: string;
  name: string;
  description: string;
  path: string;
  source: string;
  provider: string;
  model: string;
}

interface Server {
  name: string;
  command: string;
  args: string[];
  url: string;
  cwd: string;
  status: string;
}

interface Config {
  path: string;
  sha256: string;
  state: string;
  error: string;
  servers: Server[];
}

interface Context {
  cwd: string;
  instructions: Instruction[];
  skills: Skill[];
  mcp: Config[];
  prompt: string;
  sections: { base: string; instructions: string; skills: string };
}

interface Decision { action: string; path: string; sha256: string }

function el<K extends keyof HTMLElementTagNameMap>(tag: K, css?: string, text?: string): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (css) node.className = css;
  if (text) node.textContent = text;
  return node;
}

function button(label: string, click: () => void, primary = false): HTMLButtonElement {
  const node = el("button", primary ? "primary" : "bp-btn-ghost", label);
  node.type = "button";
  node.addEventListener("click", click);
  return node;
}

function code(text: string): HTMLElement {
  return el("pre", "bp-context-code", text);
}

function badge(status: string): HTMLElement {
  const tone = status === "Native" || status === "Enabled" ? "active" : status === "Untrusted" || status === "Invalid" ? "warn" : "";
  return el("span", `bp-context-badge ${tone}`, trs(status));
}

function heading(title: string, status: string): HTMLElement {
  const node = el("div", "bp-context-row");
  node.append(el("strong", undefined, title), badge(status));
  return node;
}

export class ThreadContextDialog {
  private dialog = el("dialog", "bp-context");
  private title = el("h2");
  private cwd = el("div", "bp-context-path");
  private tabs = el("div", "bp-context-tabs");
  private body = el("div", "bp-context-body");
  private notice = el("p", "bp-context-notice");
  private current?: Context;
  private thread = "";
  private requestId = 0;
  private tab = 0;
  private busy = false;

  constructor(root: HTMLElement, private fetchContext: (thread: string, decision?: Decision) => Promise<Context>) {
    const head = el("div", "bp-context-head");
    const caption = el("div");
    caption.append(this.title, this.cwd);
    this.title.id = "bp-context-title";
    this.dialog.setAttribute("aria-labelledby", this.title.id);
    this.notice.setAttribute("role", "status");
    head.append(caption, button(t("common.refresh", "Refresh"), () => { void this.load(); }), button(t("common.close", "Close"), () => this.dialog.close()));
    this.dialog.append(head, this.tabs, this.notice, this.body);
    this.dialog.addEventListener("close", () => { this.requestId++; });
    root.append(this.dialog);
  }

  open(thread: string): void {
    this.thread = thread;
    this.tab = 0;
    this.current = undefined;
    this.busy = false;
    this.title.textContent = t("thread.context.title", "Context & Prompt");
    this.cwd.textContent = "";
    if (!this.dialog.open) this.dialog.showModal();
    this.render();
    void this.load();
  }

  private async load(decision?: Decision): Promise<void> {
    if (this.busy) return;
    const id = ++this.requestId;
    this.busy = true;
    this.notice.textContent = t("thread.context.reading", "Reading thread context…");
    this.render();
    try {
      const snapshot = await this.fetchContext(this.thread, decision);
      if (id !== this.requestId) return;
      this.current = snapshot;
      this.cwd.textContent = snapshot.cwd;
      this.notice.textContent = "";
    } catch (error) {
      if (id === this.requestId) this.notice.textContent = String(error);
    } finally {
      if (id === this.requestId) {
        this.busy = false;
        this.render();
      }
    }
  }

  private render(): void {
    const labels = [t("thread.context.instructions", "Instructions"), t("thread.context.system", "System Prompt"),
      t("thread.context.skills", "Skills"), t("thread.context.mcp", "Project MCPs")];
    this.tabs.replaceChildren(...labels.map((label, index) => {
      const tab = button(label, () => {
        this.tab = index;
        this.render();
        this.body.scrollTop = 0;
        (this.tabs.children[index] as HTMLElement).focus();
      });
      tab.setAttribute("aria-pressed", String(index === this.tab));
      return tab;
    }));
    this.body.replaceChildren();
    const data = this.current;
    if (!data) return;
    switch (this.tab) {
      case 0: this.instructions(data.instructions); break;
      case 1: this.prompt(data); break;
      case 2: this.skills(data.skills); break;
      case 3: this.mcps(data.mcp); break;
    }
  }

  private instructions(files: Instruction[]): void {
    if (!files.length) this.body.append(el("p", "bp-dim", t("thread.context.none.instructions", "No project instructions found.")));
    for (const file of files) {
      const card = el("details", "bp-context-card");
      const summary = el("summary");
      summary.append(heading(file.path, file.status), el("small", "bp-dim", t("thread.context.size", "{bytes} B · ≈{tokens} tokens",
        { bytes: file.bytes, tokens: file.estimated_tokens })));
      card.append(summary, code(file.preview));
      if (file.note) card.append(el("small", "bp-dim", file.note));
      this.body.append(card);
    }
  }

  private prompt(data: Context): void {
    const header = el("div", "bp-context-row");
    const copy = button(t("thread.context.copy", "Copy prompt"), () => {
      void navigator.clipboard.writeText(data.prompt).then(() => {
        this.notice.textContent = t("thread.context.copied", "Prompt copied.");
      }).catch(error => { this.notice.textContent = String(error); });
    });
    header.append(el("p", "bp-dim", t("thread.context.native.note", "Backplane’s launch prompt. Native files are loaded by the provider.")), copy);
    this.body.append(header);
    for (const [label, value] of [
      [t("thread.context.base", "Backplane"), data.sections.base],
      [t("thread.context.injected.label", "Injected project instructions"), data.sections.instructions],
      [t("thread.context.index", "Available skills index"), data.sections.skills],
    ]) {
      if (value) this.body.append(el("h3", undefined, label), code(value));
    }
  }

  private skills(skills: Skill[]): void {
    for (const scope of ["Project", "User", "Bundled"]) {
      const group = skills.filter(skill => skill.scope === scope);
      if (!group.length) continue;
      this.body.append(el("h3", undefined, trs(scope)));
      for (const skill of group) {
        const card = el("article", "bp-context-card");
        card.append(heading(skill.name, skill.status), el("p", undefined, skill.description),
          el("small", "bp-context-path", [skill.source, skill.provider, skill.model].filter(Boolean).join(" · ")), el("small", "bp-context-path", skill.path));
        this.body.append(card);
      }
    }
  }

  private mcps(configs: Config[]): void {
    if (!configs.length) this.body.append(el("p", "bp-dim", t("thread.context.none.mcp", "No project MCP configurations found.")));
    for (const config of configs) {
      const card = el("article", "bp-context-card");
      card.append(heading(config.path, config.state));
      for (const server of config.servers) {
        const endpoint = server.url || [server.command, ...server.args].map(word => JSON.stringify(word)).join(" ");
        card.append(heading(server.name, server.status), code(endpoint));
        if (server.cwd) card.append(el("small", "bp-context-path", server.cwd));
      }
      if (config.error) card.append(el("p", "bp-warn", config.error));
      const action = config.state === "Untrusted" ? "trust" : config.state === "Enabled" ? "disable" : config.state === "Disabled" ? "enable" : "";
      if (action) {
        const label = action === "trust" ? t("thread.context.trust", "Trust and enable") : action === "disable"
          ? t("thread.context.disable", "Disable") : t("thread.context.enable", "Enable");
        const apply = button(label, () => { void this.load({ action, path: config.path, sha256: config.sha256 }); }, action === "trust");
        apply.disabled = this.busy;
        const buttons = el("div", "bp-context-actions");
        buttons.append(apply);
        card.append(buttons);
      }
      this.body.append(card);
    }
  }
}
