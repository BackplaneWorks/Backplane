import SwiftUI
import UIKit

// Terminals on the phone (feature `terms`): the card in a thread, the full screen, and the Terminals screen of a project or the hub. The hub streams a terminal only while
// one of these is up (TermStore); every redraw is a push of bytes, never a timer.

/// The terminal's state in the hub's words, flat.
private struct TermState: View {
    let state: String
    var body: some View {
        Text(Tr.t(TermCard.word(state))).dmono(11).lineLimit(1)
            .foregroundStyle(TermCard.live(state) ? BackplaneDesign.ok : state.hasPrefix("exited 0") ? BackplaneDesign.dim : state.hasPrefix("exited") || state == "failed to start" ? BackplaneDesign.bad : BackplaneDesign.dim)
    }
}

// MARK: the card

/// A few rows of a terminal's screen, drawn as the last screen held.
private struct TermCardSurface: UIViewRepresentable {
    let screen: TermScreen
    let rev: Int
    func makeUIView(context: Context) -> TermSurface { let v = TermSurface(); v.isUserInteractionEnabled = false; return v }
    private func fit(_ width: CGFloat) -> CGFloat {
        let cols = max(CGFloat(screen.vt.cols), 20)
        return min(max((width / (cols * TermSurface.aspect)).rounded(.down), 6), 11)
    }
    private var shown: Int { min(8, screen.vt.rows) }
    func updateUIView(_ v: TermSurface, context: Context) {
        let r = TermCard.rows(screen.vt.lines(0), cursor: screen.vt.cursor, n: shown)
        v.lines = r.lines; v.cursor = r.cursorRow.map { (screen.vt.cursor.x, $0) }
        v.isAccessibilityElement = true; v.accessibilityLabel = Tr.t("Terminal screen"); v.accessibilityValue = r.lines.map { $0.runs.map(\.text).joined() }.joined(separator: "\n")
        _ = rev
    }
    func sizeThatFits(_ proposal: ProposedViewSize, uiView v: TermSurface, context: Context) -> CGSize? {
        let w = proposal.width ?? 320
        v.fontSize = fit(w)
        return CGSize(width: w, height: CGFloat(shown) * v.lh)
    }
}

struct DataTermCard: View {
    let client: DataClient
    let info: DTermInfo
    @State private var seen = false

    var body: some View {
        let screen = client.terms.screen(info.key, cols: info.cols, rows: info.rows)
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 8) {
                Image(systemName: "terminal").font(.system(size: 12)).foregroundStyle(BackplaneDesign.dim)
                Text(info.title.isEmpty ? Tr.t("Terminal") : info.title).dmono(12, bold: true).foregroundStyle(BackplaneDesign.text).lineLimit(1)
                Spacer(minLength: 4)
                if screen.missing { Text(Tr.t("gone")).dmono(11).foregroundStyle(BackplaneDesign.dim) } else { TermState(state: screen.state).accessibilityIdentifier("term.card.state") }
                if screen.live {
                    Button { client.terms.stop(info.key) } label: { Text(Tr.t("Stop")).dmono(11, bold: true).foregroundStyle(BackplaneDesign.bad).padding(.horizontal, 8).frame(minHeight: 28).overlay(Rectangle().strokeBorder(BackplaneDesign.line, lineWidth: 1)) }
                        .buttonStyle(.plain).accessibilityIdentifier("term.card.stop")
                }
            }.padding(.horizontal, 8).frame(minHeight: 34).background(BackplaneDesign.raised)
            TermCardSurface(screen: screen, rev: screen.rev).accessibilityIdentifier("term.card.screen")
        }
        .overlay(Rectangle().strokeBorder(BackplaneDesign.line, lineWidth: 1))
        .contentShape(Rectangle())
        .onTapGesture { client.terms.open(info.key, title: info.title) }
        .modifier(TermVisible(seen: $seen, key: info.key, terms: client.terms, cols: info.cols, rows: info.rows))
        .padding(.vertical, 4).accessibilityElement(children: .contain).accessibilityIdentifier("data.entry.term")
    }
}

/// Watch the terminal while it is on screen (from iOS 18 the scroll position decides; before it, the thread being open does).
private struct TermVisible: ViewModifier {
    @Binding var seen: Bool
    let key: String
    let terms: TermStore
    let cols: Int, rows: Int
    private func set(_ on: Bool) { guard on != seen else { return }; seen = on; if on { terms.want(key, cols: cols, rows: rows) } else { terms.unwant(key) } }
    func body(content: Content) -> some View {
        if #available(iOS 18, *) {
            content.onScrollVisibilityChange(threshold: 0.05) { set($0) }.onDisappear { set(false) }
        } else {
            content.onAppear { set(true) }.onDisappear { set(false) }
        }
    }
}

// MARK: full screen

/// What the bar reaches of the UIKit view.
@MainActor final class TermHandle { weak var surface: TermInputSurface? }

private struct TermInputRepresentable: UIViewRepresentable {
    let key: String
    let screen: TermScreen
    let rev: Int
    let scroll: Int
    let fontSize: CGFloat
    let terms: TermStore
    let handle: TermHandle
    let onFont: (CGFloat) -> Void
    let onScroll: (Int) -> Void

    func makeUIView(context: Context) -> TermInputSurface {
        let v = TermInputSurface()
        v.fontSize = fontSize; handle.surface = v
        v.vt = { [screen] in screen.vt }
        v.onBytes = { [terms, key] in terms.write(key, $0) }
        v.onGrid = { [terms, key] c, r in terms.resize(key, cols: c, rows: r) }
        v.onFontSize = { onFont($0) }
        return v
    }
    func updateUIView(_ v: TermInputSurface, context: Context) {
        v.fontSize = fontSize
        v.onScroll = { [weak v] in if let v { onScroll(v.scroll) } }
        let history = screen.vt.history
        let sc = min(v.scroll, history)
        let lines = screen.vt.lines(sc)
        v.lines = lines
        let c = screen.vt.cursor
        v.cursor = c.visible && sc == 0 ? (c.x, c.y) : nil
        v.accessibilityValue = lines.map { $0.runs.map(\.text).joined() }.joined(separator: "\n")
        _ = rev; _ = scroll
    }
}

