import SwiftUI
import UIKit

// Settings > MCP servers: the MCP servers every agent Backplane runs gets. The hub draws the page (native/host/src/mcpx/page.rs) as rows over
// data/1: `mcp.page {ui, fields}` -> {rows, ui}, `mcp.page.act {action, value, ui, fields}` -> {rows, ui} or a refusal. The phone keeps the
// hub's `ui` (McpPage.swift), sends what is typed, asks again as the person types (the hub parses a paste live) and every few seconds while
// the page is open (a check's "Checking" turns into its result). A vault box's value goes only to secret.set, never to the page.

@MainActor
@Observable
final class McpModel {
    private(set) var rows: [McpRow] = []
    private(set) var state = McpState()
    /// the hub's refusal of the last press, shown at the top until the next one
    private(set) var error = ""
    private(set) var loaded = false
    /// vault boxes: typed here, sent only to the vault
    private var secrets: [String: String] = [:]
    @ObservationIgnored weak var client: DataClient?
    @ObservationIgnored private var gen = 0
    @ObservationIgnored private var acting = 0
    @ObservationIgnored private var debounce: Task<Void, Never>?

    init() {
        #if DEBUG
        // a check without hands (the screenshots): the page state and the typed boxes to start with, as JSON
        let env = ProcessInfo.processInfo.environment
        func obj(_ k: String) -> [String: String] { McpState.strings(env[k].flatMap { try? JSONSerialization.jsonObject(with: Data($0.utf8)) }) }
        state.ui = obj("BACKPLANE_MCP_UI"); state.typed = obj("BACKPLANE_MCP_FIELDS")
        #endif
    }

    private var vaultNames: Set<String> { Set(rows.flatMap { $0.fields.filter(\.vault).map(\.name) }).union(["msec"]) }
    private func params() -> [String: Any] { ["ui": state.ui, "fields": state.sent(vault: vaultNames)] }

    func load() async {
        guard let client, client.conn == .online, acting == 0 else { return }
        gen += 1; let g = gen
        guard let r: McpReply = try? await client.call("mcp.page", params()), g == gen, acting == 0 else { return }
        rows = McpRow.parse(r.rows); loaded = true
    }

    func text(_ f: McpField) -> String { f.vault ? secrets[f.name] ?? "" : state.typed[f.name] ?? f.value }
    func type(_ f: McpField, _ t: String) {
        if f.vault { secrets[f.name] = t; return }
        state.typed[f.name] = t
        debounce?.cancel()
        debounce = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(250))
            if !Task.isCancelled { await self?.load() }
        }
    }
    func dismissError() { error = "" }

    /// A press: the phone's own (open-url), the vault's Save, or the hub's.
    func press(_ p: McpPress, row: McpRow) {
        if DPageLocal.run(p.action, p.value) { return }
        Task { await act(p, row: row) }
    }
    private func act(_ p: McpPress, row: McpRow) async {
        guard let client else { return }
        if p.action == "secret-save" {
            let box = row.fields.first { $0.vault } ?? row.fields.first { $0.secret }
            guard let box, let q = mcpVaultSave(packed: p.value, value: secrets[box.name] ?? "") else { error = Tr.k("secret.value.ph"); return }
            do {
                let _: DEmpty = try await client.call("secret.set", q)
                secrets[box.name] = nil; error = ""
                await load()
            } catch { self.error = error.localizedDescription }
            return
        }
        acting += 1; gen += 1
        defer { acting -= 1 }
        do {
            var q = params(); q["action"] = p.action; q["value"] = p.value
            let r: McpReply = try await client.call("mcp.page.act", q)
            state.acted(p.action, p.value, ui: McpState.strings(r.ui))
            if McpState.clearing.contains(p.action) { secrets = [:] }
            rows = McpRow.parse(r.rows); loaded = true; error = ""
        } catch { self.error = error.localizedDescription }
    }
}

struct McpReply: Decodable {
    var rows: [[String: Any]]
    var ui: [String: Any]
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: K.self)
        rows = (try? c.decode(DJSON.self, forKey: .rows))?.value as? [[String: Any]] ?? []
        ui = (try? c.decode(DJSON.self, forKey: .ui))?.value as? [String: Any] ?? [:]
    }
    enum K: String, CodingKey { case rows, ui }
}

