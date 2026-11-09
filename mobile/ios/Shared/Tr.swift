import Foundation
import Observation

// The app's words, from the catalogs i18n/en.msg and i18n/ja.msg (TrCatalog.swift, made by scripts/i18n-ios.py).
//   Tr.k("common.cancel")        a message without arguments, by key
//   Tr.thread_count(n: 3)        a message with arguments: a generated function for each such key
//   Tr.t("Cancel")               English text to the language on show: an exact catalog message, or one that fits a templated message
//                                ("3 files changed" fits `{n, plural, one {# file changed} other {# files changed}}`) rendered with what it captured.
//                                Anything else (agent text, file names, what people wrote) comes back unchanged.
// The language is Tr.lang ("en" or "ja"), set at the root from the app's setting. Reading it in a view's body (every Tr call does) is
// what re-renders the view when it changes.
enum Tr {
    /// "en" or "ja": the language the app shows.
    static var lang: String {
        get { TrLang.shared.tag }
        set { if newValue != TrLang.shared.tag { cache.clear(); TrLang.shared.tag = newValue } }
    }

    /// The app's language from the setting: "auto" follows the system (its first preferred language starting with ja is Japanese).
    static func resolve(_ setting: String, preferred: [String] = Locale.preferredLanguages) -> String {
        switch setting {
        case "en", "ja": setting
        default: (preferred.first ?? "en").lowercased().hasPrefix("ja") ? "ja" : "en"
        }
    }

    /// The locale for dates and numbers the app formats itself: Japanese, or the system's own when it is not Japanese.
    static var locale: Locale {
        lang == "ja" ? Locale(identifier: "ja_JP") : Locale.current.language.languageCode?.identifier == "ja" ? Locale(identifier: "en_US") : Locale.current
    }

    /// "one" or "other", for the catalog's `{plural, select, one {…} other {…}}` beside a count.
    static func one(_ n: Int) -> String { n == 1 ? "one" : "other" }

    /// A message without arguments; the key itself when the catalog has no such key.
    static func k(_ key: String) -> String {
        guard let e = table.byKey[key] else { return key }
        return render(lang == "ja" ? e.ja : e.en, [:], lang)
    }

    /// A message with arguments (numbers as their decimal text).
    static func f(_ key: String, _ args: [String: String]) -> String {
        guard let e = table.byKey[key] else { return key }
        return render(lang == "ja" ? e.ja : e.en, args, lang)
    }

    /// English text in the language on show (see the top of this file).
    static func t(_ text: String) -> String {
        let tag = lang
        if tag == "en" || text.isEmpty { return text }
        if let hit = cache.get(text) { return hit }
        let out = translate(text, to: tag) ?? text
        cache.put(text, out)
        return out
    }

    /// The key whose English is this text, or fits it, with what it captured; nil for text the catalog does not have.
    static func match(_ text: String) -> (key: String, args: [String: String])? {
        fitting(text).map { ($0.key, $0.args) }
    }

    /// match, and the names of the text holes it filled (not numbers, not a choice's branch).
    static func fitting(_ text: String) -> (key: String, args: [String: String], holes: [String])? {
        if let key = table.exact[text] { return (key, [:], []) }
        let t = Array(text.unicodeScalars)
        for tpl in table.templates where tpl.anchor.isEmpty || text.contains(tpl.anchor) {
            var caps = tpl.fixed
            // a hole takes no " · " unless the message joins pieces itself: "3 tool calls · Read · 2 design changes" is three pieces, not one long {count}
            let joins = tpl.parts.contains { if case .lit(let s) = $0 { s.contains(" · ") } else { false } }
            if fit(tpl.parts, 0, t, 0, &caps, joins: joins) {
                let holes = tpl.parts.compactMap { p -> String? in if case .cap(let n, false) = p, tpl.fixed[n] == nil { return n }; return nil }
                return (tpl.key, caps, holes)
            }
        }
        return nil
    }

