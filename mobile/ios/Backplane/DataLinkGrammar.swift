import Foundation

// What in a message's text is a link, as the phone finds it (the Android app's DataLinkGrammar.kt is the same rules): web addresses, backplane:// links, emails,
// phone numbers and file paths (src/x.rs:12). Pure, Foundation only, so mobile/ios/Tests/LinkGrammarTests.swift runs it without the app.
// The hub decides what a backplane:// link or a path opens (link.resolve); this only says where the link is and what to hand it.

/// A path written in text, as a URL the text can carry: bppath:<percent-encoded path[:line]>
let dPathScheme = "bppath"
func dPathURL(_ p: String) -> URL? { URL(string: dPathScheme + ":" + (p.addingPercentEncoding(withAllowedCharacters: .urlPathAllowed.subtracting(CharacterSet(charactersIn: ":"))) ?? p)) }
func dPathFrom(_ u: URL) -> String? { u.scheme == dPathScheme ? String(u.absoluteString.dropFirst(dPathScheme.count + 1)).removingPercentEncoding : nil }

private func rx(_ p: String) -> NSRegularExpression { try! NSRegularExpression(pattern: p) }
private let reEmail = rx("^[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(\\.[A-Za-z0-9-]+)*\\.[A-Za-z]{2,}$")
// a path has a slash and a last part with an extension, or is a file with a known extension; an optional :line or :line:col
private let rePath = rx("^(?:~/|/|\\./|\\.\\./)?(?:[A-Za-z0-9_.@+-]+/)+[A-Za-z0-9_@+-][A-Za-z0-9_.@+-]*\\.[A-Za-z0-9]{1,10}(?::\\d{1,6}){0,2}(?:#L\\d{1,6}(?:-L?\\d{1,6})?)?$")
private let reFile = rx("^[A-Za-z0-9_@+-][A-Za-z0-9_.@+-]*\\.(?:rs|swift|kt|kts|java|js|jsx|ts|tsx|py|go|c|h|cc|cpp|hpp|m|mm|md|txt|toml|json|yaml|yml|lock|sh|css|html|sql|kicad_pcb|kicad_sch|kicad_pro|kicad_sym|kicad_mod|step|stp|pdf|csv|log|ini|cfg|gradle|xml|svg|png|jpg)(?::\\d{1,6}){0,2}(?:#L\\d{1,6}(?:-L?\\d{1,6})?)?$")
private let rePhone = rx("^(?:\\+\\d{1,3}[ .-]?)?(?:\\(\\d{3}\\)[ .-]?|\\d{3}[ .-])\\d{3}[ .-]\\d{4}(?!\\d)|^\\+\\d{1,3}(?:[ .-]?\\d{2,4}){2,4}(?!\\d)")

private func full(_ s: String, _ r: NSRegularExpression) -> Bool { r.firstMatch(in: s, range: NSRange(location: 0, length: (s as NSString).length)) != nil }
private func lead(_ s: String, _ r: NSRegularExpression) -> Int? {
    guard let m = r.firstMatch(in: s, range: NSRange(location: 0, length: (s as NSString).length)) else { return nil }
    return (s as NSString).substring(to: m.range.length).count
}

func dIsPathText(_ t: String) -> Bool { !t.hasPrefix("http") && !t.contains("://") && (full(t, rePath) || full(t, reFile)) }

/// Whether a link may start before s[i]: after a space, a new line or an opening mark, never inside a word.
private func boundary(_ s: [Character], _ i: Int) -> Bool {
    if i == 0 { return true }
    let p = s[i - 1]
    return p.isWhitespace || "([{<\"'`,;:".contains(p)
}

/// The email, phone number or path that starts at s[i], as (end, url). Web addresses are the caller's (they know their schemes).
func dAutoLink(_ s: [Character], _ i: Int) -> (end: Int, url: URL)? {
    guard i < s.count, boundary(s, i) else { return nil }
    let c = s[i]
    guard c.isLetter || c.isNumber || "+(/.~_@".contains(c) else { return nil }
    var e = i; while e < s.count, !s[e].isWhitespace, e - i < 300 { e += 1 }
    var tok = Array(s[i..<e])
    while let l = tok.last, ".,;:!?)]}>'\"*`".contains(l) {
        if l == ")" && tok.filter({ $0 == "(" }).count >= tok.filter({ $0 == ")" }).count { break }
        tok.removeLast()
    }
    if !tok.isEmpty {
        let t = String(tok)
        if full(t, reEmail), let u = URL(string: "mailto:" + t) { return (i + tok.count, u) }
        if dIsPathText(t), let u = dPathURL(t) { return (i + tok.count, u) }
    }
    if c == "+" || c == "(" || c.isNumber {
        let rest = String(s[i..<min(s.count, i + 24)])
        if let n = lead(rest, rePhone) {
            let t = String(s[i..<(i + n)]); let digits = t.filter(\.isNumber)
            if digits.count >= 7, digits.count <= 15, let u = URL(string: "tel:" + (c == "+" ? "+" : "") + digits) { return (i + n, u) }
        }
    }
    return nil
}

/// Plain text cut into runs: (text, link?) — for a message that is not Markdown (a user's, a room post).
func dLinkRuns(_ text: String) -> [(text: String, url: URL?)] {
    let s = Array(text); var out: [(String, URL?)] = []; var plain = ""; var i = 0
    func flush() { if !plain.isEmpty { out.append((plain, nil)); plain = "" } }
    while i < s.count {
        if let (e, u) = dWebLink(s, i) ?? dAutoLink(s, i) { flush(); out.append((String(s[i..<e]), u)); i = e; continue }
        plain.append(s[i]); i += 1
    }
    flush(); return out
}

/// An http(s):// or backplane:// address that starts at s[i], to the end of its characters (trailing marks and an unmatched ) stay out).
func dWebLink(_ s: [Character], _ i: Int) -> (end: Int, url: URL)? {
    if i > 0, s[i - 1].isLetter || s[i - 1].isNumber { return nil }
    var hit = false
    for p in ["http://", "https://", "backplane://"] where i + p.count <= s.count && String(s[i..<i + p.count]) == p { hit = true }
    guard hit else { return nil }
    var e = i; while e < s.count, !s[e].isWhitespace, !"<>\"'".contains(s[e]) { e += 1 }
    while e > i, ".,;:!?)]}*_".contains(s[e - 1]) {
        if s[e - 1] == ")" { let seg = s[i..<e]; if seg.filter({ $0 == "(" }).count >= seg.filter({ $0 == ")" }).count { break } }
        e -= 1
    }
    guard let u = URL(string: String(s[i..<e])) else { return nil }
    return (e, u)
}
