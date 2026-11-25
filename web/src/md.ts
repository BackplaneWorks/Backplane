// Safe, zero-dependency Markdown -> DOM renderer (never assigns untrusted innerHTML).
// Supports fenced code blocks (with copy button), headings (h1-h6), hr, tables,
// nested & task lists, blockquotes, inline code, bold, italic, strikethrough,
// inline images, and safe http(s) links.
import { t } from "./i18n";

const SVG_NS = "http://www.w3.org/2000/svg";
const COPY_PATHS = ["M9 9h11v11H9z", "M5 15H4a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h10a1 1 0 0 1 1 1v1"];
const CHECK_PATHS = ["M20 6L9 17l-5-5"];

function makeTinySvg(paths: string[]): SVGSVGElement {
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("width", "11");
  svg.setAttribute("height", "11");
  svg.setAttribute("fill", "none");
  svg.setAttribute("stroke", "currentColor");
  svg.setAttribute("stroke-width", "2");
  svg.setAttribute("aria-hidden", "true");
  for (const d of paths) {
    const p = document.createElementNS(SVG_NS, "path");
    p.setAttribute("d", d);
    svg.appendChild(p);
  }
  return svg;
}

export function exactCopyText(text: string): string {
  return text.replace(/[\r\n]+$/, "");
}

function triggerInlineCopy(wrap: HTMLElement, btn: HTMLButtonElement, rawText: string): void {
  const exact = exactCopyText(rawText);
  const clip = typeof navigator !== "undefined" ? navigator.clipboard : undefined;
  if (!clip?.writeText) return;
  clip.writeText(exact).then(() => {
    wrap.classList.add("is-copied");
    btn.classList.add("is-copied");
    btn.replaceChildren(makeTinySvg(CHECK_PATHS));
    const copiedLbl = t("web.clone.copied", "Copied");
    btn.setAttribute("aria-label", copiedLbl);
    btn.title = copiedLbl;
    const setTimer = typeof window !== "undefined" ? window.setTimeout.bind(window) : setTimeout;
    setTimer(() => {
      wrap.classList.remove("is-copied");
      btn.classList.remove("is-copied");
      btn.replaceChildren(makeTinySvg(COPY_PATHS));
      const copyLbl = t("common.copy", "Copy");
      btn.setAttribute("aria-label", copyLbl);
      btn.title = copyLbl;
    }, 1200);
  }).catch(() => {});
}

function makeInlineCopyButton(wrap: HTMLElement, rawText: string): HTMLButtonElement {
  const btn = document.createElement("button");
  btn.type = "button";
  btn.className = "bp-inline-copy";
  btn.setAttribute("data-bp-no-copy", "1");
  const copyLbl = t("common.copy", "Copy");
  btn.setAttribute("aria-label", copyLbl);
  btn.title = copyLbl;
  btn.appendChild(makeTinySvg(COPY_PATHS));
  btn.addEventListener("mousedown", (e) => {
    e.preventDefault();
    e.stopPropagation();
  });
  btn.addEventListener("click", (e) => {
    e.stopPropagation();
    triggerInlineCopy(wrap, btn, rawText);
  });
  return btn;
}

