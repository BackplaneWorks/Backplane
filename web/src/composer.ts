// Composer matching GPUI native-chat.png:
// Tall bordered input box ("Ask anything: @bot, >thread, %project, #room, $skill, /btw"),
// slash-command popup (hello.commands), mentions popup, and bottom toolbar row
// with model chip on the left and accent-blue Send / red Stop on the right.
import { t } from "./i18n";

export interface SlashCommand {
  word: string;
  label: string;
  desc: string;
}

export interface MentionItem {
  sigil: string;
  id: string;
  label: string;
  detail?: string;
  word: string;
}

export interface ComposerCallbacks {
  onSend: (text: string) => void;
  onRunCommand: (text: string) => void;
  onInterrupt: () => void;
  onQueryMentions: (q: string, kind?: string) => Promise<MentionItem[]>;
}

export class ComposerView {
  private popupEl: HTMLElement;
  private textarea: HTMLTextAreaElement;
  private modelChipEl: HTMLElement;
  private memoryEl: HTMLElement;
  private memoryFill: HTMLElement;
  private memoryText: HTMLElement;
  private sendBtn: HTMLButtonElement;
  private stopBtn: HTMLButtonElement;
  private errorEl: HTMLElement;
  private cb: ComposerCallbacks;

  private commands: SlashCommand[] = [];
  private popupMode: "none" | "slash" | "mention" = "none";
  private filteredSlash: SlashCommand[] = [];
  private mentionItems: MentionItem[] = [];
  private popupIdx = 0;
  private enabled = false;

