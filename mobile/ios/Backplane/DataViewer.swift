import SwiftUI
import PDFKit

// The viewer over a thread on the phone (the same tabs as the desktop's right rail and the Android app's DataViewer.kt): Board, Schematic, 3D, Mech, Files, Browser,
// on the data/1 viewer requests (native/host/src/data/ext.rs). Plots and models arrive as the hub's own binary frames (DataClient routes them to `plots`)
// and are drawn by the app's Metal view (PlotCanvasView: pinch, pan, orbit, tap to inspect). Written without a simulator run: compile-checked only.

struct DViewerRoute: Hashable { var id: String }
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
struct DInfo: Decodable { var info: String
    enum CodingKeys: String, CodingKey { case info }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); info = c.str(.info) } }

extension DataClient {
    func viewerFiles(_ thread: String) async -> DProjectFiles? { try? await call("viewer.files", ["thread": thread]) }
    func watch(_ thread: String, kind: String, path: String = "") async throws {
        var p: [String: Any] = ["thread": thread, "kind": kind]; if !path.isEmpty { p["path"] = path }
        _ = try await call("viewer.watch", p, as: DEmpty.self)
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
private let boardLayers = [12, 11, 10, 9, 8, 7, 6, 5, 13, 23, 24, 15, 2, 16, 3, 14, 4, 19, 20, 22, 21, 17, 1, 18, 0]
private let boardNames = ["B.Fab", "B.Courtyard", "B.Paste", "B.Silkscreen", "B.Mask", "B.Cu", "In6.Cu", "In5.Cu", "In4.Cu", "In3.Cu", "In2.Cu", "In1.Cu", "F.Cu", "Pads", "F.Mask", "F.Paste", "F.Silkscreen", "F.Courtyard", "F.Fab", "User.Drawings", "User.Comments", "Margin", "Edge.Cuts", "Vias", "Holes"]
private let schNames = ["page", "sheet fill", "body fill", "sheet", "graphic", "note", "body", "pin", "pin name", "pin number", "field", "field other", "bus", "bus entry", "wire", "no connect", "junction", "label", "global label", "hier label", "sheet name", "sheet file", "sheet pin", "dnp"]

/// The Viewer the Metal view wants, with the defaults the hub's own view would send; only what the data app decides (layers, the picked piece) varies.
private func plainViewer(open: String, source: String, picked: String, off: UInt32, dim: UInt32, lift: Int) -> Viewer? {
    let j = """
    {"open":"\(open)","key":"","layers":"\(source)","choices":[],"bg":852754,"fade":220,"margin":0.9,"zmin":0.5,"zmax":0.5,"tap":14,"top":[],"bottom":[],"slab":2060099,"fov":35,
     "picked":"\(picked)","off":\(off),"dim":\(dim),"lift":\(lift)}
    """
    return try? JSONDecoder().decode(Viewer.self, from: Data(j.utf8))
}

struct DataViewerView: View {
    let client: DataClient
    let thread: DThread
    @State private var tab = "Files"
    @State private var target: (tab: String, path: String)?
    @State private var files: DProjectFiles?
    var body: some View {
        VStack(spacing: 0) {
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 0) {
                    ForEach(tabs, id: \.self) { t in
                        Button { tab = t; target = nil } label: { Text(t).font(.system(.subheadline, design: .monospaced)).fontWeight(t == tab ? .bold : .regular).foregroundStyle(t == tab ? BackplaneDesign.accent : .secondary).padding(.horizontal, 14).padding(.vertical, 10) }
                            .accessibilityIdentifier("data.vtab." + t)
                    }
                }
            }.background(BackplaneDesign.panel)
            Divider()
            Group {
                switch tab {
                case "Board": DataPlotTab(client: client, thread: thread, kind: "board", path: target?.tab == "Board" ? target!.path : "", has: !(files?.pcb ?? "").isEmpty || target != nil, none: "No board in this project yet")
                case "Schematic": DataPlotTab(client: client, thread: thread, kind: "schematic", path: target?.tab == "Schematic" ? target!.path : "", has: !(files?.sch ?? "").isEmpty || target != nil, none: "No schematic in this project yet")
                case "3D": DataPlotTab(client: client, thread: thread, kind: "3d", path: "", has: !(files?.pcb ?? "").isEmpty, none: "No board in this project yet")
                case "Split": DataSplitTab(client: client, thread: thread, files: files)
                case "Diff": DataDiffTab(client: client, thread: thread)
                case "Review": DataReviewTab(client: client, thread: thread)
                case "Mech": DataMechTab(client: client, thread: thread, open: target?.tab == "Mech" ? target!.path : nil)
                case "Files": DataFilesTab(client: client, thread: thread) { target = $0; tab = $0.tab }
                default: DataBrowserTab(client: client, thread: thread)
                }
            }
        }
        .navigationTitle("Viewer · " + (thread.title.isEmpty ? "New thread" : thread.title)).navigationBarTitleDisplayMode(.inline)
        .task { files = await client.viewerFiles(thread.id); if !(files?.pcb ?? "").isEmpty { tab = "Board" } }
        .onDisappear { Task { try? await client.watch(thread.id, kind: "") } }
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
    @State private var off: UInt32 = 0
    @State private var dim: UInt32 = 0
    @State private var lift = 255
    @State private var picked = ""
    @State private var info = ""
    @State private var menu = false
    @State private var error = ""
    private var three: Bool { kind == "3d" }
    var body: some View {
        let head = three ? "0|" : kind == "schematic" ? "1|" : "0|"
        let frame = client.plots.frame.flatMap { $0.key.hasPrefix(head) ? $0 : nil }
        let mesh = three ? client.plots.mesh.flatMap { $0.key.hasPrefix("2|") ? $0 : nil } : nil
        let present = Set(frame?.chunks.map { $0.layer } ?? [])
        ZStack(alignment: .topLeading) {
            if let v = plainViewer(open: three ? "3d" : kind, source: kind + "|" + path, picked: picked, off: off, dim: dim, lift: lift) {
                PlotCanvasView(frame: (frame?.none.isEmpty ?? true) ? frame : nil, mesh: mesh, scope: thread.id, viewer: v, pick: { p in
                    picked = p
                    let ps = p.split(separator: ",").compactMap { Int($0) }
                    if ps.count == 2, let f = frame { Task { info = await client.info(f.key, at: ps[0], idx: ps[1]) } }
                })
                .accessibilityIdentifier("data.viewer.surface")
            }
            let say = !has ? none : !error.isEmpty ? error : (frame?.none ?? "").isEmpty ? (mesh?.none ?? "") : frame!.none
            if !say.isEmpty { Text(say).font(.system(.footnote, design: .monospaced)).foregroundStyle(.secondary).padding(24).frame(maxWidth: .infinity, maxHeight: .infinity) }
            else if frame == nil && (!three || mesh == nil) { ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity) }
            if !three && !present.isEmpty {
                Button { menu.toggle() } label: { Text("Layers").font(.system(.footnote, design: .monospaced)).padding(.horizontal, 10).padding(.vertical, 6).background(BackplaneDesign.raised).overlay(Rectangle().stroke(.secondary.opacity(0.4))) }
                    .padding(8).accessibilityIdentifier("data.viewer.layers")
                if menu { DataLayerMenu(sch: kind == "schematic", present: present, off: $off, dim: $dim, lift: $lift).padding(.top, 44).padding(.leading, 8) }
            }
            if !info.isEmpty {
                VStack(alignment: .leading, spacing: 4) {
                    HStack { Text("PICKED").font(.system(.caption, design: .monospaced).bold()).foregroundStyle(BackplaneDesign.accent); Spacer(); Button("close") { info = ""; picked = "" }.font(.caption) }
                    Text(info.replacingOccurrences(of: "\u{0}", with: "  ")).font(.system(.footnote, design: .monospaced)).lineLimit(8)
                }.padding(10).background(BackplaneDesign.panel).frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottom).accessibilityIdentifier("data.viewer.info")
            }
        }
        .background(BackplaneDesign.panel.opacity(0.0))
        .task(id: kind + "|" + path + "|\(has)") {
            guard has else { return }
            error = ""; off = 0; dim = 0; lift = 255; info = ""; picked = ""
            do { try await client.watch(thread.id, kind: kind, path: path) } catch { self.error = error.localizedDescription }
        }
    }
}

