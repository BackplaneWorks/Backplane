import SwiftUI

// Browser tasks (docs/teach.md): the Browser tab (the strip, the frame that
// takes the person's input while they drive, the save sheet), the Tasks tab,
// and the picture on a paused run's card. All of it is the screen's
// (src/mobile/bots.bend); these views only draw it and send actions back.

private let amber = Color(red: 0.96, green: 0.49, blue: 0)

// a picture the hub serves (a path with its query), with the pairing token
struct HubPicture: View {
    let model: AppModel
    let path: String

    var body: some View {
        if let u = model.web(path) {
            AsyncImage(url: u) { phase in
                switch phase {
                case .success(let img): img.resizable().scaledToFit()
                case .failure: Image(systemName: "photo").foregroundStyle(.secondary)
                default: ProgressView()
                }
            }
            .overlay(Rectangle().stroke(.quaternary, lineWidth: 1))
        }
    }
}

private struct StripButtons: View {
    let model: AppModel
    let buttons: [TBtn]

    var body: some View {
        ViewThatFits(in: .horizontal) {
            HStack(spacing: 8) { items }
            VStack(alignment: .leading, spacing: 6) { items }
        }
    }

    @ViewBuilder private var items: some View {
        ForEach(buttons, id: \.self) { b in
            if b.primary {
                Button(b.label) { model.act(b.action, b.value) }.buttonStyle(.borderedProminent).buttonBorderShape(.roundedRectangle(radius: 3))
            } else {
                Button(b.label) { model.act(b.action, b.value) }.buttonStyle(.bordered).buttonBorderShape(.roundedRectangle(radius: 3))
            }
        }
    }
}

// a field whose text goes to the client as "bfield" as it is typed
private struct TaskField: View {
    let model: AppModel
    let name: String
    let value: String
    let label: String
    var go: (() -> Void)?
    @State private var text = ""

    var body: some View {
        TextField(label, text: $text)
            .textFieldStyle(.roundedBorder)
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
            .submitLabel(go == nil ? .done : .go)
            .onSubmit { go?() }
            .onAppear { text = value }
            .onChange(of: value) { _, v in if v != text && v.isEmpty { text = "" } }
            .onChange(of: text) { _, t in model.field(name, t) }
    }
}

// the card of a paused run: its question, the screenshot, the choices
struct BrowserAskCard: View {
    let model: AppModel
    let ask: Ask

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Label(ask.head, systemImage: "hand.raised").font(.subheadline.bold())
            if !ask.detail.isEmpty { Text(ask.detail).font(.callout) }
            if let p = ask.image, !p.isEmpty { HubPicture(model: model, path: p) }
            FlowButtons(buttons: ask.buttons) { b in
                let a = b.action ?? ""
                model.act(a.isEmpty ? "answer" : a, b.value)
            }
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(amber.opacity(0.12), in: .rect(cornerRadius: 4))
    }
}

// the frame: while the person drives, a tap is "x y" in thousandths of the
// image as drawn and a vertical drag scrolls the page (about twice the
// finger's distance, in page pixels)
private struct DrivenFrame: View {
    let model: AppModel
    let image: UIImage
    let input: Bool
    var onTap: () -> Void = {}
    @State private var pulled: CGFloat = 0

    var body: some View {
        GeometryReader { g in
            Image(uiImage: image).resizable().scaledToFit()
                .frame(width: g.size.width, height: g.size.height)
                .contentShape(Rectangle())
                .gesture(DragGesture(minimumDistance: 0).onChanged { v in
                    guard input else { return }
                    let dy = v.translation.height
                    if abs(dy) > 12 {
                        let step = dy - pulled
                        if abs(step) >= 24 {
                            // a finger moving up scrolls the page down
                            model.act("bb-scroll", String(Int(-step * 2)))
                            pulled = dy
                        }
                    }
                }.onEnded { v in
                    defer { pulled = 0 }
                    guard input, abs(v.translation.height) < 12, abs(v.translation.width) < 12, g.size.width > 0, g.size.height > 0 else { return }
                    let x = Int(max(0, min(1000, v.location.x / g.size.width * 1000)))
                    let y = Int(max(0, min(1000, v.location.y / g.size.height * 1000)))
                    model.act("bb-click", "\(x) \(y)")
                    onTap()
                })
        }
        .aspectRatio(image.size.width / max(image.size.height, 1), contentMode: .fit)
        .overlay(Rectangle().stroke(input ? amber : .clear, lineWidth: 2))
    }
}

// while the person drives: the address, and the keys the frame cannot send
private struct DrivingBar: View {
    @Environment(\.backplaneLanguage) private var lang
    let model: AppModel
    let browser: BotBrowser
    @State private var typed = ""

