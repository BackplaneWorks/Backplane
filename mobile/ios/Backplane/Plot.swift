import Foundation
import Observation
import simd

// Plots from the hub as GPU-ready geometry. Nothing
// is decided here: which chunks, their order, colour and opacity come from
// the hub, which piece a tap picks from , the fade and zoom range from
// the screen. A plot lists each chunk as the index of one this app holds
// (the last plot applied) or the chunk itself; applying it is Plot.apply
// (law plot_delta_exact), so `held` always mirrors what the hub thinks
// this connection holds, index for index (a tap names a chunk by it).

// CBOR, as much as plots use:
// unsigned and negative ints, byte and text strings, arrays, maps. Keys 1
// and 31 are the hub's dictionary words "t" and "key".
enum Cbor {
    static func decode(_ d: Data) -> Any? {
        d.withUnsafeBytes { raw -> Any? in
            let b = raw.bindMemory(to: UInt8.self)
            var i = 0
            func arg(_ ai: UInt8) -> Int? {
                switch ai {
                case 0..<24: return Int(ai)
                case 24: guard i < b.count else { return nil }; i += 1; return Int(b[i - 1])
                case 25: guard i + 1 < b.count else { return nil }; i += 2; return Int(b[i - 2]) << 8 | Int(b[i - 1])
                case 26:
                    guard i + 3 < b.count else { return nil }
                    i += 4
                    return Int(b[i - 4]) << 24 | Int(b[i - 3]) << 16 | Int(b[i - 2]) << 8 | Int(b[i - 1])
                default: return nil
                }
            }
            func item() -> Any? {
                guard i < b.count else { return nil }
                let h = b[i]
                i += 1
                guard let n = arg(h & 31) else { return nil }
                switch h >> 5 {
                case 0: return n
                case 1: return -1 - n
                case 2:
                    guard i + n <= b.count else { return nil }
                    i += n
                    return Data(b[(i - n)..<i])
                case 3:
                    guard i + n <= b.count else { return nil }
                    i += n
                    return String(decoding: b[(i - n)..<i], as: UTF8.self)
                case 4:
                    var a: [Any] = []
                    a.reserveCapacity(n)
                    for _ in 0..<n { guard let x = item() else { return nil }; a.append(x) }
                    return a
                case 5:
                    var o: [String: Any] = [:]
                    for _ in 0..<n {
                        guard let k = item(), let v = item() else { return nil }
                        let key = (k as? Int).map { $0 == 1 ? "t" : $0 == 31 ? "key" : String($0) } ?? (k as? String ?? "")
                        o[key] = v
                    }
                    return o
                default: return nil
                }
            }
            return item()
        }
    }
}

// Varints (7 bits a byte, low first) and zigzag, as Enc writes them.
struct Varints {
    let b: [UInt8]
    var i = 0

    init(_ d: Data) { b = [UInt8](d) }

    private(set) var valid = true
    var more: Bool { valid && i < b.count }

    mutating func next() -> UInt32 {
        var acc: UInt32 = 0
        for index in 0..<5 {
            let shift = UInt32(index * 7)
            guard i < b.count else { valid = false; return 0 }
            let v = b[i]; i += 1
            guard shift < 28 || (v & 0xF0) == 0 else { valid = false; return 0 }
            acc |= UInt32(v & 127) << shift
            if v < 128 { return acc }
        }
        valid = false
        return 0
    }

    mutating func signed() -> Int32 {
        let z = next()
        return Int32(bitPattern: (z >> 1) ^ (0 &- (z & 1)))
    }
}

// One chunk, decoded once: capsules (ax ay bx by r, micrometres; a dot is
// a capsule of no length) and fill triangles (x y, three per triangle);
// and its pieces, for tap hit tests: each piece's info index (what the hub
// is asked), its capsules and triangles, and its box.
final class PlotChunk: @unchecked Sendable {
    struct Piece {
        let info: Int
        let caps: Range<Int>
        let tris: Range<Int>
        let box: SIMD4<Float>
    }

