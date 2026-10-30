import App from "./app.bend";
const Reflow = { defaults: () => App.reflow_defaults(), alloys: () => App.reflow_alloys(), plan: (profile, alloy) => App.reflow_plan(profile, alloy) };

// Only the pure Bend planner supplies the numeric preview. This host owns
// editing and playback; it never reports a thermal/DRC/solver pass or modifies
// the CAD model. Profiles are per viewer session and can be exported explicitly.
const SOURCE = "https://www.indium.com/products/alloys/solder-alloys/";
const SVG = "http://www.w3.org/2000/svg";
const states = new Map();
const saved = key => {
  if (!states.has(key)) {
    states.set(key, { profile: JSON.parse(Reflow.defaults()), alloy: "SAC305", time: 0, speed: 30 });
    if (states.size > 24) states.delete(states.keys().next().value);
  }
  return states.get(key);
};
export function reset() { states.clear(); }

function el(tag, cls, text) {
  const n = document.createElement(tag);
  if (cls) n.className = cls;
  if (text != null) n.textContent = text;
  return n;
}
function button(label, action, title = label) {
  const b = el("button", "", label); b.type = "button"; b.title = title;
  b.addEventListener("click", action); return b;
}
function svg(tag, attrs, text) {
  const n = document.createElementNS(SVG, tag);
  for (const [k, v] of Object.entries(attrs)) n.setAttribute(k, String(v));
  if (text != null) n.textContent = text;
  return n;
}
function plot(label, samples, duration, fraction = false) {
  const s = svg("svg", { viewBox: "0 0 300 135", role: "img", "aria-label": label });
  s.append(svg("title", {}, label));
  const x = t => 34 + t / duration * 256, y = v => 108 - v / (fraction ? 1 : 350) * 96;
  for (const v of fraction ? [0, .5, 1] : [0, 100, 200, 300]) {
    s.append(svg("path", { d: `M34 ${y(v)}H290`, class: "reflow-grid" }));
    s.append(svg("text", { x: 28, y: y(v) + 3, "text-anchor": "end" }, fraction ? `${v * 100}%` : `${v}°`));
  }
  for (const t of [0, Math.round(duration / 2), duration]) s.append(svg("text", { x: x(t), y: 126, "text-anchor": "middle" }, `${t}s`));
  for (const [idx, cls] of fraction ? [[3, "reflow-liquid"]] : [[1, "reflow-furnace"], [2, "reflow-board"]]) {
    s.append(svg("path", { d: samples.map((v, i) => `${i ? "L" : "M"}${x(v[0]).toFixed(2)} ${y(v[idx]).toFixed(2)}`).join(""), class: cls }));
  }
  const cursor = svg("path", { d: "M34 10V108", class: "reflow-cursor" }); s.append(cursor);
  return { node: s, at(t) { cursor.setAttribute("d", `M${x(t)} 10V108`); } };
}

