import { describe, expect, mock, test } from "bun:test";

mock.module("virtual:i18n", () => ({ CATALOGS: {} }));

type Attrs = Record<string, string>;

class FakeNode {
  parentNode: FakeEl | null = null;
  nodeType = 3;
  childNodes: FakeNode[] = [];
  nodeValue: string | null = null;
  get parentElement(): FakeEl | null { return this.parentNode; }
  get nextSibling(): FakeNode | null {
    if (!this.parentNode) return null;
    const i = this.parentNode.childNodes.indexOf(this);
    return i >= 0 && i + 1 < this.parentNode.childNodes.length ? this.parentNode.childNodes[i + 1] : null;
  }
  get textContent(): string {
    return this.nodeValue ?? this.childNodes.map((c) => c.textContent).join("");
  }
  set textContent(v: string) {
    this.nodeValue = v;
    this.childNodes = [];
  }
}

class FakeEl extends FakeNode {
  tagName: string;
  attrs: Attrs = {};
  style: Record<string, string> = {};
  className = "";
  href = "";
  target = "";
  rel = "";
  src = "";
  alt = "";
  type = "";
  title = "";
  value = "";
  rows = 0;
  disabled = false;
  scrollTop = 0;
  scrollHeight = 100;
  clientHeight = 100;
  classList = {
    add: (...c: string[]) => {
      const s = new Set(this.className.split(/\s+/).filter(Boolean));
      for (const x of c) s.add(x);
      this.className = Array.from(s).join(" ");
    },
    remove: (...c: string[]) => {
      const s = new Set(this.className.split(/\s+/).filter(Boolean));
      for (const x of c) s.delete(x);
      this.className = Array.from(s).join(" ");
    },
    toggle: (x: string, force?: boolean) => {
      const has = this.className.split(/\s+/).includes(x);
      if (force === true || (force === undefined && !has)) this.classList.add(x);
      else this.classList.remove(x);
    },
    contains: (x: string) => this.className.split(/\s+/).includes(x),
  };
  constructor(tag: string) {
    super();
    this.nodeType = 1;
    this.tagName = tag.toUpperCase();
  }
  get lastElementChild(): FakeEl | null {
    for (let i = this.childNodes.length - 1; i >= 0; i--) {
      if (this.childNodes[i] instanceof FakeEl) return this.childNodes[i] as FakeEl;
    }
    return null;
  }
  get firstChild(): FakeNode | null { return this.childNodes[0] ?? null; }
  appendChild<T extends FakeNode>(n: T): T {
    if (n.tagName === "#FRAGMENT") {
      for (const c of [...n.childNodes]) this.appendChild(c);
      return n;
    }
    if (n.parentNode) n.parentNode.removeChild(n);
    n.parentNode = this;
    this.childNodes.push(n);
    return n;
  }
  append(...ns: Array<FakeNode | string>): void {
    for (const n of ns) {
      if (typeof n === "string") {
        const t = new FakeNode();
        t.nodeValue = n;
        this.appendChild(t);
      } else this.appendChild(n);
    }
  }
  insertBefore<T extends FakeNode>(n: T, ref: FakeNode | null): T {
    if (n instanceof FakeEl && n.tagName === "#FRAGMENT") {
      for (const c of [...n.childNodes]) this.insertBefore(c, ref);
      return n;
    }
    if (n.parentNode) n.parentNode.removeChild(n);
    if (!ref) return this.appendChild(n);
    const i = this.childNodes.indexOf(ref);
    if (i < 0) return this.appendChild(n);
    n.parentNode = this;
    this.childNodes.splice(i, 0, n);
    return n;
  }
  replaceChild<T extends FakeNode>(newChild: FakeNode, oldChild: T): T {
    this.insertBefore(newChild, oldChild);
    this.removeChild(oldChild);
    return oldChild;
  }
  removeChild(n: FakeNode): void {
    const i = this.childNodes.indexOf(n);
    if (i >= 0) {
      this.childNodes.splice(i, 1);
      n.parentNode = null;
    }
  }
  remove(): void { this.parentNode?.removeChild(this); }
  replaceWith(n: FakeNode): void {
    if (!this.parentNode) return;
    this.parentNode.insertBefore(n, this);
    this.remove();
  }
  replaceChildren(...ns: Array<FakeNode | string>): void {
    for (const c of this.childNodes) c.parentNode = null;
    this.childNodes = [];
    this.append(...ns);
  }
  setAttribute(k: string, v: string): void { this.attrs[k] = v; }
  getAttribute(k: string): string | null { return this.attrs[k] ?? null; }
  addEventListener(): void {}
  closest(sel: string): FakeEl | null {
    const tags = new Set(sel.split(",").map((s) => s.trim().toUpperCase()));
    let cur: FakeEl | null = this;
    while (cur) {
      if (tags.has(cur.tagName)) return cur;
      cur = cur.parentNode;
    }
    return null;
  }
  querySelectorAll(tagOrCls: string): FakeEl[] {
    const out: FakeEl[] = [];
    const attrM = /^\[([^=\]]+)="([^"]*)"\]$/.exec(tagOrCls);
    const wantClasses = tagOrCls.startsWith(".") ? tagOrCls.slice(1).split(".").filter(Boolean) : [];
    const wantTag = wantClasses.length === 0 && !attrM ? tagOrCls.toUpperCase() : "";
    const walk = (n: FakeNode) => {
      if (n instanceof FakeEl) {
        if (
          (attrM && n.attrs[attrM[1]] === attrM[2]) ||
          (wantClasses.length > 0 && wantClasses.every((c) => n.classList.contains(c))) ||
          (wantTag && n.tagName === wantTag)
        ) out.push(n);
      }
      for (const c of n.childNodes) walk(c);
    };
    for (const c of this.childNodes) walk(c);
    return out;
  }
  querySelector(s: string): FakeEl | null { return this.querySelectorAll(s)[0] ?? null; }
  override get textContent(): string { return this.childNodes.map((c) => c.textContent).join(""); }
  override set textContent(v: string) {
    this.childNodes = [];
    if (v) {
      const t = new FakeNode();
      t.nodeValue = v;
      t.parentNode = this;
      this.childNodes.push(t);
    }
  }
}

