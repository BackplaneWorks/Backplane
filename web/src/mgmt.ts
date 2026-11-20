// Hub-driven Management Strip (BOM | Checks | Lifecycle | Plan | Fab):
// Matches docs/MANAGEMENT.md, DataMgmt.kt, and native/gpui/src/screens/mgmt*.rs.
// All tab order, badges, counts, and rows come from the hub's `mgmt.*` and `kicad.checks.*`
// info frames; clicking a tab calls `mgmt.panel { project, toggle }` and opens the panel.
import { trs } from "./i18n";
import {
  abbrev,
  badgeOf,
  drcWords,
  filterBomLines,
  filterCheckGroups,
  formatUtcDay,
  lifeWord,
  mgmtPanelsOf,
  money,
  pkgName,
  planDone,
  planMark,
  refsFit,
  stockSummary,
  tierWords,
  type BomInfo,
  type BomLine,
  type ChecksInfo,
  type FabInfo,
  type LifeInfo,
  type MgmtStateInfo,
  type PlanInfo,
} from "./mgmt-model";
import type { HubClient } from "./ws";

export interface MgmtCallbacks {
  hub: HubClient;
  getThreadId: () => string;
  onSendText: (text: string) => void;
}

interface BomUndoState {
  refs: string[];
  field: string;
  was: string;
}

export class MgmtStrip {
  private hostEl: HTMLElement;
  private panelEl: HTMLElement;
  private stripEl: HTMLElement;
  private cb: MgmtCallbacks;

  private infos = new Map<string, Record<string, unknown>>();
  private digest = "";
  private project = "";
  private askedProject = "";
  private watchGen = 0;
  private openPanel = "";
  private errorMsg = "";

  // Panel-local UI state
  private bomFilter = "";
  private bomSelectedId = "";
  private bomEditingId = "";
  private bomEditDraft = "";
  private bomEditStatus = "";
  private bomLastUndo: BomUndoState | null = null;

  private checksSource = "";
  private checksSelectedId = "";
  private checksStatus = "";

  private lifeSelectedKey = "";
  private lifePartSamples: Array<{ at?: number; stock?: number; price?: number; lifecycle?: string }> | null = null;
  private lifePartLoading = false;

  private planHideDone = false;
  private planStatus = "";

  private fabStatus = "";

  constructor(container: HTMLElement, cb: MgmtCallbacks) {
    this.cb = cb;

    this.hostEl = document.createElement("section");
    this.hostEl.className = "bp-mgmt-host bp-hidden";
    this.hostEl.setAttribute("data-testid", "mgmt-host");
    this.hostEl.setAttribute("aria-label", "Project management panels");

    this.panelEl = document.createElement("div");
    this.panelEl.className = "bp-mgmt-panel bp-hidden";

    this.stripEl = document.createElement("div");
    this.stripEl.className = "bp-mgmt-strip";
    this.stripEl.setAttribute("data-testid", "mgmt-strip");

    this.hostEl.append(this.panelEl, this.stripEl);
    container.appendChild(this.hostEl);
  }

  public isOpen(): boolean {
    return this.openPanel !== "";
  }

  public closePanel(): void {
    if (!this.openPanel) return;
    this.openPanel = "";
    if (this.project) {
      void this.cb.hub.rpc("mgmt.panel", { project: this.project, open: "" }).catch(() => {});
    }
    this.render();
  }

  public clear(): void {
    this.watchGen++;
    this.askedProject = "";
    this.digest = "";
    this.project = "";
    this.openPanel = "";
    this.errorMsg = "";
    this.render();
  }

  public async watchProject(projectPath: string): Promise<void> {
    if (!projectPath || !this.cb.hub.hasFeature("mgmt")) {
      this.clear();
      return;
    }
    if (projectPath === this.askedProject && this.digest) {
      this.render();
      return;
    }
    const gen = ++this.watchGen;
    this.askedProject = projectPath;
    if (this.project !== projectPath) {
      this.digest = "";
      this.project = "";
      this.openPanel = "";
      this.errorMsg = "";
      this.bomSelectedId = "";
      this.bomEditingId = "";
      this.bomLastUndo = null;
      this.checksSelectedId = "";
      this.lifeSelectedKey = "";
      this.lifePartSamples = null;
      this.render();
    }
    try {
      const res = await this.cb.hub.rpc<{
        digest?: string;
        project?: string;
        state?: MgmtStateInfo;
        data?: Record<string, Record<string, unknown>>;
      }>("mgmt.watch", { project: projectPath });
      if (gen !== this.watchGen) return;
      const d = typeof res.digest === "string" ? res.digest : "";
      if (!d) {
        this.render();
        return;
      }
      this.digest = d;
      this.project = typeof res.project === "string" && res.project ? res.project : projectPath;
      if (res.state && typeof res.state === "object") {
        this.infos.set(`mgmt.state.${d}`, res.state as unknown as Record<string, unknown>);
      }
      if (res.data && typeof res.data === "object") {
        for (const [k, v] of Object.entries(res.data)) {
          if (v && typeof v === "object") {
            this.infos.set(`mgmt.${k}.${d}`, v);
          }
        }
      }
      this.errorMsg = "";
      this.render();
    } catch {
      if (gen === this.watchGen) this.render();
    }
  }

  public handleInfo(info: Record<string, unknown>): void {
    let changed = false;
    for (const [key, raw] of Object.entries(info)) {
      if (!key.startsWith("mgmt.") && !key.startsWith("kicad.checks.")) continue;
      if (raw === "" || raw === null || raw === undefined) {
        if (this.infos.delete(key)) changed = true;
        continue;
      }
      if (typeof raw === "string") {
        try {
          const parsed = JSON.parse(raw) as Record<string, unknown>;
          if (parsed && typeof parsed === "object") {
            this.infos.set(key, parsed);
            changed = true;
          }
        } catch { /* ignore malformed info */ }
      } else if (typeof raw === "object") {
        this.infos.set(key, raw as Record<string, unknown>);
        changed = true;
      }
    }
    if (changed) this.render();
  }

