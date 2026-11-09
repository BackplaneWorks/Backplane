import SwiftUI
import UIKit

// Dictation on the phone (docs/PHONE-DATA.md, "Dictation"). The hub decides everything: what the strip says, where the words go, what a chip does.
// This keeps one number per thread (the mark: where dictated words end, in characters), applies the hub's edits at it (VoiceEdit.swift) and draws the hub's `view`.

struct DVoiceButton: Identifiable, Hashable {
    var id = UUID()
    var label = "", action = "", value = "", tip = ""
    var primary = false
}
struct DVoiceField: Hashable { var name = "", hint = "", value = ""; var secret = false }
struct DVoiceAsk: Hashable {
    var text = "", fixing = false
    var field: DVoiceField?
    var buttons: [DVoiceButton] = []
}
struct DVoiceView: Hashable {
    var on = false, bad = false
    var status = "", note = "", stop = ""
    var fixes: [DVoiceButton] = []
    var ask: DVoiceAsk?
    var showing: Bool { on || !note.isEmpty || !fixes.isEmpty || ask != nil }
    init() {}
    init(_ o: [String: Any]) {
        on = o["on"] as? Bool ?? false; bad = o["bad"] as? Bool ?? false
        status = o["status"] as? String ?? ""; note = o["note"] as? String ?? ""; stop = o["stop"] as? String ?? ""
        fixes = (o["fixes"] as? [[String: Any]] ?? []).map(DVoiceButton.init)
        if let a = o["ask"] as? [String: Any] {
            var k = DVoiceAsk(text: a["text"] as? String ?? "", fixing: a["fixing"] as? Bool ?? false)
            k.field = (a["field"] as? [String: Any]).map(DVoiceField.init)
            k.buttons = (a["buttons"] as? [[String: Any]] ?? []).map(DVoiceButton.init)
            ask = k
        }
    }
}
extension DVoiceButton {
    init(_ o: [String: Any]) {
        self.init(label: o["label"] as? String ?? "", action: o["action"] as? String ?? "", value: o["value"] as? String ?? "", tip: o["tip"] as? String ?? "", primary: o["primary"] as? Bool ?? false)
    }
}
extension DVoiceField {
    init(_ o: [String: Any]) { self.init(name: o["name"] as? String ?? "", hint: o["hint"] as? String ?? "", value: o["value"] as? String ?? "", secret: o["secret"] as? Bool ?? false) }
}

struct DVoiceRow: Identifiable {
    var id: Int
    var label = "", note = "", tone = ""
    var buttons: [(label: String, action: String, value: String, on: Bool)] = []
    var fields: [DVoiceField] = []
    /// a code to scan (devices.page): one string of "0"/"1" a module row, and the line under it
    var qr: [String] = [], qrnote = ""
    /// the hub's `rows` array (voice.page, machines.page, devices.page)
    static func parse(_ v: Any?) -> [DVoiceRow] {
        (v as? [[String: Any]] ?? []).enumerated().map { i, r in
            DVoiceRow(id: i, label: r["label"] as? String ?? "", note: r["note"] as? String ?? "", tone: r["tone"] as? String ?? "",
                      buttons: (r["buttons"] as? [[String: Any]] ?? []).map { ($0["label"] as? String ?? "", $0["action"] as? String ?? "", $0["value"] as? String ?? "", $0["on"] as? Bool ?? false) },
                      fields: (r["fields"] as? [[String: Any]] ?? []).map(DVoiceField.init),
                      qr: r["qr"] as? [String] ?? [], qrnote: r["qrnote"] as? String ?? "")
        }
    }
}