    var body: some View {
        VStack(spacing: 6) {
            HStack(spacing: 6) {
                Button(NativeMessages.tour_back(lang)) { model.act("bb-back") }.buttonStyle(.bordered).buttonBorderShape(.roundedRectangle(radius: 3))
                Button(NativeMessages.common_reload(lang)) { model.act("bb-reload") }.buttonStyle(.bordered).buttonBorderShape(.roundedRectangle(radius: 3))
                TaskField(model: model, name: "bburl", value: browser.bburl ?? "", label: NativeMessages.browser_address(lang)) { model.act("bb-nav") }
                Button(NativeMessages.common_go(lang)) { model.act("bb-nav") }.buttonStyle(.borderedProminent).buttonBorderShape(.roundedRectangle(radius: 3))
            }
            HStack(spacing: 6) {
                TextField(NativeMessages.native_browser_typing(lang), text: $typed).textFieldStyle(.roundedBorder)
                    .textInputAutocapitalization(.never).autocorrectionDisabled()
                Button(NativeMessages.composer_send(lang)) {
                    if !typed.isEmpty { model.act("bb-type", typed); typed = "" }
                }.buttonStyle(.borderedProminent).buttonBorderShape(.roundedRectangle(radius: 3))
            }
            HStack(spacing: 6) {
                ForEach([(NativeMessages.native_key_enter(lang), "Enter"), (NativeMessages.native_key_tab(lang), "Tab"), ("\u{232B}", "Backspace"), (NativeMessages.native_key_esc(lang), "Escape")], id: \.0) { k in
                    Button(k.0) { model.act("bb-key", k.1) }.buttonStyle(.bordered).buttonBorderShape(.roundedRectangle(radius: 3))
                }
                Spacer(minLength: 0)
            }
        }
        .padding(.horizontal, 12)
    }
}

// a recording stopped: name it, mark typed values as parameters
private struct SaveSheet: View {
    @Environment(\.backplaneLanguage) private var lang
    let model: AppModel
    let browser: BotBrowser

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 10) {
                Text(browser.title ?? NativeMessages.teach_save(lang)).font(.headline)
                Text(NativeMessages.teach_name_note(lang))
                    .font(.footnote).foregroundStyle(.secondary)
                TaskField(model: model, name: "tkname", value: browser.tkname ?? "", label: NativeMessages.bots_name(lang))
                TaskField(model: model, name: "tkdesc", value: browser.tkdesc ?? "", label: NativeMessages.teach_description(lang))
                ForEach(browser.rows ?? []) { r in
                    VStack(alignment: .leading, spacing: 4) {
                        HStack(alignment: .firstTextBaseline, spacing: 8) {
                            Text("\(r.n).").font(.callout.monospaced()).foregroundStyle(.secondary)
                            Text(r.text).font(.callout)
                            Spacer(minLength: 0)
                            if r.secret { Text(NativeMessages.native_task_private(lang)).font(.caption2).foregroundStyle(.secondary) }
                        }
                        if !r.chip.isEmpty {
                            Button { model.act("tk-mark", String(r.n)) } label: {
                                Text(r.chip).lineLimit(1).font(.caption.monospaced())
                                    .padding(.horizontal, 8).padding(.vertical, 3)
                                    .background(r.marked ? Color.accentColor.opacity(0.25) : Color.secondary.opacity(0.15), in: .rect(cornerRadius: 2))
                            }.buttonStyle(.plain)
                        }
                        if r.marked { TaskField(model: model, name: r.field, value: "", label: r.pname) }
                    }
                }
                StripButtons(model: model, buttons: browser.btns ?? [])
            }
            .padding(12)
        }
    }
}

// Browser: the strip, then the page; a paused run's card leads
struct BrowserTab: View {
    @Environment(\.backplaneLanguage) private var lang
    let model: AppModel
    let browser: BotBrowser?
    let name: String
    @State private var image: UIImage?
    @State private var keyboard = false

    private var url: URL? {
        browser.flatMap { model.hubURL($0.url, query: [URLQueryItem(name: "n", value: $0.n)]) }
    }