    let layer: Int
    let color: UInt32
    let alpha: Float
    // a tint (fills with a width): drawn in its own colour, not its layer's
    let own: Bool
    var caps: [Float] = []
    var tris: [Float] = []
    var pieces: [Piece] = []

    init(layer: Int, color: UInt32, alpha: Float, own: Bool = false) {
        self.layer = layer
        self.color = color
        self.alpha = alpha
        self.own = own
    }

    static func decode(_ o: [String: Any]) -> PlotChunk {
        let mode = o["m"] as? Int ?? 1
        let c = PlotChunk(layer: o["l"] as? Int ?? 0, color: UInt32(o["c"] as? Int ?? 0), alpha: Float(o["a"] as? Int ?? 255) / 255,
                          own: mode == 0 && (o["w"] as? Int ?? 0) != 0)
        let r = Float(o["w"] as? Int ?? 0) / 2
        var v = Varints(o["d"] as? Data ?? Data())
        var px: Int32 = 0, py: Int32 = 0
        func point() -> (Float, Float) {
            px = px &+ v.signed()
            py = py &+ v.signed()
            return (Float(px), Float(py))
        }
        while v.more {
            let info = Int(v.next())
            let c0 = c.caps.count / 5, t0 = c.tris.count / 6
            var pts: [Float] = []
            if mode == 2 {
                let (x, y) = point()
                pts = [x, y]
                c.caps += [x, y, x, y, r]
            } else {
                let n = Int(v.next())
                pts.reserveCapacity(n * 2)
                for _ in 0..<n where v.more { let (x, y) = point(); pts += [x, y] }
                if mode == 0 { fan(pts, into: &c.tris) } else { path(pts, r, into: &c.caps) }
            }
            var lo = SIMD2<Float>(.infinity, .infinity), hi = -lo
            for k in stride(from: 0, to: pts.count - 1, by: 2) {
                lo = simd_min(lo, SIMD2(pts[k], pts[k + 1]))
                hi = simd_max(hi, SIMD2(pts[k], pts[k + 1]))
            }
            let pad = mode == 0 ? 0 : r
            c.pieces.append(Piece(info: info, caps: c0..<(c.caps.count / 5), tris: t0..<(c.tris.count / 6),
                                  box: SIMD4(lo.x - pad, lo.y - pad, hi.x + pad, hi.y + pad)))
        }
        return c
    }

    static func path(_ p: [Float], _ r: Float, into caps: inout [Float]) {
        if p.count == 2 { caps += [p[0], p[1], p[0], p[1], r]; return }
        var k = 2
        while k + 1 < p.count {
            caps += [p[k - 2], p[k - 1], p[k], p[k + 1], r]
            k += 2
        }
    }

    // a polygon as a triangle fan, turned counter-clockwise, so the
    // stencil's winding count fills every polygon of a layer as one union
    static func fan(_ p: [Float], into tris: inout [Float]) {
        let n = p.count / 2
        guard n >= 3 else { return }
        var area: Float = 0
        for k in 0..<n {
            let j = (k + 1) % n
            area += p[2 * k] * p[2 * j + 1] - p[2 * j] * p[2 * k + 1]
        }
        for k in 1..<(n - 1) {
            let (a, b) = area >= 0 ? (k, k + 1) : (k + 1, k)
            tris += [p[0], p[1], p[2 * a], p[2 * a + 1], p[2 * b], p[2 * b + 1]]
        }
    }