// MARK: the page

struct DataMcpView: View {
    @Bindable var client: DataClient
    @State private var model = McpModel()

    private var title: String { model.rows.first { $0.kind == .head }.map { Tr.t($0.label) } ?? Tr.t("MCP servers") }
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                if !model.error.isEmpty {
                    McpNotice(label: model.error, note: "", tone: "bad", buttons: [McpPress(label: Tr.t("Dismiss"), action: "", value: "")]) { _ in model.dismissError() }
                        .padding(.bottom, 8).accessibilityIdentifier("mcp.error")
                }
                ForEach(McpBlock.of(model.rows)) { b in McpBlockView(model: model, block: b) }
                if !model.loaded && model.rows.isEmpty {
                    Text(Tr.t(client.conn == .online ? "Loading…" : "Not connected")).dmono(12).foregroundStyle(BackplaneDesign.dim).padding(.top, 8)
                }
            }
            .padding(.horizontal, 16).padding(.top, 14).padding(.bottom, 40)
        }
        .scrollDismissesKeyboard(.interactively)
        .background(BackplaneDesign.background)
        .dataBar(title)
        .task(id: client.conn) {
            model.client = client
            guard client.conn == .online else { return }
            await model.load()
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(4))
                if Task.isCancelled { break }
                await model.load()
            }
        }
    }
}

/// A row by its kind; a card with the rows drawn inside it.
struct McpBlockView: View {
    let model: McpModel
    let block: McpBlock
    var body: some View {
        let r = block.row
        switch r.kind {
        case .card: McpCard(model: model, row: r, inner: block.inner)
        default: McpRowView(model: model, row: r, inCard: false)
        }
    }
}

