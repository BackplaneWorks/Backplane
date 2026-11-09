import SwiftUI
import PDFKit

// The viewer over a thread on the phone (the same tabs as the desktop's right rail and the Android app's DataViewer.kt): Board, Schematic, 3D, Mech, Files, Browser,
// on the data/1 viewer requests (native/host/src/data/ext.rs). Plots and models arrive as the hub's own binary frames (DataClient routes them to `plots`)
// and are drawn by the app's Metal view (PlotCanvasView: pinch, pan, orbit, tap to inspect). Written without a simulator run: compile-checked only.

/// `file` and `line`: a link to a file (src/x.rs:12) opens the viewer at it
struct DViewerRoute: Hashable { var id: String; var file = ""; var line = 0 }
struct DEntryRow: Decodable, Identifiable { var dir: Bool; var size: Int; var name: String; var id: String { name }
    enum CodingKeys: String, CodingKey { case dir, size, name }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); dir = c.bool(.dir); size = c.int(.size); name = c.str(.name) } }
struct DEntries: Decodable { var entries: [DEntryRow]
    enum CodingKeys: String, CodingKey { case entries }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); entries = (try? c.decode([DEntryRow].self, forKey: .entries)) ?? [] } }
struct DProjectFiles: Decodable { var root: String; var pcb: String; var sch: String
    enum CodingKeys: String, CodingKey { case root, pcb, sch }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); root = c.str(.root); pcb = c.str(.pcb); sch = c.str(.sch) } }
struct DMech: Decodable { var parts: [String]
    enum CodingKeys: String, CodingKey { case parts }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); parts = (try? c.decode([String].self, forKey: .parts)) ?? [] } }
/// `viewer.layers`: what a press does, and what to draw; the rules are the hub's (present/viewer.rs `layer_reply`).
struct DLayerRow: Decodable, Hashable { var layer: Int; var name: String; var on: Bool; var active: Bool }
struct DLayers: Decodable, Equatable {
    var own: UInt32; var focus: String; var off: UInt32; var dim: UInt32; var lift: Int; var layerBits: UInt32
    var layerHead: String; var layerMode: Int; var layerModes: Bool; var layerReset: Bool; var layerList: [DLayerRow]; var layerMarks: [DLayerRow]
}
/// `viewer.watch`: the thread's folder and the keys of the plots the view shows (an older hub names none)
struct DWatch: Decodable { var root: String; var keys: [String]
    enum CodingKeys: String, CodingKey { case root, keys }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); root = c.str(.root); keys = (try? c.decode([String].self, forKey: .keys)) ?? [] } }
struct DRef: Decodable, Hashable { var kind: String; var label: String; var commit: String }
struct DRefs: Decodable { var refs: [DRef] }
struct DInfo: Decodable { var info: String
    enum CodingKeys: String, CodingKey { case info }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); info = c.str(.info) } }

extension DataClient {
    func viewerFiles(_ thread: String) async -> DProjectFiles? { try? await call("viewer.files", ["thread": thread]) }
    /// A viewer (tab, file, thread) opens: the last one's plots go at once and late ones for it are dropped (PlotStore.begin/shown);
    /// a 3D view shows its own cached model once the hub names that model's file among the view's.
    func watch(_ thread: String, kind: String, path: String = "") async throws {
        var p: [String: Any] = ["thread": thread, "kind": kind]; if !path.isEmpty { p["path"] = path }
        let slot = kind.hasPrefix("3d") ? [hubName, thread, kind, path].joined(separator: "|") : ""
        // (a stop changes nothing here: it may come after the next viewer has begun, and that one drops the last one's plots itself)
        var serial = 0
        if !kind.isEmpty { let b = plots.begin(slot); serial = b.serial; if !b.kept.isEmpty { p["kept"] = b.kept } }
        do {
            let r: DWatch = try await call("viewer.watch", p)
            if !kind.isEmpty { plots.shown(r.keys.isEmpty ? PlotStore.keys(kind: kind, root: r.root, path: path) : r.keys, of: serial) }
        } catch { if !kind.isEmpty { plots.failed(of: serial) }; throw error }
    }
    func layers(_ kind: String, present: UInt32, own: UInt32, focus: String, action: String = "", value: String = "") async -> DLayers? {
        try? await call("viewer.layers", ["kind": kind, "present": present, "off": own, "focus": focus, "action": action, "value": value])
    }
    func refs(_ thread: String) async -> [DRef] { ((try? await call("viewer.refs", ["thread": thread]) as DRefs)?.refs) ?? [] }
    func compare(_ thread: String, kind: String, commit: String, bits: UInt32, path: String) async throws {
        _ = try await call("viewer.compare", ["thread": thread, "kind": kind, "commit": commit, "bits": bits, "path": path], as: DEmpty.self)
    }
    func info(_ key: String, at: Int, idx: Int) async -> String { ((try? await call("viewer.info", ["key": key, "at": at, "idx": idx]) as DInfo)?.info) ?? "" }
    func entries(_ thread: String, path: String) async throws -> [DEntryRow] { (try await call("files.entries", ["thread": thread, "path": path]) as DEntries).entries }
    func mech(_ thread: String) async -> [String] { ((try? await call("mech.list", ["thread": thread]) as DMech)?.parts) ?? [] }
    func browser(_ thread: String, _ op: String, _ extra: [String: Any] = [:]) async throws {
        var p = extra; p["thread"] = thread; p["op"] = op
        _ = try await call("browser", p, as: DEmpty.self)
    }
    func fetch(_ path: String, query: [URLQueryItem] = []) async -> Data? {
        guard let link, let u = Pairing.http(link, path: path, query: query), let (d, r) = try? await URLSession.shared.data(from: u), (r as? HTTPURLResponse)?.statusCode == 200 else { return nil }
        return d
    }
    func fileBytes(_ thread: String, _ path: String) async -> Data? { await fetch("/files/get", query: [URLQueryItem(name: "thread", value: thread), URLQueryItem(name: "path", value: path)]) }
    func screenBytes(_ thread: String) async -> Data? { await fetch("/bots/\(thread)/screen.jpg") }
}

private let tabs = ["Board", "Schematic", "3D", "Split", "Mech", "Files", "Diff", "Review", "Browser"]

