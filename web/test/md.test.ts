import { describe, expect, mock, test } from "bun:test";

mock.module("virtual:i18n", () => ({ CATALOGS: {} }));
const { exactCopyText, renderMarkdown } = await import("../src/md");

interface FakeEl {
  tagName: string;
  className: string;
  textContent: string;
  attrs: Map<string, string>;
  children: Array<FakeEl | FakeText>;
  listeners: Map<string, Array<(e?: any) => void>>;
  classList: {
    add: (c: string) => void;
    remove: (c: string) => void;
    contains: (c: string) => boolean;
  };
  setAttribute: (k: string, v: string) => void;
  getAttribute: (k: string) => string | null;
  append: (...nodes: Array<FakeEl | FakeText>) => void;
  appendChild: (node: FakeEl | FakeText) => FakeEl | FakeText;
  replaceChildren: (...nodes: Array<FakeEl | FakeText>) => void;
  addEventListener: (ev: string, fn: (e?: any) => void) => void;
  click: () => void;
  title: string;
  type?: string;
  href?: string;
  target?: string;
  rel?: string;
}
interface FakeText {
  nodeType: 3;
  textContent: string;
}

function makeFakeDom() {
  const mkEl = (tag: string): FakeEl => {
    const classes = new Set<string>();
    const el: FakeEl = {
      tagName: tag.toUpperCase(),
      get className() {
        return Array.from(classes).join(" ");
      },
      set className(v: string) {
        classes.clear();
        for (const c of v.split(/\s+/).filter(Boolean)) classes.add(c);
      },
      textContent: "",
      attrs: new Map(),
      children: [],
      listeners: new Map(),
      title: "",
      classList: {
        add: (c) => classes.add(c),
        remove: (c) => classes.delete(c),
        contains: (c) => classes.has(c),
      },
      setAttribute(k, v) {
        this.attrs.set(k, v);
      },
      getAttribute(k) {
        return this.attrs.get(k) ?? null;
      },
      append(...nodes) {
        this.children.push(...nodes);
      },
      appendChild(n) {
        this.children.push(n);
        return n;
      },
      replaceChildren(...nodes) {
        this.children = [...nodes];
      },
      addEventListener(ev, fn) {
        const list = this.listeners.get(ev) ?? [];
        list.push(fn);
        this.listeners.set(ev, list);
      },
      click() {
        for (const fn of this.listeners.get("click") ?? []) {
          fn({ stopPropagation() {}, preventDefault() {} });
        }
      },
    };
    return el;
  };
  const mkFrag = () => ({
    children: [] as Array<FakeEl | FakeText>,
    appendChild(n: FakeEl | FakeText) {
      this.children.push(n);
      return n;
    },
  });
  (globalThis as any).document = {
    createElement: (tag: string) => mkEl(tag),
    createElementNS: (_ns: string, tag: string) => mkEl(tag),
    createTextNode: (t: string): FakeText => ({ nodeType: 3, textContent: t }),
    createDocumentFragment: () => mkFrag(),
  };
  let copied = "";
  let selection = "";
  const timers = new Map<number, () => void>();
  let nextTimer = 1;
  (globalThis as any).window = {
    location: { origin: "https://example.com" },
    setTimeout: (fn: () => void) => {
      const id = nextTimer++;
      timers.set(id, fn);
      return id;
    },
    clearTimeout: (id: number) => {
      timers.delete(id);
    },
    getSelection: () => ({ toString: () => selection }),
  };
  (globalThis as any).navigator = {
    clipboard: {
      writeText: (t: string) => {
        copied = t;
        return Promise.resolve();
      },
    },
  };
  return {
    getCopied: () => copied,
    resetCopied: () => { copied = ""; },
    setSelection: (s: string) => { selection = s; },
    flushTimers: () => {
      const pending = Array.from(timers.values());
      timers.clear();
      for (const fn of pending) fn();
    },
  };
}

function findAll(node: any, pred: (el: FakeEl) => boolean, out: FakeEl[] = []): FakeEl[] {
  if (!node || !Array.isArray(node.children)) return out;
  for (const c of node.children) {
    if (c && c.tagName) {
      if (pred(c)) out.push(c);
      findAll(c, pred, out);
    }
  }
  return out;
}

describe("web md in-place copy", () => {
  test("exactCopyText strips trailing line breaks only", () => {
    expect(exactCopyText("git status\n\r\n")).toBe("git status");
    expect(exactCopyText("  keep spaces  ")).toBe("  keep spaces  ");
  });

  test("renders inline code with in-place copy button and click-to-copy without trailing chips", async () => {
    const dom = makeFakeDom();
    const frag = renderMarkdown("Use `EPC2306` and `docs/architecture.md` with `LM5113QDPRRQ1`.");
    const wraps = findAll(frag, (el) => el.classList.contains("bp-code-wrap"));
    expect(wraps.length).toBe(3);

    const codes = findAll(frag, (el) => el.classList.contains("bp-code-inline"));
    expect(codes.map((c) => c.textContent)).toEqual(["EPC2306", "docs/architecture.md", "LM5113QDPRRQ1"]);

    const btns = findAll(frag, (el) => el.classList.contains("bp-inline-copy"));
    expect(btns.length).toBe(3);
    expect(btns.every((b) => b.getAttribute("data-bp-no-copy") === "1")).toBe(true);

    // Collapsed click on inline code copies it in place after double-click window
    codes[0].click();
    dom.flushTimers();
    await Promise.resolve();
    expect(dom.getCopied()).toBe("EPC2306");
    expect(wraps[0].classList.contains("is-copied")).toBe(true);

    // Double-click cancels delayed click-to-copy
    dom.resetCopied();
    codes[0].click();
    for (const fn of codes[0].listeners.get("dblclick") ?? []) fn({});
    dom.flushTimers();
    await Promise.resolve();
    expect(dom.getCopied()).toBe("");

    // Active drag selection does NOT trigger click-to-copy
    dom.resetCopied();
    dom.setSelection("EPC2306");
    codes[1].click();
    dom.flushTimers();
    await Promise.resolve();
    expect(dom.getCopied()).toBe("");

    // Clicking the explicit hover copy button still works immediately
    dom.setSelection("");
    btns[2].click();
    await Promise.resolve();
    expect(dom.getCopied()).toBe("LM5113QDPRRQ1");
  });

  test("renders bare file paths with in-place copy affordance and rejects non-path ratios/dates", async () => {
    const dom = makeFakeDom();
    const frag = renderMarkdown("See docs/architecture.md and /etc/hosts (not 1/2.5, and/or., w/o.x, 11/25.2026):\n```bash\necho ok\n\n```");
    const paths = findAll(frag, (el) => el.classList.contains("bp-md-path"));
    expect(paths.map((p) => p.textContent)).toEqual(["docs/architecture.md", "/etc/hosts"]);

    const copyBtn = findAll(frag, (el) => el.classList.contains("bp-code-copy"))[0];
    expect(copyBtn.getAttribute("data-bp-no-copy")).toBe("1");
    copyBtn.click();
    await Promise.resolve();
    expect(dom.getCopied()).toBe("echo ok");
  });
});
