import SwiftUI
import UIKit

// A message's Markdown for the data app, as the desktop draws it (native/gpui/src/screens/md.rs): headings, bold, italic, strike, inline code,
// fenced code with Copy, bullet, numbered and to-do lists, quotes, tables that scroll sideways, links (backplane://thread/<id> opens the thread),
// images, rules. The same grammar as the Android app's DataMarkdown.kt.
// A reply that is still arriving is parsed again as it grows: whatever is not closed yet (a fence, a **, a table's separator) shows as it stands.
// Parsed blocks and styled text are cached by their source text, so a message that did not change costs nothing to draw again.
indirect enum DMdBlock: Equatable {
    case p(String), h(Int, String)
    /// task: -1 not a to-do, 0 open, 1 done
    case li(depth: Int, marker: String, task: Int, text: String)
    case code(lang: String, text: String), quote([DMdBlock])
    /// aligns: 0 left, 1 centre, 2 right
    case table(head: [String], aligns: [Int], rows: [[String]])
    case image(alt: String, url: String)
    case rule
}

private func rx(_ p: String) -> NSRegularExpression { try! NSRegularExpression(pattern: p) }
private let reHead = rx("^(#{1,6})\\s+(.*?)(\\s+#+)?$")
private let reRule = rx("^([-*_])(\\s*\\1){2,}$")
private let reItem = rx("^(\\s*)([-*+]|\\d{1,9}[.)])\\s+(.*)$")
private let reTask = rx("^\\[([ xX])\\]\\s+(.*)$")
private let reSep = rx("^\\|?\\s*:?-+:?\\s*(\\|\\s*:?-+:?\\s*)*\\|?$")
private let reImg = rx("^!\\[([^\\]]*)\\]\\(\\s*<?([^)\\s>]+)>?(\\s+\"[^\"]*\")?\\s*\\)$")

private extension String {
    func groups(_ r: NSRegularExpression) -> [String]? {
        let ns = self as NSString
        guard let m = r.firstMatch(in: self, range: NSRange(location: 0, length: ns.length)) else { return nil }
        return (0..<m.numberOfRanges).map { m.range(at: $0).location == NSNotFound ? "" : ns.substring(with: m.range(at: $0)) }
    }
    func matches(_ r: NSRegularExpression) -> Bool { groups(r) != nil }
    var trimmed: String { trimmingCharacters(in: .whitespaces) }
}

private func isFence(_ t: String) -> Bool { t.hasPrefix("```") || t.hasPrefix("~~~") }
private func indentOf(_ l: String) -> Int { var n = 0; for c in l { if c == " " { n += 1 } else if c == "\t" { n += 4 } else { break } }; return n }
private func startsBlock(_ l: String, _ next: String?) -> Bool {
    let t = l.trimmed
    if t.isEmpty || isFence(t) || t.matches(reHead) || t.matches(reRule) || l.matches(reItem) || t.hasPrefix(">") { return true }
    if t.contains("|"), let n = next, n.trimmed.matches(reSep), n.contains("-") { return true }
    return false
}

func dSplitCells(_ row: String) -> [String] {
    var t = row.trimmed; if t.hasPrefix("|") { t.removeFirst() }; if t.hasSuffix("|") && !t.hasSuffix("\\|") { t.removeLast() }
    var out: [String] = []; var b = ""; var tick = false; let cs = Array(t); var i = 0
    while i < cs.count {
        let c = cs[i]
        if c == "\\", i + 1 < cs.count, cs[i + 1] == "|" { b.append("|"); i += 2; continue }
        if c == "`" { tick.toggle() }
        if c == "|" && !tick { out.append(b.trimmed); b = "" } else { b.append(c) }
        i += 1
    }
    out.append(b.trimmed); return out
}

