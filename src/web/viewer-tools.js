import { ReflowPanel } from "./reflow.js";
import { ComponentStore, mountInspector, attachSolid } from "./components.js";
import { ViewerQuality } from "./viewer-quality.js";

// Local controls for the shared web/Electron canvas. Native phone renderers
// consume the same model wire but have their own presentation controls.
function node(tag, cls, text) {
  const n = document.createElement(tag); if (cls) n.className = cls;
  if (text != null) n.textContent = text; return n;
}
function button(label, fn, title = label) {
  const b = node("button", "", label); b.type = "button"; b.title = title; b.addEventListener("click", fn); return b;
}
let workbench = null;
const panels = new Map();
const revisions = new Map();
class Workbench {
  constructor(root, solid, services = {}) {
    this.root = root; this.solid = solid; this.scope = root.dataset.scope; this.key = root.dataset.key;
    this.services = services; this.boardKey = root.dataset.boardKey || ""; this.alive = true;
    this.components = new ComponentStore(this.boardKey); this.metadataGeneration = 0;
    const stateKey = `${this.scope}|${this.key}`;
    if (!panels.has(stateKey)) { panels.set(stateKey, { pcb: false, reflow: false, camera: false }); if (panels.size > 24) panels.delete(panels.keys().next().value); }
    this.state = panels.get(stateKey); root.classList.toggle("solid-split", this.state.pcb);
    this.toolbar = root.querySelector(".solid-toolbar"); this.inspector = root.querySelector(".solid-inspector");
    this.toolbar.replaceChildren(); this.toolbar.setAttribute("aria-label", "3D viewer controls"); this.toolbar.setAttribute("role", "toolbar");
    const orientation = node("div", "solid-orientation"); orientation.setAttribute("role", "group"); orientation.setAttribute("aria-label", "View orientation");
    for (const [label, action, hint] of [["Fit", "fit", "Fit the whole model (F)"], ["Iso", "iso", "Isometric view (0)"], ["Top", "top", "Top view (1)"], ["Bottom", "bottom", "Bottom view (2)"], ["Front", "front", "Front view (3)"], ["Right", "right", "Right view (4)"], ["Back", "back", "Back view (5)"], ["Left", "left", "Left view (6)"]]) {
      orientation.append(button(label, () => this.act(action), hint));
    }
    this.toolbar.append(orientation);
    this.pcb = root.querySelector(".solid-pcb");
    if (this.pcb && root.dataset.boardKey) {
      const split = button("PCB", () => {
        const on = !root.classList.contains("solid-split"); this.state.pcb = on; root.classList.toggle("solid-split", on); split.setAttribute("aria-pressed", String(on));
      }, "Show the saved PCB beside the assembly"); split.setAttribute("aria-pressed", String(this.state.pcb)); this.toolbar.append(split);
    }
    const camera = node("details", "solid-camera"); camera.open = this.state.camera; camera.addEventListener("toggle", () => { this.state.camera = camera.open; }); camera.append(node("summary", "", "Camera")); const cameraBody = node("div", "solid-camera-body"); camera.append(cameraBody);
    const projection = node("label", "solid-field", "Projection"); this.projection = node("select"); this.projection.setAttribute("aria-label", "Projection");
    for (const [value, label] of [["perspective", "Perspective"], ["orthographic", "Orthographic"]]) { const o = node("option", "", label); o.value = value; this.projection.append(o); }
    this.projection.addEventListener("change", () => this.act("projection", this.projection.value)); projection.append(this.projection); cameraBody.append(projection);
    this.focal = this.range(cameraBody, "Focal length", "mm", 15, 250, 1, "focal");
    this.exposure = this.range(cameraBody, "Exposure", "EV", -3, 3, .1, "exposure");
    this.focus = node("output", "solid-explanation"); this.focus.setAttribute("aria-label", "Focus distance"); cameraBody.append(this.focus);
    this.aperture = node("select"); this.aperture.setAttribute("aria-label", "Depth of field aperture");
    for (const value of ["off", "2.8", "5.6", "8", "14", "22", "57"]) { const o = node("option", "", value === "off" ? "Off" : `f/${value}`); o.value = value; this.aperture.append(o); }
    this.aperture.addEventListener("change", () => this.act("aperture", this.aperture.value));
    this.apertureLabel = node("label", "solid-field", "Depth of field"); this.apertureLabel.append(this.aperture); cameraBody.append(this.apertureLabel);
    this.focusLock = button("Lock focus", () => this.act("focus-lock"), "Keep the current focus distance while moving the camera"); cameraBody.append(this.focusLock);
    const help = node("details", "solid-help"); help.append(node("summary", "", "Controls"), node("p", "solid-explanation", "Drag to orbit · Shift/right drag to pan · wheel/pinch to zoom · Alt drag to roll · double click or F to fit. Touch: one finger orbit, two fingers pan/pinch/twist. Focus canvas for arrow keys; Shift+arrows pans, +/− zooms, 0–6 selects a view."));
    cameraBody.append(help, node("p", "solid-explanation", "Material finishes and lighting affect appearance. Model placement and colors follow saved CAD.")); this.toolbar.append(camera);
    this.reflowButton = button("Reflow", () => this.showReflow(!this.panel)); this.reflowButton.setAttribute("aria-expanded", "false"); this.reflowButton.setAttribute("aria-controls", "solid-inspector"); this.toolbar.append(this.reflowButton);
    if (this.boardKey) {
      this.componentsButton = button("Components", () => this.showComponents(!this.state.components)); this.componentsButton.setAttribute("aria-expanded", "false"); this.componentsButton.setAttribute("aria-controls", "solid-inspector"); this.toolbar.append(this.componentsButton);
    }
    this.model = node("span", "solid-model-name"); const path = this.key.slice(this.key.indexOf("|") + 1); this.model.textContent = path.split("/").pop() || "3D model"; this.model.title = path; this.toolbar.append(this.model);
    this.qualityRevision = this.assemblyRevision();
    this.quality = new ViewerQuality({ root, solid, services, components: this.components, revision: () => this.assemblyRevision(), state: this.state });
    this.prepareAssembly();
    this.change = () => { if (this.alive) this.sync(); }; root.addEventListener("solid-camera-change", this.change); root.addEventListener("solid-quality-change", this.change);
    this.unselect = this.components.subscribe(component => {
      this.services.plot?.selectComponent?.(this.components.selected, this.boardKey);
      const selection = component ? { id: component.id, reference: component.reference, sourceRevision: this.components.data?.sourceRevision, kind: "saved-footprint" } : null;
      const token = selection ? `${selection.id}|${selection.sourceRevision}` : "";
      if (token !== this.selectionToken) { this.selectionToken = token; this.solid.setSelection?.(selection); }
      this.syncComponents();
      // A selection the person made opens the components pane; one restored by
      // a metadata refresh (readComponents) must not take the inspector from
      // whatever pane is open, such as the reflow preview.
      if (component && !this.state.components && !this.restoring) this.showComponents(true);
    });
    this.sync();
    if (this.state.components) this.showComponents(true); else this.showReflow(this.state.reflow);
    if (this.boardKey) void this.readComponents();
  }
  range(parent, label, unit, min, max, step, action) {
    const wrap = node("label", "solid-field", label), input = node("input"), value = node("output");
    input.type = "range"; Object.assign(input, { min, max, step }); input.setAttribute("aria-label", label);
    input.addEventListener("input", () => { this.act(action, Number(input.value)); value.value = `${input.value} ${unit}`; });
    wrap.append(input, value); parent.append(wrap); return { input, value, wrap, unit };
  }
  act(action, value) {
    const layout = this.root.querySelector(".solid-layout");
    if (layout) layout.scrollTop = 0;
    this.solid.cameraAction?.(action, value); this.sync();
  }
  sync() {
    this.root.classList.toggle("solid-split", this.state.pcb);
    this.syncComponents();
    this.quality?.sync();
    const state = this.solid.cameraState?.(); if (!state) return;
    this.projection.value = state.projection || "perspective";
    const ortho = state.projection === "orthographic";
    this.focal.wrap.hidden = ortho;
    for (const [field, value] of [[this.focal, state.focal], [this.exposure, state.exposure]]) {
      if (value == null) continue;
      field.input.value = value; field.value.value = `${Number(value).toFixed(field === this.exposure ? 1 : 0)} ${field.unit}`;
    }
    // An unsupported depth control is absent, never a decorative slider.
    const dof = Object.hasOwn(state, "aperture"); this.apertureLabel.hidden = ortho || !dof; this.focusLock.hidden = ortho || !dof;
    this.focus.hidden = ortho || state.focusDistance == null;
    if (state.focusDistance != null) this.focus.value = `Focus ${Number(state.focusDistance).toFixed(1)} mm${state.focusLocked ? " · locked" : ""}`;
    if (dof) this.aperture.value = state.aperture > 0 ? String(state.aperture) : "off";
    this.focusLock.setAttribute("aria-pressed", state.focusLocked ? "true" : "false"); this.focusLock.textContent = state.focusLocked ? "Unlock focus" : "Lock focus";
  }
  showReflow(on) {
    if (on) { this.state.components = false; this.componentPanel?.(); this.componentPanel = null; this.componentsButton?.setAttribute("aria-expanded", "false"); }
    this.state.reflow = on;
    this.panel?.dispose(); this.panel = null; this.inspector.replaceChildren(); this.inspector.hidden = !on;
    this.reflowButton.setAttribute("aria-expanded", on ? "true" : "false");
    if (on) this.panel = new ReflowPanel(this.inspector, `${this.scope}|${this.key}`, this.solid);
  }
  showComponents(on) {
    this.panel?.dispose(); this.panel = null; this.state.reflow = false; this.reflowButton.setAttribute("aria-expanded", "false");
    this.componentPanel?.(); this.componentPanel = null; this.state.components = on;
    this.inspector.replaceChildren(); this.inspector.hidden = !on; this.componentsButton?.setAttribute("aria-expanded", String(on));
    if (on) {
      const head = node("div", "solid-panel-head", "Components · saved CAD");
      const tools = node("div", "reflow-controls"); tools.append(button("Refresh", () => this.readComponents()));
      this.alignmentNote = node("p", "solid-explanation"); const body = node("div", "component-body");
      this.inspector.append(head, tools, this.alignmentNote, body); this.componentPanel = mountInspector(body, this.components); this.syncComponents();
    }
  }
  async readComponents() {
    if (!this.boardKey || !this.services.components) return;
    if (this.reading) { this.readAgain = true; return; }
    const generation = ++this.metadataGeneration; this.reading = true; let loaded = "";
    this.readAbort = new AbortController();
    try {
      const data = await this.services.components(this.scope, { signal: this.readAbort.signal });
      if (!this.alive || generation !== this.metadataGeneration) return;
      const explicitPath = this.boardKey.split("|").slice(2).join("|");
      if (explicitPath && explicitPath !== data.source) throw Error("Component metadata is available for the canonical saved board. This pane shows a different source.");
      const wanted = revisions.get(this.boardKey);
      if (wanted && data.sourceRevision !== wanted) throw Error("Component metadata and the displayed PCB are different saved revisions. Refresh after the viewer finishes updating.");
      const saved = this.solid.cameraState?.()?.selection;
      this.components.setData(this.boardKey, data); loaded = data.sourceRevision || "";
      if (saved?.sourceRevision === data.sourceRevision && !this.components.selected) {
        this.restoring = true;
        try { this.components.select(saved.id); } finally { this.restoring = false; }
      }
    } catch (e) {
      if (this.alive && generation === this.metadataGeneration) this.components.setData(this.boardKey, { error: e.message || "Saved component metadata is unavailable." });
    } finally {
      this.reading = false;
      // A revision that arrived during the read asks again, unless the read
      // already returned exactly that revision (the first read starts before
      // the PCB's revision is known, and it usually is the same one).
      if (this.alive && this.readAgain) { this.readAgain = false; if (!loaded || loaded !== revisions.get(this.boardKey)) void this.readComponents(); else this.syncComponents(); }
    }
  }
  // A board's assembly (key 2|…) and its PCB (key 0|…) name the same saved
  // .kicad_pcb, so their revisions are the same file stamp. The PCB's arrives
  // first: the plot actor sends the assembly's only after building its own
  // thinned mesh, which the original model does not use. The original asset is
  // still checked against that exact stamp by the hub (render_asset.js) before
  // it is exported and again when it is published.
  sameBoardFile() { return sameBoardFile(this.key, this.boardKey); }
  assemblyRevision() { return assemblyRevision(this.key, this.boardKey, revisions); }
  revision(key) {
    const next = this.assemblyRevision();
    if (next !== this.qualityRevision) { this.qualityRevision = next; this.quality?.sourceChanged(); }
    if (key === this.boardKey) {
      this.prepareAssembly();
      this.components.setData(this.boardKey, null); this.overlay?.(); this.overlay = null; this.overlayData = null;
      void this.readComponents();
    }
    this.syncComponents();
  }
  prepareAssembly() {
    // The saved PCB's revision prepares the authenticated asset as soon as it
    // is known (assemblyRevision); the hub checks the file stamp again.
    if (this.quality?.loading || !this.sameBoardFile()) return;
    this.quality?.prepareAsset(revisions.get(this.boardKey));
  }
  syncComponents() {
    const data = this.components?.data;
    const pcbReady = !!data && !!data.sourceRevision && data.sourceRevision === revisions.get(this.boardKey);
    const status = this.solid.assetState?.();
    const assemblyReady = pcbReady && data.sourceRevision === revisions.get(this.key) && status?.status === "ready" && !status.stale;
    const board = pcbReady ? data : null;
    if (this.boundBoard !== board) { this.boundBoard = board; this.services.plot?.setComponents?.(this.boardKey, board, id => this.components.select(id)); }
    if (this.alignmentNote) this.alignmentNote.textContent = assemblyReady ? this.quality?.visible ? "Saved revisions match. Model bodies use exact component references; unmatched bodies are not selectable." : "Saved PCB and assembly revisions match. Canvas selection identifies footprint geometry." : pcbReady ? "PCB revision matches. Assembly selection waits for the same saved revision." : "Waiting for saved-source revision confirmation; canvas selection is unavailable.";
    const overlayData = assemblyReady && !this.quality?.visible ? data : null;
    if (overlayData !== this.overlayData) {
      this.overlay?.(); this.overlay = null; this.overlayData = overlayData;
      const canvas = this.root.querySelector("#solid");
      if (overlayData && canvas) this.overlay = attachSolid(canvas, this.components, { projectPoint: this.solid.projectPoint, rayToBoard: this.solid.rayToBoard });
    }
  }
  dispose() {
    this.alive = false; this.metadataGeneration++; this.readAbort?.abort(); this.quality?.dispose(); this.panel?.dispose(); this.componentPanel?.(); this.overlay?.(); this.unselect?.();
    this.services.plot?.setComponents?.(this.boardKey, null, null); this.root.removeEventListener("solid-camera-change", this.change); this.root.removeEventListener("solid-quality-change", this.change);
  }
}

