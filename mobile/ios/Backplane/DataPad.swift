import SwiftUI
import UIKit

// The drawing pad on the phone. The model is the desktop's native/gpui/src/screens/pad.rs gesture for gesture, and Android's DataPad.kt:
// boxes dragged out, moved, resized and given typed lengths, each new one taking the next colour; a pen and an eraser. Coordinates are the paper's
// own pixels (padW x padH). `padPaint` draws the screen and the attachment, so what is attached is what was drawn. A finger is wider than a mouse,
// so the view sets `edge` and `eraser` from its scale. Attach hands a JPEG to the composer's pictures, like one from the photo library.

let padW: CGFloat = 960, padH: CGFloat = 600
let padUnit: CGFloat = 4 // paper pixels per unit of a typed length
let padColors: [UInt32] = [0xe5484d, 0xf5a524, 0x3e9b4f, 0x2f80ed, 0x8e4ec6, 0x12a594, 0x1f2328]
private let padMin: CGFloat = 6 // a smaller drag is a tap, not a box
private let padStep: CGFloat = 3 // the largest gap between a stroke's points, so the eraser can cut it anywhere
private let padInk: UInt32 = 0x1f2328

enum PadTool: CaseIterable { case pen, boxes, eraser }
/// Which part of an item a point is on: a side, or its middle.
enum PadPart { case l, r, t, b, inside }
struct PadBox: Equatable { var x, y, w, h: CGFloat; var color: UInt32; var dimW = ""; var dimH = "" }
struct PadStroke: Equatable { var pts: [CGPoint]; var color: UInt32 }
enum PadItem: Equatable { case box(PadBox), stroke(PadStroke) }
/// A typed length: 40 and "mm".
struct PadLength: Equatable { var n: CGFloat; var unit: String }