    /// The text in `tag`: the message it is or fits; else, for words joined with " · " (the hub's and the app's rows), each piece the catalog
    /// knows, the rest as it is; nil when nothing in it is the catalog's.
    static func translate(_ text: String, to tag: String) -> String? {
        if text.count > 600 { return nil }
        if let (key, args, holes) = fitting(text), let e = table.byKey[key] {
            // a hole that is itself one of the catalog's plain messages (a state word, a reason) is put in the language too
            var a = args
            for h in holes { if let v = a[h], let k = table.exact[v], let m = table.byKey[k] { a[h] = render(tag == "ja" ? m.ja : m.en, [:], tag) } }
            return render(tag == "ja" ? e.ja : e.en, a, tag)
        }
        guard text.contains(" · ") else { return nil }
        let pieces = text.components(separatedBy: " · ")
        let done = pieces.map { translate($0, to: tag) }
        guard done.contains(where: { $0 != nil }) else { return nil }
        return zip(pieces, done).map { $1 ?? $0 }.joined(separator: " · ")
    }

    // MARK: the catalog

    struct Entry { let en: [TrNode]; let ja: [TrNode] }
    struct Template { let key: String; let parts: [TrPart]; let fixed: [String: String]; let anchor: String; let weight: Int }
    struct Table {
        var byKey: [String: Entry] = [:]
        var exact: [String: String] = [:]
        var templates: [Template] = []
    }

    static let table: Table = {
        var t = Table()
        for line in catalog.split(separator: "\n", omittingEmptySubsequences: true) {
            let f = line.split(separator: "\t", omittingEmptySubsequences: false)
            guard f.count == 3 else { continue }
            let key = String(f[0]), en = TrParse.message(String(f[1])), ja = TrParse.message(String(f[2]))
            t.byKey[key] = Entry(en: en, ja: ja)
            if en.allSatisfy({ if case .lit = $0 { true } else { false } }) {
                let s = en.map { if case .lit(let s) = $0 { s } else { "" } }.joined()
                if t.exact[s] == nil { t.exact[s] = key }
                continue
            }
            for v in TrParse.variants(en, fixed: [:], hashArg: nil) {
                let parts = TrParse.merge(v.parts)
                let lits = parts.compactMap { if case .lit(let s) = $0 { s } else { nil } }
                let letters = lits.joined().unicodeScalars.filter { CharacterSet.letters.contains($0) }.count
                let caps = parts.filter { if case .cap = $0 { true } else { false } }
                let textCaps = parts.filter { if case .cap(_, false) = $0 { true } else { false } }.count
                if caps.isEmpty {
                    let s = lits.joined()
                    t.templates.append(Template(key: key, parts: parts, fixed: v.fixed, anchor: s, weight: Int.max))
                    continue
                }
                // a template that is mostly a hole would take people's text for a message: it needs words of its own
                let marks = lits.joined().unicodeScalars.filter { !CharacterSet.whitespacesAndNewlines.contains($0) }.count
                if letters < 3 && !(textCaps == 0 && letters >= 1) && !(letters >= 1 && marks >= 4) { continue }
                t.templates.append(Template(key: key, parts: parts, fixed: v.fixed, anchor: lits.max(by: { $0.count < $1.count }) ?? "", weight: lits.joined().count))
            }
        }
        // the most specific first: more words of its own
        t.templates.sort { $0.weight > $1.weight }
        return t
    }()

    // MARK: rendering

    static func render(_ nodes: [TrNode], _ args: [String: String], _ tag: String, hash: String? = nil) -> String {
        var out = ""
        for n in nodes {
            switch n {
            case .lit(let s): out += s
            case .arg(let name, let num):
                if let v = args[name] { out += num ? number(v) : v } else { out += "{" + name + "}" }
            case .hash: out += hash.map(number) ?? "#"
            case .choice(let name, let plural, let branches):
                let v = args[name] ?? "other"
                if plural {
                    let n = Int(v.replacingOccurrences(of: ",", with: "")) ?? 0
                    let pick = branches.first { $0.0 == "=\(n)" } ?? branches.first { $0.0 == category(n, tag) } ?? branches.first { $0.0 == "other" }
                    out += render(pick?.1 ?? [], args, tag, hash: v)
                } else {
                    let pick = branches.first { $0.0 == v } ?? branches.first { $0.0 == "other" }
                    out += render(pick?.1 ?? [], args, tag, hash: hash)
                }
            }
        }
        return out
    }

    /// The plural category: English has one and other, Japanese only other.
    static func category(_ n: Int, _ tag: String) -> String { tag == "en" && n == 1 ? "one" : "other" }

