// Swarm board view: full parity with GPUI desktop and mobile swarm rooms.
// Uses the shared DOM-only markdown renderer (`md.ts`), incremental post DOM
// reconciliation (so native text selection across posts is preserved across
// updates), sticky-bottom scroll with a "Latest" jump button, member chips,
// synthesis cards, claim file chips, and image previews.
import { t } from "./i18n";
import { renderMarkdown } from "./md";
import { formatAge } from "./sidebar";

export interface SwarmRoom {
  id: string;
  name: string;
  kind: string;
  project: string;
  goal: string;
  state: string;
  reason: string;
  by: string;
}

export interface SwarmMember {
  member: string;
  thread: string;
  name: string;
  color: string;
  role: string;
  provider: string;
  model: string;
  state: string;
  title: string;
}

export interface SwarmPost {
  id: string;
  from: string;
  thread: string;
  color: string;
  text: string;
  tags: string[];
  reply: string;
  mentions: string[];
  files: string[];
  images: string[];
  guard: boolean;
  held: string;
  at: number;
}

export interface SwarmClaim {
  post: string;
  thread: string;
  files: string[];
  state: string;
}

export interface SwarmBoardData {
  room: SwarmRoom;
  members: SwarmMember[];
  posts: SwarmPost[];
  claims: SwarmClaim[];
}

export interface SwarmViewCallbacks {
  onLoad: (roomId: string, since?: string) => Promise<SwarmBoardData>;
  onPost: (roomId: string, text: string) => Promise<void>;
  onAct: (roomId: string, action: string, thread?: string) => Promise<void>;
  onOpenThread: (threadId: string) => void;
  onLoadImage: (entryId: string, path: string) => Promise<{ url?: string; mime?: string; data?: string } | null>;
}

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls: string, text?: string): HTMLElementTagNameMap[K] {
  const n = document.createElement(tag);
  if (cls) n.className = cls;
  if (text !== undefined) n.textContent = text;
  return n;
}

function str(o: Record<string, unknown>, k: string): string {
  const v = o[k];
  return typeof v === "string" ? v : "";
}

function strList(v: unknown): string[] {
  return Array.isArray(v) ? v.filter((x): x is string => typeof x === "string" && x.length > 0) : [];
}

function normMs(at: unknown): number {
  const n = typeof at === "number" ? at : 0;
  return n > 0 && n < 1e12 ? n * 1000 : n;
}

function safeHex(c: string): string {
  return /^#[0-9a-fA-F]{3,8}$/.test(c.trim()) ? c.trim() : "";
}

export function swarmEnded(state: string): boolean {
  return state === "done" || state === "stopped";
}

export function swarmPostLabels(p: SwarmPost): string[] {
  return p.guard ? [] : p.tags.filter((x) => x !== "synthesis");
}

export function parseSwarmBoard(raw: unknown): SwarmBoardData {
  const o = (raw && typeof raw === "object" ? raw : {}) as Record<string, unknown>;
  const r = (o.room && typeof o.room === "object" ? o.room : {}) as Record<string, unknown>;
  const room: SwarmRoom = {
    id: str(r, "id"),
    name: str(r, "name"),
    kind: str(r, "kind"),
    project: str(r, "project"),
    goal: str(r, "goal"),
    state: str(r, "state"),
    reason: str(r, "reason"),
    by: str(r, "by"),
  };
  const members: SwarmMember[] = Array.isArray(o.members)
    ? o.members
        .filter((m): m is Record<string, unknown> => Boolean(m && typeof m === "object"))
        .map((m) => ({
          member: str(m, "member"),
          thread: str(m, "thread"),
          name: str(m, "name"),
          color: str(m, "color"),
          role: str(m, "role"),
          provider: str(m, "provider"),
          model: str(m, "model"),
          state: str(m, "state"),
          title: str(m, "title"),
        }))
    : [];
  const posts: SwarmPost[] = Array.isArray(o.posts)
    ? o.posts
        .filter((p): p is Record<string, unknown> => Boolean(p && typeof p === "object"))
        .map((p) => ({
          id: str(p, "id"),
          from: str(p, "from"),
          thread: str(p, "thread"),
          color: str(p, "color"),
          text: str(p, "text"),
          tags: strList(p.tags),
          reply: str(p, "reply"),
          mentions: strList(p.mentions),
          files: strList(p.files),
          images: strList(p.images),
          guard: Boolean(p.guard),
          held: typeof p.held === "string" ? p.held : p.held === true ? "held" : "",
          at: normMs(p.at),
        }))
    : [];
  const claims: SwarmClaim[] = Array.isArray(o.claims)
    ? o.claims
        .filter((c): c is Record<string, unknown> => Boolean(c && typeof c === "object"))
        .map((c) => ({
          post: str(c, "post"),
          thread: str(c, "thread"),
          files: strList(c.files),
          state: str(c, "state"),
        }))
    : [];
  return { room, members, posts, claims };
}

