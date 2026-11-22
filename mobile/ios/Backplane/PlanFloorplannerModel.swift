import Foundation
import CoreGraphics

// Pure PCB Floorplanner models & geometry/layout logic shared between the UI Canvas and standalone swiftc tests.
// Matches web/src/mgmt-model.ts and native/gpui/src/screens/mgmt_plan_canvas.rs.

struct PlanCopperLayer: Codable, Equatable {
    var layer: String = ""
    var index: Int = 0
    var role: String = ""
    var title: String = ""
    var subtitle: String = ""

    enum CodingKeys: String, CodingKey { case layer, index, role, title, subtitle }
    init(layer: String = "", index: Int = 0, role: String = "", title: String = "", subtitle: String = "") {
        self.layer = layer; self.index = index; self.role = role; self.title = title; self.subtitle = subtitle
    }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        layer = (try? c.decodeIfPresent(String.self, forKey: .layer)) ?? ""
        index = (try? c.decodeIfPresent(Int.self, forKey: .index)) ?? 0
        role = (try? c.decodeIfPresent(String.self, forKey: .role)) ?? ""
        title = (try? c.decodeIfPresent(String.self, forKey: .title)) ?? ""
        subtitle = (try? c.decodeIfPresent(String.self, forKey: .subtitle)) ?? ""
    }
}

struct PlanItem: Codable, Equatable, Identifiable {
    var id: String = ""
    var kind: String = "corridor" // corridor | plane
    var name: String = ""
    var color: String = ""
    var state: String = "proposed"
    var nets: Int = 0
    var routed: Int = 0
    var total: Int = 0
    var width: Double? = nil
    var impedance: String = ""
    var reference: String = ""
    var locked: Bool = false
    var edited: Bool = false
    var problems: Int = 0
    var problemText: String = ""
    var layer: String = ""

    enum CodingKeys: String, CodingKey {
        case id, kind, name, color, state, nets, routed, total, width, impedance, reference, locked, edited, problems
        case problemText = "problem_text", layer
    }
    init(
        id: String = "", kind: String = "corridor", name: String = "", color: String = "",
        state: String = "proposed", nets: Int = 0, routed: Int = 0, total: Int = 0,
        width: Double? = nil, impedance: String = "", reference: String = "",
        locked: Bool = false, edited: Bool = false, problems: Int = 0, problemText: String = "", layer: String = ""
    ) {
        self.id = id; self.kind = kind; self.name = name; self.color = color
        self.state = state; self.nets = nets; self.routed = routed; self.total = total
        self.width = width; self.impedance = impedance; self.reference = reference
        self.locked = locked; self.edited = edited; self.problems = problems
        self.problemText = problemText; self.layer = layer
    }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        id = (try? c.decodeIfPresent(String.self, forKey: .id)) ?? ""
        kind = (try? c.decodeIfPresent(String.self, forKey: .kind)) ?? "corridor"
        name = (try? c.decodeIfPresent(String.self, forKey: .name)) ?? ""
        color = (try? c.decodeIfPresent(String.self, forKey: .color)) ?? ""
        state = (try? c.decodeIfPresent(String.self, forKey: .state)) ?? "proposed"
        nets = (try? c.decodeIfPresent(Int.self, forKey: .nets)) ?? 0
        routed = (try? c.decodeIfPresent(Int.self, forKey: .routed)) ?? 0
        total = (try? c.decodeIfPresent(Int.self, forKey: .total)) ?? 0
        width = try? c.decodeIfPresent(Double.self, forKey: .width)
        impedance = (try? c.decodeIfPresent(String.self, forKey: .impedance)) ?? ""
        reference = (try? c.decodeIfPresent(String.self, forKey: .reference)) ?? ""
        locked = (try? c.decodeIfPresent(Bool.self, forKey: .locked)) ?? false
        edited = (try? c.decodeIfPresent(Bool.self, forKey: .edited)) ?? false
        problems = (try? c.decodeIfPresent(Int.self, forKey: .problems)) ?? 0
        problemText = (try? c.decodeIfPresent(String.self, forKey: .problemText)) ?? ""
        layer = (try? c.decodeIfPresent(String.self, forKey: .layer)) ?? ""
    }
}

struct PlanLayer: Codable, Equatable, Identifiable {
    var layer: String = ""
    var index: Int = 0
    var title: String = ""
    var subtitle: String = ""
    var role: String = ""
    var status: String = ""
    var done: Int = 0
    var approved: Int = 0
    var total: Int = 0
    var stackup: String = ""
    var items: [PlanItem] = []
    var id: String { layer }

    enum CodingKeys: String, CodingKey {
        case layer, index, title, subtitle, role, status, done, approved, total, stackup, items
    }
    init(
        layer: String = "", index: Int = 0, title: String = "", subtitle: String = "",
        role: String = "", status: String = "", done: Int = 0, approved: Int = 0,
        total: Int = 0, stackup: String = "", items: [PlanItem] = []
    ) {
        self.layer = layer; self.index = index; self.title = title; self.subtitle = subtitle
        self.role = role; self.status = status; self.done = done; self.approved = approved
        self.total = total; self.stackup = stackup; self.items = items
    }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        layer = (try? c.decodeIfPresent(String.self, forKey: .layer)) ?? ""
        index = (try? c.decodeIfPresent(Int.self, forKey: .index)) ?? 0
        title = (try? c.decodeIfPresent(String.self, forKey: .title)) ?? ""
        subtitle = (try? c.decodeIfPresent(String.self, forKey: .subtitle)) ?? ""
        role = (try? c.decodeIfPresent(String.self, forKey: .role)) ?? ""
        status = (try? c.decodeIfPresent(String.self, forKey: .status)) ?? ""
        done = (try? c.decodeIfPresent(Int.self, forKey: .done)) ?? 0
        approved = (try? c.decodeIfPresent(Int.self, forKey: .approved)) ?? 0
        total = (try? c.decodeIfPresent(Int.self, forKey: .total)) ?? 0
        stackup = (try? c.decodeIfPresent(String.self, forKey: .stackup)) ?? ""
        items = (try? c.decodeIfPresent([PlanItem].self, forKey: .items)) ?? []
    }
}