enum PadMath {
    static func hit(_ r: PadBox, _ x: CGFloat, _ y: CGFloat, edge: CGFloat) -> PadPart? {
        if x < r.x - edge || x > r.x + r.w + edge || y < r.y - edge || y > r.y + r.h + edge { return nil }
        let inX = x >= r.x && x <= r.x + r.w, inY = y >= r.y && y <= r.y + r.h
        let sides: [(PadPart, Bool, CGFloat)] = [(.l, inY, abs(x - r.x)), (.r, inY, abs(x - r.x - r.w)), (.t, inX, abs(y - r.y)), (.b, inX, abs(y - r.y - r.h))]
        if let best = sides.filter({ $0.1 }).min(by: { $0.2 < $1.2 }), best.2 <= edge { return best.0 }
        return x > r.x && x < r.x + r.w && y > r.y && y < r.y + r.h ? .inside : nil
    }
    /// The topmost item under a point, and the part of it.
    static func pick(_ items: [PadItem], _ x: CGFloat, _ y: CGFloat, edge: CGFloat) -> (Int, PadPart)? {
        for i in items.indices.reversed() {
            switch items[i] {
            case .box(let b): if let p = hit(b, x, y, edge: edge) { return (i, p) }
            case .stroke(let s): if s.pts.contains(where: { abs($0.x - x) <= edge && abs($0.y - y) <= edge }) { return (i, .inside) }
            }
        }
        return nil
    }
    /// "40", "40 mm", "12.5cm"; anything else is none.
    static func parseLength(_ text: String) -> PadLength? {
        let t = text.trimmingCharacters(in: .whitespaces)
        let end = t.firstIndex { !$0.isASCII || !($0.isNumber || $0 == ".") } ?? t.endIndex
        let num = String(t[..<end]), unit = String(t[end...]).trimmingCharacters(in: .whitespaces)
        if num.isEmpty || num.hasPrefix(".") || num.hasSuffix(".") || unit.count > 4 || !unit.allSatisfy({ ($0.isASCII && $0.isLetter) || $0 == "µ" || $0 == "\"" || $0 == "'" }) { return nil }
        guard let n = Double(num), n > 0 else { return nil }
        return PadLength(n: CGFloat(n), unit: unit)
    }
    private static func label(_ n: CGFloat) -> String { n == n.rounded() ? String(Int(n)) : String(Double(n)) }
    /// A box with one side's length set: the side's axis takes n units, the opposite side stays, and the label is kept for the picture.
    static func withLength(_ r: PadBox, _ side: PadPart, _ len: PadLength) -> PadBox {
        let px = max((len.n * padUnit).rounded(), padMin)
        let l = len.unit.isEmpty ? label(len.n) : "\(label(len.n)) \(len.unit)"
        var o = r
        switch side {
        case .l: o.x = r.x + r.w - px; o.w = px; o.dimW = l
        case .r: o.w = px; o.dimW = l
        case .t: o.y = r.y + r.h - px; o.h = px; o.dimH = l
        case .b: o.h = px; o.dimH = l
        case .inside: break
        }
        return o
    }
    /// A side dragged to (x, y): the box with that side there, never flipped inside out; a typed length on that axis no longer holds.
    static func resize(_ r: PadBox, _ side: PadPart, _ x: CGFloat, _ y: CGFloat) -> PadBox {
        var o = r
        switch side {
        case .l: let right = r.x + r.w; o.x = min(x, right - padMin); o.w = right - o.x; o.dimW = ""
        case .r: o.w = max(x - r.x, padMin); o.dimW = ""
        case .t: let bottom = r.y + r.h; o.y = min(y, bottom - padMin); o.h = bottom - o.y; o.dimH = ""
        case .b: o.h = max(y - r.y, padMin); o.dimH = ""
        case .inside: break
        }
        return o
    }
    static func bounds(_ it: PadItem) -> CGRect {
        switch it {
        case .box(let b): return CGRect(x: b.x, y: b.y, width: b.w, height: b.h)
        case .stroke(let s):
            let xs = s.pts.map(\.x), ys = s.pts.map(\.y)
            return CGRect(x: xs.min()!, y: ys.min()!, width: xs.max()! - xs.min()!, height: ys.max()! - ys.min()!)
        }
    }
    /// The points from `a` to `b`, `b` included, no further apart than padStep.
    static func walk(_ a: CGPoint, _ b: CGPoint) -> [CGPoint] {
        let n = max(Int((hypot(b.x - a.x, b.y - a.y) / padStep).rounded(.up)), 1)
        return (1...n).map { i in let t = CGFloat(i) / CGFloat(n); return CGPoint(x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t) }
    }
    /// What is left after the eraser touched (x, y): a box it touches goes, a stroke is cut around it. Nil when nothing changed.
    static func erased(_ items: [PadItem], _ x: CGFloat, _ y: CGFloat, radius: CGFloat) -> [PadItem]? {
        let touched = { (p: CGPoint) in hypot(p.x - x, p.y - y) <= radius }
        var out: [PadItem] = [], changed = false
        for it in items {
            switch it {
            case .box(let r) where x >= r.x - radius && x <= r.x + r.w + radius && y >= r.y - radius && y <= r.y + r.h + radius: changed = true
            case .stroke(let s) where s.pts.contains(where: touched):
                changed = true
                // what stays is the runs of points the eraser missed; a lone point is a crumb, not a stroke
                for run in s.pts.split(whereSeparator: touched) where run.count > 1 { out.append(.stroke(PadStroke(pts: Array(run), color: s.color))) }
            default: out.append(it)
            }
        }
        return changed ? out : nil
    }
}

private enum PadDrag: Equatable {
    case new(x0: CGFloat, y0: CGFloat)
    case pen
    case erase(kept: Bool, at: CGPoint)
    case item(i: Int, part: PadPart, x0: CGFloat, y0: CGFloat, from: PadItem, moved: Bool)
}

