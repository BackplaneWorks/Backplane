import SwiftUI

// A bot's cat, drawn from its rig. The
// rig decides everything: parts back to front in a 64 x 64 box, path data
// (absolute M L C Q Z), even-odd fill and/or a round outline, opacity, and
// animations applied outer first. The math is the rig's, as the window and
// the web do it:
//   t = ((ms - delay) mod per) / per; w(x) = (1 - cos(2 pi x)) / 2
//   io: w(t); blink: 0 below 0.94, else w((t - 0.94) / 0.06); saw: t
//   value = from + (to - from) * e

struct CatAnim: Decodable {
    let k: String
    let from: Double
    let to: Double
    let px: Double
    let py: Double
    let per: Int
    let delay: Int
    let ease: String

    func value(_ ms: Double) -> Double {
        let per = max(self.per, 1)
        let off = (per - delay % per) % per
        let t = (ms + Double(off)).truncatingRemainder(dividingBy: Double(per)) / Double(per)
        func w(_ x: Double) -> Double { (1 - cos(2 * .pi * x)) / 2 }
        let e: Double
        switch ease {
        case "saw": e = t
        case "blink": e = t < 0.94 ? 0 : w((t - 0.94) / 0.06)
        default: e = w(t)
        }
        return from + (to - from) * e
    }
}

struct CatPart: Decodable {
    let name: String
    let d: String
    let fill: Int?
    let stroke: Int?
    let sw: Double
    let alpha: Int
    let anims: [CatAnim]
}

func catRig(_ json: Data) -> [CatPart] {
    (try? JSONDecoder().decode([CatPart].self, from: json)) ?? []
}

// path data: absolute M L C Q Z only
func catPath(_ d: String) -> Path {
    var toks: [Substring] = []
    var i = d.startIndex
    while i < d.endIndex {
        let c = d[i]
        if "MLCQZ".contains(c) {
            toks.append(d[i...i]); i = d.index(after: i)
        } else if c == "-" || c == "." || c.isNumber {
            var j = d.index(after: i)
            while j < d.endIndex, d[j] == "." || d[j].isNumber { j = d.index(after: j) }
            toks.append(d[i..<j]); i = j
        } else {
            i = d.index(after: i)
        }
    }
    var p = Path()
    var k = 0
    var cmd: Character = "M"
    func num() -> CGFloat {
        defer { k += 1 }
        return k < toks.count ? CGFloat(Double(toks[k]) ?? 0) : 0
    }
    func pt() -> CGPoint { let x = num(); return CGPoint(x: x, y: num()) }
    while k < toks.count {
        if let c = toks[k].first, c.isLetter { cmd = c; k += 1 }
        switch cmd {
        case "M": p.move(to: pt()); cmd = "L"
        case "L": p.addLine(to: pt())
        case "C": let a = pt(), b = pt(); p.addCurve(to: pt(), control1: a, control2: b)
        case "Q": let a = pt(); p.addQuadCurve(to: pt(), control: a)
        case "Z": p.closeSubpath(); cmd = " "
        default: k += 1
        }
    }
    return p
}

private func catColor(_ c: Int) -> Color {
    Color(red: Double((c >> 16) & 255) / 255, green: Double((c >> 8) & 255) / 255, blue: Double(c & 255) / 255)
}

// A cat of size x size. It animates while any part moves, unless the
// system asks for reduced motion (then it holds its first pose).
struct CatView: View {
    let rig: [CatPart]
    let size: CGFloat
    private let paths: [Path]
    @State private var start = Date()
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    init(rig: [CatPart], size: CGFloat) {
        self.rig = rig
        self.size = size
        self.paths = rig.map { catPath($0.d) }
    }

    private var moving: Bool { !reduceMotion && rig.contains { !$0.anims.isEmpty } }

    var body: some View {
        TimelineView(.animation(paused: !moving)) { tl in
            let ms = moving ? tl.date.timeIntervalSince(start) * 1000 : 0
            Canvas { ctx, sz in
                let s = min(sz.width, sz.height) / 64
                ctx.scaleBy(x: s, y: s)
                for (i, p) in rig.enumerated() {
                    var op = Double(p.alpha) / 255
                    for a in p.anims where a.k == "op" { op *= a.value(ms) }
                    if op <= 0 { continue }
                    var c = ctx
                    for a in p.anims {
                        let v = a.value(ms)
                        switch a.k {
                        case "rot":
                            c.translateBy(x: a.px, y: a.py); c.rotate(by: .degrees(v)); c.translateBy(x: -a.px, y: -a.py)
                        case "tx": c.translateBy(x: v, y: 0)
                        case "ty": c.translateBy(x: 0, y: v)
                        case "sx":
                            c.translateBy(x: a.px, y: a.py); c.scaleBy(x: v, y: 1); c.translateBy(x: -a.px, y: -a.py)
                        case "sy":
                            c.translateBy(x: a.px, y: a.py); c.scaleBy(x: 1, y: v); c.translateBy(x: -a.px, y: -a.py)
                        default: break
                        }
                    }
                    c.opacity = min(max(op, 0), 1)
                    if let f = p.fill {
                        c.fill(paths[i], with: .color(catColor(f)), style: FillStyle(eoFill: true))
                    }
                    if let l = p.stroke {
                        c.stroke(paths[i], with: .color(catColor(l)), style: StrokeStyle(lineWidth: p.sw, lineCap: .round, lineJoin: .round))
                    }
                }
            }
        }
        .frame(width: size, height: size)
    }
}
