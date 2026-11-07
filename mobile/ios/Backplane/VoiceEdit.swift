// Dictation's draft edits as the hub sends them (docs/PHONE-DATA.md, "Dictation"). Pure: Foundation only, so
// scripts/mobile/ios-voice-edit-test.sh compiles it alone and checks it against test/fixtures/voice-edits.json.
import Foundation

/// The hub's edit and the typing rule, in characters (Unicode scalars, as the hub counts). Pure; the same vectors as the hub's place.rs.
enum VoiceEdit {
    static func scalars(_ s: String) -> [Unicode.Scalar] { Array(s.unicodeScalars) }
    static func string(_ a: some Sequence<Unicode.Scalar>) -> String { var v = String.UnicodeScalarView(); v.append(contentsOf: a); return String(v) }
    static func count(_ s: String) -> Int { s.unicodeScalars.count }
    /// Replace `del` characters after the mark with `ins`; the mark moves `adv`, the caret goes `to` past the old mark. A mark past the end is the end.
    static func apply(_ draft: String, mark: Int, del: Int, ins: String, adv: Int, to: Int) -> (text: String, mark: Int, caret: Int) {
        let cs = scalars(draft), m = min(mark, cs.count), end = min(m + del, cs.count)
        return (string(cs[..<m] + scalars(ins) + cs[end...]), m + adv, m + to)
    }
    /// The person typed: the mark moves with what changed before it.
    static func shift(_ old: String, _ new: String, mark: Int) -> Int {
        let a = scalars(old), b = scalars(new)
        var p = 0
        while p < a.count, p < b.count, a[p] == b[p] { p += 1 }
        if p >= mark { return mark }
        return max(p, max(0, mark + b.count - a.count))
    }
    /// UTF-16 offset in the text field of a caret in characters.
    static func utf16(_ s: String, caret: Int) -> Int { string(scalars(s).prefix(caret)).utf16.count }
    /// Characters before a UTF-16 offset.
    static func chars(_ s: String, utf16 at: Int) -> Int { let u = Array(s.utf16.prefix(at)); return count(String(decoding: u, as: UTF16.self)) }
}