    // how close a piece comes to (x, y), micrometres (0 inside a fill)
    func distance(_ p: Piece, _ x: Float, _ y: Float) -> Float {
        var best = Float.infinity
        var k = p.caps.lowerBound * 5
        while k < p.caps.upperBound * 5 {
            let a = SIMD2(caps[k], caps[k + 1]), b = SIMD2(caps[k + 2], caps[k + 3]), q = SIMD2(x, y)
            let ba = b - a
            let h = simd_clamp(simd_dot(q - a, ba) / max(simd_dot(ba, ba), 1e-6), 0, 1)
            best = min(best, simd_length(q - a - ba * h) - caps[k + 4])
            k += 5
        }
        // inside a polygon: its fan's signed triangles containing the point sum to non-zero
        var wind = 0
        k = p.tris.lowerBound * 6
        while k < p.tris.upperBound * 6 {
            let a = SIMD2(tris[k], tris[k + 1]), b = SIMD2(tris[k + 2], tris[k + 3]), c = SIMD2(tris[k + 4], tris[k + 5])
            func side(_ u: SIMD2<Float>, _ v: SIMD2<Float>) -> Float { (v.x - u.x) * (y - u.y) - (v.y - u.y) * (x - u.x) }
            let s0 = side(a, b), s1 = side(b, c), s2 = side(c, a)
            if (s0 >= 0 && s1 >= 0 && s2 >= 0) || (s0 <= 0 && s1 <= 0 && s2 <= 0) {
                let sign = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
                wind += sign >= 0 ? 1 : -1
            }
            k += 6
        }
        return wind != 0 ? 0 : best
    }
}

// A 3D model (Solid): triangles in board axes, micrometres, with a colour
// each; normals are the triangles' own.
struct PlotMesh: @unchecked Sendable {
    let verts: [Float]      // x y z nx ny nz, per vertex
    let colors: [UInt32]    // per vertex, 0xRRGGBB
    let box: [Float]        // x0 y0 z0 x1 y1 z1

    static func decode(_ o: [String: Any], cancelled: () -> Bool = { false }) throws -> PlotMesh {
        let n = o["n"] as? Int ?? 0
        guard n > 0, n <= (64 << 20) / 84, let bytes = o["mesh"] as? Data, bytes.count >= n * 10 else {
            throw NSError(domain: "PlotMesh", code: 1, userInfo: [NSLocalizedDescriptionKey: "Invalid or oversized model"])
        }
        let b = (o["box3"] as? [Int] ?? []).map { Float(Int32(truncatingIfNeeded: $0)) * 10 }
        var v = Varints(o["mesh"] as? Data ?? Data())
        // written in place: no array per triangle (a 45k-triangle board decodes in a few milliseconds on a phone)
        var verts = [Float](repeating: 0, count: n * 18), colors = [UInt32](repeating: 0, count: n * 3)
        var color: UInt32 = 0, px: Int32 = 0, py: Int32 = 0, pz: Int32 = 0
        for triangle in 0..<n {
            if triangle % 256 == 0 && cancelled() { throw CancellationError() }
            color ^= v.next()
            @inline(__always) func corner() -> SIMD3<Float> { px = px &+ v.signed(); py = py &+ v.signed(); pz = pz &+ v.signed(); return SIMD3(Float(px), Float(py), Float(pz)) * 10 }
            let a = corner(), b = corner(), c = corner()
            guard v.valid else { throw NSError(domain: "PlotMesh", code: 2, userInfo: [NSLocalizedDescriptionKey: "Incomplete model"]) }
            let nrm = simd_normalize(simd_cross(b - a, c - a))
            let nn = nrm.x.isNaN ? SIMD3<Float>(0, 0, 1) : nrm
            var at = triangle * 18
            for q in [a, b, c] { verts[at] = q.x; verts[at + 1] = q.y; verts[at + 2] = q.z; verts[at + 3] = nn.x; verts[at + 4] = nn.y; verts[at + 5] = nn.z; at += 6 }
            colors[triangle * 3] = color; colors[triangle * 3 + 1] = color; colors[triangle * 3 + 2] = color
        }
        guard !v.more, v.valid else { throw NSError(domain: "PlotMesh", code: 3, userInfo: [NSLocalizedDescriptionKey: "Unexpected model bytes"]) }
        return PlotMesh(verts: verts, colors: colors, box: b.count == 6 ? b : [])
    }
    /// A model's id: FNV-1a, 64 bits, of its wire bytes, as 16 hex digits (the hub's plot::mesh_id). `viewer.watch {kept}` names the
    /// cached model with it, and the hub sends the model only if it is another.
    static func id(_ d: Data) -> String {
        var h: UInt64 = 0xcbf29ce484222325
        for b in d { h = (h ^ UInt64(b)) &* 0x100000001b3 }
        return String(format: "%016llx", h)
    }
}

