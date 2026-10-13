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
                })
        }
        .aspectRatio(image.size.width / max(image.size.height, 1), contentMode: .fit)
        .overlay(Rectangle().stroke(input ? amber : .clear, lineWidth: 2))
    }
}

// while the person drives: the address, and the keys the frame cannot send
private struct DrivingBar: View {
    let model: AppModel
    let browser: BotBrowser
    @State private var typed = ""

    var body: some View {
        VStack(spacing: 6) {
            HStack(spacing: 6) {
                Button("Back") { model.act("bb-back") }.buttonStyle(.bordered).buttonBorderShape(.roundedRectangle(radius: 3))
                Button("Reload") { model.act("bb-reload") }.buttonStyle(.bordered).buttonBorderShape(.roundedRectangle(radius: 3))
                TaskField(model: model, name: "bburl", value: browser.bburl ?? "", label: "Address") { model.act("bb-nav") }
                Button("Go") { model.act("bb-nav") }.buttonStyle(.borderedProminent).buttonBorderShape(.roundedRectangle(radius: 3))
            }
            HStack(spacing: 6) {
                TextField("Type into the page", text: $typed).textFieldStyle(.roundedBorder)
                    .textInputAutocapitalization(.never).autocorrectionDisabled()
                Button("Send") {
                    if !typed.isEmpty { model.act("bb-type", typed); typed = "" }
                }.buttonStyle(.borderedProminent).buttonBorderShape(.roundedRectangle(radius: 3))
            }
            HStack(spacing: 6) {
                ForEach([("Enter", "Enter"), ("Tab", "Tab"), ("\u{232B}", "Backspace"), ("Esc", "Escape")], id: \.0) { k in
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
    let model: AppModel
    let browser: BotBrowser

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 10) {
                Text(browser.title ?? "Save this task").font(.headline)
                Text("Name it. Mark a value you typed to ask for it each time the task runs.")
                    .font(.footnote).foregroundStyle(.secondary)
                TaskField(model: model, name: "tkname", value: browser.tkname ?? "", label: "Name")
                TaskField(model: model, name: "tkdesc", value: browser.tkdesc ?? "", label: "What it does (optional)")
                ForEach(browser.rows ?? []) { r in
                    VStack(alignment: .leading, spacing: 4) {
                        HStack(alignment: .firstTextBaseline, spacing: 8) {
                            Text("\(r.n).").font(.callout.monospaced()).foregroundStyle(.secondary)
                            Text(r.text).font(.callout)
                            Spacer(minLength: 0)
                            if r.secret { Text("private").font(.caption2).foregroundStyle(.secondary) }
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
    let model: AppModel
    let browser: BotBrowser?
    let name: String
    @State private var image: UIImage?

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
                                    Text("\(b.clock ?? "") \u{00B7} \(n) \(n == 1 ? "step" : "steps")").font(.caption).foregroundStyle(.secondary)
                                }
                            }
                            StripButtons(model: model, buttons: b.btns ?? [])
                        }.padding(.horizontal, 12)
                    } else if let b = browser {
                        StripButtons(model: model, buttons: b.btns ?? []).padding(.horizontal, 12)
                    }
                    if let b = browser, b.input == true { DrivingBar(model: model, browser: b) }
                    if let image {
                        DrivenFrame(model: model, image: image, input: browser?.input ?? false)
                    } else {
                        ContentUnavailableView("No page yet", systemImage: "globe").frame(maxWidth: .infinity)
                    }
                    if let b = browser, b.mode == 1 {
                        HStack(spacing: 6) {
                            TaskField(model: model, name: "tknote", value: b.tknote ?? "", label: "Tell it what matters") { model.act("tk-note") }
                            Button("Add note") { model.act("tk-note") }.buttonStyle(.bordered).buttonBorderShape(.roundedRectangle(radius: 3))
                        }.padding(.horizontal, 12).padding(.bottom, 12)
                    }
                }
            }
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
    let model: AppModel
    let tasks: BotTasks?

    var body: some View {
        let items = tasks?.items ?? []
        let runs = tasks?.runs ?? []
        List {
            Text(items.isEmpty ? "None yet. Teach one in the Browser tab, or tell it in chat." : "Open one to run it. A routine can run it too.")
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
                            if !t.last.isEmpty { Text(t.last).font(.caption).foregroundStyle(statusColor(t.last)) }
                        }
                        .contentShape(Rectangle())
                    }.buttonStyle(.plain)
                    if t.open {
                        ForEach(t.params, id: \.field) { p in
                            TaskField(model: model, name: p.field, value: p.value, label: p.label + (p.example.isEmpty ? "" : " (\(p.example))"))
                        }
                        HStack(spacing: 8) {
                            Button("Run") { model.act("tk-run", t.slug) }.buttonStyle(.borderedProminent).buttonBorderShape(.roundedRectangle(radius: 3))
                            Button(t.asking ? "Delete this task?" : "Delete", role: .destructive) { model.act("tk-del", t.slug) }
                                .buttonStyle(.bordered).buttonBorderShape(.roundedRectangle(radius: 3))
                        }
                    }
                }
            }
            if !runs.isEmpty {
                Section("Recent runs") {
                    ForEach(runs) { r in
                        VStack(alignment: .leading, spacing: 6) {
                            HStack(alignment: .firstTextBaseline) {
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(r.slug).font(.subheadline.weight(.semibold))
                                    if !r.summary.isEmpty { Text(r.summary).font(.caption).foregroundStyle(.secondary) }
                                }
                                Spacer(minLength: 0)
                                Text(r.status).font(.caption).foregroundStyle(statusColor(r.status))
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