private func parseBlocks(_ lines: [String]) -> [DMdBlock] {
    var out: [DMdBlock] = []; var i = 0
    while i < lines.count {
        let l = lines[i]; let t = l.trimmed
        if t.isEmpty { i += 1; continue }
        if isFence(t) {
            let mark = String(t.prefix(3)); let lang = String(t.dropFirst(3)).trimmed.split(whereSeparator: { $0 == " " }).first.map(String.init) ?? ""
            let ind = indentOf(l); var b: [String] = []; i += 1
            while i < lines.count, !lines[i].trimmed.hasPrefix(mark) { var s = Substring(lines[i]); var k = 0; while k < ind, s.first == " " { s.removeFirst(); k += 1 }; b.append(String(s)); i += 1 }
            i += 1; out.append(.code(lang: lang, text: b.joined(separator: "\n"))); continue
        }
        if let h = t.groups(reHead) { out.append(.h(h[1].count, h[2])); i += 1; continue }
        if t.matches(reRule) { out.append(.rule); i += 1; continue }
        if t.hasPrefix(">") {
            var q: [String] = []
            while i < lines.count, lines[i].trimmed.hasPrefix(">") { var s = lines[i].trimmed; s.removeFirst(); if s.hasPrefix(" ") { s.removeFirst() }; q.append(s); i += 1 }
            out.append(.quote(parseBlocks(q))); continue
        }
        if let m = l.groups(reItem) {
            var body = m[3]; var task = -1
            if let k = body.groups(reTask) { task = k[1] == " " ? 0 : 1; body = k[2] }
            let mk = m[2]; let depth = min(indentOf(l) / 2, 6); let base = indentOf(l); i += 1
            var text = body
            while i < lines.count, !lines[i].trimmed.isEmpty, indentOf(lines[i]) >= base + 2, !lines[i].matches(reItem), !isFence(lines[i].trimmed) { text += "\n" + lines[i].trimmed; i += 1 }
            out.append(.li(depth: depth, marker: mk.first!.isNumber ? mk : "•", task: task, text: text)); continue
        }
        if t.contains("|"), i + 1 < lines.count, lines[i + 1].trimmed.matches(reSep), lines[i + 1].contains("-") {
            let head = dSplitCells(t)
            let aligns = dSplitCells(lines[i + 1]).map { c in c.hasPrefix(":") && c.hasSuffix(":") ? 1 : c.hasSuffix(":") ? 2 : 0 }
            i += 2; var rows: [[String]] = []
            while i < lines.count, !lines[i].trimmed.isEmpty, lines[i].contains("|") { rows.append(dSplitCells(lines[i])); i += 1 }
            out.append(.table(head: head, aligns: aligns, rows: rows)); continue
        }
        if let m = t.groups(reImg) { out.append(.image(alt: m[1], url: m[2])); i += 1; continue }
        var para = [l.trimmingCharacters(in: CharacterSet(charactersIn: " \t"))]; i += 1
        while i < lines.count, !startsBlock(lines[i], i + 1 < lines.count ? lines[i + 1] : nil) { para.append(lines[i]); i += 1 }
        out.append(.p(para.joined(separator: "\n")))
    }
    return out
}

private final class DMdCache { nonisolated(unsafe) static let blocks = NSCache<NSString, Box<[DMdBlock]>>(); nonisolated(unsafe) static let inline = NSCache<NSString, Box<AttributedString>>() }
private final class Box<T> { let v: T; init(_ v: T) { self.v = v } }

func dMdBlocks(_ src: String) -> [DMdBlock] {
    if let c = DMdCache.blocks.object(forKey: src as NSString) { return c.v }
    let b = parseBlocks(src.replacingOccurrences(of: "\r\n", with: "\n").components(separatedBy: "\n"))
    DMdCache.blocks.setObject(Box(b), forKey: src as NSString); return b
}

// MARK: inline

private struct Fmt { var bold = false, italic = false, strike = false, link: URL? = nil }

private func find(_ s: [Character], _ tok: [Character], from: Int) -> Int? {
    guard from >= 0, !tok.isEmpty, s.count >= tok.count else { return nil }
    var i = from
    while i + tok.count <= s.count { if Array(s[i..<i + tok.count]) == tok { return i }; i += 1 }
    return nil
}

