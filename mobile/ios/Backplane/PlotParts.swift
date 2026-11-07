import Foundation
import Observation
import QuartzCore
import simd

// The parts of a board's 3D model (docs/PHONE-DATA.md, "Parts of a board's 3D model"): the bare board comes first, then one frame
// per part model with every instance of it. Each frame is a group that fades in over 300 ms from its arrival. Nothing is decided
// here beyond the geometry: the hub says which parts, where, and how many are still to come.

/// A part group's opacity `ms` after it arrived (solid.rs `fade_at`).
enum PartFade {
    static let ms: Double = 300
    static func alpha(_ elapsed: Double) -> Float { Float(min(max(elapsed / ms, 0), 1)) }
}

/// One frame's triangles, already in the board model's frame: x y z nx ny nz per vertex (micrometres), a colour per vertex.
struct PartGroup: @unchecked Sendable {
    let verts: [Float]
    let colors: [UInt32]
    /// CACurrentMediaTime() when it was applied (the fade starts here)
    var at: Double = 0
    var count: Int { colors.count }
}

enum PartFrame: Sendable {
    case start(of: Int, rev: String)
    case part(PartGroup, done: Int)

    private static let maxTriangles = (64 << 20) / 84

    /// A part frame's fields (`parts` or `part`/`n`/`mesh`/`at`/`done`), decoded; nil if it is not one or is malformed.
    static func parse(_ o: [String: Any], cancelled: () -> Bool = { false }) -> PartFrame? {
        if let p = o["parts"] as? [String: Any] {
            return .start(of: p["of"] as? Int ?? 0, rev: p["rev"] as? String ?? "")
        }
        guard o["part"] is String, let n = o["n"] as? Int, let mesh = o["mesh"] as? Data, let at = o["at"] as? Data else { return nil }
        guard let g = try? decode(n: n, mesh: mesh, at: at, cancelled: cancelled) else { return nil }
        return .part(g, done: o["done"] as? Int ?? 0)
    }

    /// The model's triangles (zigzag-delta varints, as a board's mesh) once per instance, each instance's 3x4 affine applied to the
    /// integers BEFORE the 10 µm → µm scale, as the board's own mesh goes through.
    static func decode(n: Int, mesh: Data, at: Data, cancelled: () -> Bool = { false }) throws -> PartGroup {
        func bad(_ s: String) -> Error { NSError(domain: "PlotParts", code: 1, userInfo: [NSLocalizedDescriptionKey: s]) }
        guard n > 0, n <= maxTriangles, mesh.count >= n * 10 else { throw bad("Invalid or oversized part") }
        guard !at.isEmpty, at.count % 48 == 0 else { throw bad("Invalid part placement") }
        let k = at.count / 48
        guard n * k <= maxTriangles else { throw bad("Too many part triangles") }
        var v = Varints(mesh)
        var raw = [SIMD3<Float>](), cols = [UInt32]()
        raw.reserveCapacity(n * 3); cols.reserveCapacity(n)
        var color: UInt32 = 0, px: Int32 = 0, py: Int32 = 0, pz: Int32 = 0
        for t in 0..<n {
            if t % 256 == 0 && cancelled() { throw CancellationError() }
            color ^= v.next()
            for _ in 0..<3 {
                px = px &+ v.signed(); py = py &+ v.signed(); pz = pz &+ v.signed()
                raw.append(SIMD3(Float(px), Float(py), Float(pz)))
            }
            guard v.valid else { throw bad("Incomplete part") }
            cols.append(color)
        }
        guard !v.more, v.valid else { throw bad("Unexpected part bytes") }
        var m = [Float](repeating: 0, count: k * 12)
        at.withUnsafeBytes { b in
            for i in 0..<(k * 12) { m[i] = Float(bitPattern: UInt32(littleEndian: b.loadUnaligned(fromByteOffset: i * 4, as: UInt32.self))) }
        }
        guard m.allSatisfy({ $0.isFinite }) else { throw bad("Invalid part placement") }
        var verts = [Float](repeating: 0, count: n * k * 18), colors = [UInt32](repeating: 0, count: n * k * 3)
        var out = 0, cout = 0
        for inst in 0..<k {
            let a = inst * 12
            @inline(__always) func place(_ p: SIMD3<Float>) -> SIMD3<Float> {
                SIMD3(m[a] * p.x + m[a + 1] * p.y + m[a + 2] * p.z + m[a + 3],
                      m[a + 4] * p.x + m[a + 5] * p.y + m[a + 6] * p.z + m[a + 7],
                      m[a + 8] * p.x + m[a + 9] * p.y + m[a + 10] * p.z + m[a + 11]) * 10
            }
            for t in 0..<n {
                let p0 = place(raw[3 * t]), p1 = place(raw[3 * t + 1]), p2 = place(raw[3 * t + 2])
                let nrm = simd_normalize(simd_cross(p1 - p0, p2 - p0))
                let nn = nrm.x.isNaN ? SIMD3<Float>(0, 0, 1) : nrm
                for q in [p0, p1, p2] {
                    verts[out] = q.x; verts[out + 1] = q.y; verts[out + 2] = q.z
                    verts[out + 3] = nn.x; verts[out + 4] = nn.y; verts[out + 5] = nn.z
                    out += 6
                }
                colors[cout] = cols[t]; colors[cout + 1] = cols[t]; colors[cout + 2] = cols[t]; cout += 3
            }
        }
        return PartGroup(verts: verts, colors: colors)
    }
}

