// Safe, zero-dependency Markdown -> DOM renderer (never assigns untrusted innerHTML).
// Supports fenced code blocks (with copy button), headings, lists, blockquotes,
// inline code, bold, italic, and safe http(s) links.
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

function appendInline(parent: HTMLElement, text: string): void {
  const re = /(`[^`]+`|\*\*[^*]+\*\*|\*[^*]+\*|\[([^\]]+)\]\(([^)]+)\))/g;
  let last = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text)) !== null) {
    if (m.index > last) {
      appendPlainWithPaths(parent, text.slice(last, m.index));
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
    } else if (tok.startsWith("**") && tok.endsWith("**")) {
      const strong = document.createElement("strong");
      appendInline(strong, tok.slice(2, -2));
      parent.appendChild(strong);
    } else if (tok.startsWith("*") && tok.endsWith("*")) {
      const em = document.createElement("em");
      appendInline(em, tok.slice(1, -1));
      parent.appendChild(em);
    } else if (tok.startsWith("[")) {
      const label = m[2] ?? "";
      const href = safeUrl(m[3] ?? "");
      if (href) {
        const a = document.createElement("a");
        a.href = href;
        a.target = "_blank";
        a.rel = "noopener noreferrer";
        a.textContent = label;
        parent.appendChild(a);
      } else {
        parent.appendChild(document.createTextNode(label));
      }
    }
    last = re.lastIndex;
  }
  if (last < text.length) {
    appendPlainWithPaths(parent, text.slice(last));
  }
}

export function renderMarkdown(src: string): DocumentFragment {
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

    // Heading
    const hm = /^(#{1,4})\s+(.+)$/.exec(line);
    if (hm) {
      const h = document.createElement(`h${Math.min(4, hm[1].length + 1)}`);
      h.className = "bp-md-h";
      appendInline(h, hm[2]);
      frag.appendChild(h);
      i++;
      continue;
    }

    // Blockquote
    if (line.startsWith("> ")) {
      const bq = document.createElement("blockquote");
      bq.className = "bp-md-quote";
      const qLines: string[] = [];
      while (i < lines.length && lines[i].startsWith("> ")) {
        qLines.push(lines[i].slice(2));
        i++;
      }
      appendInline(bq, qLines.join("\n"));
      frag.appendChild(bq);
      continue;
    }

    // Unordered list
    if (/^\s*[-*]\s+/.test(line)) {
      const ul = document.createElement("ul");
      ul.className = "bp-md-list";
      while (i < lines.length && /^\s*[-*]\s+/.test(lines[i])) {
        const li = document.createElement("li");
        appendInline(li, lines[i].replace(/^\s*[-*]\s+/, ""));
        ul.appendChild(li);
        i++;
      }
      frag.appendChild(ul);
      continue;
    }

    // Ordered list
    if (/^\s*\d+\.\s+/.test(line)) {
      const ol = document.createElement("ol");
      ol.className = "bp-md-list";
      while (i < lines.length && /^\s*\d+\.\s+/.test(lines[i])) {
        const li = document.createElement("li");
        appendInline(li, lines[i].replace(/^\s*\d+\.\s+/, ""));
        ol.appendChild(li);
        i++;
      }
      frag.appendChild(ol);
      continue;
    }

    // Paragraph (join consecutive non-special lines)
    const pLines: string[] = [];
    while (
      i < lines.length &&
      lines[i].trim() !== "" &&
      !lines[i].trimStart().startsWith("```") &&
      !/^#{1,4}\s+/.test(lines[i]) &&
      !lines[i].startsWith("> ") &&
      !/^\s*[-*]\s+/.test(lines[i]) &&
      !/^\s*\d+\.\s+/.test(lines[i])
    ) {
      pLines.push(lines[i]);
      i++;
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