function appendPlainWithPaths(parent: HTMLElement, text: string): void {
  const pathRe = /(^|[\s(\["'<])((?:\.\.?\/|~\/|\/(?=[A-Za-z0-9._-]))[A-Za-z0-9._~+/-]+|[A-Za-z_][A-Za-z0-9._-]+(?:\/[A-Za-z0-9._-]+)+\.[A-Za-z][A-Za-z0-9]{0,9})/g;
  let last = 0;
  let m: RegExpExecArray | null;
  while ((m = pathRe.exec(text)) !== null) {
    const lead = m[1] ?? "";
    const raw = m[2] ?? "";
    const start = m.index + lead.length;
    if (start > last) {
      parent.appendChild(document.createTextNode(text.slice(last, start)));
    }
    let path = raw;
    while (path.length > 0 && /[.,;:!?)\]]/.test(path[path.length - 1])) {
      path = path.slice(0, -1);
    }
    if (!path || !/[A-Za-z0-9]/.test(path) || path.includes("://")) {
      parent.appendChild(document.createTextNode(raw));
      last = start + raw.length;
      continue;
    }
    const wrap = document.createElement("span");
    wrap.className = "bp-path-wrap";
    const span = document.createElement("span");
    span.className = "bp-md-path";
    span.textContent = path;
    const btn = makeInlineCopyButton(wrap, path);
    wrap.append(span, btn);
    parent.appendChild(wrap);
    if (path.length < raw.length) {
      parent.appendChild(document.createTextNode(raw.slice(path.length)));
    }
    last = start + raw.length;
  }
  if (last < text.length) {
    parent.appendChild(document.createTextNode(text.slice(last)));
  }
}

function safeUrl(raw: string): string | null {
  try {
    const u = new URL(raw, window.location.origin);
    if (u.protocol === "https:" || u.protocol === "http:") return u.href;
  } catch {
    // ignore invalid URLs
  }
  return null;
}

function appendStyledText(parent: HTMLElement, text: string): void {
  const re = /(\*\*(?!\s)[^*\n]+?(?<!\s)\*\*|(?<![A-Za-z0-9_])__(?!\s)[^_\n]+?(?<!\s)__(?![A-Za-z0-9_])|~~(?!\s)[^~\n]+?(?<!\s)~~|\*(?!\s)[^*\n]+?(?<!\s)\*|(?<![A-Za-z0-9_])_(?!\s)[^_\n]+?(?<!\s)_(?![A-Za-z0-9_]))/g;
  let last = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text)) !== null) {
    if (m.index > last) appendPlainWithPaths(parent, text.slice(last, m.index));
    const tok = m[0];
    if (tok.startsWith("__") && tok.endsWith("__") && /^[A-Za-z0-9]+$/.test(tok.slice(2, -2))) {
      appendPlainWithPaths(parent, tok);
    } else if ((tok.startsWith("**") && tok.endsWith("**")) || (tok.startsWith("__") && tok.endsWith("__"))) {
      const strong = document.createElement("strong");
      appendInline(strong, tok.slice(2, -2));
      parent.appendChild(strong);
    } else if (tok.startsWith("~~") && tok.endsWith("~~")) {
      const del = document.createElement("del");
      appendInline(del, tok.slice(2, -2));
      parent.appendChild(del);
    } else {
      const em = document.createElement("em");
      appendInline(em, tok.slice(1, -1));
      parent.appendChild(em);
    }
    last = re.lastIndex;
  }
  if (last < text.length) appendPlainWithPaths(parent, text.slice(last));
}

