// Pure coordinate mapping and input-serialization helpers for the web Browser pane.
// Mirrors mobile/ios/Backplane/DataViewer.swift (0..1000 click mapping on object-fit:contain
// frame + scroll scaling + serialized op queue).

export interface ImageBox {
  /** Bounding client rect of the <img> element */
  left: number;
  top: number;
  width: number;
  height: number;
  /** Natural pixel dimensions of the loaded screen.jpg */
  naturalWidth: number;
  naturalHeight: number;
}

/**
 * Map a pointer event (clientX, clientY) over an `object-fit: contain` (aligned top-center
 * or center) image into the hub's 0..1000 page coordinates. Returns null if natural size is 0
 * or the click lands outside the rendered image bounds.
 */
export function mapClickToPageCoords(
  clientX: number,
  clientY: number,
  box: ImageBox,
  alignTop = true
): { x: number; y: number } | null {
  if (box.width <= 0 || box.height <= 0 || box.naturalWidth <= 0 || box.naturalHeight <= 0) {
    return null;
  }
  const scale = Math.min(box.width / box.naturalWidth, box.height / box.naturalHeight);
  const w = box.naturalWidth * scale;
  const h = box.naturalHeight * scale;
  const x0 = box.left + (box.width - w) / 2;
  const y0 = alignTop ? box.top : box.top + (box.height - h) / 2;
  const rx = (clientX - x0) / w;
  const ry = (clientY - y0) / h;
  if (rx < 0 || rx > 1 || ry < 0 || ry > 1) return null;
  return {
    x: Math.max(0, Math.min(1000, Math.round(rx * 1000))),
    y: Math.max(0, Math.min(1000, Math.round(ry * 1000))),
  };
}

/** Allowed toolbar commands per hub webpolicy.rs (PR #1433). */
export type BrowserToolbarCommand = "back" | "forward" | "reload";

/** Quick key buttons matching mobile/ios/Backplane/BrowserInput. */
export const BROWSER_KEY_BUTTONS: ReadonlyArray<readonly [label: string, key: string]> = [
  ["Enter", "Enter"],
  ["Tab", "Tab"],
  ["Bksp", "Backspace"],
  ["Esc", "Escape"],
  ["↑", "ArrowUp"],
  ["↓", "ArrowDown"],
  ["←", "ArrowLeft"],
  ["→", "ArrowRight"],
];

/**
 * Normalize a user-entered URL for `browser {op:"navigate", url}`.
 * Trims whitespace and prepends `https://` when no scheme is present.
 */
export function normalizeBrowserUrl(raw: string): string {
  const s = raw.trim();
  if (!s) return "";
  if (/^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(s)) return s;
  return `https://${s}`;
}

/**
 * Serialized async queue matching iOS `DataBrowserTab.run`: ensures browser RPCs
 * (takeover, click, type, press, scroll, navigate, toolbar, handback) execute strictly
 * in send order.
 */
export class SerialQueue {
  private tail: Promise<void> = Promise.resolve();

  enqueue(fn: () => Promise<void>): Promise<void> {
    const next = this.tail.then(fn, fn);
    this.tail = next.catch(() => {});
    return next;
  }
}