struct DataPad: Equatable {
    private(set) var items: [PadItem] = []
    private(set) var tool: PadTool = .pen
    /// The colour the next stroke or box takes, an index into padColors that may run past its end.
    private(set) var next = 0
    /// A colour was chosen by hand: new boxes keep it instead of taking the next.
    private var pinned = false
    private(set) var picked: Int?
    /// The side being given a length.
    private(set) var editing: (Int, PadPart)? { get { editingAt.map { ($0, editingSide) } } set { editingAt = newValue?.0; editingSide = newValue?.1 ?? .inside } }
    private var editingAt: Int?, editingSide: PadPart = .inside
    /// The box being dragged out and the stroke being drawn, for the screen only.
    private(set) var draft: PadBox?
    private(set) var ink: PadStroke?
    private(set) var pointer: CGPoint?
    /// How near a side counts as on it, and the eraser's radius, in paper pixels.
    var edge: CGFloat = 7
    var eraser: CGFloat = 12
    private var undoStack: [([PadItem], Int)] = []
    private var drag: PadDrag?

    static func == (a: DataPad, b: DataPad) -> Bool {
        a.items == b.items && a.tool == b.tool && a.next == b.next && a.picked == b.picked && a.editingAt == b.editingAt && a.editingSide == b.editingSide
            && a.draft == b.draft && a.ink == b.ink && a.pointer == b.pointer && a.drag == b.drag && a.undoStack.count == b.undoStack.count
    }
    var color: UInt32 { padColors[next % padColors.count] }
    var isEmpty: Bool { items.isEmpty }
    var dragging: Bool { drag != nil }
    var canUndo: Bool { !undoStack.isEmpty }
    private mutating func keep() { undoStack.append((items, next)); if undoStack.count > 100 { undoStack.removeFirst() } }
    private func clamp(_ x: CGFloat, _ y: CGFloat) -> CGPoint { CGPoint(x: min(max(x, 0), padW), y: min(max(y, 0), padH)) }
    mutating func setTool(_ t: PadTool) { tool = t; picked = nil; pointer = nil }
    mutating func setColor(_ i: Int) { next = i % padColors.count; pinned = true }

