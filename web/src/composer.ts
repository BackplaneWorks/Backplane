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

export interface PendingAttachment {
  name: string;
  mime: string;
  data: string; // raw base64
  previewUrl: string;
}

export interface ComposerCallbacks {
  onSend: (text: string, attachments: Array<{ name: string; mime: string; data: string }>) => void;
  onRunCommand: (text: string) => void;
  onInterrupt: () => void;
  onQueryMentions: (q: string, kind?: string) => Promise<MentionItem[]>;
  onToggleVoice?: () => void;
}

const SVG_NS = "http://www.w3.org/2000/svg";
const MAX_ATTACHMENTS = 4;
const ALLOWED_MIMES = new Set(["image/png", "image/jpeg", "image/webp", "image/gif"]);

function makeComposerIcon(paths: string[]): SVGSVGElement {
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("width", "15");
  svg.setAttribute("height", "15");
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

function providerBadge(provider?: string): { glyph: string; color: string; title: string } {
  const p = (provider || "").toLowerCase();
  if (p.includes("claude") || p.includes("anthropic")) return { glyph: "A", color: "#d97706", title: "Claude" };
  if (p.includes("codex") || p.includes("openai")) return { glyph: "O", color: "#10b981", title: "Codex" };
  if (p.includes("gemini") || p.includes("antigravity")) return { glyph: "G", color: "#3b82f6", title: "Gemini" };
  if (p.includes("grok")) return { glyph: "X", color: "#94a3b8", title: "Grok" };
  return { glyph: "●", color: "var(--accent)", title: provider || "Model" };
}

export class ComposerView {
  private popupEl: HTMLElement;
  private attachStripEl: HTMLElement;
  private fileInput: HTMLInputElement;
  private textarea: HTMLTextAreaElement;
  private memoryEl: HTMLElement;
  private memoryFill: HTMLElement;
  private memoryText: HTMLElement;
  private providerMarkEl: HTMLElement;
  private modelTextEl: HTMLElement;
  private ctxEl: HTMLElement;
  private attachBtn: HTMLButtonElement;
  private voiceBtn: HTMLButtonElement;
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
  private attachments: PendingAttachment[] = [];

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

    this.attachStripEl = document.createElement("div");
    this.attachStripEl.className = "bp-attach-strip bp-hidden";
    this.attachStripEl.setAttribute("data-testid", "composer-attachments");

    this.fileInput = document.createElement("input");
    this.fileInput.type = "file";
    this.fileInput.accept = "image/png,image/jpeg,image/webp,image/gif";
    this.fileInput.multiple = true;
    this.fileInput.className = "bp-hidden";
    this.fileInput.setAttribute("data-testid", "composer-file-input");
    this.fileInput.addEventListener("change", () => {
      if (this.fileInput.files) void this.addFiles(Array.from(this.fileInput.files));
      this.fileInput.value = "";
    });

    this.textarea = document.createElement("textarea");
    this.textarea.className = "bp-composer-input";
    this.textarea.rows = 2;
    this.textarea.placeholder = t("composer.placeholder", "Ask anything: @bot, >thread, %project, #room, $skill, /btw");
    this.textarea.setAttribute("data-testid", "composer-input");

    // Toolbar row under the input box (matches GPUI native-chat.png)
    const bar = document.createElement("div");
    bar.className = "bp-composer-bar";

    const modelWrap = document.createElement("div");
    modelWrap.className = "bp-composer-model";
    this.providerMarkEl = document.createElement("span");
    this.providerMarkEl.className = "bp-provider-mark";
    this.providerMarkEl.textContent = "●";
    this.modelTextEl = document.createElement("span");
    this.modelTextEl.className = "bp-model-text";
    this.modelTextEl.textContent = "default";
    this.ctxEl = document.createElement("span");
    this.ctxEl.className = "bp-ctx-pct bp-hidden";
    this.ctxEl.setAttribute("data-testid", "composer-ctx");
    modelWrap.append(this.providerMarkEl, this.modelTextEl, this.ctxEl);

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

    this.attachBtn = document.createElement("button");
    this.attachBtn.type = "button";
    this.attachBtn.className = "icon bp-composer-tool";
    this.attachBtn.title = "Attach image";
    this.attachBtn.setAttribute("aria-label", "Attach image");
    this.attachBtn.setAttribute("data-testid", "composer-attach");
    this.attachBtn.appendChild(makeComposerIcon([
      "M21.44 11.05l-9.19 9.19a6 6 0 0 1-8.49-8.49l9.19-9.19a4 4 0 0 1 5.66 5.66l-9.2 9.19a2 2 0 0 1-2.83-2.83l8.49-8.48",
    ]));
    this.attachBtn.addEventListener("click", () => this.fileInput.click());

    this.voiceBtn = document.createElement("button");
    this.voiceBtn.type = "button";
    this.voiceBtn.className = "icon bp-composer-tool";
    this.voiceBtn.title = "Voice dictation";
    this.voiceBtn.setAttribute("aria-label", "Voice dictation");
    this.voiceBtn.setAttribute("data-testid", "composer-voice");
    this.voiceBtn.appendChild(makeComposerIcon([
      "M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z",
      "M19 10v2a7 7 0 0 1-14 0v-2",
      "M12 19v4",
      "M8 23h8",
    ]));
    this.voiceBtn.addEventListener("click", () => this.cb.onToggleVoice?.());

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

    btns.append(this.attachBtn, this.voiceBtn, this.stopBtn, this.sendBtn);
    bar.append(modelWrap, this.memoryEl, btns);
    inner.append(this.popupEl, this.errorEl, this.attachStripEl, this.fileInput, this.textarea, bar);
    wrap.appendChild(inner);
    container.appendChild(wrap);

    this.textarea.addEventListener("paste", (e) => {
      const items = e.clipboardData?.items;
      if (!items) return;
      const files: File[] = [];
      for (let i = 0; i < items.length; i++) {
        if (items[i].kind === "file" && ALLOWED_MIMES.has(items[i].type)) {
          const f = items[i].getAsFile();
          if (f) files.push(f);
        }
      }
      if (files.length > 0) {
        e.preventDefault();
        void this.addFiles(files);
      }
    });

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

  public setModelChip(chip: string, provider?: string, ctxPct?: number): void {
    const badge = providerBadge(provider);
    this.providerMarkEl.textContent = badge.glyph;
    this.providerMarkEl.style.color = badge.color;
    this.providerMarkEl.title = badge.title;
    this.modelTextEl.textContent = chip || "default";
    if (typeof ctxPct === "number" && Number.isFinite(ctxPct) && ctxPct >= 0) {
      this.ctxEl.textContent = `ctx ${Math.round(ctxPct)}%`;
      this.ctxEl.classList.remove("bp-hidden");
    } else {
      this.ctxEl.textContent = "";
      this.ctxEl.classList.add("bp-hidden");
    }
  }

  public setFeatures(imagesOn: boolean, voiceOn: boolean): void {
    this.attachBtn.classList.toggle("bp-hidden", !imagesOn);
    this.voiceBtn.classList.toggle("bp-hidden", !voiceOn);
  }

  public setVoiceRecording(active: boolean): void {
    this.voiceBtn.classList.toggle("recording", active);
    this.voiceBtn.setAttribute("aria-pressed", active ? "true" : "false");
  }

  public setVoiceLevel(level: number): void {
    this.voiceBtn.setAttribute("data-level", String(level));
  }

  public getDraft(): string {
    return this.textarea.value;
  }

  public getCaret(): number {
    return this.textarea.selectionStart ?? this.textarea.value.length;
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
    this.attachBtn.disabled = !enabled;
    this.voiceBtn.disabled = !enabled;
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

  private async addFiles(files: File[]): Promise<void> {
    this.clearError();
    for (const file of files) {
      if (!ALLOWED_MIMES.has(file.type)) {
        this.showError(`Unsupported image type: ${file.type || file.name}`);
        continue;
      }
      if (this.attachments.length >= MAX_ATTACHMENTS) {
        this.showError(`At most ${MAX_ATTACHMENTS} images per message.`);
        break;
      }
      const dataUrl = await new Promise<string>((resolve, reject) => {
        const reader = new FileReader();
        reader.onload = () => resolve(String(reader.result || ""));
        reader.onerror = () => reject(new Error("read failed"));
        reader.readAsDataURL(file);
      }).catch(() => "");
      const comma = dataUrl.indexOf(",");
      if (comma < 0) continue;
      this.attachments.push({
        name: file.name || `image-${this.attachments.length + 1}.png`,
        mime: file.type,
        data: dataUrl.slice(comma + 1),
        previewUrl: dataUrl,
      });
    }
    this.renderAttachments();
  }

  private renderAttachments(): void {
    if (this.attachments.length === 0) {
      this.attachStripEl.classList.add("bp-hidden");
      this.attachStripEl.replaceChildren();
      return;
    }
    const frag = document.createDocumentFragment();
    this.attachments.forEach((att, idx) => {
      const chip = document.createElement("div");
      chip.className = "bp-attach-chip";
      const img = document.createElement("img");
      img.src = att.previewUrl;
      img.alt = att.name;
      img.className = "bp-attach-thumb";
      const rm = document.createElement("button");
      rm.type = "button";
      rm.className = "icon bp-attach-rm";
      rm.textContent = "×";
      rm.title = "Remove";
      rm.addEventListener("click", () => {
        this.attachments.splice(idx, 1);
        this.renderAttachments();
      });
      chip.append(img, rm);
      frag.appendChild(chip);
    });
    this.attachStripEl.replaceChildren(frag);
    this.attachStripEl.classList.remove("bp-hidden");
  }

  private submit(): void {
    if (!this.enabled) return;
    const text = this.textarea.value.trim();
    if (!text && this.attachments.length === 0) return;
    this.closePopup();
    this.clearError();

    if (text.startsWith("/") && this.attachments.length === 0) {
      const firstWord = text.slice(1).split(/\s+/)[0]?.toLowerCase() ?? "";
      if (this.commands.some((c) => c.word.toLowerCase() === firstWord)) {
        this.textarea.value = "";
        this.autoGrow();
        this.cb.onRunCommand(text);
        return;
      }
    }

    const outAtts = this.attachments.map(({ name, mime, data }) => ({ name, mime, data }));
    this.attachments = [];
    this.renderAttachments();
    this.textarea.value = "";
    this.autoGrow();
    this.cb.onSend(text, outAtts);
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