/// KiCad's high contrast: a box shows or hides a layer, a name makes it the active one (the rest dimmed), again brings all back.
struct DataLayerMenu: View {
    let sch: Bool
    let present: Set<Int>
    @Binding var off: UInt32
    @Binding var dim: UInt32
    @Binding var lift: Int
    var body: some View {
        let order = sch ? present.sorted() : boardLayers.filter { present.contains($0) }
        ScrollView {
            VStack(alignment: .leading, spacing: 8) {
                ForEach(order, id: \.self) { l in
                    HStack(spacing: 10) {
                        let on = l < 0 || l > 31 || (off >> UInt32(l)) & 1 == 0
                        Button { if l >= 0 && l < 32 { off ^= (1 << UInt32(l)) } } label: { Rectangle().fill(on ? BackplaneDesign.accent : Color.clear).frame(width: 14, height: 14).overlay(Rectangle().stroke(.secondary.opacity(0.5))) }
                        Button {
                            if lift == l { dim = 0; lift = 255 } else { dim = order.filter { $0 != l && $0 >= 0 && $0 < 32 }.reduce(UInt32(0)) { $0 | (1 << UInt32($1)) }; lift = l }
                        } label: { Text((sch ? schNames : boardNames).indices.contains(l) ? (sch ? schNames : boardNames)[l] : "layer \(l)").font(.system(.footnote, design: .monospaced)).foregroundStyle(lift == l ? BackplaneDesign.accent : .primary) }
                    }
                }
            }.padding(8)
        }
        .frame(maxHeight: 420).fixedSize(horizontal: true, vertical: false).background(BackplaneDesign.panel).overlay(Rectangle().stroke(.secondary.opacity(0.4))).accessibilityIdentifier("data.layer.menu")
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
                if ps.isEmpty { Text("No models in mech/ yet").font(.system(.footnote, design: .monospaced)).foregroundStyle(.secondary).padding(24) }
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
        let mesh = client.plots.mesh.flatMap { ($0.key.hasPrefix("2|") || $0.key.hasPrefix("3|")) ? $0 : nil }
        ZStack {
            if let v = plainViewer(open: "mech", source: "mech|" + path, picked: "", off: 0, dim: 0, lift: 255) {
                PlotCanvasView(frame: nil, mesh: mesh, scope: thread.id, viewer: v, pick: { _ in }).accessibilityIdentifier("data.mech.surface")
            }
            let say = !error.isEmpty ? error : (mesh?.none ?? "")
            if !say.isEmpty { Text(say).font(.system(.footnote, design: .monospaced)).foregroundStyle(.secondary).padding(24) }
            else if mesh == nil { ProgressView() }
        }
        .task(id: path) { error = ""; do { try await client.watch(thread.id, kind: "3d-bare", path: path) } catch { self.error = error.localizedDescription } }
    }
}

