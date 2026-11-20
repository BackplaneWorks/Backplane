// Real browser Terminal pane backed by vendored @xterm/xterm@5.5.0 + @xterm/addon-fit@0.10.0.
// Built as a standalone lazy chunk (/assets/terminal.js) loaded only when Terminal opens,
// keeping the P1 first-load bundle untouched.

import { Terminal } from "../../vendor/xterm/xterm.js";
import { FitAddon } from "../../vendor/xterm/addon-fit.js";
import type { ViewerHostApi } from "../app";

export interface TerminalPaneHandle {
  setTheme(light: boolean): void;
  destroy(): void;
}

function decodeBase64Bytes(b64: string): Uint8Array {
  try {
    const bin = atob(b64);
    const out = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
    return out;
  } catch {
    return new Uint8Array(0);
  }
}

function encodeBase64Utf8(str: string): string {
  const bytes = new TextEncoder().encode(str);
  let bin = "";
  for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]);
  return btoa(bin);
}

function xtermTheme(light: boolean) {
  if (light) {
    return {
      background: "#f5f4ef",
      foreground: "#1b1d21",
      cursor: "#1f6fd1",
      selectionBackground: "rgba(31,111,209,0.22)",
      black: "#1b1d21",
      red: "#c93434",
      green: "#1c8550",
      yellow: "#9a6700",
      blue: "#1f6fd1",
      magenta: "#a3399a",
      cyan: "#127c84",
      white: "#d3d1ca",
      brightBlack: "#5b6068",
      brightRed: "#e05252",
      brightGreen: "#2ea065",
      brightYellow: "#b87d00",
      brightBlue: "#3b82f6",
      brightMagenta: "#b44bb0",
      brightCyan: "#18949e",
      brightWhite: "#fafaf8",
    };
  }
  return {
    background: "#08090b",
    foreground: "#d9dde4",
    cursor: "#5fb3ff",
    selectionBackground: "rgba(95,179,255,0.25)",
    black: "#13161b",
    red: "#f06a6a",
    green: "#58c28a",
    yellow: "#e3b341",
    blue: "#5fb3ff",
    magenta: "#e07bd0",
    cyan: "#4fc1c9",
    white: "#d9dde4",
    brightBlack: "#5c6473",
    brightRed: "#ff8080",
    brightGreen: "#6ed99e",
    brightYellow: "#f0c85a",
    brightBlue: "#7ec4ff",
    brightMagenta: "#ea94dc",
    brightCyan: "#6fd4dc",
    brightWhite: "#ffffff",
  };
}

function ensureXtermCss(): void {
  if (document.getElementById("bp-xterm-css")) return;
  const link = document.createElement("link");
  link.id = "bp-xterm-css";
  link.rel = "stylesheet";
  link.href = "/assets/xterm.css";
  document.head.appendChild(link);
}