  private getStateInfo(): MgmtStateInfo | null {
    if (!this.digest) return null;
    return (this.infos.get(`mgmt.state.${this.digest}`) as MgmtStateInfo | undefined) ?? null;
  }

  private getPanelData<T>(id: string): T | null {
    if (!this.digest) return null;
    return (this.infos.get(`mgmt.${id}.${this.digest}`) as T | undefined) ?? null;
  }

  private togglePanel(id: string): void {
    const next = this.openPanel === id ? "" : id;
    this.openPanel = next;
    this.errorMsg = "";
    this.render();
    if (this.project) {
      void this.cb.hub.rpc("mgmt.panel", { project: this.project, open: next })
        .catch((e) => {
          this.errorMsg = e instanceof Error ? e.message : "Could not update panel";
          this.render();
        });
    }
  }

  public render(): void {
    const state = this.getStateInfo();
    const tabs = mgmtPanelsOf(state);
    if (!this.digest || tabs.length === 0) {
      this.hostEl.classList.add("bp-hidden");
      this.panelEl.classList.add("bp-hidden");
      this.panelEl.replaceChildren();
      this.stripEl.replaceChildren();
      return;
    }

    this.hostEl.classList.remove("bp-hidden");

    // 1. Render tab strip verbatim from hub `mgmt.state.<digest>.panels`
    const stripFrag = document.createDocumentFragment();
    const tabsRow = document.createElement("div");
    tabsRow.className = "bp-mgmt-tabs";
    for (const tab of tabs) {
      const btn = document.createElement("button");
      btn.type = "button";
      const active = this.openPanel === tab.id;
      btn.className = "bp-mgmt-tab" + (active ? " active" : "");
      btn.setAttribute("data-testid", `mgmt-tab-${tab.id}`);
      btn.setAttribute("aria-pressed", active ? "true" : "false");

      const lbl = document.createElement("span");
      lbl.textContent = trs(tab.label);
      btn.appendChild(lbl);

      const data = this.getPanelData<{ badge?: { n?: number; tone?: string } }>(tab.id);
      const badge = badgeOf(data);
      if (badge) {
        const b = document.createElement("span");
        b.className = `bp-mgmt-badge bp-tone-${badge.tone}`;
        b.textContent = badge.text;
        btn.appendChild(b);
      }

      btn.addEventListener("click", () => this.togglePanel(tab.id));
      tabsRow.appendChild(btn);
    }
    stripFrag.appendChild(tabsRow);

    if (this.openPanel) {
      const closeBtn = document.createElement("button");
      closeBtn.type = "button";
      closeBtn.className = "icon bp-mgmt-close";
      closeBtn.textContent = "×";
      closeBtn.title = "Close panel";
      closeBtn.setAttribute("aria-label", "Close management panel");
      closeBtn.setAttribute("data-testid", "mgmt-close");
      closeBtn.addEventListener("click", () => this.closePanel());
      stripFrag.appendChild(closeBtn);
    }
    this.stripEl.replaceChildren(stripFrag);

    // 2. Render active panel body
    if (!this.openPanel) {
      this.panelEl.classList.add("bp-hidden");
      this.panelEl.removeAttribute("data-testid");
      this.panelEl.replaceChildren();
      return;
    }

    this.panelEl.classList.remove("bp-hidden");
    this.panelEl.setAttribute("data-testid", `mgmt-panel-${this.openPanel}`);
    const body = document.createDocumentFragment();
    if (this.errorMsg) {
      const err = document.createElement("div");
      err.className = "bp-mgmt-note bp-tone-bad";
      err.textContent = this.errorMsg;
      body.appendChild(err);
    }
    switch (this.openPanel) {
      case "bom":
        body.appendChild(this.renderBom());
        break;
      case "checks":
        body.appendChild(this.renderChecks());
        break;
      case "life":
        body.appendChild(this.renderLife());
        break;
      case "plan":
        body.appendChild(this.renderPlan());
        break;
      case "fab":
        body.appendChild(this.renderFab());
        break;
    }
    this.panelEl.replaceChildren(body);
  }

