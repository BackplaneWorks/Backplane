// Safe, zero-dependency Markdown -> DOM renderer (never assigns untrusted innerHTML).
// Supports fenced code blocks (with copy button), headings (h1-h6), hr, tables,
// nested & task lists, blockquotes, inline code, bold, italic, strikethrough,
// inline images, safe http(s) links, /s/ shares, file:// and project file links.
import { t } from "./i18n";

export interface MarkdownFileLink {
  path: string;
  name?: string;
  size?: number;
  open?: string;
  project?: string;
  absPath?: string;
  relPath?: string;
}

export interface RenderMarkdownOptions {
  onOpenLink?: (href: string, project?: string) => void;
  files?: MarkdownFileLink[];
}

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

const SHARE_ROUTE_RE = /^\/s\/[0-9a-fA-F]{48}\/(?:[A-Za-z0-9._~-]|%[0-9a-fA-F]{2})+$/;

export function parseShareRoute(raw: string): string | null {
  const clean = raw.trim().split("#")[0].split("?")[0];
  if (!SHARE_ROUTE_RE.test(clean)) return null;
  const last = clean.slice(clean.lastIndexOf("/") + 1);
  if (!last || last === "." || last === "..") return null;
  try {
    const dec = decodeURIComponent(last);
    if (!dec || dec === "." || dec === ".." || dec.includes("/") || dec.includes("\\") || dec.includes("\0")) return null;
  } catch {
    return null;
  }
  return clean;
}

