// Unified Web Viewer Pane for Backplane: Board, Schematic, 3D (+ worker), Mech, Gerbers.
// Plugs into app.ts via `virtual:viewer-pane` (`mountViewerPane(el, api)`).
// Panel-C header: `page · file`, Layers, Fit, Follow agent, Close. No KiCad toggle, no Split pane, no compare kinds.
// Shared empty/loading/error states honor the hub's `retryable` flag and send `force: true` on Retry.

import { onLocaleChange, t } from "../i18n";
import type { ViewerHostApi } from "../app";
import { decodeCbor, decodePlot2DFrame, isPlotFrame, type CborValue, type PlotChunk } from "./cbor";
import { Plot2DViewer, type LayerMasks } from "./plot2d";
import { Solid3DViewer } from "./solid3d";

interface LayerRow {
  id: number;
  label: string;
  color?: string;
  on?: boolean;
  focus?: boolean;
}

interface LayersReply {
  own: number;
  focus: string;
  off: number;
  dim: number;
  lift: number;
  layerList?: LayerRow[];
  layerHead?: string;
  layerMode?: number;
  layerModes?: string[];
  layerReset?: boolean;
}

interface ViewerFilesReply {
  root?: string;
  pcb?: string;
  sch?: string;
  hasGerbers?: boolean;
}

function basename(path: string | undefined): string {
  if (!path) return "";
  const clean = path.replace(/\\/g, "/").replace(/\/+$/, "");
  const idx = clean.lastIndexOf("/");
  return idx >= 0 ? clean.slice(idx + 1) : clean;
}

function pageLabel(page: string): string {
  switch (page) {
    case "board": return t("viewer.board", "Board");
    case "schematic": return t("viewer.schematic", "Schematic");
    case "3d": return t("viewer.solid", "3D");
    case "gerbers": return t("gpui.gerbers", "Gerbers");
    case "mech": return t("viewer.mech", "Mech");
    default: return page ? page.charAt(0).toUpperCase() + page.slice(1) : "";
  }
}