    /// Grouped digits ("12,345") for a whole number; any other text as it is.
    static func number(_ v: String) -> String {
        let raw = v.replacingOccurrences(of: ",", with: "")
        guard let n = Int(raw) else { return v }
        var digits = String(abs(n)), out = ""
        while digits.count > 3 { out = "," + digits.suffix(3) + out; digits.removeLast(3) }
        return (n < 0 ? "-" : "") + digits + out
    }

    // MARK: matching

    /// Does text[ti...] fit parts[pi...]? A hole takes the shortest text that lets the rest fit (a number hole only digits).
    static func fit(_ parts: [TrPart], _ pi: Int, _ t: [Unicode.Scalar], _ ti: Int, _ caps: inout [String: String], joins: Bool = true) -> Bool {
        if pi == parts.count { return ti == t.count }
        switch parts[pi] {
        case .lit(let s):
            let l = Array(s.unicodeScalars)
            guard t.count - ti >= l.count, Array(t[ti..<ti + l.count]) == l else { return false }
            return fit(parts, pi + 1, t, ti + l.count, &caps, joins: joins)
        case .cap(let name, let num):
            var end = ti + 1
            while end <= t.count {
                let piece = String(String.UnicodeScalarView(t[ti..<end]))
                if num, let last = piece.unicodeScalars.last, !(("0"..."9").contains(last) || last == "," || last == "-") { return false }
                if num, !isNumber(piece) { end += 1; continue }
                if !joins, piece.hasSuffix(" · ") { return false }
                if let old = caps[name], old != piece { end += 1; continue }
                let had = caps[name]
                caps[name] = piece
                if fit(parts, pi + 1, t, end, &caps, joins: joins) { return true }
                caps[name] = had
                end += 1
            }
            return false
        }
    }

    static func isNumber(_ s: String) -> Bool {
        !s.isEmpty && s.first != "," && s.last != "," && s.unicodeScalars.allSatisfy { ("0"..."9").contains($0) || $0 == "," || $0 == "-" } && !s.dropFirst().contains("-")
    }

    // MARK: cache

    final class Cache: @unchecked Sendable {
        private var map: [String: String] = [:]
        private let lock = NSLock()
        func get(_ k: String) -> String? { lock.lock(); defer { lock.unlock() }; return map[k] }
        func put(_ k: String, _ v: String) { lock.lock(); defer { lock.unlock() }; if map.count > 4000 { map.removeAll() }; map[k] = v }
        func clear() { lock.lock(); map.removeAll(); lock.unlock() }
    }
    static let cache = Cache()
}

/// The language on show, observed: a view whose body asked Tr for words draws again when it changes.
@Observable
final class TrLang: @unchecked Sendable {
    static let shared = TrLang()
    var tag = "en"
}

indirect enum TrNode: Equatable {
    case lit(String)
    case arg(String, Bool)                       // name, a number
    case hash                                    // the nearest plural's number
    case choice(String, Bool, [(String, [TrNode])]) // name, plural (else select), branches in order

    static func == (a: TrNode, b: TrNode) -> Bool {
        switch (a, b) {
        case (.lit(let x), .lit(let y)): x == y
        case (.arg(let x, let p), .arg(let y, let q)): x == y && p == q
        case (.hash, .hash): true
        case (.choice(let x, let p, let u), .choice(let y, let q, let v)):
            x == y && p == q && u.count == v.count && zip(u, v).allSatisfy { $0.0 == $1.0 && $0.1 == $1.1 }
        default: false
        }
    }
}

enum TrPart: Equatable {
    case lit(String)
    case cap(String, Bool)                       // name, a number
}

/// The catalogs' message syntax (docs/i18n/README.md): `\s` `\n` `\\`, {name}, {n, number}, {n, plural, =0 {…} one {# …} other {…}},
/// {x, select, a {…} other {…}}, and apostrophes that quote before {}#| ('' is an apostrophe).
enum TrParse {
    static func message(_ raw: String) -> [TrNode] {
        let s = Array(unescape(raw))
        var i = 0
        return nodes(s, &i, inPlural: false, depth: 0)
    }

    static func unescape(_ v: String) -> String {
        guard v.contains("\\") else { return v }
        var out = "", esc = false
        for c in v {
            if esc { out.append(c == "s" ? " " : c == "n" ? "\n" : c); esc = false } else if c == "\\" { esc = true } else { out.append(c) }
        }
        return out
    }

