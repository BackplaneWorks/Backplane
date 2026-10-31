// Original-model quality rendering is optional and lazy. The compact preview
// remains interactive while the authenticated asset and worker prepare.
function button(label, action, title) {
  const n = document.createElement("button"); n.type = "button"; n.textContent = label; n.title = title || label; n.addEventListener("click", action); return n;
}
export class ViewerQuality {
  constructor({ root, solid, services, components, revision, state }) {
    Object.assign(this, { root, solid, services, components, revision, state });
    this.alive = true; this.generation = 0; this.pointers = new Map(); this.cleanup = [];
    this.canvas = root.querySelector("#solid"); this.stage = this.canvas?.parentElement; if (!this.stage) return;
    state.renderMode ||= state.quality ? "pathtrace" : "studio";
    state.quality = state.renderMode !== "fast";
    this.mode = document.createElement("select"); this.mode.className = "solid-render-mode"; this.mode.setAttribute("aria-label", "Rendering");
    for (const [value, label] of [["fast", "Fast"], ["studio", "Studio"], ["pathtrace", "Path trace"]]) { const o = document.createElement("option"); o.value = value; o.textContent = label; this.mode.append(o); }
    this.mode.value = state.renderMode;
    this.mode.addEventListener("change", () => {
      state.renderMode = this.mode.value;
      this.samples.hidden = state.renderMode !== "pathtrace";
      if (state.renderMode === "fast") { void this.enable(false); return; }
      state.quality = true;
      if (this.renderer && this.asset?.revision === this.revision()) { this.renderer.setMode(state.renderMode === "pathtrace" ? "pathtrace" : "raster", { samples: Number(this.samples.value) }); this.sync(); }
      else if (!this.loading) void this.enable(true);
    });
    this.frame = button("Frame part", () => this.framePart(), "Frame the selected component using its actual source model bounds"); this.frame.disabled = true;
    this.samples = document.createElement("select"); this.samples.className = "solid-samples"; this.samples.setAttribute("aria-label", "Path tracing sample limit");
    for (const n of [32, 64, 128, 256]) { const o = document.createElement("option"); o.value = n; o.textContent = `${n} samples`; this.samples.append(o); }
    this.samples.value = state.samples || 64; this.samples.hidden = state.renderMode !== "pathtrace";
    this.samples.addEventListener("change", () => { state.samples = Number(this.samples.value); this.renderer?.setMode("pathtrace", { samples: state.samples }); });
    const bar = root.querySelector(".solid-toolbar"), before = bar.querySelector(".solid-model-name");
    for (const n of [this.mode, this.samples, this.frame]) bar.insertBefore(n, before);
    this.note = document.createElement("div"); this.note.className = "solid-render-status"; this.note.setAttribute("role", "status"); this.stage.append(this.note);
    state.materialPolicy ||= "cad-studio";
    this.materials = document.createElement("select"); this.materials.setAttribute("aria-label", "Material finish"); this.materials.disabled = true;
    for (const [value, label] of [["realistic", "Realistic"], ["cad-studio", "CAD studio"], ["source-defaults", "Source materials"]]) {
      const option = document.createElement("option"); option.value = value; option.textContent = label; option.disabled = value === "realistic"; this.materials.append(option);
    }
    this.materials.value = state.materialPolicy;
    const materialLabel = document.createElement("label"); materialLabel.className = "solid-field"; materialLabel.append(document.createTextNode("Materials"), this.materials);
    this.materialNote = document.createElement("p"); this.materialNote.className = "solid-explanation"; this.materialNote.setAttribute("role", "status");
    this.materialNote.textContent = "CAD studio estimates missing surface factors. Source materials use the original export settings.";
    root.querySelector(".solid-camera-body")?.append(materialLabel, this.materialNote);
    this.materials.addEventListener("change", () => this.updateMaterials());
    this.solder = document.createElement("input"); this.solder.type = "checkbox"; this.solder.disabled = true;
    this.solder.setAttribute("aria-label", "Illustrative solder");
    const solderLabel = document.createElement("label"); solderLabel.className = "solid-check"; solderLabel.append(this.solder, document.createTextNode("Illustrative solder"));
    this.solderNote = document.createElement("p"); this.solderNote.className = "solid-explanation"; this.solderNote.setAttribute("role", "status");
    this.solderNote.textContent = "Estimated joint shapes on supported pads and terminals. Not manufactured joint geometry.";
    root.querySelector(".solid-camera-body")?.append(solderLabel, this.solderNote);
    this.solder.addEventListener("change", () => this.updateSolder());
    const listen = (name, fn, options) => { this.canvas.addEventListener(name, fn, options); this.cleanup.push(() => this.canvas.removeEventListener(name, fn, options)); };
    listen("pointerdown", e => { this.pointers.set(e.pointerId, { x: e.clientX, y: e.clientY }); this.dragged = this.pointers.size > 1; this.renderer?.setInteractive(true); });
    listen("pointermove", e => { const a = this.pointers.get(e.pointerId); if (a && Math.hypot(e.clientX - a.x, e.clientY - a.y) > 4) this.dragged = true; });
    listen("pointerup", e => {
      const a = this.pointers.get(e.pointerId); this.pointers.delete(e.pointerId);
      if (a && !this.dragged && this.visible && e.button === 0) {
        const b = this.canvas.getBoundingClientRect(); void this.renderer?.pick(e.clientX - b.left, e.clientY - b.top).then(id => { if (this.alive && this.visible && id != null) this.components.select(id); }).catch(() => {});
      }
      if (!this.pointers.size) this.settle();
    });
    listen("pointercancel", () => { this.pointers.clear(); this.dragged = true; this.settle(); });
    listen("wheel", () => { this.renderer?.setInteractive(true); this.settle(180); }, { passive: true });
    listen("keydown", e => { if (/^(Arrow|[0-6fF+\-=])/.test(e.key)) { this.renderer?.setInteractive(true); this.settle(120); } });
    this.unsubscribe = components.subscribe(() => this.syncComponents());
    if (state.quality) void this.enable(true);
  }
  prepareAsset(revision) {
    if (!this.alive || !this.state.quality || !revision) return null;
    if (this.preparedAsset?.revision === revision) return this.preparedAsset.promise;
    this.preparedAsset?.abort.abort();
    const abort = new AbortController();
    const parts = this.root.dataset.key.split("|"), path = parts.slice(2).join("|");
    const promise = this.services.asset(this.root.dataset.scope, path, parts[0], revision, { signal: abort.signal });
    const prepared = { revision, abort, promise };
    this.preparedAsset = prepared;
    // Preparation may precede independent assembly revision confirmation. A
    // rejected speculative read must neither leak nor become a render result.
    promise.catch(() => { if (this.preparedAsset === prepared) this.preparedAsset = null; });
    return promise;
  }
  settle(delay = 80) { clearTimeout(this.settleTimer); this.settleTimer = setTimeout(() => { if (this.alive) this.renderer?.setInteractive(false); }, delay); }
  message(text) { if (this.note && this.note.textContent !== text) this.note.textContent = text; }
  show(on) {
    const changed = this.visible !== !!on;
    this.visible = !!on;
    if (this.qualityCanvas) this.qualityCanvas.style.visibility = this.visible ? "visible" : "hidden";
    this.root.classList.toggle("solid-original-visible", this.visible);
    if (changed && this.alive) this.root.dispatchEvent(new CustomEvent("solid-quality-change"));
  }
  status(status) {
    this.lastStatus = status;
    const traceOption = this.mode.querySelector('option[value="pathtrace"]');
    traceOption.disabled = status.pathTraceAvailable === false;
    traceOption.title = traceOption.disabled ? status.pathTraceReason || "Path tracing is unavailable on this graphics backend." : "";
    if (traceOption.disabled && this.state.renderMode === "pathtrace") {
      this.state.renderMode = "studio"; this.mode.value = "studio"; this.samples.hidden = true;
      this.renderer?.setMode("raster");
    }
    if (this.materials) {
      this.materials.querySelector('option[value="realistic"]').disabled = !status.materialPolicies?.includes("realistic");
      if (!this.materialBusy && status.materialPolicy) this.materials.value = status.materialPolicy;
      if (!this.materialBusy && status.visible && status.materialPolicy) this.describeMaterials(status);
    }
    if (this.solder && !this.solderBusy) this.solder.checked = status.illustrativeSolder === true;
    if (status.visible && status.key === this.root.dataset.key && status.revision === this.revision()) this.ready = true;
    if (["unsupported", "error"].includes(status.phase)) { this.ready = false; this.show(false); }
    if (this.state.quality) {
      const samples = Number(status.samples || 0);
      const words = status.phase === "converged" ? `Path traced · ${samples.toFixed(0)} samples${status.denoised ? " · denoised" : ""}`
        : status.phase === "pathtrace" ? `Refining · ${samples.toFixed(1)} / ${this.samples.value} samples`
        : status.phase === "paused" ? `Refinement paused · ${samples.toFixed(1)} / ${this.samples.value} samples${status.denoised ? " · denoised" : ""}${status.reason ? ` · ${status.reason}` : ""}`
        : status.phase === "raster" ? `Studio · original model${status.pathTraceAvailable === false ? ` · ${status.pathTraceReason || "Path tracing unavailable"}` : ""}` : status.reason || "Preparing original model…";
      this.message(words);
    }
    this.sync();
  }
  async enable(on) {
    this.state.quality = !!on; this.samples.hidden = this.state.renderMode !== "pathtrace";
    if (!on) { this.reload = false; this.stop(); this.message(""); return; }
    const rev = this.revision();
    if (!rev) { this.message("Waiting for the saved model revision…"); return; }
    if (this.loading) { this.reload = true; return; }
    const generation = ++this.generation; this.loading = true; this.ready = false; this.show(false);
    this.abort = new AbortController(); this.message("Preparing the original saved model…");
    try {
      const asset = await this.prepareAsset(rev);
      if (!this.alive || !this.state.quality || generation !== this.generation || this.revision() !== rev) return;
      if (asset.revision !== rev) throw Error("The original model is from a different saved revision. Refresh the viewer.");
      const { createPathTraceView } = await import("./solid-pathtrace.js");
      if (!this.alive || generation !== this.generation) return;
      this.qualityCanvas = document.createElement("canvas"); this.qualityCanvas.className = "solid-pathtrace"; this.qualityCanvas.style.visibility = "hidden"; this.qualityCanvas.setAttribute("aria-hidden", "true"); this.stage.append(this.qualityCanvas);
      const renderer = await createPathTraceView({ canvas: this.qualityCanvas, materialPolicy: this.state.materialPolicy, maxSamples: Number(this.samples.value), maxMs: 30000, onStatus: s => { if (this.alive && generation === this.generation) this.status(s); } });
      if (!this.alive || generation !== this.generation) { renderer.dispose(); return; }
      this.renderer = renderer; this.asset = asset; this.pose = "";
      renderer.setMode(this.state.renderMode === "pathtrace" ? "pathtrace" : "raster", { samples: Number(this.samples.value) });
      this.sync(); await renderer.load({ ...asset, key: this.root.dataset.key, signal: this.abort.signal });
      if (!this.alive || generation !== this.generation) return;
      this.syncComponents(); this.sync();
    } catch (e) {
      if (this.alive && generation === this.generation) { this.show(false); this.message(`${e.message || "Quality rendering unavailable"} · Preview available`); }
    } finally {
      this.loading = false;
      if (this.alive && this.reload && this.state.quality) { this.reload = false; this.stop(); void this.enable(true); }
    }
  }
  syncComponents() {
    const data = this.components.data;
    const eligible = this.asset && data?.source === this.asset.source && data?.sourceRevision === this.asset.revision;
    if (this.renderer && this.boundData !== (eligible ? data : null)) {
      this.boundData = eligible ? data : null;
      void this.renderer.setComponents(this.boundData, id => this.components.select(id)).catch(() => {});
    }
    const selected = eligible ? this.components.selected : "";
    if (this.renderer && selected !== this.selectedSent) { this.selectedSent = selected; this.renderer.selectComponent(selected); }
    if (this.frame) this.frame.disabled = !eligible || !this.components.current() || !this.renderer;
    if (this.materials) this.materials.disabled = !this.ready || !this.renderer?.setMaterialPolicy || !!this.materialBusy;
    if (this.solder) this.solder.disabled = !eligible || !this.ready || !this.renderer?.setSolderPreview || !!this.solderBusy;
  }
  async updateMaterials() {
    const renderer = this.renderer, generation = this.generation, policy = this.materials.value;
    if (!renderer?.setMaterialPolicy || this.materials.disabled) return;
    this.materialBusy = true; this.syncComponents();
    try {
      const result = await renderer.setMaterialPolicy(policy);
      if (!this.alive || generation !== this.generation || renderer !== this.renderer) return;
      if (result.materialPolicy !== policy) throw Error("This renderer does not support the selected material finish.");
      this.state.materialPolicy = policy;
      this.describeMaterials(result);
    } catch (e) {
      if (this.alive && generation === this.generation) { this.materials.value = this.state.materialPolicy; this.materialNote.textContent = e.message || "Material finish could not be changed."; }
    } finally {
      if (generation === this.generation) { this.materialBusy = false; this.syncComponents(); }
    }
  }
  describeMaterials(status) {
    this.materialNote.textContent = status.materialPolicy === "realistic"
      ? "Estimated physical finishes on identified materials. Saved colors and model placement are preserved; source materials remain available."
      : status.materialPolicy === "source-defaults" ? "Original CAD export materials, including its transparency and default surface factors."
      : "CAD studio estimates only missing surface factors. Explicit export settings are preserved.";
    if (status.materialWarnings?.length) this.materialNote.textContent += ` ${status.materialWarnings.join(" ")}`;
  }
  async updateSolder() {
    const renderer = this.renderer, generation = this.generation;
    if (!renderer?.setSolderPreview || this.solder.disabled) return;
    const enabled = this.solder.checked;
    this.solderBusy = true; this.syncComponents();
    try {
      const result = await renderer.setSolderPreview(enabled);
      if (!this.alive || generation !== this.generation || renderer !== this.renderer) return;
      if (result.source !== this.asset.source || result.revision !== this.revision()) throw Error("Saved model changed while preparing illustrative solder.");
      this.solder.checked = result.enabled === true;
      const c = result.coverage;
      this.solderNote.textContent = result.enabled ? `${c?.joints || 0} estimated joints on ${c?.components || 0} supported parts. ${result.skipped?.length || 0} skipped. Not manufactured joint geometry.` : "Estimated joint shapes on supported pads and terminals. Not manufactured joint geometry.";
    } catch (e) {
      if (this.alive && generation === this.generation) { this.solder.checked = this.lastStatus?.illustrativeSolder === true; this.solderNote.textContent = e.message || "Illustrative solder is unavailable for this model."; }
    } finally {
      if (generation === this.generation) { this.solderBusy = false; this.syncComponents(); }
    }
  }
  sync() {
    if (!this.renderer) return;
    const pose = this.solid.cameraSnapshot?.(); if (!pose) return;
    const text = JSON.stringify(pose);
    // Until the main view has fitted its own model (or has a saved pose to
    // restore), its camera is an unfitted default that may not see the original
    // model at all; the renderer then frames the original model itself.
    if (text !== this.pose && (this.solid.cameraFitted?.() ?? true)) { this.pose = text; this.renderer.setCamera(pose); }
    const same = this.asset?.revision === this.revision();
    this.show(this.state.quality && this.ready && same && !pose.thermal?.enabled);
    if (pose.thermal?.enabled && this.state.quality) this.message("Illustrative heat overlay · path tracing paused");
    if (!!pose.thermal?.enabled !== this.heat) { this.heat = !!pose.thermal?.enabled; this.renderer.setInteractive(this.heat || this.pointers.size > 0); }
    this.syncComponents();
  }
  sourceChanged() { this.stop(this.revision()); if (this.state.quality) void this.enable(true); }
  async framePart() {
    const id = this.components.selected; if (!this.renderer || !id) return;
    try {
      const bounds = await this.renderer.componentBounds(id);
      if (!this.alive || id !== this.components.selected) return;
      if (!bounds || bounds.revision !== this.revision()) throw Error("This saved component has no uniquely identified source model.");
      const layout = this.root.querySelector(".solid-layout");
      if (layout) layout.scrollTop = 0;
      this.solid.frameBounds?.(bounds.min, bounds.max);
    } catch (e) { this.message(e.message || "The component cannot be framed."); }
  }
  stop(keepPreparedRevision) {
    if (!keepPreparedRevision || this.preparedAsset?.revision !== keepPreparedRevision) {
      this.preparedAsset?.abort.abort(); this.preparedAsset = null;
    }
    this.generation++; this.abort?.abort(); this.renderer?.dispose(); this.renderer = null; this.qualityCanvas?.remove(); this.qualityCanvas = null; this.asset = null; this.boundData = null; this.selectedSent = undefined; this.ready = false; this.pose = ""; this.show(false);
    this.solderBusy = false;
    this.materialBusy = false;
    if (this.materials) this.materials.disabled = true;
    if (this.materialNote) this.materialNote.textContent = "Material finishes are available in Studio and Path trace.";
    if (this.solder) { this.solder.checked = false; this.solder.disabled = true; }
    if (this.solderNote) this.solderNote.textContent = "Estimated joint shapes on supported pads and terminals. Not manufactured joint geometry.";
    this.lastStatus = null;
  }
  dispose() { this.alive = false; this.stop(); this.unsubscribe?.(); clearTimeout(this.settleTimer); for (const fn of this.cleanup) fn(); this.note?.remove(); }
}