struct PlanPartBox: Codable, Equatable {
    var r: String = ""
    var s: String = ""
    var b: [Double] = []
    var o: Bool = false

    enum CodingKeys: String, CodingKey { case r, s, b, o }
    init(r: String = "", s: String = "", b: [Double] = [], o: Bool = false) {
        self.r = r; self.s = s; self.b = b; self.o = o
    }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        r = (try? c.decodeIfPresent(String.self, forKey: .r)) ?? ""
        s = (try? c.decodeIfPresent(String.self, forKey: .s)) ?? ""
        b = (try? c.decodeIfPresent([Double].self, forKey: .b)) ?? []
        o = (try? c.decodeIfPresent(Bool.self, forKey: .o)) ?? false
    }
}

struct PlanBoardGeom: Codable, Equatable {
    var bbox: [Double] = [0, 0, 100, 70]
    var outline: [[[Double]]] = []
    var parts: [PlanPartBox] = []

    enum CodingKeys: String, CodingKey { case bbox, outline, parts }
    init(bbox: [Double] = [0, 0, 100, 70], outline: [[[Double]]] = [], parts: [PlanPartBox] = []) {
        self.bbox = bbox; self.outline = outline; self.parts = parts
    }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        let bb = (try? c.decodeIfPresent([Double].self, forKey: .bbox)) ?? []
        bbox = bb.count == 4 ? bb : [0, 0, 100, 70]
        outline = (try? c.decodeIfPresent([[[Double]]].self, forKey: .outline)) ?? []
        parts = (try? c.decodeIfPresent([PlanPartBox].self, forKey: .parts)) ?? []
    }
}

struct PlanBand: Codable, Equatable {
    var id: String = ""
    var layer: String = ""
    var name: String = ""
    var color: String = ""
    var state: String = "proposed"
    var width: Double = 1.5
    var nets: Int = 0
    var locked: Bool = false
    var edited: Bool = false
    var path: [[Double]] = []
    var vias: [[Double]] = []
    var guides: [[Double]] = []
    var from: [String] = []
    var to: [String] = []

    enum CodingKeys: String, CodingKey {
        case id, layer, name, color, state, width, nets, locked, edited, path, vias, guides, from, to
    }
    init(
        id: String = "", layer: String = "", name: String = "", color: String = "",
        state: String = "proposed", width: Double = 1.5, nets: Int = 0, locked: Bool = false,
        edited: Bool = false, path: [[Double]] = [], vias: [[Double]] = [], guides: [[Double]] = [],
        from: [String] = [], to: [String] = []
    ) {
        self.id = id; self.layer = layer; self.name = name; self.color = color
        self.state = state; self.width = width; self.nets = nets; self.locked = locked
        self.edited = edited; self.path = path; self.vias = vias; self.guides = guides
        self.from = from; self.to = to
    }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        id = (try? c.decodeIfPresent(String.self, forKey: .id)) ?? ""
        layer = (try? c.decodeIfPresent(String.self, forKey: .layer)) ?? ""
        name = (try? c.decodeIfPresent(String.self, forKey: .name)) ?? ""
        color = (try? c.decodeIfPresent(String.self, forKey: .color)) ?? ""
        state = (try? c.decodeIfPresent(String.self, forKey: .state)) ?? "proposed"
        width = (try? c.decodeIfPresent(Double.self, forKey: .width)) ?? 1.5
        nets = (try? c.decodeIfPresent(Int.self, forKey: .nets)) ?? 0
        locked = (try? c.decodeIfPresent(Bool.self, forKey: .locked)) ?? false
        edited = (try? c.decodeIfPresent(Bool.self, forKey: .edited)) ?? false
        path = (try? c.decodeIfPresent([[Double]].self, forKey: .path)) ?? []
        vias = (try? c.decodeIfPresent([[Double]].self, forKey: .vias)) ?? []
        guides = (try? c.decodeIfPresent([[Double]].self, forKey: .guides)) ?? []
        from = (try? c.decodeIfPresent([String].self, forKey: .from)) ?? []
        to = (try? c.decodeIfPresent([String].self, forKey: .to)) ?? []
    }
}

struct PlanRegion: Codable, Equatable {
    var id: String = ""
    var layer: String = ""
    var net: String = ""
    var color: String = ""
    var state: String = "proposed"
    var locked: Bool = false
    var edited: Bool = false
    var outline: [[Double]] = []
    var holes: [[[Double]]] = []

    enum CodingKeys: String, CodingKey {
        case id, layer, net, color, state, locked, edited, outline, holes
    }
    init(
        id: String = "", layer: String = "", net: String = "", color: String = "",
        state: String = "proposed", locked: Bool = false, edited: Bool = false,
        outline: [[Double]] = [], holes: [[[Double]]] = []
    ) {
        self.id = id; self.layer = layer; self.net = net; self.color = color
        self.state = state; self.locked = locked; self.edited = edited
        self.outline = outline; self.holes = holes
    }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        id = (try? c.decodeIfPresent(String.self, forKey: .id)) ?? ""
        layer = (try? c.decodeIfPresent(String.self, forKey: .layer)) ?? ""
        net = (try? c.decodeIfPresent(String.self, forKey: .net)) ?? ""
        color = (try? c.decodeIfPresent(String.self, forKey: .color)) ?? ""
        state = (try? c.decodeIfPresent(String.self, forKey: .state)) ?? "proposed"
        locked = (try? c.decodeIfPresent(Bool.self, forKey: .locked)) ?? false
        edited = (try? c.decodeIfPresent(Bool.self, forKey: .edited)) ?? false
        outline = (try? c.decodeIfPresent([[Double]].self, forKey: .outline)) ?? []
        holes = (try? c.decodeIfPresent([[[Double]]].self, forKey: .holes)) ?? []
    }
}