function installFakeDom() {
  (globalThis as Record<string, unknown>).window = { location: { origin: "https://hub.example.test" }, setTimeout, clearTimeout, setInterval, clearInterval, getSelection: () => null };
  (globalThis as Record<string, unknown>).NodeFilter = { SHOW_TEXT: 4 };
  (globalThis as Record<string, unknown>).document = {
    createElement: (t: string) => new FakeEl(t),
    createElementNS: (_ns: string, t: string) => new FakeEl(t),
    createTextNode: (v: string) => { const n = new FakeNode(); n.nodeValue = v; return n; },
    createDocumentFragment: () => new FakeEl("#fragment"),
    createTreeWalker: (root: FakeEl) => {
      const nodes: FakeNode[] = [];
      const walk = (n: FakeNode) => {
        if (n.nodeType === 3) nodes.push(n);
        for (const c of n.childNodes) walk(c);
      };
      walk(root);
      let idx = 0;
      return { nextNode: () => nodes[idx++] ?? null };
    },
  };
}

installFakeDom();
const { renderMarkdown } = await import("../src/md");

describe("rich web markdown renderer (swarm parity)", () => {
  test("renders GFM tables with column alignment and inline formatting", () => {
    const frag = renderMarkdown([
      "| Component | Rating | Notes |",
      "| :--- | :---: | ---: |",
      "| `L1` | **10uH** | 12A sat |",
    ].join("\n")) as unknown as FakeEl;
    const tbl = frag.querySelector("table")!;
    expect(tbl).toBeDefined();
    const ths = tbl.querySelectorAll("th");
    expect(ths.map((x) => x.textContent)).toEqual(["Component", "Rating", "Notes"]);
    expect(ths[0].style.textAlign).toBe("left");
    expect(ths[1].style.textAlign).toBe("center");
    expect(ths[2].style.textAlign).toBe("right");
    const tds = tbl.querySelectorAll("td");
    expect(tds[0].querySelector("code")!.textContent).toBe("L1");
    expect(tds[1].querySelector("strong")!.textContent).toBe("10uH");
  });

  test("renders nested lists, task lists, headings h1-h6, hr, strikethrough, images, and safe links", () => {
    const frag = renderMarkdown([
      "### Architecture",
      "",
      "---",
      "",
      "- Top ~~old~~ item",
      "  - [x] Done subtask",
      "  - [ ] Open subtask",
      "",
      "See `src/main.rs` or [spec](https://example.com/spec) and ![wave](/blob/img-1) and ![trav](/blob/../secret) and ![ext](https://evil.example/leak.png) and ![bad](javascript:alert(1)).",
    ].join("\n")) as unknown as FakeEl;
    expect(frag.querySelectorAll("h4").length).toBe(1);
    expect(frag.querySelectorAll("hr").length).toBe(1);
    expect(frag.querySelectorAll("del").map((x) => x.textContent)).toEqual(["old"]);
    expect(frag.querySelectorAll(".bp-md-task").length).toBe(2);
    expect(frag.querySelectorAll(".bp-md-check.checked").length).toBe(1);
    const links = frag.querySelectorAll("a");
    expect(links.length).toBe(1);
    expect(links[0].href).toBe("https://example.com/spec");
    const imgs = frag.querySelectorAll("img");
    expect(imgs.length).toBe(1);
    expect(imgs[0].src).toBe("/blob/img-1");
  });

  test("preserves underscores/math outside backticks and caps deeply nested blockquotes", () => {
    const frag = renderMarkdown("Call foo_bar_baz and __init__ with 2 * 3 * 4 and _italic phrase_ and **bold**.") as unknown as FakeEl;
    expect(frag.querySelectorAll("em").map((x) => x.textContent)).toEqual(["italic phrase"]);
    expect(frag.querySelectorAll("strong").map((x) => x.textContent)).toEqual(["bold"]);
    expect(frag.textContent).toContain("foo_bar_baz");
    expect(frag.textContent).toContain("__init__");
    expect(frag.textContent).toContain("2 * 3 * 4");

    const deep = Array.from({ length: 25 }, (_, i) => "> ".repeat(i + 1) + "hi").join("\n");
    const deepFrag = renderMarkdown(deep) as unknown as FakeEl;
    expect(deepFrag.querySelectorAll("blockquote").length).toBeLessThanOrEqual(14);
  });
});
