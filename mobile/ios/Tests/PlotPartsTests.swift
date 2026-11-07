// The parts of a board's 3D model against hand-built frames. Compiled with Plot.swift and PlotParts.swift alone, outside the app:
// scripts/ios-parts-test.sh.
import Foundation

var failed = 0, passed = 0
func check(_ ok: Bool, _ name: String, _ got: Any = "") { if ok { passed += 1 } else { failed += 1; print("FAIL \(name): got \(got)") } }

// ---- encoders, as the hub's plot.rs writes them ----
func head(_ o: inout [UInt8], _ major: UInt8, _ n: Int) {
    if n < 24 { o.append(major << 5 | UInt8(n)) }
    else if n < 256 { o += [major << 5 | 24, UInt8(n)] }
    else if n < 65536 { o += [major << 5 | 25, UInt8(n >> 8), UInt8(n & 255)] }
    else { o += [major << 5 | 26, UInt8(n >> 24), UInt8((n >> 16) & 255), UInt8((n >> 8) & 255), UInt8(n & 255)] }
}
func text(_ o: inout [UInt8], _ s: String) { head(&o, 3, s.utf8.count); o += Array(s.utf8) }
func bytes(_ o: inout [UInt8], _ b: [UInt8]) { head(&o, 2, b.count); o += b }
func varint(_ o: inout [UInt8], _ v: UInt32) { var v = v; while v >= 128 { o.append(UInt8(v & 127) | 128); v >>= 7 }; o.append(UInt8(v)) }
func zig(_ v: Int32) -> UInt32 { UInt32(bitPattern: (v << 1) ^ (v >> 31)) }
/// triangles as colour-xor then three corners, each a zigzag delta from the last point
func mesh(_ tris: [(UInt32, [[Int32]])]) -> [UInt8] {
    var o: [UInt8] = [], color: UInt32 = 0, p: [Int32] = [0, 0, 0]
    for (c, corners) in tris {
        varint(&o, color ^ c); color = c
        for q in corners { for a in 0..<3 { varint(&o, zig(q[a] &- p[a])); p[a] = q[a] } }
    }
    return o
}
func floats(_ m: [[Float]]) -> [UInt8] { m.flatMap { $0.flatMap { f in withUnsafeBytes(of: f.bitPattern.littleEndian) { Array($0) } } } }
func partsFrame(_ key: String, of: Int, rev: String) -> Data {
    var o: [UInt8] = []; head(&o, 5, 3); head(&o, 0, 1); text(&o, "plot"); head(&o, 0, 31); text(&o, key)
    text(&o, "parts"); head(&o, 5, 2); text(&o, "of"); head(&o, 0, of); text(&o, "rev"); text(&o, rev)
    return Data(o)
}
func partFrame(_ key: String, id: String, n: Int, mesh m: [UInt8], at: [UInt8], refs: [String], done: Int) -> Data {
    var o: [UInt8] = []; head(&o, 5, 8); head(&o, 0, 1); text(&o, "plot"); head(&o, 0, 31); text(&o, key)
    text(&o, "part"); text(&o, id); text(&o, "n"); head(&o, 0, n); text(&o, "mesh"); bytes(&o, m); text(&o, "at"); bytes(&o, at)
    text(&o, "refs"); head(&o, 4, refs.count); for r in refs { text(&o, r) }; text(&o, "done"); head(&o, 0, done)
    return Data(o)
}
let tri = mesh([(0xff0000, [[0, 0, 0], [100, 0, 0], [0, 100, 0]])])
let identity: [Float] = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0]

// ---- the fade ----
check(PartFade.alpha(0) == 0 && PartFade.alpha(150) == 0.5 && PartFade.alpha(300) == 1 && PartFade.alpha(900) == 1 && PartFade.alpha(-5) == 0, "fade 0..1 over 300 ms")
check(PartsFrame.progress(done: 3, of: 10) == "parts 3/10", "progress while streaming", PartsFrame.progress(done: 3, of: 10))
check(PartsFrame.progress(done: 10, of: 10) == "" && PartsFrame.progress(done: 0, of: 0) == "", "no progress when done or none")