    static func nodes(_ s: [Character], _ i: inout Int, inPlural: Bool, depth: Int) -> [TrNode] {
        var out: [TrNode] = [], buf = ""
        func flush() { if !buf.isEmpty { out.append(.lit(buf)); buf = "" } }
        while i < s.count {
            let c = s[i]
            if c == "'" {
                if i + 1 < s.count, s[i + 1] == "'" { buf.append("'"); i += 2; continue }
                if i + 1 < s.count, "{}#|".contains(s[i + 1]) {
                    var j = i + 1
                    while j < s.count {
                        if s[j] == "'" { if j + 1 < s.count, s[j + 1] == "'" { buf.append("'"); j += 2; continue }; break }
                        buf.append(s[j]); j += 1
                    }
                    i = j + 1; continue
                }
                buf.append(c); i += 1; continue
            }
            if c == "}" { if depth > 0 { break }; buf.append(c); i += 1; continue }
            if c == "#", inPlural { flush(); out.append(.hash); i += 1; continue }
            if c == "{" {
                flush(); i += 1
                let name = word(s, &i)
                skip(s, &i)
                if i < s.count, s[i] == "}" { i += 1; out.append(.arg(name, false)); continue }
                if i < s.count, s[i] == "," { i += 1 }
                let kind = word(s, &i)
                skip(s, &i)
                if kind == "number" { if i < s.count, s[i] == "}" { i += 1 }; out.append(.arg(name, true)); continue }
                if i < s.count, s[i] == "," { i += 1 }
                var branches: [(String, [TrNode])] = []
                while true {
                    skip(s, &i)
                    if i >= s.count { break }
                    if s[i] == "}" { i += 1; break }
                    var b = ""
                    while i < s.count, s[i] != "{", !s[i].isWhitespace { b.append(s[i]); i += 1 }
                    skip(s, &i)
                    guard i < s.count, s[i] == "{" else { break }
                    i += 1
                    let sub = nodes(s, &i, inPlural: inPlural || kind == "plural", depth: depth + 1)
                    if i < s.count, s[i] == "}" { i += 1 }
                    branches.append((b, sub))
                }
                out.append(.choice(name, kind == "plural", branches))
                continue
            }
            buf.append(c); i += 1
        }
        flush()
        return out
    }

    static func word(_ s: [Character], _ i: inout Int) -> String {
        skip(s, &i)
        var w = ""
        while i < s.count, s[i].isLetter || s[i].isNumber || s[i] == "_" { w.append(s[i]); i += 1 }
        return w
    }

    static func skip(_ s: [Character], _ i: inout Int) { while i < s.count, s[i].isWhitespace { i += 1 } }

    /// Every way the message can come out in English: its pieces of text and holes, and what each choice was set to.
    struct Variant { var parts: [TrPart]; var fixed: [String: String] }

    static func variants(_ nodes: [TrNode], fixed: [String: String], hashArg: String?) -> [Variant] {
        var acc = [Variant(parts: [], fixed: fixed)]
        for n in nodes {
            switch n {
            case .lit(let s): for i in acc.indices { acc[i].parts.append(.lit(s)) }
            case .arg(let name, let num): for i in acc.indices { acc[i].parts.append(.cap(name, num)) }
            case .hash:
                for i in acc.indices { acc[i].parts.append(hashArg.map { .cap($0, true) } ?? .lit("#")) }
            case .choice(let name, let plural, let branches):
                var next: [Variant] = []
                for v in acc {
                    for (b, sub) in branches {
                        var f = v.fixed
                        if plural {
                            // =N fixes the number; one and other leave it to the # (or to another hole for it)
                            if b.hasPrefix("=") { f[name] = String(b.dropFirst()) } else if b == "one" { f[name] = "1" }
                        } else {
                            if let old = f[name], old != b { continue }
                            f[name] = b
                        }
                        for w in variants(sub, fixed: f, hashArg: plural ? name : hashArg) {
                            next.append(Variant(parts: v.parts + w.parts, fixed: w.fixed))
                        }
                        if next.count > 64 { break }
                    }
                }
                acc = next
            }
        }
        return acc
    }

    /// Neighbouring pieces of text as one.
    static func merge(_ parts: [TrPart]) -> [TrPart] {
        var out: [TrPart] = []
        for p in parts {
            if case .lit(let s) = p, s.isEmpty { continue }
            if case .lit(let s) = p, case .lit(let prev)? = out.last { out[out.count - 1] = .lit(prev + s) } else { out.append(p) }
        }
        return out
    }
}
