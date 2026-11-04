// Inside the phone apps (docs/phone-app-shell.md): the page uses
// window.backplaneApp, the bridge the Android and iOS apps inject, where it
// is there; in a browser nothing here runs.
// - a tapped notification ("open-thread") opens its thread;
// - after each render the app hears which thread is on screen, how many
//   threads need the person and the page's theme (setState), so it raises
//   no alert for the thread on show and draws its bars in the page's colours;
// - Settings' "App settings" ([data-ph-app]) opens the app's own settings;
// - something shared into the app ("share-in") waits for a thread, then goes
//   into its composer (text) and attaches (files);
// - a hub that refuses this phone's token tells the app (pairingFailed);
// - the hub's desktop notifications for this page (the desk used last: a
//   thread that finished or needs the person) go to the app, which posts
//   them as its own alerts (a WebView has no Notification API).

export function appBridge(win = globalThis.window) {
  const app = win?.backplaneApp;
  return app && app.bridge === 1 && typeof app.on === "function" ? app : null;
}

// what the app is told after a render: the thread on show ("" on a list
// or Settings), the Now tab's need-you count, the theme and background
export function shellState(doc, sel) {
  const page = doc.querySelector(".app.phone");
  const screen = page?.getAttribute("data-ph-screen") || "";
  const shows = screen === "thread" || screen === "viewer";
  const badge = doc.querySelector(".app.phone .ph-tabs .ph-tab[data-value=now] .ph-badge");
  const needs = badge ? parseInt(badge.textContent || "0", 10) || 0 : null;
  const html = doc.documentElement;
  const bg = (doc.defaultView?.getComputedStyle?.(html)?.getPropertyValue("--bg") || "").trim();
  return { thread: shows ? String(sel || "") : "", ...(needs === null ? {} : { needs }), theme: html.classList.contains("light") ? "light" : "dark", ...(bg ? { bg } : {}) };
}

function base64Bytes(b64) {
  const s = atob(b64), out = new Uint8Array(s.length);
  for (let i = 0; i < s.length; i++) out[i] = s.charCodeAt(i);
  return out;
}

export function installAppShell({ win = window, doc = document, dispatch, selected, attach }) {
  const app = appBridge(win);
  if (!app) return { app: null, rendered() {}, pairingFailed() {}, notify: () => false };
  const html = doc.documentElement;
  html.classList.add("ph-in-app");
  const has = (f) => Array.isArray(app.features) && app.features.includes(f);

  app.on("open-thread", ({ thread }) => { if (typeof thread === "string" && thread) dispatch("select", thread); });

  doc.addEventListener("click", (e) => {
    const b = e.target.closest?.("[data-ph-app]");
    if (!b) return;
    e.preventDefault();
    if (b.getAttribute("data-ph-app") === "settings" && has("settings")) app.openAppSettings();
  });

  // shared into the app: held (with a line saying so) until a thread is on screen
  let shared = null, line = null;
  const showLine = () => {
    if (!shared) { line?.remove();line = null;return; }
    if (!line) {
      line = doc.createElement("div");line.className = "ph-share-wait";line.setAttribute("role", "status");
      const words = doc.createElement("span");
      const cancel = doc.createElement("button");cancel.type = "button";cancel.textContent = "Cancel";
      cancel.addEventListener("click", () => { shared = null;showLine(); });
      line.append(words, cancel);doc.body.append(line);
    }
    const n = shared.files.length;
    line.firstChild.textContent = "Open a thread to add " + (n ? n + (n === 1 ? " file" : " files") + (shared.text ? " and the text" : "") : "the shared text");
  };
  const deliver = () => {
    const field = doc.querySelector(".app.phone #composer");
    if (!shared || !field) return false;
    const { text, files } = shared;
    shared = null;showLine();
    if (text) {
      field.value = field.value ? field.value.replace(/\s*$/, "") + "\n" + text : text;
      field.dispatchEvent(new Event("input", { bubbles: true }));
    }
    if (files.length) attach?.(files);
    return true;
  };
  app.on("share-in", ({ text, files }) => {
    const list = (Array.isArray(files) ? files : []).map(f => {
      try { return new File([base64Bytes(String(f.data || ""))], String(f.name || "shared"), { type: String(f.type || "application/octet-stream") }); } catch { return null; }
    }).filter(Boolean);
    if (!text && !list.length) return;
    shared = { text: String(text || ""), files: list };
    if (!deliver()) showLine();
  });

  let told = "";
  return {
    app,
    rendered() {
      const s = shellState(doc, selected());
      const k = JSON.stringify(s);
      if (k !== told) { told = k;app.setState(s); }
      if (shared && s.thread) deliver();
    },
    pairingFailed(cause) { app.pairingFailed(cause); },
    // a Notify command (core/desk.bend's desktop notification): the app's to show
    notify(c) {
      if (!has("notify") || typeof app.notify !== "function") return false;
      app.notify({ title: String(c.title || ""), body: String(c.body || ""), thread: String(c.thread || ""), key: String(c.key || ""), kind: String(c.kind || "") });
      return true;
    },
  };
}