    /// A finger went down on the paper at (x, y).
    mutating func down(_ x: CGFloat, _ y: CGFloat) {
        let p = clamp(x, y)
        picked = nil
        switch tool {
        case .pen: ink = PadStroke(pts: [p], color: color); drag = .pen
        case .eraser: pointer = p; drag = .erase(kept: false, at: p); eraseTo(p)
        case .boxes:
            if let (i, part) = PadMath.pick(items, p.x, p.y, edge: edge) { picked = i; drag = .item(i: i, part: part, x0: p.x, y0: p.y, from: items[i], moved: false) }
            else { drag = .new(x0: p.x, y0: p.y) }
        }
    }
    private mutating func eraseTo(_ to: CGPoint) {
        guard case .erase(var kept, let at) = drag else { return }
        for p in [at] + PadMath.walk(at, to) {
            guard let left = PadMath.erased(items, p.x, p.y, radius: eraser) else { continue }
            if !kept { kept = true; keep() } // one sweep is one undo
            items = left
        }
        drag = .erase(kept: kept, at: to)
    }
    /// The finger moved to (x, y), on the paper or off it.
    mutating func moved(_ x: CGFloat, _ y: CGFloat) {
        let c = clamp(x, y)
        switch drag {
        case nil: return
        case .pen?: if var s = ink, let last = s.pts.last, last != c { s.pts += PadMath.walk(last, c); ink = s }
        case .erase?: pointer = c; eraseTo(c)
        case .new(let x0, let y0)?:
            if hypot(c.x - x0, c.y - y0) >= 3 || draft != nil { draft = PadBox(x: min(x0, c.x), y: min(y0, c.y), w: abs(c.x - x0), h: abs(c.y - y0), color: color) }
        case .item(let i, let part, let x0, let y0, let from, var moved)?:
            guard moved || hypot(x - x0, y - y0) >= 3 else { return }
            if !moved { moved = true; keep() }
            switch (from, part) {
            case (.box(var r), .inside): r.x = min(max(r.x + x - x0, 0), padW - r.w).rounded(); r.y = min(max(r.y + y - y0, 0), padH - r.h).rounded(); items[i] = .box(r)
            case (.box(let r), let side): items[i] = .box(PadMath.resize(r, side, c.x.rounded(), c.y.rounded()))
            case (.stroke(let s), _):
                let b = PadMath.bounds(from)
                let dx = min(max(x - x0, -b.minX), padW - b.width - b.minX), dy = min(max(y - y0, -b.minY), padH - b.height - b.minY)
                items[i] = .stroke(PadStroke(pts: s.pts.map { CGPoint(x: $0.x + dx, y: $0.y + dy) }, color: s.color))
            }
            drag = .item(i: i, part: part, x0: x0, y0: y0, from: from, moved: moved)
        }
    }
    /// The finger came up. True when it was a tap on a box's side (no drag): the view asks for its length, and `editing` says which side.
    mutating func up() -> Bool {
        guard let d = drag else { return false }
        drag = nil; pointer = nil
        switch d {
        case .pen: if let s = ink { keep(); items.append(.stroke(s)) }; ink = nil; return false
        case .erase: return false
        case .new:
            if let r = draft, r.w >= padMin, r.h >= padMin {
                keep()
                items.append(.box(PadBox(x: r.x.rounded(), y: r.y.rounded(), w: r.w.rounded(), h: r.h.rounded(), color: r.color)))
                picked = items.count - 1
                if !pinned { next += 1 }
            }
            draft = nil; return false
        case .item(let i, let part, _, _, let from, let moved):
            guard !moved, part != .inside, case .box = from else { return false }
            editing = (i, part); return true
        }
    }
    /// The touch was taken away (a system gesture): what was being drawn is dropped.
    mutating func cancel() { drag = nil; draft = nil; ink = nil; pointer = nil }
    /// What the length box starts with: the side's typed length, else its length in units.
    func lengthText() -> String {
        guard let (i, side) = editing, i < items.count, case .box(let r) = items[i] else { return "" }
        let horizontal = side == .l || side == .r
        let label = horizontal ? r.dimW : r.dimH
        return label.isEmpty ? String(Int(((horizontal ? r.w : r.h) / padUnit).rounded())) : label
    }
    /// The length box closed with `text`; text that is no length changes nothing.
    mutating func commitLength(_ text: String) {
        guard let (i, side) = editing else { return }
        editing = nil
        guard let len = PadMath.parseLength(text), i < items.count, case .box(let r) = items[i] else { return }
        keep(); items[i] = .box(PadMath.withLength(r, side, len))
    }
    mutating func cancelLength() { editing = nil }
    /// Take back the last change.
    mutating func undo() {
        guard !dragging, let (was, n) = undoStack.popLast() else { return }
        items = was; next = n; picked = nil
    }
    /// Delete the selection; with none, clear the paper. Either is one undo.
    mutating func delete() {
        guard !dragging, !items.isEmpty else { return }
        keep()
        if let p = picked, p < items.count { items.remove(at: p) } else { items.removeAll() }
        picked = nil
    }
}

private func cg(_ rgb: UInt32, _ a: CGFloat = 1) -> CGColor {
    CGColor(srgbRed: CGFloat((rgb >> 16) & 255) / 255, green: CGFloat((rgb >> 8) & 255) / 255, blue: CGFloat(rgb & 255) / 255, alpha: a)
}

