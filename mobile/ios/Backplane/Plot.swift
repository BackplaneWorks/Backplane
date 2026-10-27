import Foundation
import Observation
import simd

// Plots from the hub (src/core/plot.bend) as GPU-ready geometry. Nothing
// is decided here: which chunks, their order, colour and opacity come from
// the hub, which piece a tap picks from Bend, the fade and zoom range from
// the screen. A plot lists each chunk as the index of one this app holds
// (the last plot applied) or the chunk itself; applying it is Plot.apply
// (law plot_delta_exact), so `held` always mirrors what the hub thinks
// this connection holds, index for index (a tap names a chunk by it).

// CBOR, as much as plots use (core/plot.bend writes them straight):
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
        var verts: [Float] = [], colors: [UInt32] = []
        verts.reserveCapacity(n * 18)
        colors.reserveCapacity(n * 3)
        var color: UInt32 = 0, px: Int32 = 0, py: Int32 = 0, pz: Int32 = 0
        for triangle in 0..<n {
            if triangle % 256 == 0 && cancelled() { throw CancellationError() }
            color ^= v.next()
            var p: [SIMD3<Float>] = []
            for _ in 0..<3 {
                px = px &+ v.signed(); py = py &+ v.signed(); pz = pz &+ v.signed()
                p.append(SIMD3(Float(px), Float(py), Float(pz)) * 10)
            }
            guard v.valid else { throw NSError(domain: "PlotMesh", code: 2, userInfo: [NSLocalizedDescriptionKey: "Incomplete model"]) }
            let nrm = simd_normalize(simd_cross(p[1] - p[0], p[2] - p[0]))
            let nn = nrm.x.isNaN ? SIMD3<Float>(0, 0, 1) : nrm
            for q in p { verts += [q.x, q.y, q.z, nn.x, nn.y, nn.z]; colors.append(color) }
        }
        guard !v.more, v.valid else { throw NSError(domain: "PlotMesh", code: 3, userInfo: [NSLocalizedDescriptionKey: "Unexpected model bytes"]) }
        return PlotMesh(verts: verts, colors: colors, box: b.count == 6 ? b : [])
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
    func issue(_ key: String, epoch: Int) -> UInt64? { lock.withLock { guard generation == epoch else { return nil }; serial &+= 1; tickets[key] = serial; return serial } }
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
    @ObservationIgnored private let queue = DispatchQueue(label: "plot", qos: .userInitiated)
    @ObservationIgnored private var generation = 0
    @ObservationIgnored private let assets = PlotMeshAssets()
    @ObservationIgnored private let meshQueue = DispatchQueue(label: "plot.mesh", qos: .userInitiated)

    func cachedMesh(_ key: String) -> MeshFrame? { assets.frame(key) }

    // a plot frame: a map whose first key is 1 ("t") and value "plot"
    static func isPlot(_ d: Data) -> Bool {
        d.count > 7 && d[d.startIndex] >= 0xA0 && d[d.startIndex] <= 0xB7 &&
            d[d.startIndex + 1] == 1 && [UInt8](d[(d.startIndex + 2)..<(d.startIndex + 7)]) == [0x64, 0x70, 0x6C, 0x6F, 0x74]
    }

    // a new connection holds nothing
    func reset() {
        generation += 1
        assets.reset()
        held = []
        frame = nil
        mesh = nil
    }

    // decoded off the main thread, applied in arrival order
    func receive(_ d: Data) {
        let gen = generation
        let assets = self.assets, meshQueue = self.meshQueue, epoch = assets.epoch()
        queue.async {
            guard let o = Cbor.decode(d) as? [String: Any] else { return }
            let key = o["key"] as? String ?? ""
            let fam = (o["fam"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? key
            // a 3D model: with its parts (2) or the board alone (3)
            let solid = key.hasPrefix("2|") || key.hasPrefix("3|") || key.hasPrefix("4|")
            if let why = o["none"] as? String {
                guard let ticket = solid ? assets.issue(key, epoch: epoch) : 0 else { return }
                DispatchQueue.main.async {
                    MainActor.assumeIsolated {
                        guard gen == self.generation else { return }
                        if solid {
                            guard assets.current(key, ticket) else { return }
                            let previous = assets.frame(key)?.mesh
                            self.mesh = MeshFrame(key: key, mesh: previous, none: why + (previous == nil ? "" : " — previous saved revision shown"), at: Date()); return
                        }
                        self.held = []
                        self.frame = PlotFrame(key: key, fam: fam, box: [], edge: [], thick: 0, chunks: [], fresh: [], none: why, at: Date())
                    }
                }
                return
            }
            // a note about the model (parts with no 3D model) is not a model
            if solid, o["mesh"] == nil { return }
            if solid {
                guard let ticket = assets.issue(key, epoch: epoch) else { return }
                let previous = assets.frame(key)?.mesh
                DispatchQueue.main.async {
                    MainActor.assumeIsolated {
                        guard gen == self.generation, assets.current(key, ticket) else { return }
                        self.mesh = MeshFrame(key: key, mesh: previous, none: "Received model; decoding…" + (previous == nil ? "" : " — previous saved revision shown"), at: Date())
                    }
                }
                meshQueue.async {
                    guard assets.current(key, ticket) else { return }
                    do {
                        let bytes = o["mesh"] as? Data ?? Data(), n = o["n"] as? Int ?? 0
                        let m = try assets.same(key, n: n, bytes: bytes) ?? PlotMesh.decode(o, cancelled: { !assets.current(key, ticket) })
                        guard assets.current(key, ticket) else { return }
                        assets.put(key, n: n, bytes: bytes, mesh: m, ticket: ticket)
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
                        if let k = r as? Int, k < self.held.count { cs[i] = self.held[k] }
                    }
                    let all = cs.compactMap { $0 }
                    self.held = all
                    self.frame = PlotFrame(key: key, fam: fam, box: box, edge: edge, thick: thick, chunks: all, fresh: fresh, none: "", at: Date())
                }
            }
        }
    }
}
