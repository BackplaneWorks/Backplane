// Composer matching GPUI native-chat.png:
// Tall bordered input box ("Ask anything: @bot, >thread, %project, #room, $skill, /btw"),
// slash-command popup (hello.commands), mentions popup, and bottom toolbar row
// with model chip on the left and accent-blue Send / red Stop on the right.
import { t, trs } from "./i18n";
import {
  applyFavoriteKeysToSnapshot,
  buildPickerViewModel,
  normalizeFavKey,
  normalizeFavKeys,
  parsePickerResponse,
  toggleFavoriteKey,
  type PickerSnapshot,
} from "./model-picker-model";
import type { PlanModeInfo } from "./sidebar";
// @ts-ignore virtual module injected by web/build.ts
import { PROVIDER_MARKS } from "virtual:provider-marks";

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
  onCyclePlanMode?: () => void;
  onSetPlanMode?: (mode: "once" | "always" | "off") => void;
  onLoadModelPicker?: () => Promise<unknown>;
  onSelectModel?: (provider: string, model: string) => Promise<void> | void;
  onSelectEffort?: (effort: string) => Promise<void> | void;
  onToggleFavorite?: (provider: string, model: string, on: boolean) => Promise<string[] | null | void> | void;
  getFavoriteModels?: () => string[];
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

function makePlanDocIcon(): SVGSVGElement {
  return makeComposerIcon([
    "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z",
    "M14 2v6h6",
    "M16 13H8",
    "M16 17H8",
    "M10 9H8",
  ]);
}

function makePlanPlayIcon(): SVGSVGElement {
  return makeComposerIcon(["M6 4l14 8-14 8z"]);
}

function makeStarIcon(filled: boolean): SVGSVGElement {
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("width", "13");
  svg.setAttribute("height", "13");
  svg.setAttribute("fill", filled ? "currentColor" : "none");
  svg.setAttribute("stroke", "currentColor");
  svg.setAttribute("stroke-width", "1.8");
  svg.setAttribute("stroke-linejoin", "round");
  svg.setAttribute("aria-hidden", "true");
  const p = document.createElementNS(SVG_NS, "path");
  p.setAttribute("d", "M12 2.5l2.9 6 6.6.9-4.8 4.6 1.2 6.5L12 17.4l-5.9 3.1 1.2-6.5-4.8-4.6 6.6-.9z");
  svg.appendChild(p);
  return svg;
}

function providerBadge(provider?: string): { mark?: string; glyph: string; color: string; title: string } {
  const p = (provider || "").toLowerCase();
  if (p.includes("claude") || p.includes("anthropic")) return { mark: "claude", glyph: "A", color: "#d97757", title: "Claude" };
  if (p.includes("codex") || p.includes("openai")) return { mark: "codex", glyph: "O", color: "#10a37f", title: "Codex" };
  if (p.includes("gemini") || p.includes("antigravity") || p === "agy") return { mark: "antigravity", glyph: "G", color: "#4285f4", title: "Antigravity" };
  if (p.includes("grok")) return { mark: "grok", glyph: "X", color: "#6b7280", title: "Grok" };
  if (p.includes("muse")) return { mark: "muse", glyph: "M", color: "#8b5cf6", title: "Muse" };
  if (p.includes("opencode")) return { mark: "opencode", glyph: "C", color: "#2563eb", title: "OpenCode" };
  if (p.includes("omp") || p.includes("oh-my-pi")) return { mark: "omp", glyph: "Ω", color: "#c026d3", title: "Oh My Pi" };
  if (p === "pi" || p.startsWith("pi/") || p.startsWith("@pi")) return { mark: "pi", glyph: "π", color: "#db2777", title: "Pi" };
  return { glyph: "●", color: "var(--accent)", title: provider || "Model" };
}

function makeProviderMarkSvg(mark: string): SVGSVGElement | null {
  const info = PROVIDER_MARKS[mark];
  if (!info || !info.d) return null;
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("viewBox", info.viewBox || "0 0 24 24");
  svg.setAttribute("width", "14");
  svg.setAttribute("height", "14");
  svg.setAttribute("fill", "currentColor");
  svg.setAttribute("aria-hidden", "true");
  const path = document.createElementNS(SVG_NS, "path");
  path.setAttribute("d", info.d);
  svg.appendChild(path);
  return svg;
}