private func inl(_ s: [Character], _ f: Fmt, _ size: CGFloat, _ out: inout AttributedString, _ depth: Int) {
    func put(_ t: String, _ f: Fmt, code: Bool = false) {
        var a = AttributedString(t)
        let sz = code ? size - 0.5 : size
        // the bundled mono face has no italic: slant it (about 12 degrees)
        if f.italic {
            let ui = Mono.uiFont(size: sz, weight: f.bold ? .bold : .regular)
            a.font = Font(UIFont(descriptor: ui.fontDescriptor.withMatrix(CGAffineTransform(a: 1, b: 0, c: 0.21, d: 1, tx: 0, ty: 0)), size: sz))
        } else { a.font = Font.mono(size: sz, weight: f.bold ? .bold : .regular) }
        if f.strike { a.strikethroughStyle = .single }
        if code { a.backgroundColor = BackplaneDesign.raised }
        if let u = f.link { a.link = u; a.foregroundColor = BackplaneDesign.accent; a.underlineStyle = .single }
        out.append(a)
    }
    var i = 0; var plain = ""
    func flush() { if !plain.isEmpty { put(plain, f); plain = "" } }
    while i < s.count {
        let c = s[i]
        if depth > 8 { plain += String(s[i...]); break }
        if c == "\\", i + 1 < s.count, "\\`*_{}[]()#+-.!|~<>".contains(s[i + 1]) { plain.append(s[i + 1]); i += 2; continue }
        if c == "`" {
            var n = 0; while i + n < s.count, s[i + n] == "`" { n += 1 }
            let tick = [Character](repeating: "`", count: n)
            if let e = find(s, tick, from: i + n) {
                flush(); var inner = String(s[(i + n)..<e]); if inner.hasPrefix(" "), inner.hasSuffix(" "), !inner.trimmed.isEmpty { inner = inner.trimmed }
                var g = f; if dIsPathText(inner), let u = dPathURL(inner) { g.link = u }
                put(inner, g, code: true); i = e + n; continue
            }
            plain += String(tick); i += n; continue
        }
        if c == "[" || (c == "!" && i + 1 < s.count && s[i + 1] == "[") {
            let st = c == "!" ? i + 1 : i
            var d = 0, j = st, close = -1
            while j < s.count { if s[j] == "\\" { j += 2; continue }; if s[j] == "[" { d += 1 } else if s[j] == "]" { d -= 1; if d == 0 { close = j; break } }; j += 1 }
            if close > 0, close + 1 < s.count, s[close + 1] == "(" {
                var pd = 0, k = close + 1, end = -1
                while k < s.count { if s[k] == "(" { pd += 1 } else if s[k] == ")" { pd -= 1; if pd == 0 { end = k; break } }; k += 1 }
                if end > 0 {
                    var url = String(s[(close + 2)..<end]).trimmed
                    if let sp = url.firstIndex(of: " ") { url = String(url[..<sp]) }
                    url = url.trimmingCharacters(in: CharacterSet(charactersIn: "<>"))
                    let label = Array(s[(st + 1)..<close])
                    flush(); var g = f; g.link = dIsPathText(url) ? dPathURL(url) : (URL(string: url) ?? URL(string: url.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) ?? ""))
                    if c == "!" { put(label.isEmpty ? url : String(label), g) } else { inl(label, g, size, &out, depth + 1) }
                    i = end + 1; continue
                }
            }
        }
        if let (e, u) = dWebLink(s, i) { flush(); var g = f; g.link = u; put(String(s[i..<e]), g); i = e; continue }
        if c == "*" || c == "_", i + 2 < s.count, s[i + 1] == c, !s[i + 2].isWhitespace, c == "*" || i == 0 || !(s[i - 1].isLetter || s[i - 1].isNumber) {
            let tok = [c, c]; var e = find(s, tok, from: i + 2)
            while let x = e, x > 0, s[x - 1].isWhitespace { e = find(s, tok, from: x + 2) }
            while let x = e, x + 2 < s.count, s[x + 2] == c { e = x + 1 }
            if let x = e, x > i + 2 { flush(); var g = f; g.bold = true; inl(Array(s[(i + 2)..<x]), g, size, &out, depth + 1); i = x + 2; continue }
        }
        if c == "~", i + 2 < s.count, s[i + 1] == "~", !s[i + 2].isWhitespace, let x = find(s, ["~", "~"], from: i + 2), x > i + 2 {
            flush(); var g = f; g.strike = true; inl(Array(s[(i + 2)..<x]), g, size, &out, depth + 1); i = x + 2; continue
        }
        if c == "*" || c == "_", i + 1 < s.count, !s[i + 1].isWhitespace, s[i + 1] != c, c == "*" || i == 0 || !(s[i - 1].isLetter || s[i - 1].isNumber) {
            var j = i + 1; var e = -1
            while j < s.count {
                if s[j] == "\\" { j += 2; continue }
                if s[j] == "`", let q = find(s, ["`"], from: j + 1) { j = q + 1; continue }
                if s[j] == c {
                    if j + 1 < s.count, s[j + 1] == c { j += 2; continue }
                    if !s[j - 1].isWhitespace, c == "*" || j + 1 == s.count || !(s[j + 1].isLetter || s[j + 1].isNumber) { e = j; break }
                }
                j += 1
            }
            if e > i + 1 { flush(); var g = f; g.italic = true; inl(Array(s[(i + 1)..<e]), g, size, &out, depth + 1); i = e + 1; continue }
        }
        // emails, phone numbers and file paths in running text
        if let (e, u) = dAutoLink(s, i) { flush(); var g = f; g.link = u; put(String(s[i..<e]), g); i = e; continue }
        plain.append(c); i += 1
    }
    flush()
}

