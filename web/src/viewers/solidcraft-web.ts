// Embedded self-hosted SolidCraft Web (WASM) controller for the Web UI Mech/CAD pane.
// Communicates strictly same-origin with /solidcraft-web/ over postMessage (zero external solidcraft.io calls).

import { t } from "../i18n";

export interface SolidCraftWebStatus {
  installed: boolean;
  version: string;
  source: string;
  available: boolean;
  size: number;
  from: string;
  folder: string;
  installing: boolean;
  copy?: boolean;
  install?: {
    state: string;
    note: string;
    done: number;
    total: number;
  };
}

export interface SolidCraftWebCallbacks {
  onReady?: (loadMs: number) => void;
  onSaving?: () => void;
  onSaved?: (path: string, size: number) => void;
  onError?: (op: string, message: string) => void;
}

const SOLIDCRAFT_CAD_EXTS = new Set([
  "sc",
  "solidcraft",
  "step",
  "stp",
  "stl",
  "3mf",
  "iges",
  "igs",
  "obj",
  "glb",
]);

export function isSolidCraftCad(path: string): boolean {
  const clean = path.trim().toLowerCase();
  const dot = clean.lastIndexOf(".");
  if (dot <= 0) return false;
  return SOLIDCRAFT_CAD_EXTS.has(clean.slice(dot + 1));
}

export class SolidCraftWebEmbed {
  private iframe: HTMLIFrameElement;
  private threadId: string;
  private relPath: string;
  private light: boolean;
  private readOnly: boolean;
  private cbs: SolidCraftWebCallbacks;
  private onMsg: (ev: MessageEvent) => void;
  private destroyed = false;

  constructor(
    stage: HTMLElement,
    threadId: string,
    relPath: string,
    light: boolean,
    readOnly: boolean,
    cbs: SolidCraftWebCallbacks = {}
  ) {
    this.threadId = threadId;
    this.relPath = relPath;
    this.light = light;
    this.readOnly = readOnly;
    this.cbs = cbs;

    this.iframe = document.createElement("iframe");
    this.iframe.className = "bp-sc-frame";
    this.iframe.setAttribute("data-testid", "solidcraft-web-frame");
    this.iframe.setAttribute("title", t("solidcraft.web.open", "SolidCraft Web"));
    // Trust boundary: same-origin iframe executes pinned, SHA-256-verified self-hosted WASM under strict CSP
    // (zero external solidcraft.io requests, no sw.js) and shares the paired session to read/save via hub endpoints.
    this.iframe.setAttribute("sandbox", "allow-scripts allow-same-origin allow-downloads");
    this.iframe.src = this.buildUrl();
    stage.replaceChildren(this.iframe);

    this.onMsg = (ev: MessageEvent) => {
      if (this.destroyed) return;
      if (ev.origin !== window.location.origin) return;
      if (ev.source !== this.iframe.contentWindow) return;
      const d = ev.data as Record<string, unknown> | null;
      if (!d || typeof d !== "object") return;
      switch (d.type) {
        case "bp:solidcraft:ready":
          this.cbs.onReady?.(typeof d.loadMs === "number" ? d.loadMs : 0);
          break;
        case "bp:solidcraft:saving":
          this.cbs.onSaving?.();
          break;
        case "bp:solidcraft:saved":
          this.cbs.onSaved?.(
            typeof d.path === "string" ? d.path : this.relPath,
            typeof d.size === "number" ? d.size : 0
          );
          break;
        case "bp:solidcraft:error":
          this.cbs.onError?.(
            typeof d.op === "string" ? d.op : "error",
            typeof d.error === "string" ? d.error : "SolidCraft Web error"
          );
          break;
      }
    };
    window.addEventListener("message", this.onMsg);
  }

  private buildUrl(): string {
    const q = new URLSearchParams({
      thread: this.threadId,
      path: this.relPath,
      theme: this.light ? "light" : "dark",
    });
    if (this.readOnly) q.set("readonly", "1");
    return `/solidcraft-web/?${q.toString()}`;
  }

  private post(msg: Record<string, unknown>): void {
    if (this.destroyed || !this.iframe.contentWindow) return;
    this.iframe.contentWindow.postMessage(msg, window.location.origin);
  }

  open(threadId: string, relPath: string): void {
    if (this.destroyed) return;
    if (threadId === this.threadId && relPath === this.relPath) return;
    this.threadId = threadId;
    this.relPath = relPath;
    this.post({ type: "bp:solidcraft:open", thread: threadId, path: relPath });
  }

  save(): void {
    this.post({ type: "bp:solidcraft:save" });
  }

  fit(): void {
    this.post({ type: "bp:solidcraft:fit" });
  }

  setTheme(light: boolean): void {
    this.light = light;
    this.post({ type: "bp:solidcraft:theme", light });
  }

  destroy(): void {
    if (this.destroyed) return;
    this.destroyed = true;
    window.removeEventListener("message", this.onMsg);
    this.iframe.src = "about:blank";
    this.iframe.remove();
  }
}
