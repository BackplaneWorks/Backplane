import Foundation

/// The Diff tab's model: the hub's unified `git diff` text (viewer.diff) as files with their state, totals and numbered lines.
/// Pure: the view only draws what this returns (Tests/DiffModelTests.swift; the Android twin is DiffModel.kt).
enum DiffModel {
    /// One shown line: kind "+", "-", " ", "@" (a hunk head, its whole text) or "\\" (a note such as "No newline"); text without the
    /// leading marker except for "@" and "\\"; old/new line numbers, 0 when the side has none.
    struct Line: Equatable {
        var kind: Character
        var text: String
        var old: Int
        var new: Int
    }
    /// A changed file: path (the new one), from (the old path when renamed, else ""), state "", "new", "deleted", "renamed" or "binary";
    /// cut: some of its lines were dropped by the parse's line cap (the view ends it with "the rest is not shown").
    struct File: Equatable {
        var path: String
        var from: String
        var state: String
        var added: Int
        var removed: Int
        var lines: [Line]
        var cut: Bool = false
    }
    struct Parsed: Equatable {
        var files: [File]
        var added: Int
        var removed: Int
    }

    private final class Acc {
        var path: String
        var from = "", state = "", added = 0, removed = 0
        var big: (Int, Int)? = nil
        var hunk = false, old = 0, new = 0, cut = false
        var lines: [Line] = []
        init(_ path: String) { self.path = path }
        func done() -> File { File(path: path, from: from, state: state, added: big?.0 ?? added, removed: big?.1 ?? removed, lines: lines, cut: cut) }
    }

    /// git quotes odd paths as "a/x y": strip the quotes (escapes stay as git wrote them).
    private static func unquote(_ s: String) -> String {
        if s.count >= 2 && s.hasPrefix("\"") && s.hasSuffix("\"") { return String(s.dropFirst().dropLast()) }
        return s
    }
    private static func dropB(_ s: String) -> String { s.hasPrefix("b/") ? String(s.dropFirst(2)) : s }

    /// The new path from `diff --git a/X b/Y`.
    private static func gitPath(_ rest: String) -> String {
        let r = rest.trimmingCharacters(in: .whitespaces)
        if r.hasSuffix("\""), let i = r.range(of: " \"b/", options: .backwards) {
            return dropB(unquote(String(r[r.index(after: i.lowerBound)...])))
        }
        if let i = r.range(of: " b/", options: .backwards) { return dropB(unquote(String(r[r.index(after: i.lowerBound)...]))) }
        return unquote(r)
    }

    /// The start line after the sign in "@@ -a,b +c,d @@".
    private static func hunkStart(_ s: String, _ sign: Character) -> Int {
        let chars = Array(s)
        guard chars.count > 2, let i = chars[2...].firstIndex(of: sign) else { return 0 }
        var j = i + 1
        var digits = ""
        while j < chars.count, chars[j].isASCII, chars[j].isNumber { digits.append(chars[j]); j += 1 }
        return Int(digits) ?? 0
    }

    /// "+N -M lines" in the hub's "\ Too big to show here" line.
    private static func tooBig(_ s: String) -> (Int, Int)? {
        guard let r = s.range(of: #"\+(\d+) -(\d+) lines"#, options: .regularExpression) else { return nil }
        let nums = s[r].split(whereSeparator: { !$0.isNumber }).compactMap { Int($0) }
        return nums.count >= 2 ? (nums[0], nums[1]) : nil
    }

    static func parse(_ text: String, maxLines: Int = 4000) -> Parsed {
        var files: [File] = []
        var cur: Acc? = nil
        var kept = 0
        var all = text.components(separatedBy: "\n")
        if all.last == "" { all.removeLast() }
        func keep(_ a: Acc, _ l: Line) { if kept < maxLines { a.lines.append(l); kept += 1 } else { a.cut = true } }
        for line in all {
            let raw = line.hasSuffix("\r") ? String(line.dropLast()) : line
            if raw.hasPrefix("diff --git ") {
                if let c = cur { files.append(c.done()) }
                cur = Acc(gitPath(String(raw.dropFirst("diff --git ".count))))
                continue
            }
            guard let a = cur else { continue }   // text before the first file
            if raw.hasPrefix("@@") {
                a.hunk = true; a.old = hunkStart(raw, "-"); a.new = hunkStart(raw, "+")
                keep(a, Line(kind: "@", text: raw, old: 0, new: 0))
                continue
            }
            if !a.hunk {
                if raw.hasPrefix("new file mode") { a.state = "new" }
                else if raw.hasPrefix("deleted file mode") { a.state = "deleted" }
                else if raw.hasPrefix("rename from ") { a.from = unquote(String(raw.dropFirst("rename from ".count))); a.state = "renamed" }
                else if raw.hasPrefix("rename to ") { a.path = unquote(String(raw.dropFirst("rename to ".count))); a.state = "renamed" }
                else if raw.hasPrefix("Binary files ") || raw.hasPrefix("GIT binary patch") { a.state = "binary" }
                continue   // index, ---, +++, modes, similarity: header lines, not shown
            }
            switch raw.first {
            case "+"?:
                a.added += 1; keep(a, Line(kind: "+", text: String(raw.dropFirst()), old: 0, new: a.new)); a.new += 1
            case "-"?:
                a.removed += 1; keep(a, Line(kind: "-", text: String(raw.dropFirst()), old: a.old, new: 0)); a.old += 1
            case "\\"?:
                if let b = tooBig(raw) { a.big = b }
                keep(a, Line(kind: "\\", text: raw, old: 0, new: 0))
            case " "?, nil:
                keep(a, Line(kind: " ", text: raw.isEmpty ? "" : String(raw.dropFirst()), old: a.old, new: a.new)); a.old += 1; a.new += 1
            default:
                if raw.hasPrefix("Binary files ") { a.state = "binary" }
            }
        }
        if let c = cur { files.append(c.done()) }
        return Parsed(files: files, added: files.reduce(0) { $0 + $1.added }, removed: files.reduce(0) { $0 + $1.removed })
    }
}