struct PlanDraw: Codable, Equatable {
    var bands: [PlanBand] = []
    var regions: [PlanRegion] = []

    enum CodingKeys: String, CodingKey { case bands, regions }
    init(bands: [PlanBand] = [], regions: [PlanRegion] = []) {
        self.bands = bands; self.regions = regions
    }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        bands = (try? c.decodeIfPresent([PlanBand].self, forKey: .bands)) ?? []
        regions = (try? c.decodeIfPresent([PlanRegion].self, forKey: .regions)) ?? []
    }
}

struct PlanAskNote: Codable, Equatable {
    var what: String = ""
    var note: String = ""
    enum CodingKeys: String, CodingKey { case what, note }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        what = (try? c.decodeIfPresent(String.self, forKey: .what)) ?? ""
        note = (try? c.decodeIfPresent(String.self, forKey: .note)) ?? ""
    }
}

struct PlanElsewhere: Codable, Equatable {
    var name: String = ""
    var pcb: String = ""
    var rel: String = ""
    enum CodingKeys: String, CodingKey { case name, pcb, rel }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        name = (try? c.decodeIfPresent(String.self, forKey: .name)) ?? ""
        pcb = (try? c.decodeIfPresent(String.self, forKey: .pcb)) ?? ""
        rel = (try? c.decodeIfPresent(String.self, forKey: .rel)) ?? ""
    }
}

struct PlanPanel: Codable, Equatable {
    var empty: Bool = false
    var on: Bool = false
    var live: Bool = false
    var pro: String = ""
    var boardPath: String = ""
    var boardRel: String = ""
    var routing: Bool = false
    var awaiting: Int = 0
    var approved: Int = 0
    var done: Int = 0
    var blocked: Int = 0
    var total: Int = 0
    var revision: Int = 0
    var status: String = ""
    var boardTrimmed: Bool = false
    var stackupSummary: String = ""
    var impedances: [String] = []
    var keysNote: String = ""
    var asks: [PlanAskNote] = []
    var elsewhere: [PlanElsewhere] = []
    var layers: [PlanLayer] = []
    var copperLayers: [PlanCopperLayer] = []
    var board: PlanBoardGeom? = nil
    var draw: PlanDraw? = nil

    enum CodingKeys: String, CodingKey {
        case empty, on, live, pro, routing, awaiting, approved, done, blocked, total, revision, status
        case boardPath = "board_path", boardRel = "board_rel", boardTrimmed = "board_trimmed"
        case stackupSummary = "stackup_summary", impedances, keysNote = "keys_note", asks, elsewhere, layers
        case copperLayers = "copper_layers", board, draw
    }
    private enum ExtraCodingKeys: String, CodingKey { case project }
    init(
        empty: Bool = false, on: Bool = false, live: Bool = false, pro: String = "",
        routing: Bool = false, awaiting: Int = 0, approved: Int = 0, done: Int = 0, blocked: Int = 0,
        total: Int = 0, revision: Int = 0, status: String = "", boardTrimmed: Bool = false,
        stackupSummary: String = "", impedances: [String] = [], layers: [PlanLayer] = [],
        copperLayers: [PlanCopperLayer] = [], board: PlanBoardGeom? = nil, draw: PlanDraw? = nil
    ) {
        self.empty = empty; self.on = on; self.live = live; self.pro = pro
        self.routing = routing; self.awaiting = awaiting; self.approved = approved
        self.done = done; self.blocked = blocked; self.total = total; self.revision = revision
        self.status = status; self.boardTrimmed = boardTrimmed
        self.stackupSummary = stackupSummary; self.impedances = impedances
        self.layers = layers; self.copperLayers = copperLayers; self.board = board; self.draw = draw
    }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        empty = (try? c.decodeIfPresent(Bool.self, forKey: .empty)) ?? false
        on = (try? c.decodeIfPresent(Bool.self, forKey: .on)) ?? false
        live = (try? c.decodeIfPresent(Bool.self, forKey: .live)) ?? false
        let rawPro = (try? c.decodeIfPresent(String.self, forKey: .pro)) ?? ""
        let ex = try? d.container(keyedBy: ExtraCodingKeys.self)
        let rawProject = (try? ex?.decodeIfPresent(String.self, forKey: .project)) ?? ""
        pro = !rawPro.isEmpty ? rawPro : rawProject
        boardPath = (try? c.decodeIfPresent(String.self, forKey: .boardPath)) ?? ""
        boardRel = (try? c.decodeIfPresent(String.self, forKey: .boardRel)) ?? ""
        routing = (try? c.decodeIfPresent(Bool.self, forKey: .routing)) ?? false
        awaiting = (try? c.decodeIfPresent(Int.self, forKey: .awaiting)) ?? 0
        approved = (try? c.decodeIfPresent(Int.self, forKey: .approved)) ?? 0
        done = (try? c.decodeIfPresent(Int.self, forKey: .done)) ?? 0
        blocked = (try? c.decodeIfPresent(Int.self, forKey: .blocked)) ?? 0
        total = (try? c.decodeIfPresent(Int.self, forKey: .total)) ?? 0
        revision = (try? c.decodeIfPresent(Int.self, forKey: .revision)) ?? 0
        status = (try? c.decodeIfPresent(String.self, forKey: .status)) ?? ""
        boardTrimmed = (try? c.decodeIfPresent(Bool.self, forKey: .boardTrimmed)) ?? false
        stackupSummary = (try? c.decodeIfPresent(String.self, forKey: .stackupSummary)) ?? ""
        impedances = (try? c.decodeIfPresent([String].self, forKey: .impedances)) ?? []
        keysNote = (try? c.decodeIfPresent(String.self, forKey: .keysNote)) ?? ""
        asks = (try? c.decodeIfPresent([PlanAskNote].self, forKey: .asks)) ?? []
        elsewhere = (try? c.decodeIfPresent([PlanElsewhere].self, forKey: .elsewhere)) ?? []
        layers = (try? c.decodeIfPresent([PlanLayer].self, forKey: .layers)) ?? []
        copperLayers = (try? c.decodeIfPresent([PlanCopperLayer].self, forKey: .copperLayers)) ?? []
        board = try? c.decodeIfPresent(PlanBoardGeom.self, forKey: .board)
        draw = try? c.decodeIfPresent(PlanDraw.self, forKey: .draw)
    }
}