/// The Viewer the Metal view wants, with the defaults the hub's own view would send; only what the data app decides (layers, the picked piece) varies.
private func plainViewer(open: String, source: String, picked: String, off: UInt32, dim: UInt32, lift: Int) -> Viewer? {
    // strings are encoded, never pasted in: a quote in one (a pick's candidates were JSON) made the whole Viewer nil and the canvas vanished
    func q(_ s: String) -> String { (try? JSONSerialization.data(withJSONObject: [s])).flatMap { String(data: $0, encoding: .utf8) }.map { String($0.dropFirst().dropLast()) } ?? "\"\"" }
    let j = """
    {"open":\(q(open)),"key":"","layers":\(q(source)),"choices":[],"bg":852754,"fade":220,"margin":0.9,"zmin":0.5,"zmax":0.5,"tap":14,"top":[],"bottom":[],"slab":2060099,"fov":35,
     "picked":\(q(picked)),"off":\(off),"dim":\(dim),"lift":\(lift)}
    """
    return try? JSONDecoder().decode(Viewer.self, from: Data(j.utf8))
}

struct DataViewerView: View {
    let client: DataClient
    let thread: DThread
    var openFile = ""
    var line = 0
    @State private var tab: String
    @State private var target: (tab: String, path: String)?
    @State private var files: DProjectFiles?
    init(client: DataClient, thread: DThread, openFile: String = "", line: Int = 0) {
        self.client = client; self.thread = thread; self.openFile = openFile; self.line = line
        // a linked file opens where it belongs: a board or schematic in its tab, a model in Mech, anything else previewed in Files
        let e = ext(openFile)
        _tab = State(initialValue: openFile.isEmpty ? "Files" : e == "kicad_pcb" ? "Board" : e == "kicad_sch" ? "Schematic" : ["step", "stp", "glb"].contains(e) ? "Mech" : "Files")
        _target = State(initialValue: e == "kicad_pcb" ? ("Board", openFile) : e == "kicad_sch" ? ("Schematic", openFile) : ["step", "stp", "glb"].contains(e) ? ("Mech", openFile) : nil)
    }
    var body: some View {
        VStack(spacing: 0) {
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 0) {
                    ForEach(tabs, id: \.self) { t in
                        Button { tab = t; target = nil } label: { Text(Tr.t(t)).dmono(12, bold: t == tab).foregroundStyle(t == tab ? BackplaneDesign.onAccent : BackplaneDesign.text).padding(.horizontal, 14).padding(.vertical, 10).background(t == tab ? BackplaneDesign.accent : BackplaneDesign.panel) }
                            .accessibilityIdentifier("data.vtab." + t)
                    }
                }
            }.background(BackplaneDesign.panel)
            DLine()
            Group {
                switch tab {
                case "Board": DataPlotTab(client: client, thread: thread, kind: "board", path: target?.tab == "Board" ? target!.path : "", has: files == nil || !(files?.pcb ?? "").isEmpty || target != nil, none: "No board in this project yet")
                case "Schematic": DataPlotTab(client: client, thread: thread, kind: "schematic", path: target?.tab == "Schematic" ? target!.path : "", has: files == nil || !(files?.sch ?? "").isEmpty || target != nil, none: "No schematic in this project yet")
                // unknown files (no hub yet) still open the 3D view: it shows the model it showed last
                case "3D": DataPlotTab(client: client, thread: thread, kind: "3d", path: "", has: files == nil || !(files?.pcb ?? "").isEmpty, none: "No board in this project yet")
                case "Split": DataSplitTab(client: client, thread: thread, files: files)
                case "Diff": DataDiffTab(client: client, thread: thread)
                case "Review": DataReviewTab(client: client, thread: thread)
                case "Mech": DataMechTab(client: client, thread: thread, open: target?.tab == "Mech" ? target!.path : nil)
                case "Files": DataFilesTab(client: client, thread: thread, start: ext(openFile) == "kicad_pcb" || ext(openFile) == "kicad_sch" ? "" : openFile, line: line) { target = $0; tab = $0.tab }
                default: DataBrowserTab(client: client, thread: thread)
                }
            }
        }
        .background(BackplaneDesign.background)
        .dataBar(thread.title.isEmpty ? Tr.t("New thread") : thread.title)
        .task(id: client.conn == .online) {
            guard files == nil else { return }
            files = await client.viewerFiles(thread.id); if !(files?.pcb ?? "").isEmpty && tab == "Files" && openFile.isEmpty { tab = "Board" }
            #if DEBUG || BP_MEASURE
            // a check without hands: BACKPLANE_VTAB opens that tab (BACKPLANE_SELECT=viewer:<thread> opens the viewer)
            if let t = ProcessInfo.processInfo.environment["BACKPLANE_VTAB"], tabs.contains(t) { tab = t }
            #endif
        }
        .onDisappear { Task { try? await client.watch(thread.id, kind: "") } }
    }
}

/// A tap on the plot: the canvas's candidates `[{"c": chunk, "p": piece, "a": area, "l": layer}]` (PlotCanvas.tap), the smallest meant
/// (a pad over its copper), as Android's DataViewer.kt picks; and the hub's info text (`name=value;...`) as rows.
enum DPick {
    static func best(_ json: String) -> (Int, Int)? {
        guard let a = (try? JSONSerialization.jsonObject(with: Data(json.utf8))) as? [[String: Any]] else { return nil }
        let n = { (o: [String: Any], k: String) in (o[k] as? NSNumber)?.intValue }
        guard let b = a.filter({ n($0, "c") != nil && n($0, "p") != nil }).min(by: { (n($0, "a") ?? .max) < (n($1, "a") ?? .max) }) else { return nil }
        return (n(b, "c")!, n(b, "p")!)
    }
    static func rows(_ info: String) -> [(String, String)] {
        info.replacingOccurrences(of: "\u{0}", with: ";").split(separator: ";").map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }.prefix(10).map { f in
            if let e = f.firstIndex(of: "=") { return (String(f[..<e]), String(f[f.index(after: e)...])) }
            return ("", f)
        }
    }
}