export function mountViewerPane(container: HTMLElement, api: ViewerHostApi): {
  open: (kind: string, threadId: string) => void;
  close: () => void;
} {
  const header = document.createElement("div");
  header.className = "bp-vh";

  const titleWrap = document.createElement("div");
  titleWrap.className = "bp-vh-title";
  const pageNameEl = document.createElement("span");
  pageNameEl.className = "bp-vh-page";
  const fileSepEl = document.createElement("span");
  fileSepEl.className = "bp-vh-sep";
  fileSepEl.textContent = "·";
  fileSepEl.hidden = true;
  const fileNameEl = document.createElement("span");
  fileNameEl.className = "bp-vh-file";
  fileNameEl.hidden = true;
  titleWrap.append(pageNameEl, fileSepEl, fileNameEl);

  const controlsEl = document.createElement("div");
  controlsEl.className = "bp-vh-actions";

  header.append(titleWrap, controlsEl);

  const bannerEl = document.createElement("div");
  bannerEl.className = "bp-v-banner";
  bannerEl.hidden = true;

  const stageWrap = document.createElement("div");
  stageWrap.className = "bp-v-stage-wrap";

  const stage = document.createElement("div");
  stage.className = "bp-v-stage";

  const overlayEl = document.createElement("div");
  overlayEl.className = "bp-v-overlay";
  overlayEl.hidden = true;

  const layersPop = document.createElement("div");
  layersPop.className = "bp-v-layers";
  layersPop.hidden = true;

  const infoEl = document.createElement("div");
  infoEl.className = "bp-v-info";
  infoEl.hidden = true;

  stageWrap.append(stage, overlayEl, layersPop, infoEl);
  container.replaceChildren(header, bannerEl, stageWrap);

  let threadId = "";
  let page = "";
  let watchGen = 0;
  let activeKeys = new Set<string>();
  let heldChunks = new Map<string, PlotChunk[]>();
  let filesInfo: ViewerFilesReply = {};
  let mechParts: string[] = [];
  let selectedMech = "";

  // Follow agent state
  let followOn = false;
  let followPaused = false;
  let followSeq = 0;

  // Hub-driven Layers state
  let layersOpen = false;
  let layerOwn = 0;
  let layerFocus = "";
  let layerPresent = 0;
  let lastLayers: LayersReply | null = null;

  let plot2d: Plot2DViewer | null = null;
  let solid3d: Solid3DViewer | null = null;

  function isLight(): boolean {
    return document.documentElement.classList.contains("light");
  }

  const themeObs = new MutationObserver(() => {
    const light = isLight();
    plot2d?.setTheme(light);
    solid3d?.setTheme(light);
  });
  themeObs.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] });

  function setOverlay(mode: "none" | "loading" | "empty" | "error", text = "", retryable = false): void {
    if (mode === "none") {
      overlayEl.hidden = true;
      overlayEl.replaceChildren();
      return;
    }
    overlayEl.hidden = false;
    overlayEl.className = `bp-v-overlay is-${mode}`;
    const card = document.createElement("div");
    card.className = "bp-v-state-card";
    const msg = document.createElement("div");
    msg.className = "bp-v-state-msg";
    msg.textContent = text;
    card.append(msg);
    if (retryable) {
      const retryBtn = document.createElement("button");
      retryBtn.type = "button";
      retryBtn.className = "bp-v-btn is-primary";
      retryBtn.textContent = t("tools.retry", "Retry");
      retryBtn.addEventListener("click", () => void startWatch(true));
      card.append(retryBtn);
    }
    overlayEl.replaceChildren(card);
  }

  function setBanner(note: string, warn: boolean): void {
    bannerEl.textContent = note;
    bannerEl.hidden = !note;
    bannerEl.classList.toggle("is-warn", warn);
  }

  function currentFileName(): string {
    if (page === "board" || page === "3d") return basename(filesInfo.pcb);
    if (page === "schematic") return basename(filesInfo.sch);
    if (page === "mech") return basename(selectedMech);
    return "";
  }

  function renderHeader(): void {
    pageNameEl.textContent = pageLabel(page);
    const fname = currentFileName();
    fileSepEl.hidden = !fname;
    fileNameEl.hidden = !fname;
    fileNameEl.textContent = fname;

    controlsEl.replaceChildren();

    if (page === "mech" && mechParts.length > 1) {
      const sel = document.createElement("select");
      sel.className = "bp-v-select";
      sel.setAttribute("aria-label", t("viewer.mech", "Mech"));
      for (const p of mechParts) {
        const opt = document.createElement("option");
        opt.value = p;
        opt.textContent = basename(p);
        opt.selected = p === selectedMech;
        sel.append(opt);
      }
      sel.addEventListener("change", () => {
        selectedMech = sel.value;
        renderHeader();
        void startWatch(false);
      });
      controlsEl.append(sel);
    }

    if (page === "board" || page === "schematic") {
      const layersBtn = document.createElement("button");
      layersBtn.type = "button";
      layersBtn.className = "bp-v-btn" + (layersOpen ? " is-on" : "");
      layersBtn.textContent = lastLayers?.layerHead || "Layers";
      layersBtn.setAttribute("data-testid", "viewer-layers-btn");
      layersBtn.addEventListener("click", () => {
        layersOpen = !layersOpen;
        layersPop.hidden = !layersOpen;
        layersBtn.classList.toggle("is-on", layersOpen);
        if (layersOpen) void syncLayers("", "");
      });
      controlsEl.append(layersBtn);
    }

    if (page === "board" || page === "schematic" || page === "3d" || page === "mech" || page === "gerbers") {
      const fitBtn = document.createElement("button");
      fitBtn.type = "button";
      fitBtn.className = "bp-v-btn";
      fitBtn.textContent = "Fit";
      fitBtn.setAttribute("data-testid", "viewer-fit-btn");
      fitBtn.addEventListener("click", () => {
        plot2d?.fit();
        solid3d?.fit();
      });
      controlsEl.append(fitBtn);
    }

    if ((page === "board" || page === "schematic") && api.hub.hasFeature("follow")) {
      const followBtn = document.createElement("button");
      followBtn.type = "button";
      followBtn.className = "bp-v-btn" + (followOn && !followPaused ? " is-on" : "");
      followBtn.textContent = followOn
        ? followPaused
          ? t("gpui.follow.paused", "Follow (paused)")
          : "Follow agent"
        : "Follow agent";
      followBtn.setAttribute("data-testid", "viewer-follow-btn");
      followBtn.addEventListener("click", () => void toggleFollow());
      controlsEl.append(followBtn);
    }

    const closeBtn = document.createElement("button");
    closeBtn.type = "button";
    closeBtn.className = "bp-v-btn bp-v-close";
    closeBtn.textContent = "✕";
    closeBtn.title = t("common.close", "Close");
    closeBtn.setAttribute("aria-label", t("common.close", "Close"));
    closeBtn.setAttribute("data-testid", "viewer-close-btn");
    closeBtn.addEventListener("click", () => api.closeViewer());
    controlsEl.append(closeBtn);
  }

  function renderLayersPopover(): void {
    if (!lastLayers) {
      layersPop.replaceChildren();
      return;
    }
    const frag = document.createDocumentFragment();

    const head = document.createElement("div");
    head.className = "bp-v-layers-head";
    const title = document.createElement("span");
    title.textContent = lastLayers.layerHead || "Layers";
    head.append(title);
    if (lastLayers.layerReset) {
      const resetBtn = document.createElement("button");
      resetBtn.type = "button";
      resetBtn.className = "bp-v-btn";
      resetBtn.textContent = "Reset";
      resetBtn.addEventListener("click", () => void syncLayers("reset", ""));
      head.append(resetBtn);
    }
    frag.append(head);

    if (Array.isArray(lastLayers.layerModes) && lastLayers.layerModes.length > 0) {
      const modes = document.createElement("div");
      modes.className = "bp-v-layers-modes";
      lastLayers.layerModes.forEach((lbl, idx) => {
        const b = document.createElement("button");
        b.type = "button";
        b.className = "bp-v-btn" + (lastLayers?.layerMode === idx ? " is-on" : "");
        b.textContent = lbl;
        b.addEventListener("click", () => void syncLayers("mode", String(idx)));
        modes.append(b);
      });
      frag.append(modes);
    }

    const list = document.createElement("div");
    list.className = "bp-v-layers-list";
    for (const row of lastLayers.layerList || []) {
      const item = document.createElement("div");
      item.className = "bp-v-layer-row" + (row.focus ? " is-focus" : "");

      const chk = document.createElement("input");
      chk.type = "checkbox";
      chk.checked = row.on !== false;
      chk.addEventListener("change", () => void syncLayers("toggle", String(row.id)));

      const sw = document.createElement("span");
      sw.className = "bp-v-layer-swatch";
      if (row.color) sw.style.background = row.color;

      const lbl = document.createElement("button");
      lbl.type = "button";
      lbl.className = "bp-v-layer-name";
      lbl.textContent = row.label;
      lbl.addEventListener("click", () => void syncLayers("pick", String(row.id)));

      item.append(chk, sw, lbl);
      list.append(item);
    }
    frag.append(list);
    layersPop.replaceChildren(frag);
  }

  async function syncLayers(action: string, value: string): Promise<void> {
    if (page !== "board" && page !== "schematic") return;
    const gen = watchGen;
    try {
      const res = await api.hub.rpc<LayersReply>("viewer.layers", {
        kind: page,
        present: layerPresent,
        off: layerOwn,
        focus: layerFocus,
        ...(action ? { action, value } : {}),
      });
      if (gen !== watchGen || !res) return;
      layerOwn = Number(res.own) || 0;
      layerFocus = typeof res.focus === "string" ? res.focus : "";
      lastLayers = res;
      const masks: LayerMasks = {
        off: Number(res.off) || 0,
        dim: Number(res.dim) || 0,
        lift: Number(res.lift) || 0,
      };
      plot2d?.setLayers(masks);
      if (layersOpen) renderLayersPopover();
    } catch {
      // ignore transient layer errors
    }
  }

  async function pauseFollowOnUserMove(): Promise<void> {
    if (!followOn || followPaused || !threadId || !api.hub.hasFeature("follow")) return;
    followPaused = true;
    renderHeader();
    try {
      await api.hub.rpc("viewer.follow", { thread: threadId, action: "pause" });
    } catch {}
  }

  async function toggleFollow(): Promise<void> {
    if (!threadId || !api.hub.hasFeature("follow")) return;
    const action = !followOn ? "on" : followPaused ? "resume" : "off";
    try {
      const res = await api.hub.rpc<{ on?: boolean; paused?: boolean; seq?: number; focus?: { box?: number[] } }>(
        "viewer.follow",
        { thread: threadId, action }
      );
      followOn = !!res.on;
      followPaused = !!res.paused;
      renderHeader();
      if (res.on && !res.paused && res.focus && Array.isArray(res.focus.box) && res.focus.box.length === 4 && (res.seq || 0) > followSeq) {
        followSeq = res.seq || followSeq;
        plot2d?.zoomToBoxMm([res.focus.box[0], res.focus.box[1], res.focus.box[2], res.focus.box[3]]);
      }
    } catch {}
  }

  async function onPick2D(hit: { at: number; idx: number } | null): Promise<void> {
    if (!hit) {
      infoEl.hidden = true;
      plot2d?.setHighlights([]);
      return;
    }
    const key = Array.from(activeKeys)[0] || "";
    if (!key) return;
    try {
      const res = await api.hub.rpc<{ info?: string }>("viewer.info", { key, at: hit.at, idx: hit.idx });
      const info = typeof res.info === "string" ? res.info : "";
      if (!info) {
        infoEl.hidden = true;
        return;
      }
      infoEl.textContent = info;
      infoEl.hidden = false;
      const netMatch = info.match(/net=([^;]+)/);
      if (netMatch && netMatch[1]) {
        const probe = await api.hub.rpc<{ matches?: [number, number][] }>("viewer.probe", {
          key,
          name: "net",
          value: netMatch[1],
        });
        if (Array.isArray(probe.matches)) plot2d?.setHighlights(probe.matches);
      }
    } catch {
      infoEl.hidden = true;
    }
  }

  function clearStage(): void {
    plot2d?.destroy();
    plot2d = null;
    solid3d?.destroy();
    solid3d = null;
    stage.replaceChildren();
    layersOpen = false;
    layersPop.hidden = true;
    infoEl.hidden = true;
    setBanner("", false);
    setOverlay("none");
  }

  async function startWatch(force: boolean): Promise<void> {
    if (!threadId || !page) return;
    const gen = ++watchGen;
    clearStage();
    renderHeader();
    setOverlay("loading", "Loading…");

    // Fetch project viewer metadata once per thread
    if (!filesInfo.root) {
      try {
        const vf = await api.hub.rpc<ViewerFilesReply>("viewer.files", { thread: threadId });
        if (gen !== watchGen) return;
        filesInfo = vf || {};
        renderHeader();
      } catch {}
    }

    if (page === "gerbers" && filesInfo.hasGerbers === false) {
      setOverlay("empty", "No Gerbers in this project", false);
      return;
    }

    const light = isLight();
    if (page === "3d" || page === "mech") {
      solid3d = new Solid3DViewer(stage, {
        onStatus: (msg) => {
          if (msg) setOverlay("loading", msg);
          else setOverlay("none");
        },
        onUserMove: () => void pauseFollowOnUserMove(),
      });
      solid3d.setTheme(light);
    } else if (page === "board" || page === "schematic" || page === "gerbers") {
      plot2d = new Plot2DViewer(stage, {
        onPick: (hit) => void onPick2D(hit),
        onUserMove: () => void pauseFollowOnUserMove(),
      });
      plot2d.setTheme(light);
    } else {
      setOverlay("empty", `${pageLabel(page)} is not available in this viewer`, false);
      return;
    }

    const params: Record<string, unknown> = { thread: threadId };
    if (force) params.force = true;

    if (page === "mech") {
      if (mechParts.length === 0) {
        try {
          const ml = await api.hub.rpc<{ parts?: string[]; primary?: string }>("mech.list", { thread: threadId });
          if (gen !== watchGen) return;
          mechParts = Array.isArray(ml.parts) ? ml.parts : [];
          selectedMech = ml.primary || mechParts[0] || "";
          renderHeader();
        } catch (err) {
          if (gen !== watchGen) return;
          setOverlay("error", err instanceof Error ? err.message : String(err), true);
          return;
        }
      }
      if (!selectedMech) {
        setOverlay("empty", t("gpui.rail.no.mech", "No mechanical parts in this project"), false);
        return;
      }
      params.kind = "3d";
      params.path = selectedMech;
    } else {
      params.kind = page; // "board" | "schematic" | "3d" | "gerbers"
      if (page === "3d" && !force && solid3d) {
        const kept = solid3d.keptToken();
        if (kept) params.kept = kept;
      }
    }

    try {
      const res = await api.hub.rpc<{ keys?: string[] }>("viewer.watch", params);
      if (gen !== watchGen) return;
      activeKeys = new Set(Array.isArray(res.keys) ? res.keys : []);
    } catch (err) {
      if (gen !== watchGen) return;
      const msg = err instanceof Error ? err.message : String(err);
      const permanent = /no gerbers|not in this project|another machine/i.test(msg);
      setOverlay(permanent ? "empty" : "error", msg, !permanent);
    }
  }

  function handlePlotNone(key: string, none: string, retryable: boolean): void {
    if (activeKeys.size > 0 && !activeKeys.has(key)) return;
    setOverlay(retryable ? "error" : "empty", none || "Nothing to display", retryable);
  }

  function handleBinary(buf: ArrayBuffer): void {
    if (!page) return;
    const bytes = new Uint8Array(buf);
    if (!isPlotFrame(bytes)) return;
    const decoded = decodeCbor(bytes);
    if (!decoded || typeof decoded !== "object" || Array.isArray(decoded) || decoded instanceof Uint8Array) return;
    const map = decoded as Record<string, CborValue>;
    if (map.t !== "plot") return;
    const key = typeof map.key === "string" ? map.key : "";
    if (!key) return;
    if (activeKeys.size > 0 && !activeKeys.has(key)) return;

    if (typeof map.none === "string") {
      handlePlotNone(key, map.none, map.retryable === 1 || (map.retryable as unknown) === true);
      return;
    }

    if (map.mesh instanceof Uint8Array || map.parts) {
      solid3d?.handleFrame(map);
      return;
    }

    const prev = heldChunks.get(key) || [];
    const frame = decodePlot2DFrame(map, prev);
    heldChunks.set(key, frame.chunks);
    setOverlay("none");
    setBanner(frame.note, frame.warn);

    if (plot2d) {
      plot2d.setFrame(frame);
      const present = plot2d.presentMask();
      if (present !== layerPresent && (page === "board" || page === "schematic")) {
        layerPresent = present;
        void syncLayers("", "");
      }
    }
  }

  const unsubBinary = api.hub.onBinary(handleBinary);
  const unsubFrame = api.hub.onFrame((f) => {
    if (f.t === "plot" && typeof f.none === "string" && typeof f.key === "string") {
      handlePlotNone(f.key, f.none, f.retryable === true);
    }
  });
  const unsubLocale = onLocaleChange(() => {
    if (page) renderHeader();
  });

  return {
    open(kind: string, nextThread: string) {
      if (nextThread !== threadId) {
        threadId = nextThread;
        filesInfo = {};
        mechParts = [];
        selectedMech = "";
        heldChunks.clear();
        layerOwn = 0;
        layerFocus = "";
        layerPresent = 0;
        lastLayers = null;
        if (api.hub.hasFeature("follow")) {
          void api.hub
            .rpc<{ on?: boolean; paused?: boolean; seq?: number }>("viewer.follow", { thread: threadId })
            .then((f) => {
              followOn = !!f.on;
              followPaused = !!f.paused;
              followSeq = Number(f.seq) || 0;
              if (page) renderHeader();
            })
            .catch(() => {});
        }
      }
      page = kind;
      void startWatch(false);
    },
    close() {
      watchGen++;
      if (threadId) {
        void api.hub.rpc("viewer.watch", { thread: threadId, kind: "" }).catch(() => {});
      }
      page = "";
      activeKeys.clear();
      clearStage();
      // Keep listeners alive for subsequent opens; app.ts keeps the mounted controller for the session.
      void unsubBinary;
      void unsubFrame;
      void unsubLocale;
      void themeObs;
    },
  };
}