export class ComposerView {
  private popupEl: HTMLElement;
  private attachStripEl: HTMLElement;
  private planBarEl: HTMLElement;
  private fileInput: HTMLInputElement;
  private textarea: HTMLTextAreaElement;
  private memoryEl: HTMLElement;
  private memoryFill: HTMLElement;
  private memoryText: HTMLElement;
  private modelBtn: HTMLButtonElement;
  private providerMarkEl: HTMLElement;
  private modelTextEl: HTMLElement;
  private ctxEl: HTMLElement;
  private modelPickerEl: HTMLElement;
  private modelSearchInput: HTMLInputElement;
  private modelPickerBodyEl: HTMLElement;
  private modelPickerEffortsEl: HTMLElement;
  private modelPickerOpen = false;
  private modelPickerLoading = false;
  private modelPickerSnap: PickerSnapshot | null = null;
  private modelPickerQuery = "";
  private modelPickerFocusIdx = -1;
  private onDocMouseDown: (e: MouseEvent) => void;
  private planChipBtn: HTMLButtonElement;
  private planChipIconHost: HTMLElement;
  private planChipLbl: HTMLElement;
  private attachBtn: HTMLButtonElement;
  private voiceBtn: HTMLButtonElement;
  private sendBtn: HTMLButtonElement;
  private stopBtn: HTMLButtonElement;
  private errorEl: HTMLElement;
  private replyEl: HTMLElement;
  private cb: ComposerCallbacks;

  private commands: SlashCommand[] = [];
  private popupMode: "none" | "slash" | "mention" = "none";
  private filteredSlash: SlashCommand[] = [];
  private mentionItems: MentionItem[] = [];
  private popupIdx = 0;
  private enabled = false;
  private attachments: PendingAttachment[] = [];
  private planMode: PlanModeInfo | null = null;

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

    // Reply's strip (the desktop's reply-strip): who wrote the message and its first line; a tap goes to it, × drops the reply
    this.replyEl = document.createElement("div");
    this.replyEl.className = "bp-reply-strip bp-hidden";
    this.replyEl.setAttribute("data-testid", "composer-reply");

    this.attachStripEl = document.createElement("div");
    this.attachStripEl.className = "bp-attach-strip bp-hidden";
    this.attachStripEl.setAttribute("data-testid", "composer-attachments");

    this.planBarEl = document.createElement("div");
    this.planBarEl.className = "bp-plan-bar bp-hidden";
    this.planBarEl.setAttribute("data-testid", "composer-plan-bar");

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

    const modelWrap = document.createElement("button");
    modelWrap.type = "button";
    modelWrap.className = "bp-composer-model";
    modelWrap.setAttribute("data-testid", "composer-model-chip");
    modelWrap.setAttribute("aria-haspopup", "dialog");
    modelWrap.setAttribute("aria-expanded", "false");
    modelWrap.title = t("composer.model.tip", "Choose the model and effort");
    this.modelBtn = modelWrap;
    this.providerMarkEl = document.createElement("span");
    this.providerMarkEl.className = "bp-provider-mark";
    this.providerMarkEl.textContent = "●";
    this.modelTextEl = document.createElement("span");
    this.modelTextEl.className = "bp-model-text";
    this.modelTextEl.textContent = "default";
    this.ctxEl = document.createElement("span");
    this.ctxEl.className = "bp-ctx-pct bp-hidden";
    this.ctxEl.setAttribute("data-testid", "composer-ctx");
    const modelChevron = document.createElement("span");
    modelChevron.className = "bp-model-caret";
    modelChevron.textContent = "▾";
    modelWrap.append(this.providerMarkEl, this.modelTextEl, this.ctxEl, modelChevron);
    modelWrap.addEventListener("click", () => { void this.toggleModelPicker(); });

