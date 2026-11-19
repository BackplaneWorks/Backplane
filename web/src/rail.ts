// Right viewer rail: 72px wide, 48px vertical items, 2px left accent bar when active.
// Renders verbatim from the hub's `viewer.rail` model — hub is the sole authority (no hardcoded fallback).

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
  const paths = ICON_PATHS[name] ?? ICON_PATHS.box;
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

  constructor(container: HTMLElement, onPress: (id: string) => void) {
    this.el = container;
    this.onPress = onPress;
    this.render();
  }

  public setItems(items: RailEntry[]): void {
    this.items = items;
    this.render();
  }

  public setActive(id: string): void {
    this.activeId = id;
    this.render();
  }

  public render(): void {
    const frag = document.createDocumentFragment();
    let prevGroup: string | undefined;

    for (const item of this.items) {
      if (prevGroup !== undefined && item.group !== prevGroup) {
        const sep = document.createElement("div");
        sep.className = "bp-rail-gap";
        frag.appendChild(sep);
      }
      prevGroup = item.group;

      const isOn = item.state === "on" || (this.activeId !== "" && item.id === this.activeId);
      const isDisabled = item.state === "disabled";

      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "bp-rail-btn" + (isOn ? " active" : "") + (isDisabled ? " disabled" : "");
      btn.disabled = isDisabled;
      btn.title = item.why ? `${item.label} — ${item.why}` : item.key ? `${item.label} (${item.key})` : item.label;
      btn.setAttribute("data-rail-id", item.id);
      btn.setAttribute("data-testid", `rail-${item.id}`);

      btn.appendChild(makeIcon(item.icon));
      const lbl = document.createElement("span");
      lbl.className = "bp-rail-lbl";
      lbl.textContent = item.label;
      btn.appendChild(lbl);

      if (!isDisabled) {
        btn.addEventListener("click", () => this.onPress(item.id));
      }
      frag.appendChild(btn);
    }

    this.el.replaceChildren(frag);
  }
}
