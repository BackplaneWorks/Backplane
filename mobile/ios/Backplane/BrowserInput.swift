// The Browser tab's input, pure (test/mobile/browser_input_cases.json holds this and Android's BrowserInput.kt to the same answers):
// the key field's change as the type/press ops sent to the page, and a drag on the picture as the page's scroll.
import Foundation

enum BrowserInput {
    /// What the key field starts as (and goes back to): a Backspace on it is a change the field reports.
    static let sentinel = "\u{200B}"
    /// The key buttons under the field: (label, the key's name for `press`).
    static let keyButtons: [(String, String)] = [("⇥", "Tab"), ("←", "ArrowLeft"), ("↑", "ArrowUp"), ("↓", "ArrowDown"), ("→", "ArrowRight"), ("⌫", "Backspace"), ("⏎", "Enter")]
    /// The ops for the field going from `old` to `text`, in order: ("type", text) or ("press", key). Compared by code point: what is kept at both ends
    /// is not sent again; each removed character other than the sentinel is a Backspace, the sentinel removed alone is one.
    static func keys(_ old: String, _ text: String) -> [(String, String)] {
        let a = Array(old.unicodeScalars), b = Array(text.unicodeScalars), s0 = sentinel.unicodeScalars.first!
        var p = 0; while p < a.count && p < b.count && a[p] == b[p] { p += 1 }
        var s = 0; while s < a.count - p && s < b.count - p && a[a.count - 1 - s] == b[b.count - 1 - s] { s += 1 }
        let gone = a[p..<(a.count - s)], added = b[p..<(b.count - s)]
        var out: [(String, String)] = []
        let n = gone.filter { $0 != s0 }.count
        for _ in 0..<(n > 0 ? n : (gone.contains(s0) ? 1 : 0)) { out.append(("press", "Backspace")) }
        var run = String.UnicodeScalarView()
        func flush() { if !run.isEmpty { out.append(("type", String(run))); run = String.UnicodeScalarView() } }
        var i = added.startIndex
        while i < added.endIndex {
            let c = added[i]
            if c == s0 {} else if c == "\r" || c == "\n" { flush(); out.append(("press", "Enter")); if c == "\r", added.index(after: i) < added.endIndex, added[added.index(after: i)] == "\n" { i = added.index(after: i) } }
            else if c == "\t" { flush(); out.append(("press", "Tab")) }
            else { run.append(c) }
            i = added.index(after: i)
        }
        flush()
        return out
    }
    /// What the field holds after `text`: the sentinel again when it is gone, after an Enter or a Tab, or past 64 characters after it.
    static func next(_ text: String) -> String {
        let u = text.unicodeScalars
        return !text.contains(sentinel) || u.contains(where: { $0 == "\n" || $0 == "\r" || $0 == "\t" }) || u.count > 65 ? sentinel : text
    }
    /// A drag of (dx, dy) on a picture `image` pixels wide shown `shown` points wide: the page scrolls the other way, in its own pixels.
    static func scroll(_ dx: Double, _ dy: Double, image: Double, shown: Double) -> (Int, Int) {
        guard shown > 0, image > 0 else { return (0, 0) }
        let s = image / shown
        return (Int((-dx * s).rounded()), Int((-dy * s).rounded()))
    }
    /// A drag sends its scroll in steps of at least this many page pixels (and the rest when the finger lifts).
    static let step = 48
    /// Finger travel, in points, under which a touch is a tap (a click), not a drag.
    static let slop = 10.0
}