/// The pad's picture on `c`, in paper pixels (the caller scales; y runs down). `marks`: the selection and what is being drawn, for the screen;
/// without, for the attachment.
func padPaint(_ c: CGContext, _ pad: DataPad, marks: Bool) {
    let font = UIFont.monospacedSystemFont(ofSize: 26, weight: .regular)
    func text(_ s: String, centredOn x: CGFloat, baseline y: CGFloat, _ color: UInt32) {
        let a = NSAttributedString(string: s, attributes: [.font: font, .foregroundColor: UIColor(cgColor: cg(color))])
        a.draw(at: CGPoint(x: x - a.size().width / 2, y: y - font.ascender))
    }
    func dashed(_ r: CGRect, _ color: UInt32) {
        c.saveGState(); c.setStrokeColor(cg(color)); c.setLineWidth(1.5); c.setLineDash(phase: 0, lengths: [6, 4]); c.stroke(r); c.restoreGState()
    }
    func box(_ r: PadBox) {
        let rect = CGRect(x: r.x, y: r.y, width: r.w, height: r.h)
        c.setFillColor(cg(r.color, 0.2)); c.fill(rect)
        c.setStrokeColor(cg(r.color)); c.setLineWidth(3); c.stroke(rect.insetBy(dx: 1.5, dy: 1.5))
        if !r.dimW.isEmpty { text(r.dimW, centredOn: r.x + r.w / 2, baseline: r.y - 6, r.color) }
        if !r.dimH.isEmpty { c.saveGState(); c.translateBy(x: r.x - 8, y: r.y + r.h / 2); c.rotate(by: -.pi / 2); text(r.dimH, centredOn: 0, baseline: 0, r.color); c.restoreGState() }
    }
    func stroke(_ s: PadStroke) {
        guard let first = s.pts.first else { return }
        if s.pts.count == 1 { c.setFillColor(cg(s.color)); c.fillEllipse(in: CGRect(x: first.x - 1.5, y: first.y - 1.5, width: 3, height: 3)); return }
        c.setStrokeColor(cg(s.color)); c.setLineWidth(3); c.setLineCap(.round); c.setLineJoin(.round)
        c.beginPath(); c.addLines(between: s.pts); c.strokePath()
    }
    UIGraphicsPushContext(c) // the labels are drawn by UIKit
    defer { UIGraphicsPopContext() }
    for (i, it) in pad.items.enumerated() {
        switch it { case .box(let b): box(b); case .stroke(let s): stroke(s) }
        if marks && pad.picked == i { dashed(PadMath.bounds(it).insetBy(dx: -4, dy: -4), padInk) }
    }
    guard marks else { return }
    if let d = pad.draft { dashed(CGRect(x: d.x, y: d.y, width: d.w, height: d.h), d.color) }
    if let s = pad.ink { stroke(s) }
    if pad.tool == .eraser, let p = pad.pointer {
        c.setStrokeColor(cg(padInk)); c.setLineWidth(1.5); c.strokeEllipse(in: CGRect(x: p.x - pad.eraser, y: p.y - pad.eraser, width: pad.eraser * 2, height: pad.eraser * 2))
    }
}

/// The attachment: the paper at its own size, white, as a JPEG.
func padPicture(_ pad: DataPad) -> Data? {
    let fmt = UIGraphicsImageRendererFormat(); fmt.scale = 1; fmt.opaque = true
    return UIGraphicsImageRenderer(size: CGSize(width: padW, height: padH), format: fmt).image { ctx in
        UIColor.white.setFill(); ctx.fill(CGRect(x: 0, y: 0, width: padW, height: padH))
        padPaint(ctx.cgContext, pad, marks: false)
    }.jpegData(compressionQuality: 0.92)
}

/// The pad over the thread: the composer's pen button opens it. Cancel drops it; Attach adds the picture to the message's pictures.
struct DataPadView: View {
    let attach: (Data) -> Void
    @State private var pad = DataPad()
    @State private var asking = false
    @State private var length = ""
    @GestureState private var pressing = false
    @Environment(\.dismiss) private var dismiss