struct McpRowView: View {
    let model: McpModel
    let row: McpRow
    let inCard: Bool
    private var dim: Color { BackplaneDesign.dim }
    var body: some View {
        let r = row
        switch r.kind {
        case .head:
            VStack(alignment: .leading, spacing: 12) {
                if !r.note.isEmpty { Text(Tr.t(r.note)).dmono(12).foregroundStyle(dim).fixedSize(horizontal: false, vertical: true) }
                if !r.buttons.isEmpty {
                    HStack(spacing: 6) {
                        ForEach(Array(r.buttons.enumerated()), id: \.offset) { i, b in
                            McpChipButton(b: b, primary: i == r.primary, wide: true) { model.press(b, row: r) }
                        }
                    }
                }
            }
            .padding(.bottom, 4).frame(maxWidth: .infinity, alignment: .leading)
        case .group:
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Text(Tr.t(r.label).uppercased()).dmono(10.5, bold: true).foregroundStyle(BackplaneDesign.faint).fixedSize()
                if !r.note.isEmpty { Text(r.note).dmono(10.5).foregroundStyle(BackplaneDesign.faint).lineLimit(1).truncationMode(.middle) }
            }
            .padding(.top, inCard ? 12 : 22).padding(.bottom, 6)
            .accessibilityAddTraits(.isHeader)
        case .notice:
            McpNotice(label: r.label, note: r.note, tone: r.tone, buttons: r.buttons) { b in model.press(b, row: r) }.padding(.vertical, 4)
        case .code:
            VStack(alignment: .leading, spacing: 2) {
                ForEach(Array(r.text.split(separator: "\n", omittingEmptySubsequences: false).prefix(14).enumerated()), id: \.offset) { _, l in
                    Text(String(l)).dmono(10.5).foregroundStyle(dim).lineLimit(1).truncationMode(.tail)
                }
            }
            .padding(10).frame(maxWidth: .infinity, alignment: .leading)
            .background(BackplaneDesign.view).overlay(Rectangle().strokeBorder(BackplaneDesign.line, lineWidth: 1))
            .textSelection(.enabled).padding(.vertical, 4)
        case .empty:
            VStack(spacing: 10) {
                Image(systemName: "powerplug").font(.system(size: 22)).foregroundStyle(BackplaneDesign.faint)
                Text(Tr.t(r.label)).dmono(14, bold: true).foregroundStyle(BackplaneDesign.text).multilineTextAlignment(.center)
                if !r.note.isEmpty { Text(Tr.t(r.note)).dmono(12).foregroundStyle(dim).multilineTextAlignment(.center).fixedSize(horizontal: false, vertical: true) }
                if !r.buttons.isEmpty {
                    McpFlow(spacing: 6, align: .center) { ForEach(Array(r.buttons.enumerated()), id: \.offset) { i, b in McpChipButton(b: b, primary: i == 0) { model.press(b, row: r) } } }
                        .padding(.top, 4)
                }
            }
            .padding(.horizontal, 20).padding(.vertical, 26).frame(maxWidth: .infinity)
            .overlay(Rectangle().strokeBorder(BackplaneDesign.line, style: StrokeStyle(lineWidth: 1, dash: [4, 3])))
            .padding(.vertical, 6)
        case .panel:
            VStack(alignment: .leading, spacing: 10) {
                HStack(alignment: .center, spacing: 10) {
                    Text(Tr.t(r.label)).dmono(14, bold: true).foregroundStyle(BackplaneDesign.text).frame(maxWidth: .infinity, alignment: .leading)
                    if r.buttons.count == 1, let b = r.buttons.first { McpChipButton(b: b, small: true) { model.press(b, row: r) } }
                }
                if !r.note.isEmpty { Text(Tr.t(r.note)).dmono(11).foregroundStyle(dim).fixedSize(horizontal: false, vertical: true) }
                if r.buttons.count > 1 { McpSegments(buttons: r.buttons) { b in model.press(b, row: r) } }
            }
            .padding(.top, 14).padding(.bottom, 8)
            .overlay(alignment: .top) { Rectangle().fill(BackplaneDesign.accent).frame(height: 1) }
            .padding(.top, 14)
        case .paste:
            VStack(alignment: .leading, spacing: 8) {
                if !r.note.isEmpty { Text(Tr.t(r.note)).dmono(11).foregroundStyle(dim).fixedSize(horizontal: false, vertical: true) }
                ForEach(r.fields, id: \.name) { f in McpBox(model: model, field: f) }
            }
            .padding(.vertical, 8)
        case .actions:
            VStack(alignment: .leading, spacing: 8) {
                if !r.note.isEmpty {
                    Text(Tr.t(r.note)).dmono(11).foregroundStyle(r.buttons.contains { $0.action == "mcp-remove" && $0.on } ? BackplaneDesign.bad : dim)
                        .fixedSize(horizontal: false, vertical: true)
                }
                McpFlow(spacing: 6, align: .trailing) {
                    ForEach(Array(r.buttons.enumerated()), id: \.offset) { _, b in
                        McpChipButton(b: b, primary: b.on && b.action != "mcp-remove", danger: b.action == "mcp-remove" && b.on, small: inCard) { model.press(b, row: r) }
                    }
                }
                .frame(maxWidth: .infinity, alignment: .trailing)
            }
            .padding(.vertical, inCard ? 10 : 12)
        case .tool:
            let on = r.toggle?.on ?? true
            HStack(alignment: .center, spacing: 10) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(r.label).dmono(12, bold: true).foregroundStyle(on ? BackplaneDesign.text : BackplaneDesign.faint).lineLimit(1).truncationMode(.middle)
                    if !r.note.isEmpty { Text(r.note).dmono(10.5).foregroundStyle(on ? dim : BackplaneDesign.faint).lineLimit(2) }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                if let t = r.toggle { McpSwitch(toggle: t) { model.press(t.press, row: r) } }
            }
            .padding(.vertical, 7).padding(.leading, inCard ? 48 : 0)
            .overlay(alignment: .bottom) { Rectangle().fill(BackplaneDesign.line).frame(height: 1).padding(.leading, inCard ? 48 : 0) }
        case .card:
            McpCard(model: model, row: r, inner: [])
        case .plain:
            McpPlainRow(model: model, row: r).padding(.vertical, 8)
        }
    }
}