struct PlanCamera: Equatable {
    var scale: Double
    var cx: Double
    var cy: Double

    static func fit(bbox: [Double], width: Double, height: Double, pad: Double = 22) -> PlanCamera {
        let minx = bbox.count == 4 ? bbox[0] : 0
        let miny = bbox.count == 4 ? bbox[1] : 0
        let maxx = bbox.count == 4 ? bbox[2] : 100
        let maxy = bbox.count == 4 ? bbox[3] : 70
        let spanX = max(1, maxx - minx)
        let spanY = max(1, maxy - miny)
        let aw = max(24, width - pad * 2)
        let ah = max(24, height - pad * 2)
        let scale = min(aw / spanX, ah / spanY)
        return PlanCamera(scale: scale, cx: (minx + maxx) * 0.5, cy: (miny + maxy) * 0.5)
    }

    func toPx(x: Double, y: Double, width: Double, height: Double) -> CGPoint {
        CGPoint(x: width * 0.5 + (x - cx) * scale, y: height * 0.5 + (y - cy) * scale)
    }

    func toWorld(px: Double, py: Double, width: Double, height: Double) -> [Double] {
        [cx + (px - width * 0.5) / max(scale, 0.01), cy + (py - height * 0.5) / max(scale, 0.01)]
    }

    func panned(dxPx: Double, dyPx: Double) -> PlanCamera {
        PlanCamera(scale: scale, cx: cx - dxPx / max(scale, 0.01), cy: cy - dyPx / max(scale, 0.01))
    }

    func zoomed(factor: Double, anchorX: Double, anchorY: Double, width: Double, height: Double) -> PlanCamera {
        let next = min(64, max(0.8, scale * factor))
        let ratio = scale / next
        return PlanCamera(
            scale: next,
            cx: cx + (anchorX - width * 0.5) / max(scale, 0.01) * (1 - ratio),
            cy: cy + (anchorY - height * 0.5) / max(scale, 0.01) * (1 - ratio)
        )
    }
}

enum PlanHit: Equatable {
    case vertex(id: String, isPlane: Bool, idx: Int)
    case midpoint(id: String, isPlane: Bool, segIdx: Int)
    case band(id: String)
    case region(id: String)

    var itemId: String {
        switch self {
        case .vertex(let id, _, _), .midpoint(let id, _, _), .band(let id), .region(let id): return id
        }
    }
}

struct PlanPlacedLabel: Equatable {
    var text: String
    var cx: Double
    var cy: Double
}

struct PlanCorridorPill: Equatable {
    var id: String
    var text: String
    var cx: Double
    var cy: Double
    var pw: Double
    var ph: Double
    var colorRGB: UInt32
    var selected: Bool

    var rect: CGRect {
        CGRect(x: cx - pw * 0.5, y: cy - ph * 0.5, width: pw, height: ph)
    }
}

struct PlanOptimisticPatch: Equatable {
    var path: [[Double]]? = nil
    var outline: [[Double]]? = nil
    var width: Double? = nil
    var layer: String? = nil
    var locked: Bool? = nil
}

struct PlanBoardRef: Equatable {
    var value: String = ""
    var pcb: String = ""
    var sch: String = ""
    var project: String = ""
}

struct PlanUndoEntry: Equatable {
    var pro: String
    var id: String
    var patch: PlanOptimisticPatch
    var onlyGeom: String? = nil
}

private struct PlanPanelEnvelope: Decodable {
    var pro: String?
    var project: String?
    var panel: PlanPanel?
    var result: PlanPanelInner?
}
private struct PlanPanelInner: Decodable {
    var pro: String?
    var project: String?
    var panel: PlanPanel?
}
private struct PlanBadgeEnvelope: Decodable {
    var badge: Badge?
    struct Badge: Decodable {
        var n: Int?
        var tone: String?
    }
}

enum PlanFloorplannerLogic {
    private static let railAmber: UInt32 = 0xF59E0B
    private static let railViolet: UInt32 = 0xC084FC
    private static let railCoral: UInt32 = 0xFB7185
    private static let railMint: UInt32 = 0x34D399
    private static let railSlate: UInt32 = 0x94A3B8
    private static let railCyan: UInt32 = 0x38BDF8
    private static let railGold: UInt32 = 0xFACC15
    private static let busTeal: UInt32 = 0x14B8A6
    private static let busSky: UInt32 = 0x38BDF8
    private static let busAmber: UInt32 = 0xF59E0B
    private static let busViolet: UInt32 = 0xA78BFA
    private static let busRose: UInt32 = 0xF472B6
    private static let busLime: UInt32 = 0x4ADE80
    private static let busOrange: UInt32 = 0xFB923C
    private static let busYellow: UInt32 = 0xEAB308

    static func unwrapPanel(from data: Data) -> PlanPanel? {
        if let env = try? JSONDecoder().decode(PlanPanelEnvelope.self, from: data) {
            if var p = env.panel {
                if p.pro.isEmpty {
                    p.pro = [env.pro, env.project].compactMap { $0 }.first(where: { !$0.isEmpty }) ?? ""
                }
                return p
            }
            if var p = env.result?.panel {
                if p.pro.isEmpty {
                    p.pro = [env.result?.pro, env.result?.project, env.pro, env.project].compactMap { $0 }.first(where: { !$0.isEmpty }) ?? ""
                }
                return p
            }
        }
        if let direct = try? JSONDecoder().decode(PlanPanel.self, from: data) {
            return direct
        }
        return nil
    }