// Files: browse the project, preview what a phone can show, open a board, schematic or model in its tab
private let imageExt: Set<String> = ["png", "jpg", "jpeg", "gif", "webp", "bmp", "heic"]
private func ext(_ n: String) -> String { (n.split(separator: ".").last.map(String.init) ?? "").lowercased() }
private func sizeText(_ n: Int) -> String { n < 1024 ? "\(n) B" : n < 1 << 20 ? "\(n / 1024) KB" : "\(n >> 20) MB" }

struct DataFilesTab: View {
    let client: DataClient
    let thread: DThread
    let open: ((tab: String, path: String)) -> Void
    @State private var dir = ""
    @State private var list: [DEntryRow]?
    @State private var err = ""
    @State private var preview: String?
    var body: some View {
        VStack(spacing: 0) {
            if let p = preview { DataFilePreview(client: client, thread: thread, path: p) { preview = nil } }
            else {
                HStack {
                    if !dir.isEmpty { Button("..") { dir = dir.split(separator: "/").dropLast().joined(separator: "/") }.font(.system(.subheadline, design: .monospaced).bold()).accessibilityIdentifier("data.files.up") }
                    Text("/" + dir).font(.system(.footnote, design: .monospaced))
                    Spacer()
                }.padding(.horizontal, 12).padding(.vertical, 8).background(BackplaneDesign.panel)
                if !err.isEmpty { Text(err).foregroundStyle(.red).font(.footnote).padding(12) }
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
    }
}

struct DataFilePreview: View {
    let client: DataClient
    let thread: DThread
    let path: String
    let close: () -> Void
    @State private var data: Data?
    @State private var failed = false
    var body: some View {
        VStack(spacing: 0) {
            HStack { Button { close() } label: { Text("‹ " + (path.split(separator: "/").last.map(String.init) ?? path)).font(.system(.subheadline, design: .monospaced).bold()) }.accessibilityIdentifier("data.preview.back"); Spacer() }
                .padding(.horizontal, 12).padding(.vertical, 8).background(BackplaneDesign.panel)
            Divider()
            if failed { Text("This file could not be read.").foregroundStyle(.red).padding(16) }
            else if let d = data {
                if imageExt.contains(ext(path)), let u = UIImage(data: d) { Image(uiImage: u).resizable().scaledToFit().padding(8).accessibilityIdentifier("data.preview.image") }
                else if ext(path) == "pdf" { DataPDFView(data: d).accessibilityIdentifier("data.preview.pdf") }
                else if d.count < 200_000, !d.contains(0) {
                    ScrollView { let t = String(decoding: d, as: UTF8.self)
                        (ext(path) == "md" ? Text(.init(t)) : Text(t)).font(.system(.footnote, design: .monospaced)).frame(maxWidth: .infinity, alignment: .leading).padding(12) }.accessibilityIdentifier("data.preview.text")
                } else { Text("No preview for this file (\(sizeText(d.count))).").foregroundStyle(.secondary).padding(16) }
            } else { ProgressView().padding(24) }
            Spacer(minLength: 0)
        }
        .task(id: path) { data = nil; failed = false; data = await client.fileBytes(thread.id, path); failed = data == nil }
    }
}
struct DataPDFView: UIViewRepresentable {
    let data: Data
    func makeUIView(context: Context) -> PDFView { let v = PDFView(); v.autoScales = true; v.document = PDFDocument(data: data); return v }
    func updateUIView(_ v: PDFView, context: Context) {}
}

// Browser: the page's newest frame; a tap is a click once the person has the page
struct DataBrowserTab: View {
    let client: DataClient
    let thread: DThread
    @State private var frame: UIImage?
    @State private var none = false
    @State private var driving = false
    @State private var url = ""
    @State private var err = ""
    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 8) {
                if !driving { Button("Take over") { driving = true; run("takeover") }.buttonStyle(.borderedProminent).accessibilityIdentifier("data.browser.takeover") }
                else {
                    Button("‹") { run("toolbar", ["command": "back"]) }.accessibilityIdentifier("data.browser.back")
                    TextField("Address", text: $url).textFieldStyle(.roundedBorder).textInputAutocapitalization(.never).autocorrectionDisabled().accessibilityIdentifier("data.browser.url")
                    Button("Go") { run("navigate", ["url": pageUrl(url)]) }.disabled(url.trimmingCharacters(in: .whitespaces).isEmpty).accessibilityIdentifier("data.browser.go")
                    Button("Hand back") { driving = false; run("handback") }.accessibilityIdentifier("data.browser.handback")
                }
                Spacer(minLength: 0)
            }.padding(8).background(BackplaneDesign.panel)
            if !err.isEmpty { Text(err).foregroundStyle(.red).font(.footnote).padding(.horizontal, 12) }
            Divider()
            GeometryReader { geo in
                if let f = frame {
                    Image(uiImage: f).resizable().scaledToFit().frame(width: geo.size.width, height: geo.size.height, alignment: .top)
                        .contentShape(Rectangle())
                        .gesture(DragGesture(minimumDistance: 0).onEnded { g in
                            guard driving else { return }
                            // the picture is fitted to the width (and the height): the click is where it landed on the picture, 0 to 1000
                            let s = min(geo.size.width / f.size.width, geo.size.height / f.size.height)
                            let w = f.size.width * s, h = f.size.height * s
                            let x0 = (geo.size.width - w) / 2
                            let x = Int(max(0, min(1000, (g.location.x - x0) * 1000 / w))), y = Int(max(0, min(1000, g.location.y * 1000 / h)))
                            run("click", ["x": x, "y": y])
                        }).accessibilityIdentifier("data.browser.frame")
                } else if none { Text("No browser page yet. An agent opens one when it needs the web.").font(.system(.footnote, design: .monospaced)).foregroundStyle(.secondary).padding(24) }
                else { ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity) }
            }
            if driving { Text("You have the page: a tap clicks. Hand it back when you are done.").font(.caption2).foregroundStyle(.secondary).padding(8) }
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
        Task { err = ""; do { try await client.browser(thread.id, op, extra) } catch { err = error.localizedDescription } }
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
            Text(label.uppercased()).font(.system(.caption2, design: .monospaced)).foregroundStyle(.secondary).padding(6)
            let say = !none.isEmpty ? none : (frame?.none ?? "")
            if !say.isEmpty { Text(say).font(.system(.footnote, design: .monospaced)).foregroundStyle(.secondary).padding(12).frame(maxWidth: .infinity, maxHeight: .infinity) }
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
            HStack { Text("CHANGES").font(.system(.caption, design: .monospaced)).foregroundStyle(.secondary); Spacer(); Button("Refresh") { n += 1 }.accessibilityIdentifier("data.diff.refresh") }
                .padding(8).background(BackplaneDesign.panel)
            Divider()
            if !err.isEmpty { Text(err).foregroundStyle(.red).font(.footnote).padding(16) }
            else if let d = diff {
                if !d.nogit.isEmpty { Text("This project is not a git repository, so there is nothing to compare.").foregroundStyle(.secondary).padding(16).accessibilityIdentifier("data.diff.nogit") }
                else if d.diff.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { Text("No changes.").foregroundStyle(.secondary).padding(16) }
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
                Button(p) { page = p }.buttonStyle(.bordered).tint(p == page ? BackplaneDesign.accent : .secondary).accessibilityIdentifier("data.review.page." + p) } ; Spacer() }
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
                Button(ticks ? "Run checks" : "Refresh") { run += 1 }.buttonStyle(.bordered).accessibilityIdentifier("data.review.run")
                if !err.isEmpty { Text(err).foregroundStyle(.red).font(.footnote) }
                if let r = rep {
                    if !r.line.isEmpty { Text(r.line).font(.system(.footnote, design: .monospaced).bold()).foregroundStyle(BackplaneDesign.accent) }
                    if !r.gate.isEmpty { Text(r.gate).font(.caption).foregroundStyle(.secondary) }
                    if !r.err.isEmpty { Text(r.err).font(.footnote).foregroundStyle(.red) }
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
                    if !err.isEmpty { Text(err).foregroundStyle(.red).font(.footnote) }
                    if let l = list {
                        if l.isEmpty { Text("No comments yet. Add one below; the agent reads the open ones with its next message.").font(.footnote).foregroundStyle(.secondary) }
                        ForEach(l.filter { $0.parent.isEmpty }) { c in
                            VStack(alignment: .leading, spacing: 4) {
                                HStack { Text((c.by.isEmpty ? "you" : c.by) + (c.resolved ? " · resolved" : "")).font(.system(.caption, design: .monospaced)).foregroundStyle(.secondary); Spacer()
                                    Button(c.resolved ? "reopen" : "resolve") { Task { try? await client.comment(thread.id, c.resolved ? "reopen" : "resolve", ["id": c.id]); await load() } }.font(.caption).accessibilityIdentifier("data.review.resolve." + c.id) }
                                Text(c.text).font(.system(.footnote, design: .monospaced))
                                ForEach(l.filter { $0.parent == c.id }) { r in Text("↳ " + r.text).font(.system(.caption, design: .monospaced)).foregroundStyle(.secondary) }
                            }.padding(10).frame(maxWidth: .infinity, alignment: .leading).background(BackplaneDesign.panel).overlay(Rectangle().stroke(c.resolved ? Color.secondary.opacity(0.3) : BackplaneDesign.accent.opacity(0.6)))
                        }
                    } else { ProgressView() }
                }.padding(12)
            }.accessibilityIdentifier("data.review.comments")
            Divider()
            HStack {
                TextField("A review comment", text: $draft, axis: .vertical).textFieldStyle(.roundedBorder).accessibilityIdentifier("data.review.composer")
                Button("Add") { let t = draft; draft = ""; Task { do { try await client.comment(thread.id, "add", ["text": t]) } catch { err = error.localizedDescription }; await load() } }
                    .buttonStyle(.borderedProminent).disabled(draft.trimmingCharacters(in: .whitespaces).isEmpty).accessibilityIdentifier("data.review.add")
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