export function parseFileUrl(raw: string): string | null {
  const t = raw.trim();
  if (!/^file:\/\//i.test(t)) return null;
  let rest = t.slice(7);
  if (rest.startsWith("localhost/")) rest = rest.slice("localhost".length);
  let anchor = "";
  const hash = rest.indexOf("#");
  if (hash >= 0) {
    const frag = rest.slice(hash + 1);
    rest = rest.slice(0, hash);
    if (/^L\d{1,6}(?:-L?\d{1,6})?$/.test(frag)) anchor = `#${frag}`;
  }
  const q = rest.indexOf("?");
  if (q >= 0) rest = rest.slice(0, q);
  let decoded = rest;
  try { decoded = decodeURIComponent(rest); } catch { /* keep literal on invalid escape */ }
  if (/^\/[A-Za-z]:\//.test(decoded)) decoded = decoded.slice(1);
  if (!decoded || decoded.includes("\0")) return null;
  if (!(decoded.startsWith("/") || decoded.startsWith("~/") || /^[A-Za-z]:\//.test(decoded))) return null;
  return decoded + anchor;
}

export function isHubPath(raw: string): boolean {
  const t = raw.trim();
  if (!t || t.includes("://")) return false;
  if (parseShareRoute(t) !== null) return true;
  if ((t.startsWith("/") && !t.startsWith("//")) || t.startsWith("~/") || t.startsWith("./") || t.startsWith("../")) {
    return !t.includes("\0") && !/\s/.test(t);
  }
  return /^[A-Za-z0-9_@+-][A-Za-z0-9_.@+-]*(?:\/[A-Za-z0-9_.@+-]+)+(?::\d{1,6}){0,2}(?:#L\d{1,6}(?:-L?\d{1,6})?)?$/.test(t)
    || /^[A-Za-z0-9_@+-][A-Za-z0-9_.@+-]*\.(?:md|markdown|txt|pdf|json|yaml|yml|toml|rs|ts|tsx|js|jsx|py|c|h|cc|cpp|hpp|swift|kt|java|go|sh|css|html|xml|csv|svg|png|jpg|jpeg|webp|kicad_pcb|kicad_sch|kicad_pro|kicad_dru|kicad_sym|kicad_mod|pcbdoc|schdoc|prjpcb|step|stp|sc|solidcraft|iges|igs|stl|3mf|obj|glb|gbr|gbrjob|drl|xln)(?::\d{1,6}){0,2}(?:#L\d{1,6}(?:-L?\d{1,6})?)?$/i.test(t);
}

function normalizeLinkDest(raw: string): string {
  const t = raw.trim();
  if (t.startsWith("<") && t.indexOf(">", 1) > 0) {
    return t.slice(1, t.indexOf(">", 1)).trim();
  }
  const sp = t.search(/\s/);
  return (sp >= 0 ? t.slice(0, sp) : t).replace(/^<|>$/g, "");
}

function normalizeHubTarget(dest: string): string {
  const fileP = parseFileUrl(dest);
  if (fileP) return fileP;
  if (dest.startsWith("backplane://")) return dest;
  let rest = dest;
  let anchor = "";
  const hash = rest.indexOf("#");
  if (hash >= 0) {
    const frag = rest.slice(hash + 1);
    rest = rest.slice(0, hash);
    if (/^L\d{1,6}(?:-L?\d{1,6})?$/.test(frag)) anchor = `#${frag}`;
  }
  const q = rest.indexOf("?");
  if (q >= 0) rest = rest.slice(0, q);
  try { rest = decodeURIComponent(rest); } catch { /* keep literal */ }
  return rest + anchor;
}

function extractLineAnchor(raw: string): string {
  const hashMatch = /#L\d{1,6}(?:-L?\d{1,6})?$/.exec(raw.trim());
  if (hashMatch) return hashMatch[0];
  const noFrag = raw.trim().split("#")[0];
  const colonMatch = /(?::\d{1,6}){1,2}$/.exec(noFrag);
  return colonMatch ? colonMatch[0] : "";
}

function withLineAnchor(base: string, original: string): string {
  const anchor = extractLineAnchor(original);
  if (!anchor || extractLineAnchor(base)) return base;
  return `${base}${anchor}`;
}

function barePath(raw: string): string {
  const s = (parseFileUrl(raw) ?? raw.trim()).split("#")[0].split("?")[0];
  return s.replace(/(?::\d{1,6}){1,2}$/, "");
}

function matchFileLink(target: string, files?: MarkdownFileLink[]): MarkdownFileLink | undefined {
  if (!files || files.length === 0) return undefined;
  const want = barePath(target);
  if (!want) return undefined;
  return files.find((f) => barePath(f.path) === want || (f.absPath ? barePath(f.absPath) === want : false));
}

function appendPlainWithPaths(parent: HTMLElement, text: string, opts?: RenderMarkdownOptions): void {
  const pathRe = /(^|[\s(\["'<])((?:file:\/\/\/?[^\s<>)"']+|\/s\/[0-9a-fA-F]{48}\/(?:[A-Za-z0-9._~-]|%[0-9a-fA-F]{2})+(?:[?#][^\s<>)"']*)?|(?:\.\.?\/|~\/|\/(?=[A-Za-z0-9._-]))[A-Za-z0-9._~+/-]+|[A-Za-z_][A-Za-z0-9._-]+(?:\/[A-Za-z0-9._-]+)+\.[A-Za-z][A-Za-z0-9]{0,9}))/g;
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
    const share = parseShareRoute(path);
    if (share) {
      const a = document.createElement("a");
      a.className = "bp-md-link bp-md-share";
      a.href = share;
      a.target = "_blank";
      a.rel = "noopener noreferrer";
      a.setAttribute("data-bp-share", share);
      a.textContent = path;
      if (opts?.onOpenLink) {
        a.addEventListener("click", (e) => {
          e.preventDefault();
          opts.onOpenLink?.(share);
        });
      }
      parent.appendChild(a);
      if (path.length < raw.length) {
        parent.appendChild(document.createTextNode(raw.slice(path.length)));
      }
      last = start + raw.length;
      continue;
    }
    const fileUrlTarget = parseFileUrl(path);
    if (!path || !/[A-Za-z0-9]/.test(path) || (path.includes("://") && !fileUrlTarget)) {
      parent.appendChild(document.createTextNode(raw));
      last = start + raw.length;
      continue;
    }
    const resolvedInput = fileUrlTarget ?? path;
    const matched = matchFileLink(resolvedInput, opts?.files);
    const wrap = document.createElement("span");
    wrap.className = "bp-path-wrap";
    const span = document.createElement("span");
    span.className = "bp-md-path";
    span.textContent = path;
    span.setAttribute("data-bp-link", resolvedInput);
    if (matched?.project) span.setAttribute("data-bp-project", matched.project);
    if (opts?.onOpenLink) {
      span.addEventListener("click", () => {
        if (typeof window !== "undefined" && window.getSelection()?.toString()) return;
        const resolved = matched?.absPath ? withLineAnchor(matched.absPath, resolvedInput) : resolvedInput;
        opts.onOpenLink?.(resolved, matched?.project);
      });
    }
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

function appendStyledText(parent: HTMLElement, text: string, opts?: RenderMarkdownOptions, inLink = false): void {
  const re = /(\*\*(?!\s)[^*\n]+?(?<!\s)\*\*|(?<![A-Za-z0-9_])__(?!\s)[^_\n]+?(?<!\s)__(?![A-Za-z0-9_])|~~(?!\s)[^~\n]+?(?<!\s)~~|\*(?!\s)[^*\n]+?(?<!\s)\*|(?<![A-Za-z0-9_])_(?!\s)[^_\n]+?(?<!\s)_(?![A-Za-z0-9_]))/g;
  let last = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text)) !== null) {
    if (m.index > last) {
      const pre = text.slice(last, m.index);
      if (inLink) parent.appendChild(document.createTextNode(pre));
      else appendPlainWithPaths(parent, pre, opts);
    }
    const tok = m[0];
    if (tok.startsWith("__") && tok.endsWith("__") && /^[A-Za-z0-9]+$/.test(tok.slice(2, -2))) {
      if (inLink) parent.appendChild(document.createTextNode(tok));
      else appendPlainWithPaths(parent, tok, opts);
    } else if ((tok.startsWith("**") && tok.endsWith("**")) || (tok.startsWith("__") && tok.endsWith("__"))) {
      const strong = document.createElement("strong");
      appendInline(strong, tok.slice(2, -2), opts, inLink);
      parent.appendChild(strong);
    } else if (tok.startsWith("~~") && tok.endsWith("~~")) {
      const del = document.createElement("del");
      appendInline(del, tok.slice(2, -2), opts, inLink);
      parent.appendChild(del);
    } else {
      const em = document.createElement("em");
      appendInline(em, tok.slice(1, -1), opts, inLink);
      parent.appendChild(em);
    }
    last = re.lastIndex;
  }
  if (last < text.length) {
    const tail = text.slice(last);
    if (inLink) parent.appendChild(document.createTextNode(tail));
    else appendPlainWithPaths(parent, tail, opts);
  }
}

function matchBracketLink(text: string, idx: number): { isImg: boolean; label: string; rawDest: string; end: number } | null {
  const isImg = text[idx] === "!";
  const st = isImg ? idx + 1 : idx;
  if (text[st] !== "[") return null;
  let d = 0;
  let j = st;
  let close = -1;
  while (j < text.length) {
    if (text[j] === "\\") { j += 2; continue; }
    if (text[j] === "[") d++;
    else if (text[j] === "]") { d--; if (d === 0) { close = j; break; } }
    j++;
  }
  if (close < 0 || close + 1 >= text.length || text[close + 1] !== "(") return null;
  let pd = 0;
  let k = close + 1;
  let end = -1;
  while (k < text.length) {
    if (text[k] === "\\") { k += 2; continue; }
    if (text[k] === "(") pd++;
    else if (text[k] === ")") { pd--; if (pd === 0) { end = k; break; } }
    k++;
  }
  if (end < 0) return null;
  return {
    isImg,
    label: text.slice(st + 1, close),
    rawDest: text.slice(close + 2, end),
    end: end + 1,
  };
}

function appendInline(parent: HTMLElement, text: string, opts?: RenderMarkdownOptions, inLink = false): void {
  let last = 0;
  let i = 0;
  const flushPlain = (end: number) => {
    if (end > last) appendStyledText(parent, text.slice(last, end), opts, inLink);
  };
  while (i < text.length) {
    const c = text[i];
    if (c === "\\") { i += 2; continue; }
    if (c === "`") {
      const e = text.indexOf("`", i + 1);
      if (e > i) {
        flushPlain(i);
        const codeText = text.slice(i + 1, e);
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
        code.addEventListener("click", (ev) => {
          if (ev.detail && ev.detail !== 1) return;
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
        i = e + 1;
        last = i;
        continue;
      }
    }
    if (!inLink && ((c === "!" && text[i + 1] === "[") || c === "[")) {
      const m = matchBracketLink(text, i);
      if (m) {
        flushPlain(i);
        if (m.isImg) {
          const src = normalizeLinkDest(m.rawDest);
          if (/^\/blob\/[A-Za-z0-9._-]+$/.test(src) && !src.includes("..")) {
            const img = document.createElement("img");
            img.className = "bp-md-img";
            img.alt = m.label || src;
            img.src = src;
            parent.appendChild(img);
          } else {
            const span = document.createElement("code");
            span.className = "bp-code-inline";
            span.textContent = m.label ? `${m.label} (${src})` : src;
            parent.appendChild(span);
          }
        } else {
          const dest = normalizeLinkDest(m.rawDest);
          const isHttp = /^https?:\/\//i.test(dest);
          const httpHref = isHttp ? safeUrl(dest) : null;
          const shareRoute = parseShareRoute(dest);
          const fileUrl = parseFileUrl(dest);
          const isBp = dest.startsWith("backplane://");
          const hubPath = !isHttp && !shareRoute && (fileUrl !== null || isBp || isHubPath(dest));
          if (httpHref) {
            const a = document.createElement("a");
            a.href = httpHref;
            a.target = "_blank";
            a.rel = "noopener noreferrer";
            appendInline(a, m.label, opts, true);
            parent.appendChild(a);
          } else if (shareRoute) {
            const a = document.createElement("a");
            a.className = "bp-md-link bp-md-share";
            a.href = shareRoute;
            a.target = "_blank";
            a.rel = "noopener noreferrer";
            a.setAttribute("data-bp-share", shareRoute);
            if (opts?.onOpenLink) {
              a.addEventListener("click", (ev) => {
                ev.preventDefault();
                opts.onOpenLink?.(shareRoute);
              });
            }
            appendInline(a, m.label || shareRoute, opts, true);
            parent.appendChild(a);
          } else if (hubPath) {
            const target = fileUrl ?? normalizeHubTarget(dest);
            const matched = matchFileLink(target, opts?.files);
            const a = document.createElement("a");
            a.className = "bp-md-link bp-md-file";
            a.href = "#";
            a.setAttribute("data-bp-link", target);
            a.title = target;
            if (matched?.project) a.setAttribute("data-bp-project", matched.project);
            a.addEventListener("click", (ev) => {
              ev.preventDefault();
              const resolved = matched?.absPath ? withLineAnchor(matched.absPath, target) : target;
              opts?.onOpenLink?.(resolved, matched?.project);
            });
            appendInline(a, m.label || target, opts, true);
            parent.appendChild(a);
          } else {
            const code = document.createElement("code");
            code.className = "bp-code-inline";
            code.textContent = m.label || m.rawDest;
            parent.appendChild(code);
          }
        }
        i = m.end;
        last = i;
        continue;
      }
    }
    i++;
  }
  flushPlain(text.length);
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

function parseList(lines: string[], start: number, minIndent: number, opts?: RenderMarkdownOptions): [HTMLUListElement | HTMLOListElement, number] {
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
      const [sub, next] = parseList(lines, i, indent, opts);
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
      appendInline(li, task[2], opts);
    } else {
      appendInline(li, m[3], opts);
    }
    list.appendChild(li);
    i++;
  }
  return [list, i];
}

export function renderMarkdown(
  src: string,
  depthOrOpts?: number | RenderMarkdownOptions,
  depthArg = 0,
): DocumentFragment {
  const opts = typeof depthOrOpts === "object" && depthOrOpts !== null ? depthOrOpts : undefined;
  const depth = typeof depthOrOpts === "number" ? depthOrOpts : depthArg;
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
      appendInline(h, hm[2], opts);
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
          appendInline(th, headers[c] ?? "", opts);
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
            appendInline(td, cells[c] ?? "", opts);
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
        appendInline(bq, qLines.join("\n"), opts);
      } else {
        bq.appendChild(renderMarkdown(qLines.join("\n"), opts, depth + 1));
      }
      frag.appendChild(bq);
      continue;
    }

    // List (ordered, unordered, nested, or task)
    const lm = LIST_ITEM_RE.exec(line);
    if (lm) {
      const [listEl, next] = parseList(lines, i, lm[1].length, opts);
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
      appendInline(p, pLines[j], opts);
    }
    frag.appendChild(p);
  }

  return frag;
}
