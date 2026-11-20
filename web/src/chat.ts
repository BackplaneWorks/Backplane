// Thread timeline matching GPUI native-chat.png:
// Bottom-anchored conversation, right-aligned user bubble with Copy · Reply · time,
// left-aligned plain assistant reply with Finished time · Copy · Reply,
// hidden routine git/system notes, load-on-scroll pagination, asks, #1309 review bar, #1311 unanswered notice.
import { t } from "./i18n";
import { renderMarkdown } from "./md";
import type { ThreadItem } from "./sidebar";

export interface EntryTool { name: string; summary?: string; state?: string; }
export interface EntryAsk {
  id: string;
  state: "open" | "answered";
  question: string;
  options?: Array<{ id: string; label: string }>;
  answered?: string;
}
export interface EntryAttachment {
  name?: string;
  mime?: string;
  path?: string;
  previewUrl?: string;
}
export interface EntryItem {
  id: string;
  kind: string;
  text: string;
  at: number;
  clientId?: string;
  live?: boolean;
  agentOnly?: boolean;
  tool?: EntryTool;
  ask?: EntryAsk;
  replyTo?: { id: string; quote: string };
  images?: string[];
  attachments?: EntryAttachment[];
}
export interface ReviewBar { on?: boolean; text?: string; button?: string; }
export interface UnansweredNotice {
  head?: string;
  text?: string;
  since?: string;
  msgs?: number;
  buttons?: Array<{ label: string; action: string; value: string; primary?: boolean }>;
}
export interface ChatCallbacks {
  onLoadOlder: () => void;
  onAnswerAsk: (askId: string, answer: string) => void;
  onToggleReview: (nextOn: boolean) => void;
  onUnansweredAction: (action: string, value: string) => void;
  onReplyEntry: (entry: EntryItem) => void;
  onLoadImage: (entryId: string, path: string) => Promise<{ mime: string; data: string } | null>;
}

const SVG_NS = "http://www.w3.org/2000/svg";