// Board, Schematic and 3D: the hub's plot on the Metal view
struct DataPlotTab: View {
    let client: DataClient
    let thread: DThread
    let kind: String
    let path: String
    let has: Bool
    let none: String
    @State private var layers: DLayers?
    @State private var refs: [DRef]?
    @State private var comparing = ""
    @State private var picked = ""
    @State private var info = ""
    @State private var menu = false
    @State private var error = ""
    private var three: Bool { kind == "3d" }
    var body: some View {
        let head = three ? "0|" : kind == "schematic" ? "1|" : "0|"
        let frame = client.plots.frame.flatMap { $0.key.hasPrefix(head) ? $0 : nil }
        let mesh = three ? client.plots.mesh.flatMap { $0.key.hasPrefix("2|") ? $0 : nil } : nil
        let parts = mesh.flatMap { client.plots.parts.parts($0.key) }
        let present = Set(frame?.chunks.map { $0.layer } ?? [])
        let bits = present.filter { $0 >= 0 && $0 < 32 }.reduce(UInt32(0)) { $0 | (1 << UInt32($1)) }
        ZStack(alignment: .topLeading) {
            if let v = plainViewer(open: three ? "3d" : kind, source: kind + "|" + path, picked: picked, off: layers?.off ?? 0, dim: layers?.dim ?? 0, lift: layers?.lift ?? 255) {
                PlotCanvasView(frame: (frame?.none.isEmpty ?? true) ? frame : nil, mesh: mesh, parts: parts, scope: thread.id, viewer: v, pick: { p in
                    // the canvas says what lies under the finger as candidates; the smallest is what the tap means. Nothing there: deselect.
                    guard let (at, idx) = DPick.best(p), let f = frame else { info = ""; picked = ""; return }
                    picked = "\(at),\(idx)"
                    Task { let t = await client.info(f.key, at: at, idx: idx); if picked == "\(at),\(idx)" { info = t.isEmpty ? "piece \(at),\(idx)" : t } }
                })
                .accessibilityIdentifier("data.viewer.surface")
                .accessibilityValue([frame?.key, mesh?.mesh == nil ? nil : mesh?.key].compactMap { $0 }.joined(separator: "\n")) // what is on show, by the hub's keys (the switch tests read it)
            }
            let say = !has ? none : !error.isEmpty ? error : (frame?.none ?? "").isEmpty ? (mesh.map { $0.decoding ? "" : $0.none } ?? "") : frame!.none
            if let progress = parts?.progress, !progress.isEmpty {
                // the parts still streaming in after the board
                Text(Tr.t(progress)).font(.system(.caption, design: .monospaced)).foregroundStyle(.secondary).padding(8).background(BackplaneDesign.panel.opacity(0.85)).padding(8)
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomLeading).allowsHitTesting(false).accessibilityIdentifier("data.viewer.parts")
            }
            if !say.isEmpty && three && mesh?.mesh != nil {
                // a model on show stays; why it may be old is said above it
                Text(Tr.t(say)).font(.system(.caption, design: .monospaced)).foregroundStyle(.secondary).padding(8).background(BackplaneDesign.panel.opacity(0.85)).padding(8).accessibilityIdentifier("data.viewer.note")
            }
            else if !say.isEmpty { Text(Tr.t(say)).font(.system(.footnote, design: .monospaced)).foregroundStyle(.secondary).padding(24).frame(maxWidth: .infinity, maxHeight: .infinity) }
            else if three && (mesh?.mesh == nil || mesh?.decoding == true) { DLoading(text: Tr.t("Loading 3D model…")) }
            else if frame == nil && !three { ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity) }
            if !three && !present.isEmpty && client.features.contains("layers"), let l = layers {
                HStack(spacing: 6) {
                    chip(Tr.t(l.layerHead), id: "data.viewer.layers", on: l.layerMode > 0) { refs = nil; menu.toggle() }
                    if comparing.isEmpty {
                        chip(Tr.t("Compare"), id: "data.viewer.compare", on: refs != nil) { menu = false; if refs != nil { refs = nil } else { Task { refs = await client.refs(thread.id) } } }
                    } else {
                        chip(Tr.t("Close compare"), id: "data.viewer.compare.close", on: true) { close() }
                    }
                }.padding(8)
                if menu { DataLayerMenu(layers: l, present: present) { a, v in act(a, v, bits) }.padding(.top, 44).padding(.leading, 8) }
                if let rows = refs { DataRefMenu(rows: rows) { c in refs = nil; open(c) }.padding(.top, 44).padding(.leading, 8) }
            }
            if !info.isEmpty {
                // a card over the viewer's foot: the viewer keeps its size and camera
                VStack(alignment: .leading, spacing: 4) {
                    HStack { Text(Tr.t("PICKED")).font(.system(.caption, design: .monospaced).bold()).foregroundStyle(BackplaneDesign.accent); Spacer(); Button(Tr.t("close")) { info = ""; picked = "" }.font(.caption).accessibilityIdentifier("data.viewer.info.close") }
                    ForEach(Array(DPick.rows(info).enumerated()), id: \.offset) { _, r in
                        HStack(alignment: .firstTextBaseline, spacing: 8) {
                            if !r.0.isEmpty { Text(Tr.t(r.0)).foregroundStyle(.secondary).frame(width: 84, alignment: .leading) }
                            Text(r.1).textSelection(.enabled).lineLimit(2)
                        }.font(.system(.footnote, design: .monospaced)).accessibilityElement(children: .combine).accessibilityIdentifier("data.viewer.info.row")
                    }
                }.padding(10).background(BackplaneDesign.panel).overlay(Rectangle().stroke(.secondary.opacity(0.4)))
                .accessibilityElement(children: .contain).accessibilityIdentifier("data.viewer.info")
                .padding(8).frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottom)
            }
        }
        .background(BackplaneDesign.panel.opacity(0.0))
        .task(id: kind + "|" + path + "|\(has)|\(client.conn == .online)") {
            guard has else { return }
            error = ""; layers = nil; refs = nil; comparing = ""; info = ""; picked = ""
            do { try await client.watch(thread.id, kind: kind, path: path) } catch { self.error = error.localizedDescription }
        }
        .task(id: "\(kind)|\(path)|\(bits)") { if !three && bits != 0 { act("", "", bits) } }
    }
    private func chip(_ label: String, id: String, on: Bool, _ tap: @escaping () -> Void) -> some View {
        Button(action: tap) { Text(label).font(.system(.footnote, design: .monospaced)).foregroundStyle(on ? BackplaneDesign.accent : .primary).padding(.horizontal, 10).padding(.vertical, 6).background(BackplaneDesign.raised).overlay(Rectangle().stroke(on ? BackplaneDesign.accent : .secondary.opacity(0.4))) }
            .accessibilityIdentifier(id)
    }
    /// A press: the hub answers what it does; a comparison on show keeps its marks to the active layer.
    private func act(_ action: String, _ value: String, _ present: UInt32) {
        Task {
            guard let r = await client.layers(kind, present: present, own: layers?.own ?? 0, focus: layers?.focus ?? "", action: action, value: value) else { return }
            let moved = (layers?.layerBits ?? 0) != r.layerBits
            layers = r
            if moved && !comparing.isEmpty { try? await client.compare(thread.id, kind: kind, commit: comparing, bits: r.layerBits, path: path) }
        }
    }
    private func open(_ commit: String) {
        Task { do { try await client.compare(thread.id, kind: kind, commit: commit, bits: layers?.layerBits ?? 0, path: path); comparing = commit } catch { self.error = error.localizedDescription } }
    }
    private func close() {
        comparing = ""
        Task { do { try await client.watch(thread.id, kind: kind, path: path) } catch { self.error = error.localizedDescription } }
    }
}