    var body: some View {
        if let b = browser, b.mode == 2 {
            SaveSheet(model: model, browser: b)
        } else {
            ScrollView {
                VStack(alignment: .leading, spacing: 8) {
                    ForEach(browser?.asks ?? []) { a in
                        BrowserAskCard(model: model, ask: a).padding(.horizontal, 12).padding(.top, 4)
                    }
                    if let b = browser, let t = b.title, !t.isEmpty {
                        VStack(alignment: .leading, spacing: 6) {
                            HStack {
                                Text(t).font(.subheadline.bold())
                                if b.mode == 1 {
                                    let n = b.steps ?? 0
                                    Text(NativeMessages.teach_steps(lang, b.clock ?? "", String(n), n == 1 ? "one" : "other")).font(.caption).foregroundStyle(.secondary)
                                }
                            }
                            StripButtons(model: model, buttons: b.btns ?? [])
                        }.padding(.horizontal, 12)
                    } else if let b = browser {
                        StripButtons(model: model, buttons: b.btns ?? []).padding(.horizontal, 12)
                    }
                    if let b = browser, b.input == true {
                        DrivingBar(model: model, browser: b).id(b.url)
                        Button(keyboard ? "Hide keyboard" : "Keyboard") { keyboard.toggle() }.padding(.horizontal)
                        BrowserTextInput(active: $keyboard) { action, value in model.act(action, value) }
                            .frame(height: 1).id(b.url)
                    }
                    if let image {
                        DrivenFrame(model: model, image: image, input: browser?.input ?? false, onTap: { keyboard = true })
                    } else {
                        ContentUnavailableView(NativeMessages.native_browser_empty(lang), systemImage: "globe").frame(maxWidth: .infinity)
                    }
                    if let b = browser, b.mode == 1 {
                        HStack(spacing: 6) {
                            TaskField(model: model, name: "tknote", value: b.tknote ?? "", label: NativeMessages.native_task_note(lang)) { model.act("tk-note") }
                            Button(NativeMessages.teach_note_add(lang)) { model.act("tk-note") }.buttonStyle(.bordered).buttonBorderShape(.roundedRectangle(radius: 3))
                        }.padding(.horizontal, 12).padding(.bottom, 12)
                    }
                }
            }
            .onChange(of: browser?.url) { _, _ in keyboard = false; image = nil }
            .onChange(of: browser?.input) { _, enabled in if enabled != true { keyboard = false } }
            .task(id: url) {
                guard let u = url, let (d, r) = try? await URLSession.shared.data(from: u),
                      (r as? HTTPURLResponse)?.statusCode == 200, let i = UIImage(data: d) else { return }
                image = i
            }
        }
    }
}

private func statusColor(_ s: String) -> Color {
    switch s {
    case "ok": return .green
    case "failed", "stopped": return .red
    default: return .secondary
    }
}

// Tasks: what the bot has learned, each opened for its run form, then the
// recent runs with their screenshots
struct TasksTab: View {
    @Environment(\.backplaneLanguage) private var lang
    let model: AppModel
    let tasks: BotTasks?

