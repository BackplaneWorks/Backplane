// Unified Web Viewer Pane for Backplane: Board, Schematic, 3D (+ worker), Mech, Gerbers.
// Plugs into app.ts via `virtual:viewer-pane` (`mountViewerPane(el, api)`).
// Panel-C header: `page · file`, Layers, Fit, Follow agent, Close. No KiCad toggle, no Split pane, no compare kinds.
// Shared empty/loading/error states honor the hub's `retryable` flag and send `force: true` on Retry.

import { onLocaleChange, t } from "../i18n";
import type { ViewerHostApi } from "../app";
import { decodeCbor, decodePlot2DFrame, isPlotFrame, type CborValue, type PlotChunk } from "./cbor";
import { mountBrowserPane } from "./browser";
import { mountDiffPane, mountFilesPane, mountReviewPane, type PaneHandle } from "./panes";
import { Plot2DViewer, type LayerMasks } from "./plot2d";
import { Solid3DViewer } from "./solid3d";

interface LayerRow {
  layer: number;
  name: string;
  on?: boolean;
  active?: boolean;
}

interface LayersReply {
  own: number;
  focus: string;
  off: number;
  dim: number;
  lift: number;
  layerList?: LayerRow[];
  layerMarks?: LayerRow[];
  layerHead?: string;
  layerMode?: number;
  layerModes?: boolean;
  layerReset?: boolean;
}

type PendingFrame =
  | { kind: "bin"; buf: ArrayBuffer }
  | { kind: "none"; key: string; none: string; retryable: boolean };

const MAX_PENDING_FRAMES = 64;
const MAX_PENDING_BYTES = 16 << 20;

interface ViewerFilesReply {
  root?: string;
  pcb?: string;
  sch?: string;
  hasGerbers?: boolean;
}

export interface RailHas {
  board?: boolean;
  schematic?: boolean;
  mech?: boolean;
  gerbers?: boolean;
}

function pageHasBacking(page: string, has: RailHas): boolean {
  switch (page) {
    case "board":
    case "3d":
      return has.board !== false;
    case "schematic":
      return has.schematic !== false;
    case "mech":
      return has.mech !== false;
    case "gerbers":
      return has.gerbers !== false;
    default:
      return true;
  }
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
    case "files": return t("viewer.files", "Files");
    case "diff": return t("viewer.diff", "Diff");
    case "review": return t("viewer.review", "Review");
    case "browser": return t("bots.tab.browser", "Browser");
    case "terminal": return t("viewer.terminal", "Terminal");
    default: return page ? page.charAt(0).toUpperCase() + page.slice(1) : "";
  }
}

function isLayerPage(p: string): boolean {
  return p === "board" || p === "schematic" || p === "gerbers";
}

function isNonPlotPane(p: string): boolean {
  return p === "files" || p === "diff" || p === "review" || p === "browser" || p === "terminal";
}