/// KiCad's high contrast, as the hub lists it: how the others show (a board), a box per layer (show or hide it), its name (make it the active one, again: all back), a comparison's marks last, Reset layers.
struct DataLayerMenu: View {
    let layers: DLayers
    let present: Set<Int>
    let act: (String, String) -> Void
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 8) {
                if layers.layerModes {
                    HStack(spacing: 4) {
                        ForEach(Array([Tr.k("viewer.layers.all"), Tr.k("viewer.layers.dim"), Tr.k("viewer.layers.only")].enumerated()), id: \.offset) { i, label in
                            Button { act("mode", String(i)) } label: { Text(label).font(.system(.footnote, design: .monospaced)).foregroundStyle(layers.layerMode == i ? BackplaneDesign.accent : .primary).padding(.horizontal, 8).padding(.vertical, 3).overlay(Rectangle().stroke(layers.layerMode == i ? BackplaneDesign.accent : .secondary.opacity(0.4))) }
                                .accessibilityIdentifier("data.layer.mode.\(i)")
                        }
                    }
                }
                ForEach((layers.layerList + layers.layerMarks).filter { present.contains($0.layer) }, id: \.layer) { l in
                    HStack(spacing: 10) {
                        Button { act("toggle", String(l.layer)) } label: { Rectangle().fill(l.on ? BackplaneDesign.accent : Color.clear).frame(width: 14, height: 14).overlay(Rectangle().stroke(.secondary.opacity(0.5))) }
                            .accessibilityIdentifier("data.layer.box.\(l.layer)")
                        Button { act("pick", String(l.layer)) } label: { Text(l.name).font(.system(.footnote, design: .monospaced)).foregroundStyle(l.active ? BackplaneDesign.accent : .primary) }
                            .accessibilityIdentifier("data.layer.name.\(l.layer)")
                    }
                }
                if layers.layerReset {
                    Button { act("reset", "") } label: { Text(Tr.t("Reset layers")).font(.system(.footnote, design: .monospaced)).foregroundStyle(BackplaneDesign.accent) }
                        .accessibilityIdentifier("data.layer.reset")
                }
            }.padding(8)
        }
        .frame(maxHeight: 420).fixedSize(horizontal: true, vertical: false).background(BackplaneDesign.panel).overlay(Rectangle().stroke(.secondary.opacity(0.4))).accessibilityIdentifier("data.layer.menu")
    }
}

/// The versions a comparison may take, as the hub lists them; a press shows the Diff against that one.
struct DataRefMenu: View {
    let rows: [DRef]
    let pick: (String) -> Void
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 8) {
                if rows.isEmpty { Text(Tr.t("No versions to compare with")).font(.system(.footnote, design: .monospaced)).foregroundStyle(.secondary) }
                ForEach(rows, id: \.self) { r in
                    Button { pick(r.commit) } label: {
                        HStack(spacing: 8) { Text(Tr.t(r.kind)).foregroundStyle(.secondary); Text(r.label) }.font(.system(.footnote, design: .monospaced))
                    }.accessibilityIdentifier("data.compare.\(r.commit)")
                }
            }.padding(8)
        }
        .frame(maxHeight: 420).fixedSize(horizontal: true, vertical: false).background(BackplaneDesign.panel).overlay(Rectangle().stroke(.secondary.opacity(0.4))).accessibilityIdentifier("data.compare.menu")
    }
}

// Mech: the project's models, one at a time
struct DataMechTab: View {
    let client: DataClient
    let thread: DThread
    let open: String?
    @State private var parts: [String]?
    @State private var part: String?
    var body: some View {
        VStack(spacing: 0) {
            if let ps = parts {
                if ps.isEmpty { Text(Tr.t("No models in mech/ yet")).font(.system(.footnote, design: .monospaced)).foregroundStyle(.secondary).padding(24) }
                else {
                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack(spacing: 8) { ForEach(ps, id: \.self) { p in
                            Button { part = p } label: { Text(p.split(separator: "/").last.map(String.init) ?? p).font(.system(.footnote, design: .monospaced)).padding(.horizontal, 10).padding(.vertical, 6).foregroundStyle(p == part ? Color.white : .primary).background(p == part ? BackplaneDesign.accent : BackplaneDesign.raised) }
                        } }.padding(8)
                    }.background(BackplaneDesign.panel)
                    if let p = part ?? open ?? ps.first { DataMechSolid(client: client, thread: thread, path: p) }
                }
            } else { ProgressView().padding(24) }
            Spacer(minLength: 0)
        }
        .task { parts = await client.mech(thread.id); if let o = open { part = o } }
    }
}
struct DataMechSolid: View {
    let client: DataClient
    let thread: DThread
    let path: String
    @State private var error = ""
    var body: some View {
        let mesh = client.plots.mesh.flatMap { $0.key.hasSuffix("/" + path) ? $0 : nil } // the part asked for, never the board's model of the 3D tab
        ZStack {
            if let v = plainViewer(open: "mech", source: "mech|" + path, picked: "", off: 0, dim: 0, lift: 255) {
                PlotCanvasView(frame: nil, mesh: mesh, scope: thread.id, viewer: v, pick: { _ in }).accessibilityIdentifier("data.mech.surface")
                    .accessibilityValue(mesh?.mesh == nil ? "" : mesh!.key) // what is drawn, by its key (the tests read it)
            }
            let say = !error.isEmpty ? error : (mesh.map { $0.decoding ? "" : $0.none } ?? "")
            if !say.isEmpty { Text(Tr.t(say)).font(.system(.footnote, design: .monospaced)).foregroundStyle(.secondary).padding(24) }
            else if mesh?.mesh == nil || mesh?.decoding == true { DLoading(text: Tr.t("Loading 3D model…")) }
        }
        .task(id: path) { error = ""; do { try await client.watch(thread.id, kind: "3d", path: path) } catch { self.error = error.localizedDescription } }
    }
}

/// A model on its way: a spinner and what it waits for, over the board's plain slab while the parts come (never a blank view)
struct DLoading: View {
    let text: String
    var body: some View {
        VStack(spacing: 10) { ProgressView(); Text(text).font(.system(.footnote, design: .monospaced)).foregroundStyle(.secondary) }
            .padding(16).background(BackplaneDesign.panel.opacity(0.85))
            .frame(maxWidth: .infinity, maxHeight: .infinity).allowsHitTesting(false).accessibilityIdentifier("data.viewer.loading")
    }
}

// Files: browse the project, preview what a phone can show, open a board, schematic or model in its tab
private let imageExt: Set<String> = ["png", "jpg", "jpeg", "gif", "webp", "bmp", "heic"]
private func ext(_ n: String) -> String { (n.split(separator: ".").last.map(String.init) ?? "").lowercased() }
private func sizeText(_ n: Int) -> String { n < 1024 ? "\(n) B" : n < 1 << 20 ? "\(n / 1024) KB" : "\(n >> 20) MB" }

