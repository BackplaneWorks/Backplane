import SwiftUI
import UIKit

private func rgbColor(_ hex: UInt32, alpha: Double = 1.0) -> Color {
    Color(
        red: Double((hex >> 16) & 0xFF) / 255.0,
        green: Double((hex >> 8) & 0xFF) / 255.0,
        blue: Double(hex & 0xFF) / 255.0,
        opacity: alpha
    )
}

private let pcbSubstrateRGB: UInt32 = 0x111917
private let pcbBoardFillRGB: UInt32 = 0x16201D
private let pcbEdgeStrokeRGB: UInt32 = 0xD4E0DB
private let pcbPartStrokeRGB: UInt32 = 0x7A9188
private let pcbOverhangRGB: UInt32 = 0xF59E0B
private let pcbRefdesRGB: UInt32 = 0x8FA39B
private let pcbBadgeBgRGB: UInt32 = 0x111917
private let pcbBadgeFgRGB: UInt32 = 0xEEF5F2
private let pcbBadRedRGB: UInt32 = 0xEF4444

/// Management strip + full-pane interactive Plan floorplanner mounted in Board/Schematic tabs.
struct DataMgmtHost: View {
    @Bindable var client: DataClient
    let project: String
    var home: String = ""
    @Binding var open: String

    private struct MgmtPanelMeta: Identifiable, Equatable {
        var id: String
        var label: String
    }
    private struct MgmtStateRaw: Decodable {
        var panels: [MgmtPanelItem]?
        struct MgmtPanelItem: Decodable { var id: String?; var label: String? }
    }
    private struct MgmtBadgeRaw: Decodable {
        var n: Int?; var tone: String?
    }

    private func panelsList() -> [MgmtPanelMeta] {
        let d = client.mgmtDigest
        guard !d.isEmpty, let raw = client.mgmt["mgmt.state.\(d)"],
              let parsed = try? JSONDecoder().decode(MgmtStateRaw.self, from: raw) else {
            return []
        }
        return (parsed.panels ?? []).compactMap { p in
            guard let id = p.id, id == "plan" else { return nil }
            return MgmtPanelMeta(id: id, label: p.label ?? "")
        }
    }

    private func badgeFor(_ id: String) -> (n: Int, tone: String) {
        guard let raw = client.mgmtRaw(id),
              let b = try? JSONDecoder().decode(MgmtBadgeRaw.self, from: raw) else {
            return (0, "warn")
        }
        return (b.n ?? 0, b.tone ?? "warn")
    }

    var body: some View {
        let _ = client.mgmtRev
        let panels = panelsList()
        VStack(spacing: 0) {
            if !panels.isEmpty {
                if open == "plan" {
                    DLine()
                    DataPlanFloorplannerView(client: client, fallbackProject: project)
                        .id(project)
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                        .background(BackplaneDesign.background)
                }
                DLine()
                HStack(spacing: 0) {
                    ForEach(panels) { p in
                        let on = open == p.id
                        let badge = badgeFor(p.id)
                        Button {
                            open = on ? "" : p.id
                        } label: {
                            HStack(spacing: 6) {
                                Text(p.label.isEmpty ? Tr.k("gpui.mgmt.plan.tab") : Tr.t(p.label))
                                    .dmono(12, bold: on)
                                    .foregroundStyle(on ? BackplaneDesign.text : BackplaneDesign.dim)
                                if badge.n > 0 {
                                    let tcol = badge.tone == "bad" ? BackplaneDesign.bad : BackplaneDesign.warn
                                    let bstr = badge.n > 99 ? "99+" : "\(badge.n)"
                                    Text(bstr)
                                        .dmono(10, bold: true)
                                        .foregroundStyle(tcol)
                                        .padding(.horizontal, 4)
                                        .padding(.vertical, 1)
                                        .overlay(Rectangle().strokeBorder(tcol, lineWidth: 1))
                                }
                            }
                            .padding(.horizontal, 14)
                            .frame(minHeight: 40)
                            .background(on ? BackplaneDesign.background : BackplaneDesign.panel)
                        }
                        .buttonStyle(.plain)
                        .accessibilityIdentifier("mgmt.tab.\(p.id)")
                    }
                    Spacer()
                    if !open.isEmpty {
                        Button {
                            open = ""
                        } label: {
                            Image(systemName: "xmark")
                                .font(.system(size: 13, weight: .bold))
                                .foregroundStyle(BackplaneDesign.dim)
                                .frame(width: 40, height: 40)
                        }
                        .buttonStyle(.plain)
                        .accessibilityLabel(Tr.k("common.close"))
                        .accessibilityIdentifier("mgmt.close")
                    }
                }
                .frame(maxWidth: .infinity)
                .background(BackplaneDesign.panel)
            }
        }
        .task(id: "\(project)|\(client.conn == .online)") {
            if !project.isEmpty && client.conn == .online && client.has("mgmt") {
                await client.mgmtWatch(project)
            }
        }
        .onChange(of: home) { _, _ in
            open = ""
        }
    }
}

struct DataPlanFloorplannerView: View {
    @Bindable var client: DataClient
    let fallbackProject: String

    @State private var activeLayer = ""
    @State private var selectedId = ""
    @State private var selectedVertex: (id: String, idx: Int)? = nil
    @State private var inspectorOpen = false
    @State private var stackupOpen = false
    @State private var cam: PlanCamera? = nil
    @State private var canvasSize: CGSize = .zero

    // Board geometry hydration (`plan.get {pro, board: true}`)
    @State private var hydratedKey = ""
    @State private var hydratedRev: Int = -1
    @State private var hydratedBoard: PlanBoardGeom? = nil
    @State private var hydratedDraw: PlanDraw? = nil

    // Optimistic steering edits & undo stack
    @State private var optimistic: [String: PlanOptimisticPatch] = [:]
    @State private var optimisticRev: Int? = nil
    @State private var undoStack: [PlanOptimisticPatch] = []
    @State private var errMsg = ""

    // Active drag / finger-lift HUD state
    enum DragKind {
        case none
        case pan(startCam: PlanCamera)
        case vertex(id: String, isPlane: Bool, idx: Int, startPts: [[Double]], grabOffsetPx: CGSize)
    }
    @State private var dragKind: DragKind = .none
    @State private var dragWorldCoord: [Double]? = nil
    @State private var dragScreenPt: CGPoint? = nil

    private var rawPanel: PlanPanel? {
        let _ = client.mgmtRev
        guard let d = client.mgmtRaw("plan") else { return nil }
        return PlanFloorplannerLogic.unwrapPanel(from: d)
    }

    private func effectivePanel(from base: PlanPanel) -> PlanPanel {
        var panel = base
        let pro = base.pro.isEmpty ? fallbackProject : base.pro
        let matchPrefix = "\(pro):"
        if let hb = hydratedBoard, hydratedKey.hasPrefix(matchPrefix), hydratedRev >= base.revision {
            panel.board = hb
        }
        if let hd = hydratedDraw, hydratedKey.hasPrefix(matchPrefix), hydratedRev >= base.revision {
            panel.draw = hd
        }
        return PlanFloorplannerLogic.mergeOptimistic(panel: panel, patches: optimistic)
    }