    static func decodeBadge(from data: Data) -> (n: Int, tone: String) {
        guard let env = try? JSONDecoder().decode(PlanBadgeEnvelope.self, from: data),
              let b = env.badge else {
            return (0, "warn")
        }
        let tone = (b.tone ?? "").isEmpty ? "warn" : (b.tone ?? "warn")
        return (b.n ?? 0, tone)
    }

    private static func normPath(_ raw: String, root: String) -> String {
        var p = raw.trimmingCharacters(in: .whitespacesAndNewlines).replacingOccurrences(of: "\\", with: "/")
        while p.count > 1 && p.hasSuffix("/") { p.removeLast() }
        guard !p.isEmpty else { return "" }
        if p.hasPrefix("/") { return p }
        var r = root.trimmingCharacters(in: .whitespacesAndNewlines).replacingOccurrences(of: "\\", with: "/")
        while r.count > 1 && r.hasSuffix("/") { r.removeLast() }
        return r.isEmpty ? p : "\(r)/\(p)"
    }

    private static func parentDir(_ path: String) -> String {
        guard let idx = path.lastIndex(of: "/") else { return "" }
        return idx == path.startIndex ? "/" : String(path[..<idx])
    }

    static func resolveMgmtProject(
        targetPath: String?,
        selectedBoard: PlanBoardRef?,
        boards: [PlanBoardRef],
        root: String
    ) -> String {
        let absTarget = normPath(targetPath ?? "", root: root)
        if !absTarget.isEmpty {
            let boardId: (PlanBoardRef) -> String = { b in
                for raw in [b.project, b.pcb, b.value] {
                    let p = normPath(raw, root: root)
                    if !p.isEmpty { return p }
                }
                return absTarget
            }
            for b in boards {
                for raw in [b.pcb, b.sch, b.value, b.project] {
                    let cand = normPath(raw, root: root)
                    if !cand.isEmpty && cand == absTarget {
                        return boardId(b)
                    }
                }
            }
            let tDir = parentDir(absTarget)
            if !tDir.isEmpty {
                var best: (len: Int, id: String)? = nil
                for b in boards {
                    for raw in [b.project, b.pcb, b.sch, b.value] {
                        let cand = normPath(raw, root: root)
                        guard !cand.isEmpty else { continue }
                        let bDir = parentDir(cand)
                        guard !bDir.isEmpty else { continue }
                        if tDir == bDir || tDir.hasPrefix(bDir + "/") {
                            if best == nil || bDir.count > best!.len {
                                best = (bDir.count, boardId(b))
                            }
                        }
                    }
                }
                if let best { return best.id }
            }
            return absTarget
        }
        if let sb = selectedBoard {
            let p = normPath(sb.project, root: root)
            if !p.isEmpty { return p }
            let pcb = normPath(sb.pcb, root: root)
            if !pcb.isEmpty { return pcb }
        }
        return normPath(root, root: "")
    }

    static func hasUndo(stack: [PlanUndoEntry], pro: String, id: String) -> Bool {
        !id.isEmpty && stack.contains(where: { $0.pro == pro && $0.id == id })
    }

    static func popUndo(stack: inout [PlanUndoEntry], pro: String, id: String) -> PlanUndoEntry? {
        guard !id.isEmpty, let idx = stack.lastIndex(where: { $0.pro == pro && $0.id == id }) else {
            return nil
        }
        return stack.remove(at: idx)
    }

    static func mergePatch(_ base: PlanOptimisticPatch?, over: PlanOptimisticPatch) -> PlanOptimisticPatch {
        var cur = base ?? PlanOptimisticPatch()
        if let p = over.path { cur.path = p }
        if let o = over.outline { cur.outline = o }
        if let w = over.width { cur.width = w }
        if let l = over.layer { cur.layer = l }
        if let lk = over.locked { cur.locked = lk }
        return cur
    }

    static func foldPatches(
        _ entries: [(id: String, patch: PlanOptimisticPatch)],
        preview: (id: String, patch: PlanOptimisticPatch)? = nil
    ) -> [String: PlanOptimisticPatch] {
        var out: [String: PlanOptimisticPatch] = [:]
        for (id, patch) in entries {
            out[id] = mergePatch(out[id], over: patch)
        }
        if let preview {
            out[preview.id] = mergePatch(out[preview.id], over: preview.patch)
        }
        return out
    }

    static func parseHexRGB(_ hex: String, fallback: UInt32) -> UInt32 {
        let clean = hex.trimmingCharacters(in: .whitespacesAndNewlines).replacingOccurrences(of: "#", with: "")
        guard clean.count == 6, let v = UInt32(clean, radix: 16) else { return fallback }
        return v
    }

    private static func stableHash(_ s: String) -> UInt64 {
        var h: UInt64 = 0x811C9DC5
        for b in s.utf8 {
            h ^= UInt64(b)
            h = h &* 0x01000193
        }
        return h
    }

    static func railColorRGB(_ net: String, fallback: String = "") -> UInt32 {
        let up = net.uppercased()
        if up.contains("3V3") || up.contains("3.3") { return railAmber }
        if up.contains("1V8") || up.contains("1.8") || up.contains("1V2") || up.contains("0V") { return railViolet }
        if up.contains("VBUS") || up.contains("5V") { return railCoral }
        if up.contains("VBAT") || up.contains("VIN") || up.contains("12V") || up.contains("24V") { return railGold }
        if up.contains("GND") { return railSlate }
        let fb = fallback.lowercased()
        if !fb.isEmpty && fb != "#5a7a9a" && fb != "#4c6a88" && fb != "#52667a" {
            return parseHexRGB(fallback, fallback: railAmber)
        }
        let pal = [railAmber, railViolet, railCoral, railMint, railCyan, railGold, railSlate]
        return pal[Int(stableHash(net) % UInt64(pal.count))]
    }

    static func busColorRGB(_ name: String, fallback: String = "") -> UInt32 {
        let fb = fallback.trimmingCharacters(in: .whitespacesAndNewlines)
        if !fb.isEmpty { return parseHexRGB(fb, fallback: busTeal) }
        let pal = [busTeal, busSky, busAmber, busViolet, busRose, busLime, busOrange, busYellow]
        return pal[Int(stableHash(name) % UInt64(pal.count))]
    }