/// A row with no kind: the same as every other Settings page draws it (DPageRowView).
struct McpPlainRow: View {
    let model: McpModel
    let row: McpRow
    var body: some View {
        let r = row
        let v = DVoiceRow(id: 0, label: r.label, note: r.note, tone: r.tone, buttons: r.buttons.map { ($0.label, $0.action, $0.value, $0.on) },
                          fields: r.fields.map { DVoiceField(name: $0.name, hint: $0.hint, value: $0.value, secret: $0.secret || $0.vault) })
        DPageRowView(row: v, idPrefix: "mcp", note: r.note, noteTone: r.tone, told: nil, compact: true,
                     text: { f in
                         let mf = r.fields.first { $0.name == f.name }!
                         return Binding(get: { model.text(mf) }, set: { model.type(mf, $0) })
                     },
                     press: { b in model.press(McpPress(label: b.label, action: b.action, value: b.value, on: b.on), row: r) })
    }
}

// MARK: a server's card

struct McpCard: View {
    let model: McpModel
    let row: McpRow
    let inner: [McpRow]
    var body: some View {
        let r = row
        let edge = r.open ? BackplaneDesign.accent : BackplaneDesign.line
        VStack(alignment: .leading, spacing: 0) {
            head
            if !r.fields.isEmpty || !r.cardButtons.isEmpty {
                VStack(alignment: .leading, spacing: 8) {
                    ForEach(r.fields, id: \.name) { f in McpBox(model: model, field: f) }
                    if !r.cardButtons.isEmpty {
                        HStack(spacing: 6) {
                            Spacer(minLength: 0)
                            ForEach(Array(r.cardButtons.enumerated()), id: \.offset) { _, b in McpChipButton(b: b, primary: b.on, small: true) { model.press(b, row: r) } }
                        }
                    }
                }
                .padding(.horizontal, 12).padding(.bottom, 10).padding(.leading, r.fields.isEmpty ? 0 : tile + 12)
            }
            if !inner.isEmpty {
                Rectangle().fill(BackplaneDesign.line).frame(height: 1)
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(inner) { x in McpRowView(model: model, row: x, inCard: true) }
                }
                .padding(.horizontal, 12).padding(.top, 2)
            }
        }
        .background(BackplaneDesign.panel)
        .overlay(Rectangle().strokeBorder(edge, lineWidth: 1))
        .padding(.top, r.open ? 10 : 6).padding(.bottom, r.open ? 4 : 0)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("mcp.card." + (r.cardId.isEmpty ? r.shownTitle : r.cardId))
    }
    private var tile: CGFloat { row.compact ? 28 : 36 }
    private var head: some View {
        let r = row
        return HStack(alignment: .center, spacing: 12) {
            McpTile(text: r.iconText, bg: r.iconBg, size: tile)
            VStack(alignment: .leading, spacing: 3) {
                HStack(alignment: .center, spacing: 6) {
                    Text(Tr.t(r.shownTitle)).dmono(r.compact ? 13 : 14, bold: true).foregroundStyle(BackplaneDesign.text).lineLimit(1).truncationMode(.tail)
                    if !r.badge.isEmpty {
                        Text(Tr.t(r.badge)).dmono(9.5).foregroundStyle(BackplaneDesign.dim).lineLimit(1).fixedSize()
                            .padding(.horizontal, 4).padding(.vertical, 1).overlay(Rectangle().strokeBorder(BackplaneDesign.line, lineWidth: 1))
                    }
                    if !r.pillText.isEmpty { McpPill(text: r.pillText, tone: r.pillTone) }
                }
                if !r.sub.isEmpty { Text(r.sub).dmono(11).foregroundStyle(BackplaneDesign.dim).lineLimit(1).truncationMode(.tail) }
                if !r.meta.isEmpty { Text(Tr.t(r.meta)).dmono(10.5).foregroundStyle(BackplaneDesign.faint).lineLimit(2).fixedSize(horizontal: false, vertical: true) }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            if let t = r.toggle { McpSwitch(toggle: t) { model.press(t.press, row: r) } }
        }
        .padding(.horizontal, 12).padding(.vertical, r.compact ? 8 : 11)
        .contentShape(Rectangle())
        .onTapGesture { if let p = r.press { model.press(p, row: r) } }
        .accessibilityAddTraits(r.press != nil ? .isButton : [])
    }
}