/// The last model each view showed (hub, thread, kind, path), as the hub sent it, in the app's caches: shown at once when the view opens
/// again, on another launch or offline, and not downloaded again while the hub's model is the same. The newest few are kept.
enum MeshDisk {
    private static let most = 8
    private static var dir: URL? {
        guard let c = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask).first else { return nil }
        let d = c.appendingPathComponent("models", isDirectory: true)
        try? FileManager.default.createDirectory(at: d, withIntermediateDirectories: true)
        return d
    }
    private static func file(_ slot: String) -> URL? { dir?.appendingPathComponent(PlotMesh.id(Data(slot.utf8)) + ".plist") }
    /// {key, n, box3, mesh}: the frame's own fields, so PlotMesh.decode reads it as it reads the wire
    static func load(_ slot: String) -> [String: Any]? {
        guard !slot.isEmpty, let f = file(slot), let d = try? Data(contentsOf: f),
              let o = try? PropertyListSerialization.propertyList(from: d, format: nil) as? [String: Any], o["mesh"] is Data, o["key"] is String else { return nil }
        try? FileManager.default.setAttributes([.modificationDate: Date()], ofItemAtPath: f.path)
        return o
    }
    static func save(_ slot: String, key: String, n: Int, box3: [Int], mesh: Data) {
        guard !slot.isEmpty, let f = file(slot), let dir,
              let d = try? PropertyListSerialization.data(fromPropertyList: ["key": key, "n": n, "box3": box3, "mesh": mesh] as [String: Any], format: .binary, options: 0) else { return }
        try? d.write(to: f, options: .atomic)
        let fs = (try? FileManager.default.contentsOfDirectory(at: dir, includingPropertiesForKeys: [.contentModificationDateKey])) ?? []
        let old = fs.sorted { ((try? $0.resourceValues(forKeys: [.contentModificationDateKey]).contentModificationDate) ?? .distantPast) > ((try? $1.resourceValues(forKeys: [.contentModificationDateKey]).contentModificationDate) ?? .distantPast) }.dropFirst(most)
        for u in old { try? FileManager.default.removeItem(at: u) }
    }
}

// What the renderer draws: the chunks in paint order and which of them
// just arrived (they fade in), the box, the board's rectangle (empty
// from an older hub, or x0 > x1 for a schematic) and thickness, or why
// there is nothing to draw. fam: the file the source is a version of
// (versions of one file share it; the key itself when the hub names none).
struct PlotFrame {
    let key: String
    let fam: String
    let box: [Float]
    let edge: [Float]
    let thick: Float
    let chunks: [PlotChunk]
    let fresh: Set<Int>
    let none: String
    let at: Date
}

struct MeshFrame {
    let key: String
    let mesh: PlotMesh?
    let none: String
    let at: Date
    var decoding = false
}