export function mergeSwarmBoard(prev: SwarmBoardData | null, incoming: SwarmBoardData, since: string): SwarmBoardData {
  if (!prev || !since || prev.room.id !== incoming.room.id) return incoming;
  const map = new Map<string, number>();
  const posts = [...prev.posts];
  for (let i = 0; i < posts.length; i++) map.set(posts[i].id, i);
  for (const p of incoming.posts) {
    const idx = map.get(p.id);
    if (idx !== undefined) posts[idx] = p;
    else {
      map.set(p.id, posts.length);
      posts.push(p);
    }
  }
  return { room: incoming.room, members: incoming.members, posts, claims: incoming.claims };
}

/** Bolds `@name` mentions in text nodes without touching `<code>`, `<pre>`, or `<a>`. */
export function highlightSwarmMentions(root: HTMLElement, mentions: string[], memberMap: Map<string, string>): void {
  if (mentions.length === 0) return;
  const names = new Set<string>();
  for (const m of mentions) {
    const mapped = memberMap.get(m) ?? memberMap.get(m.toLowerCase()) ?? m;
    const n = mapped.trim().toLowerCase();
    if (n) names.add(n);
  }
  if (names.size === 0) return;

  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  const textNodes: Text[] = [];
  let node: Node | null;
  while ((node = walker.nextNode())) {
    const p = node.parentElement;
    if (!p || p.closest("code, pre, a")) continue;
    textNodes.push(node as Text);
  }
  const re = /@([A-Za-z0-9._-]+)/g;
  for (const tn of textNodes) {
    const raw = tn.nodeValue ?? "";
    if (!raw.includes("@")) continue;
    re.lastIndex = 0;
    let last = 0;
    let m: RegExpExecArray | null;
    let changed = false;
    const frag = document.createDocumentFragment();
    while ((m = re.exec(raw)) !== null) {
      if (!names.has(m[1].toLowerCase())) continue;
      changed = true;
      if (m.index > last) frag.appendChild(document.createTextNode(raw.slice(last, m.index)));
      const strong = document.createElement("strong");
      strong.className = "bp-swarm-mention";
      strong.textContent = m[0];
      frag.appendChild(strong);
      last = re.lastIndex;
    }
    if (changed) {
      if (last < raw.length) frag.appendChild(document.createTextNode(raw.slice(last)));
      tn.parentNode?.replaceChild(frag, tn);
    }
  }
}

function postSignature(p: SwarmPost, claimState: string): string {
  return `${p.from}|${p.color}|${p.held}|${p.guard ? 1 : 0}|${p.tags.join(",")}|${p.files.join(",")}|${p.images.join(",")}|${claimState}|${p.text}`;
}

export class SwarmView {
  private root: HTMLElement;
  private barEl: HTMLElement;
  private membersEl: HTMLElement;
  private scrollEl: HTMLElement;
  private feedEl: HTMLElement;
  private latestBtn: HTMLButtonElement;
  private composerEl: HTMLElement;
  private inputEl: HTMLTextAreaElement;
  private postBtn: HTMLButtonElement;
  private errEl: HTMLElement;
  private cb: SwarmViewCallbacks;