export function mount(root, solid, services = {}) {
  if (!root) { workbench?.dispose(); workbench = null; return; }
  if (workbench?.root === root && workbench.scope === root.dataset.scope && workbench.key === root.dataset.key) { workbench.sync(); return; }
  workbench?.dispose(); workbench = new Workbench(root, solid, services);
}
// a board's assembly (2|root|path) and its PCB (0|root|path) on one saved file
export function sameBoardFile(key, boardKey) {
  return !!boardKey && typeof key === "string" && key.split("|")[0] === "2" && boardKey.split("|")[0] === "0"
    && boardKey.split("|").slice(1).join("|") === key.split("|").slice(1).join("|");
}
// the revision that authorizes the original model: the PCB's for the same file
// (it arrives first), else the assembly's own
export function assemblyRevision(key, boardKey, known) {
  const own = known.get(key);
  return sameBoardFile(key, boardKey) ? known.get(boardKey) || own : own;
}
export function sourceRevision(key, revision) {
  if (typeof key !== "string" || typeof revision !== "string" || !revision || revisions.get(key) === revision) return;
  revisions.set(key, revision); if (revisions.size > 64) revisions.delete(revisions.keys().next().value);
  if (workbench && [workbench.key, workbench.boardKey].includes(key)) workbench.revision(key);
}
export function reset() { workbench?.dispose(); workbench = null; revisions.clear(); }
