// Command Palette DOM modal ("Jump or run a command"):
// Opens from `Commands` rail entry, `> Jump or run a command`, Search icon, Ctrl+K, Ctrl+Shift+P.
import { t, trs } from "./i18n";
import {
  buildPaletteItems,
  filterPaletteItems,
  markRange,
  type PaletteAction,
  type PaletteItem,
  type PaletteSearchResult,
  type PaletteState,
} from "./palette-model";

export type { PaletteAction, PaletteItem, PaletteSearchResult, PaletteState } from "./palette-model";

export interface CommandPaletteCallbacks {
  getState: () => PaletteState;
  onSearch: (q: string) => Promise<PaletteSearchResult>;
  onExecute: (action: PaletteAction) => void;
}

export class CommandPalette {
  private root: HTMLElement;
  private cb: CommandPaletteCallbacks;
  private backdrop: HTMLElement | null = null;
  private inputEl: HTMLInputElement | null = null;
  private listEl: HTMLElement | null = null;
  private query = "";
  private activeIdx = 0;
  private listed: PaletteItem[] = [];
  private searchHits: PaletteSearchResult | null = null;
  private searchHitsQuery = "";
  private searchTimer: number | null = null;
  private searchGen = 0;
  private prevFocus: HTMLElement | null = null;

  constructor(root: HTMLElement, cb: CommandPaletteCallbacks) {
    this.root = root;
    this.cb = cb;
  }

  public isOpen(): boolean {
    return this.backdrop !== null;
  }

  public open(initialQuery = ""): void {
    if (this.backdrop) {
      if (this.inputEl) {
        this.inputEl.focus();
        this.inputEl.select();
      }
      return;
    }

    this.prevFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    this.query = initialQuery;
    this.activeIdx = 0;
    this.searchHits = null;
    this.searchHitsQuery = "";
    this.searchGen++;

    const backdrop = document.createElement("div");
    backdrop.className = "bp-palette-backdrop";
    backdrop.setAttribute("data-testid", "command-palette");
    backdrop.setAttribute("role", "dialog");
    backdrop.setAttribute("aria-modal", "true");
    backdrop.setAttribute("aria-label", t("palette.command.placeholder", "Jump or run a command"));

    const box = document.createElement("div");
    box.className = "bp-palette-box";

    const inputWrap = document.createElement("div");
    inputWrap.className = "bp-palette-input-wrap";
    const input = document.createElement("input");
    input.type = "text";
    input.className = "bp-palette-input";
    input.setAttribute("data-testid", "palette-input");
    input.placeholder = "Jump to a thread, run a command or find what was said";
    input.value = this.query;
    input.autocomplete = "off";
    input.spellcheck = false;
    inputWrap.appendChild(input);

    const list = document.createElement("div");
    list.className = "bp-palette-list";
    list.setAttribute("role", "listbox");
    list.setAttribute("data-testid", "palette-list");

    box.append(inputWrap, list);
    backdrop.appendChild(box);

    backdrop.addEventListener("mousedown", (e) => {
      if (e.target === backdrop) this.close();
    });

    input.addEventListener("input", () => {
      this.query = input.value;
      this.activeIdx = 0;
      this.renderList();
      this.scheduleSearch();
    });

    input.addEventListener("keydown", (e) => {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        this.close();
      } else if (e.key === "ArrowDown") {
        e.preventDefault();
        if (this.listed.length > 0) {
          this.activeIdx = (this.activeIdx + 1) % this.listed.length;
          this.renderList(true);
        }
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        if (this.listed.length > 0) {
          this.activeIdx = (this.activeIdx - 1 + this.listed.length) % this.listed.length;
          this.renderList(true);
        }
      } else if (e.key === "Enter") {
        e.preventDefault();
        this.runAt(this.activeIdx);
      }
    });

    this.backdrop = backdrop;
    this.inputEl = input;
    this.listEl = list;
    this.root.appendChild(backdrop);