  // ---- BOM panel ----
  private renderBom(): HTMLElement {
    const wrap = document.createElement("div");
    wrap.className = "bp-mgmt-body";
    const d = this.getPanelData<BomInfo>("bom");
    if (!d) {
      wrap.appendChild(this.makeEmpty("Reading the schematic…"));
      return wrap;
    }
    const lines = Array.isArray(d.lines) ? d.lines : [];

    const head = document.createElement("div");
    head.className = "bp-mgmt-head";
    const summary = document.createElement("span");
    summary.className = "bp-dim";
    summary.textContent = `${d.parts ?? 0} parts · ${lines.length} lines`;
    const src = document.createElement("span");
    const live = d.source === "kicad";
    src.className = "bp-mgmt-pill" + (live ? " accent" : "");
    src.textContent = live ? "live" : "saved";
    head.append(summary, src);
    if ((d.priced ?? 0) > 0 && (d.checking ?? 0) === 0 && typeof d.cost === "number") {
      const cost = document.createElement("span");
      cost.className = "bp-dim";
      cost.textContent = (d.priced ?? 0) < (d.lookups ?? 0) ? `≥ ${money(d.cost)}/board` : `${money(d.cost)}/board`;
      head.appendChild(cost);
    }
    if (d.paused) {
      const paused = document.createElement("span");
      paused.className = "bp-mgmt-faint";
      paused.textContent = "Stock check paused";
      head.appendChild(paused);
    }
    wrap.appendChild(head);

    if (d.stock_note) {
      const note = document.createElement("div");
      note.className = "bp-mgmt-note bp-mgmt-faint";
      note.textContent = trs(d.stock_note);
      wrap.appendChild(note);
    }

    const editingLine = this.bomEditingId ? lines.find((l) => l.id === this.bomEditingId) ?? null : null;
    if (editingLine) {
      wrap.appendChild(this.renderBomEditor(editingLine));
    } else {
      const filterRow = document.createElement("div");
      filterRow.className = "bp-mgmt-filter-row";
      const input = document.createElement("input");
      input.type = "text";
      input.className = "bp-mgmt-filter";
      input.placeholder = "Filter BOM (ref, value, footprint, MPN, dnp)…";
      input.value = this.bomFilter;
      input.setAttribute("data-testid", "mgmt-bom-filter");
      input.addEventListener("input", () => {
        this.bomFilter = input.value;
        this.renderBomRows(listEl, lines);
      });
      filterRow.appendChild(input);
      wrap.appendChild(filterRow);
    }

    const listEl = document.createElement("div");
    listEl.className = "bp-mgmt-rows";
    listEl.setAttribute("data-testid", "mgmt-bom-rows");
    this.renderBomRows(listEl, lines);
    wrap.appendChild(listEl);
    return wrap;
  }

  private renderBomRows(listEl: HTMLElement, lines: BomLine[]): void {
    const shown = filterBomLines(lines, this.bomFilter);
    if (shown.length === 0) {
      listEl.replaceChildren(this.makeEmpty(lines.length === 0 ? "No parts found." : "No matching lines."));
      return;
    }
    const started = lines.some((l) => Boolean(l.mpn) || Boolean(l.fab));
    const frag = document.createDocumentFragment();
    for (const l of shown) {
      const refs = Array.isArray(l.refs) ? l.refs : [];
      const row = document.createElement("div");
      row.className = "bp-mgmt-row" + (this.bomSelectedId === l.id ? " active" : "") + (l.dnp ? " faint" : "");
      row.setAttribute("data-testid", `mgmt-bom-row-${l.id}`);

      const top = document.createElement("div");
      top.className = "bp-mgmt-row-line";
      const refEl = document.createElement("span");
      refEl.className = "bp-mgmt-strong bp-mgmt-flex1";
      refEl.textContent = refsFit(refs, 24);
      top.appendChild(refEl);

      if (l.dnp) {
        const dnp = document.createElement("span");
        dnp.className = "bp-mgmt-pill";
        dnp.textContent = "DNP";
        top.appendChild(dnp);
      } else {
        const st = stockSummary(l.stock);
        if (st) {
          const stEl = document.createElement("span");
          stEl.className = `bp-tone-${st.tone}`;
          stEl.textContent = st.text;
          top.appendChild(stEl);
        }
      }

      const editBtn = document.createElement("button");
      editBtn.type = "button";
      editBtn.className = "bp-mgmt-btn";
      editBtn.textContent = "Edit";
      editBtn.addEventListener("click", (e) => {
        e.stopPropagation();
        this.bomEditingId = l.id;
        this.bomEditDraft = l.mpn ?? "";
        this.bomEditStatus = "";
        this.bomLastUndo = null;
        this.render();
      });
      top.appendChild(editBtn);

      const sub = document.createElement("div");
      sub.className = "bp-mgmt-row-sub";
      const desc = [`${l.qty ?? refs.length}×`, l.value ?? "", pkgName(l.footprint ?? "")].filter(Boolean).join(" · ");
      const descEl = document.createElement("span");
      descEl.className = "bp-dim";
      descEl.textContent = desc;
      const mpnEl = document.createElement("span");
      mpnEl.className = "bp-mgmt-flex1";
      if (l.mpn) {
        mpnEl.textContent = l.mpn;
      } else if (started && !l.dnp) {
        mpnEl.className += " bp-tone-warn";
        mpnEl.textContent = "no MPN";
      }
      sub.append(descEl, mpnEl);
      if (!l.dnp && typeof l.stock?.price === "number") {
        const pr = document.createElement("span");
        pr.className = "bp-dim";
        pr.textContent = money(l.stock.price);
        sub.appendChild(pr);
      }
      if (!l.dnp && l.stock?.class) {
        const cls = document.createElement("span");
        cls.className = "bp-mgmt-faint";
        cls.textContent = trs(l.stock.class);
        sub.appendChild(cls);
      }

      row.append(top, sub);
      row.addEventListener("click", () => {
        this.bomSelectedId = l.id;
        this.renderBomRows(listEl, lines);
      });
      frag.appendChild(row);
    }
    listEl.replaceChildren(frag);
  }