@MainActor
@Observable
final class VoiceModel {
    private(set) var views: [String: DVoiceView] = [:]
    private(set) var level = 0
    /// The thread whose audio this phone is sending.
    private(set) var active: String?
    private(set) var rows: [DVoiceRow] = []
    /// A caret the hub asked for (characters), with a counter so the composer sets it once.
    private(set) var caret: (thread: String, at: Int, rev: Int) = ("", 0, 0)
    /// What this phone says itself (the microphone was refused), until Close.
    var local: [String: String] = [:]
    @ObservationIgnored private var marks: [String: Int] = [:]
    @ObservationIgnored let recorder = VoiceRecorder()
    @ObservationIgnored weak var client: DataClient?

    init() { recorder.onGone = { [weak self] in self?.recorderGone() } }

    func shown(_ thread: String) -> DVoiceView? {
        var v = views[thread] ?? DVoiceView()
        if v.note.isEmpty, let n = local[thread] { v.note = n; v.bad = true }
        if active == thread { v.on = true }
        return v.showing ? v : nil
    }
    func listening(_ thread: String) -> Bool { active == thread || views[thread]?.on == true }

    // MARK: the press
    func press(_ thread: String, draft: String, caret: Int) {
        guard let client else { return }
        local[thread] = nil
        if listening(thread) {
            recorder.stop(); let was = active == thread; if was { active = nil }
            Task { _ = try? await client.call("voice.stop", ["thread": thread], as: DEmpty.self) }
            return
        }
        guard active == nil, let link = client.link else { return }
        Task {
            switch await recorder.begin(link: link) {
            case .refused: local[thread] = "Microphone access is off for Backplane. Turn it on in Settings, then press the mic again."; return
            case .failed: local[thread] = "The microphone could not start."; return
            case .ok: break
            }
            active = thread
            marks[thread] = caret
            do {
                let r: DVoiceStart = try await client.call("voice.start", ["thread": thread, "draft": draft, "caret": caret])
                if r.path.isEmpty { recorder.gone(); if active == thread { active = nil } } else { recorder.address(path: r.path) }
            } catch {
                recorder.gone(); if active == thread { active = nil }
                local[thread] = error.localizedDescription
            }
        }
    }
    private func recorderGone() { active = nil }

    func act(_ thread: String, _ b: DVoiceButton, value: String? = nil, draft: String) {
        if b.action == "voice-err" { local[thread] = nil }
        guard let client else { return }
        Task { _ = try? await client.call("voice.act", ["thread": thread, "action": b.action, "value": value ?? b.value, "draft": draft], as: DEmpty.self) }
    }
    /// The message went: what was left to clarify in it goes too.
    func sent(_ thread: String, draft: String) {
        guard let v = views[thread], v.ask != nil || !v.fixes.isEmpty else { return }
        act(thread, DVoiceButton(action: "voice-clear"), draft: draft)
    }
    /// The person typed (or the draft was set by something else): the mark moves with what changed before it.
    func typed(_ thread: String, old: String, new: String) {
        if let m = marks[thread] { marks[thread] = VoiceEdit.shift(old, new, mark: m) }
    }
    func resync() async {
        guard let client else { return }
        for t in Set(views.filter { $0.value.on }.keys).union(active.map { [$0] } ?? []) {
            if let r: DVoiceViewReply = try? await client.call("voice.view", ["thread": t]) { frame(["ev": "view", "thread": t, "view": r.view]) }
        }
    }