    static func allCanvasLayers(planned: [PlanLayer], copper: [PlanCopperLayer]) -> [PlanLayer] {
        guard !copper.isEmpty else { return planned }
        let byName = Dictionary(planned.map { ($0.layer, $0) }, uniquingKeysWith: { a, _ in a })
        var out: [PlanLayer] = []
        var seen = Set<String>()
        for cl in copper where !cl.layer.isEmpty {
            seen.insert(cl.layer)
            if var ex = byName[cl.layer] {
                if ex.title.isEmpty { ex.title = cl.title }
                if ex.subtitle.isEmpty { ex.subtitle = cl.subtitle }
                if ex.role.isEmpty { ex.role = cl.role }
                out.append(ex)
            } else {
                out.append(PlanLayer(
                    layer: cl.layer,
                    index: cl.index,
                    title: cl.title.isEmpty ? "L\(cl.index) · \(cl.layer)" : cl.title,
                    subtitle: cl.subtitle,
                    role: cl.role,
                    status: "proposed"
                ))
            }
        }
        for l in planned where !l.layer.isEmpty && !seen.contains(l.layer) {
            out.append(l)
        }
        return out
    }

    static func defaultActiveLayer(layers: [PlanLayer]) -> String {
        var bestSig: (Int, String)?
        var bestPwr: (Int, String)?
        for l in layers where !l.layer.isEmpty {
            let corN = l.items.filter { $0.kind == "corridor" }.count
            let plnN = l.items.filter { $0.kind == "plane" }.count
            if corN > 0 || l.role == "signal" {
                if bestSig == nil || corN > bestSig!.0 { bestSig = (corN, l.layer) }
            }
            if plnN > 0 || l.role == "power" || l.role == "plane" {
                if bestPwr == nil || plnN > bestPwr!.0 { bestPwr = (plnN, l.layer) }
            }
        }
        return bestSig?.1 ?? bestPwr?.1 ?? layers.first?.layer ?? ""
    }

    static func layerDisplayTitle(_ layer: String, copper: [PlanCopperLayer], allLayers: [PlanLayer]) -> String {
        for c in copper where c.layer == layer && !c.title.isEmpty { return c.title }
        for l in allLayers where l.layer == layer && !l.title.isEmpty { return l.title }
        return layer
    }

    static func bandStrokePx(widthMm: Double, scale: Double) -> Double {
        min(42, max(6, widthMm * scale))
    }

    static func polylinePointAt(pts: [[Double]], frac: Double) -> [Double]? {
        guard !pts.isEmpty else { return nil }
        if pts.count == 1 { return pts[0] }
        let f = min(1, max(0, frac))
        var total = 0.0
        for i in 1..<pts.count {
            total += hypot(pts[i][0] - pts[i - 1][0], pts[i][1] - pts[i - 1][1])
        }
        if total < 1e-6 { return pts[0] }
        let target = total * f
        var acc = 0.0
        for i in 1..<pts.count {
            let dx = pts[i][0] - pts[i - 1][0]
            let dy = pts[i][1] - pts[i - 1][1]
            let len = hypot(dx, dy)
            if acc + len >= target && len > 1e-9 {
                let t = (target - acc) / len
                return [pts[i - 1][0] + dx * t, pts[i - 1][1] + dy * t]
            }
            acc += len
        }
        return pts.last
    }

    static func polylineMidpoint(pts: [[Double]]) -> [Double]? {
        polylinePointAt(pts: pts, frac: 0.5)
    }

    static func insertVertex(pts: [[Double]], segIdx: Int, pos: [Double], closed: Bool) -> [[Double]] {
        var out = pts
        if out.isEmpty { return [pos] }
        let at = (closed && segIdx + 1 >= out.count) ? out.count : min(segIdx + 1, out.count)
        out.insert(pos, at: at)
        return out
    }

    static func removeVertex(pts: [[Double]], idx: Int, closed: Bool) -> [[Double]]? {
        let minCount = closed ? 3 : 2
        guard pts.count > minCount, idx >= 0, idx < pts.count else { return nil }
        var out = pts
        out.remove(at: idx)
        return out
    }

    static func pointInRing(x: Double, y: Double, ring: [[Double]]) -> Bool {
        guard ring.count >= 3 else { return false }
        var inside = false
        var j = ring.count - 1
        for i in 0..<ring.count {
            let xi = ring[i][0], yi = ring[i][1]
            let xj = ring[j][0], yj = ring[j][1]
            if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / ((yj - yi) + 1e-12) + xi {
                inside.toggle()
            }
            j = i
        }
        return inside
    }

    static func pourLabelPoint(outer: [[Double]], holes: [[[Double]]]) -> [Double]? {
        guard outer.count >= 3 else { return nil }
        var minx = Double.infinity, miny = Double.infinity, maxx = -Double.infinity, maxy = -Double.infinity
        var sx = 0.0, sy = 0.0
        for p in outer {
            minx = min(minx, p[0]); miny = min(miny, p[1])
            maxx = max(maxx, p[0]); maxy = max(maxy, p[1])
            sx += p[0]; sy += p[1]
        }
        var cands: [[Double]] = [[(minx + maxx) * 0.5, (miny + maxy) * 0.5], [sx / Double(outer.count), sy / Double(outer.count)]]
        for fy in [0.5, 0.25, 0.75, 0.125, 0.875] {
            for fx in [0.5, 0.25, 0.75, 0.125, 0.875] {
                cands.append([minx + (maxx - minx) * fx, miny + (maxy - miny) * fy])
            }
        }
        for c in cands {
            if pointInRing(x: c[0], y: c[1], ring: outer) && !holes.contains(where: { pointInRing(x: c[0], y: c[1], ring: $0) }) {
                return c
            }
        }
        return nil
    }