  private renderBomEditor(line: BomLine): HTMLElement {
    const box = document.createElement("div");
    box.className = "bp-mgmt-editor";
    box.setAttribute("data-testid", "mgmt-bom-edit");
    const refs = Array.isArray(line.refs) ? line.refs : [];

    const head = document.createElement("div");
    head.className = "bp-mgmt-row-line";
    const title = document.createElement("span");
    title.className = "bp-mgmt-strong";
    title.textContent = refsFit(refs, 30);
    const closeBtn = document.createElement("button");
    closeBtn.type = "button";
    closeBtn.className = "icon";
    closeBtn.textContent = "×";
    closeBtn.addEventListener("click", () => {
      this.bomEditingId = "";
      this.render();
    });
    head.append(title, closeBtn);

    const input = document.createElement("input");
    input.type = "text";
    input.className = "bp-mgmt-filter";
    input.placeholder = "MPN";
    input.value = this.bomEditDraft;
    input.addEventListener("input", () => { this.bomEditDraft = input.value; });

    const writeField = async (field: string, value: string, isUndo = false): Promise<void> => {
      this.bomEditStatus = "Writing…";
      this.render();
      try {
        const res = await this.cb.hub.rpc<{ changed?: number; via?: string; was?: string }>("mgmt.bom.edit", {
          project: this.project,
          refs,
          field,
          value,
        });
        this.bomLastUndo = !isUndo && (res.changed ?? 0) > 0
          ? { refs, field, was: typeof res.was === "string" ? res.was : (field === "mpn" ? (line.mpn ?? "") : (line.dnp ? "yes" : "no")) }
          : null;
        this.bomEditStatus = isUndo
          ? "Restored"
          : (res.changed ?? 0) === 0
            ? "No change"
            : res.via === "kicad" ? "Updated in KiCad" : "Saved to schematic";
      } catch (e) {
        this.bomEditStatus = e instanceof Error ? e.message : "Could not save edit";
      }
      this.render();
    };

    const actions = document.createElement("div");
    actions.className = "bp-mgmt-btn-row";
    const saveBtn = document.createElement("button");
    saveBtn.type = "button";
    saveBtn.className = "primary bp-mgmt-btn";
    saveBtn.textContent = "Save MPN";
    saveBtn.addEventListener("click", () => { void writeField("mpn", this.bomEditDraft.trim()); });

    const dnpBtn = document.createElement("button");
    dnpBtn.type = "button";
    dnpBtn.className = "bp-mgmt-btn";
    dnpBtn.textContent = line.dnp ? "Clear DNP" : "Mark DNP";
    dnpBtn.addEventListener("click", () => { void writeField("dnp", line.dnp ? "no" : "yes"); });
    actions.append(saveBtn, dnpBtn);

    if (this.bomLastUndo) {
      const undo = this.bomLastUndo;
      const undoBtn = document.createElement("button");
      undoBtn.type = "button";
      undoBtn.className = "bp-mgmt-btn";
      undoBtn.textContent = "Undo";
      undoBtn.addEventListener("click", () => { void writeField(undo.field, undo.was, true); });
      actions.appendChild(undoBtn);
    }

    box.append(head, input, actions);
    if (this.bomEditStatus) {
      const st = document.createElement("div");
      st.className = "bp-mgmt-faint";
      st.textContent = this.bomEditStatus;
      box.appendChild(st);
    }
    return box;
  }

  // ---- Checks panel ----
  private renderChecks(): HTMLElement {
    const wrap = document.createElement("div");
    wrap.className = "bp-mgmt-body";
    const d = this.getPanelData<ChecksInfo>("checks");
    if (!d) {
      wrap.appendChild(this.makeEmpty("Reading checks…"));
      return wrap;
    }
    const groups = Array.isArray(d.groups) ? d.groups : [];
    const sources = Array.from(new Set(groups.map((g) => g.source).filter(Boolean)));
    const labelOf = (s: string): string => d.sources?.[s]?.label || s.toUpperCase();
    const isJudged = (s: string): boolean => d.sources?.[s]?.judged === true;

    const head = document.createElement("div");
    head.className = "bp-mgmt-head bp-mgmt-scroll-x";
    const allBtn = document.createElement("button");
    allBtn.type = "button";
    allBtn.className = "bp-mgmt-btn" + (this.checksSource === "" ? " active" : "");
    allBtn.textContent = "All";
    allBtn.addEventListener("click", () => {
      this.checksSource = "";
      this.render();
    });
    head.appendChild(allBtn);

    for (const s of sources) {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "bp-mgmt-btn" + (this.checksSource === s ? " active" : "");
      btn.textContent = labelOf(s) + (isJudged(s) ? " · judged" : "");
      btn.addEventListener("click", () => {
        this.checksSource = this.checksSource === s ? "" : s;
        this.render();
      });
      head.appendChild(btn);
    }

    const spacer = document.createElement("span");
    spacer.className = "bp-mgmt-flex1";
    const runBtn = document.createElement("button");
    runBtn.type = "button";
    runBtn.className = "bp-mgmt-btn";
    runBtn.textContent = "Run";
    runBtn.addEventListener("click", () => {
      this.checksStatus = "Running checks…";
      this.render();
      void this.cb.hub.rpc("kicad.checks.run", { project: this.project })
        .then(() => { this.checksStatus = ""; this.render(); })
        .catch((e) => {
          this.checksStatus = e instanceof Error ? e.message : "Could not run checks";
          this.render();
        });
    });
    head.append(spacer, runBtn);
    wrap.appendChild(head);

    if (this.checksStatus) {
      const note = document.createElement("div");
      note.className = "bp-mgmt-note bp-dim";
      note.textContent = this.checksStatus;
      wrap.appendChild(note);
    }

    const listEl = document.createElement("div");
    listEl.className = "bp-mgmt-rows";
    listEl.setAttribute("data-testid", "mgmt-checks-rows");
    const filtered = filterCheckGroups(groups, this.checksSource);
    if (filtered.length === 0) {
      listEl.appendChild(this.makeEmpty("No check findings."));
    } else {
      for (const g of filtered) {
        const tone = g.severity === "error" ? "bad" : g.severity === "warning" ? "warn" : "faint";
        const gh = document.createElement("div");
        gh.className = "bp-mgmt-group-head";
        const dot = document.createElement("span");
        dot.className = `bp-mgmt-dot bp-tone-bg-${tone}`;
        const glbl = document.createElement("span");
        glbl.className = "bp-mgmt-flex1";
        glbl.textContent = `${labelOf(g.source)} · ${g.rule}`;
        const cnt = document.createElement("span");
        cnt.className = `bp-tone-${tone}`;
        cnt.textContent = String(g.n);
        gh.append(dot, glbl, cnt);
        listEl.appendChild(gh);

        for (const r of g.rows ?? []) {
          const row = document.createElement("div");
          row.className = "bp-mgmt-row" + (this.checksSelectedId === r.id ? " active" : "");
          const title = document.createElement("div");
          title.textContent = trs(r.title ?? "");
          row.appendChild(title);

          if (r.names || this.checksSelectedId === r.id) {
            const sub = document.createElement("div");
            sub.className = "bp-mgmt-row-sub";
            const names = document.createElement("span");
            names.className = "bp-dim bp-mgmt-flex1";
            names.textContent = r.names ?? "";
            sub.appendChild(names);
            if (this.checksSelectedId === r.id) {
              const fixBtn = document.createElement("button");
              fixBtn.type = "button";
              fixBtn.className = "bp-mgmt-btn accent";
              fixBtn.textContent = "Fix with agent";
              fixBtn.addEventListener("click", (e) => {
                e.stopPropagation();
                void this.cb.hub.rpc<{ text?: string }>("kicad.checks.fix", { project: this.project, id: r.id })
                  .then((res) => {
                    if (res.text) {
                      this.cb.onSendText(res.text);
                      this.checksStatus = "Sent to agent";
                      this.render();
                    }
                  })
                  .catch((err) => {
                    this.checksStatus = err instanceof Error ? err.message : "Could not send finding";
                    this.render();
                  });
              });
              sub.appendChild(fixBtn);
            }
            row.appendChild(sub);
          }

          row.addEventListener("click", () => {
            this.checksSelectedId = r.id;
            void this.cb.hub.rpc("kicad.checks.focus", { project: this.project, id: r.id }).catch(() => {});
            this.render();
          });
          listEl.appendChild(row);
        }
      }
    }
    wrap.appendChild(listEl);
    return wrap;
  }