    // MARK: the hub's frames
    func frame(_ o: [String: Any]) {
        guard let client else { return }
        let thread = o["thread"] as? String ?? ""
        func n(_ k: String) -> Int { (o[k] as? Int) ?? (o[k] as? Double).map(Int.init) ?? 0 }
        switch o["ev"] as? String ?? "" {
        case "view":
            guard let v = o["view"] as? [String: Any] else { return }
            let view = DVoiceView(v)
            views[thread] = view
            if !view.on { marks[thread] = nil; level = 0; if active == thread { recorder.gone(); active = nil } }
        case "level": level = max(0, min(100, n("v")))
        case "edit":
            let text = client.drafts[thread] ?? ""
            let mark = marks[thread] ?? VoiceEdit.count(text)
            let r = VoiceEdit.apply(text, mark: mark, del: n("del"), ins: o["ins"] as? String ?? "", adv: n("adv"), to: n("to"))
            marks[thread] = r.mark
            client.putDraft(thread, r.text)
            caret = (thread, r.caret, caret.rev + 1)
        case "draft":
            let old = client.drafts[thread] ?? "", new = o["text"] as? String ?? ""
            if let m = marks[thread] { marks[thread] = VoiceEdit.shift(old, new, mark: m) }
            client.putDraft(thread, new)
            caret = (thread, VoiceEdit.count(new), caret.rev + 1)
        case "page": setRows(o["rows"])
        default: break
        }
    }

    // MARK: Settings, Voice
    func setRows(_ v: Any?) { rows = DVoiceRow.parse(v) }
    func loadPage() async {
        guard let client, let r: DVoicePage = try? await client.call("voice.page") else { return }
        setRows(r.rows)
    }
    /// A press on a row's button; `fields` are the texts typed in the page. Returns the hub's error for the row, if any.
    func pageAct(action: String, value: String, fields: [String: String]) async -> String {
        guard let client else { return "" }
        do {
            if action == "secret-save" { _ = try await client.call("voice.key", ["key": fields["kval"] ?? ""], as: DEmpty.self) }
            else { _ = try await client.call("voice.page.act", ["action": action, "value": action == "voice-term" ? (fields["vterm"] ?? "") : value], as: DEmpty.self) }
            await loadPage()
            return ""
        } catch { await loadPage(); return error.localizedDescription }
    }
}

struct DVoiceStart: Decodable { var path: String = ""
    init(from d: Decoder) throws { path = (try? d.container(keyedBy: K.self).decode(String.self, forKey: .path)) ?? "" }
    enum K: String, CodingKey { case path } }
struct DVoiceViewReply: Decodable {
    var view: [String: Any]
    init(from d: Decoder) throws { let c = try d.container(keyedBy: K.self); view = (try c.decode(DJSON.self, forKey: .view)).value as? [String: Any] ?? [:] }
    enum K: String, CodingKey { case view }
}
struct DVoicePage: Decodable {
    var rows: [[String: Any]]
    init(from d: Decoder) throws { let c = try d.container(keyedBy: K.self); rows = (try c.decode(DJSON.self, forKey: .rows)).value as? [[String: Any]] ?? [] }
    enum K: String, CodingKey { case rows }
}
/// Any JSON value, for the hub's views (their shape is the hub's).
struct DJSON: Decodable {
    var value: Any
    init(from d: Decoder) throws {
        let c = try d.singleValueContainer()
        if c.decodeNil() { value = NSNull() }
        else if let b = try? c.decode(Bool.self) { value = b }
        else if let i = try? c.decode(Int.self) { value = i }
        else if let x = try? c.decode(Double.self) { value = x }
        else if let s = try? c.decode(String.self) { value = s }
        else if let a = try? c.decode([DJSON].self) { value = a.map(\.value) }
        else if let o = try? c.decode([String: DJSON].self) { value = o.mapValues(\.value) }
        else { value = NSNull() }
    }
}

// MARK: drawing

/// What shows above the composer: the strip while on (status and level, never the words), a note, the fixes as chips, the word to clarify.
struct DVoiceStrip: View {
    @Bindable var client: DataClient
    let thread: String
    let draft: String
    @State private var fixText = ""
    @State private var fixSeeded = false

