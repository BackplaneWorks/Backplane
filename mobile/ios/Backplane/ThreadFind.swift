import Foundation

/// The thread's find bar and the search snippets, kept out of the views (the UI is dumb): which match is current when an answer comes,
/// next and previous with wrap, the "n of total" label, and the query's words to set bold in a snippet. Tested by Tests/ThreadFindTests.swift.
/// `matches` are the hub's `find` answer, oldest first (docs/PHONE-DATA.md, find).
enum ThreadFind {
    /// The match to show when an answer arrives: the newest one (the last), none when nothing matched.
    static func first(count: Int) -> Int? { count > 0 ? count - 1 : nil }

    /// One step: `by` -1 is the previous (older) match, +1 the next (newer); past either end it wraps. With no current match a step goes to the newest.
    static func step(_ at: Int?, count: Int, by: Int) -> Int? {
        guard count > 0 else { return nil }
        guard let at, at >= 0, at < count else { return count - 1 }
        return ((at + by) % count + count) % count
    }

    /// "3 of 12" (the current match counted from the oldest), "0 of 0" when nothing matched, "" before any answer; "+" when the hub stopped counting (`more`).
    static func label(_ at: Int?, total: Int?, more: Bool = false) -> String {
        guard let total else { return "" }
        let n = total > 0 ? (at.map { $0 + 1 } ?? 0) : 0
        return "\(n) of \(total)\(more ? "+" : "")"
    }

    /// Whether a query is worth asking the hub for (two characters or more, spaces aside).
    static func asks(_ q: String) -> Bool { q.trimmingCharacters(in: .whitespacesAndNewlines).count >= 2 }

    /// `text` cut into runs, each with whether it is one of the query's words (any case, every place it occurs), for a snippet with the words in bold.
    static func spans(_ text: String, _ q: String) -> [(String, Bool)] {
        let words = q.lowercased().split(whereSeparator: { $0.isWhitespace }).map(String.init).filter { !$0.isEmpty }
        guard !words.isEmpty, !text.isEmpty else { return text.isEmpty ? [] : [(text, false)] }
        let chars = Array(text), low = chars.map { String($0).lowercased() }
        var hit = [Bool](repeating: false, count: chars.count)
        for w in words {
            let wc = w.map { String($0) }; guard wc.count <= low.count else { continue }
            var i = 0
            while i + wc.count <= low.count {
                if Array(low[i..<i + wc.count]) == wc { for k in i..<i + wc.count { hit[k] = true }; i += wc.count } else { i += 1 }
            }
        }
        var out: [(String, Bool)] = []
        for (c, h) in zip(chars, hit) {
            if let last = out.last, last.1 == h { out[out.count - 1].0.append(c) } else { out.append((String(c), h)) }
        }
        return out
    }
}