// Exact wire revision cache, independent of view lifetime. Locks protect reset,
// demultiplexing and the dedicated mesh queue; immutable geometry is shared.
private final class PlotMeshAssets: @unchecked Sendable {
    private struct Entry { let n: Int; let bytes: Data; let frame: MeshFrame }
    private let lock = NSLock()
    private var entries: [String: Entry] = [:]
    private var order: [String] = []
    private var tickets: [String: UInt64] = [:]
    private var serial: UInt64 = 0
    private var generation = 0
    func epoch() -> Int { lock.withLock { generation } }
    func reset() { lock.withLock { serial &+= 1; generation += 1; tickets = [:]; entries = [:]; order = [] } }
    func issue(_ key: String, epoch: Int) -> UInt64? { lock.withLock { guard generation == epoch else { return nil }; serial &+= 1; tickets.removeAll(keepingCapacity: true); tickets[key] = serial; return serial } }
    func current(_ key: String, _ ticket: UInt64) -> Bool { lock.withLock { tickets[key] == ticket } }
    func frame(_ key: String) -> MeshFrame? { lock.withLock { entries[key]?.frame } }
    func same(_ key: String, n: Int, bytes: Data) -> PlotMesh? {
        lock.withLock { guard let e = entries[key], e.n == n, e.bytes == bytes else { return nil }; return e.frame.mesh }
    }
    func put(_ key: String, n: Int, bytes: Data, mesh: PlotMesh, ticket: UInt64) {
        lock.withLock {
            guard tickets[key] == ticket else { return }
            entries[key] = Entry(n: n, bytes: bytes, frame: MeshFrame(key: key, mesh: mesh, none: "", at: Date()))
            order.removeAll { $0 == key }; order.append(key)
            func size() -> Int { entries.values.reduce(0) { $0 + $1.bytes.count + ($1.frame.mesh?.verts.count ?? 0) * 4 + ($1.frame.mesh?.colors.count ?? 0) * 4 } }
            while order.count > 4 || (size() > 64 << 20 && order.count > 1) { entries.removeValue(forKey: order.removeFirst()) }
        }
    }
}

@MainActor
@Observable
final class PlotStore {
    private(set) var frame: PlotFrame?
    private(set) var mesh: MeshFrame?
    @ObservationIgnored private var held: [PlotChunk] = []
    @ObservationIgnored private var splitFrames: [String: PlotFrame] = [:]
    @ObservationIgnored private var splitOrder: [String] = []
    @ObservationIgnored private let queue = DispatchQueue(label: "plot", qos: .userInitiated)
    private(set) var generation = 0
    @ObservationIgnored private let assets = PlotMeshAssets()
    /// the parts of the board model streaming in after it (PlotParts.swift)
    let parts = PartsStore()
    @ObservationIgnored private let meshQueue = DispatchQueue(label: "plot.mesh", qos: .userInitiated)
    /// The view the models now arriving are for (MeshDisk's slot), set by `cached`
    @ObservationIgnored private var slot = ""
    #if DEBUG && targetEnvironment(simulator)
    @ObservationIgnored private var qualificationDrop = ProcessInfo.processInfo.environment["BACKPLANE_QUALIFICATION"] == "1"
        ? ProcessInfo.processInfo.environment["BACKPLANE_QUALIFICATION_DROP_PLOTS"] ?? "" : ""
    #endif

    // A dedicated simulator may withhold incoming geometry to qualify actual
    // loading UX while the real socket remains healthy. Retry ends the drop.
    func resumeQualificationDelivery() {
        #if DEBUG && targetEnvironment(simulator)
        qualificationDrop = ""
        #endif
    }

    func cachedMesh(_ key: String) -> MeshFrame? { assets.frame(key) }

    // The view on show: the hub's keys of the plots it shows (`viewer.watch` answers them), nil while its watch is on its way.
    // A plot for another key came late for a view that was left and is dropped; one that comes before the answer waits for it.
    @ObservationIgnored private var accept: Set<String>? = []
    @ObservationIgnored private var parked: [(key: String, o: [String: Any]?, f: PlotFrame?)] = []
    /// This view's cached model (MeshDisk), drawn once the hub names its key among the view's
    @ObservationIgnored private var disk: [String: Any]?
    /// The latest watch (`begin`): only its answer opens the gate
    @ObservationIgnored private var watching = 0
    /// A 3D model's frames other than the model (`parts`, `part`: kicad's native board mesher streams the parts after the board), already
    /// gated to the view on show; the parts' decoding hooks in here. A `parts` frame may come alone when the phone's kept model was the board.
    @ObservationIgnored var more: ((String, [String: Any]) -> Void)?
    private func wanted(_ key: String) -> Bool? { accept.map { $0.contains(key) } }
    private func park(_ key: String, _ o: [String: Any]?, _ f: PlotFrame?) {
        // a newer plot or model replaces the parked one of its key; a model's parts all wait, in order
        parked.removeAll { $0.key == key && (($0.f != nil && f != nil) || ($0.o.map(PlotStore.isModel) == true && o.map(PlotStore.isModel) == true)) }
        parked.append((key, o, f))
    }