    this.modelPickerEl = document.createElement("div");
    this.modelPickerEl.className = "bp-model-picker bp-hidden";
    this.modelPickerEl.setAttribute("data-testid", "model-picker");
    this.modelPickerEl.setAttribute("role", "dialog");
    const mpHead = document.createElement("div");
    mpHead.className = "bp-mp-search-wrap";
    this.modelSearchInput = document.createElement("input");
    this.modelSearchInput.type = "search";
    this.modelSearchInput.className = "bp-mp-search";
    this.modelSearchInput.placeholder = t("gpui.search.models", "Search models");
    this.modelSearchInput.setAttribute("data-testid", "model-picker-search");
    this.modelSearchInput.addEventListener("input", () => {
      this.modelPickerQuery = this.modelSearchInput.value;
      this.modelPickerFocusIdx = -1;
      this.renderModelPicker();
    });
    this.modelSearchInput.addEventListener("keydown", (e) => {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        this.closeModelPicker();
        this.textarea.focus();
        return;
      }
      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        const items = this.modelPickerFocusableButtons();
        if (items.length === 0) return;
        e.preventDefault();
        const delta = e.key === "ArrowDown" ? 1 : -1;
        this.modelPickerFocusIdx =
          this.modelPickerFocusIdx < 0
            ? (delta > 0 ? 0 : items.length - 1)
            : (this.modelPickerFocusIdx + delta + items.length) % items.length;
        this.updateModelPickerKbdFocus(items);
        return;
      }
      if (e.key === "Enter") {
        const items = this.modelPickerFocusableButtons();
        const target =
          this.modelPickerFocusIdx >= 0 && this.modelPickerFocusIdx < items.length
            ? items[this.modelPickerFocusIdx]
            : (this.modelPickerQuery.trim() ? items[0] : undefined);
        if (target) {
          e.preventDefault();
          target.click();
        }
      }
    });
    mpHead.appendChild(this.modelSearchInput);
    this.modelPickerBodyEl = document.createElement("div");
    this.modelPickerBodyEl.className = "bp-mp-body";
    this.modelPickerEffortsEl = document.createElement("div");
    this.modelPickerEffortsEl.className = "bp-mp-efforts bp-hidden";
    this.modelPickerEl.append(mpHead, this.modelPickerBodyEl, this.modelPickerEffortsEl);

    this.planChipBtn = document.createElement("button");
    this.planChipBtn.type = "button";
    this.planChipBtn.className = "bp-plan-chip bp-hidden";
    this.planChipBtn.setAttribute("data-testid", "composer-plan-chip");
    this.planChipBtn.setAttribute("aria-pressed", "false");
    this.planChipIconHost = document.createElement("span");
    this.planChipIconHost.className = "bp-plan-chip-ic";
    this.planChipIconHost.appendChild(makePlanDocIcon());
    this.planChipLbl = document.createElement("span");
    this.planChipLbl.textContent = t("gpui.mgmt.plan.tab", "Plan");
    this.planChipBtn.append(this.planChipIconHost, this.planChipLbl);
    this.planChipBtn.addEventListener("click", () => this.cb.onCyclePlanMode?.());

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
    this.attachBtn.title = t("web.composer.attach", "Attach image");
    this.attachBtn.setAttribute("aria-label", t("web.composer.attach", "Attach image"));
    this.attachBtn.setAttribute("data-testid", "composer-attach");
    this.attachBtn.appendChild(makeComposerIcon([
      "M21.44 11.05l-9.19 9.19a6 6 0 0 1-8.49-8.49l9.19-9.19a4 4 0 0 1 5.66 5.66l-9.2 9.19a2 2 0 0 1-2.83-2.83l8.49-8.48",
    ]));
    this.attachBtn.addEventListener("click", () => this.fileInput.click());

    this.voiceBtn = document.createElement("button");
    this.voiceBtn.type = "button";
    this.voiceBtn.className = "icon bp-composer-tool";
    this.voiceBtn.title = t("web.composer.voice", "Voice dictation");
    this.voiceBtn.setAttribute("aria-label", t("web.composer.voice", "Voice dictation"));
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
    bar.append(modelWrap, this.planChipBtn, this.memoryEl, btns);
    inner.append(this.modelPickerEl, this.popupEl, this.errorEl, this.replyEl, this.attachStripEl, this.planBarEl, this.fileInput, this.textarea, bar);
    wrap.appendChild(inner);
    container.appendChild(wrap);

    this.onDocMouseDown = (e: MouseEvent) => {
      if (!this.modelPickerEl.isConnected) {
        document.removeEventListener("mousedown", this.onDocMouseDown);
        return;
      }
      if (!this.modelPickerOpen) return;
      const target = e.target as Node | null;
      if (target && (this.modelPickerEl.contains(target) || this.modelBtn.contains(target))) return;
      this.closeModelPicker();
    };
    document.addEventListener("mousedown", this.onDocMouseDown);

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
      if (e.key === "Tab" && e.shiftKey && this.planMode !== null) {
        e.preventDefault();
        this.cb.onCyclePlanMode?.();
        return;
      }
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        this.submit();
      }
    });
  }

  public setCommands(cmds: SlashCommand[]): void {
    // the hub's words carry their slash ("/project", session/composer.rs WORDS); the composer keeps the bare word
    this.commands = cmds.map((c) => ({ ...c, word: c.word.replace(/^\//, "") }));
  }

  public setModelChip(chip: string, provider?: string, ctxPct?: number): void {
    const badge = providerBadge(provider);
    const svg = badge.mark ? makeProviderMarkSvg(badge.mark) : null;
    if (svg) this.providerMarkEl.replaceChildren(svg);
    else this.providerMarkEl.textContent = badge.glyph;
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

  public setPlanMode(mode: PlanModeInfo | null): void {
    this.planMode = mode;
    this.textarea.classList.remove("is-plan", "is-plan-exec");
    this.sendBtn.classList.remove("is-plan", "is-plan-exec");
    this.planChipBtn.classList.remove("is-plan", "is-exec");
    this.planBarEl.classList.remove("is-exec");

    if (!mode) {
      this.planChipBtn.classList.add("bp-hidden");
      this.planChipBtn.setAttribute("aria-pressed", "false");
      this.planBarEl.classList.add("bp-hidden");
      this.planBarEl.replaceChildren();
      this.sendBtn.textContent = t("composer.send", "Send");
      return;
    }

    const phase = mode.phase;
    const active = phase !== "off";
    const isPlanTone = phase === "armed" || phase === "planning" || phase === "pending";
    const isExec = phase === "executing";

    this.planChipBtn.classList.remove("bp-hidden");
    this.planChipBtn.classList.toggle("is-plan", isPlanTone);
    this.planChipBtn.classList.toggle("is-exec", isExec);
    this.planChipBtn.setAttribute("aria-pressed", active ? "true" : "false");
    const tip = `${t("gpui.planmode.tip", "Plan before acting (1 turn, always, or off)")} · Shift+Tab`;
    this.planChipBtn.title = tip;
    this.planChipBtn.setAttribute("aria-label", tip);
    this.planChipIconHost.replaceChildren(isExec ? makePlanPlayIcon() : makePlanDocIcon());
    this.planChipLbl.textContent = isExec
      ? t("gpui.planmode.chip.building", "Building")
      : phase === "pending"
        ? t("gpui.mgmt.fab.reviewbtn", "Review")
        : isPlanTone && mode.once
          ? t("gpui.planmode.chip.once", "Plan · 1t")
          : t("gpui.mgmt.plan.tab", "Plan");

    this.textarea.classList.toggle("is-plan", isPlanTone);
    this.textarea.classList.toggle("is-plan-exec", isExec);
    this.sendBtn.classList.toggle("is-plan", isPlanTone);
    this.sendBtn.classList.toggle("is-plan-exec", isExec);
    this.sendBtn.textContent = isPlanTone
      ? t("gpui.mgmt.plan.tab", "Plan")
      : t("composer.send", "Send");

    if (!active) {
      this.planBarEl.classList.add("bp-hidden");
      this.planBarEl.replaceChildren();
      return;
    }

    this.planBarEl.classList.remove("bp-hidden");
    this.planBarEl.classList.toggle("is-exec", isExec);
    const barIcon = document.createElement("span");
    barIcon.className = "bp-plan-bar-ic";
    barIcon.appendChild(isExec ? makePlanPlayIcon() : makePlanDocIcon());
    const barMsg = document.createElement("span");
    barMsg.className = "bp-plan-bar-text";
    barMsg.textContent = phase === "armed" && mode.once
      ? t("gpui.planmode.bar.once", "Plan mode (1 turn) — explores and proposes a plan before editing")
      : phase === "armed"
        ? t("gpui.planmode.bar.always", "Plan mode — explores and proposes a plan before editing")
        : phase === "planning"
          ? t("gpui.planmode.bar.planning", "Planning — drafting a plan before edits")
          : phase === "pending"
            ? t("gpui.planmode.bar.pending", "Proposed plan awaits review below")
            : t("gpui.planmode.bar.executing", "Building the approved plan");

    const nodes: HTMLElement[] = [barIcon, barMsg];
    if (phase === "armed" || phase === "planning") {
      const scopes = document.createElement("div");
      scopes.className = "bp-plan-scopes";
      const mkScope = (lbl: string, on: boolean, target: "once" | "always" | "off", tid: string) => {
        const b = document.createElement("button");
        b.type = "button";
        b.className = "bp-plan-scope-btn" + (on ? " active" : "");
        b.textContent = lbl;
        b.disabled = !this.enabled;
        b.setAttribute("data-testid", tid);
        b.addEventListener("click", () => this.cb.onSetPlanMode?.(target));
        return b;
      };
      scopes.append(
        mkScope(t("gpui.planmode.scope.once", "1 turn"), mode.once, "once", "plan-scope-once"),
        mkScope(t("gpui.planmode.scope.always", "Always"), !mode.once, "always", "plan-scope-always"),
        mkScope(t("gpui.bots.recall.off", "Off"), false, "off", "plan-scope-off"),
      );
      nodes.push(scopes);
    }
    this.planBarEl.replaceChildren(...nodes);
  }

  public setState(enabled: boolean, running: boolean): void {
    this.enabled = enabled;
    this.textarea.disabled = !enabled;
    this.sendBtn.disabled = !enabled;
    this.attachBtn.disabled = !enabled;
    this.voiceBtn.disabled = !enabled;
    this.modelBtn.disabled = !enabled;
    if (!enabled) this.closeModelPicker();
    this.planChipBtn.disabled = !enabled;
    this.planBarEl.querySelectorAll<HTMLButtonElement>(".bp-plan-scope-btn").forEach((b) => { b.disabled = !enabled; });
    this.stopBtn.classList.toggle("bp-hidden", !running);
  }

  public destroy(): void {
    document.removeEventListener("mousedown", this.onDocMouseDown);
  }

  public isModelPickerOpen(): boolean {
    return this.modelPickerOpen;
  }

  public closeModelPicker(): void {
    if (!this.modelPickerOpen) return;
    this.modelPickerOpen = false;
    this.modelPickerFocusIdx = -1;
    this.modelPickerEl.classList.add("bp-hidden");
    this.modelBtn.classList.remove("is-open");
    this.modelBtn.setAttribute("aria-expanded", "false");
  }

  private modelPickerFocusableButtons(): HTMLButtonElement[] {
    return Array.from(this.modelPickerBodyEl.querySelectorAll<HTMLButtonElement>(".bp-mp-row-main:not(:disabled)"));
  }

  private updateModelPickerKbdFocus(items: HTMLButtonElement[]): void {
    items.forEach((btn, idx) => {
      const active = idx === this.modelPickerFocusIdx;
      btn.classList.toggle("is-kbd", active);
      if (active) btn.scrollIntoView({ block: "nearest" });
    });
  }

  public updateFavoriteModels(keys: string[]): void {
    if (!this.modelPickerSnap) return;
    this.modelPickerSnap = applyFavoriteKeysToSnapshot(this.modelPickerSnap, keys);
    if (this.modelPickerOpen) this.renderModelPicker();
  }

  private async toggleModelPicker(): Promise<void> {
    if (!this.enabled) return;
    if (this.modelPickerOpen) {
      this.closeModelPicker();
      return;
    }
    this.closePopup();
    this.modelPickerOpen = true;
    this.modelPickerQuery = "";
    this.modelPickerFocusIdx = -1;
    this.modelSearchInput.value = "";
    this.modelPickerEl.classList.remove("bp-hidden");
    this.modelBtn.classList.add("is-open");
    this.modelBtn.setAttribute("aria-expanded", "true");
    this.modelPickerLoading = true;
    this.renderModelPicker();
    this.modelSearchInput.focus();
    await this.reloadModelPicker();
  }

  private async reloadModelPicker(): Promise<void> {
    if (!this.cb.onLoadModelPicker) {
      this.modelPickerLoading = false;
      this.renderModelPicker();
      return;
    }
    try {
      const raw = await this.cb.onLoadModelPicker();
      if (!this.modelPickerOpen) return;
      this.modelPickerSnap = parsePickerResponse(raw, this.cb.getFavoriteModels?.() ?? []);
    } catch {
      // keep prior snapshot if reload failed
    } finally {
      this.modelPickerLoading = false;
      if (this.modelPickerOpen) this.renderModelPicker();
    }
  }

  private async toggleFavoriteInPicker(provider: string, model: string): Promise<void> {
    if (!this.modelPickerSnap) return;
    const prev = this.modelPickerSnap;
    const nk = normalizeFavKey(provider, model);
    if (!nk) return;
    const wasStarred = prev.favoriteKeys.includes(nk);
    const toggled = toggleFavoriteKey(prev.favoriteKeys, provider, model);
    if (toggled.keys.length === prev.favoriteKeys.length && toggled.starred === wasStarred) {
      return;
    }
    this.modelPickerSnap = applyFavoriteKeysToSnapshot(prev, toggled.keys);
    this.renderModelPicker();
    if (!this.cb.onToggleFavorite) return;
    try {
      const next = await this.cb.onToggleFavorite(provider, model, toggled.starred);
      if (Array.isArray(next) && this.modelPickerSnap) {
        this.modelPickerSnap = applyFavoriteKeysToSnapshot(this.modelPickerSnap, normalizeFavKeys(next));
        if (this.modelPickerOpen) this.renderModelPicker();
      }
    } catch (err) {
      if (this.modelPickerSnap) {
        const auth = this.cb.getFavoriteModels?.();
        const reverted = Array.isArray(auth)
          ? normalizeFavKeys(auth)
          : toggleFavoriteKey(this.modelPickerSnap.favoriteKeys, provider, model, !toggled.starred).keys;
        this.modelPickerSnap = applyFavoriteKeysToSnapshot(this.modelPickerSnap, reverted);
        if (this.modelPickerOpen) this.renderModelPicker();
      }
      this.showError(err instanceof Error && err.message ? err.message : "Could not update favorite model");
    }
  }

  private renderProviderGlyph(provider: string): HTMLElement {
    const badge = providerBadge(provider);
    const wrap = document.createElement("span");
    wrap.className = "bp-mp-mark";
    wrap.style.color = badge.color;
    wrap.title = badge.title;
    const svg = badge.mark ? makeProviderMarkSvg(badge.mark) : null;
    if (svg) wrap.appendChild(svg);
    else wrap.textContent = badge.glyph;
    return wrap;
  }

  private makeStarButton(
    provider: string,
    model: string,
    starred: boolean,
    testIdPrefix = "model-star",
  ): HTMLButtonElement {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "bp-mp-star" + (starred ? " is-starred" : "");
    const label = starred
      ? t("gpui.picker.favorite.remove", "Remove from favorites")
      : t("gpui.picker.favorite.add", "Add to favorites");
    btn.title = label;
    btn.setAttribute("aria-label", label);
    btn.setAttribute("aria-pressed", starred ? "true" : "false");
    btn.setAttribute("data-testid", `${testIdPrefix}-${provider}-${model}`);
    btn.appendChild(makeStarIcon(starred));
    btn.addEventListener("click", (e) => {
      e.preventDefault();
      e.stopPropagation();
      void this.toggleFavoriteInPicker(provider, model);
    });
    return btn;
  }

  private renderModelPicker(): void {
    if (!this.modelPickerOpen) return;
    this.modelPickerFocusIdx = -1;
    if (!this.modelPickerSnap) {
      const empty = document.createElement("div");
      empty.className = "bp-mp-empty";
      empty.textContent = this.modelPickerLoading ? "…" : t("gpui.no.models", "No models");
      this.modelPickerBodyEl.replaceChildren(empty);
      this.modelPickerEffortsEl.classList.add("bp-hidden");
      this.modelPickerEffortsEl.replaceChildren();
      return;
    }

    const vm = buildPickerViewModel(this.modelPickerSnap, this.modelPickerQuery);
    const frag = document.createDocumentFragment();

    if (vm.favorites.length > 0) {
      const favSec = document.createElement("div");
      favSec.className = "bp-mp-section bp-mp-favorites";
      favSec.setAttribute("data-testid", "model-picker-favorites");

      const head = document.createElement("div");
      head.className = "bp-mp-head";
      const starIc = document.createElement("span");
      starIc.className = "bp-mp-head-star";
      starIc.appendChild(makeStarIcon(true));
      const title = document.createElement("span");
      title.textContent = t("gpui.picker.favorites", "Favorites");
      head.append(starIc, title);
      favSec.appendChild(head);

      for (const fav of vm.favorites) {
        const row = document.createElement("div");
        row.className = "bp-mp-row" + (fav.on ? " is-active" : "") + (!fav.ok ? " is-disabled" : "");
        row.setAttribute("data-testid", `model-fav-row-${fav.provider}-${fav.model}`);

        const main = document.createElement("button");
        main.type = "button";
        main.className = "bp-mp-row-main";
        main.disabled = !fav.ok;
        if (!fav.ok && fav.why) main.title = fav.why;
        const chk = document.createElement("span");
        chk.className = "bp-mp-check";
        chk.textContent = fav.on ? "✓" : "";
        const name = document.createElement("span");
        name.className = "bp-mp-name";
        name.textContent = fav.label;
        const note = document.createElement("span");
        note.className = "bp-mp-note";
        note.textContent = !fav.ok && fav.why ? trs(fav.why) : (fav.note && fav.note !== fav.label ? trs(fav.note) : fav.provider);
        main.append(chk, this.renderProviderGlyph(fav.provider), name, note);
        main.addEventListener("click", () => {
          if (!fav.ok) return;
          this.closeModelPicker();
          this.textarea.focus();
          void this.cb.onSelectModel?.(fav.provider, fav.model);
        });

        row.append(main, this.makeStarButton(fav.provider, fav.model, true, "model-fav-star"));
        favSec.appendChild(row);
      }
      frag.appendChild(favSec);
    }

    const groupsWrap = document.createElement("div");
    groupsWrap.className = "bp-mp-groups";
    groupsWrap.setAttribute("data-testid", "model-picker-groups");

    if (vm.groups.length === 0) {
      const empty = document.createElement("div");
      empty.className = "bp-mp-empty";
      empty.textContent = this.modelPickerQuery.trim()
        ? t("gpui.nothing.matches", "Nothing matches")
        : t("gpui.no.models", "No models");
      groupsWrap.appendChild(empty);
    } else {
      for (const grp of vm.groups) {
        const gEl = document.createElement("div");
        gEl.className = "bp-mp-group";
        const gHead = document.createElement("div");
        gHead.className = "bp-mp-group-head";
        const gLbl = document.createElement("span");
        gLbl.className = "bp-mp-group-title";
        gLbl.textContent = grp.label;
        gHead.append(this.renderProviderGlyph(grp.provider), gLbl);
        if (!grp.ok && grp.why) {
          const gWhy = document.createElement("span");
          gWhy.className = "bp-mp-group-why";
          gWhy.textContent = trs(grp.why);
          gHead.appendChild(gWhy);
        }
        gEl.appendChild(gHead);

        for (const m of grp.models) {
          const row = document.createElement("div");
          row.className = "bp-mp-row" + (m.on ? " is-active" : "") + (!m.ok ? " is-disabled" : "");
          row.setAttribute("data-testid", `model-row-${grp.provider}-${m.value || "default"}`);

          const main = document.createElement("button");
          main.type = "button";
          main.className = "bp-mp-row-main";
          main.disabled = !m.ok;
          if (!m.ok && m.why) main.title = m.why;
          const chk = document.createElement("span");
          chk.className = "bp-mp-check";
          chk.textContent = m.on ? "✓" : "";
          const name = document.createElement("span");
          name.className = "bp-mp-name";
          name.textContent = m.label;
          const note = document.createElement("span");
          note.className = "bp-mp-note";
          note.textContent = m.note && m.note !== m.label ? trs(m.note) : "";
          main.append(chk, name, note);
          main.addEventListener("click", () => {
            if (!m.ok) return;
            this.closeModelPicker();
            this.textarea.focus();
            void this.cb.onSelectModel?.(grp.provider, m.value);
          });

          row.appendChild(main);
          if (m.starrable) {
            row.appendChild(this.makeStarButton(grp.provider, m.value, m.starred));
          }
          gEl.appendChild(row);
        }
        groupsWrap.appendChild(gEl);
      }
    }
    frag.appendChild(groupsWrap);
    this.modelPickerBodyEl.replaceChildren(frag);

    if (vm.efforts.length > 0) {
      const effFrag = document.createDocumentFragment();
      const lbl = document.createElement("span");
      lbl.className = "bp-mp-efforts-lbl";
      lbl.textContent = t("model.col.effort", "Effort");
      effFrag.appendChild(lbl);
      for (const ef of vm.efforts) {
        const b = document.createElement("button");
        b.type = "button";
        b.className = "bp-mp-effort" + (ef.on ? " active" : "");
        b.textContent = ef.label;
        b.addEventListener("click", () => {
          void Promise.resolve(this.cb.onSelectEffort?.(ef.value)).then(() => {
            if (this.modelPickerOpen) void this.reloadModelPicker();
          });
        });
        effFrag.appendChild(b);
      }
      this.modelPickerEffortsEl.replaceChildren(effFrag);
      this.modelPickerEffortsEl.classList.remove("bp-hidden");
    } else {
      this.modelPickerEffortsEl.classList.add("bp-hidden");
      this.modelPickerEffortsEl.replaceChildren();
    }
  }

  public setDraft(text: string): void {
    this.textarea.value = text;
    this.autoGrow();
    this.textarea.focus();
  }

  /** The reply being written, or null: the strip over the box. Text goes in as text only. */
  public setReply(r: { who: string; line: string } | null, onGo?: () => void, onDrop?: () => void): void {
    if (!r) { this.replyEl.replaceChildren(); this.replyEl.classList.add("bp-hidden"); return; }
    const body = document.createElement("button");
    body.type = "button";
    body.className = "bp-reply-strip-body";
    body.setAttribute("data-testid", "composer-reply-go");
    const who = document.createElement("span");
    who.className = "bp-reply-strip-who";
    who.textContent = r.who;
    const line = document.createElement("span");
    line.className = "bp-reply-strip-line";
    line.textContent = t("gpui.b.replying.to", "↩ Replying to: {line}", { line: r.line });
    body.append(who, line);
    if (onGo) body.addEventListener("click", onGo);
    const drop = document.createElement("button");
    drop.type = "button";
    drop.className = "icon bp-reply-strip-drop";
    drop.textContent = "×";
    const cancel = t("reply.cancel", "Cancel reply");
    drop.title = cancel;
    drop.setAttribute("aria-label", cancel);
    drop.setAttribute("data-testid", "composer-reply-drop");
    if (onDrop) drop.addEventListener("click", onDrop);
    this.replyEl.replaceChildren(body, drop);
    this.replyEl.classList.remove("bp-hidden");
    this.textarea.focus();
  }

  public prependDraft(prefix: string): void {
    const cur = this.textarea.value;
    this.textarea.value = cur ? `${prefix}\n${cur}` : prefix;
    this.autoGrow();
    this.textarea.focus();
  }

  public showError(msg: string, action?: { label: string; onClick: () => void }): void {
    const txt = document.createElement("span");
    txt.textContent = trs(msg);
    if (action) {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "bp-composer-err-act";
      btn.setAttribute("data-testid", "composer-err-accounts");
      btn.textContent = action.label;
      btn.addEventListener("click", action.onClick);
      this.errorEl.replaceChildren(txt, btn);
    } else {
      this.errorEl.replaceChildren(txt);
    }
    this.errorEl.classList.remove("bp-hidden");
  }

  public clearError(): void {
    this.errorEl.replaceChildren();
    this.errorEl.classList.add("bp-hidden");
  }

  public focus(): void {
    this.textarea.focus();
  }

  public refreshLocale(): void {
    this.textarea.placeholder = t("composer.placeholder", "Ask anything: @bot, >thread, %project, #room, $skill, /btw");
    this.attachBtn.title = t("web.composer.attach", "Attach image");
    this.attachBtn.setAttribute("aria-label", t("web.composer.attach", "Attach image"));
    this.voiceBtn.title = t("web.composer.voice", "Voice dictation");
    this.voiceBtn.setAttribute("aria-label", t("web.composer.voice", "Voice dictation"));
    this.modelBtn.title = t("composer.model.tip", "Choose the model and effort");
    this.modelSearchInput.placeholder = t("gpui.search.models", "Search models");
    this.sendBtn.textContent = t("composer.send", "Send");
    this.stopBtn.textContent = t("composer.stop", "Stop");
    this.setPlanMode(this.planMode);
    if (this.modelPickerOpen) this.renderModelPicker();
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
        this.showError(t("web.composer.unsupported", "Unsupported image type: {type}", { type: file.type || file.name }));
        continue;
      }
      if (this.attachments.length >= MAX_ATTACHMENTS) {
        this.showError(t("web.composer.maximages", "At most {max} images per message.", { max: MAX_ATTACHMENTS }));
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
      rm.title = t("common.remove", "Remove");
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
