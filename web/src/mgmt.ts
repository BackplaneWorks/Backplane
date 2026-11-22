// Hub-driven Management Strip (BOM | Checks | Lifecycle | Plan | Fab):
// Matches docs/MANAGEMENT.md, DataMgmt.kt, and native/gpui/src/screens/mgmt*.rs.
// All tab order, badges, counts, and rows come from the hub's `mgmt.*` and `kicad.checks.*`
// info frames; clicking a tab calls `mgmt.panel { project, toggle }` and opens the panel.
import { t, trs } from "./i18n";
import {
  abbrev,
  badgeOf,
  buildBomUndoParams,
  drcWords,
  filterBomLines,
  filterCheckGroups,
  formatUtcDay,
  isBomWriteCurrent,
  isChecksFixCurrent,
  isPlanHydrationCurrent,
  lifeWord,
  mgmtPanelsOf,
  money,
  pkgName,
  planBusColor,
  planAllCanvasLayers,
  planDefaultLayerPair,
  planDone,
  planMark,
  planProgressSegments,
  planRailColor,
  planStatusBadge,
  refsFit,
  stockSummary,
  tierWords,
  unwrapPlanGetPanel,
  type BomInfo,
  type BomLine,
  type BomUndoRecord,
  type BomWriteSnapshot,
  type ChecksFixSnapshot,
  type ChecksInfo,
  type FabInfo,
  type LifeInfo,
  type MgmtStateInfo,
  type PlanBand,
  type PlanBoardGeom,
  type PlanCopperLayer,
  type PlanInfo,
  type PlanItem,
  type PlanLayer,
  type PlanRegion,
} from "./mgmt-model";
import { PlanFloorplannerCanvas, type PlanGridMode, type PlanViewMode } from "./plan-canvas";
import type { HubClient } from "./ws";

export interface MgmtCallbacks {
  hub: HubClient;
  getThreadId: () => string;
  getThreadGen: () => number;
  onSendText: (text: string) => void;
  onOpenChange?: (open: boolean) => void;
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
  private bomEditGen = 0;
  private bomEditDraft = "";
  private bomEditStatus = "";
  private bomLastUndo: BomUndoRecord | null = null;

  private checksSource = "";
  private checksSelectedId = "";
  private checksStatus = "";

  private lifeSelectedKey = "";
  private lifePartSamples: Array<{ at?: number; stock?: number; price?: number; lifecycle?: string }> | null = null;
  private lifePartLoading = false;

  private planHideDone = false;
  private planStatus = "";
  private planSelectedId = "";
  private planStackupOpen = false;
  private planViewMode: PlanViewMode = "split";
  private planGridMode: PlanGridMode = "pair";
  private planSyncZoom = true;
  private planExpanded = false;
  private planShownLayers: string[] = [];
  private planOptimistic = new Map<string, Record<string, unknown>>();
  private planOptimisticRev: number | undefined = undefined;
  private planHydratedKey = "";
  private planHydrateAskedKey = "";
  private planHydrateGen = 0;
  private planHydratedRev: number | undefined = undefined;
  private planRenderDeferred = false;
  private planHydratedBoard: PlanBoardGeom | null = null;
  private planHydratedDraw: { bands?: PlanBand[]; regions?: PlanRegion[] } | null = null;
  private planNoteOpen = false;
  private planCanvas: PlanFloorplannerCanvas | null = null;
  private planCommitFields: ((id: string, fields: Record<string, unknown>) => void) | null = null;

  private fabStatus = "";

  constructor(panelSlot: HTMLElement, stripSlot: HTMLElement, cb: MgmtCallbacks) {
    this.cb = cb;

    this.hostEl = stripSlot;
    this.panelEl = document.createElement("aside");
    this.panelEl.className = "bp-mgmt-panel bp-hidden";
    this.panelEl.setAttribute("aria-label", "Project management panel");

    this.stripEl = document.createElement("div");
    this.stripEl.className = "bp-mgmt-strip bp-hidden";
    this.stripEl.setAttribute("data-testid", "mgmt-strip");
    this.stripEl.setAttribute("aria-label", "Project management tabs");

    panelSlot.appendChild(this.panelEl);
    stripSlot.appendChild(this.stripEl);
  }

  public openTab(id: string): void {
    const state = this.getStateInfo();
    if (!mgmtPanelsOf(state).some((p) => p.id === id)) return;
    if (this.openPanel !== id) this.togglePanel(id);
  }

  public getTabs(): Array<{ id: string; label: string }> {
    return mgmtPanelsOf(this.getStateInfo());
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
    this.bomEditGen++;
    this.askedProject = "";
    this.digest = "";
    this.project = "";
    this.openPanel = "";
    this.errorMsg = "";
    this.bomEditingId = "";
    this.bomLastUndo = null;
    this.bomEditStatus = "";
    this.checksStatus = "";
    this.planStatus = "";
    this.planSelectedId = "";
    this.planStackupOpen = false;
    this.planShownLayers = [];
    this.planOptimistic.clear();
    this.planOptimisticRev = undefined;
    this.planHydratedKey = "";
    this.planHydrateAskedKey = "";
    this.planHydrateGen++;
    this.planHydratedRev = undefined;
    this.planRenderDeferred = false;
    this.planHydratedBoard = null;
    this.planHydratedDraw = null;
    this.planNoteOpen = false;
    this.planCanvas?.destroy();
    this.planCanvas = null;
    this.planCommitFields = null;
    this.fabStatus = "";
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
      this.bomEditGen++;
      this.digest = "";
      this.project = "";
      this.openPanel = "";
      this.errorMsg = "";
      this.bomSelectedId = "";
      this.bomEditingId = "";
      this.bomEditStatus = "";
      this.bomLastUndo = null;
      this.checksSelectedId = "";
      this.checksStatus = "";
      this.planStatus = "";
      this.planSelectedId = "";
      this.planStackupOpen = false;
      this.planShownLayers = [];
      this.planOptimistic.clear();
      this.planOptimisticRev = undefined;
      this.planHydratedKey = "";
      this.planHydrateAskedKey = "";
      this.planHydrateGen++;
      this.planHydratedRev = undefined;
      this.planRenderDeferred = false;
      this.planHydratedBoard = null;
      this.planHydratedDraw = null;
      this.planNoteOpen = false;
      this.planCanvas?.destroy();
      this.planCanvas = null;
      this.planCommitFields = null;
      this.fabStatus = "";
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
    if (this.openPanel === "plan" && this.planCanvas?.isDragging()) {
      this.planRenderDeferred = true;
      return;
    }
    this.planRenderDeferred = false;
    const state = this.getStateInfo();
    const tabs = mgmtPanelsOf(state);
    if (!this.digest || tabs.length === 0) {
      this.planCanvas?.destroy();
      this.planCanvas = null;
      this.panelEl.classList.remove("bp-plan-expanded");
      this.stripEl.classList.add("bp-hidden");
      this.panelEl.classList.add("bp-hidden");
      this.panelEl.removeAttribute("data-testid");
      this.panelEl.replaceChildren();
      this.stripEl.replaceChildren();
      this.cb.onOpenChange?.(false);
      return;
    }

    this.stripEl.classList.remove("bp-hidden");

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
      closeBtn.title = t("gpui.mgmt.close", "Close the panel");
      closeBtn.setAttribute("aria-label", t("gpui.mgmt.close", "Close the panel"));
      closeBtn.setAttribute("data-testid", "mgmt-close");
      closeBtn.addEventListener("click", () => this.closePanel());
      stripFrag.appendChild(closeBtn);
    }
    this.stripEl.replaceChildren(stripFrag);

