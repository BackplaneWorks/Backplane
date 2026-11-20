// Right viewer rail: 72px wide, 48px vertical items, 2px left accent bar when active.
// Renders verbatim from the hub's `viewer.rail` model (PR #1367 / #1376):
// hub order, disabled entry dimmed via --rail-disabled / --rail-disabled-icon with hub `why`,
// tools group separated by --rail-rule at bottom, Split pane omitted until docking tiles land.
import { trs } from "./i18n";

export interface RailEntry {
  id: string;
  label: string;
  icon: string;
  group?: string;
  key?: string;
  state: "on" | "off" | "disabled";
  why?: string;
}

const SVG_NS = "http://www.w3.org/2000/svg";

// Minimal crisp 18x18 stroke icons matching GPUI's Lucide set
const ICON_PATHS: Record<string, string[]> = {
  terminal: ["M4 17l6-6-6-6", "M12 19h8"],
  search: ["M11 19a8 8 0 1 0 0-16 8 8 0 0 0 0 16z", "M21 21l-4.3-4.3"],
  "circuit-board": ["M3 3h18v18H3z", "M9 3v6h6", "M9 21v-6", "M15 9a2 2 0 1 0 0-4 2 2 0 0 0 0 4z", "M9 15a2 2 0 1 0 0-4 2 2 0 0 0 0 4z"],
  cpu: ["M6 6h12v12H6z", "M9 2v4", "M15 2v4", "M9 18v4", "M15 18v4", "M2 9h4", "M2 15h4", "M18 9h4", "M18 15h4"],
  box: ["M21 16V8l-9-5-9 5v8l9 5 9-5z", "M3.3 7L12 12l8.7-5", "M12 22V12"],
  gauge: ["M12 14l4-4", "M3.3 19a10 10 0 1 1 17.4 0"],
  "file-stack": ["M16 2H6v12h10V2z", "M20 6v12H10", "M4 18h10v4H4z"],
  "layout-panel-left": ["M3 3h18v18H3z", "M10 3v18"],
  layers: ["M12 2L2 7l10 5 10-5-10-5z", "M2 12l10 5 10-5", "M2 17l10 5 10-5"],
  wrench: ["M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"],
  folder: ["M3 6h6l2 2h10v11H3z"],
  diff: ["M12 5v10", "M7 10h10", "M7 19h10"],
  "circle-check": ["M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20z", "M9 12l2 2 4-4"],
  globe: ["M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20z", "M2 12h20", "M12 2a15 15 0 0 1 0 20 15 15 0 0 1 0-20z"],
};

function makeIcon(name: string): SVGSVGElement {
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("width", "18");
  svg.setAttribute("height", "18");
  svg.setAttribute("fill", "none");
  svg.setAttribute("stroke", "currentColor");
  svg.setAttribute("stroke-width", "1.75");
  svg.setAttribute("stroke-linecap", "square");
  svg.setAttribute("stroke-linejoin", "miter");
  svg.setAttribute("aria-hidden", "true");
  const paths = ICON_PATHS[name] ?? ICON_PATHS.folder;
  for (const d of paths) {
    const p = document.createElementNS(SVG_NS, "path");
    p.setAttribute("d", d);
    svg.appendChild(p);
  }
  return svg;
}

export class RailView {
  private el: HTMLElement;
  private items: RailEntry[] = [];
  private activeId = "";
  private onPress: (id: string) => void;
  private pagesRo: ResizeObserver | null = null;

  constructor(container: HTMLElement, onPress: (id: string) => void) {
    this.el = container;
    this.onPress = onPress;
    this.render();
  }

  public setItems(items: RailEntry[]): void {
    // Draw the hub's `viewer.rail` list verbatim without client-side hiding.
    this.items = items.slice();
    this.render();
  }

  public setActive(id: string): void {
    this.activeId = id;
    this.render();
  }

  private renderButton(item: RailEntry): HTMLButtonElement {
    const isDisabled = item.state === "disabled";
    const isOn = !isDisabled && (item.state === "on" || (this.activeId !== "" && item.id === this.activeId));
    const label = trs(item.label);
    const why = item.why ? trs(item.why) : "";

    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "bp-rail-btn" + (isOn ? " active" : "") + (isDisabled ? " disabled" : "");
    btn.disabled = isDisabled;
    btn.setAttribute("aria-pressed", isOn ? "true" : "false");
    if (isDisabled) {
      btn.title = why || label;
      if (why) btn.setAttribute("data-why", why);
    } else {
      btn.title = item.key ? `${label} (${item.key.toUpperCase()})` : label;
    }
    btn.setAttribute("data-rail-id", item.id);
    btn.setAttribute("data-testid", `rail-${item.id}`);

    btn.appendChild(makeIcon(item.icon));
    const lbl = document.createElement("span");
    lbl.className = "bp-rail-lbl";
    lbl.textContent = label;
    btn.appendChild(lbl);

    if (!isDisabled) {
      btn.addEventListener("click", () => this.onPress(item.id));
    }
    return btn;
  }

  public render(): void {
    const pages = this.items.filter((x) => x.group !== "tools");
    const tools = this.items.filter((x) => x.group === "tools");

    const pagesWrap = document.createElement("div");
    pagesWrap.className = "bp-rail-pages";
    let prevGroup: string | undefined;
    for (const item of pages) {
      if (prevGroup !== undefined && item.group !== prevGroup) {
        const sep = document.createElement("div");
        sep.className = "bp-rail-rule";
        pagesWrap.appendChild(sep);
      }
      prevGroup = item.group;
      pagesWrap.appendChild(this.renderButton(item));
    }

    const syncFade = (): void => {
      const canLeft = pagesWrap.scrollLeft > 2;
      const canRight = pagesWrap.scrollLeft + pagesWrap.clientWidth < pagesWrap.scrollWidth - 2;
      pagesWrap.classList.toggle("bp-fade-left", canLeft);
      pagesWrap.classList.toggle("bp-fade-right", canRight);
    };
    pagesWrap.addEventListener("scroll", syncFade, { passive: true });
    this.pagesRo?.disconnect();
    this.pagesRo = null;
    if (typeof ResizeObserver !== "undefined") {
      this.pagesRo = new ResizeObserver(syncFade);
      this.pagesRo.observe(pagesWrap);
    }
    requestAnimationFrame(syncFade);

    const frag = document.createDocumentFragment();
    frag.appendChild(pagesWrap);

    if (tools.length > 0) {
      const toolsWrap = document.createElement("div");
      toolsWrap.className = "bp-rail-tools";
      for (const item of tools) {
        toolsWrap.appendChild(this.renderButton(item));
      }
      frag.appendChild(toolsWrap);
    }

    this.el.replaceChildren(frag);
  }
}