struct DataFilesTab: View {
    let client: DataClient
    let thread: DThread
    var start = ""
    var line = 0
    let open: ((tab: String, path: String)) -> Void
    @State private var dir = ""
    @State private var list: [DEntryRow]?
    @State private var err = ""
    @State private var preview: String?
    @State private var started = false
    @State private var previewLine = 0
    var body: some View {
        VStack(spacing: 0) {
            if let p = preview { DataFilePreview(client: client, thread: thread, path: p, line: previewLine) { preview = nil; previewLine = 0 } }
            else {
                HStack {
                    if !dir.isEmpty { Button("..") { dir = dir.split(separator: "/").dropLast().joined(separator: "/") }.font(.system(.subheadline, design: .monospaced).bold()).accessibilityIdentifier("data.files.up") }
                    Text("/" + dir).font(.system(.footnote, design: .monospaced))
                    Spacer()
                }.padding(.horizontal, 12).padding(.vertical, 8).background(BackplaneDesign.panel)
                if !err.isEmpty { Text(Tr.t(err)).foregroundStyle(BackplaneDesign.bad).font(.footnote).padding(12) }
                if let l = list {
                    List(l.sorted { ($0.dir ? 0 : 1, $0.name.lowercased()) < ($1.dir ? 0 : 1, $1.name.lowercased()) }) { e in
                        let full = dir.isEmpty ? e.name : dir + "/" + e.name
                        Button {
                            if e.dir { dir = full } else {
                                switch ext(e.name) {
                                case "kicad_pcb": open(("Board", full))
                                case "kicad_sch": open(("Schematic", full))
                                case "step", "stp", "glb": open(("Mech", full))
                                default: preview = full
                                }
                            }
                        } label: { HStack { Image(systemName: e.dir ? "folder" : "doc").foregroundStyle(.secondary); Text(e.name).font(.system(.body, design: .monospaced)).lineLimit(1); Spacer(); if !e.dir { Text(sizeText(e.size)).font(.caption).foregroundStyle(.secondary) } } }
                        .accessibilityIdentifier("data.file." + e.name)
                    }.listStyle(.plain)
                } else if err.isEmpty { ProgressView().padding(24) }
                Spacer(minLength: 0)
            }
        }
        .task(id: dir) { list = nil; err = ""; do { list = try await client.entries(thread.id, path: dir) } catch { self.err = error.localizedDescription } }
        .onAppear {
            // a linked file: its folder behind it, the file open at the line
            guard !started, !start.isEmpty else { return }
            started = true; dir = start.split(separator: "/").dropLast().joined(separator: "/"); preview = start; previewLine = line
        }
    }
}

struct DataFilePreview: View {
    let client: DataClient
    let thread: DThread
    let path: String
    var line = 0
    let close: () -> Void
    @State private var data: Data?
    @State private var failed = false
    var body: some View {
        VStack(spacing: 0) {
            HStack { Button { close() } label: { Text("‹ " + (path.split(separator: "/").last.map(String.init) ?? path)).font(.system(.subheadline, design: .monospaced).bold()) }.accessibilityIdentifier("data.preview.back"); Spacer() }
                .padding(.horizontal, 12).padding(.vertical, 8).background(BackplaneDesign.panel)
            Divider()
            if failed { Text(Tr.t("This file could not be read.")).foregroundStyle(BackplaneDesign.bad).padding(16) }
            else if let d = data {
                if imageExt.contains(ext(path)), let u = UIImage(data: d) { Image(uiImage: u).resizable().scaledToFit().padding(8).accessibilityIdentifier("data.preview.image") }
                else if ext(path) == "pdf" { DataPDFView(data: d).accessibilityIdentifier("data.preview.pdf") }
                else if d.count < 200_000, !d.contains(0) {
                    let t = String(decoding: d, as: UTF8.self)
                    if line > 0 { DataLinedText(text: t, line: line).accessibilityIdentifier("data.preview.text") }
                    else { ScrollView { (ext(path) == "md" ? Text(.init(t)) : Text(t)).font(.system(.footnote, design: .monospaced)).frame(maxWidth: .infinity, alignment: .leading).padding(12) }.accessibilityIdentifier("data.preview.text") }
                } else { Text(Tr.ios_preview_none(size: sizeText(d.count))).foregroundStyle(.secondary).padding(16) }
            } else { ProgressView().padding(24) }
            Spacer(minLength: 0)
        }
        .task(id: path) { data = nil; failed = false; data = await client.fileBytes(thread.id, path); failed = data == nil }
    }
}
/// A file with its line numbers, scrolled to the line a link named and that line marked.
struct DataLinedText: View {
    let text: String, line: Int
    var body: some View {
        let lines = text.components(separatedBy: "\n")
        ScrollViewReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(Array(lines.enumerated()), id: \.offset) { i, l in
                        HStack(alignment: .top, spacing: 8) {
                            Text("\(i + 1)").font(.system(.footnote, design: .monospaced)).foregroundStyle(.secondary).frame(width: 44, alignment: .trailing)
                            Text(l.isEmpty ? " " : l).font(.system(.footnote, design: .monospaced)).frame(maxWidth: .infinity, alignment: .leading)
                        }.padding(.vertical, 1).background(i + 1 == line ? BackplaneDesign.accent.opacity(0.22) : Color.clear).id(i + 1)
                            .accessibilityIdentifier(i + 1 == line ? "data.preview.line" : "data.preview.l\(i + 1)")
                    }
                }.padding(.vertical, 8).textSelection(.enabled)
            }.onAppear { DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) { proxy.scrollTo(max(1, line - 3), anchor: .top) } }
        }
    }
}
struct DataPDFView: UIViewRepresentable {
    let data: Data
    func makeUIView(context: Context) -> PDFView { let v = PDFView(); v.autoScales = true; v.document = PDFDocument(data: data); return v }
    func updateUIView(_ v: PDFView, context: Context) {}
}