  // ---- Lifecycle panel ----
  private renderLife(): HTMLElement {
    const wrap = document.createElement("div");
    wrap.className = "bp-mgmt-body";
    const d = this.getPanelData<LifeInfo>("life");
    if (!d) {
      wrap.appendChild(this.makeEmpty("Reading lifecycle…"));
      return wrap;
    }

    const head = document.createElement("div");
    head.className = "bp-mgmt-head";
    const counts = d.counts ?? {};
    for (const k of ["active", "nrnd", "eol"] as const) {
      const n = counts[k] ?? 0;
      const w = lifeWord(k);
      const item = document.createElement("span");
      item.className = n > 0 ? `bp-tone-${w.tone}` : "bp-mgmt-faint";
      item.textContent = `${w.label} ${n}`;
      head.appendChild(item);
    }
    if ((counts.unknown ?? 0) > 0) {
      const unk = document.createElement("span");
      unk.className = "bp-mgmt-faint";
      unk.textContent = `${counts.unknown} unknown`;
      head.appendChild(unk);
    }
    if (d.since) {
      const sp = document.createElement("span");
      sp.className = "bp-mgmt-flex1";
      const since = document.createElement("span");
      since.className = "bp-mgmt-faint";
      since.textContent = `since ${formatUtcDay(d.since)} (${d.samples ?? 0})`;
      head.append(sp, since);
    }
    wrap.appendChild(head);

    const rows = document.createElement("div");
    rows.className = "bp-mgmt-rows";
    rows.setAttribute("data-testid", "mgmt-life-rows");

    const costPts = d.bom?.cost ?? [];
    const instockPts = d.bom?.instock ?? [];
    const linesCount = d.bom?.lines ?? 0;
    if (costPts.length === 0) {
      rows.appendChild(this.makeEmpty(
        linesCount === 0
          ? "No line has an MPN or fab part number to follow yet."
          : "No reading of the fab's stock yet: the BOM panel's lookups start it.",
      ));
    } else {
      const latest = costPts[costPts.length - 1];
      const latestStock = instockPts[instockPts.length - 1];
      const note = document.createElement("div");
      note.className = "bp-mgmt-note bp-dim";
      note.textContent = `Parts/board: ${money(latest[1])} · ${Math.round(latestStock?.[1] ?? 0)}/${linesCount} lines in stock (${formatUtcDay(latest[0])})`;
      rows.appendChild(note);
    }

    for (const p of d.parts ?? []) {
      const active = this.lifeSelectedKey === p.key;
      const w = lifeWord(p.lifecycle ?? "unknown");
      const row = document.createElement("div");
      row.className = "bp-mgmt-row" + (active ? " active" : "");
      const line = document.createElement("div");
      line.className = "bp-mgmt-row-line";
      const refEl = document.createElement("span");
      refEl.className = "bp-mgmt-strong";
      refEl.textContent = refsFit(p.refs ?? [], 14);
      const mpnEl = document.createElement("span");
      mpnEl.className = "bp-mgmt-flex1 bp-dim";
      mpnEl.textContent = p.mpn || p.key;
      const stEl = document.createElement("span");
      stEl.className = `bp-tone-${w.tone}`;
      stEl.textContent = w.label;
      line.append(refEl, mpnEl, stEl);
      if (typeof p.stock === "number") {
        const s = document.createElement("span");
        s.className = "bp-dim";
        s.textContent = abbrev(p.stock);
        line.appendChild(s);
      }
      row.appendChild(line);

      if (active) {
        const detail = document.createElement("div");
        detail.className = "bp-mgmt-row-sub bp-mgmt-faint";
        if (this.lifePartLoading) {
          detail.textContent = "Loading history…";
        } else if (this.lifePartSamples && this.lifePartSamples.length > 0) {
          const last = this.lifePartSamples[this.lifePartSamples.length - 1];
          detail.textContent = `${this.lifePartSamples.length} samples · latest ${last.at ? formatUtcDay(last.at) : ""}` +
            (typeof last.stock === "number" ? ` · stock ${abbrev(last.stock)}` : "") +
            (typeof last.price === "number" ? ` · ${money(last.price)}` : "");
        } else {
          detail.textContent = "No history samples yet.";
        }
        row.appendChild(detail);
      }

      row.addEventListener("click", () => {
        if (this.lifeSelectedKey === p.key) {
          this.lifeSelectedKey = "";
          this.lifePartSamples = null;
          this.render();
          return;
        }
        this.lifeSelectedKey = p.key;
        this.lifePartSamples = null;
        this.lifePartLoading = true;
        this.render();
        void this.cb.hub.rpc<{ key?: string; samples?: Array<{ at?: number; stock?: number; price?: number; lifecycle?: string }> }>("mgmt.life.part", {
          project: this.project,
          key: p.key,
        })
          .then((res) => {
            if (this.lifeSelectedKey !== p.key) return;
            this.lifePartLoading = false;
            this.lifePartSamples = Array.isArray(res.samples) ? res.samples : [];
            this.render();
          })
          .catch(() => {
            if (this.lifeSelectedKey !== p.key) return;
            this.lifePartLoading = false;
            this.lifePartSamples = [];
            this.render();
          });
      });
      rows.appendChild(row);
    }

    wrap.appendChild(rows);
    return wrap;
  }

