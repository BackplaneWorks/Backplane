import SwiftUI

// Draws the markdown blocks Md.render made (p, h3-h5, ul/li, ol/li,
// blockquote, hr, table/tr/th/td, pre, code, strong, em, a) with native
// text styles.

// a file link as a URL a thread's openURL handler knows: a tap asks for
// the file's menu ("file-menu" with the path)
enum FileLink {
    static func url(_ path: String) -> URL? {
        var c = URLComponents()
        c.scheme = "backplane-file"
        c.host = "f"
        c.queryItems = [URLQueryItem(name: "p", value: path)]
        return c.url
    }

    static func path(_ u: URL) -> String? {
        guard u.scheme == "backplane-file" else { return nil }
        return URLComponents(url: u, resolvingAgainstBaseURL: false)?.queryItems?.first { $0.name == "p" }?.value
    }
}

private func inline(_ bs: [Block]) -> AttributedString {
    var out = AttributedString()
    for b in bs {
        if let t = b.text {
            out += AttributedString(t)
            continue
        }
        var part = inline(b.kids ?? [])
        switch b.tag {
        case "code":
            part.font = .body.monospaced()
            part.backgroundColor = Color(.secondarySystemFill)
        case "strong":
            part.font = .body.weight(.semibold)
        case "em":
            part.font = .body.italic()
        // Explicit references go through the same OpenURLAction as external
        // links; bare IDs retain the existing plain-text phone presentation.
        case "a":
            if let ref = b.ref {
                if b.bare != true { part.link = URL(string: ref) }
            } else if let h = b.href, let u = b.file == true ? FileLink.url(h) : URL(string: h) {
                part.link = u
            }
            if part.link != nil { part.underlineStyle = .single }
        default:
            break
        }
        out += part
    }
    return out
}

struct MarkdownView: View {
    let blocks: [Block]

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            ForEach(Array(blocks.enumerated()), id: \.offset) { _, b in
                BlockView(block: b)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

private struct BlockView: View {
    let block: Block

    var body: some View {
        if let t = block.text {
            Text(t)
        } else {
            let kids = block.kids ?? []
            switch block.tag {
            case "p": Text(inline(kids))
            case "h3": Text(inline(kids)).font(.title3.bold())
            case "h4": Text(inline(kids)).font(.headline)
            case "h5": Text(inline(kids)).font(.subheadline.bold())
            case "ul":
                VStack(alignment: .leading, spacing: 4) {
                    ForEach(Array(kids.enumerated()), id: \.offset) { _, li in
                        HStack(alignment: .firstTextBaseline, spacing: 8) {
                            Text("•")
                            Text(inline(li.kids ?? []))
                        }
                    }
                }
            case "ol":
                VStack(alignment: .leading, spacing: 4) {
                    ForEach(Array(kids.enumerated()), id: \.offset) { _, li in
                        HStack(alignment: .firstTextBaseline, spacing: 6) {
                            Text("\(li.n ?? "").").monospacedDigit()
                            Text(inline(li.kids ?? []))
                        }
                    }
                }
            case "blockquote":
                Text(inline(kids))
                    .foregroundStyle(.secondary)
                    .padding(.leading, 10)
                    .overlay(alignment: .leading) { Rectangle().fill(Color(.separator)).frame(width: 2) }
            case "hr":
                Rectangle().fill(Color(.separator)).frame(height: 1).padding(.vertical, 6)
            case "table":
                ScrollView(.horizontal, showsIndicators: false) {
                    TableView(rows: kids.filter { $0.tag == "tr" })
                }
            case "pre":
                ScrollView(.horizontal, showsIndicators: false) {
                    Text(block.plain).font(.callout.monospaced()).padding(12)
                }
                .background(Color(.secondarySystemBackground))
            // the web page's copy button: the message's context menu copies
            case "button": EmptyView()
            default: MarkdownView(blocks: kids)
            }
        }
    }
}

// a table: each column as wide as its widest cell (up to 240 pt, wrapping
// past that), each row as tall as its tallest cell; wider than the screen,
// it scrolls sideways
private struct TableView: View {
    let rows: [Block]

    var body: some View {
        let cols = rows.map { ($0.kids ?? []).count }.max() ?? 0
        TableLayout(cols: cols) {
            ForEach(Array(rows.enumerated()), id: \.offset) { _, r in
                let cells = r.kids ?? []
                ForEach(0..<cols, id: \.self) { i in
                    let c = i < cells.count ? cells[i] : nil
                    let th = c?.tag == "th"
                    Text(inline(c?.kids ?? []))
                        .font(th ? .callout.weight(.semibold) : .callout)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 4)
                        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                        .background(th ? Color(.secondarySystemFill) : Color.clear)
                        .border(Color(.separator), width: 0.5)
                }
            }
        }
    }
}

private struct TableLayout: Layout {
    let cols: Int
    let cap: CGFloat = 240

    private func grid(_ vs: Subviews) -> ([CGFloat], [CGFloat]) {
        guard cols > 0 else { return ([], []) }
        var w = [CGFloat](repeating: 0, count: cols)
        for (k, v) in vs.enumerated() {
            w[k % cols] = max(w[k % cols], min(v.sizeThatFits(.unspecified).width, cap))
        }
        var h = [CGFloat](repeating: 0, count: (vs.count + cols - 1) / cols)
        for (k, v) in vs.enumerated() {
            h[k / cols] = max(h[k / cols], v.sizeThatFits(ProposedViewSize(width: w[k % cols], height: nil)).height)
        }
        return (w, h)
    }

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let (w, h) = grid(subviews)
        return CGSize(width: w.reduce(0, +), height: h.reduce(0, +))
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let (w, h) = grid(subviews)
        var y = bounds.minY
        for r in h.indices {
            var x = bounds.minX
            for i in 0..<cols where r * cols + i < subviews.count {
                subviews[r * cols + i].place(at: CGPoint(x: x, y: y), proposal: ProposedViewSize(width: w[i], height: h[r]))
                x += w[i]
            }
            y += h[r]
        }
    }
}