    this.renderList();
    if (this.query.trim().length >= 2) this.scheduleSearch();
    input.focus();
  }

  public close(): void {
    if (this.searchTimer !== null) {
      window.clearTimeout(this.searchTimer);
      this.searchTimer = null;
    }
    this.searchGen++;
    if (this.backdrop) {
      this.backdrop.remove();
      this.backdrop = null;
      this.inputEl = null;
      this.listEl = null;
    }
    this.prevFocus?.focus();
    this.prevFocus = null;
  }

  private scheduleSearch(): void {
    if (this.searchTimer !== null) {
      window.clearTimeout(this.searchTimer);
      this.searchTimer = null;
    }
    const q = this.query.trim();
    if (q.length < 2) {
      this.searchHits = null;
      this.searchHitsQuery = "";
      return;
    }
    if (q === this.searchHitsQuery) return;
    const gen = ++this.searchGen;
    this.searchTimer = window.setTimeout(() => {
      this.searchTimer = null;
      void this.cb.onSearch(q)
        .then((res) => {
          if (gen !== this.searchGen || !this.backdrop) return;
          if (this.query.trim() !== q) return;
          this.searchHits = res;
          this.searchHitsQuery = q;
          this.renderList();
        })
        .catch(() => {});
    }, 180);
  }

  private runAt(index: number): void {
    const item = this.listed[index];
    if (!item) return;
    this.close();
    this.cb.onExecute(item.action);
  }

  private renderList(scrollActive = false): void {
    if (!this.listEl) return;
    const state = this.cb.getState();
    const all = buildPaletteItems(state, { t, trs });
    const hits = this.query.trim() === this.searchHitsQuery ? this.searchHits : null;
    this.listed = filterPaletteItems(all, this.query, hits, state.projects);
    if (this.activeIdx >= this.listed.length) {
      this.activeIdx = Math.max(0, this.listed.length - 1);
    }

    if (this.listed.length === 0) {
      const empty = document.createElement("div");
      empty.className = "bp-palette-empty";
      empty.textContent = "Nothing matches";
      this.listEl.replaceChildren(empty);
      return;
    }

    const frag = document.createDocumentFragment();
    let activeEl: HTMLElement | null = null;
    for (let i = 0; i < this.listed.length; i++) {
      const it = this.listed[i];
      const row = document.createElement("button");
      row.type = "button";
      row.className = "bp-palette-row" + (i === this.activeIdx ? " active" : "");
      row.setAttribute("role", "option");
      row.setAttribute("aria-selected", i === this.activeIdx ? "true" : "false");
      row.setAttribute("data-palette-kind", it.kind);

      if (it.dotClass) {
        const dot = document.createElement("span");
        dot.className = `bp-dot ${it.dotClass}`;
        row.appendChild(dot);
      }

      const lbl = document.createElement("span");
      lbl.className = "bp-palette-label" + (it.kind === "you said" || it.kind === "agent said" ? " bp-palette-msg" : "");
      const range = (it.kind === "you said" || it.kind === "agent said") ? markRange(it.label, this.query) : null;
      if (range) {
        const [a, b] = range;
        if (a > 0) lbl.appendChild(document.createTextNode(it.label.slice(0, a)));
        const mark = document.createElement("strong");
        mark.className = "bp-palette-mark";
        mark.textContent = it.label.slice(a, b);
        lbl.appendChild(mark);
        if (b < it.label.length) lbl.appendChild(document.createTextNode(it.label.slice(b)));
      } else {
        lbl.textContent = it.label;
      }
      row.appendChild(lbl);

      if (it.want) {
        const want = document.createElement("span");
        want.className = "bp-palette-want" + (it.want === "failed" ? " dim" : "");
        want.textContent = it.want;
        row.appendChild(want);
      }

      if (it.shortcut) {
        const key = document.createElement("span");
        key.className = "bp-palette-key";
        key.textContent = it.shortcut;
        row.appendChild(key);
      }

      const place = document.createElement("span");
      place.className = "bp-palette-place";
      place.textContent = it.place || it.kind;
      row.appendChild(place);

      row.addEventListener("mouseenter", () => {
        if (this.activeIdx !== i) {
          this.activeIdx = i;
          this.updateActiveClass();
        }
      });
      row.addEventListener("click", () => this.runAt(i));

      if (i === this.activeIdx) activeEl = row;
      frag.appendChild(row);
    }

    this.listEl.replaceChildren(frag);
    if (scrollActive && activeEl) {
      activeEl.scrollIntoView({ block: "nearest" });
    }
  }

  private updateActiveClass(): void {
    if (!this.listEl) return;
    const children = this.listEl.children;
    for (let i = 0; i < children.length; i++) {
      const isAct = i === this.activeIdx;
      children[i].classList.toggle("active", isAct);
      children[i].setAttribute("aria-selected", isAct ? "true" : "false");
    }
  }
}