  private roomId = "";
  private gen = 0;
  private board: SwarmBoardData | null = null;
  private postEls = new Map<string, { sig: string; el: HTMLElement }>();
  private imageCache = new Map<string, string>();
  private pinnedToBottom = true;
  private pollTimer: number | null = null;
  private loadingGen = -1;

  constructor(container: HTMLElement, cb: SwarmViewCallbacks) {
    this.cb = cb;
    this.root = el("section", "bp-swarm-view bp-hidden");
    this.root.setAttribute("data-testid", "swarm-view");

    this.barEl = el("div", "bp-swarm-bar");
    this.membersEl = el("div", "bp-swarm-members");
    this.errEl = el("div", "bp-swarm-err bp-hidden");

    this.scrollEl = el("div", "bp-swarm-scroll");
    this.scrollEl.setAttribute("data-testid", "swarm-feed-scroll");
    this.feedEl = el("div", "bp-swarm-feed");
    this.scrollEl.appendChild(this.feedEl);

    this.latestBtn = el("button", "bp-swarm-latest bp-hidden", t("gpui.swarm.latest", "Latest"));
    this.latestBtn.type = "button";
    this.latestBtn.addEventListener("click", () => {
      this.pinnedToBottom = true;
      this.latestBtn.classList.add("bp-hidden");
      this.scrollEl.scrollTop = this.scrollEl.scrollHeight;
    });

    this.scrollEl.addEventListener("scroll", () => {
      const near = this.scrollEl.scrollHeight - this.scrollEl.scrollTop - this.scrollEl.clientHeight <= 24;
      this.pinnedToBottom = near;
      this.latestBtn.classList.toggle("bp-hidden", near);
    });

    // Drag selection copy: strip hover Copy buttons and insert newlines between multiple posts.
    this.feedEl.addEventListener("copy", (e) => {
      const sel = window.getSelection();
      if (!sel || sel.isCollapsed || sel.rangeCount === 0) return;
      const range = sel.getRangeAt(0);
      if (!this.feedEl.contains(range.commonAncestorContainer)) return;
      const frag = range.cloneContents();
      for (const btn of frag.querySelectorAll(".bp-swarm-copy, [data-bp-no-copy]")) btn.remove();
      const posts = frag.querySelectorAll("[data-swarm-post-id]");
      for (let i = 1; i < posts.length; i++) {
        posts[i].before(document.createTextNode("\n"));
      }
      if (e.clipboardData) {
        e.preventDefault();
        e.clipboardData.setData("text/plain", frag.textContent ?? "");
      }
    });

    this.composerEl = el("div", "bp-swarm-composer");
    this.inputEl = document.createElement("textarea");
    this.inputEl.className = "bp-swarm-input";
    this.inputEl.rows = 2;
    this.inputEl.placeholder = t("android.swarm.post.hint", "Post to the board…");
    this.inputEl.setAttribute("data-testid", "swarm-input");
    this.postBtn = el("button", "primary", t("android.post", "Post"));
    this.postBtn.type = "button";
    this.postBtn.setAttribute("data-testid", "swarm-post-btn");
    this.postBtn.disabled = true;

    this.inputEl.addEventListener("input", () => {
      this.postBtn.disabled = !this.inputEl.value.trim();
    });
    this.inputEl.addEventListener("keydown", (e) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        void this.submit();
      }
    });
    this.postBtn.addEventListener("click", () => { void this.submit(); });
    this.composerEl.append(this.inputEl, this.postBtn);

    const wrap = el("div", "bp-swarm-scroll-wrap");
    wrap.append(this.scrollEl, this.latestBtn);

    this.root.append(this.barEl, this.membersEl, this.errEl, wrap, this.composerEl);
    container.appendChild(this.root);
  }

  public currentRoomId(): string { return this.roomId; }
  public currentRoom(): SwarmRoom | null { return this.board?.room ?? null; }

  public open(roomId: string): void {
    const changed = this.roomId !== roomId;
    this.roomId = roomId;
    const g = ++this.gen;
    if (changed) {
      this.board = null;
      this.postEls.clear();
      this.feedEl.replaceChildren(el("div", "bp-dim bp-swarm-empty", t("common.loading", "Loading…")));
      this.barEl.replaceChildren();
      this.membersEl.replaceChildren();
      this.pinnedToBottom = true;
      this.latestBtn.classList.add("bp-hidden");
    }
    this.setError("");
    this.root.classList.remove("bp-hidden");
    this.stopPoll();
    void this.refresh(false, g).then(() => {
      if (g === this.gen && this.roomId === roomId) this.startPoll(g);
    });
  }

  public hide(): void {
    this.gen++;
    this.roomId = "";
    this.stopPoll();
    this.root.classList.add("bp-hidden");
  }

  public async refresh(incremental = true, expectedGen = this.gen): Promise<void> {
    if (!this.roomId || expectedGen !== this.gen || this.loadingGen === expectedGen) return;
    this.loadingGen = expectedGen;
    const id = this.roomId;
    const hasHeld = Boolean(this.board?.posts.some((p) => p.held !== ""));
    const since = incremental && !hasHeld && this.board && this.board.posts.length > 0
      ? this.board.posts[this.board.posts.length - 1].id
      : "";
    try {
      const next = await this.cb.onLoad(id, since || undefined);
      if (expectedGen !== this.gen || this.roomId !== id) return;
      this.setError("");
      this.board = mergeSwarmBoard(this.board, next, since);
      this.renderBoard();
    } catch (e) {
      if (expectedGen === this.gen && this.roomId === id && !this.board) {
        this.setError(e instanceof Error ? e.message : "Could not load swarm.");
      }
    } finally {
      if (this.loadingGen === expectedGen) this.loadingGen = -1;
    }
  }

  private startPoll(g: number): void {
    this.stopPoll();
    if (!this.roomId || (this.board && swarmEnded(this.board.room.state))) return;
    this.pollTimer = window.setInterval(() => {
      if (g !== this.gen || !this.roomId) { this.stopPoll(); return; }
      if (this.board && swarmEnded(this.board.room.state)) { this.stopPoll(); return; }
      void this.refresh(true, g);
    }, 2500);
  }

  private stopPoll(): void {
    if (this.pollTimer !== null) {
      window.clearInterval(this.pollTimer);
      this.pollTimer = null;
    }
  }

  private setError(msg: string): void {
    this.errEl.textContent = msg;
    this.errEl.classList.toggle("bp-hidden", !msg);
  }

  private async submit(): Promise<void> {
    const text = this.inputEl.value.trim();
    if (!text || !this.roomId) return;
    const id = this.roomId;
    this.inputEl.value = "";
    this.postBtn.disabled = true;
    try {
      await this.cb.onPost(id, text);
      this.pinnedToBottom = true;
      await this.refresh(true, this.gen);
    } catch (e) {
      this.inputEl.value = text;
      this.postBtn.disabled = false;
      this.setError(e instanceof Error ? e.message : "Failed to post.");
    }
  }

  private renderBoard(): void {
    const b = this.board;
    if (!b) return;

    // Status + goal + controls bar
    this.barEl.replaceChildren();
    const stateCls = b.room.state === "active" ? "bp-dot-work"
      : b.room.state === "paused" ? "bp-dot-need"
      : b.room.state === "done" ? "bp-dot-done"
      : "bp-dot-idle";
    const stWrap = el("div", "bp-swarm-state");
    stWrap.append(el("span", `bp-dot ${stateCls}`), el("span", "bp-swarm-state-word",
      b.room.reason ? `${b.room.state} (${b.room.reason})` : b.room.state || "idle"));
    this.barEl.appendChild(stWrap);

    if (b.room.goal) {
      this.barEl.appendChild(el("div", "bp-swarm-goal", b.room.goal));
    } else {
      this.barEl.appendChild(el("div", "bp-swarm-spacer"));
    }

    const acts = el("div", "bp-swarm-acts");
    const controls = b.room.state === "active" ? ["pause", "stop"]
      : b.room.state === "paused" ? ["resume", "stop"]
      : [];
    for (const act of controls) {
      const btn = el("button", act === "stop" ? "bp-btn-ghost bp-danger-text" : "bp-btn-ghost",
        act === "pause" ? t("gpui.swarm.pause", "Pause")
          : act === "resume" ? t("gpui.swarm.resume", "Resume")
          : t("gpui.swarm.stop", "Stop"));
      btn.type = "button";
      btn.addEventListener("click", () => {
        void this.cb.onAct(b.room.id, act).then(() => this.refresh(false, this.gen)).catch((e) => {
          this.setError(e instanceof Error ? e.message : `Could not ${act}`);
        });
      });
      acts.appendChild(btn);
    }
    this.barEl.appendChild(acts);

    // Members
    this.membersEl.replaceChildren();
    const ended = swarmEnded(b.room.state);
    const memberMap = new Map<string, string>();
    for (const m of b.members) {
      if (m.member && m.name) memberMap.set(m.member, m.name);
      if (m.name) memberMap.set(m.name.toLowerCase(), m.name);
    }
    for (const m of b.members) {
      const col = safeHex(m.color);
      const gone = m.state === "removed";
      const chip = el("div", "bp-swarm-chip" + (gone ? " gone" : "") + (m.thread && !gone ? " clickable" : ""));
      const bar = el("span", "bp-swarm-chip-bar");
      if (col) bar.style.backgroundColor = col;
      const mCls = m.state === "working" ? "bp-dot-work" : m.state === "waiting" ? "bp-dot-need" : "bp-dot-idle";
      const nameEl = el("span", "bp-swarm-chip-name", m.name);
      if (col) nameEl.style.color = col;
      chip.append(bar, el("span", `bp-dot ${mCls}`), nameEl);
      if (m.role && m.role !== m.name) chip.appendChild(el("span", "bp-swarm-chip-role", m.role));
      if (m.thread && !gone) {
        chip.title = [m.title, [m.provider, m.model].filter(Boolean).join(" / "), m.state].filter(Boolean).join(" · ");
        chip.addEventListener("click", () => this.cb.onOpenThread(m.thread));
        if (!ended) {
          const rm = el("button", "bp-swarm-chip-rm", "×");
          rm.type = "button";
          rm.title = t("gpui.swarm.remove", "Remove");
          rm.addEventListener("click", (e) => {
            e.stopPropagation();
            void this.cb.onAct(b.room.id, "remove", m.thread).then(() => this.refresh(false, this.gen)).catch(() => {});
          });
          chip.appendChild(rm);
        }
      }
      this.membersEl.appendChild(chip);
    }

    // Composer visibility
    this.composerEl.classList.toggle("bp-hidden", ended);

    // Posts (reconcile incrementally so selection is kept)
    if (b.posts.length === 0) {
      this.postEls.clear();
      this.feedEl.replaceChildren(el("div", "bp-dim bp-swarm-empty", t("android.swarm.none", "No posts yet.")));
      return;
    }

    const claimStateByPost = new Map<string, string>();
    for (const c of b.claims) {
      if (c.post && !claimStateByPost.has(c.post)) claimStateByPost.set(c.post, c.state);
    }

    const wasAtBottom = this.pinnedToBottom;
    const prevNodes = new Set(b.posts.map((p) => p.id));
    for (const [id, rec] of this.postEls) {
      if (!prevNodes.has(id)) {
        rec.el.remove();
        this.postEls.delete(id);
      }
    }
    const emptyEl = this.feedEl.querySelector(".bp-swarm-empty");
    if (emptyEl) emptyEl.remove();

    let cursor: Node | null = this.feedEl.firstChild;
    for (const p of b.posts) {
      const claimState = claimStateByPost.get(p.id) ?? "";
      const sig = postSignature(p, claimState);
      let rec = this.postEls.get(p.id);
      if (!rec) {
        const node = this.buildPostEl(p, claimState, memberMap);
        rec = { sig, el: node };
        this.postEls.set(p.id, rec);
      } else if (rec.sig !== sig) {
        const node = this.buildPostEl(p, claimState, memberMap);
        if (rec.el === cursor) cursor = node;
        rec.el.replaceWith(node);
        rec.sig = sig;
        rec.el = node;
      }
      if (rec.el !== cursor) {
        this.feedEl.insertBefore(rec.el, cursor);
      } else {
        cursor = cursor.nextSibling;
      }
    }

    if (wasAtBottom) {
      this.scrollEl.scrollTop = this.scrollEl.scrollHeight;
      this.latestBtn.classList.add("bp-hidden");
    } else {
      this.latestBtn.classList.remove("bp-hidden");
    }
  }

  private buildPostEl(p: SwarmPost, claimState: string, memberMap: Map<string, string>): HTMLElement {
    if (p.guard) {
      const row = el("div", "bp-swarm-guard", p.text);
      row.setAttribute("data-swarm-post-id", p.id);
      row.setAttribute("data-testid", "swarm-guard");
      return row;
    }

    const syn = p.tags.includes("synthesis");
    const col = safeHex(p.color) || (p.thread ? "" : "var(--accent)");
    const post = el("article", "bp-swarm-post" + (syn ? " synthesis" : "") + (p.held ? " held" : ""));
    post.setAttribute("data-swarm-post-id", p.id);
    post.setAttribute("data-testid", syn ? "swarm-synthesis" : `swarm-post-${p.id}`);

    const bar = el("div", "bp-swarm-post-bar");
    if (col) bar.style.backgroundColor = col;

    const inner = el("div", "bp-swarm-post-inner");
    const head = el("div", "bp-swarm-post-head");
    const author = el("span", "bp-swarm-post-author", p.from || t("secret.you", "You"));
    if (col) author.style.color = col;
    head.appendChild(author);

    if (syn) {
      const sTag = el("span", "bp-swarm-tag synthesis", t("synthesis", "SYNTHESIS"));
      if (col) { sTag.style.borderColor = col; sTag.style.color = col; }
      head.appendChild(sTag);
    }
    for (const tag of swarmPostLabels(p)) {
      head.appendChild(el("span", "bp-swarm-tag", tag));
    }
    if (p.at) {
      head.appendChild(el("span", "bp-swarm-post-ago", formatAge(p.at)));
    }
    if (p.held) {
      head.appendChild(el("span", "bp-swarm-post-held", `held: ${p.held}`));
    }
    head.appendChild(el("span", "bp-swarm-spacer"));

    const copyBtn = el("button", "bp-btn-ghost bp-swarm-copy", t("common.copy", "Copy"));
    copyBtn.type = "button";
    copyBtn.addEventListener("click", () => {
      navigator.clipboard?.writeText(p.text).then(() => {
        copyBtn.textContent = "✓";
        window.setTimeout(() => { copyBtn.textContent = t("common.copy", "Copy"); }, 1200);
      }).catch(() => {});
    });
    head.appendChild(copyBtn);

    const body = el("div", "bp-swarm-post-body bp-msg-md");
    body.appendChild(renderMarkdown(p.text));
    highlightSwarmMentions(body, p.mentions, memberMap);

    inner.append(head, body);

    if (p.images.length > 0) {
      const imgs = el("div", "bp-reply-images");
      for (const path of p.images) {
        const img = document.createElement("img");
        img.className = "bp-reply-img";
        img.alt = path;
        const key = `${p.id}|${path}`;
        const cached = this.imageCache.get(key);
        if (cached) img.src = cached;
        else {
          void this.cb.onLoadImage(p.id, path).then((res) => {
            if (!res) return;
            const url = res.url ?? (res.mime && res.data ? `data:${res.mime};base64,${res.data}` : "");
            if (!url) return;
            this.imageCache.set(key, url);
            img.src = url;
          });
        }
        imgs.appendChild(img);
      }
      inner.appendChild(imgs);
    }

    if (p.files.length > 0) {
      const files = el("div", "bp-swarm-files");
      for (const f of p.files) files.appendChild(el("span", "bp-swarm-file", f));
      if (claimState === "waiting") files.appendChild(el("span", "bp-swarm-wait", t("waiting", "waiting")));
      inner.appendChild(files);
    }

    post.append(bar, inner);
    return post;
  }
}
