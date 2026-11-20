// Browser rail panes: Files, Diff, and Review (Checks / BOM / Comments).
// Zero-dependency DOM renderers driven strictly by hub data/1 RPCs and /files/get.
// All dynamic text uses textContent (never innerHTML).

import type { ViewerHostApi } from "../app";

export interface PaneHandle {
  destroy(): void;
}

function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className?: string,
  text?: string
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function formatSize(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1 << 20) return `${Math.round(bytes / 1024)} KB`;
  return `${(bytes / (1 << 20)).toFixed(1)} MB`;
}

function fileExt(name: string): string {
  const idx = name.lastIndexOf(".");
  return idx >= 0 ? name.slice(idx + 1).toLowerCase() : "";
}

function isImageExt(ext: string): boolean {
  return ["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "ico"].includes(ext);
}

function isTextExt(ext: string): boolean {
  return [
    "txt", "md", "markdown", "json", "toml", "yaml", "yml", "xml", "csv", "tsv",
    "rs", "ts", "tsx", "js", "mjs", "cjs", "jsx", "css", "html", "htm", "sh", "bash",
    "py", "c", "h", "cc", "cpp", "hpp", "go", "java", "kt", "swift", "rb", "lua",
    "sql", "ini", "cfg", "conf", "env", "gitignore", "lock", "log",
    "gbr", "gtl", "gbl", "gts", "gbs", "gto", "gbo", "gm1", "drl", "pos", "net",
    "kicad_pro", "kicad_prl", "kicad_dru", "kicad_sym", "kicad_mod", "kicad_wks",
  ].includes(ext);
}

function viewerKindForFile(name: string): string | null {
  const ext = fileExt(name);
  if (ext === "kicad_pcb") return "board";
  if (ext === "kicad_sch") return "schematic";
  if (ext === "step" || ext === "stp" || ext === "glb") return "mech";
  return null;
}

// ============================================================================
// 1. Files Pane
// ============================================================================

interface FileEntry {
  dir: boolean;
  size: number;
  name: string;
}

export function mountFilesPane(
  stage: HTMLElement,
  api: ViewerHostApi,
  threadId: string,
  onOpenViewer: (kind: string, relPath: string) => void
): PaneHandle {
  let alive = true;
  let currentDir = "";
  let selectedPath = "";

  const root = el("div", "bp-pane bp-files-pane");
  root.setAttribute("data-testid", "pane-files");

  const bar = el("div", "bp-pane-bar");
  const crumbs = el("div", "bp-files-crumbs");
  const refreshBtn = el("button", "bp-v-btn", "Refresh");
  refreshBtn.type = "button";
  refreshBtn.addEventListener("click", () => void loadDir(currentDir));
  bar.append(crumbs, refreshBtn);

  const split = el("div", "bp-files-split");
  const listEl = el("div", "bp-files-list");
  listEl.setAttribute("data-testid", "files-list");
  const previewEl = el("div", "bp-files-preview");
  previewEl.setAttribute("data-testid", "files-preview");
  previewEl.append(el("div", "bp-pane-empty", "Select a file to preview"));
  split.append(listEl, previewEl);

  root.append(bar, split);
  stage.replaceChildren(root);

  function renderCrumbs(): void {
    crumbs.replaceChildren();
    const rootBtn = el("button", "bp-crumb-btn", "project");
    rootBtn.type = "button";
    rootBtn.addEventListener("click", () => void loadDir(""));
    crumbs.append(rootBtn);
    if (!currentDir) return;
    const parts = currentDir.split("/").filter(Boolean);
    let acc = "";
    for (const part of parts) {
      acc = acc ? `${acc}/${part}` : part;
      const target = acc;
      crumbs.append(el("span", "bp-vh-sep", "/"));
      const b = el("button", "bp-crumb-btn", part);
      b.type = "button";
      b.addEventListener("click", () => void loadDir(target));
      crumbs.append(b);
    }
  }

  async function openPreview(relPath: string, name: string, size: number): Promise<void> {
    selectedPath = relPath;
    previewEl.replaceChildren();

    const head = el("div", "bp-files-prev-head");
    head.append(el("span", "bp-files-prev-name", relPath));
    const acts = el("div", "bp-files-prev-acts");
    const vk = viewerKindForFile(name);
    if (vk) {
      const openBtn = el("button", "bp-v-btn is-primary", `Open in ${vk}`);
      openBtn.type = "button";
      openBtn.setAttribute("data-testid", "files-open-viewer");
      openBtn.addEventListener("click", () => onOpenViewer(vk, relPath));
      acts.append(openBtn);
    }
    const url = `/files/get?thread=${encodeURIComponent(threadId)}&path=${encodeURIComponent(relPath)}`;
    const dl = el("a", "bp-v-btn", "Download");
    dl.href = url;
    dl.download = name;
    acts.append(dl);
    head.append(acts);
    previewEl.append(head);

    const ext = fileExt(name);
    if (isImageExt(ext)) {
      const wrap = el("div", "bp-files-img-wrap");
      const img = document.createElement("img");
      img.className = "bp-files-img";
      img.alt = name;
      img.src = url;
      wrap.append(img);
      previewEl.append(wrap);
      return;
    }

    if (size > 512 * 1024 && !isTextExt(ext)) {
      previewEl.append(el("div", "bp-pane-empty", `Binary file (${formatSize(size)}). Use Download to save.`));
      return;
    }

    if (isTextExt(ext) || size <= 64 * 1024) {
      const loading = el("div", "bp-pane-empty", "Loading…");
      previewEl.append(loading);
      try {
        const res = await fetch(url, { credentials: "same-origin" });
        if (!alive || selectedPath !== relPath) return;
        if (!res.ok) {
          loading.textContent = `Could not load file (${res.status})`;
          return;
        }
        const ct = res.headers.get("content-type") || "";
        if (ct.startsWith("image/")) {
          loading.remove();
          const img = document.createElement("img");
          img.className = "bp-files-img";
          img.alt = name;
          img.src = url;
          previewEl.append(img);
          return;
        }
        // Bounded streaming read: cancel the body stream as soon as MAX_PREVIEW_BYTES is reached.
        const MAX_PREVIEW_BYTES = 256 * 1024;
        const chunks: Uint8Array[] = [];
        let total = 0;
        let truncated = false;
        if (res.body) {
          const reader = res.body.getReader();
          try {
            while (true) {
              const { done, value } = await reader.read();
              if (done || !value) break;
              if (total + value.byteLength > MAX_PREVIEW_BYTES) {
                const remain = MAX_PREVIEW_BYTES - total;
                if (remain > 0) {
                  chunks.push(value.subarray(0, remain));
                  total += remain;
                }
                truncated = true;
                await reader.cancel();
                break;
              }
              chunks.push(value);
              total += value.byteLength;
            }
          } finally {
            reader.releaseLock();
          }
        }
        if (!alive || selectedPath !== relPath) return;
        const slice = new Uint8Array(total);
        let offset = 0;
        for (const c of chunks) {
          slice.set(c, offset);
          offset += c.byteLength;
        }
        // Detect NUL bytes -> binary
        let binary = false;
        for (let i = 0; i < Math.min(slice.length, 4096); i++) {
          if (slice[i] === 0) { binary = true; break; }
        }
        loading.remove();
        if (binary) {
          previewEl.append(el("div", "bp-pane-empty", `Binary file (${formatSize(size || total)}). Use Download to save.`));
          return;
        }
        const text = new TextDecoder("utf-8", { fatal: false }).decode(slice);
        const pre = el("pre", "bp-files-code", text + (truncated ? "\n… (truncated at 256 KB)" : ""));
        previewEl.append(pre);
      } catch (err) {
        if (!alive || selectedPath !== relPath) return;
        loading.textContent = err instanceof Error ? err.message : String(err);
      }
      return;
    }

    previewEl.append(el("div", "bp-pane-empty", `${formatSize(size)} — use Download to save.`));
  }

  async function loadDir(dir: string): Promise<void> {
    currentDir = dir;
    renderCrumbs();
    listEl.replaceChildren(el("div", "bp-pane-empty", "Loading…"));
    try {
      const res = await api.hub.rpc<{ dir?: string; entries?: FileEntry[] }>("files.entries", {
        thread: threadId,
        ...(dir ? { path: dir } : {}),
      });
      if (!alive) return;
      const entries = Array.isArray(res?.entries) ? res.entries.slice() : [];
      entries.sort((a, b) => (a.dir === b.dir ? a.name.localeCompare(b.name) : a.dir ? -1 : 1));
      listEl.replaceChildren();

      if (dir) {
        const up = el("button", "bp-file-row is-dir");
        up.type = "button";
        up.append(el("span", "bp-file-icon", "↰"), el("span", "bp-file-name", ".."));
        const parent = dir.includes("/") ? dir.slice(0, dir.lastIndexOf("/")) : "";
        up.addEventListener("click", () => void loadDir(parent));
        listEl.append(up);
      }

      if (entries.length === 0) {
        listEl.append(el("div", "bp-pane-empty", "Empty folder"));
        return;
      }

      for (const entry of entries) {
        const rel = dir ? `${dir}/${entry.name}` : entry.name;
        const row = el("button", "bp-file-row" + (entry.dir ? " is-dir" : "") + (rel === selectedPath ? " is-sel" : ""));
        row.type = "button";
        row.setAttribute("data-testid", `file-row-${entry.name}`);
        row.append(
          el("span", "bp-file-icon", entry.dir ? "▸" : "·"),
          el("span", "bp-file-name", entry.name),
          el("span", "bp-file-size", entry.dir ? "" : formatSize(entry.size))
        );
        row.addEventListener("click", () => {
          if (entry.dir) {
            void loadDir(rel);
          } else {
            listEl.querySelectorAll(".bp-file-row.is-sel").forEach((n) => n.classList.remove("is-sel"));
            row.classList.add("is-sel");
            void openPreview(rel, entry.name, entry.size);
          }
        });
        listEl.append(row);
      }
    } catch (err) {
      if (!alive) return;
      listEl.replaceChildren(el("div", "bp-pane-empty", err instanceof Error ? err.message : String(err)));
    }
  }

  void loadDir("");
  return {
    destroy() {
      alive = false;
    },
  };
}

// ============================================================================
// 2. Diff Pane
// ============================================================================

export function mountDiffPane(
  stage: HTMLElement,
  api: ViewerHostApi,
  threadId: string
): PaneHandle {
  let alive = true;

  const root = el("div", "bp-pane bp-diff-pane");
  root.setAttribute("data-testid", "pane-diff");

  const bar = el("div", "bp-pane-bar");
  const summaryEl = el("span", "bp-diff-summary", "Loading diff…");
  const refreshBtn = el("button", "bp-v-btn", "Refresh");
  refreshBtn.type = "button";
  refreshBtn.addEventListener("click", () => void load());
  bar.append(summaryEl, refreshBtn);

  const body = el("div", "bp-diff-body");
  body.setAttribute("data-testid", "diff-body");
  root.append(bar, body);
  stage.replaceChildren(root);

  async function load(): Promise<void> {
    summaryEl.textContent = "Loading diff…";
    body.replaceChildren();
    try {
      const res = await api.hub.rpc<{ diff?: string; nogit?: boolean }>("viewer.diff", { thread: threadId });
      if (!alive) return;
      if (res?.nogit) {
        summaryEl.textContent = "Not a git repository";
        body.append(el("div", "bp-pane-empty", "This project is not a git repository."));
        return;
      }
      const raw = typeof res?.diff === "string" ? res.diff : "";
      if (!raw.trim()) {
        summaryEl.textContent = "Clean working tree";
        body.append(el("div", "bp-pane-empty", "No uncommitted changes."));
        return;
      }

      const lines = raw.split("\n");
      let files = 0;
      let added = 0;
      let removed = 0;
      const frag = document.createDocumentFragment();
      let currentBlock = el("div", "bp-diff-file");

      for (const line of lines) {
        if (line.startsWith("diff --git ")) {
          if (currentBlock.childNodes.length > 0) frag.append(currentBlock);
          currentBlock = el("div", "bp-diff-file");
          files++;
          const m = line.match(/ b\/(.+)$/);
          currentBlock.append(el("div", "bp-diff-file-head", m ? m[1] : line));
          continue;
        }
        if (line.startsWith("index ") || line.startsWith("--- ") || line.startsWith("+++ ")) {
          currentBlock.append(el("div", "bp-diff-line is-meta", line));
        } else if (line.startsWith("@@")) {
          currentBlock.append(el("div", "bp-diff-line is-hunk", line));
        } else if (line.startsWith("+")) {
          added++;
          currentBlock.append(el("div", "bp-diff-line is-add", line));
        } else if (line.startsWith("-")) {
          removed++;
          currentBlock.append(el("div", "bp-diff-line is-del", line));
        } else {
          currentBlock.append(el("div", "bp-diff-line", line));
        }
      }
      if (currentBlock.childNodes.length > 0) frag.append(currentBlock);

      summaryEl.textContent = `${files || 1} file${files === 1 ? "" : "s"} · +${added} -${removed}`;
      body.append(frag);
    } catch (err) {
      if (!alive) return;
      summaryEl.textContent = "Diff unavailable";
      body.append(el("div", "bp-pane-empty", err instanceof Error ? err.message : String(err)));
    }
  }

  void load();
  return {
    destroy() {
      alive = false;
    },
  };
}

// ============================================================================
// 3. Review Pane (Checks / BOM / Comments)
// ============================================================================

interface ReviewTask {
  text?: string;
  label?: string;
  done?: boolean;
  checked?: boolean;
  index?: number;
}

interface ReviewReport {
  md?: string;
  err?: string;
  gate?: string;
  line?: string;
  tasks?: ReviewTask[];
}

interface ReviewComment {
  id: string;
  text: string;
  by?: string;
  at?: number;
  resolved?: boolean;
  file?: string;
  layer?: string;
  item?: string;
}

export function mountReviewPane(
  stage: HTMLElement,
  api: ViewerHostApi,
  threadId: string
): PaneHandle {
  let alive = true;
  let tab: "checks" | "bom" | "comments" = "checks";

  const root = el("div", "bp-pane bp-review-pane");
  root.setAttribute("data-testid", "pane-review");

  const bar = el("div", "bp-pane-bar");
  const tabsEl = el("div", "bp-review-tabs");
  const checksBtn = el("button", "bp-v-btn is-on", "Checks");
  const bomBtn = el("button", "bp-v-btn", "BOM");
  const commentsBtn = el("button", "bp-v-btn", "Comments");
  checksBtn.type = bomBtn.type = commentsBtn.type = "button";
  tabsEl.append(checksBtn, bomBtn, commentsBtn);

  const refreshBtn = el("button", "bp-v-btn", "Refresh");
  refreshBtn.type = "button";
  refreshBtn.addEventListener("click", () => void load());
  bar.append(tabsEl, refreshBtn);

  const body = el("div", "bp-review-body");
  body.setAttribute("data-testid", "review-body");
  root.append(bar, body);
  stage.replaceChildren(root);

  function setTab(next: "checks" | "bom" | "comments"): void {
    tab = next;
    checksBtn.classList.toggle("is-on", tab === "checks");
    bomBtn.classList.toggle("is-on", tab === "bom");
    commentsBtn.classList.toggle("is-on", tab === "comments");
    void load();
  }
  checksBtn.addEventListener("click", () => setTab("checks"));
  bomBtn.addEventListener("click", () => setTab("bom"));
  commentsBtn.addEventListener("click", () => setTab("comments"));

  function renderMarkdownSafe(md: string): HTMLElement {
    const wrap = el("div", "bp-review-md");
    const lines = md.split("\n");
    let i = 0;
    while (i < lines.length) {
      const line = lines[i];
      if (line.startsWith("### ")) {
        wrap.append(el("h4", "bp-review-h", line.slice(4)));
        i++;
      } else if (line.startsWith("## ")) {
        wrap.append(el("h3", "bp-review-h", line.slice(3)));
        i++;
      } else if (line.startsWith("# ")) {
        wrap.append(el("h2", "bp-review-h", line.slice(2)));
        i++;
      } else if (line.startsWith("|") && i + 1 < lines.length && lines[i + 1].includes("---")) {
        const table = el("table", "bp-review-table");
        const thead = document.createElement("thead");
        const htr = document.createElement("tr");
        for (const cell of line.split("|").slice(1, -1)) {
          htr.append(el("th", undefined, cell.trim()));
        }
        thead.append(htr);
        table.append(thead);
        const tbody = document.createElement("tbody");
        i += 2;
        while (i < lines.length && lines[i].startsWith("|")) {
          const tr = document.createElement("tr");
          for (const cell of lines[i].split("|").slice(1, -1)) {
            tr.append(el("td", undefined, cell.trim()));
          }
          tbody.append(tr);
          i++;
        }
        table.append(tbody);
        wrap.append(table);
      } else if (line.trim() === "") {
        i++;
      } else {
        wrap.append(el("p", "bp-review-p", line));
        i++;
      }
    }
    return wrap;
  }

  async function load(): Promise<void> {
    body.replaceChildren(el("div", "bp-pane-empty", "Loading…"));
    try {
      if (tab === "comments") {
        const res = await api.hub.rpc<{ comments?: ReviewComment[] }>("review.list", { thread: threadId });
        if (!alive) return;
        const comments = Array.isArray(res?.comments) ? res.comments : [];
        body.replaceChildren();

        const form = el("form", "bp-review-form");
        const input = el("input", "bp-review-input");
        input.type = "text";
        input.placeholder = "Add a review note…";
        const submit = el("button", "bp-v-btn is-primary", "Add");
        submit.type = "submit";
        form.append(input, submit);
        form.addEventListener("submit", (ev) => {
          ev.preventDefault();
          const text = input.value.trim();
          if (!text) return;
          submit.disabled = true;
          void api.hub.rpc("review.comment", { thread: threadId, text }).then(
            () => { input.value = ""; submit.disabled = false; void load(); },
            () => { submit.disabled = false; }
          );
        });
        body.append(form);

        if (comments.length === 0) {
          body.append(el("div", "bp-pane-empty", "No review comments yet."));
          return;
        }
        for (const c of comments) {
          const card = el("div", "bp-review-comment" + (c.resolved ? " is-resolved" : ""));
          const meta = [c.by || "review", c.file, c.layer, c.item].filter(Boolean).join(" · ");
          card.append(el("div", "bp-review-comment-meta", meta), el("div", "bp-review-comment-text", c.text));
          body.append(card);
        }
        return;
      }

      const res = await api.hub.rpc<{ report?: ReviewReport }>("review.page", { thread: threadId, page: tab });
      if (!alive) return;
      const rep = res?.report || {};
      body.replaceChildren();

      if (rep.line || rep.gate) {
        const banner = el("div", "bp-review-gate", [rep.gate, rep.line].filter(Boolean).join(" · "));
        body.append(banner);
      }
      if (rep.err) {
        body.append(el("div", "bp-review-err", rep.err));
      }
      if (Array.isArray(rep.tasks) && rep.tasks.length > 0) {
        const list = el("div", "bp-review-tasks");
        rep.tasks.forEach((task, idx) => {
          const row = el("label", "bp-review-task");
          const chk = document.createElement("input");
          chk.type = "checkbox";
          chk.checked = !!(task.done ?? task.checked);
          const taskIndex = typeof task.index === "number" ? task.index : idx;
          chk.addEventListener("change", () => {
            void api.hub.rpc("review.tick", { thread: threadId, index: taskIndex }).then(() => void load(), () => {});
          });
          row.append(chk, el("span", undefined, task.text || task.label || `Item ${idx + 1}`));
          list.append(row);
        });
        body.append(list);
      }
      if (rep.md && rep.md.trim()) {
        body.append(renderMarkdownSafe(rep.md));
      } else if (!rep.err && (!rep.tasks || rep.tasks.length === 0)) {
        body.append(el("div", "bp-pane-empty", `No ${tab === "bom" ? "BOM" : "design checks"} report available.`));
      }
    } catch (err) {
      if (!alive) return;
      body.replaceChildren(el("div", "bp-pane-empty", err instanceof Error ? err.message : String(err)));
    }
  }

  void load();
  return {
    destroy() {
      alive = false;
    },
  };
}