  // ---- Plan panel ----
  private renderPlan(): HTMLElement {
    const wrap = document.createElement("div");
    wrap.className = "bp-mgmt-body";
    const d = this.getPanelData<PlanInfo>("plan");
    if (!d) {
      wrap.appendChild(this.makeEmpty("Reading the routing plan…"));
      return wrap;
    }

    const sendPlan = (method: string, params: Record<string, unknown>): void => {
      this.planStatus = "";
      void this.cb.hub.rpc(method, { pro: this.project, ...params })
        .catch((e) => {
          this.planStatus = e instanceof Error ? e.message : "Plan action refused";
          this.render();
        });
    };

    if (d.empty) {
      wrap.appendChild(this.makeEmpty("No routing plan for this board."));
      return wrap;
    }

    const head = document.createElement("div");
    head.className = "bp-mgmt-head";
    const awaiting = d.awaiting ?? 0;
    const status = document.createElement("span");
    status.className = "bp-mgmt-flex1 " + (awaiting > 0 ? "bp-tone-warn" : "bp-dim");
    status.textContent = awaiting > 0 ? `${awaiting} awaiting approval` : "Nothing awaits your approval";
    head.appendChild(status);

    if (!d.live) {
      const saved = document.createElement("span");
      saved.className = "bp-mgmt-faint";
      saved.textContent = "saved plan";
      head.appendChild(saved);
    }
    if (awaiting > 0) {
      const okAll = document.createElement("button");
      okAll.type = "button";
      okAll.className = "bp-mgmt-btn";
      okAll.textContent = "Approve all";
      okAll.addEventListener("click", () => sendPlan("plan.approve", { layer: "", ids: [], approve: true }));
      head.appendChild(okAll);
    }
    if (d.routing) {
      const stop = document.createElement("button");
      stop.type = "button";
      stop.className = "bp-mgmt-btn";
      stop.textContent = "Stop routing";
      stop.addEventListener("click", () => sendPlan("plan.stop", {}));
      head.appendChild(stop);
    }
    const hideBtn = document.createElement("button");
    hideBtn.type = "button";
    hideBtn.className = "bp-mgmt-btn";
    hideBtn.textContent = this.planHideDone ? "Show done" : "Hide done";
    hideBtn.addEventListener("click", () => {
      this.planHideDone = !this.planHideDone;
      this.render();
    });
    head.appendChild(hideBtn);
    wrap.appendChild(head);

    if (this.planStatus) {
      const err = document.createElement("div");
      err.className = "bp-mgmt-note bp-tone-bad";
      err.textContent = this.planStatus;
      wrap.appendChild(err);
    }

    const listEl = document.createElement("div");
    listEl.className = "bp-mgmt-rows";
    listEl.setAttribute("data-testid", "mgmt-plan-rows");
    for (const l of d.layers ?? []) {
      const items = l.items ?? [];
      const gh = document.createElement("div");
      gh.className = "bp-mgmt-group-head";
      const lbl = document.createElement("span");
      lbl.className = "bp-mgmt-strong";
      lbl.textContent = l.layer;
      const meta = document.createElement("span");
      meta.className = "bp-dim bp-mgmt-flex1";
      meta.textContent = `${l.role ? `${l.role} · ` : ""}${l.done ?? 0}/${items.length} done`;
      gh.append(lbl, meta);
      if (l.status !== "approved" && items.length > 0) {
        const okLayer = document.createElement("button");
        okLayer.type = "button";
        okLayer.className = "bp-mgmt-btn";
        okLayer.textContent = "Approve layer";
        okLayer.addEventListener("click", () => sendPlan("plan.approve", { layer: l.layer, ids: [], approve: true }));
        const rejLayer = document.createElement("button");
        rejLayer.type = "button";
        rejLayer.className = "bp-mgmt-btn";
        rejLayer.textContent = "Reject";
        rejLayer.addEventListener("click", () => sendPlan("plan.approve", { layer: l.layer, ids: [], approve: false }));
        gh.append(okLayer, rejLayer);
      }
      listEl.appendChild(gh);

      for (const it of items) {
        if (this.planHideDone && planDone(it)) continue;
        const row = document.createElement("div");
        row.className = "bp-mgmt-row";
        const line = document.createElement("div");
        line.className = "bp-mgmt-row-line";
        const name = document.createElement("span");
        name.className = "bp-mgmt-flex1" + (it.state === "blocked" ? " bp-tone-bad" : it.state === "edited" ? " bp-tone-warn" : "");
        name.textContent = `${it.name || it.id}${it.locked ? " 🔒" : ""}`;
        const detail = document.createElement("span");
        detail.className = "bp-dim";
        detail.textContent = it.kind === "plane" ? (it.reference ? `ref ${it.reference}` : "") : `${it.nets ?? 0} nets`;
        const mk = document.createElement("span");
        mk.className = it.state === "blocked" ? "bp-tone-bad" : "bp-dim";
        mk.textContent = planMark(it);
        line.append(name, detail, mk);
        if (it.state === "proposed" || it.state === "edited") {
          const ok = document.createElement("button");
          ok.type = "button";
          ok.className = "bp-mgmt-btn";
          ok.textContent = "✓";
          ok.addEventListener("click", (e) => { e.stopPropagation(); sendPlan("plan.approve", { layer: "", ids: [it.id], approve: true }); });
          const no = document.createElement("button");
          no.type = "button";
          no.className = "bp-mgmt-btn";
          no.textContent = "✗";
          no.addEventListener("click", (e) => { e.stopPropagation(); sendPlan("plan.approve", { layer: "", ids: [it.id], approve: false }); });
          line.append(ok, no);
        }
        row.appendChild(line);
        row.addEventListener("click", () => sendPlan("plan.focus", { id: it.id }));
        listEl.appendChild(row);
      }
    }
    wrap.appendChild(listEl);
    return wrap;
  }