    // 2. Render active panel body
    if (!this.openPanel) {
      this.planCanvas?.destroy();
      this.planCanvas = null;
      this.panelEl.classList.remove("bp-plan-expanded");
      this.panelEl.classList.add("bp-hidden");
      this.panelEl.removeAttribute("data-testid");
      this.panelEl.replaceChildren();
      this.cb.onOpenChange?.(false);
      return;
    }

    if (this.openPanel !== "plan") {
      this.planCanvas?.destroy();
      this.planCanvas = null;
      this.panelEl.classList.remove("bp-plan-expanded");
    } else {
      this.panelEl.classList.toggle("bp-plan-expanded", this.planExpanded);
    }
    this.panelEl.classList.remove("bp-hidden");
    this.panelEl.setAttribute("data-testid", `mgmt-panel-${this.openPanel}`);
    this.cb.onOpenChange?.(true);
    const keepCanvasFocus =
      this.openPanel === "plan" &&
      this.planCanvas !== null &&
      document.activeElement instanceof HTMLElement &&
      this.panelEl.contains(document.activeElement) &&
      (document.activeElement === this.planCanvas.root || this.planCanvas.root.contains(document.activeElement));
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
    if (keepCanvasFocus) {
      this.planCanvas?.root.focus({ preventScroll: true });
    }
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
        dnp.textContent = t("gpui.mgmt.bom.dnp", "DNP");
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
        this.bomEditGen++;
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
        mpnEl.textContent = t("gpui.mgmt.bom.nompn", "no MPN");
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
      this.bomEditGen++;
      this.bomEditingId = "";
      this.bomLastUndo = null;
      this.bomEditStatus = "";
      this.render();
    });
    head.append(title, closeBtn);

    const input = document.createElement("input");
    input.type = "text";
    input.className = "bp-mgmt-filter";
    input.placeholder = "MPN";
    input.value = this.bomEditDraft;
    input.addEventListener("input", () => { this.bomEditDraft = input.value; });

    const currentBomSnap = (): BomWriteSnapshot => ({
      watchGen: this.watchGen,
      editGen: this.bomEditGen,
      project: this.project,
      editId: this.bomEditingId,
    });

    const writeField = async (
      params: { project: string; refs: string[]; field: string; value: string },
      isUndo = false,
    ): Promise<void> => {
      const start = currentBomSnap();
      this.bomEditStatus = "Writing…";
      this.render();
      try {
        const res = await this.cb.hub.rpc<{ changed?: number; via?: string; was?: string }>("mgmt.bom.edit", params);
        if (!isBomWriteCurrent(start, currentBomSnap())) return;
        this.bomLastUndo = !isUndo && (res.changed ?? 0) > 0
          ? {
            project: params.project,
            editId: start.editId,
            refs: params.refs.slice(),
            field: params.field,
            was: typeof res.was === "string" ? res.was : (params.field === "mpn" ? (line.mpn ?? "") : (line.dnp ? "yes" : "no")),
          }
          : null;
        this.bomEditStatus = isUndo
          ? "Restored"
          : (res.changed ?? 0) === 0
            ? "No change"
            : res.via === "kicad" ? "Updated in KiCad" : "Saved to schematic";
      } catch (e) {
        if (!isBomWriteCurrent(start, currentBomSnap())) return;
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
    saveBtn.addEventListener("click", () => {
      void writeField({ project: this.project, refs: refs.slice(), field: "mpn", value: this.bomEditDraft.trim() });
    });

    const dnpBtn = document.createElement("button");
    dnpBtn.type = "button";
    dnpBtn.className = "bp-mgmt-btn";
    dnpBtn.textContent = line.dnp ? "Clear DNP" : "Mark DNP";
    dnpBtn.addEventListener("click", () => {
      void writeField({ project: this.project, refs: refs.slice(), field: "dnp", value: line.dnp ? "no" : "yes" });
    });
    actions.append(saveBtn, dnpBtn);

    if (this.bomLastUndo && this.bomLastUndo.project === this.project && this.bomLastUndo.editId === this.bomEditingId) {
      const undo = this.bomLastUndo;
      const undoBtn = document.createElement("button");
      undoBtn.type = "button";
      undoBtn.className = "bp-mgmt-btn";
      undoBtn.textContent = "Undo";
      undoBtn.addEventListener("click", () => { void writeField(buildBomUndoParams(undo), true); });
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
    allBtn.textContent = t("gpui.mgmt.checks.all", "All");
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
    runBtn.textContent = t("gpui.mgmt.checks.run", "Run");
    runBtn.addEventListener("click", () => {
      const gen = this.watchGen;
      const proj = this.project;
      this.checksStatus = "Running checks…";
      this.render();
      void this.cb.hub.rpc("kicad.checks.run", { project: proj })
        .then(() => {
          if (gen !== this.watchGen || proj !== this.project) return;
          this.checksStatus = "";
          this.render();
        })
        .catch((e) => {
          if (gen !== this.watchGen || proj !== this.project) return;
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
              fixBtn.textContent = t("gpui.mgmt.checks.fix", "Fix with agent");
              fixBtn.addEventListener("click", (e) => {
                e.stopPropagation();
                const start: ChecksFixSnapshot = {
                  watchGen: this.watchGen,
                  project: this.project,
                  threadId: this.cb.getThreadId(),
                  threadGen: this.cb.getThreadGen(),
                };
                void this.cb.hub.rpc<{ text?: string }>("kicad.checks.fix", { project: start.project, id: r.id })
                  .then((res) => {
                    const now: ChecksFixSnapshot = {
                      watchGen: this.watchGen,
                      project: this.project,
                      threadId: this.cb.getThreadId(),
                      threadGen: this.cb.getThreadGen(),
                    };
                    if (!isChecksFixCurrent(start, now)) return;
                    if (res.text) {
                      this.cb.onSendText(res.text);
                      this.checksStatus = "Sent to agent";
                      this.render();
                    }
                  })
                  .catch((err) => {
                    const now: ChecksFixSnapshot = {
                      watchGen: this.watchGen,
                      project: this.project,
                      threadId: this.cb.getThreadId(),
                      threadGen: this.cb.getThreadGen(),
                    };
                    if (!isChecksFixCurrent(start, now)) return;
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
          detail.textContent = t("gpui.mgmt.life.reading", "Reading its history…");
        } else if (this.lifePartSamples && this.lifePartSamples.length > 0) {
          const last = this.lifePartSamples[this.lifePartSamples.length - 1];
          detail.textContent = `${this.lifePartSamples.length} samples · latest ${last.at ? formatUtcDay(last.at) : ""}` +
            (typeof last.stock === "number" ? ` · stock ${abbrev(last.stock)}` : "") +
            (typeof last.price === "number" ? ` · ${money(last.price)}` : "");
        } else {
          detail.textContent = t("gpui.mgmt.life.nopart", "No reading of this part yet.");
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
      wrap.appendChild(this.makeEmpty(t("gpui.mgmt.plan.reading", "Reading the routing plan…")));
      return wrap;
    }

    const sendPlan = (method: string, params: Record<string, unknown>): void => {
      const gen = this.watchGen;
      const proj = this.project;
      this.planStatus = "";
      void this.cb.hub.rpc(method, { pro: proj, ...params })
        .catch((e) => {
          if (gen !== this.watchGen || proj !== this.project) return;
          this.planOptimistic.clear();
          this.planStatus = e instanceof Error ? e.message : t("gpui.mgmt.plan.refused", "KiCad refused");
          this.render();
        });
    };

    if (d.revision !== this.planOptimisticRev) {
      this.planOptimisticRev = d.revision;
      this.planOptimistic.clear();
    }

    const hydKey = `${this.project}:${d.revision ?? 0}`;
    if (d.board_trimmed && this.planHydrateAskedKey !== hydKey && this.project) {
      this.planHydrateAskedKey = hydKey;
      const gen = this.watchGen;
      const hydGen = ++this.planHydrateGen;
      const proj = this.project;
      const fallbackRev = d.revision ?? 0;
      void this.cb.hub.rpc<unknown>("plan.get", { pro: proj, board: true })
        .then((res) => {
          const panel = unwrapPlanGetPanel(res);
          if (!panel) return;
          const panelRev = typeof panel.revision === "number" ? panel.revision : fallbackRev;
          if (!isPlanHydrationCurrent(gen, this.watchGen, hydGen, this.planHydrateGen, proj, this.project, panelRev, this.planHydratedRev)) {
            return;
          }
          this.planHydratedRev = panelRev;
          this.planHydratedKey = `${proj}:${panelRev}`;
          this.planHydratedBoard = panel.board ?? null;
          this.planHydratedDraw = panel.draw ?? null;
          this.render();
        })
        .catch(() => {});
    }

    const toggleRow = document.createElement("div");
    toggleRow.className = "bp-plan-toggle";
    const on = d.on === true;
    const toggleLbl = document.createElement("span");
    toggleLbl.className = "bp-mgmt-flex1 bp-mgmt-strong";
    toggleLbl.textContent = t("gpui.mgmt.plan.before", "Plan before routing");
    toggleLbl.title = on
      ? t("gpui.mgmt.plan.onwords", "Before routing a board with 4 or more copper layers or more than 60 nets, the agent proposes corridors and plane regions here. It routes only what you approve.")
      : t("gpui.mgmt.plan.offwords", "Agents route as you ask. Turn it on to have them propose a routing plan first and route only what you approve here.");
    const onBtn = document.createElement("button");
    onBtn.type = "button";
    onBtn.className = "bp-mgmt-btn bp-plan-chip-btn" + (on ? " active" : "");
    onBtn.setAttribute("data-testid", "mgmt-plan-on");
    onBtn.setAttribute("aria-pressed", on ? "true" : "false");
    onBtn.textContent = t("gpui.bots.recall.on", "On");
    onBtn.addEventListener("click", () => sendPlan("plan.enable", { on: true }));
    const offBtn = document.createElement("button");
    offBtn.type = "button";
    offBtn.className = "bp-mgmt-btn bp-plan-chip-btn" + (!on ? " active" : "");
    offBtn.setAttribute("data-testid", "mgmt-plan-off");
    offBtn.setAttribute("aria-pressed", !on ? "true" : "false");
    offBtn.textContent = t("gpui.bots.recall.off", "Off");
    offBtn.addEventListener("click", () => sendPlan("plan.enable", { on: false }));
    toggleRow.append(toggleLbl, onBtn, offBtn);
    wrap.appendChild(toggleRow);

    for (const e of d.elsewhere ?? []) {
      const bar = document.createElement("div");
      bar.className = "bp-plan-elsewhere";
      const txt = document.createElement("span");
      txt.className = "bp-tone-warn bp-mgmt-flex1";
      txt.textContent = t("gpui.mgmt.plan.elsewhere", "A routing plan for {board} awaits your approval", {
        board: e.name || e.rel || "",
      });
      bar.appendChild(txt);
      wrap.appendChild(bar);
    }

    if (d.empty) {
      const emptyBox = document.createElement("div");
      emptyBox.className = "bp-mgmt-note";
      const lead = document.createElement("div");
      lead.className = "bp-dim";
      lead.textContent = t("gpui.mgmt.plan.noboard", "No routing plan for this board.");
      const sub = document.createElement("div");
      sub.className = "bp-mgmt-faint";
      sub.textContent = t("gpui.mgmt.plan.emptysub", "Layers, corridors, and planes appear here once a plan is proposed.");
      emptyBox.append(lead, sub);
      wrap.appendChild(emptyBox);
      return wrap;
    }

    const awaiting = d.awaiting ?? 0;
    const blocked = d.blocked ?? 0;
    const doneN = d.done ?? 0;
    const totalN = d.total ?? 0;
    const rev = d.revision ?? 0;
    const badge = planStatusBadge(d);
    const segs = planProgressSegments(d);
    const segTotal = segs.reduce((acc, s) => acc + s.count, 0);

    const statusStrip = document.createElement("div");
    statusStrip.className = "bp-plan-status";
    const statusTop = document.createElement("div");
    statusTop.className = "bp-plan-status-top";
    const statusPill = document.createElement("span");
    statusPill.className = `bp-plan-status-badge tone-${badge.tone}`;
    statusPill.textContent = t(badge.key, badge.label);
    statusTop.appendChild(statusPill);
    if (rev > 0) {
      const revEl = document.createElement("span");
      revEl.className = "bp-dim";
      revEl.textContent = `v${rev}`;
      statusTop.appendChild(revEl);
    }
    const counts = document.createElement("span");
    counts.className = "bp-mgmt-flex1 " + (awaiting > 0 ? "bp-tone-warn" : blocked > 0 ? "bp-tone-bad" : "bp-dim");
    counts.textContent = totalN > 0
      ? `${doneN}/${totalN} ${t("gpui.mgmt.plan.done", "done")}`
      : awaiting > 0
        ? t("web.plan.awaiting", "{n} awaiting approval", { n: awaiting })
        : t("gpui.mgmt.plan.nothing", "Nothing awaits your approval");
    statusTop.appendChild(counts);
    if (!d.live) {
      const saved = document.createElement("span");
      saved.className = "bp-mgmt-pill";
      saved.textContent = t("gpui.mgmt.plan.saved", "saved plan");
      statusTop.appendChild(saved);
    }
    statusStrip.appendChild(statusTop);
    if (segTotal > 0) {
      const segBar = document.createElement("div");
      segBar.className = "bp-plan-segbar";
      for (const s of segs) {
        if (s.count <= 0) continue;
        const fill = document.createElement("div");
        fill.className = `bp-plan-seg bp-tone-bg-${s.tone}`;
        fill.style.flexGrow = String(s.count);
        segBar.appendChild(fill);
      }
      statusStrip.appendChild(segBar);
    }
    wrap.appendChild(statusStrip);

    const head = document.createElement("div");
    head.className = "bp-mgmt-head";
    if (awaiting > 0) {
      const okAll = document.createElement("button");
      okAll.type = "button";
      okAll.className = "primary bp-mgmt-btn";
      okAll.setAttribute("data-testid", "mgmt-plan-approve-all");
      okAll.textContent = t("gpui.mgmt.plan.all", "Approve all");
      okAll.addEventListener("click", () => sendPlan("plan.approve", { layer: "", ids: [], approve: true }));
      head.appendChild(okAll);
    }
    if (d.routing) {
      const stop = document.createElement("button");
      stop.type = "button";
      stop.className = "bp-mgmt-btn";
      stop.setAttribute("data-testid", "mgmt-plan-stop");
      stop.textContent = t("gpui.mgmt.plan.stop", "Stop routing");
      stop.addEventListener("click", () => sendPlan("plan.stop", {}));
      head.appendChild(stop);
    }
    if (d.live && doneN > 0) {
      const verify = document.createElement("button");
      verify.type = "button";
      verify.className = "bp-mgmt-btn";
      verify.setAttribute("data-testid", "mgmt-plan-verify");
      verify.textContent = t("gpui.mgmt.plan.verify", "Verify");
      verify.addEventListener("click", () => sendPlan("plan.verify", {}));
      head.appendChild(verify);
    }
    const spacer = document.createElement("span");
    spacer.className = "bp-mgmt-flex1";
    const hideBtn = document.createElement("button");
    hideBtn.type = "button";
    hideBtn.className = "bp-mgmt-btn";
    hideBtn.setAttribute("data-testid", "mgmt-plan-hide");
    hideBtn.textContent = this.planHideDone
      ? t("gpui.mgmt.plan.show", "Show done")
      : t("gpui.mgmt.plan.hide", "Hide done");
    hideBtn.addEventListener("click", () => {
      this.planHideDone = !this.planHideDone;
      this.render();
    });
    head.append(spacer, hideBtn);
    wrap.appendChild(head);

    const openAsks = Array.isArray(d.asks) ? d.asks : [];
    if (openAsks.length > 0) {
      const what = openAsks.map((a) => a.what || "").filter(Boolean).join(", ");
      const noteText = openAsks[0]?.note || "";
      const askBar = document.createElement("div");
      askBar.className = "bp-plan-elsewhere";
      const col = document.createElement("div");
      col.className = "bp-plan-ask-col";
      const lbl = document.createElement("div");
      lbl.className = "bp-tone-warn";
      lbl.textContent = what
        ? t("gpui.mgmt.plan.requested", "Approval requested: {what}", { what })
        : t("gpui.mgmt.plan.reqplain", "Approval requested");
      col.appendChild(lbl);
      if (noteText) {
        const nt = document.createElement("div");
        nt.className = "bp-dim";
        nt.textContent = noteText;
        col.appendChild(nt);
      }
      askBar.appendChild(col);
      wrap.appendChild(askBar);
    }

    if (this.planStatus) {
      const err = document.createElement("div");
      err.className = "bp-mgmt-note bp-plan-err bp-tone-bad";
      const txt = document.createElement("span");
      txt.className = "bp-mgmt-flex1";
      txt.textContent = this.planStatus === "KiCad refused" ? t("gpui.mgmt.plan.refused", "KiCad refused") : trs(this.planStatus);
      const closeErr = document.createElement("button");
      closeErr.type = "button";
      closeErr.className = "icon";
      closeErr.textContent = "×";
      closeErr.title = t("common.close", "Close");
      closeErr.addEventListener("click", () => {
        this.planStatus = "";
        this.render();
      });
      err.append(txt, closeErr);
      wrap.appendChild(err);
    }

    if (d.keys_note) {
      const kn = document.createElement("div");
      kn.className = "bp-mgmt-note bp-dim";
      const stdKeysNote = "On this KiCad version, approve from this panel: in plan view KiCad's A, R and W keys aren't the plan's yet (A opens Add Footprint). Update KiCad.";
      kn.textContent = d.keys_note === stdKeysNote
        ? t("gpui.mgmt.plan.keysnote", stdKeysNote)
        : trs(d.keys_note);
      wrap.appendChild(kn);
    }

    const stackupSum = d.stackup_summary ?? "";
    const impedances = Array.isArray(d.impedances) ? d.impedances : [];
    if (stackupSum || impedances.length > 0) {
      const stackWrap = document.createElement("div");
      stackWrap.className = "bp-plan-stackup";
      const toggle = document.createElement("button");
      toggle.type = "button";
      toggle.className = "bp-plan-stackup-head";
      toggle.setAttribute("data-testid", "mgmt-plan-stackup-toggle");
      toggle.setAttribute("aria-expanded", this.planStackupOpen ? "true" : "false");
      const chev = document.createElement("span");
      chev.className = "bp-plan-chev";
      chev.textContent = this.planStackupOpen ? "▾" : "▸";
      const lbl = document.createElement("span");
      lbl.textContent = t("gpui.mgmt.plan.stackup", "Stackup & impedances");
      toggle.append(chev, lbl);
      toggle.addEventListener("click", () => {
        this.planStackupOpen = !this.planStackupOpen;
        this.render();
      });
      stackWrap.appendChild(toggle);
      if (this.planStackupOpen) {
        const sbody = document.createElement("div");
        sbody.className = "bp-plan-stackup-body";
        sbody.setAttribute("data-testid", "mgmt-plan-stackup-body");
        if (stackupSum) {
          const sl = document.createElement("div");
          sl.className = "bp-dim";
          sl.textContent = stackupSum;
          sbody.appendChild(sl);
        }
        for (const imp of impedances) {
          const il = document.createElement("div");
          il.className = "bp-mgmt-faint";
          il.textContent = `· ${imp}`;
          sbody.appendChild(il);
        }
        stackWrap.appendChild(sbody);
      }
      wrap.appendChild(stackWrap);
    }

    const mkChip = (label: string, active: boolean, testId: string, onClick: () => void): HTMLButtonElement => {
      const b = document.createElement("button");
      b.type = "button";
      b.className = "bp-mgmt-btn bp-plan-chip-btn" + (active ? " active" : "");
      b.setAttribute("data-testid", testId);
      b.setAttribute("aria-pressed", active ? "true" : "false");
      b.textContent = label;
      b.addEventListener("click", onClick);
      return b;
    };

    const viewBar = document.createElement("div");
    viewBar.className = "bp-plan-viewbar";
    viewBar.append(
      mkChip(t("gpui.mgmt.plan.view.canvas", "Canvas"), this.planViewMode === "canvas", "mgmt-plan-view-canvas", () => {
        this.planViewMode = "canvas";
        this.render();
      }),
      mkChip(t("gpui.mgmt.plan.view.split", "Split"), this.planViewMode === "split", "mgmt-plan-view-split", () => {
        this.planViewMode = "split";
        this.render();
      }),
      mkChip(t("gpui.mgmt.plan.view.list", "List"), this.planViewMode === "list", "mgmt-plan-view-list", () => {
        this.planViewMode = "list";
        this.render();
      }),
    );
    if (this.planViewMode !== "list") {
      const sep1 = document.createElement("span");
      sep1.className = "bp-plan-sep";
      const sep2 = document.createElement("span");
      sep2.className = "bp-plan-sep";
      const fitBtn = document.createElement("button");
      fitBtn.type = "button";
      fitBtn.className = "bp-mgmt-btn bp-plan-mini-btn";
      fitBtn.setAttribute("data-testid", "mgmt-plan-fit");
      fitBtn.textContent = t("gpui.mgmt.plan.fit", "Fit");
      fitBtn.addEventListener("click", () => this.planCanvas?.fitAll());

      viewBar.append(
        sep1,
        mkChip(t("gpui.mgmt.plan.grid.pair", "Side by side"), this.planGridMode === "pair", "mgmt-plan-grid-pair", () => {
          this.planGridMode = "pair";
          this.render();
        }),
        mkChip(t("gpui.mgmt.plan.grid.all", "All layers"), this.planGridMode === "all", "mgmt-plan-grid-all", () => {
          this.planGridMode = "all";
          this.render();
        }),
        mkChip(t("gpui.mgmt.plan.grid.single", "Single"), this.planGridMode === "single", "mgmt-plan-grid-single", () => {
          this.planGridMode = "single";
          this.render();
        }),
        sep2,
        mkChip(t("gpui.mgmt.plan.sync", "Link zoom"), this.planSyncZoom, "mgmt-plan-sync", () => {
          this.planSyncZoom = !this.planSyncZoom;
          this.render();
        }),
        fitBtn,
      );
    }
    const viewSpacer = document.createElement("span");
    viewSpacer.className = "bp-mgmt-flex1";
    const expBtn = document.createElement("button");
    expBtn.type = "button";
    expBtn.className = "bp-mgmt-btn bp-plan-mini-btn";
    expBtn.setAttribute("data-testid", "mgmt-plan-expand");
    expBtn.textContent = this.planExpanded
      ? t("gpui.mgmt.plan.compact", "Compact")
      : t("gpui.mgmt.plan.expand", "Expand");
    expBtn.addEventListener("click", () => {
      this.planExpanded = !this.planExpanded;
      this.render();
    });
    viewBar.append(viewSpacer, expBtn);
    wrap.appendChild(viewBar);

    const isHydrated =
      Boolean(this.planHydratedBoard) &&
      this.planHydratedKey.startsWith(`${this.project}:`) &&
      (this.planHydratedKey === hydKey || (this.planHydratedRev !== undefined && this.planHydratedRev >= (d.revision ?? 0)));
    const rawBoard: PlanBoardGeom = isHydrated && this.planHydratedBoard
      ? this.planHydratedBoard
      : d.board ?? { bbox: [0, 0, 100, 70], outline: [], parts: [] };
    const rawDraw = isHydrated && this.planHydratedDraw
      ? this.planHydratedDraw
      : d.draw ?? { bands: [], regions: [] };

    const mergedBands: PlanBand[] = (rawDraw.bands ?? []).map((b) => {
      const patch = this.planOptimistic.get(b.id);
      if (!patch) return { ...b };
      return {
        ...b,
        ...patch,
        edited: true,
        state: b.state === "proposed" ? "edited" : b.state,
      };
    });
    const mergedRegions: PlanRegion[] = (rawDraw.regions ?? []).map((r) => {
      const patch = this.planOptimistic.get(r.id);
      if (!patch) return { ...r };
      return {
        ...r,
        ...patch,
        edited: true,
        state: r.state === "proposed" ? "edited" : r.state,
      };
    });

    const movedItems: Array<{ target: string; item: PlanItem }> = [];
    const mergedLayers: PlanLayer[] = (d.layers ?? []).map((l) => {
      const kept: PlanItem[] = [];
      for (const it of l.items ?? []) {
        const patch = this.planOptimistic.get(it.id);
        if (!patch) {
          kept.push(it);
          continue;
        }
        const next: PlanItem = {
          ...it,
          width: typeof patch.width === "number" ? patch.width : it.width,
          locked: typeof patch.locked === "boolean" ? patch.locked : it.locked,
          edited: true,
          state: it.state === "proposed" ? "edited" : it.state,
        };
        if (typeof patch.layer === "string" && patch.layer !== l.layer) {
          movedItems.push({ target: patch.layer, item: next });
        } else {
          kept.push(next);
        }
      }
      return { ...l, items: kept };
    });
    for (const mv of movedItems) {
      const targetLayer = mergedLayers.find((l) => l.layer === mv.target);
      if (targetLayer) {
        targetLayer.items = [...(targetLayer.items ?? []), mv.item];
      }
    }

    const canvasLayers = planAllCanvasLayers(mergedLayers, d.copper_layers);
    const activeLayers =
      this.planGridMode === "all"
        ? canvasLayers.map((l) => l.layer)
        : this.planGridMode === "single"
          ? [
              this.planShownLayers.find((n) => canvasLayers.some((l) => l.layer === n)) ??
                planDefaultLayerPair(canvasLayers)[0] ??
                canvasLayers[0]?.layer ??
                "",
            ].filter(Boolean)
          : (this.planShownLayers.length > 0 && this.planShownLayers.every((n) => canvasLayers.some((l) => l.layer === n))
              ? this.planShownLayers.slice(0, 2)
              : planDefaultLayerPair(canvasLayers));

    if (this.planViewMode !== "list" && canvasLayers.length > 1) {
      const layerBar = document.createElement("div");
      layerBar.className = "bp-plan-layerbar";
      for (const l of canvasLayers) {
        const isShown = activeLayers.includes(l.layer);
        layerBar.appendChild(
          mkChip(l.title || l.layer, isShown, `mgmt-plan-layerpill-${l.layer}`, () => {
            if (this.planGridMode === "single") {
              this.planShownLayers = [l.layer];
            } else if (this.planGridMode === "pair") {
              if (!activeLayers.includes(l.layer)) {
                this.planShownLayers = activeLayers.length >= 2 ? [activeLayers[0], l.layer] : [...activeLayers, l.layer];
              }
            } else {
              this.planGridMode = "single";
              this.planShownLayers = [l.layer];
            }
            this.render();
          }),
        );
      }
      wrap.appendChild(layerBar);
    }

    const commitItemFields = (id: string, fields: Record<string, unknown>): void => {
      const prev = this.planOptimistic.get(id) ?? {};
      this.planOptimistic.set(id, { ...prev, ...fields });
      if (typeof fields.layer === "string") {
        if (this.planGridMode === "single") {
          this.planShownLayers = [fields.layer];
        } else if (this.planGridMode === "pair" && !activeLayers.includes(fields.layer)) {
          this.planShownLayers = activeLayers.length >= 2 ? [activeLayers[0], fields.layer] : [...activeLayers, fields.layer];
        }
      }
      const baseRev = typeof d.revision === "number" ? d.revision : undefined;
      this.render();
      sendPlan("plan.update", {
        ops: [{ op: "set", id, fields }],
        ...(baseRev !== undefined ? { base: baseRev } : {}),
      });
    };
    this.planCommitFields = commitItemFields;

    if (this.planSelectedId) {
      let selLayer = "";
      let selItem: PlanItem | null = null;
      for (const l of mergedLayers) {
        const found = (l.items ?? []).find((it) => it.id === this.planSelectedId);
        if (found) {
          selLayer = l.layer;
          selItem = found;
          break;
        }
      }
      if (selItem) {
        const ins = document.createElement("div");
        ins.className = "bp-plan-inspector";
        ins.setAttribute("data-testid", "mgmt-plan-inspector");
        const top = document.createElement("div");
        top.className = "bp-plan-ins-row";
        const sw = document.createElement("span");
        sw.className = "bp-plan-ins-sw";
        sw.style.background = selItem.kind === "corridor"
          ? planBusColor(selItem.name || selItem.id, selItem.color || "")
          : planRailColor(selItem.name || selItem.id, selItem.color || "");
        const title = document.createElement("span");
        title.className = "bp-mgmt-strong";
        title.textContent = selItem.name || selItem.id;
        const ltag = document.createElement("span");
        ltag.className = "bp-dim";
        ltag.textContent = selLayer;
        top.append(sw, title, ltag);
        if (selItem.edited || selItem.state === "edited") {
          const ep = document.createElement("span");
          ep.className = "bp-plan-ins-pill warn";
          ep.textContent = t("gpui.mgmt.plan.edited", "Edited");
          top.appendChild(ep);
        }
        if (selItem.locked) {
          const lp = document.createElement("span");
          lp.className = "bp-plan-ins-pill";
          lp.textContent = t("gpui.mgmt.plan.locked", "Locked");
          top.appendChild(lp);
        }
        const sp = document.createElement("span");
        sp.className = "bp-mgmt-flex1";
        top.appendChild(sp);

        const lockBtn = document.createElement("button");
        lockBtn.type = "button";
        lockBtn.className = "bp-mgmt-btn bp-plan-mini-btn";
        lockBtn.setAttribute("data-testid", "mgmt-plan-lock");
        lockBtn.textContent = selItem.locked
          ? t("gpui.mgmt.plan.unlock", "Unlock")
          : t("gpui.mgmt.plan.lock", "Lock");
        lockBtn.addEventListener("click", () => commitItemFields(selItem!.id, { locked: !selItem!.locked }));
        top.appendChild(lockBtn);

        if (selItem.state === "proposed" || selItem.state === "edited") {
          const okB = document.createElement("button");
          okB.type = "button";
          okB.className = "primary bp-mgmt-btn bp-plan-mini-btn";
          okB.textContent = t("android.hub.machines.approve", "Approve");
          okB.addEventListener("click", () => sendPlan("plan.approve", { layer: "", ids: [selItem!.id], approve: true }));
          const noB = document.createElement("button");
          noB.type = "button";
          noB.className = "bp-mgmt-btn bp-plan-mini-btn";
          noB.textContent = t("gpui.mgmt.plan.reject", "Reject");
          noB.addEventListener("click", () => sendPlan("plan.approve", { layer: "", ids: [selItem!.id], approve: false }));
          top.append(okB, noB);
        }

        const noteBtn = document.createElement("button");
        noteBtn.type = "button";
        noteBtn.className = "bp-mgmt-btn bp-plan-mini-btn";
        noteBtn.setAttribute("data-testid", "mgmt-plan-note-toggle");
        noteBtn.textContent = t("gpui.mgmt.plan.addnote", "Add note");
        noteBtn.addEventListener("click", () => {
          this.planNoteOpen = !this.planNoteOpen;
          this.render();
        });
        const closeSel = document.createElement("button");
        closeSel.type = "button";
        closeSel.className = "icon";
        closeSel.textContent = "×";
        closeSel.addEventListener("click", () => {
          this.planSelectedId = "";
          this.planNoteOpen = false;
          this.render();
        });
        top.append(noteBtn, closeSel);
        ins.appendChild(top);

        if (!selItem.locked) {
          const ctrlRow = document.createElement("div");
          ctrlRow.className = "bp-plan-ins-row";
          if (selItem.kind === "corridor") {
            const curW = typeof selItem.width === "number" ? selItem.width : 1.5;
            const wLbl = document.createElement("span");
            wLbl.className = "bp-dim";
            wLbl.textContent = t("gpui.mgmt.plan.width", "Width");
            const wMinus = document.createElement("button");
            wMinus.type = "button";
            wMinus.className = "bp-mgmt-btn bp-plan-mini-btn";
            wMinus.setAttribute("data-testid", "mgmt-plan-w-down");
            wMinus.textContent = "-";
            wMinus.addEventListener("click", () => {
              const nw = Math.round(Math.max(0.15, curW - 0.25) * 100) / 100;
              commitItemFields(selItem!.id, { width: nw });
            });
            const wVal = document.createElement("span");
            wVal.className = "bp-plan-ins-val";
            wVal.textContent = `${curW.toFixed(2)}mm`;
            const wPlus = document.createElement("button");
            wPlus.type = "button";
            wPlus.className = "bp-mgmt-btn bp-plan-mini-btn";
            wPlus.setAttribute("data-testid", "mgmt-plan-w-up");
            wPlus.textContent = "+";
            wPlus.addEventListener("click", () => {
              const nw = Math.round(Math.min(25, curW + 0.25) * 100) / 100;
              commitItemFields(selItem!.id, { width: nw });
            });
            const sepW = document.createElement("span");
            sepW.className = "bp-plan-sep";
            ctrlRow.append(wLbl, wMinus, wVal, wPlus, sepW);
          }

          const moveOpts = (d.copper_layers ?? []).length > 0
            ? (d.copper_layers ?? []).map((c) => c.layer)
            : mergedLayers.map((l) => l.layer);
          if (moveOpts.length > 0) {
            const mLbl = document.createElement("span");
            mLbl.className = "bp-dim";
            mLbl.textContent = t("gpui.mgmt.plan.layer.move", "Move to layer");
            ctrlRow.appendChild(mLbl);
            for (const ml of moveOpts) {
              ctrlRow.appendChild(
                mkChip(ml, ml === selLayer, `mgmt-plan-move-${ml}`, () => {
                  if (ml !== selLayer) commitItemFields(selItem!.id, { layer: ml });
                }),
              );
            }
          }
          ins.appendChild(ctrlRow);
        }

        if (this.planNoteOpen) {
          const nf = document.createElement("form");
          nf.className = "bp-plan-note-form";
          const ni = document.createElement("input");
          ni.type = "text";
          ni.className = "bp-plan-note-input";
          ni.setAttribute("data-testid", "mgmt-plan-note-input");
          ni.placeholder = t("gpui.mgmt.plan.note.placeholder", "Steering note for router…");
          const nb = document.createElement("button");
          nb.type = "submit";
          nb.className = "primary bp-mgmt-btn bp-plan-mini-btn";
          nb.setAttribute("data-testid", "mgmt-plan-note-submit");
          nb.textContent = t("gpui.mgmt.plan.addnote", "Add note");
          nf.append(ni, nb);
          nf.addEventListener("submit", (e) => {
            e.preventDefault();
            const txt = ni.value.trim();
            if (!txt) return;
            ni.value = "";
            this.planNoteOpen = false;
            const baseRev = typeof d.revision === "number" ? d.revision : undefined;
            this.render();
            sendPlan("plan.update", {
              ops: [{ op: "note", target: selItem!.id, text: txt }],
              ...(baseRev !== undefined ? { base: baseRev } : {}),
            });
          });
          ins.appendChild(nf);
        }

        if (this.planViewMode !== "list" && !selItem.locked) {
          const hint = document.createElement("div");
          hint.className = "bp-plan-hint";
          hint.textContent = t(
            "gpui.mgmt.plan.hint",
            "Drag waypoints to steer · Click + on edge to insert · Right-click vertex to remove",
          );
          ins.appendChild(hint);
        }
        wrap.appendChild(ins);
      }
    }

    const listEl = document.createElement("div");
    listEl.className = "bp-mgmt-rows" + (this.planViewMode === "split" ? " bp-plan-split-list" : "");
    listEl.setAttribute("data-testid", "mgmt-plan-rows");
    const doneWord = t("gpui.mgmt.plan.done", "done");
    for (const l of mergedLayers) {
      const items = l.items ?? [];
      const gh = document.createElement("div");
      gh.className = "bp-mgmt-group-head bp-plan-layer-head";
      const gtop = document.createElement("div");
      gtop.className = "bp-mgmt-row-line";
      const lbl = document.createElement("span");
      lbl.className = "bp-mgmt-strong";
      lbl.textContent = l.layer;
      const meta = document.createElement("span");
      meta.className = "bp-dim bp-mgmt-flex1";
      meta.textContent = `${l.role ? `${l.role} · ` : ""}${l.done ?? 0}/${items.length} ${doneWord}`;
      gtop.append(lbl, meta);
      if (l.status !== "approved" && items.length > 0) {
        const okLayer = document.createElement("button");
        okLayer.type = "button";
        okLayer.className = "bp-mgmt-btn";
        okLayer.textContent = t("gpui.mgmt.plan.layer", "Approve layer");
        okLayer.addEventListener("click", () => sendPlan("plan.approve", { layer: l.layer, ids: [], approve: true }));
        const rejLayer = document.createElement("button");
        rejLayer.type = "button";
        rejLayer.className = "bp-mgmt-btn";
        rejLayer.textContent = t("gpui.mgmt.plan.reject", "Reject");
        rejLayer.addEventListener("click", () => sendPlan("plan.approve", { layer: l.layer, ids: [], approve: false }));
        gtop.append(okLayer, rejLayer);
      }
      gh.appendChild(gtop);
      if (l.stackup) {
        const lstack = document.createElement("div");
        lstack.className = "bp-plan-layer-stackup";
        lstack.textContent = l.stackup;
        gh.appendChild(lstack);
      }
      listEl.appendChild(gh);

      for (const it of items) {
        if (this.planHideDone && planDone(it)) continue;
        const selected = this.planSelectedId === it.id;
        const row = document.createElement("div");
        row.className = "bp-mgmt-row bp-plan-row" + (selected ? " active" : "");
        row.setAttribute("data-testid", `mgmt-plan-row-${it.id}`);

        const line = document.createElement("div");
        line.className = "bp-mgmt-row-line";

        const swatch = document.createElement("span");
        swatch.className = `bp-plan-swatch bp-plan-st-${it.state || "proposed"}`;
        swatch.style.setProperty("--swatch-color", it.color || "#8a8a8a");

        const name = document.createElement("span");
        name.className = "bp-mgmt-flex1" + (
          it.state === "blocked" || it.state === "failed"
            ? " bp-tone-bad"
            : it.state === "edited"
              ? " bp-tone-warn"
              : (it.state === "rejected" || it.state === "routed" || it.state === "verified")
                ? " bp-dim"
                : ""
        );
        name.textContent = `${it.name || it.id}${it.locked ? " 🔒" : ""}`;

        const parts: string[] = [];
        if (it.kind === "plane") {
          if (it.reference) parts.push(t("gpui.mgmt.plan.reference", "over {layer}", { layer: it.reference }));
        } else {
          const nc = it.nets ?? 0;
          parts.push(nc === 1 ? t("web.plan.net.one", "1 net") : t("web.plan.nets", "{n} nets", { n: nc }));
        }
        if (typeof it.width === "number" && Number.isFinite(it.width)) {
          parts.push(`${it.width.toFixed(2)}mm`);
        }
        if (it.impedance) parts.push(it.impedance);
        const detailText = parts.join(" · ");

        const mk = document.createElement("span");
        mk.className = "bp-plan-mark " + (
          it.state === "blocked" || it.state === "failed"
            ? "bp-tone-bad"
            : it.state === "verified" || it.state === "routed"
              ? "bp-tone-ok"
              : it.state === "approved"
                ? "bp-tone-accent"
                : "bp-dim"
        );
        mk.textContent = planMark(it);

        line.append(swatch, name, mk);
        if (it.state === "proposed" || it.state === "edited") {
          const ok = document.createElement("button");
          ok.type = "button";
          ok.className = "bp-mgmt-btn";
          ok.textContent = "✓";
          ok.title = t("gpui.mgmt.plan.layer", "Approve layer");
          ok.addEventListener("click", (e) => { e.stopPropagation(); sendPlan("plan.approve", { layer: "", ids: [it.id], approve: true }); });
          const no = document.createElement("button");
          no.type = "button";
          no.className = "bp-mgmt-btn";
          no.textContent = "✗";
          no.title = t("gpui.mgmt.plan.reject", "Reject");
          no.addEventListener("click", (e) => { e.stopPropagation(); sendPlan("plan.approve", { layer: "", ids: [it.id], approve: false }); });
          line.append(ok, no);
        }
        row.appendChild(line);
        if (detailText) {
          const detail = document.createElement("div");
          detail.className = "bp-mgmt-row-sub bp-plan-detail bp-dim";
          detail.textContent = detailText;
          row.appendChild(detail);
        }

        if (it.state === "routing" && typeof it.total === "number" && it.total > 0) {
          const micro = document.createElement("div");
          micro.className = "bp-plan-microbar";
          const fill = document.createElement("div");
          fill.className = "bp-plan-microbar-fill";
          const pct = Math.max(0, Math.min(100, Math.round(((it.routed ?? 0) / it.total) * 100)));
          fill.style.width = `${pct}%`;
          micro.appendChild(fill);
          row.appendChild(micro);
        }
        if (it.problem_text) {
          const prob = document.createElement("div");
          prob.className = "bp-plan-problem";
          prob.textContent = it.problem_text;
          row.appendChild(prob);
        }

        row.addEventListener("click", () => {
          this.planSelectedId = it.id;
          sendPlan("plan.focus", { id: it.id });
          this.render();
        });
        listEl.appendChild(row);
      }
    }
    if (this.planViewMode !== "list") {
      if (!this.planCanvas) {
        this.planCanvas = new PlanFloorplannerCanvas({
          onSelect: (id) => {
            this.planSelectedId = id;
            if (!id) this.planNoteOpen = false;
            this.render();
          },
          onFocusItem: (id) => {
            sendPlan("plan.focus", { id });
          },
          onDragEnd: () => {
            if (this.planRenderDeferred) {
              this.planRenderDeferred = false;
              this.render();
            }
          },
          onCommitFields: (id, fields) => {
            (this.planCommitFields ?? commitItemFields)(id, fields);
          },
          onApproveLayer: (layer, approve) => {
            sendPlan("plan.approve", { layer, ids: [], approve });
          },
          onQuickApprove: (ids, approve) => {
            sendPlan("plan.approve", { layer: "", ids, approve });
          },
        });
      }
      const noViewer = this.panelEl.parentElement?.classList.contains("bp-has-viewer") === false;
      const twoCols = this.planExpanded || this.panelEl.clientWidth >= 540 || noViewer;
      this.planCanvas.update(
        {
          gridMode: this.planGridMode,
          syncZoom: this.planSyncZoom,
          shownLayers: activeLayers,
          selectedId: this.planSelectedId,
          board: rawBoard,
          bands: mergedBands,
          regions: mergedRegions,
          layers: canvasLayers,
          copperLayers: d.copper_layers,
        },
        twoCols,
      );
      const canvasScroll = document.createElement("div");
      canvasScroll.className = "bp-plan-canvas-scroll";
      canvasScroll.appendChild(this.planCanvas.root);
      wrap.appendChild(canvasScroll);
    }
    if (this.planViewMode !== "canvas") {
      wrap.appendChild(listEl);
    }

    if ((d.draw?.bands?.length ?? 0) > 0 || (d.draw?.regions?.length ?? 0) > 0) {
      const foot = document.createElement("div");
      foot.className = "bp-plan-footer";
      foot.textContent = t(
        "gpui.mgmt.plan.drawn",
        "The plan's corridors and planes are drawn on the board while this panel is open. Close it to hide them.",
      );
      wrap.appendChild(foot);
    }

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
      const gen = this.watchGen;
      const proj = this.project;
      this.fabStatus = "";
      void this.cb.hub.rpc("fab.profile", { pro: proj, action, ...extra })
        .catch((e) => {
          if (gen !== this.watchGen || proj !== this.project) return;
          this.fabStatus = e instanceof Error ? e.message : "Fab action failed";
          this.render();
        });
    };

    const head = document.createElement("div");
    head.className = "bp-mgmt-head";
    const title = document.createElement("span");
    title.className = "bp-mgmt-strong";
    title.textContent = t("gpui.mgmt.fab.thefab", "The fab");
    const meta = document.createElement("span");
    meta.className = "bp-dim bp-mgmt-flex1";
    meta.textContent = `rev ${d.rev ?? 0}${d.updated ? ` · ${d.updated}` : ""}${d.board ? ` · ${d.board}` : ""} · ${tierWords(d.tier ?? "")}`;
    const refreshBtn = document.createElement("button");
    refreshBtn.type = "button";
    refreshBtn.className = "bp-mgmt-btn";
    refreshBtn.textContent = t("gpui.mgmt.fab.refresh", "Refresh");
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
      fBtn.textContent = t("gpui.mgmt.fab.findings", "Findings");
      fBtn.addEventListener("click", () => this.togglePanel("checks"));
      rvLine.appendChild(fBtn);
    }
    if (!rv?.running) {
      const rBtn = document.createElement("button");
      rBtn.type = "button";
      rBtn.className = "bp-mgmt-btn";
      rBtn.textContent = t("gpui.mgmt.fab.reviewbtn", "Review");
      rBtn.addEventListener("click", () => sendFab("review"));
      rvLine.appendChild(rBtn);
    }
    if (rv?.judge) {
      const jBtn = document.createElement("button");
      jBtn.type = "button";
      jBtn.className = "bp-mgmt-btn";
      jBtn.textContent = t("gpui.mgmt.fab.judge", "Datasheets");
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
      btn.textContent = t("gpui.mgmt.fab.apply", "Apply");
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
        btn.textContent = t("gpui.mgmt.fab.use", "Use");
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
      chLbl.textContent = t("gpui.mgmt.fab.changed", "Changed since you last looked");
      const seenBtn = document.createElement("button");
      seenBtn.type = "button";
      seenBtn.className = "bp-mgmt-btn";
      seenBtn.textContent = t("gpui.mgmt.fab.seen", "Seen");
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

    // Limits table (matches native/gpui/src/screens/mgmt_fab.rs: Limit | Base price | Costs more | Read)
    const limits = Array.isArray(d.limits) ? d.limits : [];
    if (limits.length > 0) {
      const lh = document.createElement("div");
      lh.className = "bp-mgmt-group-head";
      const c0 = document.createElement("span");
      c0.className = "bp-mgmt-flex1";
      c0.textContent = t("gpui.mgmt.fab.limit", "Limit");
      const c1 = document.createElement("span");
      c1.className = "bp-mgmt-col";
      c1.textContent = t("gpui.mgmt.fab.free", "Base price");
      const c2 = document.createElement("span");
      c2.className = "bp-mgmt-col";
      c2.textContent = t("gpui.mgmt.fab.extra", "Costs more");
      const c3 = document.createElement("span");
      c3.className = "bp-mgmt-col bp-mgmt-col-read";
      c3.textContent = t("gpui.mgmt.fab.readcol", "Read");
      lh.append(c0, c1, c2, c3);
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

        const side = l.free && typeof l.free === "object" ? l.free : l.extra;
        const day = side?.fetched ?? "";
        const derived = Boolean(day) && side?.confidence !== "published";
        const readText = day ? (derived ? `${day}*` : day) : "—";
        const srcUrl = side?.source ?? "";
        let rd: HTMLElement;
        if (srcUrl.startsWith("https://") && day) {
          const a = document.createElement("a");
          a.className = "bp-mgmt-col bp-mgmt-col-read bp-mgmt-read-link";
          a.href = srcUrl;
          a.target = "_blank";
          a.rel = "noopener noreferrer";
          a.textContent = readText;
          rd = a;
        } else {
          const sp = document.createElement("span");
          sp.className = "bp-mgmt-col bp-mgmt-col-read bp-mgmt-faint";
          sp.textContent = readText;
          rd = sp;
        }
        ll.append(nm, f, ex, rd);
        lr.appendChild(ll);
        listEl.appendChild(lr);
      }

      const foot = document.createElement("div");
      foot.className = "bp-mgmt-note bp-mgmt-faint";
      foot.textContent = t(
        "gpui.mgmt.fab.derived",
        "* worked out from the fab's page. Dimmed: the rigid board's value. A date opens the source page.",
      );
      listEl.appendChild(foot);
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