function makeTinyIcon(paths: string[]): SVGSVGElement {
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("width", "12");
  svg.setAttribute("height", "12");
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

function toMs(ts: number): number {
  return ts < 10_000_000_000 ? ts * 1000 : ts;
}

function formatTime24(ms: number): string {
  if (!ms) return "";
  const d = new Date(toMs(ms));
  return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", hour12: false });
}

function formatDuration(deltaMs: number): string {
  if (!Number.isFinite(deltaMs) || deltaMs <= 0 || deltaMs > 3600_000) return "";
  const secs = Math.max(1, Math.round(deltaMs / 1000));
  if (secs < 60) return `${secs}s`;
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return s > 0 ? `${m}m ${s}s` : `${m}m`;
}

function isHiddenSystemNote(e: EntryItem): boolean {
  if (e.agentOnly) return true;
  // Match desktop GPUI: routine git snapshot/repo-init notes are not drawn in the chat timeline.
  if (e.kind === "note") {
    const txt = (e.text || "").trim();
    if (
      txt.startsWith("Made this folder a git repository") ||
      txt.startsWith("Backplane: snapshot") ||
      txt.includes("snapshot of existing work before first use")
    ) {
      return true;
    }
  }
  return false;
}

export class ChatView {
  private bannerEl: HTMLElement;
  private scrollEl: HTMLElement;
  private listEl: HTMLElement;
  private olderBtn: HTMLButtonElement;
  private cb: ChatCallbacks;
  private thread: ThreadItem | null = null;
  private entries: EntryItem[] = [];
  private hasMore = false;
  private loadingOlder = false;
  private review: ReviewBar | null = null;
  private unanswered: UnansweredNotice | null = null;
  private btwNote: { q: string; a: string } | null = null;
  private liveNodes = new Map<string, HTMLElement>();
  private imageCache = new Map<string, string>();

  constructor(container: HTMLElement, cb: ChatCallbacks) {
    this.cb = cb;

    this.bannerEl = document.createElement("div");
    this.bannerEl.className = "bp-thread-banners";

    this.scrollEl = document.createElement("div");
    this.scrollEl.className = "bp-timeline-scroll";
    this.scrollEl.setAttribute("data-testid", "timeline");

    this.olderBtn = document.createElement("button");
    this.olderBtn.type = "button";
    this.olderBtn.className = "bp-older-btn bp-hidden";
    this.olderBtn.textContent = t("android.load.earlier", "Load earlier");
    this.olderBtn.setAttribute("data-testid", "load-older-btn");
    this.olderBtn.addEventListener("click", () => {
      if (!this.loadingOlder && this.hasMore) {
        this.loadingOlder = true;
        this.cb.onLoadOlder();
      }
    });

    // Load older messages automatically on scroll near top (matching desktop paging)
    this.scrollEl.addEventListener("scroll", () => {
      if (this.hasMore && !this.loadingOlder && this.scrollEl.scrollTop < 48) {
        this.loadingOlder = true;
        this.cb.onLoadOlder();
      }
    });

    this.listEl = document.createElement("div");
    this.listEl.className = "bp-timeline-list";

    this.scrollEl.append(this.olderBtn, this.listEl);
    container.append(this.bannerEl, this.scrollEl);
  }

  public setLoading(thread: ThreadItem | null): void {
    this.thread = thread;
    this.entries = [];
    this.hasMore = false;
    this.loadingOlder = false;
    this.review = null;
    this.unanswered = null;
    this.btwNote = null;
    this.liveNodes.clear();
    this.renderBanners();
    this.olderBtn.classList.add("bp-hidden");
    const el = document.createElement("div");
    el.className = "bp-empty-state";
    el.textContent = "Loading messages…";
    this.listEl.replaceChildren(el);
  }

  public setError(thread: ThreadItem | null, message: string, onRetry: () => void): void {
    this.thread = thread;
    this.entries = [];
    this.hasMore = false;
    this.loadingOlder = false;
    this.review = null;
    this.unanswered = null;
    this.btwNote = null;
    this.liveNodes.clear();
    this.renderBanners();
    this.olderBtn.classList.add("bp-hidden");
    const box = document.createElement("div");
    box.className = "bp-empty-state";
    const msg = document.createElement("div");
    msg.className = "bp-bad";
    msg.textContent = message;
    const btn = document.createElement("button");
    btn.type = "button";
    btn.textContent = t("tools.retry", "Retry");
    btn.addEventListener("click", onRetry);
    box.append(msg, btn);
    this.listEl.replaceChildren(box);
  }

  public getEntries(): EntryItem[] {
    return this.entries.slice();
  }

  public refreshLocale(): void {
    const savedScroll = this.scrollEl.scrollTop;
    this.olderBtn.textContent = t("android.load.earlier", "Load earlier");
    this.renderBanners();
    if (this.thread && this.entries.length > 0) {
      this.renderEntries(false);
    }
    this.scrollEl.scrollTop = savedScroll;
  }

  public setThread(
    thread: ThreadItem | null,
    entries: EntryItem[],
    hasMore: boolean,
    review: ReviewBar | null,
    unanswered: UnansweredNotice | null,
  ): void {
    this.thread = thread;
    this.entries = entries.filter((e) => !isHiddenSystemNote(e));
    this.hasMore = hasMore;
    this.loadingOlder = false;
    this.review = review;
    this.unanswered = unanswered;
    this.btwNote = null;
    this.renderBanners();
    this.renderEntries(true);
  }

  public updateThreadMeta(thread: ThreadItem): void {
    if (!this.thread || this.thread.id !== thread.id) return;
    this.thread = thread;
  }

  public setReview(review: ReviewBar | null): void {
    this.review = review;
    this.renderBanners();
  }

  public setUnanswered(unanswered: UnansweredNotice | null): void {
    this.unanswered = unanswered;
    this.renderBanners();
  }

  public showBtw(q: string, a: string): void {
    this.btwNote = { q, a };
    this.renderBanners();
  }

  public prependEntries(older: EntryItem[], hasMore: boolean): void {
    this.loadingOlder = false;
    const prevHeight = this.scrollEl.scrollHeight;
    const seen = new Set(this.entries.map((e) => e.id));
    const add = older.filter((e) => !isHiddenSystemNote(e) && !seen.has(e.id));
    this.entries = [...add, ...this.entries];
    this.hasMore = hasMore;
    this.renderEntries(false);
    this.scrollEl.scrollTop += this.scrollEl.scrollHeight - prevHeight;
  }

  public clearLoadingOlder(): void {
    this.loadingOlder = false;
  }

  public upsertEntry(entry: EntryItem): void {
    if (isHiddenSystemNote(entry)) return;
    const atBottom = this.isNearBottom();
    let idx = this.entries.findIndex((e) => e.id === entry.id);
    if (idx < 0 && entry.clientId) {
      idx = this.entries.findIndex((e) => e.clientId && e.clientId === entry.clientId);
    }
    if (idx >= 0) this.entries[idx] = entry;
    else this.entries.push(entry);
    this.renderEntries(atBottom);
  }

  public removeEntry(entryId: string): void {
    const next = this.entries.filter((e) => e.id !== entryId);
    if (next.length !== this.entries.length) {
      this.entries = next;
      this.renderEntries(false);
    }
  }

  public appendLiveText(entryId: string, append: string, at: number): void {
    const atBottom = this.isNearBottom();
    let entry = this.entries.find((e) => e.id === entryId);
    if (!entry) {
      entry = { id: entryId, kind: "assistant", text: append, at, live: true };
      this.entries.push(entry);
      this.renderEntries(atBottom);
      return;
    }
    entry.text += append;
    entry.live = true;
    const node = this.liveNodes.get(entryId);
    if (node) {
      node.replaceChildren(renderMarkdown(entry.text));
      if (atBottom) this.scrollToBottom();
    } else {
      this.renderEntries(atBottom);
    }
  }

  private isNearBottom(): boolean {
    const el = this.scrollEl;
    return el.scrollHeight - el.scrollTop - el.clientHeight < 80;
  }

  private scrollToBottom(): void {
    this.scrollEl.scrollTop = this.scrollEl.scrollHeight;
  }

  private renderBanners(): void {
    const frag = document.createDocumentFragment();
    if (this.review && (this.review.text || this.review.button)) {
      const bar = document.createElement("div");
      bar.className = "bp-banner bp-review-bar";
      bar.setAttribute("data-testid", "review-bar");
      const txt = document.createElement("span");
      txt.textContent = this.review.text || "Review mode";
      bar.appendChild(txt);
      if (this.review.button) {
        const btn = document.createElement("button");
        btn.type = "button";
        btn.textContent = this.review.button;
        btn.addEventListener("click", () => this.cb.onToggleReview(!(this.review?.on ?? true)));
        bar.appendChild(btn);
      }
      frag.appendChild(bar);
    }
    if (this.unanswered && (this.unanswered.head || this.unanswered.text)) {
      const card = document.createElement("div");
      card.className = "bp-banner bp-unanswered";
      card.setAttribute("data-testid", "unanswered-notice");
      const body = document.createElement("div");
      body.className = "bp-unanswered-body";
      if (this.unanswered.head) {
        const h = document.createElement("strong");
        h.textContent = this.unanswered.head;
        body.appendChild(h);
      }
      if (this.unanswered.text) {
        const p = document.createElement("span");
        p.textContent = this.unanswered.text;
        body.appendChild(p);
      }
      card.appendChild(body);
      if (this.unanswered.buttons?.length) {
        const btns = document.createElement("div");
        btns.className = "bp-banner-actions";
        for (const b of this.unanswered.buttons) {
          const btn = document.createElement("button");
          btn.type = "button";
          if (b.primary) btn.className = "primary";
          btn.textContent = b.label;
          btn.addEventListener("click", () => this.cb.onUnansweredAction(b.action, b.value));
          btns.appendChild(btn);
        }
        card.appendChild(btns);
      }
      frag.appendChild(card);
    }
    if (this.btwNote) {
      const box = document.createElement("div");
      box.className = "bp-banner bp-btw-banner";
      const body = document.createElement("div");
      const q = document.createElement("div");
      q.className = "bp-dim";
      q.textContent = `/btw ${this.btwNote.q}`;
      const a = document.createElement("div");
      a.appendChild(renderMarkdown(this.btwNote.a));
      body.append(q, a);
      const close = document.createElement("button");
      close.type = "button";
      close.className = "icon";
      close.textContent = "×";
      close.addEventListener("click", () => { this.btwNote = null; this.renderBanners(); });
      box.append(body, close);
      frag.appendChild(box);
    }
    this.bannerEl.replaceChildren(frag);
  }

  private makeEntryActions(entry: EntryItem, isUser: boolean, prevUserAt?: number): HTMLElement {
    const foot = document.createElement("div");
    foot.className = "bp-entry-foot" + (isUser ? " user" : "");

    const copyBtn = document.createElement("button");
    copyBtn.type = "button";
    copyBtn.className = "bp-entry-act";
    const copyLbl = t("common.copy", "Copy");
    const copySpan = document.createElement("span");
    copySpan.textContent = copyLbl;
    copyBtn.append(
      makeTinyIcon(["M9 9h11v11H9z", "M5 15H4a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h10a1 1 0 0 1 1 1v1"]),
      copySpan,
    );
    copyBtn.addEventListener("click", () => {
      navigator.clipboard?.writeText(entry.text || "").then(() => {
        copySpan.textContent = "✓";
        window.setTimeout(() => { copySpan.textContent = copyLbl; }, 1000);
      }).catch(() => {});
    });

    const replyBtn = document.createElement("button");
    replyBtn.type = "button";
    replyBtn.className = "bp-entry-act";
    const replySpan = document.createElement("span");
    replySpan.textContent = "Reply";
    replyBtn.append(
      makeTinyIcon(["M9 17l-5-5 5-5", "M20 18v-2a4 4 0 0 0-4-4H4"]),
      replySpan,
    );
    replyBtn.addEventListener("click", () => this.cb.onReplyEntry(entry));

    const timeEl = document.createElement("span");
    timeEl.className = "bp-entry-time";
    const ts = formatTime24(entry.at);
    if (isUser) {
      timeEl.textContent = ts;
      foot.append(copyBtn, replyBtn, timeEl);
    } else {
      const dur = prevUserAt && entry.at ? formatDuration(toMs(entry.at) - toMs(prevUserAt)) : "";
      const finished = ts ? (dur ? `Finished ${ts} · ${dur}` : `Finished ${ts}`) : "";
      timeEl.textContent = entry.live ? "Streaming…" : finished;
      foot.append(timeEl, copyBtn, replyBtn);
    }
    return foot;
  }

  private renderEntries(scrollToBottom: boolean): void {
    this.olderBtn.classList.toggle("bp-hidden", !this.hasMore);
    this.liveNodes.clear();
    const frag = document.createDocumentFragment();
    if (!this.thread) {
      const empty = document.createElement("div");
      empty.className = "bp-empty-state";
      empty.textContent = "Choose a project thread on the left to begin.";
      frag.appendChild(empty);
      this.listEl.replaceChildren(frag);
      return;
    }
    if (this.entries.length === 0) {
      const empty = document.createElement("div");
      empty.className = "bp-empty-state";
      empty.textContent = "No messages yet. Type below to start a turn.";
      frag.appendChild(empty);
      this.listEl.replaceChildren(frag);
      return;
    }
    let lastUserAt = 0;
    for (const entry of this.entries) {
      if (entry.kind === "user" && entry.at) lastUserAt = entry.at;
      const row = document.createElement("article");
      row.className = `bp-entry bp-entry-${entry.kind}` + (entry.live ? " is-live" : "");
      row.setAttribute("data-entry-id", entry.id);

      if (entry.kind === "tool" && entry.tool) {
        const pill = document.createElement("div");
        pill.className = `bp-tool-pill bp-tool-${entry.tool.state || "done"}`;
        const name = document.createElement("span");
        name.className = "bp-tool-name";
        name.textContent = entry.tool.name;
        pill.appendChild(name);
        if (entry.tool.summary) {
          const sum = document.createElement("span");
          sum.className = "bp-tool-summary";
          sum.textContent = entry.tool.summary;
          pill.appendChild(sum);
        }
        row.appendChild(pill);
        frag.appendChild(row);
        continue;
      }

      if (entry.kind === "ask" && entry.ask) {
        const ask = entry.ask;
        const card = document.createElement("div");
        card.className = "bp-ask-card";
        card.setAttribute("data-testid", `ask-${ask.id}`);
        const q = document.createElement("div");
        q.className = "bp-ask-q";
        q.textContent = ask.question;
        card.appendChild(q);
        if (ask.state === "open") {
          const opts = document.createElement("div");
          opts.className = "bp-ask-opts";
          if (ask.options && ask.options.length > 0) {
            for (const opt of ask.options) {
              const btn = document.createElement("button");
              btn.type = "button";
              btn.textContent = opt.label;
              btn.addEventListener("click", () => this.cb.onAnswerAsk(ask.id, opt.id));
              opts.appendChild(btn);
            }
          } else {
            const inp = document.createElement("input");
            inp.type = "text";
            inp.className = "bp-input";
            inp.placeholder = "Type answer…";
            const send = document.createElement("button");
            send.type = "button";
            send.className = "primary";
            send.textContent = "Answer";
            send.addEventListener("click", () => {
              if (inp.value.trim()) this.cb.onAnswerAsk(ask.id, inp.value.trim());
            });
            opts.append(inp, send);
          }
          card.appendChild(opts);
        } else {
          const ans = document.createElement("div");
          ans.className = "bp-ask-answered";
          ans.textContent = `Answered: ${ask.answered ?? ""}`;
          card.appendChild(ans);
        }
        row.appendChild(card);
        frag.appendChild(row);
        continue;
      }

      // Hub visibility rule: never draw an empty message bubble with no text, images, or attachments.
      const hasBody = Boolean(entry.text && entry.text.trim().length > 0);
      const hasImages = Boolean(entry.images && entry.images.length > 0);
      const hasAtts = Boolean(entry.attachments && entry.attachments.length > 0);
      if (!hasBody && !hasImages && !hasAtts && !entry.live) continue;

      if (entry.replyTo?.quote) {
        const rq = document.createElement("div");
        rq.className = "bp-reply-quote";
        rq.textContent = `↩ ${t("gpui.b.in.reply.to", "In reply to:")} ${entry.replyTo.quote}`;
        row.appendChild(rq);
      }

      if (hasBody || entry.live) {
        const body = document.createElement("div");
        body.className = "bp-entry-body";
        if (entry.kind === "user") body.textContent = entry.text;
        else body.appendChild(renderMarkdown(entry.text || ""));
        if (entry.live) this.liveNodes.set(entry.id, body);
        row.appendChild(body);
      }

      if (hasAtts && entry.attachments) {
        const atts = document.createElement("div");
        atts.className = "bp-entry-images";
        for (const a of entry.attachments) {
          if (a.previewUrl) {
            const img = document.createElement("img");
            img.className = "bp-reply-img";
            img.src = a.previewUrl;
            img.alt = a.name || "attachment";
            atts.appendChild(img);
          } else if (a.path || a.name) {
            const pill = document.createElement("span");
            pill.className = "bp-tool-pill";
            pill.textContent = a.name || a.path || "image";
            atts.appendChild(pill);
          }
        }
        if (atts.childElementCount > 0) row.appendChild(atts);
      }

      if (entry.images && entry.images.length > 0) {
        const imgs = document.createElement("div");
        imgs.className = "bp-entry-images";
        for (const p of entry.images) {
          const img = document.createElement("img");
          img.className = "bp-reply-img";
          img.alt = p;
          const cacheKey = `${entry.id}|${p}`;
          const cached = this.imageCache.get(cacheKey);
          if (cached) img.src = cached;
          else {
            void this.cb.onLoadImage(entry.id, p).then((res) => {
              if (!res) return;
              const url = `data:${res.mime};base64,${res.data}`;
              this.imageCache.set(cacheKey, url);
              img.src = url;
            });
          }
          imgs.appendChild(img);
        }
        row.appendChild(imgs);
      }

      row.appendChild(this.makeEntryActions(entry, entry.kind === "user", entry.kind === "assistant" ? lastUserAt : undefined));
      frag.appendChild(row);
    }
    this.listEl.replaceChildren(frag);
    if (scrollToBottom) this.scrollToBottom();
  }
}