  // ---- Fab panel ----
  private renderFab(): HTMLElement {
    const wrap = document.createElement("div");
    wrap.className = "bp-mgmt-body";
    const d = this.getPanelData<FabInfo>("fab");
    if (!d) {
      wrap.appendChild(this.makeEmpty("Reading the fab's profile…"));
      return wrap;
    }
    if (d.error) {
      const err = document.createElement("div");
      err.className = "bp-mgmt-note bp-tone-warn";
      err.textContent = d.error;
      wrap.appendChild(err);
      return wrap;
    }

    const sendFab = (action: string, extra: Record<string, unknown> = {}): void => {
      this.fabStatus = "";
      void this.cb.hub.rpc("fab.profile", { pro: this.project, action, ...extra })
        .catch((e) => {
          this.fabStatus = e instanceof Error ? e.message : "Fab action failed";
          this.render();
        });
    };

    const head = document.createElement("div");
    head.className = "bp-mgmt-head";
    const title = document.createElement("span");
    title.className = "bp-mgmt-strong";
    title.textContent = "The fab";
    const meta = document.createElement("span");
    meta.className = "bp-dim bp-mgmt-flex1";
    meta.textContent = `rev ${d.rev ?? 0}${d.updated ? ` · ${d.updated}` : ""}${d.board ? ` · ${d.board}` : ""} · ${tierWords(d.tier ?? "")}`;
    const refreshBtn = document.createElement("button");
    refreshBtn.type = "button";
    refreshBtn.className = "bp-mgmt-btn";
    refreshBtn.textContent = "Refresh";
    refreshBtn.addEventListener("click", () => sendFab("refresh"));
    head.append(title, meta, refreshBtn);
    wrap.appendChild(head);

    if (this.fabStatus) {
      const err = document.createElement("div");
      err.className = "bp-mgmt-note bp-tone-bad";
      err.textContent = this.fabStatus;
      wrap.appendChild(err);
    }

    const listEl = document.createElement("div");
    listEl.className = "bp-mgmt-rows";
    listEl.setAttribute("data-testid", "mgmt-fab-rows");

    // Review row
    const rv = d.review;
    const rvRow = document.createElement("div");
    rvRow.className = "bp-mgmt-row";
    const rvLine = document.createElement("div");
    rvLine.className = "bp-mgmt-row-line";
    const rvText = document.createElement("span");
    const rvBad = (rv?.counts?.error ?? 0) > 0;
    rvText.className = "bp-mgmt-flex1 " + (rvBad ? "bp-tone-bad" : "bp-dim");
    rvText.textContent = rv?.running
      ? "Reviewing the design…"
      : rv?.at == null
        ? "No design review yet"
        : `${rv.counts?.error ?? 0} must-fix · ${rv.counts?.warning ?? 0} should-fix · ${rv.counts?.info ?? 0} notes${rv.scope ? ` (${rv.scope})` : ""}`;
    rvLine.appendChild(rvText);
    if (rv?.at != null) {
      const fBtn = document.createElement("button");
      fBtn.type = "button";
      fBtn.className = "bp-mgmt-btn";
      fBtn.textContent = "Findings";
      fBtn.addEventListener("click", () => this.togglePanel("checks"));
      rvLine.appendChild(fBtn);
    }
    if (!rv?.running) {
      const rBtn = document.createElement("button");
      rBtn.type = "button";
      rBtn.className = "bp-mgmt-btn";
      rBtn.textContent = "Review";
      rBtn.addEventListener("click", () => sendFab("review"));
      rvLine.appendChild(rBtn);
    }
    if (rv?.judge) {
      const jBtn = document.createElement("button");
      jBtn.type = "button";
      jBtn.className = "bp-mgmt-btn";
      jBtn.textContent = "Datasheets";
      jBtn.addEventListener("click", () => { if (rv.judge) this.cb.onSendText(rv.judge); });
      rvLine.appendChild(jBtn);
    }
    rvRow.appendChild(rvLine);
    listEl.appendChild(rvRow);

    // DRC row
    const drc = drcWords(d.drc?.state ?? "");
    const drcRow = document.createElement("div");
    drcRow.className = "bp-mgmt-row";
    const drcLine = document.createElement("div");
    drcLine.className = "bp-mgmt-row-line";
    const drcText = document.createElement("span");
    drcText.className = "bp-mgmt-flex1 " + (drc.button ? "bp-tone-warn" : "bp-dim");
    drcText.textContent = drc.text;
    drcLine.appendChild(drcText);
    if (drc.button) {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "bp-mgmt-btn";
      btn.textContent = drc.button;
      btn.addEventListener("click", () => sendFab("drc"));
      drcLine.appendChild(btn);
    }
    drcRow.appendChild(drcLine);
    listEl.appendChild(drcRow);

    // Conflicts & Suggestions
    for (const c of d.conflicts ?? []) {
      const cr = document.createElement("div");
      cr.className = "bp-mgmt-row bp-tone-bad";
      cr.textContent = c;
      listEl.appendChild(cr);
    }
    for (const s of d.suggestions ?? []) {
      const sr = document.createElement("div");
      sr.className = "bp-mgmt-row";
      const sl = document.createElement("div");
      sl.className = "bp-mgmt-row-line";
      const st = document.createElement("span");
      st.className = "bp-mgmt-flex1 bp-tone-warn";
      st.textContent = `${s.label || s.key}: board ${s.project ?? "?"} mm, fab ${s.fab ?? "?"} mm`;
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "bp-mgmt-btn";
      btn.textContent = "Apply";
      btn.addEventListener("click", () => sendFab("apply", { key: s.key }));
      sl.append(st, btn);
      sr.appendChild(sl);
      listEl.appendChild(sr);
    }

    // Stackup
    if (d.stackup?.chosen) {
      const stRow = document.createElement("div");
      stRow.className = "bp-mgmt-row";
      const stLine = document.createElement("div");
      stLine.className = "bp-mgmt-row-line";
      const proposed = d.stackup.by === "proposed";
      const stTxt = document.createElement("span");
      stTxt.className = "bp-mgmt-flex1 " + (proposed ? "bp-tone-warn" : "bp-dim");
      stTxt.textContent = `${proposed ? "Proposed: " : ""}${d.stackup.chosen}${d.stackup.outer_50_mm ? ` (50Ω ${d.stackup.outer_50_mm} mm)` : ""}`;
      stLine.appendChild(stTxt);
      if (proposed) {
        const id = d.stackup.chosen;
        const btn = document.createElement("button");
        btn.type = "button";
        btn.className = "bp-mgmt-btn";
        btn.textContent = "Use";
        btn.addEventListener("click", () => sendFab("stackup", { id }));
        stLine.appendChild(btn);
      }
      stRow.appendChild(stLine);
      listEl.appendChild(stRow);
    }

    // Changed since last looked
    if (Array.isArray(d.changed) && d.changed.length > 0) {
      const chRow = document.createElement("div");
      chRow.className = "bp-mgmt-row";
      const chHead = document.createElement("div");
      chHead.className = "bp-mgmt-row-line";
      const chLbl = document.createElement("span");
      chLbl.className = "bp-mgmt-flex1 bp-tone-warn";
      chLbl.textContent = "Changed since you last looked";
      const seenBtn = document.createElement("button");
      seenBtn.type = "button";
      seenBtn.className = "bp-mgmt-btn";
      seenBtn.textContent = "Seen";
      seenBtn.addEventListener("click", () => sendFab("seen"));
      chHead.append(chLbl, seenBtn);
      chRow.appendChild(chHead);
      for (const item of d.changed.slice(0, 12)) {
        const line = document.createElement("div");
        line.className = "bp-mgmt-faint";
        line.textContent = item;
        chRow.appendChild(line);
      }
      listEl.appendChild(chRow);
    }

    // Limits table
    const limits = Array.isArray(d.limits) ? d.limits : [];
    if (limits.length > 0) {
      const lh = document.createElement("div");
      lh.className = "bp-mgmt-group-head";
      const c0 = document.createElement("span");
      c0.className = "bp-mgmt-flex1";
      c0.textContent = "Limit";
      const c1 = document.createElement("span");
      c1.className = "bp-mgmt-col";
      c1.textContent = "Base price";
      const c2 = document.createElement("span");
      c2.className = "bp-mgmt-col";
      c2.textContent = "Costs more";
      lh.append(c0, c1, c2);
      listEl.appendChild(lh);

      for (const l of limits) {
        const lr = document.createElement("div");
        lr.className = "bp-mgmt-row";
        const ll = document.createElement("div");
        ll.className = "bp-mgmt-row-line";
        const nm = document.createElement("span");
        nm.className = "bp-mgmt-flex1" + (l.changed ? " bp-tone-warn" : l.inherited ? " bp-dim" : "");
        nm.textContent = `${l.changed ? "• " : ""}${l.label ?? ""}`;
        const f = document.createElement("span");
        f.className = "bp-mgmt-col";
        f.textContent = l.free?.text || "—";
        const ex = document.createElement("span");
        ex.className = "bp-mgmt-col bp-dim";
        ex.textContent = l.extra?.text || "—";
        ll.append(nm, f, ex);
        lr.appendChild(ll);
        listEl.appendChild(lr);
      }
    }

    wrap.appendChild(listEl);
    return wrap;
  }

  private makeEmpty(text: string): HTMLElement {
    const el = document.createElement("div");
    el.className = "bp-mgmt-note bp-mgmt-faint";
    el.textContent = text;
    return el;
  }
}