// Browser: the page's newest frame; once the person has the page a tap clicks, a drag scrolls and the key field types (BrowserInput.swift)
struct DataBrowserTab: View {
    let client: DataClient
    let thread: DThread
    @State private var frame: UIImage?
    @State private var none = false
    @State private var driving = false
    @State private var url = ""
    @State private var err = ""
    @State private var keys = BrowserInput.sentinel
    @State private var chain: Task<Void, Never>?   // the person's input reaches the hub in the order it was made
    @State private var dragSent = CGSize.zero      // how much of this drag has been sent as scroll
    @State private var dragging = false
    @FocusState private var keysFocused: Bool
    var body: some View {
        VStack(spacing: 0) {
            VStack(spacing: 6) {
                HStack(spacing: 8) {
                    if !driving { Button(Tr.t("Take over")) { driving = true; run("takeover") }.buttonStyle(DButton(primary: true)).accessibilityIdentifier("data.browser.takeover") }
                    else {
                        Button("‹") { run("toolbar", ["command": "back"]) }.accessibilityIdentifier("data.browser.back")
                        Button("›") { run("toolbar", ["command": "forward"]) }.accessibilityIdentifier("data.browser.forward")
                        Button("↻") { run("toolbar", ["command": "reload"]) }.accessibilityIdentifier("data.browser.reload")
                        TextField(Tr.t("Address"), text: $url).dmono(13).padding(8).dbox().textInputAutocapitalization(.never).autocorrectionDisabled().accessibilityIdentifier("data.browser.url")
                        Button(Tr.t("Go")) { run("navigate", ["url": pageUrl(url)]) }.disabled(url.trimmingCharacters(in: .whitespaces).isEmpty).accessibilityIdentifier("data.browser.go")
                    }
                    Spacer(minLength: 0)
                }
                if driving {
                    HStack(spacing: 8) {
                        // the key field: what the keyboard types here goes to the page (Enter, Tab and the arrows of a hardware keyboard too)
                        ZStack(alignment: .leading) {
                            if keys == BrowserInput.sentinel { Text(Tr.t("Keys")).dmono(13).foregroundStyle(.secondary).padding(8).allowsHitTesting(false) }
                            TextField("", text: Binding(get: { keys }, set: keyed)).dmono(13).padding(8)
                                .textInputAutocapitalization(.never).autocorrectionDisabled().submitLabel(.send).focused($keysFocused)
                                .onSubmit { press("Enter"); keysFocused = true }
                                .onKeyPress(keys: [.tab, .leftArrow, .upArrow, .rightArrow, .downArrow]) { k in
                                    let name: [KeyEquivalent: String] = [.tab: "Tab", .leftArrow: "ArrowLeft", .upArrow: "ArrowUp", .rightArrow: "ArrowRight", .downArrow: "ArrowDown"]
                                    guard let n = name[k.key] else { return .ignored }
                                    press(n); return .handled
                                }
                                .accessibilityIdentifier("data.browser.keys")
                        }.dbox()
                        Button(Tr.t("Hand back")) { driving = false; keysFocused = false; run("handback") }.accessibilityIdentifier("data.browser.handback")
                    }
                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack(spacing: 6) {
                            ForEach(BrowserInput.keyButtons.indices, id: \.self) { i in
                                let b = BrowserInput.keyButtons[i]
                                Button(b.0) { press(b.1) }.buttonStyle(DButton(primary: false)).accessibilityIdentifier("data.browser.key." + b.1)
                            }
                        }
                    }
                }
            }.padding(8).background(BackplaneDesign.panel)
            if !err.isEmpty { Text(Tr.t(err)).foregroundStyle(BackplaneDesign.bad).font(.footnote).padding(.horizontal, 12) }
            Divider()
            GeometryReader { geo in
                if let f = frame {
                    Image(uiImage: f).resizable().scaledToFit().frame(width: geo.size.width, height: geo.size.height, alignment: .top)
                        .contentShape(Rectangle())
                        .gesture(DragGesture(minimumDistance: 0).onChanged { g in
                            guard driving else { return }
                            let t = g.translation
                            if !dragging && hypot(t.width, t.height) < BrowserInput.slop { return }
                            dragging = true
                            scroll(t, f, geo.size, all: false)
                        }.onEnded { g in
                            guard driving else { return }
                            defer { dragging = false; dragSent = .zero }
                            if dragging { scroll(g.translation, f, geo.size, all: true); return }
                            // the picture is fitted to the width (and the height): the click is where it landed on the picture, 0 to 1000
                            let s = min(geo.size.width / f.size.width, geo.size.height / f.size.height)
                            let w = f.size.width * s, h = f.size.height * s
                            let x0 = (geo.size.width - w) / 2
                            let x = Int(max(0, min(1000, (g.location.x - x0) * 1000 / w))), y = Int(max(0, min(1000, g.location.y * 1000 / h)))
                            run("click", ["x": x, "y": y])
                        }).accessibilityIdentifier("data.browser.frame")
                } else if none { Text(Tr.t("No browser page yet. An agent opens one when it needs the web.")).font(.system(.footnote, design: .monospaced)).foregroundStyle(.secondary).padding(24) }
                else { ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity) }
            }
            if driving { Text(Tr.t("You have the page: a tap clicks, a drag scrolls, the key field types. Hand it back when you are done.")).font(.caption2).foregroundStyle(.secondary).padding(8) }
        }
        .task {
            while !Task.isCancelled { await load(); try? await Task.sleep(for: .seconds(2)) }
        }
        .onChange(of: client.shot) { _, _ in if client.shotBot == thread.id { Task { await load() } } }
    }
    private func load() async {
        if let d = await client.screenBytes(thread.id) { frame = UIImage(data: d); none = false } else { none = frame == nil }
    }
    private func run(_ op: String, _ extra: [String: Any] = [:]) {
        let prev = chain
        chain = Task { await prev?.value; err = ""; do { try await client.browser(thread.id, op, extra) } catch { err = error.localizedDescription } }
    }
    private func press(_ k: String) { run("press", ["key": k]) }
    private func keyed(_ v: String) {
        guard v != keys else { return }
        for (op, a) in BrowserInput.keys(keys, v) { if op == "type" { run("type", ["text": a]) } else { press(a) } }
        keys = BrowserInput.next(v)
    }
    /// The drag so far (`t`) less what was sent, as the page's scroll: in steps while the finger moves, the rest when it lifts.
    private func scroll(_ t: CGSize, _ f: UIImage, _ box: CGSize, all: Bool) {
        let s = min(box.width / f.size.width, box.height / f.size.height)
        let d = BrowserInput.scroll(Double(t.width - dragSent.width), Double(t.height - dragSent.height), image: Double(f.size.width), shown: Double(f.size.width * s))
        guard (all && (d.0 != 0 || d.1 != 0)) || abs(d.0) + abs(d.1) >= BrowserInput.step else { return }
        run("scroll", ["dx": d.0, "dy": d.1]); dragSent = t
    }
}


