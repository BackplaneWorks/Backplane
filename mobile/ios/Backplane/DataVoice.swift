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
    /// This session's sends while dictating, and the number of the last edit written, per thread (the hub's `Reader::cut`).
    @ObservationIgnored private var cuts: [String: Int] = [:]
    @ObservationIgnored private var seqs: [String: Int] = [:]
    @ObservationIgnored let recorder = VoiceRecorder()
    /// A voice command's draft in its undo window (cmds.rs on the hub): a send goes when it ends, a cancel is gone; and what happened, shown a moment.
    struct Pending { var id: Int; var thread: String; var text: String; var cmd: String }
    private(set) var pending: Pending?
    private(set) var said: (thread: String, text: String, until: Date)?
    @ObservationIgnored private var nextPending = 0
    @ObservationIgnored weak var client: DataClient?

    init() { recorder.onGone = { [weak self] in self?.recorderGone() } }

    func shown(_ thread: String) -> DVoiceView? {
        var v = views[thread] ?? DVoiceView()
        if v.note.isEmpty, let n = local[thread] { v.note = n; v.bad = true }
        if active == thread { v.on = true }
        if pending?.thread == thread || (said?.thread == thread && said!.until > Date()) { return v }
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
            marks[thread] = caret; cuts[thread] = nil; seqs[thread] = nil
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
    /// The message went: what was left to clarify in it goes too. While dictating, the words written so far went with it and what is said next
    /// starts the empty draft: the hub is told up to which edit (`seq`), and edits it made before it heard (`cut` behind ours) are dropped.
    func sent(_ thread: String, draft: String) {
        if let v = views[thread], v.on, let client {
            let n = (cuts[thread] ?? 0) + 1; cuts[thread] = n; marks[thread] = 0
            let p: [String: Any] = ["thread": thread, "action": "voice-clear", "value": "", "draft": draft, "cut": n, "seq": seqs[thread] ?? 0]
            Task { _ = try? await client.call("voice.act", p, as: DEmpty.self) }
            return
        }
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
            if !view.on { marks[thread] = nil; cuts[thread] = nil; seqs[thread] = nil; level = 0; if active == thread { recorder.gone(); active = nil } }
        case "level": level = max(0, min(100, n("v")))
        case "edit":
            if let c = o["cut"] as? Int, c < (cuts[thread] ?? 0) { return } // made before this phone's send: it belongs to nothing now
            if let q = o["seq"] as? Int { seqs[thread] = q }
            let text = client.drafts[thread] ?? ""
            let mark = marks[thread] ?? VoiceEdit.count(text)
            let r = VoiceEdit.apply(text, mark: mark, del: n("del"), ins: o["ins"] as? String ?? "", adv: n("adv"), to: n("to"), back: n("back"))
            marks[thread] = r.mark
            client.putDraft(thread, r.text)
            caret = (thread, r.caret, caret.rev + 1)
        case "draft":
            let old = client.drafts[thread] ?? "", new = o["text"] as? String ?? ""
            if let m = marks[thread] { marks[thread] = VoiceEdit.shift(old, new, mark: m) }
            client.putDraft(thread, new)
            caret = (thread, VoiceEdit.count(new), caret.rev + 1)
        case "command": command(thread, cmd: o["cmd"] as? String ?? "", cut: n("cut"))
        case "page": setRows(o["rows"])
        default: break
        }
    }

    // MARK: voice commands
    /// The hub's `command` frame: the whole draft is taken now (dictation goes on into the empty draft), and sent (or dropped, for cancel) after
    /// the undo window. A send goes as the Send button's does: turn.start, which steers a running turn.
    private func command(_ thread: String, cmd: String, cut: Int) {
        guard let client, cmd == "send" || cmd == "cancel" else { return }
        cuts[thread] = max(cuts[thread] ?? 0, cut); marks[thread] = 0
        if let p = pending { deliver(p.id) }
        let text = client.drafts[thread] ?? ""
        guard !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
        client.putDraft(thread, ""); caret = (thread, 0, caret.rev + 1)
        nextPending += 1; let id = nextPending
        pending = Pending(id: id, thread: thread, text: text, cmd: cmd)
        Task { @MainActor [weak self] in try? await Task.sleep(nanoseconds: 3_000_000_000); self?.deliver(id) }
    }
    func deliver(_ id: Int) {
        guard let client, let p = pending, p.id == id else { return }
        pending = nil
        if p.cmd == "send" { client.send(p.thread, p.text) }
        said = (p.thread, p.cmd == "send" ? "Sent by voice" : "Draft cleared by voice", Date().addingTimeInterval(2))
    }
    /// Undo: the message stays; its text goes back in front of whatever was dictated since.
    func undo() {
        guard let client, let p = pending else { return }
        pending = nil
        let now = client.drafts[p.thread] ?? ""
        let u = VoiceEdit.undo(p.text, now, mark: marks[p.thread] ?? 0)
        client.putDraft(p.thread, u.text); if marks[p.thread] != nil { marks[p.thread] = u.mark }
        caret = (p.thread, VoiceEdit.count(u.text), caret.rev + 1)
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
            else { _ = try await client.call("voice.page.act", ["action": action, "value": action == "voice-term" ? (fields["vterm"] ?? "") : action == "voice-phrases" ? (fields["vphrase"] ?? "") : value], as: DEmpty.self) }
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
                if let p = client.voice.pending, p.thread == thread {
                    HStack(spacing: 8) {
                        Rectangle().fill(p.cmd == "send" ? BackplaneDesign.accent : BackplaneDesign.bad).frame(width: 8, height: 8)
                        Text(Tr.t(p.cmd == "send" ? "Sending by voice…" : "Draft cleared by voice")).dmono(12).foregroundStyle(BackplaneDesign.dim).frame(maxWidth: .infinity, alignment: .leading).accessibilityIdentifier("voice.pending")
                        Button(Tr.t("Undo")) { client.voice.undo() }.buttonStyle(DButton()).accessibilityIdentifier("voice.undo")
                    }
                } else if let s = client.voice.said, s.thread == thread, s.until > Date() {
                    Text(Tr.t(s.text)).dmono(12).foregroundStyle(BackplaneDesign.dim).accessibilityIdentifier("voice.said")
                }
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
/// A row's press the phone does itself, never sent (docs/PHONE-DATA.md, as the Android app's PageRow.local): copy (and the older copy-text), share-text
/// (the share sheet), open-url (http and https only).
@MainActor
enum DPageLocal {
    static func local(_ action: String) -> Bool { ["copy", "copy-text", "share-text", "open-url"].contains(action) }
    static func copied(_ action: String) -> Bool { action == "copy" || action == "copy-text" }
    static func webURL(_ v: String) -> URL? { let t = v.trimmingCharacters(in: .whitespaces); let l = t.lowercased(); guard l.hasPrefix("https://") || l.hasPrefix("http://"), let u = URL(string: t), u.host != nil else { return nil }; return u }
    /// True when it was the phone's own press, whether or not it could be done.
    static func run(_ action: String, _ value: String) -> Bool {
        guard local(action) else { return false }
        switch action {
        case "copy", "copy-text": UIPasteboard.general.string = value
        case "share-text":
            let vc = UIActivityViewController(activityItems: [value], applicationActivities: nil)
            let top = UIApplication.shared.connectedScenes.compactMap { ($0 as? UIWindowScene)?.keyWindow?.rootViewController }.first
            var at = top; while let p = at?.presentedViewController { at = p }
            vc.popoverPresentationController?.sourceView = at?.view
            at?.present(vc, animated: true)
        default: if let u = webURL(value) { UIApplication.shared.open(u) }
        }
        return true
    }
}

struct DPageRows: View {
    let title: String
    let idPrefix: String
    let rows: [DVoiceRow]
    var empty = ""
    let act: (DVoiceRow, (label: String, action: String, value: String, on: Bool), [String: String]) async -> String
    @State private var typed: [String: String] = [:]
    @State private var errors: [Int: String] = [:]
    @State private var told: [Int: String] = [:]   // what a press the phone did itself says (Copied), under the row's note

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            DHeading(text: title)
            ForEach(rows) { r in
                DPageRowView(row: r, idPrefix: idPrefix, note: errors[r.id] ?? r.note, noteTone: errors[r.id] != nil ? "bad" : r.tone, told: told[r.id],
                             text: { binding($0) }, press: { press(r, $0) })
            }
            if rows.isEmpty, !empty.isEmpty { Text(empty).dmono(11).foregroundStyle(BackplaneDesign.dim) }
        }
    }
    private func binding(_ f: DVoiceField) -> Binding<String> { Binding(get: { typed[f.name] ?? f.value }, set: { typed[f.name] = $0 }) }
    private func press(_ r: DVoiceRow, _ b: (label: String, action: String, value: String, on: Bool)) {
        if DPageLocal.run(b.action, b.value) { told[r.id] = DPageLocal.copied(b.action) ? Tr.t("Copied") : nil; return }
        var fields: [String: String] = [:]
        for f in r.fields { fields[f.name] = typed[f.name] ?? f.value }
        Task {
            let err = await act(r, b, fields)
            errors[r.id] = err.isEmpty ? nil : err
            if err.isEmpty { for f in r.fields { typed[f.name] = nil } }
        }
    }
}