export function mountViewerPane(container: HTMLElement, api: ViewerHostApi): {
  open: (kind: string, threadId: string) => void;
  close: () => void;
  reconcileHas: (has: RailHas) => void;
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
  let watchPending = false;
  let activeKeys = new Set<string>();
  let pendingQueue: PendingFrame[] = [];
  let pendingBytes = 0;
  let heldChunks = new Map<string, PlotChunk[]>();
  let filesInfo: ViewerFilesReply = {};
  let mechParts: string[] = [];
  let selectedMech = "";
  let selectedPath = "";

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
  let activePane: PaneHandle | null = null;
  let activeTerm: { setTheme(light: boolean): void; destroy(): void } | null = null;

  function isLight(): boolean {
    return document.documentElement.classList.contains("light");
  }

  const themeObs = new MutationObserver(() => {
    const light = isLight();
    plot2d?.setTheme(light);
    solid3d?.setTheme(light);
    activeTerm?.setTheme(light);
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
    if (selectedPath) return basename(selectedPath);
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

    if (isLayerPage(page)) {
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

    if (lastLayers.layerModes) {
      const modes = document.createElement("div");
      modes.className = "bp-v-layers-modes";
      ["All", "Dim", "Only"].forEach((lbl, idx) => {
        const b = document.createElement("button");
        b.type = "button";
        b.className = "bp-v-btn" + (lastLayers?.layerMode === idx ? " is-on" : "");
        b.textContent = lbl;
        b.addEventListener("click", () => void syncLayers("mode", String(idx)));
        modes.append(b);
      });
      frag.append(modes);
    }

    const presentColors = plot2d?.presentColors() ?? new Map<number, number>();
    const allRows = (lastLayers.layerList || []).concat(lastLayers.layerMarks || []);
    const list = document.createElement("div");
    list.className = "bp-v-layers-list";
    for (const row of allRows) {
      const col = presentColors.get(row.layer);
      if (col === undefined) continue;
      const item = document.createElement("div");
      item.className = "bp-v-layer-row" + (row.active ? " is-focus" : "");

      const chk = document.createElement("input");
      chk.type = "checkbox";
      chk.checked = row.on !== false;
      chk.addEventListener("change", () => void syncLayers("toggle", String(row.layer)));

      const sw = document.createElement("span");
      sw.className = "bp-v-layer-swatch";
      const r = (col >>> 16) & 0xff;
      const g = (col >>> 8) & 0xff;
      const b = col & 0xff;
      sw.style.background = row.on !== false ? `rgb(${r},${g},${b})` : `rgba(${r},${g},${b},0.2)`;

      const lbl = document.createElement("button");
      lbl.type = "button";
      lbl.className = "bp-v-layer-name";
      lbl.textContent = row.name;
      lbl.addEventListener("click", () => void syncLayers("pick", String(row.layer)));

      item.append(chk, sw, lbl);
      list.append(item);
    }
    frag.append(list);
    layersPop.replaceChildren(frag);
  }

  async function syncLayers(action: string, value: string): Promise<void> {
    if (!isLayerPage(page)) return;
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
        lift: typeof res.lift === "number" ? res.lift : 255,
      };
      plot2d?.setLayers(masks);
      renderHeader();
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

  function clearProbe(): void {
    infoEl.hidden = true;
    infoEl.replaceChildren();
    plot2d?.setHighlights([]);
  }

  function renderProbeCard(info: string): { ref?: string; net?: string } {
    const parts = info.split(";").filter(Boolean);
    const title = parts.shift() || info;
    const rows: Array<[string, string]> = [];
    let ref = "";
    let net = "";
    for (const part of parts) {
      if (part.startsWith("g=")) continue;
      const eq = part.indexOf("=");
      if (eq > 0) {
        const k = part.slice(0, eq);
        const v = part.slice(eq + 1);
        if ((k === "ref" || k === "symbol") && !ref) ref = v;
        if (k === "net" && !net) net = v;
        rows.push([k, v]);
      } else {
        rows.push([part, ""]);
      }
    }

    const head = document.createElement("div");
    head.className = "bp-v-info-head";
    const titleEl = document.createElement("span");
    titleEl.className = "bp-v-info-title";
    titleEl.textContent = title;
    const closeBtn = document.createElement("button");
    closeBtn.type = "button";
    closeBtn.className = "bp-v-info-x";
    closeBtn.textContent = "×";
    closeBtn.setAttribute("aria-label", "Clear selection");
    closeBtn.addEventListener("click", clearProbe);
    head.append(titleEl, closeBtn);

    const grid = document.createElement("div");
    grid.className = "bp-v-info-rows";
    for (const [k, v] of rows) {
      const chip = document.createElement("span");
      chip.className = "bp-v-info-chip";
      chip.textContent = v ? `${k}: ${v}` : k;
      grid.append(chip);
    }

    infoEl.replaceChildren(head, ...(rows.length > 0 ? [grid] : []));
    infoEl.hidden = false;
    return { ref: ref || undefined, net: net || undefined };
  }

  async function onPick2D(hit: { at: number; idx: number } | null): Promise<void> {
    if (!hit) {
      clearProbe();
      return;
    }
    const key = Array.from(activeKeys)[0] || "";
    if (!key) return;
    const gen = watchGen;
    try {
      const res = await api.hub.rpc<{ info?: string }>("viewer.info", { key, at: hit.at, idx: hit.idx });
      if (gen !== watchGen) return;
      const info = typeof res.info === "string" ? res.info : "";
      if (!info) {
        clearProbe();
        return;
      }
      const { ref, net } = renderProbeCard(info);
      const queries: Array<[string, string]> = [];
      if (ref) queries.push(["ref", ref]);
      if (net) queries.push(["net", net]);
      const allMatches: [number, number][] = [[hit.at, hit.idx]];
      for (const [name, value] of queries) {
        const probe = await api.hub.rpc<{ matches?: [number, number][] }>("viewer.probe", { key, name, value });
        if (gen !== watchGen) return;
        if (Array.isArray(probe.matches)) {
          for (const m of probe.matches) allMatches.push(m);
        }
      }
      plot2d?.setHighlights(allMatches);
    } catch {
      clearProbe();
    }
  }

  function clearStage(): void {
    activePane?.destroy();
    activePane = null;
    activeTerm?.destroy();
    activeTerm = null;
    plot2d?.destroy();
    plot2d = null;
    solid3d?.destroy();
    solid3d = null;
    stage.replaceChildren();
    layersOpen = false;
    layersPop.hidden = true;
    infoEl.hidden = true;
    infoEl.replaceChildren();
    setBanner("", false);
    setOverlay("none");
  }

  function clearPending(): void {
    pendingQueue = [];
    pendingBytes = 0;
  }

  async function startWatch(force: boolean): Promise<void> {
    if (!threadId || !page) return;
    const gen = ++watchGen;
    activeKeys.clear();
    watchPending = false;
    clearPending();
    clearStage();
    renderHeader();

    if (isNonPlotPane(page)) {
      // Non-plot panes (Files, Diff, Review, Terminal) use their own data/1 RPCs directly.
      void api.hub.rpc("viewer.watch", { thread: threadId, kind: "" }).catch(() => {});
      if (page === "files") {
        activePane = mountFilesPane(stage, api, threadId, (kind, relPath) => {
          page = kind;
          selectedPath = relPath;
          if (kind === "mech") selectedMech = relPath;
          void startWatch(false);
        });
      } else if (page === "diff") {
        activePane = mountDiffPane(stage, api, threadId);
      } else if (page === "review") {
        activePane = mountReviewPane(stage, api, threadId);
      } else if (page === "browser") {
        activePane = mountBrowserPane(stage, api, threadId);
      } else if (page === "terminal") {
        setOverlay("loading", "Loading terminal…");
        const termUrl = "/assets/terminal.js";
        void import(/* @vite-ignore */ termUrl).then(
          (mod: { mountTerminalPane?: (s: HTMLElement, a: ViewerHostApi, t: string, l: boolean) => { setTheme(l: boolean): void; destroy(): void } }) => {
            if (gen !== watchGen) return;
            setOverlay("none");
            if (mod?.mountTerminalPane) {
              activeTerm = mod.mountTerminalPane(stage, api, threadId, isLight());
            } else {
              setOverlay("error", "Terminal chunk missing", true);
            }
          },
          (err: unknown) => {
            if (gen !== watchGen) return;
            setOverlay("error", err instanceof Error ? err.message : String(err), true);
          }
        );
      }
      return;
    }

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
        onStatus: (msg, streaming) => {
          if (streaming) {
            setOverlay("none");
            setBanner(msg, false);
          } else if (msg) {
            setOverlay("loading", msg);
          } else {
            setOverlay("none");
            setBanner("", false);
          }
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
          if (!selectedMech) selectedMech = ml.primary || mechParts[0] || "";
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
      if (selectedPath && (page === "board" || page === "schematic")) {
        params.path = selectedPath;
      }
      if (page === "3d" && !force && solid3d) {
        const kept = solid3d.keptToken();
        if (kept) params.kept = kept;
      }
    }

    watchPending = true;
    try {
      const res = await api.hub.rpc<{ keys?: string[] }>("viewer.watch", params);
      if (gen !== watchGen) return;
      activeKeys = new Set(Array.isArray(res.keys) ? res.keys : []);
      watchPending = false;
      const queued = pendingQueue;
      clearPending();
      for (const item of queued) {
        if (item.kind === "bin") handleBinary(item.buf);
        else handlePlotNone(item.key, item.none, item.retryable);
      }
    } catch (err) {
      if (gen !== watchGen) return;
      watchPending = false;
      activeKeys.clear();
      clearPending();
      const msg = err instanceof Error ? err.message : String(err);
      const permanent = /no gerbers|not in this project|another machine/i.test(msg);
      setOverlay(permanent ? "empty" : "error", msg, !permanent);
    }
  }

  function handlePlotNone(key: string, none: string, retryable: boolean): void {
    if (watchPending) {
      if (pendingQueue.length < MAX_PENDING_FRAMES) {
        pendingQueue.push({ kind: "none", key, none, retryable });
      }
      return;
    }
    if (!activeKeys.has(key)) return;
    setOverlay(retryable ? "error" : "empty", none || "Nothing to display", retryable);
  }

  function handleBinary(buf: ArrayBuffer): void {
    if (!page) return;
    if (watchPending) {
      if (pendingQueue.length < MAX_PENDING_FRAMES && pendingBytes + buf.byteLength <= MAX_PENDING_BYTES) {
        pendingQueue.push({ kind: "bin", buf });
        pendingBytes += buf.byteLength;
      }
      return;
    }
    const bytes = new Uint8Array(buf);
    if (!isPlotFrame(bytes)) return;
    const decoded = decodeCbor(bytes);
    if (!decoded || typeof decoded !== "object" || Array.isArray(decoded) || decoded instanceof Uint8Array) return;
    const map = decoded as Record<string, CborValue>;
    if (map.t !== "plot") return;
    const key = typeof map.key === "string" ? map.key : "";
    if (!key || !activeKeys.has(key)) return;

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
      if (present !== layerPresent && isLayerPage(page)) {
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
  const onKeyDown = (ev: KeyboardEvent): void => {
    if (ev.key === "Escape" && page && !infoEl.hidden) {
      clearProbe();
    }
  };
  window.addEventListener("keydown", onKeyDown);

  return {
    open(kind: string, nextThread: string) {
      if (nextThread !== threadId) {
        threadId = nextThread;
        filesInfo = {};
        mechParts = [];
        selectedMech = "";
        heldChunks.clear();
        followOn = false;
        followPaused = false;
        followSeq = 0;
        layerOwn = 0;
        layerFocus = "";
        layerPresent = 0;
        lastLayers = null;
        if (api.hub.hasFeature("follow")) {
          void api.hub
            .rpc<{ on?: boolean; paused?: boolean; seq?: number }>("viewer.follow", { thread: nextThread })
            .then((f) => {
              if (nextThread !== threadId) return;
              followOn = !!f.on;
              followPaused = !!f.paused;
              followSeq = Number(f.seq) || 0;
              if (page) renderHeader();
            })
            .catch(() => {});
        }
      }
      if (kind !== page) {
        layerOwn = 0;
        layerFocus = "";
        layerPresent = 0;
        lastLayers = null;
      }
      // Rule (3): switching pages sends the destination page's viewer.watch directly (no separate stop first).
      page = kind;
      selectedPath = "";
      void startWatch(false);
    },
    reconcileHas(has: RailHas) {
      if (page && !pageHasBacking(page, has)) {
        api.closeViewer();
      }
    },
    close() {
      watchGen++;
      watchPending = false;
      clearPending();
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
