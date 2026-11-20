// Pure view-preset and Board/Schematic history-strip helpers for the web viewer pane.
// Mirrors native/gpui/src/viewers/solid.rs (solid_tools / Camera::named) and
// data/1 `viewer.refs` / `viewer.compare`.

export type SolidViewName =
  | "fit"
  | "iso"
  | "top"
  | "bottom"
  | "front"
  | "right"
  | "back"
  | "left";

export const SOLID_VIEW_PRESETS: ReadonlyArray<{ id: SolidViewName; label: string }> = [
  { id: "fit", label: "Fit" },
  { id: "iso", label: "Iso" },
  { id: "top", label: "Top" },
  { id: "bottom", label: "Bottom" },
  { id: "front", label: "Front" },
  { id: "right", label: "Right" },
  { id: "back", label: "Back" },
  { id: "left", label: "Left" },
];

/** Camera (yaw, pitch) in radians matching native/gpui/src/viewers/solid.rs Camera::named. */
export function solidViewAngles(view: SolidViewName): { yaw: number; pitch: number } {
  switch (view) {
    case "top":
      return { yaw: 0, pitch: Math.PI * 0.499 };
    case "bottom":
      return { yaw: 0, pitch: -Math.PI * 0.499 };
    case "front":
      return { yaw: 0, pitch: 0 };
    case "right":
      return { yaw: Math.PI * 0.5, pitch: 0 };
    case "back":
      return { yaw: Math.PI, pitch: 0 };
    case "left":
      return { yaw: -Math.PI * 0.5, pitch: 0 };
    case "iso":
    case "fit":
    default:
      return { yaw: 0.6, pitch: 0.7 };
  }
}

export interface ViewerRef {
  kind: string;
  label: string;
  commit: string;
}

/**
 * Pure state machine for the Board / Schematic bottom history strip.
 * `index === -1` means Live working tree; `0..refs.length - 1` selects a saved ref
 * (ordered newest-first as returned by `viewer.refs`).
 */
export class HistoryStripState {
  refs: ViewerRef[] = [];
  index = -1; // -1 = Live
  highlightEdits = false;
  playing = false;

  setRefs(refs: ViewerRef[]): void {
    this.refs = refs;
    if (this.index >= refs.length) this.index = -1;
  }

  isLive(): boolean {
    return this.index < 0 || this.index >= this.refs.length;
  }

  activeRef(): ViewerRef | null {
    return this.isLive() ? null : this.refs[this.index];
  }

  /** Step older (+1) or newer (-1 towards Live). Returns true if selection changed. */
  step(delta: -1 | 1): boolean {
    if (this.refs.length === 0) return false;
    const prev = this.index;
    if (delta > 0) {
      // Older snapshot
      this.index = Math.min(this.refs.length - 1, this.index + 1);
    } else {
      // Newer snapshot (down to -1 = Live)
      this.index = Math.max(-1, this.index - 1);
    }
    if (this.index === -1) this.playing = false;
    return this.index !== prev;
  }

  selectCommit(commit: string): boolean {
    if (!commit) {
      return this.goLive();
    }
    const idx = this.refs.findIndex((r) => r.commit === commit);
    if (idx < 0) return false;
    const changed = this.index !== idx;
    this.index = idx;
    return changed;
  }

  goLive(): boolean {
    const changed = this.index !== -1 || this.playing;
    this.index = -1;
    this.playing = false;
    return changed;
  }

  toggleHighlight(): boolean {
    this.highlightEdits = !this.highlightEdits;
    // If toggling Highlight edits on while Live and refs exist, compare against the newest ref.
    if (this.highlightEdits && this.isLive() && this.refs.length > 0) {
      this.index = 0;
    }
    return this.highlightEdits;
  }

  /** Payload for `viewer.compare` when comparing, or null when Live. `bits: 0` compares all layers. */
  compareRequest(): { commit: string; bits: number } | null {
    const ref = this.activeRef();
    if (!ref) return null;
    return { commit: ref.commit, bits: 0 };
  }
}