// MARK: Split: the schematic, the board and its 3D model at once (stacked in portrait, side by side in landscape)
struct DataSplitTab: View {
    let client: DataClient
    let thread: DThread
    let files: DProjectFiles?
    var body: some View {
        let root = files?.root ?? ""
        let plots = client.plots
        let _ = (plots.frame?.at, plots.mesh?.at)   // the store changes on each frame: read it so the panes follow
        let sch = plots.cachedFrame("1|\(root)|"), brd = plots.cachedFrame("0|\(root)|"), mdl = plots.cachedMesh("2|\(root)|")
        GeometryReader { geo in
            let panes = Group {
                pane("Schematic", "sch", sch, nil, "schematic", (files?.sch ?? "").isEmpty ? "No schematic in this project yet" : "")
                pane("Board", "brd", brd, nil, "board", (files?.pcb ?? "").isEmpty ? "No board in this project yet" : "")
                pane("3D", "3d", brd, mdl, "3d", (files?.pcb ?? "").isEmpty ? "No board in this project yet" : "")
            }
            if geo.size.width > geo.size.height { HStack(spacing: 0) { panes } } else { VStack(spacing: 0) { panes } }
        }
        .task(id: root) { if !root.isEmpty { try? await client.watch(thread.id, kind: "split") } }
        .accessibilityIdentifier("data.split")
    }
    @ViewBuilder private func pane(_ label: String, _ id: String, _ frame: PlotFrame?, _ mesh: MeshFrame?, _ open: String, _ none: String) -> some View {
        ZStack(alignment: .topLeading) {
            if let v = plainViewer(open: open, source: "split|" + id, picked: "", off: 0, dim: 0, lift: 255) {
                PlotCanvasView(frame: (frame?.none.isEmpty ?? true) ? frame : nil, mesh: mesh, scope: thread.id + "|split", viewer: v, pick: { _ in }).accessibilityIdentifier("data.split." + id)
            }
            Text(Tr.t(label).uppercased()).font(.system(.caption2, design: .monospaced)).foregroundStyle(.secondary).padding(6)
            let say = !none.isEmpty ? none : (frame?.none ?? "")
            if !say.isEmpty { Text(Tr.t(say)).font(.system(.footnote, design: .monospaced)).foregroundStyle(.secondary).padding(12).frame(maxWidth: .infinity, maxHeight: .infinity) }
            else if frame == nil && (open != "3d" || mesh == nil) { ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity) }
        }
        .overlay(Rectangle().stroke(.secondary.opacity(0.3)))
    }
}

// MARK: Diff and Review (the hub's git.diff and review.* requests)
struct DDiff: Decodable { var diff: String; var nogit: String
    enum CodingKeys: String, CodingKey { case diff, nogit }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); diff = c.str(.diff); nogit = c.str(.nogit) } }
struct DTask: Decodable, Identifiable { var text: String; var done: Bool; var group: String; var id: String { group + "|" + text }
    enum CodingKeys: String, CodingKey { case text, done, group }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); text = c.str(.text); done = c.bool(.done); group = c.str(.group) } }
struct DReport: Decodable { var md: String; var err: String; var gate: String; var line: String; var tasks: [DTask]
    enum CodingKeys: String, CodingKey { case md, err, gate, line, tasks }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); md = c.str(.md); err = c.str(.err); gate = c.str(.gate); line = c.str(.line); tasks = (try? c.decode([DTask].self, forKey: .tasks)) ?? [] } }
struct DReportReply: Decodable { var report: DReport
    enum CodingKeys: String, CodingKey { case report }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); if let r = try? c.decode(DReport.self, forKey: .report) { report = r } else { report = try DReport(from: d) } } }
struct DTicked: Decodable { var ticked: DReport
    enum CodingKeys: String, CodingKey { case ticked }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); ticked = try c.decode(DReport.self, forKey: .ticked) } }
struct DComment: Decodable, Identifiable { var id: String; var text: String; var by: String; var parent: String; var resolved: Bool
    enum CodingKeys: String, CodingKey { case id, text, by, parent, resolved }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); id = c.str(.id); text = c.str(.text); by = c.str(.by); parent = c.str(.parent); resolved = c.bool(.resolved) } }
struct DComments: Decodable { var comments: [DComment]
    enum CodingKeys: String, CodingKey { case comments }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); comments = (try? c.decode([DComment].self, forKey: .comments)) ?? [] } }

extension DataClient {
    func diff(_ thread: String) async throws -> DDiff { try await call("viewer.diff", ["thread": thread]) }
    func reviewPage(_ thread: String, _ page: String) async throws -> DReport { (try await call("review.page", ["thread": thread, "page": page]) as DReportReply).report }
    func tick(_ thread: String, _ index: Int) async -> [DTask]? { (try? await call("review.tick", ["thread": thread, "index": index]) as DTicked)?.ticked.tasks }
    func comments(_ thread: String) async throws -> [DComment] { (try await call("review.list", ["thread": thread]) as DComments).comments }
    func comment(_ thread: String, _ op: String, _ extra: [String: Any] = [:]) async throws {
        var p = extra; p["thread"] = thread; p["op"] = op
        _ = try await call("review.comment", p, as: DEmpty.self)
    }
}

struct DataDiffTab: View {
    let client: DataClient
    let thread: DThread
    @State private var diff: DDiff?
    @State private var err = ""
    @State private var n = 0
    var body: some View {
        VStack(spacing: 0) {
            HStack { Text(Tr.t("CHANGES")).font(.system(.caption, design: .monospaced)).foregroundStyle(.secondary); Spacer(); Button(Tr.t("Refresh")) { n += 1 }.accessibilityIdentifier("data.diff.refresh") }
                .padding(8).background(BackplaneDesign.panel)
            Divider()
            if !err.isEmpty { Text(Tr.t(err)).foregroundStyle(BackplaneDesign.bad).font(.footnote).padding(16) }
            else if let d = diff {
                if !d.nogit.isEmpty { Text(Tr.t("This project is not a git repository, so there is nothing to compare.")).foregroundStyle(.secondary).padding(16).accessibilityIdentifier("data.diff.nogit") }
                else if d.diff.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { Text(Tr.t("No changes.")).foregroundStyle(.secondary).padding(16) }
                else {
                    ScrollView([.vertical, .horizontal]) {
                        LazyVStack(alignment: .leading, spacing: 0) {
                            ForEach(Array(d.diff.split(separator: "\n", omittingEmptySubsequences: false).prefix(4000).enumerated()), id: \.offset) { _, l in
                                let line = String(l)
                                Text(line.isEmpty ? " " : line).font(.system(size: 11, design: .monospaced)).lineLimit(1).fixedSize()
                                    .foregroundStyle(color(line)).frame(maxWidth: .infinity, alignment: .leading).background(tint(line))
                            }
                        }
                    }.accessibilityIdentifier("data.diff.list")
                }
            } else { ProgressView().padding(24) }
            Spacer(minLength: 0)
        }
        .task(id: n) { diff = nil; err = ""; do { diff = try await client.diff(thread.id) } catch { self.err = error.localizedDescription } }
    }
    private func color(_ l: String) -> Color {
        if l.hasPrefix("+++") || l.hasPrefix("---") || l.hasPrefix("diff ") || l.hasPrefix("index ") { return .primary }
        if l.hasPrefix("+") { return Color(red: 0.30, green: 0.69, blue: 0.48) }
        if l.hasPrefix("-") { return .red }
        if l.hasPrefix("@@") { return BackplaneDesign.accent }
        return .secondary
    }
    private func tint(_ l: String) -> Color {
        if l.hasPrefix("+++") || l.hasPrefix("---") { return .clear }
        if l.hasPrefix("+") { return Color(red: 0.30, green: 0.69, blue: 0.48).opacity(0.12) }
        if l.hasPrefix("-") { return Color.red.opacity(0.12) }
        return .clear
    }
}

