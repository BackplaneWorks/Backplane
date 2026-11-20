// Thread timeline: entries, pagination, streaming text, asks, #1309 review bar, #1311 unanswered notice.
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
  onThreadAction: (action: string) => void;
  onLoadImage: (entryId: string, path: string) => Promise<{ mime: string; data: string } | null>;
}

function formatTime(ms: number): string {
  if (!ms) return "";
  const d = new Date(ms < 10_000_000_000 ? ms * 1000 : ms);
  return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

export class ChatView {
  private headEl: HTMLElement;
  private bannerEl: HTMLElement;
  private scrollEl: HTMLElement;
  private listEl: HTMLElement;
  private olderBtn: HTMLButtonElement;
  private cb: ChatCallbacks;
  private thread: ThreadItem | null = null;
  private entries: EntryItem[] = [];
  private hasMore = false;
  private review: ReviewBar | null = null;
  private unanswered: UnansweredNotice | null = null;
  private btwNote: { q: string; a: string } | null = null;
  private liveNodes = new Map<string, HTMLElement>();
  private imageCache = new Map<string, string>();

  constructor(container: HTMLElement, cb: ChatCallbacks) {
    this.cb = cb;
    this.headEl = document.createElement("header");
    this.headEl.className = "bp-thread-head";
    this.headEl.setAttribute("data-testid", "thread-head");

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
    this.olderBtn.addEventListener("click", () => this.cb.onLoadOlder());

    this.listEl = document.createElement("div");
    this.listEl.className = "bp-timeline-list";

    this.scrollEl.append(this.olderBtn, this.listEl);
    container.append(this.headEl, this.bannerEl, this.scrollEl);
  }

  public setLoading(thread: ThreadItem | null): void {
    this.thread = thread;
    this.entries = [];
    this.hasMore = false;
    this.review = null;
    this.unanswered = null;
    this.btwNote = null;
    this.liveNodes.clear();
    this.renderHead();
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
    this.review = null;
    this.unanswered = null;
    this.btwNote = null;
    this.liveNodes.clear();
    this.renderHead();
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
    this.renderHead();
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
    this.entries = entries.filter((e) => !e.agentOnly);
    this.hasMore = hasMore;
    this.review = review;
    this.unanswered = unanswered;
    this.btwNote = null;
    this.renderHead();
    this.renderBanners();
    this.renderEntries(true);
  }

  public updateThreadMeta(thread: ThreadItem): void {
    if (!this.thread || this.thread.id !== thread.id) return;
    this.thread = thread;
    this.renderHead();
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
    const prevHeight = this.scrollEl.scrollHeight;
    const seen = new Set(this.entries.map((e) => e.id));
    const add = older.filter((e) => !e.agentOnly && !seen.has(e.id));
    this.entries = [...add, ...this.entries];
    this.hasMore = hasMore;
    this.renderEntries(false);
    this.scrollEl.scrollTop += this.scrollEl.scrollHeight - prevHeight;
  }

  public upsertEntry(entry: EntryItem): void {
    if (entry.agentOnly) return;
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

  private renderHead(): void {
    this.headEl.replaceChildren();
    if (!this.thread) {
      const empty = document.createElement("span");
      empty.className = "bp-dim";
      empty.textContent = "Select a thread or start a new one.";
      this.headEl.appendChild(empty);
      return;
    }
    const left = document.createElement("div");
    left.className = "bp-thread-head-left";
    const title = document.createElement("h1");
    title.className = "bp-thread-head-title";
    title.textContent = this.thread.title || "Untitled thread";
    title.setAttribute("data-testid", "thread-title");

    const statePill = document.createElement("span");
    statePill.className = "bp-state-pill";
    statePill.textContent = this.thread.status || this.thread.state || "idle";
    left.append(title, statePill);

    const right = document.createElement("div");
    right.className = "bp-thread-head-right";
    if (this.thread.chip) {
      const chip = document.createElement("span");
      chip.className = "bp-model-chip";
      chip.textContent = this.thread.chip;
      right.appendChild(chip);
    }

    const pinBtn = document.createElement("button");
    pinBtn.type = "button";
    pinBtn.className = "bp-btn-ghost";
    pinBtn.textContent = this.thread.pinned ? t("menu.unpin", "Unpin") : t("menu.pin", "Pin");
    pinBtn.addEventListener("click", () => this.cb.onThreadAction(this.thread?.pinned ? "unpin" : "pin"));

    const settleBtn = document.createElement("button");
    settleBtn.type = "button";
    settleBtn.className = "bp-btn-ghost";
    settleBtn.textContent = this.thread.settled ? t("phone.unsettle", "Unsettle") : t("menu.settle", "Settle");
    settleBtn.addEventListener("click", () => this.cb.onThreadAction(this.thread?.settled ? "unsettle" : "settle"));

    right.append(pinBtn, settleBtn);
    this.headEl.append(left, right);
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
    for (const entry of this.entries) {
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

      const meta = document.createElement("div");
      meta.className = "bp-entry-meta";
      const who = document.createElement("span");
      who.className = "bp-entry-who";
      who.textContent = entry.kind === "user" ? t("secret.you", "You") : entry.kind === "assistant" ? t("model.col.agent", "Agent") : entry.kind;
      const time = document.createElement("span");
      time.className = "bp-entry-time";
      time.textContent = entry.live ? "Streaming…" : formatTime(entry.at);
      meta.append(who, time);

      if (entry.replyTo?.quote) {
        const rq = document.createElement("div");
        rq.className = "bp-reply-quote";
        rq.textContent = `↩ ${t("gpui.b.in.reply.to", "In reply to:")} ${entry.replyTo.quote}`;
        row.appendChild(rq);
      }

      const body = document.createElement("div");
      body.className = "bp-entry-body";
      if (entry.kind === "user") body.textContent = entry.text;
      else body.appendChild(renderMarkdown(entry.text || ""));
      if (entry.live) this.liveNodes.set(entry.id, body);

      row.append(meta, body);

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
      frag.appendChild(row);
    }
    this.listEl.replaceChildren(frag);
    if (scrollToBottom) this.scrollToBottom();
  }
}