    /// A viewer switches (a tab, a file, a thread): what the last one showed goes at once, plots for it are dropped from now on, and the
    /// view's own cached model (`slot`: hub, thread, kind, path) waits to be named. Returns `viewer.watch {kept}` ("" for no cached model).
    /// `serial` names the watch: the answer of an older one (a viewer left before its watch was answered) is not this view's.
    func begin(_ slot: String) -> (kept: String, serial: Int) {
        self.slot = slot; watching += 1
        accept = nil; parked = []; frame = nil; mesh = nil; parts.reset() // the last view's parts go with its model
        _ = assets.issue("", epoch: assets.epoch()) // a model still decoding for the last view is not shown
        disk = slot.isEmpty ? nil : MeshDisk.load(slot)
        guard let o = disk, let key = o["key"] as? String, let bytes = o["mesh"] as? Data else { return ("", watching) }
        return (key + "\t" + PlotMesh.id(bytes), watching)
    }
    /// The hub named the view's plots: what waited for them is shown, the rest dropped; the cached model shows if it is of this view's file.
    func shown(_ keys: [String], of serial: Int) {
        guard serial == watching else { return } // the answer for a viewer that was left: its plots are not this one's
        accept = Set(keys)
        let waiting = parked; parked = []
        for p in waiting where accept!.contains(p.key) {
            if let f = p.f { frame = f; keepFrame() } else if let o = p.o { route(o) }
        }
        if let o = disk, let key = o["key"] as? String, accept!.contains(key), mesh?.key != key { showDisk(o, key) }
        disk = nil
    }
    /// The watch failed (no hub): the view's own cached model is all there is to show.
    func failed(of serial: Int) {
        guard serial == watching else { return }
        accept = []; parked = []
        if let o = disk, let key = o["key"] as? String { accept = [key]; showDisk(o, key) }
        disk = nil
    }
    /// The keys a view shows by the hub's rule (kicad/watch.rs `key`), for a hub that does not answer them.
    static func keys(kind: String, root: String, path: String) -> [String] {
        let p = path.isEmpty || path.hasPrefix("/") ? path : root + "/" + path
        let k = { (n: Int, q: String) in "\(n)|\(root)|\(q)" }
        switch kind {
        case "board": return [k(0, p)]
        case "schematic": return [k(1, p)]
        case "3d": return [k(2, p), k(0, p)]
        case "3d-bare": return [k(3, p), k(0, p)]
        case "3d-full": return [k(4, p), k(0, p)]
        case "split": return [k(1, p), k(0, ""), k(2, "")]
        default: return []
        }
    }
    private func showDisk(_ o: [String: Any], _ key: String) {
        guard let bytes = o["mesh"] as? Data, let ticket = assets.issue(key, epoch: assets.epoch()) else { return }
        let assets = self.assets, gen = generation
        meshQueue.async {
            guard assets.current(key, ticket), let m = try? PlotMesh.decode(o, cancelled: { !assets.current(key, ticket) }) else { return }
            assets.put(key, n: o["n"] as? Int ?? 0, bytes: bytes, mesh: m, ticket: ticket)
            DispatchQueue.main.async {
                MainActor.assumeIsolated {
                    guard gen == self.generation, assets.current(key, ticket) else { return }
                    self.mesh = MeshFrame(key: key, mesh: m, none: "", at: Date())
                }
            }
        }
    }
    func cachedFrame(_ key: String) -> PlotFrame? {
        if frame?.key == key { return frame }
        return splitFrames[key]
    }
    private func keepFrame() {
        guard let frame else { return }
        splitOrder.removeAll { $0 == frame.key }
        splitOrder.append(frame.key)
        splitFrames[frame.key] = frame
        while splitOrder.count > 3 { splitFrames.removeValue(forKey: splitOrder.removeFirst()) }
    }

