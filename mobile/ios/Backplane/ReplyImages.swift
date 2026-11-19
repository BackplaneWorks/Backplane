import Foundation

// A reply's pictures in its text (feature replyImages, docs/PHONE-DATA.md). The hub says which paths are pictures (`images`, the desktop's own
// list: native/host/src/files/mimg.rs `paths`); this decides where they go. A line that is only pictures (a bare path, in backticks, a list,
// bold, parentheses, `![alt](path)`, or a bare file name under a folder line) is not drawn as text: each run of such lines becomes one slot, a
// single picture or a carousel, where the run was. A picture named only inside a sentence keeps its sentence and is drawn after it (`loose`).
// Foundation only, so Tests/ReplyImagesTests.swift compiles it alone.
enum ReplyImages {
    /// What stands for slot n in the text: a paragraph of its own, "\u{FFFC}n".
    static let mark: Character = "\u{FFFC}"

    struct Layout: Equatable {
        /// the text with each run of picture lines replaced by its slot's mark
        var text: String
        /// each slot's pictures, in the order the text names them; a picture is in one slot at most
        var slots: [[String]]
        /// pictures no picture-only line names (a path inside a sentence, in a quote or table): drawn under the block that names them
        var loose: [String]
    }

    /// The text and slots for a reply. `images` is the hub's list; with none, the text is as it came.
    static func layout(_ text: String, images: [String]) -> Layout {
        if images.isEmpty { return Layout(text: text, slots: [], loose: []) }
        let known = Set(images)
        var out: [String] = [], slots: [[String]] = [], placed = Set<String>()
        var fence: String? = nil, folder: String? = nil, inRun = false
        for raw in text.replacingOccurrences(of: "\r\n", with: "\n").components(separatedBy: "\n") {
            let line = raw.replacingOccurrences(of: String(mark), with: "")
            let t = line.trimmingCharacters(in: .whitespaces)
            if let f = fence {
                if t.hasPrefix(f) { fence = nil }
                out.append(line); continue
            }
            if t.hasPrefix("```") || t.hasPrefix("~~~") { fence = String(t.prefix(3)); inRun = false; out.append(line); continue }
            if t.isEmpty { if !inRun { out.append(line) }; continue }
            if let pics = pictures(line, known: known, folder: folder) {
                let fresh = pics.filter { !placed.contains($0) }
                for p in fresh { placed.insert(p) }
                if !inRun {
                    if fresh.isEmpty { continue } // a picture already shown, named again alone: not drawn twice
                    out.append(""); out.append("\(mark)\(slots.count)"); out.append(""); slots.append([]); inRun = true
                }
                slots[slots.count - 1].append(contentsOf: fresh)
                continue
            }
            inRun = false
            if let d = folderOf(line, known: known) { folder = d }
            out.append(line)
        }
        let loose = images.filter { !placed.contains($0) }
        return Layout(text: out.joined(separator: "\n"), slots: slots, loose: loose)
    }

    /// The slot a paragraph stands for: "\u{FFFC}n" with n below `count`; nil for any other text.
    static func slot(_ paragraph: String, count: Int) -> Int? {
        let t = paragraph.trimmingCharacters(in: .whitespacesAndNewlines)
        guard t.first == mark, let n = Int(t.dropFirst()), n >= 0, n < count else { return nil }
        return n
    }

    /// A picture's file name, for its tile and the viewer's title.
    static func name(_ path: String) -> String {
        let p = path.hasSuffix("/") ? String(path.dropLast()) : path
        return String(p.split(whereSeparator: { $0 == "/" || $0 == "\\" }).last ?? Substring(p))
    }

    /// "2 / 5": where a carousel or the viewer is (i from 0); empty for a single picture.
    static func counter(_ i: Int, of n: Int) -> String { n < 2 ? "" : "\(min(max(i, 0), n - 1) + 1) / \(n)" }
    /// Page dots are drawn up to this many pictures; past it, only the count.
    static let maxDots = 10

    /// Where the viewer opens: the picture tapped among all the reply's pictures (the first when it is not one of them).
    static func start(_ path: String, in all: [String]) -> Int { all.firstIndex(of: path) ?? 0 }

    /// A double tap's zoom: from fitted to `step` times fitted (at most the largest); from any zoom back to fitted.
    static func doubleTap(current: Double, min lo: Double, max hi: Double, step: Double = 2.5) -> Double {
        current > lo * 1.01 ? lo : Swift.min(hi, lo * step)
    }

    /// Zoom factor relative to fitted ("1.0x", "2.5x"), for accessibility and tests.
    static func zoomLabel(current: Double, min lo: Double) -> String {
        let ratio: Double = lo > 0 ? current / lo : 1.0
        return String(format: "%.1fx", locale: Locale(identifier: "en_US_POSIX"), ratio)
    }

    // MARK: lines

    /// The words the hub splits on (mimg.rs `sep`).
    private static func sep(_ c: Character) -> Bool { " \t\r\n()[]{}<>`\"',;|".contains(c) }
    /// mimg.rs `bare`: bold and strike marks around it, "file://", and trailing . : ! ? dropped.
    private static func bare(_ w: Substring) -> String {
        var s = Substring(w)
        while s.hasPrefix("*") { s = s.dropFirst() }
        while s.hasPrefix("~~") { s = s.dropFirst(2) }
        if s.hasPrefix("file://") { s = s.dropFirst(7) }
        while let l = s.last, ".:!?*~".contains(l) { s = s.dropLast() }
        return String(s)
    }
    private static func listBody(_ line: String) -> Substring {
        var s = Substring(line).drop(while: { $0 == " " || $0 == "\t" })
        if let f = s.first, "-*+".contains(f), s.dropFirst().first == " " { return s.dropFirst(2) }
        let digits = s.prefix(while: { $0.isNumber })
        if !digits.isEmpty, digits.count <= 3 {
            let r = s.dropFirst(digits.count)
            if let f = r.first, ".)".contains(f), r.dropFirst().first == " " { s = r.dropFirst(2) }
        }
        return s
    }
    /// The pictures a line is made of, or nil when it has anything else (a word, a path that is no picture of this reply).
    private static func pictures(_ line: String, known: Set<String>, folder: String?) -> [String]? {
        let body = listBody(line).trimmingCharacters(in: .whitespaces)
        // ![alt](path), alone on the line
        if body.hasPrefix("!["), body.hasSuffix(")"), let mid = body.range(of: "](") {
            let p = String(body[mid.upperBound..<body.index(before: body.endIndex)]).trimmingCharacters(in: .whitespaces)
            return known.contains(p) ? [p] : nil
        }
        var got: [String] = []
        for w in body.split(whereSeparator: sep) {
            let p = bare(w)
            if p.isEmpty || !p.contains(where: { $0.isLetter || $0.isNumber }) { continue } // a dash, a bullet, "**"
            if known.contains(p) { got.append(p); continue }
            if let d = folder, !p.contains("/"), known.contains(d + "/" + p) { got.append(d + "/" + p); continue }
            return nil
        }
        return got.isEmpty ? nil : got
    }
    /// A line that names a folder some of the reply's pictures are in (the folder form: its files are listed by name below it).
    private static func folderOf(_ line: String, known: Set<String>) -> String? {
        for w in listBody(line).split(whereSeparator: sep) {
            var p = bare(w)
            guard p.hasPrefix("/") || p.hasPrefix("~/") else { continue }
            while p.count > 1, p.hasSuffix("/") { p.removeLast() }
            if known.contains(where: { $0.hasPrefix(p + "/") }) { return p }
        }
        return nil
    }
}