    private static func distToSegmentSq(px: Double, py: Double, ax: Double, ay: Double, bx: Double, by: Double) -> Double {
        let vx = bx - ax, vy = by - ay
        let wx = px - ax, wy = py - ay
        let c1 = vx * wx + vy * wy
        if c1 <= 0 { return wx * wx + wy * wy }
        let c2 = vx * vx + vy * vy
        if c2 <= c1 { return (px - bx) * (px - bx) + (py - by) * (py - by) }
        let t = c1 / c2
        let qx = ax + vx * t, qy = ay + vy * t
        return (px - qx) * (px - qx) + (py - qy) * (py - qy)
    }

    static func hitTest(
        bands: [PlanBand],
        regions: [PlanRegion],
        layer: String,
        selectedId: String,
        cam: PlanCamera,
        width: Double,
        height: Double,
        x: Double,
        y: Double,
        vertexSlop: Double = 22,
        midpointSlop: Double = 18
    ) -> PlanHit? {
        if !selectedId.isEmpty {
            if let b = bands.first(where: { $0.id == selectedId && $0.layer == layer && !$0.locked }) {
                for (i, p) in b.path.enumerated() {
                    let pt = cam.toPx(x: p[0], y: p[1], width: width, height: height)
                    if hypot(Double(pt.x) - x, Double(pt.y) - y) <= vertexSlop {
                        return .vertex(id: b.id, isPlane: false, idx: i)
                    }
                }
                if b.path.count >= 2 {
                    for i in 0..<(b.path.count - 1) {
                        let pt = cam.toPx(x: (b.path[i][0] + b.path[i + 1][0]) * 0.5, y: (b.path[i][1] + b.path[i + 1][1]) * 0.5, width: width, height: height)
                        if hypot(Double(pt.x) - x, Double(pt.y) - y) <= midpointSlop {
                            return .midpoint(id: b.id, isPlane: false, segIdx: i)
                        }
                    }
                }
            }
            if let r = regions.first(where: { $0.id == selectedId && $0.layer == layer && !$0.locked }) {
                for (i, p) in r.outline.enumerated() {
                    let pt = cam.toPx(x: p[0], y: p[1], width: width, height: height)
                    if hypot(Double(pt.x) - x, Double(pt.y) - y) <= vertexSlop {
                        return .vertex(id: r.id, isPlane: true, idx: i)
                    }
                }
                if r.outline.count >= 3 {
                    for i in 0..<r.outline.count {
                        let next = r.outline[(i + 1) % r.outline.count]
                        let pt = cam.toPx(x: (r.outline[i][0] + next[0]) * 0.5, y: (r.outline[i][1] + next[1]) * 0.5, width: width, height: height)
                        if hypot(Double(pt.x) - x, Double(pt.y) - y) <= midpointSlop {
                            return .midpoint(id: r.id, isPlane: true, segIdx: i)
                        }
                    }
                }
            }
        }

        var bestBand: (dist: Double, id: String)?
        for b in bands where b.layer == layer && b.path.count >= 2 {
            let thresh = max(14, bandStrokePx(widthMm: b.width, scale: cam.scale) * 0.5 + 6)
            var minD2 = Double.infinity
            for i in 0..<(b.path.count - 1) {
                let p0 = cam.toPx(x: b.path[i][0], y: b.path[i][1], width: width, height: height)
                let p1 = cam.toPx(x: b.path[i + 1][0], y: b.path[i + 1][1], width: width, height: height)
                minD2 = min(minD2, distToSegmentSq(px: x, py: y, ax: Double(p0.x), ay: Double(p0.y), bx: Double(p1.x), by: Double(p1.y)))
            }
            let d = sqrt(minD2)
            if d <= thresh && (bestBand == nil || d <= bestBand!.dist) {
                bestBand = (d, b.id)
            }
        }
        if let bestBand { return .band(id: bestBand.id) }

        let wpt = cam.toWorld(px: x, py: y, width: width, height: height)
        for r in regions.reversed() where r.layer == layer {
            guard pointInRing(x: wpt[0], y: wpt[1], ring: r.outline) else { continue }
            if !r.holes.contains(where: { pointInRing(x: wpt[0], y: wpt[1], ring: $0) }) {
                return .region(id: r.id)
            }
        }
        return nil
    }