/// What is held for one model key: the parts so far, in arrival order.
struct PartsFrame {
    let key: String
    let rev: String
    let of: Int
    var done = 0
    var groups: [PartGroup] = []
    /// changes with each reset: the renderer starts its buffer again when it differs
    let epoch: Int
    /// "parts n/N" while some are still to come, nothing when all are (or none were asked for)
    var progress: String { PartsFrame.progress(done: done, of: of) }
    static func progress(done: Int, of: Int) -> String { of == 0 || done >= of ? "" : "parts \(done)/\(of)" }
}

/// Decodes off the main thread, in arrival order, and applies on the main thread. Reset on a `parts` frame, on a new board mesh and
/// when the viewer is switched (`reset`).
@MainActor
@Observable
final class PartsStore {
    private(set) var frame: PartsFrame?
    @ObservationIgnored private let queue = DispatchQueue(label: "plot.parts", qos: .userInitiated)
    @ObservationIgnored private var generation = 0
    @ObservationIgnored private var epochs = 0

    /// The parts held for `key` (nil for another view's)
    func parts(_ key: String) -> PartsFrame? { frame?.key == key ? frame : nil }

    func reset() { generation += 1; frame = nil }

    /// A new board mesh under `key`: its parts are of the old board
    func boardChanged(_ key: String) {
        let gen = generation
        queue.async { DispatchQueue.main.async { MainActor.assumeIsolated {
            guard gen == self.generation, self.frame?.key == key else { return }
            self.frame = nil
        } } }
    }

    /// A `parts` or `part` frame under `key` (already a decoded CBOR map)
    func receive(_ key: String, _ o: [String: Any]) {
        let gen = generation
        let box = UncheckedBox(o)
        queue.async {
            let parsed = PartFrame.parse(box.value, cancelled: { false })
            DispatchQueue.main.async { MainActor.assumeIsolated {
                guard gen == self.generation, let parsed else { return }
                self.apply(key, parsed)
            } }
        }
    }

    func apply(_ key: String, _ f: PartFrame) {
        switch f {
        case .start(let of, let rev):
            epochs += 1
            frame = PartsFrame(key: key, rev: rev, of: of, epoch: epochs)
        case .part(var g, let done):
            // a part of a model whose `parts` start was not seen, or of another view, is dropped
            guard var p = frame, p.key == key else { return }
            g.at = CACurrentMediaTime()
            p.groups.append(g)
            p.done = done
            frame = p
        }
    }
}

private struct UncheckedBox: @unchecked Sendable { let value: [String: Any]; init(_ v: [String: Any]) { value = v } }
