import SwiftUI

// A reply's Markdown for the data app: paragraphs, headings, lists, fenced code, quotes, tables and inline code, bold, italics and links, in mono.
// The same as the Android app's DataMarkdown.kt. A live reply is parsed again as it grows: what is not closed yet shows as it stands.
enum DMdBlock: Equatable {
    case p(String), h(Int, String), li(String, String), code(String), quote(String), table([[String]]), rule
}

func dMdBlocks(_ src: String) -> [DMdBlock] {
    var out: [DMdBlock] = []; var para: [String] = []
    func flush() { if !para.isEmpty { out.append(.p(para.joined(separator: "\n"))); para = [] } }
    let lines = src.components(separatedBy: "\n"); var i = 0
    while i < lines.count {
        let l = lines[i]; let t = l.trimmingCharacters(in: .whitespaces)
        if t.hasPrefix("```") {
            flush(); var b: [String] = []; i += 1
            while i < lines.count, !lines[i].trimmingCharacters(in: .whitespaces).hasPrefix("```") { b.append(lines[i]); i += 1 }
            out.append(.code(b.joined(separator: "\n")))
        } else if t.isEmpty { flush() }
        else if let n = t.firstIndex(where: { $0 != "#" }), t.hasPrefix("#"), t.distance(from: t.startIndex, to: n) <= 6, t[n] == " " { flush(); out.append(.h(t.distance(from: t.startIndex, to: n), t[n...].trimmingCharacters(in: .whitespaces))) }
        else if t.count >= 3, Set(t).count == 1, "-*_".contains(t.first!) { flush(); out.append(.rule) }
        else if t.hasPrefix("- ") || t.hasPrefix("* ") || t.hasPrefix("+ ") { flush(); out.append(.li("•", String(t.dropFirst(2)))) }
        else if let d = t.firstIndex(where: { !$0.isNumber }), d != t.startIndex, ".)".contains(t[d]), t[t.index(after: d)...].hasPrefix(" ") { flush(); out.append(.li(String(t[...d]), String(t[t.index(d, offsetBy: 2)...]))) }
        else if t.hasPrefix("|") {
            flush(); var rows: [[String]] = []
            while i < lines.count, lines[i].trimmingCharacters(in: .whitespaces).hasPrefix("|") {
                let r = lines[i].trimmingCharacters(in: CharacterSet(charactersIn: " |")).components(separatedBy: "|").map { $0.trimmingCharacters(in: .whitespaces) }
                if !r.allSatisfy({ !$0.isEmpty && $0.allSatisfy { "-:".contains($0) } }) { rows.append(r) }
                i += 1
            }
            i -= 1; out.append(.table(rows))
        }
        else if t.hasPrefix("> ") { flush(); out.append(.quote(String(t.dropFirst(2)))) }
        else { para.append(l) }
        i += 1
    }
    flush(); return out
}

struct DataMarkdown: View {
    let text: String
    private func inline(_ s: String) -> AttributedString {
        var a = (try? AttributedString(markdown: s, options: .init(allowsExtendedAttributes: false, interpretedSyntax: .inlineOnlyPreservingWhitespace))) ?? AttributedString(s)
        for r in a.runs where r.inlinePresentationIntent?.contains(.code) == true { a[r.range].backgroundColor = BackplaneDesign.raised }
        for r in a.runs where r.link != nil { a[r.range].foregroundColor = BackplaneDesign.accent }
        return a
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            ForEach(Array(dMdBlocks(text).enumerated()), id: \.offset) { _, b in
                switch b {
                case .p(let s): Text(inline(s)).dmono(13).lineSpacing(3).textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading)
                case .h(let n, let s): Text(inline(s)).dmono(n <= 1 ? 17 : n == 2 ? 15 : 14, bold: true).padding(.top, 4).textSelection(.enabled)
                case .li(let m, let s): HStack(alignment: .firstTextBaseline, spacing: 6) { Text(m).dmono(13).foregroundStyle(BackplaneDesign.dim).frame(minWidth: 16, alignment: .leading); Text(inline(s)).dmono(13).lineSpacing(3).textSelection(.enabled) }
                case .code(let s):
                    ScrollView(.horizontal, showsIndicators: false) { Text(s).dmono(12).textSelection(.enabled).padding(8) }.frame(maxWidth: .infinity, alignment: .leading).dbox(fill: BackplaneDesign.panel)
                case .quote(let s): HStack(spacing: 8) { Rectangle().fill(BackplaneDesign.dim).frame(width: 2); Text(inline(s)).dmono(13).foregroundStyle(BackplaneDesign.dim) }.fixedSize(horizontal: false, vertical: true)
                case .table(let rows):
                    let w = (0..<(rows.map(\.count).max() ?? 0)).map { c in rows.map { c < $0.count ? $0[c].count : 0 }.max() ?? 0 }
                    ScrollView(.horizontal, showsIndicators: false) {
                        VStack(alignment: .leading, spacing: 2) {
                            ForEach(Array(rows.enumerated()), id: \.offset) { ri, r in
                                Text(r.enumerated().map { c, x in x.padding(toLength: c < w.count ? w[c] : x.count, withPad: " ", startingAt: 0) }.joined(separator: "  ")).dmono(11).foregroundStyle(ri == 0 ? BackplaneDesign.text : BackplaneDesign.dim)
                            }
                        }.padding(8)
                    }.frame(maxWidth: .infinity, alignment: .leading).dbox(fill: BackplaneDesign.panel)
                case .rule: DLine()
                }
            }
        }
    }
}