    var body: some View {
        if let v = client.voice.shown(thread) {
            VStack(alignment: .leading, spacing: 6) {
                if !v.note.isEmpty {
                    HStack(spacing: 8) {
                        Text(Tr.t(v.note)).dmono(12).foregroundStyle(v.bad ? BackplaneDesign.bad : BackplaneDesign.dim).frame(maxWidth: .infinity, alignment: .leading).accessibilityIdentifier("voice.note")
                        chip(DVoiceButton(label: Tr.t("Close"), action: "voice-err"))
                    }
                }
                if v.on {
                    HStack(spacing: 8) {
                        Rectangle().fill(BackplaneDesign.bad).frame(width: 8, height: 8)
                        Text(Tr.t(v.status)).dmono(12).foregroundStyle(BackplaneDesign.dim).lineLimit(1).accessibilityIdentifier("voice.status")
                        ZStack(alignment: .leading) {
                            Rectangle().fill(BackplaneDesign.line).frame(width: 72, height: 4)
                            Rectangle().fill(BackplaneDesign.accent).frame(width: 0.72 * CGFloat(client.voice.level), height: 4)
                        }.accessibilityIdentifier("voice.level").accessibilityValue("\(client.voice.level)")
                        Spacer(minLength: 0)
                        Button(Tr.t(v.stop.isEmpty ? "Stop" : v.stop)) { press() }.buttonStyle(DButton()).accessibilityIdentifier("voice.stop")
                    }
                }
                if !v.fixes.isEmpty {
                    FlowRow(spacing: 6) { ForEach(v.fixes) { f in chip(f, primary: f.primary) } }
                }
                if let a = v.ask {
                    VStack(alignment: .leading, spacing: 6) {
                        Text(Tr.t(a.text)).dmono(12).foregroundStyle(BackplaneDesign.dim).accessibilityIdentifier("voice.ask")
                        if a.fixing {
                            TextField(Tr.t(a.field?.hint.isEmpty == false ? a.field!.hint : "The right spelling"), text: $fixText)
                                .dmono(13).textInputAutocapitalization(.never).autocorrectionDisabled()
                                .padding(.horizontal, 10).frame(minHeight: 40).dbox(BackplaneDesign.accent)
                                .onSubmit { client.voice.act(thread, DVoiceButton(action: "voice-fixed"), value: fixText, draft: draft) }
                                .accessibilityIdentifier("voice.fix")
                        }
                        FlowRow(spacing: 6) {
                            ForEach(a.buttons) { b in
                                chip(b, primary: b.primary, value: b.action == "voice-fixed" ? fixText : nil)
                            }
                        }
                    }
                    .onAppear { seed(a) }
                    .onChange(of: a.fixing) { _, _ in fixSeeded = false; seed(a) }
                }
            }
            .padding(.horizontal, 12).padding(.top, 8)
            .background(BackplaneDesign.background)
        }
    }
    private func seed(_ a: DVoiceAsk) {
        guard a.fixing, !fixSeeded else { return }
        fixSeeded = true; fixText = a.field?.value ?? ""
    }
    private func press() {
        let at = ComposerHandle.view.map { VoiceEdit.chars(draft, utf16: $0.selectedRange.location) } ?? VoiceEdit.count(draft)
        client.voice.press(thread, draft: draft, caret: at)
    }
    private func chip(_ b: DVoiceButton, primary: Bool = false, value: String? = nil) -> some View {
        Button(Tr.t(b.label)) { client.voice.act(thread, b, value: value, draft: draft) }
            .buttonStyle(DButton(primary: primary))
            .accessibilityIdentifier("voice.chip")
    }
}

/// Chips that wrap onto the next line.
struct FlowRow: Layout {
    var spacing: CGFloat = 6
    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize { place(proposal.width ?? 320, subviews, nil) }
    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) { _ = place(bounds.width, subviews, bounds.origin) }
    private func place(_ width: CGFloat, _ subviews: Subviews, _ origin: CGPoint?) -> CGSize {
        var x: CGFloat = 0, y: CGFloat = 0, row: CGFloat = 0, w: CGFloat = 0
        for s in subviews {
            let z = s.sizeThatFits(.unspecified)
            if x > 0, x + z.width > width { x = 0; y += row + spacing; row = 0 }
            if let o = origin { s.place(at: CGPoint(x: o.x + x, y: o.y + y), proposal: ProposedViewSize(z)) }
            x += z.width + spacing; row = max(row, z.height); w = max(w, x - spacing)
        }
        return CGSize(width: w, height: y + row)
    }
}