    /// A frame of a board model's parts (`parts` starts them again, `part` is one model with its instances; the latter carries a `mesh` of
    /// its own, which is not the board's). A model of the board is a `mesh` that is not one of these.
    nonisolated static func isPart(_ o: [String: Any]) -> Bool { o["parts"] != nil || o["part"] != nil }
    nonisolated static func isModel(_ o: [String: Any]) -> Bool { o["mesh"] != nil && !isPart(o) }

    // a plot frame: a map whose first key is 1 ("t") and value "plot"
    static func isPlot(_ d: Data) -> Bool {
        d.count > 7 && d[d.startIndex] >= 0xA0 && d[d.startIndex] <= 0xB7 &&
            d[d.startIndex + 1] == 1 && [UInt8](d[(d.startIndex + 2)..<(d.startIndex + 7)]) == [0x64, 0x70, 0x6C, 0x6F, 0x74]
    }

    // a new connection holds nothing
    func reset() {
        generation += 1
        assets.reset()
        parts.reset()
        held = []
        splitFrames = [:]
        splitOrder = []
        frame = nil
        mesh = nil
        accept = []; parked = []
    }

    /// A plot as JSON (the data socket's own frames: `{t:"plot", key, none}`, why a view has nothing to show)
    func receiveJSON(_ o: [String: Any]) { guard o["none"] is String else { return }; route(o) }