export class ReflowPanel {
  constructor(root, key, solid) {
    this.root = root; this.state = saved(key); this.solid = solid; this.playing = false;
    root.replaceChildren(); root.setAttribute("aria-label", "Illustrative reflow preview");
    const heading = el("div", "solid-panel-head"); heading.append(el("strong", "", "Reflow preview"), el("span", "solid-tag", "Illustrative"));
    root.append(heading, el("p", "solid-explanation", "Temperature lag and melt fraction study. No board-specific calibration or process qualification."));
    const alloyLabel = el("label", "solid-field", "Solder alloy");
    this.alloy = el("select"); this.alloy.setAttribute("aria-label", "Solder alloy");
    this.alloys = JSON.parse(Reflow.alloys());
    for (const a of this.alloys) { const o = el("option", "", a.id); o.value = a.id; this.alloy.append(o); }
    this.alloy.value = this.state.alloy;
    this.alloy.addEventListener("change", () => { this.state.alloy = this.alloy.value; this.rebuild(); });
    alloyLabel.append(this.alloy); this.composition = el("div", "solid-explanation");
    const link = el("a", "solid-source", "Alloy data · Indium Corporation"); link.href = SOURCE; link.target = "_blank"; link.rel = "noopener noreferrer";
    root.append(alloyLabel, this.composition, link);
    const readings = el("dl", "reflow-readings"); this.readings = [];
    for (const name of ["Furnace", "Board mean", "Liquid fraction"]) {
      const pair = el("div"); const value = el("dd", "", "—"); pair.append(el("dt", "", name), value); readings.append(pair); this.readings.push(value);
    }
    root.append(readings); this.graphs = el("div", "reflow-graphs"); root.append(this.graphs);
    const controls = el("div", "reflow-controls"); this.play = button("Play", () => this.toggle());
    const rewind = button("↺", () => { this.pause(); this.state.time = 0; this.paint(); }, "Rewind to start");
    this.speed = el("select"); this.speed.setAttribute("aria-label", "Replay speed");
    for (const n of [1, 10, 30, 60]) { const o = el("option", "", `${n}×`); o.value = n; this.speed.append(o); }
    this.speed.value = this.state.speed; this.speed.addEventListener("change", () => { this.state.speed = Number(this.speed.value); this.last = null; });
    controls.append(rewind, this.play, this.speed); this.clock = el("output"); this.clock.setAttribute("aria-label", "Replay time"); controls.append(this.clock);
    this.scrub = el("input", "reflow-scrub"); this.scrub.type = "range"; this.scrub.min = "0"; this.scrub.step = ".1"; this.scrub.setAttribute("aria-label", "Reflow replay time in seconds");
    this.scrub.addEventListener("input", () => { this.pause(); this.state.time = Number(this.scrub.value); this.paint(); });
    this.phase = el("div", "reflow-phase"); this.phase.setAttribute("role", "status"); this.phase.setAttribute("aria-live", "off");
    this.swatch = el("div", "reflow-phase-swatch"); this.phaseLabel = el("span"); this.phase.append(this.swatch, this.phaseLabel);
    root.append(controls, this.scrub, this.phase);
    this.heat = el("input"); this.heat.type = "checkbox"; this.heat.checked = false;
    const heat = el("label", "solid-check"); heat.append(this.heat, document.createTextNode("Illustrative heat overlay"));
    this.heat.addEventListener("change", () => this.paint()); root.append(heat);
    const editor = el("details", "reflow-editor"); editor.append(el("summary", "", "Edit profile points"));
    editor.append(el("p", "solid-explanation", "Seconds and furnace °C. This example profile is editable and is not a recommended process."));
    this.rows = el("div", "reflow-profile-points"); editor.append(this.rows);
    const actions = el("div", "reflow-controls");
    actions.append(button("Add point", () => this.addPoint()), button("Apply", () => this.apply()), button("Reset", () => { this.state.profile = JSON.parse(Reflow.defaults()); this.editRows(); this.rebuild(); }));
    this.error = el("div", "solid-error"); this.error.setAttribute("role", "alert"); editor.append(actions, this.error);
    const files = el("div", "reflow-controls"); const load = el("input"); load.type = "file"; load.accept = "application/json,.json"; load.hidden = true;
    load.addEventListener("change", async () => {
      const file = load.files?.[0]; if (!file) return;
      try {
        if (file.size > 16384) throw new Error("Profile file is too large.");
        const data = JSON.parse(await file.text());
        const profile = Array.isArray(data) ? data : data.points;
        if (!Array.isArray(data) && (data.kind !== "backplane-illustrative-reflow" || data.version !== 1)) throw new Error("Unsupported profile file format.");
        const alloy = Array.isArray(data) ? this.state.alloy : data.alloy;
        const result = JSON.parse(Reflow.plan(JSON.stringify(profile), typeof alloy === "string" ? alloy : ""));
        if (result.error) throw new Error(result.error);
        this.state.profile = profile; this.state.alloy = alloy; this.alloy.value = alloy; this.editRows(); this.rebuild(); this.error.textContent = "";
      } catch (e) { this.error.textContent = e.message || "Invalid profile."; }
      load.value = "";
    });
    files.append(button("Import JSON", () => load.click()), button("Export JSON", () => this.download()), load); editor.append(files); root.append(editor);
    root.append(el("p", "solid-explanation reflow-assumptions", "Model: 24 s first-order thermal lag, 1 s steps; linear melt fraction between solidus and liquidus. No latent heat, spatial gradients, flux, wetting or settling model. Heat tint and solder swatch are illustrative; CAD geometry stays unchanged."));
    this.visibility = () => { if (document.hidden) this.pause(); }; document.addEventListener("visibilitychange", this.visibility);
    this.editRows(); this.rebuild();
  }
  editRows() {
    this.rows.replaceChildren();
    for (const [i, point] of this.state.profile.entries()) this.row(point, i);
  }
  row(point, i) {
    const r = el("div", "reflow-point");
    for (const [k, label] of [[0, "Seconds"], [1, "Furnace °C"]]) {
      const input = el("input"); input.type = "number"; input.value = point[k]; input.min = "0"; input.max = k ? "350" : "1200"; input.step = k ? ".1" : "1";
      input.setAttribute("aria-label", `Point ${i + 1} ${label}`); r.append(input);
    }
    r.append(button("×", () => r.remove(), `Remove point ${i + 1}`)); this.rows.append(r);
  }
  addPoint() {
    if (this.rows.children.length >= 32) { this.error.textContent = "At most 32 points."; return; }
    const last = this.rows.lastElementChild?.querySelector("input");
    this.row([Math.min(1200, Number(last?.value || 0) + 30), 25], this.rows.children.length);
  }
  apply() {
    const points = [...this.rows.children].map(r => [...r.querySelectorAll("input")].map(n => n.value.trim() === "" ? null : Number(n.value)));
    const result = JSON.parse(Reflow.plan(JSON.stringify(points), this.state.alloy));
    if (result.error) { this.error.textContent = result.error; return; }
    this.state.profile = points; this.error.textContent = ""; this.rebuild();
  }
  rebuild() {
    this.pause(); this.plan = JSON.parse(Reflow.plan(JSON.stringify(this.state.profile), this.state.alloy));
    if (this.plan.error) { this.error.textContent = this.plan.error; return; }
    const alloy = this.alloys.find(a => a.id === this.state.alloy);
    this.composition.textContent = `${alloy.composition} · Solidus ${alloy.solidus} °C / liquidus ${alloy.liquidus} °C`;
    this.state.time = Math.min(this.state.time, this.plan.duration); this.scrub.max = this.plan.duration;
    this.temperature = plot("Illustrative furnace and mean board temperature in Celsius", this.plan.samples, this.plan.duration);
    this.liquid = plot("Illustrative liquid fraction from zero to one hundred percent", this.plan.samples, this.plan.duration, true);
    this.graphs.replaceChildren(el("div", "solid-graph-label", "Temperature °C · furnace dashed / board solid"), this.temperature.node, el("div", "solid-graph-label", "Liquid fraction"), this.liquid.node);
    this.paint();
  }
  paint() {
    if (!this.plan?.samples) return;
    const t = Math.max(0, Math.min(this.plan.duration, this.state.time)), a = this.plan.samples[Math.floor(t)], b = this.plan.samples[Math.min(Math.ceil(t), this.plan.duration)];
    const sample = a.map((v, i) => v + (b[i] - v) * (t % 1));
    this.readings[0].textContent = `${sample[1].toFixed(1)} °C`; this.readings[1].textContent = `${sample[2].toFixed(1)} °C`; this.readings[2].textContent = `${(sample[3] * 100).toFixed(1)}%`;
    this.clock.value = `${t.toFixed(1)} / ${this.plan.duration} s`; this.scrub.value = t; this.scrub.setAttribute("aria-valuetext", `${t.toFixed(1)} of ${this.plan.duration} seconds`);
    this.temperature.at(t); this.liquid.at(t);
    const molten = sample[3]; this.swatch.style.setProperty("--liquid", molten.toFixed(3));
    this.phaseLabel.textContent = molten >= .999 ? "Liquid · illustrative" : molten > 0 ? "Partially molten · illustrative" : t > 0 && this.plan.samples.slice(0, Math.floor(t) + 1).some(v => v[3] > 0) ? "Solidified · illustrative" : "Solid · illustrative";
    this.solid.setThermal?.({ enabled: this.heat.checked, temperature: sample[2], liquid: molten });
  }
  toggle() {
    if (this.playing) return this.pause();
    if (this.state.time >= this.plan.duration) this.state.time = 0;
    this.playing = true; this.play.textContent = "Pause"; this.play.setAttribute("aria-pressed", "true"); this.last = null;
    const frame = now => {
      if (!this.playing || !this.root.isConnected) return this.pause();
      if (this.last != null) this.state.time = Math.min(this.plan.duration, this.state.time + Math.min((now - this.last) / 1000, .25) * this.state.speed);
      this.last = now;
      if (!this.painted || now - this.painted >= 100 || this.state.time >= this.plan.duration) { this.paint(); this.painted = now; }
      if (this.state.time >= this.plan.duration) return this.pause();
      this.frame = requestAnimationFrame(frame);
    };
    this.frame = requestAnimationFrame(frame);
  }
  pause() {
    this.playing = false; this.last = null; cancelAnimationFrame(this.frame); this.frame = null;
    this.play.textContent = "Play"; this.play.setAttribute("aria-pressed", "false");
  }
  download() {
    const blob = new Blob([JSON.stringify({ kind: "backplane-illustrative-reflow", version: 1, alloy: this.state.alloy, points: this.state.profile, assumptions: "24 s first-order lag; 1 s steps; linear melt fraction. Not a qualified process." }, null, 2) + "\n"], { type: "application/json" });
    const url = URL.createObjectURL(blob), a = el("a"); a.href = url; a.download = "reflow-preview.json"; a.click(); setTimeout(() => URL.revokeObjectURL(url), 1000);
  }
  dispose() {
    this.pause(); document.removeEventListener("visibilitychange", this.visibility); this.solid.setThermal?.({ enabled: false });
  }
}