    var body: some View {
        let items = tasks?.items ?? []
        let runs = tasks?.runs ?? []
        List {
            Text(items.isEmpty ? NativeMessages.teach_empty(lang) : NativeMessages.teach_run_note(lang))
                .font(.footnote).foregroundStyle(.secondary)
            ForEach(items) { t in
                VStack(alignment: .leading, spacing: 8) {
                    Button { model.act("tk-open", t.slug) } label: {
                        HStack(alignment: .firstTextBaseline, spacing: 8) {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(t.name).font(.subheadline.weight(.semibold))
                                if !t.desc.isEmpty { Text(t.desc).font(.caption).foregroundStyle(.secondary) }
                                if !t.params.isEmpty {
                                    Text(t.params.map { $0.label }.joined(separator: ", ")).font(.caption2).foregroundStyle(.secondary)
                                }
                            }
                            Spacer(minLength: 0)
                            if !t.last.isEmpty { Text(NativeMessages.native_task_state(lang, t.last)).font(.caption).foregroundStyle(statusColor(t.last)) }
                        }
                        .contentShape(Rectangle())
                    }.buttonStyle(.plain)
                    if t.open {
                        ForEach(t.params, id: \.field) { p in
                            TaskField(model: model, name: p.field, value: p.value, label: p.example.isEmpty ? p.label : NativeMessages.native_task_parameter(lang, p.label, p.example))
                        }
                        HStack(spacing: 8) {
                            Button(NativeMessages.common_run(lang)) { model.act("tk-run", t.slug) }.buttonStyle(.borderedProminent).buttonBorderShape(.roundedRectangle(radius: 3))
                            Button(t.asking ? NativeMessages.native_task_delete_ask(lang) : NativeMessages.common_delete(lang), role: .destructive) { model.act("tk-del", t.slug) }
                                .buttonStyle(.bordered).buttonBorderShape(.roundedRectangle(radius: 3))
                        }
                    }
                }
            }
            if !runs.isEmpty {
                Section(NativeMessages.teach_runs(lang)) {
                    ForEach(runs) { r in
                        VStack(alignment: .leading, spacing: 6) {
                            HStack(alignment: .firstTextBaseline) {
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(r.slug).font(.subheadline.weight(.semibold))
                                    if !r.summary.isEmpty { Text(r.summary).font(.caption).foregroundStyle(.secondary) }
                                }
                                Spacer(minLength: 0)
                                Text(NativeMessages.native_task_state(lang, r.status)).font(.caption).foregroundStyle(statusColor(r.status))
                            }
                            if !r.shots.isEmpty {
                                ScrollView(.horizontal, showsIndicators: false) {
                                    HStack(spacing: 6) {
                                        ForEach(r.shots, id: \.self) { s in HubPicture(model: model, path: s).frame(width: 120) }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// A native text responder supplies committed IME/dictation/paste text. It never
// holds the remote field value; the image remains the source of truth.
private struct BrowserTextInput: UIViewRepresentable {
    @Binding var active: Bool
    var send: (String, String) -> Void
    func makeCoordinator() -> Coordinator { Coordinator(self) }
    func makeUIView(context: Context) -> UITextView {
        let view = RemoteTextView()
        view.send = send
        view.delegate = context.coordinator
        view.text = "\u{200B}"
        view.selectedRange = NSRange(location: 1, length: 0)
        view.autocorrectionType = .no
        view.autocapitalizationType = .none
        view.smartQuotesType = .no
        view.smartDashesType = .no
        view.textColor = .clear
        view.tintColor = .clear
        view.backgroundColor = .clear
        view.accessibilityLabel = "Type into the remote browser"
        return view
    }
    func updateUIView(_ view: UITextView, context: Context) {
        context.coordinator.parent = self
        (view as? RemoteTextView)?.send = send
        if active && !view.isFirstResponder { view.becomeFirstResponder() }
        if !active && view.isFirstResponder { view.resignFirstResponder() }
    }
    final class Coordinator: NSObject, UITextViewDelegate {
        var parent: BrowserTextInput
        init(_ parent: BrowserTextInput) { self.parent = parent }
        func reset(_ view: UITextView) {
            view.text = "\u{200B}"
            view.selectedRange = NSRange(location: 1, length: 0)
        }
        func textView(_ view: UITextView, shouldChangeTextIn range: NSRange, replacementText text: String) -> Bool {
            guard parent.active else { return false }
            if view.markedTextRange != nil { return true }
            if text == "\n" { parent.send("bb-key", "Enter"); return false }
            if text.isEmpty && range.length > 0 {
                parent.send("bb-key", "Backspace"); reset(view); return false
            }
            return true
        }
        func textViewDidChange(_ view: UITextView) {
            guard parent.active, view.markedTextRange == nil else { return }
            let text = view.text.hasPrefix("\u{200B}") ? String(view.text.dropFirst()) : view.text ?? ""
            if !text.isEmpty { parent.send("bb-type", text) }
            reset(view)
        }
        func textViewDidEndEditing(_ view: UITextView) { reset(view) }
    }
}

private final class RemoteTextView: UITextView {
    var send: (String, String) -> Void = { _, _ in }
    override var keyCommands: [UIKeyCommand]? {
        let keys = [UIKeyCommand.inputLeftArrow, UIKeyCommand.inputRightArrow,
                    UIKeyCommand.inputUpArrow, UIKeyCommand.inputDownArrow, "\t", "\r", UIKeyCommand.inputEscape]
        let modifiers: [UIKeyModifierFlags] = [[], .shift, .control, .command, [.control, .shift], [.command, .shift]]
        var commands = modifiers.flatMap { flags in keys.map { key in
            UIKeyCommand(input: key, modifierFlags: flags, action: #selector(remoteKey(_:)))
        } }
        for flags: UIKeyModifierFlags in [.command, .control, [.command, .shift], [.control, .shift]] {
            for key in ["a", "z", "y"] { commands.append(UIKeyCommand(input: key, modifierFlags: flags, action: #selector(remoteKey(_:)))) }
        }
        for command in commands { command.wantsPriorityOverSystemBehavior = true }
        return commands
    }
    @objc private func remoteKey(_ command: UIKeyCommand) {
        guard let input = command.input else { return }
        let names = [UIKeyCommand.inputLeftArrow: "ArrowLeft", UIKeyCommand.inputRightArrow: "ArrowRight",
                     UIKeyCommand.inputUpArrow: "ArrowUp", UIKeyCommand.inputDownArrow: "ArrowDown",
                     "\t": "Tab", "\r": "Enter", UIKeyCommand.inputEscape: "Escape"]
        let primary = !command.modifierFlags.intersection([.command, .control]).isEmpty
        let mods = (primary ? "ControlOrMeta+" : "") + (command.modifierFlags.contains(.shift) ? "Shift+" : "")
        send("bb-key", mods + (names[input] ?? input))
    }
}
