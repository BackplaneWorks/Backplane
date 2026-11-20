// Hub Browser pane for the Backplane web client (phone parity: mobile/ios/Backplane/DataViewer.swift).
// Displays GET /bots/<thread>/screen.jpg (same-origin cookie, cache: "no-store"), refetched on
// `{"t":"shot","bot":<thread>,"n":N}` pushes, and drives the page via the hub's `browser` RPC.

import type { ViewerHostApi } from "../app";
import type { PaneHandle } from "./panes";
import {
  BROWSER_KEY_BUTTONS,
  mapClickToPageCoords,
  normalizeBrowserUrl,
  SerialQueue,
  type BrowserToolbarCommand,
} from "./browser-input";

export function mountBrowserPane(
  stage: HTMLElement,
  api: ViewerHostApi,
  threadId: string
): PaneHandle {
  let alive = true;
  let driving = false;
  let hasFrame = false;
  let objectUrl = "";
  let fetching = false;
  let fetchAgain = false;
  let abortCtrl: AbortController | null = null;
  const queue = new SerialQueue();

  const root = document.createElement("div");
  root.className = "bp-pane bp-browser-pane";
  root.setAttribute("data-testid", "pane-browser");

  // Top navigation + takeover toolbar
  const navBar = document.createElement("div");
  navBar.className = "bp-pane-bar bp-browser-nav";

  function mkBtn(label: string, testId: string, title?: string): HTMLButtonElement {
    const b = document.createElement("button");
    b.type = "button";
    b.className = "bp-v-btn";
    b.textContent = label;
    b.setAttribute("data-testid", testId);
    if (title) b.title = title;
    return b;
  }

  const backBtn = mkBtn("←", "browser-back", "Back");
  const fwdBtn = mkBtn("→", "browser-forward", "Forward");
  const reloadBtn = mkBtn("↻", "browser-reload", "Reload");

  const urlForm = document.createElement("form");
  urlForm.className = "bp-browser-url-form";
  const urlInput = document.createElement("input");
  urlInput.type = "text";
  urlInput.className = "bp-browser-input";
  urlInput.placeholder = "https://…";
  urlInput.autocomplete = "off";
  urlInput.spellcheck = false;
  urlInput.setAttribute("data-testid", "browser-url");
  const goBtn = mkBtn("Go", "browser-go", "Navigate");
  urlForm.append(urlInput, goBtn);

  const driveBtn = mkBtn("Take over", "browser-takeover");
  driveBtn.classList.add("is-primary");

  navBar.append(backBtn, fwdBtn, reloadBtn, urlForm, driveBtn);

  // Driving controls bar (key input + quick keys), shown only when driving
  const driveBar = document.createElement("div");
  driveBar.className = "bp-browser-drive-bar";
  driveBar.hidden = true;

  const keysRow = document.createElement("div");
  keysRow.className = "bp-browser-keys-row";
  const keysInput = document.createElement("input");
  keysInput.type = "text";
  keysInput.className = "bp-browser-input";
  keysInput.placeholder = "Type text or press Enter/Tab/Arrows…";
  keysInput.autocomplete = "off";
  keysInput.spellcheck = false;
  keysInput.setAttribute("data-testid", "browser-keys");
  const sendTextBtn = mkBtn("Send", "browser-type-send");
  keysRow.append(keysInput, sendTextBtn);

  const quickRow = document.createElement("div");
  quickRow.className = "bp-browser-quick-keys";
  for (const [label, keyName] of BROWSER_KEY_BUTTONS) {
    const kb = mkBtn(label, `browser-key-${keyName}`);
    kb.addEventListener("click", () => runOp("press", { key: keyName }));
    quickRow.append(kb);
  }
  driveBar.append(keysRow, quickRow);

  // Error bar
  const errEl = document.createElement("div");
  errEl.className = "bp-browser-err";
  errEl.setAttribute("data-testid", "browser-error");
  errEl.hidden = true;

  // Screen stage
  const frameWrap = document.createElement("div");
  frameWrap.className = "bp-browser-stage";
  frameWrap.setAttribute("data-testid", "browser-stage");

  const img = document.createElement("img");
  img.className = "bp-browser-img";
  img.alt = "Hub browser screen";
  img.draggable = false;
  img.hidden = true;
  img.setAttribute("data-testid", "browser-frame");

  const emptyEl = document.createElement("div");
  emptyEl.className = "bp-pane-empty";
  emptyEl.setAttribute("data-testid", "browser-empty");
  emptyEl.textContent = "Loading browser screen…";

  frameWrap.append(img, emptyEl);

  // Driving hint footer
  const hintEl = document.createElement("div");
  hintEl.className = "bp-browser-hint";
  hintEl.textContent =
    "You have the page: click to tap, scroll to wheel, type in the key box. Hand it back when done.";
  hintEl.hidden = true;

  root.append(navBar, driveBar, errEl, frameWrap, hintEl);
  stage.replaceChildren(root);

  function setError(msg: string): void {
    errEl.textContent = msg;
    errEl.hidden = !msg;
  }

  function syncDrivingUi(): void {
    driveBar.hidden = !driving;
    hintEl.hidden = !driving;
    driveBtn.textContent = driving ? "Hand back" : "Take over";
    driveBtn.setAttribute("data-testid", driving ? "browser-handback" : "browser-takeover");
    driveBtn.classList.toggle("is-on", driving);
    img.classList.toggle("is-driving", driving);
  }

  function runOp(op: string, extra: Record<string, unknown> = {}, onOk?: () => void): void {
    void queue.enqueue(async () => {
      if (!alive) return;
      setError("");
      try {
        await api.hub.rpc("browser", { thread: threadId, op, ...extra });
        if (!alive) return;
        onOk?.();
      } catch (err) {
        if (!alive) return;
        setError(err instanceof Error ? err.message : String(err));
      }
    });
  }

  function sendToolbar(command: BrowserToolbarCommand): void {
    runOp("toolbar", { command }, () => {
      driving = true;
      syncDrivingUi();
    });
  }

  backBtn.addEventListener("click", () => sendToolbar("back"));
  fwdBtn.addEventListener("click", () => sendToolbar("forward"));
  reloadBtn.addEventListener("click", () => sendToolbar("reload"));

  urlForm.addEventListener("submit", (e) => {
    e.preventDefault();
    const url = normalizeBrowserUrl(urlInput.value);
    if (!url) return;
    urlInput.value = url;
    runOp("navigate", { url }, () => {
      driving = true;
      syncDrivingUi();
    });
  });

  driveBtn.addEventListener("click", () => {
    if (driving) {
      runOp("handback", {}, () => {
        driving = false;
        syncDrivingUi();
      });
    } else {
      runOp("takeover", {}, () => {
        driving = true;
        syncDrivingUi();
        keysInput.focus();
      });
    }
  });

  function flushTypedText(): void {
    const text = keysInput.value;
    if (!text) return;
    keysInput.value = "";
    runOp("type", { text });
  }

  sendTextBtn.addEventListener("click", flushTypedText);
  keysInput.addEventListener("keydown", (e) => {
    if (!driving) return;
    if (e.key === "Enter") {
      e.preventDefault();
      if (keysInput.value) {
        flushTypedText();
      }
      runOp("press", { key: "Enter" });
      return;
    }
    if (
      e.key === "Tab" ||
      e.key === "Escape" ||
      e.key === "ArrowUp" ||
      e.key === "ArrowDown" ||
      e.key === "ArrowLeft" ||
      e.key === "ArrowRight" ||
      (e.key === "Backspace" && keysInput.value === "")
    ) {
      e.preventDefault();
      runOp("press", { key: e.key });
    }
  });

  img.addEventListener("click", (e) => {
    if (!driving || !hasFrame) return;
    const rect = img.getBoundingClientRect();
    const coords = mapClickToPageCoords(e.clientX, e.clientY, {
      left: rect.left,
      top: rect.top,
      width: rect.width,
      height: rect.height,
      naturalWidth: img.naturalWidth,
      naturalHeight: img.naturalHeight,
    });
    if (!coords) return;
    runOp("click", coords);
  });

  img.addEventListener(
    "wheel",
    (e) => {
      if (!driving || !hasFrame) return;
      e.preventDefault();
      const dx = Math.round(e.deltaX);
      const dy = Math.round(e.deltaY);
      if (dx === 0 && dy === 0) return;
      runOp("scroll", { dx, dy });
    },
    { passive: false }
  );

  async function loadFrame(): Promise<void> {
    if (!alive) return;
    if (fetching) {
      fetchAgain = true;
      return;
    }
    fetching = true;
    try {
      do {
        fetchAgain = false;
        abortCtrl = new AbortController();
        const res = await fetch(`/bots/${encodeURIComponent(threadId)}/screen.jpg`, {
          credentials: "same-origin",
          cache: "no-store",
          signal: abortCtrl.signal,
        });
        if (!alive) return;
        if (res.ok) {
          const blob = await res.blob();
          if (!alive) return;
          const nextUrl = URL.createObjectURL(blob);
          const prev = objectUrl;
          objectUrl = nextUrl;
          img.src = nextUrl;
          img.hidden = false;
          emptyEl.hidden = true;
          hasFrame = true;
          if (prev) URL.revokeObjectURL(prev);
        } else if (!hasFrame) {
          img.hidden = true;
          emptyEl.hidden = false;
          emptyEl.textContent = "No browser page yet. An agent opens one when it needs the web.";
        }
      } while (alive && fetchAgain);
    } catch {
      if (!alive) return;
      if (!hasFrame) {
        img.hidden = true;
        emptyEl.hidden = false;
        emptyEl.textContent = "No browser page yet. An agent opens one when it needs the web.";
      }
    } finally {
      abortCtrl = null;
      fetching = false;
    }
  }

  const unsub = api.hub.onFrame((f) => {
    if (!alive) return;
    if (f.t === "shot" && f.bot === threadId) {
      void loadFrame();
    }
  });

  const pollTimer = window.setInterval(() => {
    if (alive) void loadFrame();
  }, 2000);

  void loadFrame();

  return {
    destroy() {
      alive = false;
      window.clearInterval(pollTimer);
      unsub();
      abortCtrl?.abort();
      if (objectUrl) {
        URL.revokeObjectURL(objectUrl);
        objectUrl = "";
      }
    },
  };
}
