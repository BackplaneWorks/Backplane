// Safe, zero-dependency Markdown -> DOM renderer (never assigns untrusted innerHTML).
// Supports fenced code blocks (with copy button), headings, lists, blockquotes,
// inline code, bold, italic, and safe http(s) links.
import { t } from "./i18n";

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
      parent.appendChild(document.createTextNode(text.slice(last, m.index)));
    }
    const tok = m[0];
    if (tok.startsWith("`") && tok.endsWith("`")) {
      const code = document.createElement("code");
      code.className = "bp-code-inline";
      code.textContent = tok.slice(1, -1);
      parent.appendChild(code);
    } else if (tok.startsWith("**") && tok.endsWith("**")) {
      const strong = document.createElement("strong");
      strong.textContent = tok.slice(2, -2);
      parent.appendChild(strong);
    } else if (tok.startsWith("*") && tok.endsWith("*")) {
      const em = document.createElement("em");
      em.textContent = tok.slice(1, -1);
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
    parent.appendChild(document.createTextNode(text.slice(last)));
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
      const copyLabel = t("common.copy", "Copy");
      copyBtn.textContent = copyLabel;
      copyBtn.addEventListener("click", () => {
        navigator.clipboard?.writeText(codeText).then(() => {
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