struct DataReviewTab: View {
    let client: DataClient
    let thread: DThread
    @State private var page = "Checks"
    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 8) { ForEach(["Checks", "BOM", "Comments"], id: \.self) { p in
                Button(Tr.t(p)) { page = p }.buttonStyle(.bordered).tint(p == page ? BackplaneDesign.accent : .secondary).accessibilityIdentifier("data.review.page." + p) } ; Spacer() }
                .padding(8).background(BackplaneDesign.panel)
            Divider()
            switch page {
            case "BOM": DataReportPage(client: client, thread: thread, kind: "bom", ticks: false)
            case "Comments": DataCommentsPage(client: client, thread: thread)
            default: DataReportPage(client: client, thread: thread, kind: "checks", ticks: true)
            }
        }
    }
}
struct DataReportPage: View {
    let client: DataClient
    let thread: DThread
    let kind: String
    let ticks: Bool
    @State private var rep: DReport?
    @State private var tasks: [DTask] = []
    @State private var err = ""
    @State private var run = 0
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 10) {
                Button(Tr.t(ticks ? "Run checks" : "Refresh")) { run += 1 }.buttonStyle(.bordered).accessibilityIdentifier("data.review.run")
                if !err.isEmpty { Text(Tr.t(err)).foregroundStyle(BackplaneDesign.bad).font(.footnote) }
                if let r = rep {
                    if !r.line.isEmpty { Text(Tr.t(r.line)).font(.system(.footnote, design: .monospaced).bold()).foregroundStyle(BackplaneDesign.accent) }
                    if !r.gate.isEmpty { Text(Tr.t(r.gate)).font(.caption).foregroundStyle(.secondary) }
                    if !r.err.isEmpty { Text(Tr.t(r.err)).font(.footnote).foregroundStyle(BackplaneDesign.bad) }
                    if !r.md.isEmpty { ScrollView(.horizontal) { Text(.init(r.md)).font(.system(.caption, design: .monospaced)) } }
                    if ticks { ForEach(Array(tasks.enumerated()), id: \.offset) { i, t in
                        Button { Task { if let n = await client.tick(thread.id, i) { tasks = n } } } label: {
                            HStack { Image(systemName: t.done ? "checkmark.square.fill" : "square").foregroundStyle(t.done ? BackplaneDesign.accent : .secondary); Text(t.text).font(.system(.footnote, design: .monospaced)).foregroundStyle(.primary) } }
                            .accessibilityIdentifier("data.review.task.\(i)") } }
                } else if err.isEmpty { ProgressView() }
            }.padding(12).frame(maxWidth: .infinity, alignment: .leading)
        }
        .task(id: kind + "\(run)") { rep = nil; err = ""; do { let r = try await client.reviewPage(thread.id, kind); rep = r; tasks = r.tasks } catch { self.err = error.localizedDescription } }
    }
}
struct DataCommentsPage: View {
    let client: DataClient
    let thread: DThread
    @State private var list: [DComment]?
    @State private var draft = ""
    @State private var err = ""
    var body: some View {
        VStack(spacing: 0) {
            ScrollView {
                VStack(alignment: .leading, spacing: 10) {
                    if !err.isEmpty { Text(Tr.t(err)).foregroundStyle(BackplaneDesign.bad).font(.footnote) }
                    if let l = list {
                        if l.isEmpty { Text(Tr.t("No comments yet. Add one below; the agent reads the open ones with its next message.")).font(.footnote).foregroundStyle(.secondary) }
                        ForEach(l.filter { $0.parent.isEmpty }) { c in
                            VStack(alignment: .leading, spacing: 4) {
                                HStack { Text((c.by.isEmpty ? Tr.t("you") : c.by) + (c.resolved ? " · " + Tr.t("resolved") : "")).font(.system(.caption, design: .monospaced)).foregroundStyle(.secondary); Spacer()
                                    Button(Tr.t(c.resolved ? "reopen" : "resolve")) { Task { try? await client.comment(thread.id, c.resolved ? "reopen" : "resolve", ["id": c.id]); await load() } }.font(.caption).accessibilityIdentifier("data.review.resolve." + c.id) }
                                Text(c.text).font(.system(.footnote, design: .monospaced))
                                ForEach(l.filter { $0.parent == c.id }) { r in Text("↳ " + r.text).font(.system(.caption, design: .monospaced)).foregroundStyle(.secondary) }
                            }.padding(10).frame(maxWidth: .infinity, alignment: .leading).background(BackplaneDesign.panel).overlay(Rectangle().stroke(c.resolved ? Color.secondary.opacity(0.3) : BackplaneDesign.accent.opacity(0.6)))
                        }
                    } else { ProgressView() }
                }.padding(12)
            }.accessibilityIdentifier("data.review.comments")
            Divider()
            HStack {
                TextField(Tr.t("A review comment"), text: $draft, axis: .vertical).dmono(13).padding(8).dbox().accessibilityIdentifier("data.review.composer")
                Button(Tr.t("Add")) { let t = draft; draft = ""; Task { do { try await client.comment(thread.id, "add", ["text": t]) } catch { err = error.localizedDescription }; await load() } }
                    .buttonStyle(DButton(primary: true)).disabled(draft.trimmingCharacters(in: .whitespaces).isEmpty).accessibilityIdentifier("data.review.add")
            }.padding(8).background(BackplaneDesign.panel)
        }
        .task { await load() }
    }
    private func load() async { do { list = try await client.comments(thread.id) } catch { err = error.localizedDescription } }
}

/// An address as typed: with a scheme it is as it is; a local or numeric host (localhost, 127.x, 10.x, 192.168.x, host:port) is http, anything else https.
func pageUrl(_ t: String) -> String {
    let u = t.trimmingCharacters(in: .whitespaces)
    if u.contains("://") { return u }
    let host = String(u.split(separator: "/", omittingEmptySubsequences: false).first ?? "")
    let local = host.hasPrefix("localhost") || host.range(of: "^(127|10)\\.[0-9.]+(:[0-9]+)?$|^192\\.168\\.[0-9.]+(:[0-9]+)?$", options: .regularExpression) != nil || host.range(of: ":[0-9]+$", options: .regularExpression) != nil
    return (local ? "http://" : "https://") + u
}