func dMdInline(_ text: String, size: CGFloat, bold: Bool = false) -> AttributedString {
    let key = "\(size)|\(bold)|" + text
    if let c = DMdCache.inline.object(forKey: key as NSString) { return c.v }
    var out = AttributedString(); inl(Array(text), Fmt(bold: bold), size, &out, 0)
    DMdCache.inline.setObject(Box(out), forKey: key as NSString); return out
}

// MARK: pictures

final class DMdPics {
    nonisolated(unsafe) static let cache = NSCache<NSString, UIImage>()
    static func load(_ url: String) async -> UIImage? {
        if let i = cache.object(forKey: url as NSString) { return i }
        guard let u = URL(string: url), u.scheme == "https" || u.scheme == "http", let (d, r) = try? await URLSession.shared.data(from: u),
              (r as? HTTPURLResponse)?.statusCode == 200, let img = UIImage(data: d) else { return nil }
        cache.setObject(img, forKey: url as NSString); return img
    }
}

// MARK: views

struct DataMarkdown: View {
    let text: String
    /// the thread the message is in: a file path in it is looked for in that thread's project
    var thread = ""
    var body: some View {
        DMdBlocks(blocks: dMdBlocks(text))
            .environment(\.openURL, OpenURLAction { url in dMdOpen(url, thread: thread) })
    }
}

/// What a tap on a link does (DataLinks.swift): web addresses, emails and phone numbers go to the system (Safari, Mail, Phone); a backplane:// link or a path is
/// asked of the hub and opens its screen here, or says why it cannot.
@MainActor func dMdOpen(_ url: URL, thread: String = "") -> OpenURLAction.Result {
    #if DEBUG
    DataClient.shared.lastLink = url.absoluteString
    #endif
    switch url.scheme {
    case "http", "https", "mailto", "tel": return .systemAction
    case "backplane", AppIdentity.scheme, dPathScheme: DataClient.shared.openLink(url, thread: thread); return .handled
    default: return .discarded
    }
}

private struct DMdBlocks: View {
    let blocks: [DMdBlock]
    @ScaledMetric(relativeTo: .footnote) private var body13: CGFloat = 13
    private func head(_ n: Int) -> CGFloat { n <= 1 ? 18 : n == 2 ? 16 : n == 3 ? 14 : 13 }
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            ForEach(Array(blocks.enumerated()), id: \.offset) { _, b in block(b) }
        }.frame(maxWidth: .infinity, alignment: .leading)
    }
    @ViewBuilder private func block(_ b: DMdBlock) -> some View {
        switch b {
        case .p(let s): Text(dMdInline(s, size: body13)).foregroundStyle(BackplaneDesign.text).lineSpacing(3).textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading)
        case .h(let n, let s): Text(dMdInline(s, size: head(n) * body13 / 13, bold: true)).foregroundStyle(BackplaneDesign.text).padding(.top, 4).textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading)
        case .li(let depth, let marker, let task, let s):
            HStack(alignment: .firstTextBaseline, spacing: 6) {
                if task >= 0 {
                    ZStack { Rectangle().fill(task == 1 ? BackplaneDesign.accent : Color.clear); Rectangle().strokeBorder(task == 1 ? BackplaneDesign.accent : BackplaneDesign.dim, lineWidth: 1)
                        if task == 1 { Text("✓").font(.system(size: 9, weight: .bold)).foregroundStyle(BackplaneDesign.onAccent) } }
                        .frame(width: 13, height: 13).alignmentGuide(.firstTextBaseline) { $0[.bottom] - 1 }
                } else { Text(marker).dmono(13).foregroundStyle(BackplaneDesign.dim).frame(minWidth: 16, alignment: .leading) }
                Text(dMdInline(s, size: body13)).foregroundStyle(task == 1 ? BackplaneDesign.dim : BackplaneDesign.text).strikethrough(task == 1).lineSpacing(3).textSelection(.enabled)
            }.padding(.leading, CGFloat(depth) * 14).frame(maxWidth: .infinity, alignment: .leading)
        case .code(let lang, let s): DMdCode(lang: lang, text: s)
        case .quote(let inner): HStack(alignment: .top, spacing: 10) { Rectangle().fill(BackplaneDesign.dim).frame(width: 2); DMdBlocks(blocks: inner) }.fixedSize(horizontal: false, vertical: true)
        case .table(let head, let aligns, let rows): DMdTable(head: head, aligns: aligns, rows: rows, size: body13 - 1)
        case .image(let alt, let url): DMdImage(alt: alt, url: url)
        case .rule: DLine()
        }
    }
}