    var body: some View {
        let _ = client.mgmtRev
        if let base = rawPanel {
            let panel = effectivePanel(from: base)
            let pro = panel.pro.isEmpty ? fallbackProject : panel.pro
            let layers = PlanFloorplannerLogic.allCanvasLayers(planned: panel.layers, copper: panel.copperLayers)
            let curLayer = layers.contains(where: { $0.layer == activeLayer })
                ? activeLayer
                : PlanFloorplannerLogic.defaultActiveLayer(layers: layers)
            let layerObj = layers.first(where: { $0.layer == curLayer })
            let board = panel.board ?? PlanBoardGeom()
            let draw = panel.draw ?? PlanDraw()

            VStack(spacing: 0) {
                headerView(panel: panel, pro: pro)
                if !errMsg.isEmpty {
                    HStack(spacing: 8) {
                        Text(Tr.t(errMsg)).dmono(11).foregroundStyle(BackplaneDesign.bad).lineLimit(2)
                        Spacer()
                        Button { errMsg = "" } label: {
                            Image(systemName: "xmark").font(.system(size: 11, weight: .bold)).foregroundStyle(BackplaneDesign.dim)
                        }.buttonStyle(.plain).accessibilityLabel(Tr.k("common.close"))
                    }
                    .padding(.horizontal, 10).padding(.vertical, 4).background(BackplaneDesign.panel)
                    DLine()
                }
                if panel.empty {
                    VStack(alignment: .leading, spacing: 6) {
                        Text(Tr.k("gpui.mgmt.plan.noboard")).dmono(12, bold: true).foregroundStyle(BackplaneDesign.dim)
                        Text(Tr.k("gpui.mgmt.plan.emptysub")).dmono(11).foregroundStyle(BackplaneDesign.faint)
                        Spacer()
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(14)
                } else {
                    layerStripView(layers: layers, copper: panel.copperLayers, curLayer: curLayer, layerObj: layerObj, board: board, pro: pro)
                    if stackupOpen && (!panel.stackupSummary.isEmpty || !panel.impedances.isEmpty) {
                        VStack(alignment: .leading, spacing: 2) {
                            if !panel.stackupSummary.isEmpty {
                                Text(panel.stackupSummary).dmono(10).foregroundStyle(BackplaneDesign.dim)
                            }
                            ForEach(panel.impedances, id: \.self) { imp in
                                Text("· " + imp).dmono(10).foregroundStyle(BackplaneDesign.faint)
                            }
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.horizontal, 10).padding(.vertical, 6)
                        .background(BackplaneDesign.background)
                        DLine()
                    }
                    canvasArea(panel: panel, pro: pro, layers: layers, curLayer: curLayer, board: board, draw: draw)
                    if !selectedId.isEmpty {
                        steeringBar(panel: panel, pro: pro, layerObj: layerObj, draw: draw)
                    } else {
                        quickPickStrip(layerObj: layerObj, pro: pro)
                    }
                }
            }
            .onChange(of: base.revision) { _, rev in
                if let oRev = optimisticRev, rev > oRev {
                    optimistic = [:]
                    optimisticRev = nil
                }
            }
            .task(id: "\(pro)|\(base.revision)|\(base.boardTrimmed)") {
                await hydrateIfNeeded(base: base, pro: pro)
            }
            .sheet(isPresented: $inspectorOpen) {
                if let item = findItem(id: selectedId, in: panel.layers) {
                    PlanItemInspectorSheet(
                        item: item.item,
                        curLayer: item.layer,
                        copper: panel.copperLayers,
                        allLayers: layers,
                        onApprove: { ok in
                            Task {
                                do { try await client.planApprove(pro, layer: "", ids: [item.item.id], approve: ok) }
                                catch { errMsg = error.localizedDescription }
                            }
                        },
                        onSteer: { inspectorOpen = false },
                        onCommit: { patch in
                            applyOptimisticAndSend(pro: pro, id: item.item.id, patch: patch, baseRev: base.revision)
                            if let mv = patch.layer, mv != item.layer {
                                activeLayer = mv
                            }
                        },
                        onNote: { text in
                            Task {
                                do {
                                    try await client.planUpdate(pro, ops: [["op": "note", "target": item.item.id, "text": text]], base: base.revision)
                                } catch { errMsg = error.localizedDescription }
                            }
                        }
                    )
                    .presentationDetents([.medium, .large])
                }
            }
        } else {
            VStack {
                DLoading(text: Tr.k("gpui.mgmt.plan.reading"))
                Spacer()
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .padding(14)
        }
    }

    private func headerView(panel: PlanPanel, pro: String) -> some View {
        let (stKey, stCol) = statusMeta(panel)
        return VStack(spacing: 0) {
            HStack(spacing: 6) {
                Text(Tr.k("gpui.mgmt.plan.before"))
                    .dmono(11, bold: true)
                    .foregroundStyle(BackplaneDesign.text)
                    .lineLimit(1)
                if !panel.empty {
                    Text(Tr.k(stKey))
                        .dmono(10, bold: true)
                        .foregroundStyle(stCol)
                        .padding(.horizontal, 4).padding(.vertical, 1)
                        .overlay(Rectangle().strokeBorder(stCol, lineWidth: 1))
                    if panel.revision > 0 {
                        let revStr = "v" + String(panel.revision)
                        Text(revStr).dmono(10).foregroundStyle(BackplaneDesign.dim)
                    }
                    if panel.total > 0 {
                        Text(Tr.gpui_mgmt_plan_done_of(done: "\(panel.done)", total: "\(panel.total)"))
                            .dmono(10)
                            .foregroundStyle(panel.awaiting > 0 ? BackplaneDesign.warn : BackplaneDesign.dim)
                            .lineLimit(1)
                    } else if panel.awaiting > 0 {
                        Text(Tr.gpui_mgmt_plan_awaiting_short(n: "\(panel.awaiting)"))
                            .dmono(10).foregroundStyle(BackplaneDesign.warn)
                    }
                }
                Spacer(minLength: 4)
                HStack(spacing: 0) {
                    toggleChip(Tr.k("gpui.bots.recall.on"), active: panel.on) {
                        Task { try? await client.planEnable(pro, on: true) }
                    }
                    toggleChip(Tr.k("gpui.bots.recall.off"), active: !panel.on) {
                        Task { try? await client.planEnable(pro, on: false) }
                    }
                }
            }
            .padding(.horizontal, 10).padding(.vertical, 6)
            .background(BackplaneDesign.panel)

            if !panel.empty && (panel.awaiting > 0 || panel.routing || (panel.live && panel.done > 0)) {
                HStack(spacing: 6) {
                    if panel.awaiting > 0 {
                        Button(Tr.k("gpui.mgmt.plan.all")) {
                            Task {
                                do { try await client.planApprove(pro, layer: "", ids: [], approve: true) }
                                catch { errMsg = error.localizedDescription }
                            }
                        }
                        .buttonStyle(DButton(primary: true, compact: true))
                        .accessibilityIdentifier("plan.approveAll")
                    }
                    if panel.routing {
                        Button(Tr.k("gpui.mgmt.plan.stop")) {
                            Task { try? await client.planStop(pro) }
                        }
                        .buttonStyle(DButton(danger: true, compact: true))
                    }
                    if panel.live && panel.done > 0 {
                        Button(Tr.k("gpui.mgmt.plan.verify")) {
                            Task { try? await client.planVerify(pro) }
                        }
                        .buttonStyle(DButton(compact: true))
                    }
                    Spacer()
                }
                .padding(.horizontal, 10).padding(.bottom, 5)
                .background(BackplaneDesign.panel)
            }

            if !panel.empty && panel.total > 0 {
                progressBar(panel: panel)
            }
            DLine()
        }
    }

    private func toggleChip(_ title: String, active: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(title)
                .dmono(10, bold: active)
                .foregroundStyle(active ? BackplaneDesign.onAccent : BackplaneDesign.dim)
                .padding(.horizontal, 8).frame(minHeight: 26)
                .background(active ? BackplaneDesign.accent : BackplaneDesign.raised)
                .overlay(Rectangle().strokeBorder(active ? BackplaneDesign.accent : BackplaneDesign.line, lineWidth: 1))
        }
        .buttonStyle(.plain)
    }

    private func progressBar(panel: PlanPanel) -> some View {
        let segs: [(Int, Color)] = [
            (panel.done, BackplaneDesign.ok),
            (panel.approved, PhaseColor.input),
            (panel.awaiting, BackplaneDesign.warn),
            (panel.blocked, BackplaneDesign.bad),
        ]
        let tot = max(1, panel.total)
        return GeometryReader { geo in
            HStack(spacing: 0) {
                ForEach(Array(segs.enumerated()), id: \.offset) { _, seg in
                    if seg.0 > 0 {
                        Rectangle()
                            .fill(seg.1)
                            .frame(width: geo.size.width * CGFloat(seg.0) / CGFloat(tot))
                    }
                }
                Spacer(minLength: 0)
            }
            .background(BackplaneDesign.line)
        }
        .frame(height: 3)
    }

    private func layerStripView(layers: [PlanLayer], copper: [PlanCopperLayer], curLayer: String, layerObj: PlanLayer?, board: PlanBoardGeom, pro: String) -> some View {
        let hasAwaiting = layerObj?.items.contains(where: { $0.state == "proposed" || $0.state == "edited" }) == true
        let subtitle = (layerObj?.subtitle.isEmpty == false ? layerObj?.subtitle : layerObj?.stackup) ?? ""
        return VStack(spacing: 0) {
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 6) {
                    ForEach(layers) { l in
                        let active = l.layer == curLayer
                        let empty = l.items.isEmpty
                        let baseTitle = PlanFloorplannerLogic.layerDisplayTitle(l.layer, copper: copper, allLayers: layers)
                        let label = empty ? baseTitle : "\(baseTitle) · \(l.items.count)"
                        Button {
                            activeLayer = l.layer
                            selectedVertex = nil
                        } label: {
                            Text(label)
                                .dmono(11, bold: active)
                                .foregroundStyle(active ? BackplaneDesign.onAccent : empty ? BackplaneDesign.faint : BackplaneDesign.dim)
                                .padding(.horizontal, 10).frame(minHeight: 30)
                                .background(active ? BackplaneDesign.accent : BackplaneDesign.raised)
                                .overlay(Rectangle().strokeBorder(active ? BackplaneDesign.accent : BackplaneDesign.line, lineWidth: 1))
                                .opacity(empty && !active ? 0.65 : 1.0)
                        }
                        .buttonStyle(.plain)
                        .accessibilityIdentifier("plan.layer.\(l.layer)")
                    }
                }
                .padding(.horizontal, 10).padding(.vertical, 5)
            }
            .background(BackplaneDesign.background)
            DLine()
            HStack(spacing: 6) {
                if !subtitle.isEmpty {
                    Text(subtitle).dmono(10).foregroundStyle(BackplaneDesign.dim).lineLimit(1).truncationMode(.tail)
                }
                Spacer(minLength: 4)
                Button(Tr.k("gpui.mgmt.plan.stackup.short") + (stackupOpen ? " ▾" : " ▸")) {
                    stackupOpen.toggle()
                }
                .buttonStyle(DButton(primary: stackupOpen, compact: true))
                Button(Tr.k("gpui.mgmt.plan.fit")) {
                    let sz = canvasSize
                    if sz.width > 10 && sz.height > 10 {
                        cam = PlanCamera.fit(bbox: board.bbox, width: sz.width, height: sz.height, pad: 22)
                    } else {
                        cam = nil
                    }
                }
                .buttonStyle(DButton(compact: true))
                .accessibilityIdentifier("plan.fit")
                if hasAwaiting {
                    Button {
                        Task { try? await client.planApprove(pro, layer: curLayer, ids: [], approve: true) }
                    } label: {
                        Text("✓").dmono(12, bold: true).foregroundStyle(BackplaneDesign.ok).frame(width: 28, height: 28)
                            .background(BackplaneDesign.raised).overlay(Rectangle().strokeBorder(BackplaneDesign.line, lineWidth: 1))
                    }.buttonStyle(.plain).accessibilityLabel(Tr.t("Approve layer"))
                    Button {
                        Task { try? await client.planApprove(pro, layer: curLayer, ids: [], approve: false) }
                    } label: {
                        Text("✗").dmono(12, bold: true).foregroundStyle(BackplaneDesign.bad).frame(width: 28, height: 28)
                            .background(BackplaneDesign.raised).overlay(Rectangle().strokeBorder(BackplaneDesign.line, lineWidth: 1))
                    }.buttonStyle(.plain).accessibilityLabel(Tr.k("gpui.mgmt.plan.rejectlayer"))
                }
            }
            .padding(.horizontal, 10).padding(.vertical, 4)
            .background(BackplaneDesign.panel)
            DLine()
        }
    }

    private func canvasArea(panel: PlanPanel, pro: String, layers: [PlanLayer], curLayer: String, board: PlanBoardGeom, draw: PlanDraw) -> some View {
        GeometryReader { geo in
            let w = max(1, Double(geo.size.width))
            let h = max(1, Double(geo.size.height))
            let activeCam = cam ?? PlanCamera.fit(bbox: board.bbox, width: w, height: h, pad: 22)
            ZStack(alignment: .topLeading) {
                Canvas { ctx, size in
                    drawScene(
                        ctx: &ctx,
                        size: size,
                        cam: activeCam,
                        layer: curLayer,
                        board: board,
                        draw: draw,
                        selectedId: selectedId,
                        selectedVertex: selectedVertex
                    )
                }
                .background(rgbColor(pcbSubstrateRGB))

                PlanTouchOverlay(
                    onTap: { pt in
                        handleTap(pt: pt, w: w, h: h, cam: activeCam, curLayer: curLayer, draw: draw, panel: panel, pro: pro)
                    },
                    onDoubleTap: { _ in
                        cam = PlanCamera.fit(bbox: board.bbox, width: w, height: h, pad: 22)
                    },
                    onPanBegin: { pt in
                        handlePanBegin(pt: pt, w: w, h: h, cam: activeCam, curLayer: curLayer, draw: draw)
                    },
                    onPanChange: { pt, trans in
                        handlePanChange(pt: pt, trans: trans, w: w, h: h, curLayer: curLayer, draw: draw, baseCam: activeCam)
                    },
                    onPanEnd: { vel in
                        handlePanEnd(vel: vel, w: w, h: h, layers: layers, curLayer: curLayer, board: board, draw: draw, panel: panel, pro: pro, baseCam: activeCam)
                    },
                    onPinch: { factor, anchor in
                        let base = cam ?? activeCam
                        cam = base.zoomed(factor: factor, anchorX: anchor.x, anchorY: anchor.y, width: w, height: h)
                    },
                    onTwoFingerPan: { delta in
                        let base = cam ?? activeCam
                        cam = base.panned(dxPx: delta.x, dyPx: delta.y)
                    }
                )

                // Floating coordinate reticle 44pt above finger when dragging a waypoint
                if let wp = dragWorldCoord, let sp = dragScreenPt {
                    let xStr = String(format: "%.2f", wp[0])
                    let yStr = String(format: "%.2f", wp[1])
                    Text(Tr.mobile_plan_coord(x: xStr, y: yStr))
                        .dmono(11, bold: true)
                        .foregroundStyle(rgbColor(pcbBadgeFgRGB))
                        .padding(.horizontal, 8).padding(.vertical, 4)
                        .background(rgbColor(pcbBadgeBgRGB, alpha: 0.94))
                        .overlay(Rectangle().strokeBorder(BackplaneDesign.accent, lineWidth: 1))
                        .position(x: min(max(65, sp.x), w - 65), y: max(22, sp.y - 44))
                        .allowsHitTesting(false)
                }

                if selectedId.isEmpty {
                    Text(Tr.k("mobile.plan.touchhint"))
                        .dmono(10)
                        .foregroundStyle(BackplaneDesign.faint)
                        .lineLimit(1)
                        .padding(.horizontal, 8).padding(.vertical, 3)
                        .background(rgbColor(pcbSubstrateRGB, alpha: 0.85))
                        .padding(6)
                        .allowsHitTesting(false)
                }
            }
            .onAppear {
                canvasSize = geo.size
                if cam == nil && w > 10 && h > 10 {
                    cam = PlanCamera.fit(bbox: board.bbox, width: w, height: h, pad: 22)
                }
            }
            .onChange(of: geo.size) { _, sz in
                canvasSize = sz
                if cam == nil && sz.width > 10 && sz.height > 10 {
                    cam = PlanCamera.fit(bbox: board.bbox, width: sz.width, height: sz.height, pad: 22)
                }
            }
            .onChange(of: curLayer) { _, _ in
                if cam == nil && w > 10 && h > 10 {
                    cam = PlanCamera.fit(bbox: board.bbox, width: w, height: h, pad: 22)
                }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func steeringBar(panel: PlanPanel, pro: String, layerObj: PlanLayer?, draw: PlanDraw) -> some View {
        let found = findItem(id: selectedId, in: panel.layers)
        let name = found?.item.name ?? selectedId
        let colHex: UInt32 = found?.item.kind == "plane"
            ? PlanFloorplannerLogic.railColorRGB(name, fallback: found?.item.color ?? "")
            : PlanFloorplannerLogic.busColorRGB(name, fallback: found?.item.color ?? "")
        let isLocked = found?.item.locked ?? false
        let canRemoveVertex: Bool = {
            guard !isLocked, let sv = selectedVertex, sv.id == selectedId else { return false }
            if let b = draw.bands.first(where: { $0.id == sv.id }) { return b.path.count > 2 }
            if let r = draw.regions.first(where: { $0.id == sv.id }) { return r.outline.count > 3 }
            return false
        }()

        return VStack(spacing: 0) {
            DLine()
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 6) {
                    Rectangle().fill(rgbColor(colHex)).frame(width: 10, height: 10)
                    Text(name).dmono(11, bold: true).foregroundStyle(BackplaneDesign.text).lineLimit(1)
                    Button(Tr.k("mobile.plan.details")) {
                        inspectorOpen = true
                    }
                    .buttonStyle(DButton(compact: true))
                    .accessibilityIdentifier("plan.details")
                    if canRemoveVertex, let sv = selectedVertex {
                        Button(Tr.k("mobile.plan.removepoint")) {
                            removeSelectedVertex(id: sv.id, idx: sv.idx, draw: draw, pro: pro, rev: panel.revision)
                        }
                        .buttonStyle(DButton(danger: true, compact: true))
                    }
                    if !undoStack.isEmpty {
                        Button(Tr.k("voice.cmd.undo")) {
                            if let prev = undoStack.popLast() {
                                applyOptimisticAndSend(pro: pro, id: selectedId, patch: prev, baseRev: panel.revision, recordUndo: false)
                            }
                        }
                        .buttonStyle(DButton(compact: true))
                    }
                    if optimistic[selectedId] != nil {
                        Button(Tr.k("mobile.plan.reset")) {
                            optimistic.removeValue(forKey: selectedId)
                            undoStack.removeAll()
                            selectedVertex = nil
                        }
                        .buttonStyle(DButton(compact: true))
                    }
                    Button(Tr.k("tour.done")) {
                        selectedId = ""
                        selectedVertex = nil
                    }
                    .buttonStyle(DButton(primary: true, compact: true))
                }
                .padding(.horizontal, 10).padding(.vertical, 6)
            }
            .background(BackplaneDesign.panel)
        }
    }

    private func quickPickStrip(layerObj: PlanLayer?, pro: String) -> some View {
        let items = layerObj?.items ?? []
        return VStack(spacing: 0) {
            DLine()
            if items.isEmpty {
                Text(Tr.k("gpui.mgmt.plan.noitems"))
                    .dmono(11).foregroundStyle(BackplaneDesign.faint)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, 10).padding(.vertical, 8)
                    .background(BackplaneDesign.panel)
            } else {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 6) {
                        ForEach(items) { it in
                            let col = it.kind == "plane"
                                ? PlanFloorplannerLogic.railColorRGB(it.name, fallback: it.color)
                                : PlanFloorplannerLogic.busColorRGB(it.name, fallback: it.color)
                            let stColor: Color = {
                                switch it.state {
                                case "blocked", "failed": return BackplaneDesign.bad
                                case "approved": return BackplaneDesign.accent
                                case "routed", "verified": return BackplaneDesign.ok
                                case "edited": return BackplaneDesign.warn
                                default: return BackplaneDesign.line
                                }
                            }()
                            Button {
                                selectedId = it.id
                                selectedVertex = nil
                                inspectorOpen = true
                                Task { try? await client.planFocus(pro, id: it.id) }
                            } label: {
                                HStack(spacing: 5) {
                                    Rectangle().fill(rgbColor(col)).frame(width: 8, height: 8)
                                    Text(it.name.isEmpty ? it.id : it.name).dmono(11).foregroundStyle(BackplaneDesign.text)
                                    if it.nets > 0 {
                                        Text("\(it.nets)").dmono(10).foregroundStyle(BackplaneDesign.dim)
                                    }
                                }
                                .padding(.horizontal, 8).frame(minHeight: 30)
                                .background(BackplaneDesign.raised)
                                .overlay(Rectangle().strokeBorder(stColor, lineWidth: 1))
                            }
                            .buttonStyle(.plain)
                            .accessibilityIdentifier("plan.pick.\(it.id)")
                        }
                    }
                    .padding(.horizontal, 10).padding(.vertical, 6)
                }
                .background(BackplaneDesign.panel)
            }
        }
    }

    // MARK: Interaction & Hydration
    private func hydrateIfNeeded(base: PlanPanel, pro: String) async {
        guard client.conn == .online, !pro.isEmpty else { return }
        let key = "\(pro):\(base.revision)"
        let needHydrate = base.boardTrimmed || base.board == nil
        guard needHydrate else {
            hydratedKey = key; hydratedRev = base.revision
            hydratedBoard = nil; hydratedDraw = nil
            return
        }
        guard hydratedKey != key || hydratedBoard == nil else { return }
        guard let d = try? await client.planGet(pro, board: true),
              let full = PlanFloorplannerLogic.unwrapPanel(from: d) else { return }
        guard full.revision >= hydratedRev else { return }
        hydratedKey = "\(pro):\(full.revision)"
        hydratedRev = full.revision
        hydratedBoard = full.board
        hydratedDraw = full.draw
    }

    private func handleTap(pt: CGPoint, w: Double, h: Double, cam: PlanCamera, curLayer: String, draw: PlanDraw, panel: PlanPanel, pro: String) {
        let hit = PlanFloorplannerLogic.hitTest(
            bands: draw.bands, regions: draw.regions, layer: curLayer,
            selectedId: selectedId, cam: cam, width: w, height: h, x: pt.x, y: pt.y
        )
        switch hit {
        case .vertex(let id, _, let idx):
            selectedId = id
            selectedVertex = (id, idx)
        case .midpoint(let id, let isPlane, let segIdx):
            if isPlane, let r = draw.regions.first(where: { $0.id == id }), !r.locked {
                let nextIdx = (segIdx + 1) % max(1, r.outline.count)
                let mid = [(r.outline[segIdx][0] + r.outline[nextIdx][0]) * 0.5, (r.outline[segIdx][1] + r.outline[nextIdx][1]) * 0.5]
                let updated = PlanFloorplannerLogic.insertVertex(pts: r.outline, segIdx: segIdx, pos: mid, closed: true)
                let prevPatch = optimistic[id] ?? PlanOptimisticPatch(outline: r.outline)
                undoStack.append(prevPatch)
                applyOptimisticAndSend(pro: pro, id: id, patch: PlanOptimisticPatch(outline: updated), baseRev: panel.revision, recordUndo: false)
                selectedId = id; selectedVertex = (id, segIdx + 1)
            } else if !isPlane, let b = draw.bands.first(where: { $0.id == id }), !b.locked {
                let mid = [(b.path[segIdx][0] + b.path[segIdx + 1][0]) * 0.5, (b.path[segIdx][1] + b.path[segIdx + 1][1]) * 0.5]
                let updated = PlanFloorplannerLogic.insertVertex(pts: b.path, segIdx: segIdx, pos: mid, closed: false)
                let prevPatch = optimistic[id] ?? PlanOptimisticPatch(path: b.path)
                undoStack.append(prevPatch)
                applyOptimisticAndSend(pro: pro, id: id, patch: PlanOptimisticPatch(path: updated), baseRev: panel.revision, recordUndo: false)
                selectedId = id; selectedVertex = (id, segIdx + 1)
            }
        case .band(let id), .region(let id):
            let wasSame = selectedId == id
            selectedId = id
            selectedVertex = nil
            if wasSame { inspectorOpen = true }
            Task { try? await client.planFocus(pro, id: id) }
        case .none:
            selectedId = ""
            selectedVertex = nil
        }
    }

    private func handlePanBegin(pt: CGPoint, w: Double, h: Double, cam: PlanCamera, curLayer: String, draw: PlanDraw) {
        let hit = PlanFloorplannerLogic.hitTest(
            bands: draw.bands, regions: draw.regions, layer: curLayer,
            selectedId: selectedId, cam: cam, width: w, height: h, x: pt.x, y: pt.y
        )
        if case .vertex(let id, let isPlane, let idx) = hit {
            if isPlane, let r = draw.regions.first(where: { $0.id == id }), !r.locked, idx < r.outline.count {
                let vpx = cam.toPx(x: r.outline[idx][0], y: r.outline[idx][1], width: w, height: h)
                let grab = CGSize(width: vpx.x - pt.x, height: vpx.y - pt.y)
                selectedId = id; selectedVertex = (id, idx)
                undoStack.append(optimistic[id] ?? PlanOptimisticPatch(outline: r.outline))
                dragKind = .vertex(id: id, isPlane: true, idx: idx, startPts: r.outline, grabOffsetPx: grab)
                dragWorldCoord = r.outline[idx]
                dragScreenPt = vpx
                return
            } else if !isPlane, let b = draw.bands.first(where: { $0.id == id }), !b.locked, idx < b.path.count {
                let vpx = cam.toPx(x: b.path[idx][0], y: b.path[idx][1], width: w, height: h)
                let grab = CGSize(width: vpx.x - pt.x, height: vpx.y - pt.y)
                selectedId = id; selectedVertex = (id, idx)
                undoStack.append(optimistic[id] ?? PlanOptimisticPatch(path: b.path))
                dragKind = .vertex(id: id, isPlane: false, idx: idx, startPts: b.path, grabOffsetPx: grab)
                dragWorldCoord = b.path[idx]
                dragScreenPt = vpx
                return
            }
        }
        dragKind = .pan(startCam: cam)
    }

    private func handlePanChange(pt: CGPoint, trans: CGPoint, w: Double, h: Double, curLayer: String, draw: PlanDraw, baseCam: PlanCamera) {
        switch dragKind {
        case .pan(let startCam):
            cam = startCam.panned(dxPx: trans.x, dyPx: trans.y)
        case .vertex(let id, let isPlane, let idx, var pts, let grab):
            let targetPx = CGPoint(x: pt.x + grab.width, y: pt.y + grab.height)
            let wpt = baseCam.toWorld(px: targetPx.x, py: targetPx.y, width: w, height: h)
            guard idx < pts.count else { return }
            pts[idx] = [(wpt[0] * 100).rounded() / 100, (wpt[1] * 100).rounded() / 100]
            dragWorldCoord = pts[idx]
            dragScreenPt = targetPx
            var patch = optimistic[id] ?? PlanOptimisticPatch()
            if isPlane { patch.outline = pts } else { patch.path = pts }
            optimistic[id] = patch
        case .none:
            break
        }
    }

    private func handlePanEnd(vel: CGPoint, w: Double, h: Double, layers: [PlanLayer], curLayer: String, board: PlanBoardGeom, draw: PlanDraw, panel: PlanPanel, pro: String, baseCam: PlanCamera) {
        let kind = dragKind
        dragKind = .none
        dragWorldCoord = nil
        dragScreenPt = nil
        switch kind {
        case .vertex(let id, let isPlane, _, _, _):
            if let patch = optimistic[id] {
                applyOptimisticAndSend(pro: pro, id: id, patch: patch, baseRev: panel.revision, recordUndo: false, onlyGeom: isPlane ? "outline" : "path")
            }
        case .pan(let startCam):
            // Quick horizontal flick when at fit zoom switches layers
            let fitCam = PlanCamera.fit(bbox: board.bbox, width: w, height: h, pad: 22)
            if abs(startCam.scale - fitCam.scale) / max(fitCam.scale, 0.01) < 0.08 && abs(vel.x) > 520 && abs(vel.x) > abs(vel.y) * 1.6 && layers.count > 1 {
                if let idx = layers.firstIndex(where: { $0.layer == curLayer }) {
                    let step = vel.x < 0 ? 1 : -1
                    let nxt = (idx + step + layers.count) % layers.count
                    activeLayer = layers[nxt].layer
                    selectedVertex = nil
                    cam = fitCam
                }
            }
        case .none:
            break
        }
    }

    private func removeSelectedVertex(id: String, idx: Int, draw: PlanDraw, pro: String, rev: Int) {
        if let b = draw.bands.first(where: { $0.id == id }),
           let nextPts = PlanFloorplannerLogic.removeVertex(pts: b.path, idx: idx, closed: false) {
            undoStack.append(optimistic[id] ?? PlanOptimisticPatch(path: b.path))
            selectedVertex = nil
            applyOptimisticAndSend(pro: pro, id: id, patch: PlanOptimisticPatch(path: nextPts), baseRev: rev, recordUndo: false)
        } else if let r = draw.regions.first(where: { $0.id == id }),
                  let nextPts = PlanFloorplannerLogic.removeVertex(pts: r.outline, idx: idx, closed: true) {
            undoStack.append(optimistic[id] ?? PlanOptimisticPatch(outline: r.outline))
            selectedVertex = nil
            applyOptimisticAndSend(pro: pro, id: id, patch: PlanOptimisticPatch(outline: nextPts), baseRev: rev, recordUndo: false)
        }
    }

    private func applyOptimisticAndSend(pro: String, id: String, patch: PlanOptimisticPatch, baseRev: Int, recordUndo: Bool = true, onlyGeom: String? = nil) {
        if recordUndo, let prev = optimistic[id] { undoStack.append(prev) }
        var cur = optimistic[id] ?? PlanOptimisticPatch()
        if let p = patch.path { cur.path = p }
        if let o = patch.outline { cur.outline = o }
        if let w = patch.width { cur.width = w }
        if let l = patch.layer { cur.layer = l }
        if let lk = patch.locked { cur.locked = lk }
        optimistic[id] = cur
        optimisticRev = baseRev

        var fields: [String: Any] = [:]
        if onlyGeom == "path" || (onlyGeom == nil && patch.path != nil), let p = patch.path { fields["path"] = p }
        if onlyGeom == "outline" || (onlyGeom == nil && patch.outline != nil), let o = patch.outline { fields["outline"] = o }
        if let w = patch.width { fields["width"] = w }
        if let l = patch.layer { fields["layer"] = l }
        if let lk = patch.locked { fields["locked"] = lk }
        guard !fields.isEmpty else { return }
        Task {
            do {
                try await client.planUpdate(pro, ops: [["op": "set", "id": id, "fields": fields]], base: baseRev)
            } catch {
                errMsg = error.localizedDescription
            }
        }
    }

    private func findItem(id: String, in layers: [PlanLayer]) -> (layer: String, item: PlanItem)? {
        for l in layers {
            if let it = l.items.first(where: { $0.id == id }) { return (l.layer, it) }
        }
        return nil
    }

    private func statusMeta(_ d: PlanPanel) -> (String, Color) {
        let s = d.status
        if s == "blocked" || d.blocked > 0 { return ("gpui.mgmt.plan.st.blocked", BackplaneDesign.bad) }
        if s == "routing" || d.routing { return ("gpui.mgmt.plan.st.routing", BackplaneDesign.accent) }
        if s == "awaiting" || d.awaiting > 0 { return ("gpui.mgmt.plan.st.awaiting", BackplaneDesign.warn) }
        if s == "verified" { return ("gpui.mgmt.plan.st.verified", BackplaneDesign.ok) }
        if s == "routed" { return ("gpui.mgmt.plan.st.routed", BackplaneDesign.ok) }
        if s == "approved" { return ("gpui.mgmt.plan.st.approved", BackplaneDesign.accent) }
        if s == "partial" { return ("gpui.mgmt.plan.st.partial", BackplaneDesign.dim) }
        if s == "rejected" { return ("gpui.mgmt.plan.st.rejected", BackplaneDesign.faint) }
        return ("gpui.mgmt.plan.st.empty", BackplaneDesign.faint)
    }

    // MARK: 10-step PCB Darkroom Canvas Painter
    private func drawScene(
        ctx: inout GraphicsContext,
        size: CGSize,
        cam: PlanCamera,
        layer: String,
        board: PlanBoardGeom,
        draw: PlanDraw,
        selectedId: String,
        selectedVertex: (id: String, idx: Int)?
    ) {
        let w = Double(size.width)
        let h = Double(size.height)
        let pt = { (wx: Double, wy: Double) -> CGPoint in cam.toPx(x: wx, y: wy, width: w, height: h) }

        // 1. Substrate fill
        ctx.fill(Path(CGRect(origin: .zero, size: size)), with: .color(rgbColor(pcbSubstrateRGB)))

        // 2. Board interior
        let polys = board.outline.filter { $0.count >= 3 }
        if !polys.isEmpty {
            var p = Path()
            for poly in polys {
                p.move(to: pt(poly[0][0], poly[0][1]))
                for v in poly.dropFirst() { p.addLine(to: pt(v[0], v[1])) }
                p.closeSubpath()
            }
            ctx.fill(p, with: .color(rgbColor(pcbBoardFillRGB)), style: FillStyle(eoFill: true))
        } else {
            let p0 = pt(board.bbox[0], board.bbox[1]), p1 = pt(board.bbox[2], board.bbox[3])
            ctx.fill(Path(CGRect(x: min(p0.x, p1.x), y: min(p0.y, p1.y), width: abs(p1.x - p0.x), height: abs(p1.y - p0.y))), with: .color(rgbColor(pcbBoardFillRGB)))
        }

        // 3. Power plane pours & in-pour labels
        for r in draw.regions where r.layer == layer && r.outline.count >= 3 {
            let sel = selectedId == r.id
            let col = PlanFloorplannerLogic.railColorRGB(r.net.isEmpty ? r.id : r.net, fallback: r.color)
            let (fa, oa, bad): (Double, Double, Bool) = {
                switch r.state {
                case "approved", "routing": return (sel ? 0.42 : 0.30, 0.84, false)
                case "edited": return (sel ? 0.45 : 0.34, 0.90, false)
                case "routed", "verified": return (0.17, 0.46, false)
                case "rejected", "failed": return (0.09, 0.24, false)
                case "blocked": return (0.35, 0.95, true)
                default: return (sel ? 0.38 : 0.22, 0.69, false)
                }
            }()
            var fillP = Path()
            fillP.move(to: pt(r.outline[0][0], r.outline[0][1]))
            for v in r.outline.dropFirst() { fillP.addLine(to: pt(v[0], v[1])) }
            fillP.closeSubpath()
            for hole in r.holes where hole.count >= 3 {
                fillP.move(to: pt(hole[0][0], hole[0][1]))
                for v in hole.dropFirst() { fillP.addLine(to: pt(v[0], v[1])) }
                fillP.closeSubpath()
            }
            ctx.fill(fillP, with: .color(rgbColor(col, alpha: fa)), style: FillStyle(eoFill: true))

            var edgeP = Path()
            edgeP.move(to: pt(r.outline[0][0], r.outline[0][1]))
            for v in r.outline.dropFirst() { edgeP.addLine(to: pt(v[0], v[1])) }
            edgeP.closeSubpath()
            ctx.stroke(edgeP, with: .color(bad ? rgbColor(pcbBadRedRGB, alpha: 0.95) : sel ? .white : rgbColor(col, alpha: oa)), style: StrokeStyle(lineWidth: sel || bad ? 2.0 : 1.25, lineJoin: .round))

            if !r.net.isEmpty {
                var sx0 = Double.infinity, sy0 = Double.infinity, sx1 = -Double.infinity, sy1 = -Double.infinity
                for v in r.outline {
                    let q = pt(v[0], v[1])
                    sx0 = min(sx0, Double(q.x)); sy0 = min(sy0, Double(q.y))
                    sx1 = max(sx1, Double(q.x)); sy1 = max(sy1, Double(q.y))
                }
                if abs(sx1 - sx0) >= 38 && abs(sy1 - sy0) >= 18, let lp = PlanFloorplannerLogic.pourLabelPoint(outer: r.outline, holes: r.holes) {
                    let lpx = pt(lp[0], lp[1])
                    ctx.draw(Text(r.net).font(.system(size: 10, weight: .bold, design: .monospaced)).foregroundColor(rgbColor(col, alpha: sel ? 0.91 : 0.67)), at: lpx, anchor: .center)
                }
            }
        }

        // 4. Crisp Edge.Cuts outline
        if !polys.isEmpty {
            var ep = Path()
            for poly in polys {
                ep.move(to: pt(poly[0][0], poly[0][1]))
                for v in poly.dropFirst() { ep.addLine(to: pt(v[0], v[1])) }
                ep.closeSubpath()
            }
            ctx.stroke(ep, with: .color(rgbColor(pcbEdgeStrokeRGB, alpha: 0.91)), style: StrokeStyle(lineWidth: 1.25, lineCap: .round, lineJoin: .round))
        } else {
            let p0 = pt(board.bbox[0], board.bbox[1]), p1 = pt(board.bbox[2], board.bbox[3])
            ctx.stroke(Path(CGRect(x: min(p0.x, p1.x), y: min(p0.y, p1.y), width: abs(p1.x - p0.x), height: abs(p1.y - p0.y))), with: .color(rgbColor(pcbEdgeStrokeRGB, alpha: 0.91)), lineWidth: 1.25)
        }

        // 5. Component courtyards
        var solidP = Path(), dashP = Path()
        for p in board.parts where p.b.count == 4 {
            let p0 = pt(p.b[0], p.b[1]), p1 = pt(p.b[2], p.b[3])
            let rect = CGRect(x: min(p0.x, p1.x), y: min(p0.y, p1.y), width: abs(p1.x - p0.x), height: abs(p1.y - p0.y))
            if p.o { dashP.addRect(rect) } else { solidP.addRect(rect) }
        }
        ctx.stroke(solidP, with: .color(rgbColor(pcbPartStrokeRGB, alpha: 0.72)), lineWidth: 0.85)
        ctx.stroke(dashP, with: .color(rgbColor(pcbOverhangRGB, alpha: 0.85)), style: StrokeStyle(lineWidth: 0.95, dash: [4, 3]))

        // 6. Refdes labels avoiding corridor pills
        let layerBands = draw.bands.filter { $0.layer == layer }
        let pills = PlanFloorplannerLogic.placeCorridorPills(bands: layerBands, cam: cam, width: w, height: h, selectedId: selectedId)
        let pillBoxes = pills.map { $0.rect.insetBy(dx: -1, dy: -1) }
        let pinnedRefs = layerBands.flatMap { $0.from + $0.to }
        for lbl in PlanFloorplannerLogic.layoutLabels(parts: board.parts, cam: cam, width: w, height: h, pinned: pinnedRefs, blocked: pillBoxes) {
            ctx.draw(Text(lbl.text).font(.system(size: 9.5, design: .monospaced)).foregroundColor(rgbColor(pcbRefdesRGB, alpha: 0.72)), at: CGPoint(x: lbl.cx, y: lbl.cy), anchor: .center)
        }

        // 7. Signal ratsnest guides
        for b in layerBands where !b.guides.isEmpty {
            let active = selectedId == b.id
            let col = PlanFloorplannerLogic.busColorRGB(b.name.isEmpty ? b.id : b.name, fallback: b.color)
            var gp = Path()
            for seg in b.guides where seg.count == 4 {
                gp.move(to: pt(seg[0], seg[1]))
                gp.addLine(to: pt(seg[2], seg[3]))
            }
            ctx.stroke(gp, with: .color(rgbColor(col, alpha: active ? 0.38 : 0.17)), lineWidth: active ? 1.0 : 0.7)
        }

        // 8 & 9. Multi-pass signal corridors & diamond vias
        for b in layerBands where b.path.count >= 2 {
            let sel = selectedId == b.id
            let bad = b.state == "blocked"
            let (ga, ba, ca): (Double, Double, Double) = {
                switch b.state {
                case "approved", "routing": return (sel ? 0.26 : 0.16, sel ? 0.63 : 0.51, sel ? 1.0 : 0.91)
                case "edited": return (sel ? 0.30 : 0.20, sel ? 0.68 : 0.56, sel ? 1.0 : 0.95)
                case "routed", "verified": return (0.08, 0.26, 0.59)
                case "rejected", "failed": return (0.05, 0.13, 0.27)
                case "blocked": return (0.28, 0.58, 0.95)
                default: return (sel ? 0.24 : 0.14, sel ? 0.57 : 0.45, sel ? 1.0 : 0.85)
                }
            }()
            let col = PlanFloorplannerLogic.busColorRGB(b.name.isEmpty ? b.id : b.name, fallback: b.color)
            let wpx = PlanFloorplannerLogic.bandStrokePx(widthMm: b.width, scale: cam.scale)
            var polyP = Path()
            polyP.move(to: pt(b.path[0][0], b.path[0][1]))
            for v in b.path.dropFirst() { polyP.addLine(to: pt(v[0], v[1])) }
            ctx.stroke(polyP, with: .color(rgbColor(col, alpha: ga)), style: StrokeStyle(lineWidth: wpx + 6, lineCap: .round, lineJoin: .round))
            if bad || sel {
                ctx.stroke(polyP, with: .color(bad ? rgbColor(pcbBadRedRGB, alpha: 0.91) : .white.opacity(0.75)), style: StrokeStyle(lineWidth: wpx + 2.5, lineCap: .round, lineJoin: .round))
            }
            ctx.stroke(polyP, with: .color(rgbColor(col, alpha: ba)), style: StrokeStyle(lineWidth: wpx, lineCap: .round, lineJoin: .round))
            ctx.stroke(polyP, with: .color(rgbColor(col, alpha: ca)), style: StrokeStyle(lineWidth: min(5, max(1.8, wpx * 0.34)), lineCap: .round, lineJoin: .round))

            for v in b.vias where v.count >= 2 {
                let q = pt(v[0], v[1]); let r = 4.2
                var dp = Path()
                dp.move(to: CGPoint(x: q.x, y: q.y - r)); dp.addLine(to: CGPoint(x: q.x + r, y: q.y))
                dp.addLine(to: CGPoint(x: q.x, y: q.y + r)); dp.addLine(to: CGPoint(x: q.x - r, y: q.y)); dp.closeSubpath()
                ctx.fill(dp, with: .color(rgbColor(pcbSubstrateRGB, alpha: 0.95)))
                ctx.stroke(dp, with: .color(sel ? .white : rgbColor(col)), lineWidth: 1.5)
            }
        }

        // 10. Corridor pills
        for pill in pills {
            let rect = pill.rect
            ctx.fill(Path(rect), with: .color(rgbColor(pcbBadgeBgRGB, alpha: 0.94)))
            ctx.stroke(Path(rect), with: .color(pill.selected ? .white : rgbColor(pill.colorRGB)), lineWidth: 1.2)
            ctx.draw(Text(pill.text).font(.system(size: 10, weight: .bold, design: .monospaced)).foregroundColor(rgbColor(pcbBadgeFgRGB)), at: CGPoint(x: pill.cx, y: pill.cy), anchor: .center)
        }

        // 11. Touch-friendly selected vertex & + midpoint handles
        if !selectedId.isEmpty {
            func paintHandles(_ pts: [[Double]], _ closed: Bool, into c: inout GraphicsContext) {
                let segs = closed ? pts.count : max(0, pts.count - 1)
                for i in 0..<segs {
                    let a = pts[i], b = pts[(i + 1) % pts.count]
                    let mp = pt((a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5)
                    let rect = CGRect(x: mp.x - 10, y: mp.y - 10, width: 20, height: 20)
                    c.fill(Path(rect), with: .color(rgbColor(pcbBoardFillRGB, alpha: 0.92)))
                    c.stroke(Path(rect), with: .color(.white.opacity(0.65)), lineWidth: 1.2)
                    c.draw(Text("+").font(.system(size: 11, weight: .bold, design: .monospaced)).foregroundColor(.white), at: mp, anchor: .center)
                }
                for (i, p) in pts.enumerated() {
                    let q = pt(p[0], p[1])
                    let isSelV = selectedVertex?.id == selectedId && selectedVertex?.idx == i
                    let sz: Double = isSelV ? 18 : 14
                    let rect = CGRect(x: q.x - sz * 0.5, y: q.y - sz * 0.5, width: sz, height: sz)
                    c.fill(Path(rect), with: .color(isSelV ? rgbColor(pcbOverhangRGB) : .white))
                    c.stroke(Path(rect), with: .color(rgbColor(pcbSubstrateRGB)), lineWidth: 1.8)
                }
            }
            if let b = draw.bands.first(where: { $0.id == selectedId && $0.layer == layer && !$0.locked }), !b.path.isEmpty {
                paintHandles(b.path, false, into: &ctx)
            } else if let r = draw.regions.first(where: { $0.id == selectedId && $0.layer == layer && !$0.locked }), !r.outline.isEmpty {
                paintHandles(r.outline, true, into: &ctx)
            }
        }
    }
}