// ---- decode: the affine takes the integers, then 10 µm → µm ----
do {
    let g = try PartFrame.decode(n: 1, mesh: Data(tri), at: Data(floats([identity.enumerated().map { $0.offset == 3 ? 10 : $0.offset == 7 ? 20 : $0.offset == 11 ? 30 : $0.element }])))
    check(g.count == 3 && g.colors == [0xff0000, 0xff0000, 0xff0000], "one triangle, its colour per vertex", g.colors)
    check(Array(g.verts[0..<3]) == [100, 200, 300] && Array(g.verts[6..<9]) == [1100, 200, 300] && Array(g.verts[12..<15]) == [100, 1200, 300], "translated then scaled by 10", g.verts)
    check(abs(g.verts[5] - 1) < 1e-5 && g.verts[3] == 0, "flat normal", Array(g.verts[3..<6]))
    // two instances: the second doubled and shifted in x
    let two = try PartFrame.decode(n: 1, mesh: Data(tri), at: Data(floats([identity, [2, 0, 0, 5, 0, 2, 0, 0, 0, 0, 2, 0]])))
    check(two.count == 6, "an instance each", two.count)
    check(Array(two.verts[18..<21]) == [50, 0, 0] && Array(two.verts[24..<27]) == [2050, 0, 0] && Array(two.verts[30..<33]) == [50, 2000, 0], "second instance's affine", Array(two.verts[18...]))
    // a mirror turns the triangle over, the normal with it
    let mirror = try PartFrame.decode(n: 1, mesh: Data(tri), at: Data(floats([[-1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0]])))
    check(abs(mirror.verts[5] + 1) < 1e-5, "mirrored: normal turned over", mirror.verts[5])
} catch { check(false, "decode threw", error) }
func throwsDecode(_ n: Int, _ m: [UInt8], _ at: [UInt8]) -> Bool { (try? PartFrame.decode(n: n, mesh: Data(m), at: Data(at))) == nil }
check(throwsDecode(1, tri, Array(floats([identity]).dropLast())), "a placement that is not 48 bytes an instance is refused")
check(throwsDecode(1, tri, []), "no instance is refused")
check(throwsDecode(2, tri, floats([identity])), "a mesh shorter than n is refused")
check(throwsDecode(1, tri + [1], floats([identity])), "bytes left over are refused")
check(throwsDecode(1, tri, floats([[Float.nan] + identity.dropFirst()])), "a non-finite placement is refused")
check(throwsDecode(0, tri, floats([identity])), "no triangles is refused")

// ---- frames through the CBOR reader, and the held list ----
let key = "2|root|a.kicad_pcb"
MainActor.assumeIsolated {
    let store = PartsStore()
    @MainActor func feed(_ d: Data) { if let o = Cbor.decode(d) as? [String: Any], let f = PartFrame.parse(o) { store.apply((o["key"] as? String) ?? "", f) } }
    check(Cbor.decode(partsFrame(key, of: 2, rev: "r1")) != nil, "frame reads")
    feed(partFrame(key, id: "0123456789abcdef", n: 1, mesh: tri, at: floats([identity]), refs: ["R1"], done: 1))
    check(store.parts(key) == nil, "a part with no start is dropped")
    feed(partsFrame(key, of: 2, rev: "r1"))
    check(store.parts(key)?.of == 2 && store.parts(key)?.groups.isEmpty == true && store.parts(key)?.rev == "r1", "start holds nothing yet")
    check(store.parts(key)?.progress == "parts 0/2", "progress at start", store.parts(key)?.progress ?? "nil")
    feed(partFrame(key, id: "0123456789abcdef", n: 1, mesh: tri, at: floats([identity, identity]), refs: ["R1", "R2"], done: 2))
    check(store.parts(key)?.groups.count == 1 && store.parts(key)?.groups[0].count == 6 && store.parts(key)?.done == 2, "a frame is one group of both instances", store.parts(key)?.done ?? -1)
    check(store.parts(key)?.groups[0].at ?? 0 > 0, "arrival time stamped")
    check(store.parts("2|root|other") == nil, "another view's parts are not shown")
    let e = store.parts(key)!.epoch
    feed(partsFrame(key, of: 1, rev: "r2"))
    check(store.parts(key)?.groups.isEmpty == true && store.parts(key)!.epoch != e && store.parts(key)?.rev == "r2", "a new start drops the held parts", store.parts(key)?.groups.count ?? -1)
    feed(partFrame("2|root|other", id: "b123456789abcdef", n: 1, mesh: tri, at: floats([identity]), refs: ["U1"], done: 1))
    check(store.parts(key)?.groups.isEmpty == true, "a part under another key does not join")
    store.reset()
    check(store.frame == nil, "reset (a viewer switch) holds nothing")
}

print("plot parts: \(passed) passed, \(failed) failed"); exit(failed == 0 ? 0 : 1)