    private var hint: String {
        switch pad.tool {
        case .pen: return Tr.t("Drag to draw")
        case .boxes: return Tr.t("Drag to draw a box · drag its middle to move it, a side to resize · tap a side to type its length")
        case .eraser: return Tr.t("Drag over what to erase")
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(Tr.t("Draw")).dmono(15, bold: true).foregroundStyle(BackplaneDesign.text)
            Text(hint).dmono(11).foregroundStyle(BackplaneDesign.dim).fixedSize(horizontal: false, vertical: true).accessibilityIdentifier("data.pad.hint")
            HStack(spacing: 6) {
                ForEach([(PadTool.pen, "Pen"), (.boxes, "Box"), (.eraser, "Eraser")], id: \.1) { t, label in
                    Button(Tr.t(label)) { pad.setTool(t) }.buttonStyle(DButton(primary: pad.tool == t)).accessibilityIdentifier("data.pad." + label.lowercased())
                        .accessibilityAddTraits(pad.tool == t ? .isSelected : [])
                }
            }
            HStack(spacing: 6) {
                ForEach(padColors.indices, id: \.self) { i in
                    let on = pad.next % padColors.count == i
                    Button { pad.setColor(i) } label: { Rectangle().fill(Color(cgColor: cg(padColors[i]))).frame(width: 28, height: 28).padding(on ? 3 : 1).overlay(Rectangle().strokeBorder(on ? BackplaneDesign.text : BackplaneDesign.line, lineWidth: on ? 3 : 1)) }
                        .buttonStyle(.plain).accessibilityLabel(Tr.ios_pad_colour(n: String(i + 1))).accessibilityAddTraits(on ? .isSelected : []).accessibilityIdentifier("data.pad.color.\(i)")
                }
            }
            GeometryReader { geo in
                let k = padW / max(geo.size.width, 1)
                Canvas { ctx, size in
                    ctx.withCGContext { c in c.scaleBy(x: size.width / padW, y: size.width / padW); padPaint(c, pad, marks: true) }
                }
                .background(Color.white)
                .overlay(Rectangle().strokeBorder(BackplaneDesign.line, lineWidth: 1))
                .contentShape(Rectangle())
                .gesture(DragGesture(minimumDistance: 0, coordinateSpace: .local)
                    .updating($pressing) { _, s, _ in s = true }
                    .onChanged { v in
                        // a side is a fingertip wide (14 pt) and so is the eraser, whatever the phone's size; never less than the desktop's
                        pad.edge = max(7, 14 * k); pad.eraser = max(12, 14 * k)
                        if !pad.dragging { pad.down(v.startLocation.x * k, v.startLocation.y * k) }
                        pad.moved(v.location.x * k, v.location.y * k)
                    }
                    .onEnded { _ in if pad.up() { length = pad.lengthText(); asking = true } })
                .accessibilityLabel(Tr.t("Drawing paper")).accessibilityIdentifier("data.pad.paper")
            }
            .aspectRatio(padW / padH, contentMode: .fit)
            // a touch the system took away ends with no onEnded: what was being drawn is dropped (after onEnded has had its chance)
            .onChange(of: pressing) { _, on in if !on { DispatchQueue.main.async { if pad.dragging { pad.cancel() } } } }
            HStack(spacing: 6) {
                Button(Tr.t("Undo")) { pad.undo() }.buttonStyle(DButton()).disabled(!pad.canUndo).accessibilityIdentifier("data.pad.undo")
                Button(Tr.t(pad.picked != nil ? "Delete" : "Clear")) { pad.delete() }.buttonStyle(DButton()).disabled(pad.isEmpty).accessibilityIdentifier("data.pad.delete")
                Spacer(minLength: 0)
                Button(Tr.t("Cancel")) { dismiss() }.buttonStyle(DButton()).accessibilityIdentifier("data.pad.cancel")
                Button(Tr.t("Attach")) { if let d = padPicture(pad) { attach(d) }; dismiss() }.buttonStyle(DButton(primary: true)).disabled(pad.isEmpty).accessibilityIdentifier("data.pad.attach")
            }
            Spacer(minLength: 0)
        }
        .padding(14)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .background(BackplaneDesign.background)
        .alert(Tr.t("Length, e.g. 40 mm"), isPresented: $asking) {
            TextField(Tr.t("40 mm"), text: $length).accessibilityIdentifier("data.pad.length")
            Button(Tr.t("Set")) { pad.commitLength(length) }
            Button(Tr.t("Cancel"), role: .cancel) { pad.cancelLength() }
        }
    }
}