/// Settings, Voice: the hub's rows (voice.page); a press goes to voice.page.act, the OpenAI key's Save to voice.key.
struct DataVoiceSection: View {
    @Bindable var client: DataClient
    var body: some View {
        DPageRows(title: Tr.t("Voice"), idPrefix: "voice", rows: client.voice.rows, empty: Tr.t("Not connected")) { _, b, fields in
            await client.voice.pageAct(action: b.action, value: b.value, fields: fields)
        }
        .task(id: client.conn) { if client.conn == .online { await client.voice.loadPage() } }
    }
}

/// The hub's rows {label, note, buttons, fields, tone} drawn the same for every page that has them (Voice, Machines); a press goes to `act`, which returns the hub's error for the row, if any.
struct DPageRows: View {
    let title: String
    let idPrefix: String
    let rows: [DVoiceRow]
    var empty = ""
    let act: (DVoiceRow, (label: String, action: String, value: String, on: Bool), [String: String]) async -> String
    @State private var typed: [String: String] = [:]
    @State private var errors: [Int: String] = [:]

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            DHeading(text: title)
            ForEach(rows) { r in
                VStack(alignment: .leading, spacing: 6) {
                    Text(Tr.t(r.label)).dmono(13)
                    let note = errors[r.id] ?? r.note
                    if !note.isEmpty { Text(Tr.t(note)).dmono(11).foregroundStyle(errors[r.id] != nil || r.tone == "bad" ? BackplaneDesign.bad : r.tone == "warn" ? BackplaneDesign.warn : BackplaneDesign.dim) }
                    if !r.qr.isEmpty { DQRCode(rows: r.qr).accessibilityIdentifier(idPrefix + ".qr") }
                    if !r.qrnote.isEmpty { Text(Tr.t(r.qrnote)).dmono(12).foregroundStyle(BackplaneDesign.dim).textSelection(.enabled) }
                    ForEach(r.fields, id: \.name) { f in
                        Group {
                            if f.secret { SecureField(Tr.t(f.hint), text: binding(f)) } else { TextField(Tr.t(f.hint), text: binding(f)) }
                        }
                        .dmono(13).textInputAutocapitalization(.never).autocorrectionDisabled()
                        .padding(.horizontal, 10).frame(minHeight: 40).dbox()
                        .accessibilityIdentifier(idPrefix + ".field." + f.name)
                    }
                    if !r.buttons.isEmpty {
                        FlowRow(spacing: 6) {
                            ForEach(Array(r.buttons.enumerated()), id: \.offset) { _, b in
                                Button(Tr.t(b.label)) { press(r, b) }.buttonStyle(DButton(primary: b.on)).accessibilityAddTraits(b.on ? .isSelected : []).accessibilityIdentifier(idPrefix + ".page." + b.action)
                            }
                        }
                    }
                }
            }
            if rows.isEmpty, !empty.isEmpty { Text(empty).dmono(11).foregroundStyle(BackplaneDesign.dim) }
        }
    }
    private func binding(_ f: DVoiceField) -> Binding<String> { Binding(get: { typed[f.name] ?? f.value }, set: { typed[f.name] = $0 }) }
    private func press(_ r: DVoiceRow, _ b: (label: String, action: String, value: String, on: Bool)) {
        var fields: [String: String] = [:]
        for f in r.fields { fields[f.name] = typed[f.name] ?? f.value }
        Task {
            let err = await act(r, b, fields)
            errors[r.id] = err.isEmpty ? nil : err
            if err.isEmpty { for f in r.fields { typed[f.name] = nil } }
        }
    }
}