    // decoded off the main thread, applied in arrival order
    func receive(_ d: Data) {
        let gen = generation
        #if DEBUG && targetEnvironment(simulator)
        let drop = qualificationDrop
        #else
        let drop = ""
        #endif
        queue.async {
            guard let o = Cbor.decode(d) as? [String: Any] else { return }
            let key = o["key"] as? String ?? ""
            let fam = (o["fam"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? key
            // a 3D model: with its parts (2) or the board alone (3)
            let solid = key.hasPrefix("2|") || key.hasPrefix("3|") || key.hasPrefix("4|")
            #if DEBUG && targetEnvironment(simulator)
            if ProcessInfo.processInfo.environment["BACKPLANE_QUALIFICATION"] == "1" {
                // Counts and a fixed kind only; no source paths, payloads or errors.
                let kind = solid ? "mesh" : "geometry"
                NSLog("Backplane simulator plot kind=%@ bytes=%lu meshBytes=%lu none=%@ generation=%ld",
                      kind, d.count, (o["mesh"] as? Data)?.count ?? 0,
                      String(o["none"] != nil), gen)
            }
            #endif
            guard drop != "all", !(drop == "mesh" && solid) else { return }
            // a note about the model (parts with no 3D model) is not a model
            if o["none"] is String || solid {
                // a model's other frames (its parts streaming in after it, `parts`/`part`) pass the same gate to `more`; a bare note does not
                if solid && o["none"] == nil && o["mesh"] == nil && !PlotStore.isPart(o) { return }
                DispatchQueue.main.async { MainActor.assumeIsolated { guard gen == self.generation else { return }; self.route(o) } }
                return
            }
            let refs = o["cs"] as? [Any] ?? []
            var cs: [PlotChunk?] = Array(repeating: nil, count: refs.count)
            var fresh = Set<Int>()
            // new chunks decode side by side
            let objs = refs.enumerated().compactMap { i, r in (r as? [String: Any]).map { (i, $0) } }
            let decoded = UnsafeMutableBufferPointer<PlotChunk?>.allocate(capacity: objs.count)
            decoded.initialize(repeating: nil)
            DispatchQueue.concurrentPerform(iterations: objs.count) { k in decoded[k] = PlotChunk.decode(objs[k].1) }
            for (k, (i, _)) in objs.enumerated() { cs[i] = decoded[k]; fresh.insert(i) }
            decoded.deallocate()
            let box = (o["box"] as? [Int] ?? []).map { Float($0) }
            let edge = (o["edge"] as? [Int] ?? []).map { Float($0) }
            let thick = Float(o["thick"] as? Int ?? 1600)
            DispatchQueue.main.async {
                MainActor.assumeIsolated {
                    guard gen == self.generation else { return }
                    for (i, r) in refs.enumerated() {
                        if let k = r as? Int, k >= 0, k < self.held.count { cs[i] = self.held[k] }
                    }
                    guard cs.allSatisfy({ $0 != nil }) else {
                        self.held = []
                        guard self.wanted(key) != false else { return }
                        self.frame = PlotFrame(key: key, fam: fam, box: [], edge: [], thick: 0, chunks: [], fresh: [], none: "Invalid geometry reference. Retry to reload this view.", at: Date())
                        self.keepFrame()
                        return
                    }
                    let all = cs.compactMap { $0 }
                    self.held = all // the hub's deltas count on what it sent, shown or not
                    let f = PlotFrame(key: key, fam: fam, box: box, edge: edge, thick: thick, chunks: all, fresh: fresh, none: "", at: Date())
                    switch self.wanted(key) {
                    case .some(true): self.frame = f; self.keepFrame()
                    case .none: self.park(key, nil, f)
                    case .some(false): break
                    }
                }
            }
        }
    }

    /// A model or a plot's absence, on the main thread: for the view on show, waiting for it, or dropped
    private func route(_ o: [String: Any]) {
        let key = o["key"] as? String ?? ""
        switch wanted(key) { case .none: park(key, o, nil); return; case .some(false): return; default: break }
        let fam = (o["fam"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? key
        let solid = key.hasPrefix("2|") || key.hasPrefix("3|") || key.hasPrefix("4|")
        if solid && PlotStore.isPart(o) { parts.receive(key, o); more?(key, o); return } // parts after (or without) the model: for the view on show only
        let assets = self.assets, meshQueue = self.meshQueue, gen = generation, slot = self.slot
        guard let ticket = solid ? assets.issue(key, epoch: assets.epoch()) : 0 else { return }
        let previous = solid ? assets.frame(key)?.mesh : nil
        if let why = o["none"] as? String {
            if solid { mesh = MeshFrame(key: key, mesh: previous, none: why + (previous == nil ? "" : " — previous saved revision shown"), at: Date()); return }
            held = []
            frame = PlotFrame(key: key, fam: fam, box: [], edge: [], thick: 0, chunks: [], fresh: [], none: why, at: Date())
            keepFrame()
            return
        }
        parts.boardChanged(key)
        mesh = MeshFrame(key: key, mesh: previous, none: "Received model; decoding…" + (previous == nil ? "" : " — previous saved revision shown"), at: Date(), decoding: true)
        meshQueue.async {
            guard assets.current(key, ticket) else { return }
            do {
                let bytes = o["mesh"] as? Data ?? Data(), n = o["n"] as? Int ?? 0
                let m = try assets.same(key, n: n, bytes: bytes) ?? PlotMesh.decode(o, cancelled: { !assets.current(key, ticket) })
                guard assets.current(key, ticket) else { return }
                assets.put(key, n: n, bytes: bytes, mesh: m, ticket: ticket)
                MeshDisk.save(slot, key: key, n: n, box3: o["box3"] as? [Int] ?? [], mesh: bytes)
                DispatchQueue.main.async {
                    MainActor.assumeIsolated {
                        guard gen == self.generation, assets.current(key, ticket) else { return }
                        self.mesh = MeshFrame(key: key, mesh: m, none: "", at: Date())
                    }
                }
            } catch {
                DispatchQueue.main.async {
                    MainActor.assumeIsolated {
                        guard gen == self.generation, assets.current(key, ticket) else { return }
                        self.mesh = MeshFrame(key: key, mesh: previous, none: "Unable to decode model" + (previous == nil ? "" : " — previous saved revision shown"), at: Date())
                    }
                }
            }
        }
    }
}