/// A server's square: its colour, white bold letters.
struct McpTile: View {
    let text: String
    let bg: String
    let size: CGFloat
    @Environment(\.colorScheme) private var scheme
    var body: some View {
        let c = McpRow.hex(bg).map { Color(red: Double(($0 >> 16) & 255) / 255, green: Double(($0 >> 8) & 255) / 255, blue: Double($0 & 255) / 255) } ?? BackplaneDesign.raised
        let t = text.isEmpty ? "·" : text
        Text(verbatim: t).font(.mono(size: size * (t.count > 1 ? 0.36 : 0.46), weight: .bold)).foregroundStyle(.white)
            .frame(width: size, height: size).background(c)
            .overlay(Rectangle().strokeBorder(Color.white.opacity(scheme == .dark ? 0.10 : 0), lineWidth: 1))
            .accessibilityHidden(true)
    }
}

func mcpTone(_ t: String) -> Color {
    switch t { case "ok": BackplaneDesign.ok; case "warn": BackplaneDesign.warn; case "bad": BackplaneDesign.bad; case "busy": BackplaneDesign.accent; default: BackplaneDesign.dim }
}

struct McpPill: View {
    let text: String
    let tone: String
    @Environment(\.colorScheme) private var scheme
    var body: some View {
        let c = mcpTone(tone)
        Text(Tr.t(text)).dmono(10, bold: true).foregroundStyle(c).lineLimit(1).fixedSize()
            .padding(.horizontal, 6).padding(.vertical, 2).background(c.opacity(scheme == .dark ? 0.16 : 0.12))
    }
}