    static func placeCorridorPills(
        bands: [PlanBand],
        cam: PlanCamera,
        width: Double,
        height: Double,
        selectedId: String
    ) -> [PlanCorridorPill] {
        struct Cand {
            let b: PlanBand
            let pxLen: Double
            let sel: Bool
            let color: UInt32
        }
        let cands = bands.compactMap { b -> Cand? in
            guard b.nets > 0, b.path.count >= 2 else { return nil }
            var pxLen = 0.0
            for i in 1..<b.path.count {
                let a = cam.toPx(x: b.path[i - 1][0], y: b.path[i - 1][1], width: width, height: height)
                let c = cam.toPx(x: b.path[i][0], y: b.path[i][1], width: width, height: height)
                pxLen += hypot(Double(c.x - a.x), Double(c.y - a.y))
            }
            return Cand(b: b, pxLen: pxLen, sel: selectedId == b.id, color: busColorRGB(b.name.isEmpty ? b.id : b.name, fallback: b.color))
        }.sorted { lhs, rhs in
            if lhs.sel != rhs.sel { return lhs.sel && !rhs.sel }
            if lhs.b.nets != rhs.b.nets { return lhs.b.nets > rhs.b.nets }
            return lhs.b.id < rhs.b.id
        }

        var placed: [CGRect] = []
        var out: [PlanCorridorPill] = []
        let clampX = { (cx: Double, pw: Double) in
            let lo = pw * 0.5 + 3
            return min(max(lo, width - pw * 0.5 - 3), max(lo, cx))
        }
        let clampY = { (cy: Double, ph: Double) in
            let lo = ph * 0.5 + 3
            return min(max(lo, height - ph * 0.5 - 3), max(lo, cy))
        }
        for c in cands {
            let shortTxt = "\(c.b.nets)"
            let fullTxt = c.b.name.isEmpty ? shortTxt : "\(c.b.name) · \(c.b.nets)"
            let texts = (c.sel || c.pxLen >= 95) && !c.b.name.isEmpty ? [fullTxt, shortTxt] : [shortTxt]
            var chosen: PlanCorridorPill?
            outerLoop: for txt in texts {
                let pw = max(16, Double(txt.count) * 5.9 + 10)
                let ph = 15.0
                for frac in [0.5, 0.35, 0.65, 0.22, 0.78] {
                    guard let wp = polylinePointAt(pts: c.b.path, frac: frac) else { continue }
                    let px = cam.toPx(x: wp[0], y: wp[1], width: width, height: height)
                    let cx = clampX(Double(px.x), pw)
                    let cy = clampY(Double(px.y), ph)
                    let rect = CGRect(x: cx - pw * 0.5 - 2, y: cy - ph * 0.5 - 2, width: pw + 4, height: ph + 4)
                    if !placed.contains(where: { $0.intersects(rect) }) {
                        chosen = PlanCorridorPill(id: c.b.id, text: txt, cx: cx, cy: cy, pw: pw, ph: ph, colorRGB: c.color, selected: c.sel)
                        break outerLoop
                    }
                }
            }
            if chosen == nil, let wp = polylinePointAt(pts: c.b.path, frac: 0.5) {
                let px = cam.toPx(x: wp[0], y: wp[1], width: width, height: height)
                let pw = max(16, Double(shortTxt.count) * 6.5 + 8)
                let ph = 15.0
                let cx = clampX(Double(px.x), pw)
                for dy in [-(ph + 3), ph + 3] {
                    let cy = clampY(Double(px.y) + dy, ph)
                    let rect = CGRect(x: cx - pw * 0.5 - 2, y: cy - ph * 0.5 - 2, width: pw + 4, height: ph + 4)
                    if !placed.contains(where: { $0.intersects(rect) }) {
                        chosen = PlanCorridorPill(id: c.b.id, text: shortTxt, cx: cx, cy: cy, pw: pw, ph: ph, colorRGB: c.color, selected: c.sel)
                        break
                    }
                }
            }
            if let chosen {
                placed.append(chosen.rect.insetBy(dx: -2, dy: -2))
                out.append(chosen)
            }
        }
        return out
    }

    static func layoutLabels(
        parts: [PlanPartBox],
        cam: PlanCamera,
        width: Double,
        height: Double,
        pinned: [String],
        blocked: [CGRect] = []
    ) -> [PlanPlacedLabel] {
        let pinSet = Set(pinned)
        struct Cand { let text: String; let cx, cy, bw, bh, score: Double }
        var cands: [Cand] = []
        for p in parts where !p.r.isEmpty && p.b.count == 4 {
            let p0 = cam.toPx(x: p.b[0], y: p.b[1], width: width, height: height)
            let p1 = cam.toPx(x: p.b[2], y: p.b[3], width: width, height: height)
            let pw = abs(Double(p1.x - p0.x))
            let ph = abs(Double(p1.y - p0.y))
            guard pw >= 13 && ph >= 9 else { continue }
            let cx = Double(p0.x + p1.x) * 0.5
            let cy = Double(p0.y + p1.y) * 0.5
            guard cx >= 8 && cx <= width - 8 && cy >= 8 && cy <= height - 8 else { continue }
            cands.append(Cand(
                text: p.r, cx: cx, cy: cy, bw: Double(p.r.count) * 5.8 + 4, bh: 11,
                score: pw * ph + (pinSet.contains(p.r) ? 10000 : 0)
            ))
        }
        cands.sort { $0.score > $1.score }
        var placed = blocked
        var out: [PlanPlacedLabel] = []
        for c in cands {
            let rect = CGRect(x: c.cx - c.bw * 0.5, y: c.cy - c.bh * 0.5, width: c.bw, height: c.bh)
            if !placed.contains(where: { $0.intersects(rect) }) {
                placed.append(rect)
                out.append(PlanPlacedLabel(text: c.text, cx: c.cx, cy: c.cy))
            }
        }
        return out
    }

    static func mergeOptimistic(panel: PlanPanel, patches: [String: PlanOptimisticPatch]) -> PlanPanel {
        guard !patches.isEmpty else { return panel }
        var out = panel
        var draw = panel.draw ?? PlanDraw()
        draw.bands = draw.bands.map { b in
            guard let patch = patches[b.id] else { return b }
            var next = b
            if let p = patch.path { next.path = p }
            if let w = patch.width { next.width = w }
            if let l = patch.layer { next.layer = l }
            if let lk = patch.locked { next.locked = lk }
            next.edited = true
            if next.state == "proposed" { next.state = "edited" }
            return next
        }
        draw.regions = draw.regions.map { r in
            guard let patch = patches[r.id] else { return r }
            var next = r
            if let o = patch.outline { next.outline = o }
            if let l = patch.layer { next.layer = l }
            if let lk = patch.locked { next.locked = lk }
            next.edited = true
            if next.state == "proposed" { next.state = "edited" }
            return next
        }
        out.draw = draw

        var moved: [(String, PlanItem)] = []
        out.layers = panel.layers.map { l in
            var kept: [PlanItem] = []
            for var it in l.items {
                guard let patch = patches[it.id] else {
                    kept.append(it)
                    continue
                }
                if let w = patch.width { it.width = w }
                if let lk = patch.locked { it.locked = lk }
                it.edited = true
                if it.state == "proposed" { it.state = "edited" }
                if let target = patch.layer, target != l.layer {
                    it.layer = target
                    moved.append((target, it))
                } else {
                    kept.append(it)
                }
            }
            var nl = l
            nl.items = kept
            return nl
        }
        for (target, item) in moved {
            if let idx = out.layers.firstIndex(where: { $0.layer == target }) {
                out.layers[idx].items.append(item)
            }
        }
        return out
    }
}