  constructor(container: HTMLElement, cb: ComposerCallbacks) {
    this.cb = cb;

    const wrap = document.createElement("div");
    wrap.className = "bp-composer";
    wrap.setAttribute("data-testid", "composer");

    const inner = document.createElement("div");
    inner.className = "bp-composer-inner";

    this.popupEl = document.createElement("div");
    this.popupEl.className = "bp-composer-popup bp-hidden";
    this.popupEl.setAttribute("data-testid", "slash-menu");

    this.errorEl = document.createElement("div");
    this.errorEl.className = "bp-composer-err bp-hidden";

    this.textarea = document.createElement("textarea");
    this.textarea.className = "bp-composer-input";
    this.textarea.rows = 2;
    this.textarea.placeholder = t("composer.placeholder", "Ask anything: @bot, >thread, %project, #room, $skill, /btw");
    this.textarea.setAttribute("data-testid", "composer-input");

    // Toolbar row under the input box (matches GPUI native-chat.png)
    const bar = document.createElement("div");
    bar.className = "bp-composer-bar";

    this.modelChipEl = document.createElement("div");
    this.modelChipEl.className = "bp-composer-model";
    this.modelChipEl.textContent = "default";

    // infinite context: the memory's bar and the hub's words (bots/recall/progress.rs), shown only for a thread that has it
    this.memoryEl = document.createElement("div");
    this.memoryEl.className = "bp-composer-memory bp-hidden";
    this.memoryEl.setAttribute("data-testid", "composer-memory");
    const meter = document.createElement("div");
    meter.className = "bp-memory-meter";
    this.memoryFill = document.createElement("div");
    this.memoryFill.className = "bp-memory-fill";
    meter.appendChild(this.memoryFill);
    this.memoryText = document.createElement("span");
    this.memoryEl.append(meter, this.memoryText);

    const btns = document.createElement("div");
    btns.className = "bp-composer-btns";

    this.stopBtn = document.createElement("button");
    this.stopBtn.type = "button";
    this.stopBtn.className = "primary stop bp-hidden";
    this.stopBtn.textContent = t("composer.stop", "Stop");
    this.stopBtn.setAttribute("data-testid", "composer-stop");
    this.stopBtn.addEventListener("click", () => this.cb.onInterrupt());

    this.sendBtn = document.createElement("button");
    this.sendBtn.type = "button";
    this.sendBtn.className = "primary bp-send-btn";
    this.sendBtn.textContent = t("composer.send", "Send");
    this.sendBtn.setAttribute("data-testid", "composer-send");
    this.sendBtn.addEventListener("click", () => this.submit());

    btns.append(this.stopBtn, this.sendBtn);
    bar.append(this.modelChipEl, this.memoryEl, btns);
    inner.append(this.popupEl, this.errorEl, this.textarea, bar);
    wrap.appendChild(inner);
    container.appendChild(wrap);

    this.textarea.addEventListener("input", () => {
      this.autoGrow();
      this.clearError();
      this.updatePopups();
    });

    this.textarea.addEventListener("keydown", (e) => {
      if (this.popupMode !== "none") {
        const count = this.popupMode === "slash" ? this.filteredSlash.length : this.mentionItems.length;
        if (e.key === "ArrowDown") {
          e.preventDefault();
          this.popupIdx = (this.popupIdx + 1) % Math.max(1, count);
          this.renderPopup();
          return;
        }
        if (e.key === "ArrowUp") {
          e.preventDefault();
          this.popupIdx = (this.popupIdx - 1 + Math.max(1, count)) % Math.max(1, count);
          this.renderPopup();
          return;
        }
        if (e.key === "Tab" || (e.key === "Enter" && !e.shiftKey && this.shouldCompleteOnEnter())) {
          e.preventDefault();
          this.applyPopupChoice(this.popupIdx);
          return;
        }
        if (e.key === "Escape") {
          e.preventDefault();
          this.closePopup();
          return;
        }
      }
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        this.submit();
      }
    });
  }

  public setCommands(cmds: SlashCommand[]): void {
    this.commands = cmds;
  }

  public setModelChip(chip: string): void {
    this.modelChipEl.textContent = chip || "default";
  }

  /** The thread's memory progress as the hub words it (`thread.memory`), or null for a thread without infinite context. */
  public setMemory(m: MemoryProgress | null | undefined): void {
    this.memoryEl.classList.toggle("bp-hidden", !m);
    if (!m) return;
    const total = m.total ?? 0;
    const fill = total > 0 ? Math.min(1, (m.done ?? 0) / total) : 1;
    this.memoryFill.style.width = `${Math.round(fill * 100)}%`;
    this.memoryFill.dataset.state = m.state ?? "";
    this.memoryText.textContent = m.line ?? "";
    this.memoryEl.title = [m.status, m.lines, m.viewLine].filter(Boolean).join("\n");
  }

  public setState(enabled: boolean, running: boolean): void {
    this.enabled = enabled;
    this.textarea.disabled = !enabled;
    this.sendBtn.disabled = !enabled;
    this.stopBtn.classList.toggle("bp-hidden", !running);
  }

  public setDraft(text: string): void {
    this.textarea.value = text;
    this.autoGrow();
    this.textarea.focus();
  }

  public prependDraft(prefix: string): void {
    const cur = this.textarea.value;
    this.textarea.value = cur ? `${prefix}\n${cur}` : prefix;
    this.autoGrow();
    this.textarea.focus();
  }

  public showError(msg: string): void {
    this.errorEl.textContent = msg;
    this.errorEl.classList.remove("bp-hidden");
  }

  public clearError(): void {
    this.errorEl.textContent = "";
    this.errorEl.classList.add("bp-hidden");
  }

  public focus(): void {
    this.textarea.focus();
  }

  public refreshLocale(): void {
    this.textarea.placeholder = t("composer.placeholder", "Ask anything: @bot, >thread, %project, #room, $skill, /btw");
    this.sendBtn.textContent = t("composer.send", "Send");
    this.stopBtn.textContent = t("composer.stop", "Stop");
  }

  private autoGrow(): void {
    this.textarea.style.height = "auto";
    this.textarea.style.height = `${Math.min(180, Math.max(54, this.textarea.scrollHeight))}px`;
  }

  private shouldCompleteOnEnter(): boolean {
    if (this.popupMode === "mention") return true;
    if (this.popupMode === "slash") {
      const v = this.textarea.value;
      return v.startsWith("/") && !v.includes(" ");
    }
    return false;
  }

  private updatePopups(): void {
    const val = this.textarea.value;
    if (val.startsWith("/") && !val.includes(" ") && !val.includes("\n")) {
      const needle = val.slice(1).toLowerCase();
      this.filteredSlash = this.commands.filter((c) => c.word.toLowerCase().startsWith(needle));
      if (this.filteredSlash.length > 0) {
        this.popupMode = "slash";
        this.popupIdx = Math.min(this.popupIdx, this.filteredSlash.length - 1);
        this.renderPopup();
        return;
      }
    }

    const m = /(?:^|\s)([@>%#])([a-zA-Z0-9._-]{1,32})$/.exec(val.slice(0, this.textarea.selectionStart));
    if (m) {
      const sigil = m[1];
      const q = m[2];
      void this.cb.onQueryMentions(q, sigil).then((items) => {
        if (items.length > 0) {
          this.mentionItems = items;
          this.popupMode = "mention";
          this.popupIdx = 0;
          this.renderPopup();
        } else if (this.popupMode === "mention") {
          this.closePopup();
        }
      });
      return;
    }

    this.closePopup();
  }

  private closePopup(): void {
    this.popupMode = "none";
    this.popupEl.classList.add("bp-hidden");
    this.popupEl.replaceChildren();
  }

  private renderPopup(): void {
    const frag = document.createDocumentFragment();
    if (this.popupMode === "slash") {
      this.filteredSlash.forEach((cmd, idx) => {
        const row = document.createElement("button");
        row.type = "button";
        row.className = "bp-popup-row" + (idx === this.popupIdx ? " active" : "");
        row.setAttribute("data-testid", `slash-item-${cmd.word}`);
        const name = document.createElement("span");
        name.className = "bp-popup-name";
        name.textContent = `/${cmd.word}`;
        const desc = document.createElement("span");
        desc.className = "bp-popup-desc";
        desc.textContent = cmd.desc || cmd.label;
        row.append(name, desc);
        row.addEventListener("mousedown", (e) => {
          e.preventDefault();
          this.applyPopupChoice(idx);
        });
        frag.appendChild(row);
      });
    } else if (this.popupMode === "mention") {
      this.mentionItems.forEach((item, idx) => {
        const row = document.createElement("button");
        row.type = "button";
        row.className = "bp-popup-row" + (idx === this.popupIdx ? " active" : "");
        const name = document.createElement("span");
        name.className = "bp-popup-name";
        name.textContent = item.word || `${item.sigil}${item.label}`;
        const desc = document.createElement("span");
        desc.className = "bp-popup-desc";
        desc.textContent = item.detail || item.label;
        row.append(name, desc);
        row.addEventListener("mousedown", (e) => {
          e.preventDefault();
          this.applyPopupChoice(idx);
        });
        frag.appendChild(row);
      });
    }
    this.popupEl.replaceChildren(frag);
    this.popupEl.classList.remove("bp-hidden");
  }

  private applyPopupChoice(idx: number): void {
    if (this.popupMode === "slash") {
      const cmd = this.filteredSlash[idx];
      if (cmd) {
        this.textarea.value = `/${cmd.word} `;
        this.autoGrow();
        this.textarea.focus();
      }
      this.closePopup();
    } else if (this.popupMode === "mention") {
      const item = this.mentionItems[idx];
      if (item) {
        const pos = this.textarea.selectionStart;
        const before = this.textarea.value.slice(0, pos);
        const after = this.textarea.value.slice(pos);
        const replaced = before.replace(/([@>%#][a-zA-Z0-9._-]{1,32})$/, `${item.word} `);
        this.textarea.value = replaced + after;
        this.textarea.selectionStart = this.textarea.selectionEnd = replaced.length;
        this.textarea.focus();
      }
      this.closePopup();
    }
  }

  private submit(): void {
    if (!this.enabled) return;
    const text = this.textarea.value.trim();
    if (!text) return;
    this.closePopup();
    this.clearError();

    if (text.startsWith("/")) {
      const firstWord = text.slice(1).split(/\s+/)[0]?.toLowerCase() ?? "";
      if (this.commands.some((c) => c.word.toLowerCase() === firstWord)) {
        this.textarea.value = "";
        this.autoGrow();
        this.cb.onRunCommand(text);
        return;
      }
    }

    this.textarea.value = "";
    this.autoGrow();
    this.cb.onSend(text);
  }
}

/** A thread's memory progress (data/1 `thread.memory`, the hub's bots/recall/progress.rs). Every word is the hub's. */
export interface MemoryProgress {
  pct?: number;
  done?: number;
  total?: number;
  state?: string; // caught | behind | held
  status?: string;
  line?: string;
  lines?: string;
  viewLine?: string;
}