/// One of those rows: its label, note (in its tone), what a press said, a code, its fields and its buttons. `compact`: smaller buttons, for a row
/// drawn inside a card (Settings > MCP servers).
struct DPageRowView: View {
    let row: DVoiceRow
    let idPrefix: String
    let note: String
    let noteTone: String
    let told: String?
    var compact = false
    let text: (DVoiceField) -> Binding<String>
    let press: ((label: String, action: String, value: String, on: Bool)) -> Void
    var body: some View {
        let r = row
        VStack(alignment: .leading, spacing: 6) {
            Text(Tr.t(r.label)).dmono(13)
            if !note.isEmpty { Text(Tr.t(note)).dmono(11).foregroundStyle(noteTone == "bad" ? BackplaneDesign.bad : noteTone == "warn" ? BackplaneDesign.warn : BackplaneDesign.dim).fixedSize(horizontal: false, vertical: true) }
            if let t = told { Text(t).dmono(11).foregroundStyle(BackplaneDesign.dim).accessibilityIdentifier(idPrefix + ".told") }
            if !r.qr.isEmpty { DQRCode(rows: r.qr).accessibilityIdentifier(idPrefix + ".qr") }
            if !r.qrnote.isEmpty { Text(Tr.t(r.qrnote)).dmono(12).foregroundStyle(BackplaneDesign.dim).textSelection(.enabled) }
            ForEach(r.fields, id: \.name) { f in
                Group {
                    if f.secret { SecureField(Tr.t(f.hint), text: text(f)) } else { TextField(Tr.t(f.hint), text: text(f)) }
                }
                .dmono(13).textInputAutocapitalization(.never).autocorrectionDisabled()
                .padding(.horizontal, 10).frame(minHeight: 40).dbox()
                .accessibilityIdentifier(idPrefix + ".field." + f.name)
            }
            if !r.buttons.isEmpty {
                FlowRow(spacing: 6) {
                    ForEach(Array(r.buttons.enumerated()), id: \.offset) { _, b in
                        Button(Tr.t(b.label)) { press(b) }.buttonStyle(DButton(primary: b.on, compact: compact)).accessibilityAddTraits(b.on ? .isSelected : []).accessibilityIdentifier(idPrefix + ".page." + b.action)
                    }
                }
            }
        }
    }
}