private struct PlanItemInspectorSheet: View {
    let item: PlanItem
    let curLayer: String
    let copper: [PlanCopperLayer]
    let allLayers: [PlanLayer]
    let onApprove: (Bool) -> Void
    let onSteer: () -> Void
    let onCommit: (PlanOptimisticPatch) -> Void
    let onNote: (String) -> Void

    @State private var noteText = ""
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        let colHex = item.kind == "plane"
            ? PlanFloorplannerLogic.railColorRGB(item.name, fallback: item.color)
            : PlanFloorplannerLogic.busColorRGB(item.name, fallback: item.color)
        let layerTitle = PlanFloorplannerLogic.layerDisplayTitle(curLayer, copper: copper, allLayers: allLayers)
        let widthMm = item.width ?? 1.5
        let moveOpts = copper.isEmpty ? allLayers.map(\.layer) : copper.map(\.layer)

        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 8) {
                Rectangle().fill(rgbColor(colHex)).frame(width: 12, height: 12)
                Text(item.name.isEmpty ? item.id : item.name).dmono(15, bold: true).foregroundStyle(BackplaneDesign.text)
                Text(layerTitle).dmono(12).foregroundStyle(BackplaneDesign.dim)
                if item.edited || item.state == "edited" {
                    Text(Tr.k("gpui.mgmt.plan.edited")).dmono(10).foregroundStyle(BackplaneDesign.warn)
                        .padding(.horizontal, 5).padding(.vertical, 2).overlay(Rectangle().strokeBorder(BackplaneDesign.warn, lineWidth: 1))
                }
                if item.locked {
                    Text(Tr.k("gpui.mgmt.plan.locked")).dmono(10).foregroundStyle(BackplaneDesign.dim)
                        .padding(.horizontal, 5).padding(.vertical, 2).overlay(Rectangle().strokeBorder(BackplaneDesign.line, lineWidth: 1))
                }
                Spacer()
                Button { dismiss() } label: {
                    Image(systemName: "xmark").font(.system(size: 13, weight: .bold)).foregroundStyle(BackplaneDesign.dim).frame(width: 32, height: 32)
                }.buttonStyle(.plain).accessibilityLabel(Tr.k("common.close"))
            }

            HStack(spacing: 10) {
                if item.kind == "corridor" {
                    Text(Tr.gpui_mgmt_plan_nets(n: item.nets)).dmono(12).foregroundStyle(BackplaneDesign.dim)
                } else if !item.reference.isEmpty {
                    Text(Tr.gpui_mgmt_plan_reference(layer: item.reference)).dmono(12).foregroundStyle(BackplaneDesign.dim)
                }
                if !item.impedance.isEmpty {
                    Text(item.impedance).dmono(12).foregroundStyle(BackplaneDesign.dim)
                }
            }
            if !item.problemText.isEmpty {
                Text(item.problemText).dmono(11).foregroundStyle(BackplaneDesign.bad)
            }

            HStack(spacing: 8) {
                if item.state == "proposed" || item.state == "edited" {
                    Button(Tr.t("Approve")) { onApprove(true); dismiss() }.buttonStyle(DButton(primary: true, compact: true))
                    Button(Tr.k("gpui.mgmt.plan.reject")) { onApprove(false); dismiss() }.buttonStyle(DButton(danger: true, compact: true))
                }
                Button(Tr.k(item.locked ? "gpui.mgmt.plan.unlock" : "gpui.mgmt.plan.lock")) {
                    onCommit(PlanOptimisticPatch(locked: !item.locked))
                }.buttonStyle(DButton(compact: true))
                if !item.locked {
                    Button(Tr.k("mobile.plan.steer")) { onSteer() }.buttonStyle(DButton(compact: true))
                }
            }

            if !item.locked && item.kind == "corridor" {
                DLine()
                HStack(spacing: 8) {
                    Text(Tr.k("gpui.mgmt.plan.width")).dmono(12).foregroundStyle(BackplaneDesign.dim)
                    Button("−") {
                        let nw = ((max(0.15, widthMm - 0.25)) * 100).rounded() / 100
                        onCommit(PlanOptimisticPatch(width: nw))
                    }.buttonStyle(DButton(compact: true))
                    Text(Tr.mobile_plan_widthmm(v: String(format: "%.2f", widthMm))).dmono(13, bold: true).foregroundStyle(BackplaneDesign.text)
                    Button("+") {
                        let nw = ((min(25.0, widthMm + 0.25)) * 100).rounded() / 100
                        onCommit(PlanOptimisticPatch(width: nw))
                    }.buttonStyle(DButton(compact: true))
                }
            }

            if !item.locked && !moveOpts.isEmpty {
                DLine()
                Text(Tr.k("gpui.mgmt.plan.layer.move")).dmono(11).foregroundStyle(BackplaneDesign.dim)
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 6) {
                        ForEach(moveOpts, id: \.self) { ml in
                            let title = PlanFloorplannerLogic.layerDisplayTitle(ml, copper: copper, allLayers: allLayers)
                            Button(title) {
                                if ml != curLayer {
                                    onCommit(PlanOptimisticPatch(layer: ml))
                                    dismiss()
                                }
                            }.buttonStyle(DButton(primary: ml == curLayer, compact: true))
                        }
                    }
                }
            }

            DLine()
            HStack(spacing: 6) {
                TextField(Tr.k("gpui.mgmt.plan.note.placeholder"), text: $noteText)
                    .dmono(12).padding(.horizontal, 10).frame(height: 36).dbox()
                Button(Tr.k("gpui.mgmt.plan.addnote")) {
                    let t = noteText.trimmingCharacters(in: .whitespacesAndNewlines)
                    guard !t.isEmpty else { return }
                    noteText = ""
                    onNote(t)
                    dismiss()
                }.buttonStyle(DButton(primary: true, compact: true))
                    .disabled(noteText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
            Spacer(minLength: 0)
        }
        .padding(14)
        .background(BackplaneDesign.background)
    }
}