/// A square switch (the desktop's), or a tick box for a pick.
struct McpSwitch: View {
    let toggle: McpToggle
    let action: () -> Void
    var body: some View {
        let on = toggle.on
        Button(action: action) {
            Group {
                if toggle.check {
                    ZStack {
                        Rectangle().fill(on ? BackplaneDesign.accent : BackplaneDesign.view)
                        Rectangle().strokeBorder(on ? BackplaneDesign.accent : BackplaneDesign.dim, lineWidth: 1)
                        if on { Image(systemName: "checkmark").font(.system(size: 11, weight: .heavy)).foregroundStyle(BackplaneDesign.onAccent) }
                    }
                    .frame(width: 20, height: 20)
                } else {
                    HStack(spacing: 0) {
                        if on { Spacer(minLength: 0) }
                        Rectangle().fill(on ? Color.white : BackplaneDesign.dim).frame(width: 14, height: 14)
                        if !on { Spacer(minLength: 0) }
                    }
                    .padding(3).frame(width: 40, height: 22)
                    .background(on ? BackplaneDesign.accent : BackplaneDesign.raised)
                    .overlay(Rectangle().strokeBorder(on ? BackplaneDesign.accent : BackplaneDesign.line, lineWidth: 1))
                }
            }
            .frame(minWidth: 44, minHeight: 44).contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityValue(Tr.t(on ? "on" : "off"))
        .accessibilityAddTraits(.isToggle)
        .accessibilityIdentifier("mcp.toggle." + toggle.press.value)
    }
}

/// A line with its tone's stripe on the left.
struct McpNotice: View {
    let label: String
    let note: String
    let tone: String
    let buttons: [McpPress]
    let press: (McpPress) -> Void
    @Environment(\.colorScheme) private var scheme
    var body: some View {
        let c = mcpTone(tone)
        HStack(alignment: .center, spacing: 10) {
            VStack(alignment: .leading, spacing: 2) {
                Text(Tr.t(label)).dmono(12).foregroundStyle(tone == "ok" || tone == "busy" ? BackplaneDesign.text : c).fixedSize(horizontal: false, vertical: true)
                if !note.isEmpty { Text(Tr.t(note)).dmono(10.5).foregroundStyle(BackplaneDesign.dim).fixedSize(horizontal: false, vertical: true) }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            ForEach(Array(buttons.enumerated()), id: \.offset) { _, b in McpChipButton(b: b, small: true) { press(b) } }
        }
        .padding(.leading, 12).padding(.trailing, 8).padding(.vertical, 8)
        .background(c.opacity(scheme == .dark ? 0.09 : 0.07))
        .overlay(alignment: .leading) { Rectangle().fill(c).frame(width: 2) }
    }
}

/// A form's modes, side by side as one control.
struct McpSegments: View {
    let buttons: [McpPress]
    let press: (McpPress) -> Void
    var body: some View {
        HStack(spacing: -1) {
            ForEach(Array(buttons.enumerated()), id: \.offset) { _, b in
                Button { press(b) } label: {
                    Text(Tr.t(b.label)).dmono(12, bold: true).lineLimit(1).minimumScaleFactor(0.8)
                        .foregroundStyle(b.on ? BackplaneDesign.onAccent : BackplaneDesign.text)
                        .frame(maxWidth: .infinity, minHeight: 36)
                        .background(b.on ? BackplaneDesign.accent : BackplaneDesign.raised)
                        .overlay(Rectangle().strokeBorder(b.on ? BackplaneDesign.accent : BackplaneDesign.line, lineWidth: 1))
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain).zIndex(b.on ? 1 : 0)
                .accessibilityAddTraits(b.on ? .isSelected : [])
                .accessibilityIdentifier("mcp.mode." + b.value)
            }
        }
    }
}

/// The page's buttons: the main one filled, a lit one outlined in the accent, the rest a line on the raised colour.
struct McpChipButton: View {
    let b: McpPress
    var primary = false
    var danger = false
    var small = false
    var wide = false
    let action: () -> Void
    var body: some View {
        let lit = b.on && !primary && !danger
        Button(action: action) {
            Text(Tr.t(b.label)).dmono(small ? 11.5 : 12.5, bold: true).lineLimit(1)
                .foregroundStyle(primary || danger ? BackplaneDesign.onAccent : lit ? BackplaneDesign.accentText : BackplaneDesign.text)
                .padding(.horizontal, small ? 10 : 14).frame(maxWidth: wide ? .infinity : nil, minHeight: small ? 30 : 36)
                .background(danger ? BackplaneDesign.bad : primary ? BackplaneDesign.accent : BackplaneDesign.raised)
                .overlay(Rectangle().strokeBorder(danger ? BackplaneDesign.bad : primary || lit ? BackplaneDesign.accent : BackplaneDesign.line, lineWidth: 1))
                .contentShape(Rectangle())
        }
        .buttonStyle(McpPressStyle())
        .accessibilityAddTraits(b.on ? .isSelected : [])
        .accessibilityIdentifier("mcp.btn." + b.action + (b.value.isEmpty ? "" : "." + b.value))
    }
}
private struct McpPressStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View { configuration.label.opacity(configuration.isPressed ? 0.7 : 1) }
}

/// Chips that wrap, each line set to the leading edge, the middle or the trailing edge.
struct McpFlow: Layout {
    var spacing: CGFloat = 6
    var align: HorizontalAlignment = .leading
    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let w = proposal.width ?? 320
        let ls = lines(w, subviews)
        let h = ls.reduce(0) { $0 + $1.h } + spacing * CGFloat(max(0, ls.count - 1))
        return CGSize(width: proposal.width ?? (ls.map(\.w).max() ?? 0), height: h)
    }
    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        var y = bounds.minY
        for l in lines(bounds.width, subviews) {
            var x = bounds.minX + (align == .trailing ? bounds.width - l.w : align == .center ? (bounds.width - l.w) / 2 : 0)
            for i in l.items { let z = subviews[i].sizeThatFits(.unspecified); subviews[i].place(at: CGPoint(x: x, y: y + (l.h - z.height) / 2), proposal: ProposedViewSize(z)); x += z.width + spacing }
            y += l.h + spacing
        }
    }
    private func lines(_ width: CGFloat, _ subviews: Subviews) -> [(items: [Int], w: CGFloat, h: CGFloat)] {
        var out: [(items: [Int], w: CGFloat, h: CGFloat)] = []; var cur: [Int] = []; var w: CGFloat = 0, h: CGFloat = 0
        for (i, s) in subviews.enumerated() {
            let z = s.sizeThatFits(.unspecified)
            if !cur.isEmpty, w + spacing + z.width > width { out.append((cur, w, h)); cur = []; w = 0; h = 0 }
            w += (cur.isEmpty ? 0 : spacing) + z.width; h = max(h, z.height); cur.append(i)
        }
        if !cur.isEmpty { out.append((cur, w, h)) }
        return out
    }
}