struct TermFullView: View {
    let client: DataClient
    let route: TermRoute
    @AppStorage("term.font") private var font = 12.0
    @State private var scroll = 0
    @State private var handle = TermHandle()
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        let terms = client.terms
        let screen = terms.screen(route.key)
        VStack(spacing: 0) {
            HStack(spacing: 10) {
                Button { dismiss() } label: { Image(systemName: "xmark").frame(width: 36, height: 36).foregroundStyle(BackplaneDesign.dim) }.accessibilityLabel(Tr.t("Close")).accessibilityIdentifier("term.close")
                VStack(alignment: .leading, spacing: 0) {
                    Text(screen.title.isEmpty ? (route.title.isEmpty ? Tr.t("Terminal") : route.title) : screen.title).dmono(13, bold: true).foregroundStyle(BackplaneDesign.text).lineLimit(1)
                    TermState(state: screen.missing ? "gone" : screen.state).accessibilityIdentifier("term.state")
                }
                Spacer(minLength: 0)
                if scroll > 0 { Button { handle.surface?.scrollToBottom() } label: { Image(systemName: "arrow.down.to.line").frame(width: 36, height: 36).foregroundStyle(BackplaneDesign.accent) }.accessibilityLabel(Tr.t("Jump to the end")).accessibilityIdentifier("term.bottom") }
                Button { handle.surface?.pasteFromButton() } label: { Image(systemName: "doc.on.clipboard").frame(width: 36, height: 36).foregroundStyle(BackplaneDesign.dim) }.accessibilityLabel(Tr.t("Paste")).accessibilityIdentifier("term.paste")
                if screen.live { Button { terms.stop(route.key) } label: { Text(Tr.t("Stop")).dmono(12, bold: true).foregroundStyle(BackplaneDesign.bad).padding(.horizontal, 10).frame(minHeight: 32).overlay(Rectangle().strokeBorder(BackplaneDesign.line, lineWidth: 1)) }.accessibilityIdentifier("term.stop") }
            }.padding(.horizontal, 8).frame(minHeight: 44).background(BackplaneDesign.panel)
            TermInputRepresentable(key: route.key, screen: screen, rev: screen.rev, scroll: scroll, fontSize: font, terms: terms, handle: handle, onFont: { font = $0 }, onScroll: { scroll = $0 })
        }
        .background(Color(red: 0x0d / 255, green: 0x0f / 255, blue: 0x12 / 255))
        .onAppear { terms.forgetSize(route.key); terms.want(route.key); handle.surface?.announceGrid() }
        .onDisappear { terms.unwant(route.key) }
    }
}

// MARK: the Terminals screen

struct DataTermsView: View {
    let client: DataClient
    let project: String
    @State private var rows: [DTermRow] = []
    @State private var loaded = false
    @State private var creating = false
    @State private var error = ""

    private func load() async { if let r = await client.terms.list(project: project) { rows = r; loaded = true } }
    private func word(_ r: DTermRow) -> String { r.kind == "chat" ? "chat" : r.kind == "shell" ? "shell" : r.kind }
    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Button { Task { creating = true; defer { creating = false }
                    do { let k = try await client.terms.newShell(project: project, cols: 80, rows: 24); error = ""; client.terms.open(k, title: Tr.t("Shell")); await load() }
                    catch { self.error = error.localizedDescription } } } label: { Text(Tr.t("New shell")) }
                    .buttonStyle(DButton(primary: true)).disabled(creating).accessibilityIdentifier("terms.new")
                Spacer(minLength: 0)
            }.padding(10)
            if !error.isEmpty { Text(Tr.t(error)).dmono(12).foregroundStyle(BackplaneDesign.bad).frame(maxWidth: .infinity, alignment: .leading).padding(.horizontal, 10) }
            DLine()
            if rows.isEmpty {
                Text(Tr.t(loaded ? "No terminals." : "Loading…")).dmono(12).foregroundStyle(BackplaneDesign.dim).frame(maxWidth: .infinity, alignment: .leading).padding(14)
                Spacer(minLength: 0)
            } else {
                List {
                    ForEach(rows) { r in
                        Button { client.terms.open(r.key, title: r.title) } label: {
                            HStack(spacing: 8) {
                                Image(systemName: "terminal").font(.system(size: 12)).foregroundStyle(BackplaneDesign.dim)
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(r.title.isEmpty ? Tr.t("Terminal") : r.title).dmono(13, bold: true).foregroundStyle(BackplaneDesign.text).lineLimit(1)
                                    HStack(spacing: 8) { TermState(state: r.state); Text(Tr.t(word(r))).dmono(11).foregroundStyle(BackplaneDesign.faint) }
                                }
                                Spacer(minLength: 0)
                            }.padding(.horizontal, 12).frame(minHeight: 50).contentShape(Rectangle())
                        }.buttonStyle(.plain).dataRow().accessibilityIdentifier("terms.row")
                    }
                }.modifier(BackplaneListSurface())
            }
        }
        .background(BackplaneDesign.panel)
        .dataBar(Tr.t("Terminals"))
        .task(id: client.conn == .online) { await load() }
        .refreshable { await load() }
        .onChange(of: client.terms.fullscreen == nil) { _, closed in if closed { Task { await load() } } }
    }
}