export function mountTerminalPane(
  stage: HTMLElement,
  api: ViewerHostApi,
  threadId: string,
  initialLight: boolean
): TerminalPaneHandle {
  ensureXtermCss();

  let alive = true;
  let activeKey = "";
  let knownKeys: string[] = [];
  let lastCols = 0;
  let lastRows = 0;
  let fitRaf = 0;

  const root = document.createElement("div");
  root.className = "bp-pane bp-term-pane";
  root.setAttribute("data-testid", "pane-terminal");

  const bar = document.createElement("div");
  bar.className = "bp-pane-bar";

  const left = document.createElement("div");
  left.className = "bp-term-tabs";
  const selectEl = document.createElement("select");
  selectEl.className = "bp-v-select";
  selectEl.setAttribute("aria-label", "Terminal session");
  selectEl.setAttribute("data-testid", "terminal-select");
  selectEl.hidden = true;
  const statusEl = document.createElement("span");
  statusEl.className = "bp-term-status";
  statusEl.setAttribute("data-testid", "terminal-status");
  statusEl.textContent = "Connecting…";
  left.append(selectEl, statusEl);

  const acts = document.createElement("div");
  acts.className = "bp-vh-actions";
  const newBtn = document.createElement("button");
  newBtn.type = "button";
  newBtn.className = "bp-v-btn";
  newBtn.textContent = "+ New";
  newBtn.setAttribute("data-testid", "terminal-new-btn");
  const stopBtn = document.createElement("button");
  stopBtn.type = "button";
  stopBtn.className = "bp-v-btn";
  stopBtn.textContent = "Stop";
  stopBtn.setAttribute("data-testid", "terminal-stop-btn");
  acts.append(newBtn, stopBtn);

  bar.append(left, acts);

  const host = document.createElement("div");
  host.className = "bp-term-host";
  host.setAttribute("data-testid", "terminal-host");

  root.append(bar, host);
  stage.replaceChildren(root);

  const term = new Terminal({
    cursorBlink: true,
    scrollback: 5000,
    fontSize: 12,
    fontFamily: '"Noto Sans Mono CJK JP", "JetBrains Mono", ui-monospace, Menlo, Consolas, monospace',
    theme: xtermTheme(initialLight),
    convertEol: false,
  });
  const fitAddon = new FitAddon();
  term.loadAddon(fitAddon);
  term.open(host);

  function syncSessionSelect(): void {
    selectEl.hidden = knownKeys.length <= 1;
    selectEl.replaceChildren();
    knownKeys.forEach((k, idx) => {
      const opt = document.createElement("option");
      opt.value = k;
      opt.textContent = k === threadId ? "Primary" : `Shell ${idx + 1}`;
      opt.selected = k === activeKey;
      selectEl.append(opt);
    });
  }

  function sendResize(): void {
    if (!alive) return;
    try {
      fitAddon.fit();
    } catch {
      return;
    }
    const cols = Math.max(20, term.cols || 80);
    const rows = Math.max(5, term.rows || 24);
    if (cols === lastCols && rows === lastRows) return;
    lastCols = cols;
    lastRows = rows;
    if (activeKey) {
      void api.hub.rpc("term.resize", { key: activeKey, cols, rows }).catch(() => {});
    }
  }

  function scheduleFit(): void {
    if (fitRaf) return;
    fitRaf = requestAnimationFrame(() => {
      fitRaf = 0;
      sendResize();
    });
  }

  const resizeObs = new ResizeObserver(() => scheduleFit());
  resizeObs.observe(host);

  // Key-by-key input + paste directly to hub `term.write` as base64 bytes.
  const dataDisp = term.onData((data) => {
    if (!alive || !activeKey || !data) return;
    void api.hub.rpc("term.write", { key: activeKey, b: encodeBase64Utf8(data) }).catch(() => {});
  });

  async function watchKey(key: string): Promise<void> {
    if (activeKey && activeKey !== key) {
      void api.hub.rpc("term.unwatch", { key: activeKey }).catch(() => {});
    }
    activeKey = key;
    if (!knownKeys.includes(key)) knownKeys.push(key);
    syncSessionSelect();
    statusEl.textContent = key;

    const snap = await api.hub.rpc<{ b?: string; reset?: unknown; state?: string }>("term.watch", { key });
    if (!alive || activeKey !== key) return;
    term.reset();
    if (typeof snap?.b === "string" && snap.b.length > 0) {
      term.write(decodeBase64Bytes(snap.b));
    }
    statusEl.textContent = snap?.state ? `${key} · ${snap.state}` : key;
    lastCols = 0;
    lastRows = 0;
    scheduleFit();
    term.focus();
  }

  async function openPrimary(): Promise<void> {
    try {
      fitAddon.fit();
    } catch {}
    const cols = Math.max(20, term.cols || 80);
    const rows = Math.max(5, term.rows || 24);
    statusEl.textContent = "Starting shell…";
    try {
      // Use term.new({ chat: threadId }) so closing & reopening reattaches the exact same primary shell.
      const res = await api.hub.rpc<{ key?: string }>("term.new", {
        chat: threadId,
        cols,
        rows,
      });
      if (!alive) return;
      const key = typeof res?.key === "string" && res.key ? res.key : threadId;
      await watchKey(key);
    } catch (err) {
      if (!alive) return;
      statusEl.textContent = err instanceof Error ? err.message : String(err);
    }
  }

  async function createExtraShell(): Promise<void> {
    try {
      fitAddon.fit();
    } catch {}
    const cols = Math.max(20, term.cols || 80);
    const rows = Math.max(5, term.rows || 24);
    statusEl.textContent = "Starting new shell…";
    try {
      const res = await api.hub.rpc<{ key?: string }>("term.new", {
        thread: threadId,
        cols,
        rows,
      });
      if (!alive) return;
      if (typeof res?.key === "string" && res.key) {
        await watchKey(res.key);
      }
    } catch (err) {
      if (!alive) return;
      statusEl.textContent = err instanceof Error ? err.message : String(err);
    }
  }

  selectEl.addEventListener("change", () => {
    if (selectEl.value && selectEl.value !== activeKey) {
      void watchKey(selectEl.value);
    }
  });
  newBtn.addEventListener("click", () => void createExtraShell());
  stopBtn.addEventListener("click", () => {
    if (!activeKey) return;
    const k = activeKey;
    void api.hub.rpc("term.stop", { key: k }).then(
      () => {
        if (alive && activeKey === k) statusEl.textContent = `${k} · stopped`;
      },
      () => {}
    );
  });

  const unsub = api.hub.onFrame((f) => {
    if (!alive || f.t !== "d" || f.k !== "term") return;
    if (f.key !== activeKey) return;
    if (f.reset) term.reset();
    if (typeof f.b === "string" && f.b.length > 0) {
      term.write(decodeBase64Bytes(f.b));
    }
    if (typeof f.state === "string" && f.state) {
      statusEl.textContent = `${activeKey} · ${f.state}`;
    }
  });

  void openPrimary();

  return {
    setTheme(light: boolean) {
      term.options.theme = xtermTheme(light);
    },
    destroy() {
      alive = false;
      if (fitRaf) cancelAnimationFrame(fitRaf);
      resizeObs.disconnect();
      dataDisp.dispose();
      unsub();
      term.dispose();
      if (activeKey) {
        void api.hub.rpc("term.unwatch", { key: activeKey }).catch(() => {});
      }
    },
  };
}