// MARK: boxes: no smart quotes or dashes (a pasted JSON or a command line must stay as typed), mono

/// A field of the page: one line, several (`lines`), or masked (a vault box).
struct McpBox: View {
    let model: McpModel
    let field: McpField
    var body: some View {
        let f = field
        let t = Binding(get: { model.text(f) }, set: { model.type(f, $0) })
        Group {
            if f.secret || f.vault {
                SecureField(Tr.t(f.hint), text: t).dmono(13).textInputAutocapitalization(.never).autocorrectionDisabled()
                    .padding(.horizontal, 10).frame(minHeight: 40)
            } else {
                McpTextView(text: t, hint: f.hint, lines: f.lines).frame(height: f.lines > 1 ? CGFloat(f.lines) * 17 + 20 : 40)
            }
        }
        .dbox()
        .accessibilityIdentifier("mcp.field." + f.name)
    }
}

struct McpTextView: UIViewRepresentable {
    @Binding var text: String
    let hint: String
    let lines: Int
    func makeCoordinator() -> Coordinator { Coordinator(self) }
    func makeUIView(context: Context) -> UITextView {
        let v = UITextView()
        let multi = lines > 1
        v.font = Mono.uiFont(size: multi ? 12 : 13)
        v.adjustsFontForContentSizeCategory = true
        v.backgroundColor = .clear
        v.textColor = UIColor(BackplaneDesign.text)
        v.textContainerInset = multi ? UIEdgeInsets(top: 9, left: 7, bottom: 9, right: 7) : UIEdgeInsets(top: 11, left: 7, bottom: 9, right: 7)
        v.textContainer.maximumNumberOfLines = multi ? 0 : 1
        v.textContainer.lineBreakMode = multi ? .byCharWrapping : .byTruncatingTail
        v.isScrollEnabled = multi
        v.autocorrectionType = .no; v.autocapitalizationType = .none; v.spellCheckingType = .no
        v.smartQuotesType = .no; v.smartDashesType = .no; v.smartInsertDeleteType = .no
        v.keyboardType = .asciiCapable
        v.returnKeyType = multi ? .default : .done
        v.delegate = context.coordinator
        v.text = text
        let label = UILabel()
        label.text = hint; label.font = v.font; label.textColor = UIColor(BackplaneDesign.faint)
        label.numberOfLines = multi ? 0 : 1; label.lineBreakMode = .byTruncatingTail; label.tag = 77; label.isAccessibilityElement = false
        label.translatesAutoresizingMaskIntoConstraints = false
        v.addSubview(label)
        NSLayoutConstraint.activate([
            label.leadingAnchor.constraint(equalTo: v.leadingAnchor, constant: 7 + v.textContainer.lineFragmentPadding),
            label.topAnchor.constraint(equalTo: v.topAnchor, constant: v.textContainerInset.top),
            label.trailingAnchor.constraint(lessThanOrEqualTo: v.frameLayoutGuide.trailingAnchor, constant: -8),
        ])
        label.isHidden = !text.isEmpty
        v.accessibilityLabel = hint
        return v
    }
    func updateUIView(_ v: UITextView, context: Context) {
        context.coordinator.parent = self
        if v.text != text, v.markedTextRange == nil { v.text = text }
        v.textColor = UIColor(BackplaneDesign.text)
        if let l = v.viewWithTag(77) as? UILabel { if l.text != hint { l.text = hint }; l.isHidden = !text.isEmpty }
    }
    final class Coordinator: NSObject, UITextViewDelegate {
        var parent: McpTextView
        init(_ p: McpTextView) { parent = p }
        func textViewDidChange(_ v: UITextView) { parent.text = v.text; v.viewWithTag(77)?.isHidden = !v.text.isEmpty }
        func textView(_ v: UITextView, shouldChangeTextIn range: NSRange, replacementText t: String) -> Bool {
            if parent.lines <= 1, t.contains("\n") {
                if t == "\n" { v.resignFirstResponder(); return false }
                // a pasted line break in a one-line box: one line
                let one = t.replacingOccurrences(of: "\n", with: " ")
                if let r = Range(range, in: v.text) { v.text = v.text.replacingCharacters(in: r, with: one); textViewDidChange(v) }
                return false
            }
            return true
        }
    }
}