function appendInline(parent: HTMLElement, text: string): void {
  const re = /(`[^`]+`|!\[([^\]]*)\]\(([^)]+)\)|\[([^\]]+)\]\(([^)]+)\))/g;
  let last = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text)) !== null) {
    if (m.index > last) {
      appendStyledText(parent, text.slice(last, m.index));
    }
    const tok = m[0];
    if (tok.startsWith("`") && tok.endsWith("`")) {
      const codeText = tok.slice(1, -1);
      const wrap = document.createElement("span");
      wrap.className = "bp-code-wrap";
      const code = document.createElement("code");
      code.className = "bp-code-inline";
      code.textContent = codeText;
      const btn = makeInlineCopyButton(wrap, codeText);
      let clickTimer: ReturnType<typeof setTimeout> | undefined;
      const clearTimer = typeof window !== "undefined" ? window.clearTimeout.bind(window) : clearTimeout;
      const setTimer = typeof window !== "undefined" ? window.setTimeout.bind(window) : setTimeout;
      code.addEventListener("dblclick", () => {
        if (clickTimer !== undefined) {
          clearTimer(clickTimer);
          clickTimer = undefined;
        }
      });
      code.addEventListener("click", (e) => {
        if (e.detail && e.detail !== 1) return;
        if (typeof window !== "undefined" && window.getSelection()?.toString()) return;
        if (clickTimer !== undefined) clearTimer(clickTimer);
        clickTimer = setTimer(() => {
          clickTimer = undefined;
          if (typeof window !== "undefined" && window.getSelection()?.toString()) return;
          triggerInlineCopy(wrap, btn, codeText);
        }, 220);
      });
      wrap.append(code, btn);
      parent.appendChild(wrap);
    } else if (tok.startsWith("![")) {
      const alt = m[2] ?? "";
      const src = m[3] ?? "";
      if (/^\/blob\/[A-Za-z0-9._-]+$/.test(src) && !src.includes("..")) {
        const img = document.createElement("img");
        img.className = "bp-md-img";
        img.alt = alt || src;
        img.src = src;
        parent.appendChild(img);
      } else {
        const span = document.createElement("code");
        span.className = "bp-code-inline";
        span.textContent = alt ? `${alt} (${src})` : src;
        parent.appendChild(span);
      }
    } else if (tok.startsWith("[")) {
      const label = m[4] ?? "";
      const rawUrl = m[5] ?? "";
      const isHttp = /^https?:\/\//i.test(rawUrl);
      const href = isHttp ? safeUrl(rawUrl) : null;
      if (href) {
        const a = document.createElement("a");
        a.href = href;
        a.target = "_blank";
        a.rel = "noopener noreferrer";
        a.textContent = label;
        parent.appendChild(a);
      } else {
        const code = document.createElement("code");
        code.className = "bp-code-inline";
        code.textContent = label || rawUrl;
        parent.appendChild(code);
      }
    }
    last = re.lastIndex;
  }
  if (last < text.length) {
    appendStyledText(parent, text.slice(last));
  }
}

function splitTableRow(line: string): string[] {
  let s = line.trim();
  if (s.startsWith("|")) s = s.slice(1);
  if (s.endsWith("|")) s = s.slice(0, -1);
  return s.split("|").map((c) => c.trim());
}

function parseTableAlign(line: string): Array<"left" | "center" | "right" | ""> | null {
  const cells = splitTableRow(line);
  if (cells.length === 0) return null;
  const aligns: Array<"left" | "center" | "right" | ""> = [];
  for (const c of cells) {
    if (!/^:?-{3,}:?$/.test(c)) return null;
    if (c.startsWith(":") && c.endsWith(":")) aligns.push("center");
    else if (c.endsWith(":")) aligns.push("right");
    else if (c.startsWith(":")) aligns.push("left");
    else aligns.push("");
  }
  return aligns;
}

const LIST_ITEM_RE = /^(\s*)([-*+]|\d+[.)])\s+(.*)$/;
const HR_RE = /^\s{0,3}([-*_])(?:\s*\1){2,}\s*$/;

function isBlockStarter(lines: string[], idx: number): boolean {
  const line = lines[idx];
  const t0 = line.trimStart();
  if (!line.trim()) return true;
  if (t0.startsWith("```")) return true;
  if (/^#{1,6}\s+/.test(line)) return true;
  if (HR_RE.test(line)) return true;
  if (line.startsWith("> ") || line === ">") return true;
  if (LIST_ITEM_RE.test(line)) return true;
  if (line.includes("|") && idx + 1 < lines.length && parseTableAlign(lines[idx + 1]) !== null) return true;
  return false;
}

function parseList(lines: string[], start: number, minIndent: number): [HTMLUListElement | HTMLOListElement, number] {
  const first = LIST_ITEM_RE.exec(lines[start])!;
  const ordered = /\d/.test(first[2]);
  const list = document.createElement(ordered ? "ol" : "ul");
  list.className = "bp-md-list";
  let i = start;

  while (i < lines.length) {
    const m = LIST_ITEM_RE.exec(lines[i]);
    if (!m) break;
    const indent = m[1].length;
    if (indent < minIndent) break;
    if (indent > minIndent) {
      const [sub, next] = parseList(lines, i, indent);
      const lastLi = list.lastElementChild;
      if (lastLi) lastLi.appendChild(sub);
      else list.appendChild(sub);
      i = next;
      continue;
    }
    const isOrd = /\d/.test(m[2]);
    if (isOrd !== ordered) break;

    const li = document.createElement("li");
    const task = /^\[( |x|X)\]\s+(.*)$/.exec(m[3]);
    if (task) {
      li.className = "bp-md-task";
      const box = document.createElement("span");
      box.className = "bp-md-check" + (task[1].toLowerCase() === "x" ? " checked" : "");
      box.textContent = task[1].toLowerCase() === "x" ? "✓" : " ";
      li.appendChild(box);
      appendInline(li, task[2]);
    } else {
      appendInline(li, m[3]);
    }
    list.appendChild(li);
    i++;
  }
  return [list, i];
}

export function renderMarkdown(src: string, depth = 0): DocumentFragment {
  const frag = document.createDocumentFragment();
  const lines = src.replace(/\r\n/g, "\n").split("\n");
  let i = 0;

  while (i < lines.length) {
    const line = lines[i];

    // Fenced code block
    if (line.trimStart().startsWith("```")) {
      const lang = line.trimStart().slice(3).trim();
      const buf: string[] = [];
      i++;
      while (i < lines.length && !lines[i].trimStart().startsWith("```")) {
        buf.push(lines[i]);
        i++;
      }
      if (i < lines.length) i++;
      const codeText = buf.join("\n");

      const wrap = document.createElement("div");
      wrap.className = "bp-code-block";

      const head = document.createElement("div");
      head.className = "bp-code-head";
      const langEl = document.createElement("span");
      langEl.className = "bp-code-lang";
      langEl.textContent = lang || "code";
      const copyBtn = document.createElement("button");
      copyBtn.type = "button";
      copyBtn.className = "bp-btn-ghost bp-code-copy";
      copyBtn.setAttribute("data-bp-no-copy", "1");
      const copyLabel = t("common.copy", "Copy");
      copyBtn.textContent = copyLabel;
      copyBtn.addEventListener("click", () => {
        navigator.clipboard?.writeText(exactCopyText(codeText)).then(() => {
          copyBtn.textContent = "✓";
          window.setTimeout(() => { copyBtn.textContent = copyLabel; }, 1200);
        }).catch(() => {});
      });
      head.append(langEl, copyBtn);

      const pre = document.createElement("pre");
      const code = document.createElement("code");
      code.textContent = codeText;
      pre.appendChild(code);

      wrap.append(head, pre);
      frag.appendChild(wrap);
      continue;
    }

    // Blank line
    if (!line.trim()) {
      i++;
      continue;
    }

    // Horizontal rule
    if (HR_RE.test(line)) {
      const hr = document.createElement("hr");
      hr.className = "bp-md-hr";
      frag.appendChild(hr);
      i++;
      continue;
    }

    // Heading
    const hm = /^(#{1,6})\s+(.+)$/.exec(line);
    if (hm) {
      const lvl = Math.min(6, hm[1].length + 1);
      const h = document.createElement(`h${lvl}`);
      h.className = `bp-md-h bp-md-h${hm[1].length}`;
      appendInline(h, hm[2]);
      frag.appendChild(h);
      i++;
      continue;
    }

    // Table
    if (line.includes("|") && i + 1 < lines.length) {
      const aligns = parseTableAlign(lines[i + 1]);
      if (aligns !== null) {
        const wrap = document.createElement("div");
        wrap.className = "bp-md-table-wrap";
        const table = document.createElement("table");
        table.className = "bp-md-table";
        const thead = document.createElement("thead");
        const htr = document.createElement("tr");
        const headers = splitTableRow(line);
        for (let c = 0; c < aligns.length; c++) {
          const th = document.createElement("th");
          if (aligns[c]) th.style.textAlign = aligns[c];
          appendInline(th, headers[c] ?? "");
          htr.appendChild(th);
        }
        thead.appendChild(htr);
        table.appendChild(thead);

        const tbody = document.createElement("tbody");
        i += 2;
        while (i < lines.length && lines[i].trim() !== "" && lines[i].includes("|")) {
          const cells = splitTableRow(lines[i]);
          const tr = document.createElement("tr");
          for (let c = 0; c < aligns.length; c++) {
            const td = document.createElement("td");
            if (aligns[c]) td.style.textAlign = aligns[c];
            appendInline(td, cells[c] ?? "");
            tr.appendChild(td);
          }
          tbody.appendChild(tr);
          i++;
        }
        table.appendChild(tbody);
        wrap.appendChild(table);
        frag.appendChild(wrap);
        continue;
      }
    }

    // Blockquote
    if (line.startsWith("> ") || line === ">") {
      const bq = document.createElement("blockquote");
      bq.className = "bp-md-quote";
      const qLines: string[] = [];
      while (i < lines.length && (lines[i].startsWith("> ") || lines[i] === ">")) {
        qLines.push(lines[i] === ">" ? "" : lines[i].slice(2));
        i++;
      }
      if (depth >= 12) {
        appendInline(bq, qLines.join("\n"));
      } else {
        bq.appendChild(renderMarkdown(qLines.join("\n"), depth + 1));
      }
      frag.appendChild(bq);
      continue;
    }

    // List (ordered, unordered, nested, or task)
    const lm = LIST_ITEM_RE.exec(line);
    if (lm) {
      const [listEl, next] = parseList(lines, i, lm[1].length);
      frag.appendChild(listEl);
      i = next;
      continue;
    }

    // Paragraph (join consecutive non-special lines)
    const pLines: string[] = [];
    while (i < lines.length && !isBlockStarter(lines, i)) {
      pLines.push(lines[i]);
      i++;
    }
    if (pLines.length === 0) {
      // Defensive: advance on unconsumed line
      pLines.push(lines[i++]);
    }
    const p = document.createElement("p");
    p.className = "bp-md-p";
    for (let j = 0; j < pLines.length; j++) {
      if (j > 0) p.appendChild(document.createElement("br"));
      appendInline(p, pLines[j]);
    }
    frag.appendChild(p);
  }

  return frag;
}