private struct DMdCode: View {
    let lang: String, text: String
    @State private var copied = false
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text(lang).dmono(11).foregroundStyle(BackplaneDesign.dim)
                Spacer(minLength: 0)
                Button(Tr.t(copied ? "Copied" : "Copy")) {
                    UIPasteboard.general.string = text; copied = true
                    Task { try? await Task.sleep(nanoseconds: 1_500_000_000); copied = false }
                }.dmono(11).foregroundStyle(BackplaneDesign.accent).padding(.vertical, 6).accessibilityIdentifier("md.copy")
            }.padding(.horizontal, 10)
            ScrollView(.horizontal, showsIndicators: false) { Text(text).dmono(12).lineSpacing(2).fixedSize().textSelection(.enabled).padding(.horizontal, 10).padding(.bottom, 10) }
        }.frame(maxWidth: .infinity, alignment: .leading).dbox(fill: BackplaneDesign.panel)
    }
}

private struct DMdTable: View {
    let head: [String], aligns: [Int], rows: [[String]], size: CGFloat
    var body: some View {
        let cols = max(head.count, rows.map(\.count).max() ?? 0)
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(alignment: .top, spacing: 0) {
                ForEach(0..<cols, id: \.self) { c in
                    VStack(alignment: .leading, spacing: 0) {
                        cell(c < head.count ? head[c] : "", c, bold: true).background(BackplaneDesign.panel)
                        ForEach(Array(rows.enumerated()), id: \.offset) { _, r in cell(c < r.count ? r[c] : "", c, bold: false).overlay(alignment: .top) { Rectangle().fill(BackplaneDesign.line).frame(height: 0.5) } }
                    }.fixedSize(horizontal: true, vertical: false)
                }
            }.overlay(Rectangle().strokeBorder(BackplaneDesign.line, lineWidth: 1))
        }.accessibilityIdentifier("md.table")
    }
    private func cell(_ s: String, _ c: Int, bold: Bool) -> some View {
        let a = c < aligns.count ? aligns[c] : 0
        return Text(dMdInline(s, size: size, bold: bold)).foregroundStyle(BackplaneDesign.text).lineLimit(1).fixedSize().textSelection(.enabled)
            .padding(.horizontal, 10).padding(.vertical, 6).frame(minWidth: 0, maxWidth: .infinity, alignment: a == 1 ? .center : a == 2 ? .trailing : .leading)
    }
}

private struct DMdImage: View {
    let alt: String, url: String
    @State private var img: UIImage?
    @State private var failed = false
    var body: some View {
        Group {
            if let i = img ?? DMdPics.cache.object(forKey: url as NSString) { Image(uiImage: i).resizable().scaledToFit().frame(maxWidth: i.size.width, maxHeight: 360).dbox().accessibilityIdentifier("md.image") }
            else if failed { Text(alt.isEmpty ? url : alt).dmono(12).foregroundStyle(BackplaneDesign.dim) }
            else { Rectangle().fill(BackplaneDesign.panel).frame(height: 96) }
        }.frame(maxWidth: .infinity, alignment: .leading)
        .task(id: url) { if img == nil { img = await DMdPics.load(url); failed = img == nil } }
    }
}

#if DEBUG
/// A check without a hub: BACKPLANE_MDPREVIEW=<file> draws it as a message; BACKPLANE_MDSTREAM=1 lets it arrive 7 characters every 60 ms.
struct DMdPreview: View {
    let full: String, stream: Bool
    @State private var n = 0
    init(full: String, stream: Bool) {
        self.full = full; self.stream = stream
        // the check's picture stands in for any image address
        if let f = ProcessInfo.processInfo.environment["BACKPLANE_MDPREVIEW"], let img = UIImage(contentsOfFile: (f as NSString).deletingLastPathComponent + "/pic.png") {
            for b in dMdBlocks(full) { if case .image(_, let u) = b { DMdPics.cache.setObject(img, forKey: u as NSString) } }
        }
    }
    var body: some View {
        ScrollView { DataMarkdown(text: String(full.prefix(stream ? n : full.count))).padding(14) }
            .background(BackplaneDesign.view)
            .task { while stream, n < full.count { try? await Task.sleep(nanoseconds: 60_000_000); n += 7 } }
    }
}
#endif