/// UIKit gesture recognizer bridge giving smooth pinch-around-centroid, 2-finger pan, and 1-finger waypoint drag.
private struct PlanTouchOverlay: UIViewRepresentable {
    var onTap: (CGPoint) -> Void
    var onDoubleTap: (CGPoint) -> Void
    var onPanBegin: (CGPoint) -> Void
    var onPanChange: (CGPoint, CGPoint) -> Void
    var onPanEnd: (CGPoint) -> Void
    var onPinch: (Double, CGPoint) -> Void
    var onTwoFingerPan: (CGPoint) -> Void

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    func makeUIView(context: Context) -> UIView {
        let v = UIView()
        v.backgroundColor = .clear
        v.isMultipleTouchEnabled = true

        let c = context.coordinator
        let dTap = UITapGestureRecognizer(target: c, action: #selector(Coordinator.handleDoubleTap(_:)))
        dTap.numberOfTapsRequired = 2

        let tap = UITapGestureRecognizer(target: c, action: #selector(Coordinator.handleTap(_:)))
        tap.numberOfTapsRequired = 1
        tap.require(toFail: dTap)

        let onePan = UIPanGestureRecognizer(target: c, action: #selector(Coordinator.handleOnePan(_:)))
        onePan.minimumNumberOfTouches = 1
        onePan.maximumNumberOfTouches = 1

        let twoPan = UIPanGestureRecognizer(target: c, action: #selector(Coordinator.handleTwoPan(_:)))
        twoPan.minimumNumberOfTouches = 2
        twoPan.maximumNumberOfTouches = 2
        twoPan.delegate = c

        let pinch = UIPinchGestureRecognizer(target: c, action: #selector(Coordinator.handlePinch(_:)))
        pinch.delegate = c

        v.addGestureRecognizer(dTap)
        v.addGestureRecognizer(tap)
        v.addGestureRecognizer(onePan)
        v.addGestureRecognizer(twoPan)
        v.addGestureRecognizer(pinch)
        return v
    }

    func updateUIView(_ uiView: UIView, context: Context) {
        context.coordinator.parent = self
    }

    final class Coordinator: NSObject, UIGestureRecognizerDelegate {
        var parent: PlanTouchOverlay
        private var lastTwoPanTrans: CGPoint = .zero
        init(_ parent: PlanTouchOverlay) { self.parent = parent }

        func gestureRecognizer(_ g: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer) -> Bool {
            (g is UIPinchGestureRecognizer && (other as? UIPanGestureRecognizer)?.minimumNumberOfTouches == 2)
                || (other is UIPinchGestureRecognizer && (g as? UIPanGestureRecognizer)?.minimumNumberOfTouches == 2)
        }

        @objc func handleTap(_ g: UITapGestureRecognizer) {
            if g.state == .ended { parent.onTap(g.location(in: g.view)) }
        }

        @objc func handleDoubleTap(_ g: UITapGestureRecognizer) {
            if g.state == .ended { parent.onDoubleTap(g.location(in: g.view)) }
        }

        @objc func handleOnePan(_ g: UIPanGestureRecognizer) {
            let loc = g.location(in: g.view)
            let tr = g.translation(in: g.view)
            switch g.state {
            case .began: parent.onPanBegin(loc)
            case .changed: parent.onPanChange(loc, tr)
            case .ended, .cancelled, .failed: parent.onPanEnd(g.velocity(in: g.view))
            default: break
            }
        }

        @objc func handleTwoPan(_ g: UIPanGestureRecognizer) {
            let tr = g.translation(in: g.view)
            switch g.state {
            case .began: lastTwoPanTrans = .zero
            case .changed:
                let d = CGPoint(x: tr.x - lastTwoPanTrans.x, y: tr.y - lastTwoPanTrans.y)
                lastTwoPanTrans = tr
                parent.onTwoFingerPan(d)
            case .ended, .cancelled, .failed: lastTwoPanTrans = .zero
            default: break
            }
        }

        @objc func handlePinch(_ g: UIPinchGestureRecognizer) {
            guard g.state == .changed, g.numberOfTouches >= 2 else { return }
            let scale = Double(g.scale)
            g.scale = 1.0
            let anchor = g.location(in: g.view)
            parent.onPinch(scale, anchor)
        }
    }
}
