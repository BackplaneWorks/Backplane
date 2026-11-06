import SwiftUI
import UIKit
import PhotosUI

// The phone app's screens for data/1 (docs/gpui/PHONE-DATA.md): native SwiftUI over typed values from DataClient. Fresh-home scope: projects, their
// threads, one thread (entries, a composer, approvals), Improvements, and this phone's own settings.

struct DataRoot: View {
    @Bindable var client: DataClient
    @Bindable private var settings = AppSettings.shared
    @Environment(\.scenePhase) private var phase
    @State private var tab = ""

    var body: some View {
        Group {
            if client.links.isEmpty || client.needsRepair {
                NavigationStack { PairView(tag: "en", link: "", status: client.pairingError, repair: client.needsRepair) { client.pair($0) } }
            } else {
                TabView(selection: Binding(get: { tab.isEmpty ? (settings.defaultTab == "settings" || settings.defaultTab == "improvements" ? settings.defaultTab : "threads") : tab }, set: { tab = $0 })) {
                    NavigationStack { DataThreadsView(client: client) }
                        .tabItem { Label("Threads", systemImage: "bubble.left.and.bubble.right") }.tag("threads")
                    NavigationStack { DataImprovementsView(client: client) }
                        .tabItem { Label("Improvements", systemImage: "lightbulb") }.tag("improvements")
                    NavigationStack { DataSettingsView(client: client) }
                        .tabItem { Label("Settings", systemImage: "gearshape") }.tag("settings")
                }
            }
        }
        .tint(BackplaneDesign.accent)
        .preferredColorScheme(settings.colorScheme)
        .modifier(TextSizeModifier(size: settings.dynamicType))
        .task { if !client.links.isEmpty, client.conn == .offline { client.connect() } }
        .onChange(of: phase) { _, p in if p == .active { client.foreground() } }
        // a newly paired (or forgotten) hub starts again on the default screen
        .onChange(of: client.links.isEmpty) { _, _ in tab = "" }
    }
}

// MARK: threads

struct DataThreadsView: View {
    @Bindable var client: DataClient
    @State private var addingProject = false
    @State private var newThread: String?       // a project id
    @State private var path: [String] = []
    @State private var open: String?

    private func threads(_ p: DProject) -> [DThread] { client.threads.filter { $0.project == p.id }.prefix(8).map { $0 } }

    var body: some View {
        List {
            if client.conn != .online { DataConnectionBanner(client: client).listRowSeparator(.hidden) }
            if client.projects.isEmpty && client.listsLoaded {
                VStack(alignment: .leading, spacing: 10) {
                    Text("No projects yet").font(.headline)
                    Text("A project is a folder on your hub's computer. Add one to start a thread.").foregroundStyle(.secondary)
                    Button("Add a project") { addingProject = true }.buttonStyle(.borderedProminent).accessibilityIdentifier("data.project.add.empty")
                }.padding(.vertical, 12)
            }
            ForEach(client.projects) { p in
                Section {
                    ForEach(threads(p)) { t in
                        NavigationLink(value: t.id) { DataThreadRow(thread: t) }
                            .onAppear { client.prefetch(t.id) }
                            .accessibilityIdentifier("data.thread.row." + t.id)
                    }
                    if client.threads.filter({ $0.project == p.id }).isEmpty {
                        Text("No threads").foregroundStyle(.secondary)
                    }
                    Button { newThread = p.id } label: { Label("New thread", systemImage: "square.and.pencil") }
                        .accessibilityLabel("New thread in \(p.title)").accessibilityIdentifier("data.thread.new." + p.id)
                } header: {
                    HStack {
                        Text(p.title).font(.headline).textCase(nil).foregroundStyle(.primary)
                        if p.waiting > 0 { Text("\(p.waiting)").font(.caption.bold()).padding(.horizontal, 6).background(Capsule().fill(Color.orange.opacity(0.3))) }
                        Spacer()
                    }
                }
            }
        }
        .listStyle(.insetGrouped)
        .navigationTitle("Threads")
        .navigationDestination(for: String.self) { DataThreadView(client: client, id: $0) }
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button { addingProject = true } label: { Image(systemName: "folder.badge.plus") }
                    .accessibilityLabel("Add project").accessibilityIdentifier("data.project.add")
            }
        }
        .refreshable { await client.refresh() }
        .sheet(isPresented: $addingProject) { DataNewProjectSheet(client: client) }
        .sheet(item: Binding(get: { newThread.map { Wrapped(id: $0) } }, set: { newThread = $0?.id })) { w in
            DataNewThreadSheet(client: client, project: w.id)
        }
    }
    struct Wrapped: Identifiable { var id: String }
}

struct DataConnectionBanner: View {
    let client: DataClient
    var body: some View {
        HStack(spacing: 8) {
            if client.conn == .connecting { ProgressView() } else { Image(systemName: "wifi.slash") }
            VStack(alignment: .leading, spacing: 2) {
                Text(client.conn == .connecting ? "Connecting to \(client.hubName)…" : "Offline · showing what this phone has saved").font(.subheadline)
                if client.conn == .offline, !client.failure.isEmpty { Text(client.failure).font(.caption).foregroundStyle(.secondary) }
            }
            Spacer()
            if client.conn == .offline { Button("Retry") { client.connect() } }
        }
        .padding(10).background(RoundedRectangle(cornerRadius: 0).fill(BackplaneDesign.raised))
        .accessibilityIdentifier("data.connection.banner")
    }
}

struct DataThreadRow: View {
    let thread: DThread
    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            Circle().fill(color).frame(width: 10, height: 10).padding(.top, 6)
            VStack(alignment: .leading, spacing: 3) {
                Text(thread.title.isEmpty ? "New thread" : thread.title).font(.body).lineLimit(1)
                if !thread.preview.isEmpty { Text(thread.preview).font(.subheadline).foregroundStyle(.secondary).lineLimit(2) }
            }
            Spacer(minLength: 0)
            Text(ago(thread.updatedAt)).font(.caption).foregroundStyle(.secondary)
        }
    }
    private var color: Color {
        switch thread.state { case "working": .blue; case "asking": .orange; case "failed": .red; default: Color.secondary.opacity(0.35) }
    }
    private func ago(_ ms: Int) -> String {
        let s = max(0, Int(Date().timeIntervalSince1970) - ms / 1000)
        if s < 60 { return "now" }; if s < 3600 { return "\(s / 60)m" }; if s < 86400 { return "\(s / 3600)h" }; return "\(s / 86400)d"
    }
}

// MARK: new project / new thread

struct DataNewProjectSheet: View {
    let client: DataClient
    @Environment(\.dismiss) private var dismiss
    @State private var dirs: DDirs?
    @State private var path = ""
    @State private var error = ""
    @State private var busy = false

    var body: some View {
        NavigationStack {
            List {
                Section {
                    TextField("Folder on the hub's computer", text: $path).textInputAutocapitalization(.never).autocorrectionDisabled()
                        .accessibilityIdentifier("data.project.path")
                    if !error.isEmpty { Text(error).foregroundStyle(.red).font(.caption) }
                } footer: { Text("Type a path, or browse below. A folder that does not exist is created.") }
                if let d = dirs {
                    Section(d.path) {
                        if d.parent != d.path { Button { Task { await go(d.parent) } } label: { Label("..", systemImage: "arrow.up.left") } }
                        ForEach(d.dirs, id: \.self) { n in
                            Button { Task { await go(d.path.hasSuffix("/") ? d.path + n : d.path + "/" + n) } } label: { Label(n, systemImage: "folder") }
                        }
                    }
                }
            }
            .navigationTitle("Add project")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Add") { Task { await add() } }.disabled(busy || path.trimmingCharacters(in: .whitespaces).isEmpty).accessibilityIdentifier("data.project.add.confirm")
                }
            }
            .task { await go(nil) }
        }
    }
    private func go(_ p: String?) async {
        do { let d = try await client.dirs(p); dirs = d; path = d.path; error = "" } catch { self.error = error.localizedDescription }
    }
    private func add() async {
        busy = true; defer { busy = false }
        do { _ = try await client.addProject(path: path.trimmingCharacters(in: .whitespaces)); dismiss() } catch { self.error = error.localizedDescription }
    }
}

struct DataNewThreadSheet: View {
    let client: DataClient
    let project: String
    @Environment(\.dismiss) private var dismiss
    @State private var text = ""
    @State private var error = ""
    @State private var busy = false

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    TextEditor(text: $text).frame(minHeight: 140).accessibilityIdentifier("data.thread.new.text")
                } header: { Text("First message (optional)") } footer: { if !error.isEmpty { Text(error).foregroundStyle(.red) } }
            }
            .navigationTitle("New thread")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) { Button("Start") { Task { await start() } }.disabled(busy).accessibilityIdentifier("data.thread.new.start") }
            }
        }
    }
    private func start() async {
        busy = true; defer { busy = false }
        do { _ = try await client.createThread(project: project, text: text); dismiss() } catch { self.error = error.localizedDescription }
    }
}

// MARK: one thread

struct DataThreadView: View {
    @Bindable var client: DataClient
    let id: String
    @State private var text = ""
    @State private var pending: [Data] = []
    @State private var picked: [PhotosPickerItem] = []
    @State private var notice = ""

    init(client: DataClient, id: String) { self.client = client; self.id = id; client.touch(id) }

    private var thread: DThread? { client.threads.first { $0.id == id } }
    private var list: [DEntry] { client.entries[id] ?? [] }
    private var working: Bool { thread?.state == "working" || thread?.state == "asking" }

    var body: some View {
        VStack(spacing: 0) {
            if client.conn != .online { DataConnectionBanner(client: client) }
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 12) {
                        if client.older[id] != nil { ProgressView().frame(maxWidth: .infinity).onAppear { client.loadOlder(id) } }
                        ForEach(list) { e in DataEntryRow(client: client, thread: id, entry: e).id(e.id) }
                        if list.isEmpty && client.loadedThreads.contains(id) { Text("Nothing here yet. Send a message.").foregroundStyle(.secondary).padding(.top, 40).frame(maxWidth: .infinity) }
                        if thread?.state == "working", list.last?.live != true { DataThinkingRow() }
                        Color.clear.frame(height: 1).id("end")
                    }
                    .padding(.horizontal).padding(.top, 8)
                }
                .defaultScrollAnchor(.bottom)
                .scrollDismissesKeyboard(.interactively)
                .onChange(of: list.last?.id) { _, _ in proxy.scrollTo("end", anchor: .bottom) }
                .onChange(of: list.count) { _, _ in proxy.scrollTo("end", anchor: .bottom) }
            }
            if !notice.isEmpty {
                Text(notice).font(.footnote).foregroundStyle(.orange).frame(maxWidth: .infinity, alignment: .leading).padding(.horizontal).padding(.vertical, 6).background(BackplaneDesign.panel)
                    .accessibilityIdentifier("data.images.notice")
            }
            if !pending.isEmpty {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 8) {
                        ForEach(Array(pending.enumerated()), id: \.offset) { i, d in
                            ZStack(alignment: .topTrailing) {
                                if let u = UIImage(data: d) { Image(uiImage: u).resizable().scaledToFill().frame(width: 64, height: 64).clipShape(RoundedRectangle(cornerRadius: 8)) }
                                Button { pending.remove(at: i) } label: { Image(systemName: "xmark.circle.fill").foregroundStyle(.white, .black.opacity(0.6)) }
                                    .accessibilityLabel("Remove picture")
                            }
                        }
                    }.padding(.horizontal)
                }.padding(.vertical, 6).background(BackplaneDesign.panel).accessibilityIdentifier("data.pending.images")
            }
            HStack(alignment: .bottom, spacing: 8) {
                PhotosPicker(selection: $picked, maxSelectionCount: max(0, 4 - pending.count), matching: .images) {
                    Image(systemName: "paperclip").font(.system(size: 22)).frame(minWidth: 36, minHeight: 44)
                }
                .disabled(!client.canSendImages)
                .accessibilityLabel(client.canSendImages ? "Attach pictures" : "Attach pictures (this hub does not take pictures yet)").accessibilityIdentifier("data.attach")
                UIKitField(text: text, placeholder: "Message", editable: true,
                           onChange: { text = $0; client.setDraft(id, $0) },
                           onSend: { send() }, onPasteImages: { imgs in add(imgs.compactMap { $0.jpegData(compressionQuality: 0.95) }) })
                    .padding(.horizontal, 12).padding(.vertical, 8)
                    .background(BackplaneDesign.raised, in: RoundedRectangle(cornerRadius: 10))
                    .accessibilityLabel("Message")
                let blank = text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && pending.isEmpty
                let stop = working && blank
                Button { if stop { Task { await client.interrupt(id) } } else { send() } } label: {
                    Image(systemName: stop ? "stop.circle.fill" : "arrow.up.circle.fill").font(.system(size: 32))
                        .foregroundStyle(stop ? Color.red : blank ? Color.secondary : BackplaneDesign.accent)
                }
                .frame(minWidth: 44, minHeight: 44)
                .disabled(blank && !stop)
                .accessibilityLabel(stop ? "Stop" : "Send").accessibilityIdentifier("thread.send")
            }
            .padding(.horizontal).padding(.vertical, 8)
            .background(BackplaneDesign.panel)
        }
        .navigationTitle(thread?.title.isEmpty == false ? thread!.title : "Thread")
        .navigationBarTitleDisplayMode(.inline)
        .onChange(of: picked) { _, items in
            guard !items.isEmpty else { return }
            Task {
                var raw: [Data] = []
                for it in items { if let d = try? await it.loadTransferable(type: Data.self) { raw.append(d) } }
                picked = []
                add(raw)
            }
        }
        .onAppear {
            let had = !(client.entries[id] ?? []).isEmpty
            text = client.drafts[id] ?? ""; client.openThread(id)
            DispatchQueue.main.async { client.shown(id, hadCopy: had) }
        }
        .onDisappear { client.closeThread(id) }
    }
    private func send() {
        let t = text
        guard !t.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || !pending.isEmpty else { return }
        let images = pending
        text = ""; pending = []
        client.send(id, t, images: images)
    }
    // pictures chosen or pasted: shrunk off the main thread, at most four to a message
    private func add(_ raw: [Data]) {
        guard client.canSendImages else { notice = "This hub does not take pictures yet: update it, then attach again."; return }
        notice = ""
        Task {
            let small = await Task.detached { raw.compactMap { DataClient.prepared($0) } }.value
            pending = Array((pending + small).prefix(4))
        }
    }
}

struct DataThinkingRow: View {
    var body: some View { HStack(spacing: 8) { ProgressView(); Text("Working…").foregroundStyle(.secondary) }.accessibilityIdentifier("data.thinking") }
}

struct DataEntryRow: View {
    let client: DataClient
    let thread: String
    let entry: DEntry

    var body: some View {
        switch entry.kind {
        case "user":
            HStack { Spacer(minLength: 48)
                VStack(alignment: .trailing, spacing: 3) {
                    ForEach(Array(entry.localImages.enumerated()), id: \.offset) { _, d in
                        if let u = UIImage(data: d) { Image(uiImage: u).resizable().scaledToFit().frame(maxWidth: 220, maxHeight: 220).clipShape(RoundedRectangle(cornerRadius: 10)).accessibilityIdentifier("data.entry.image") }
                    }
                    if entry.localImages.isEmpty { ForEach(entry.attachments) { a in DataAttachmentImage(client: client, attachment: a) } }
                    if !entry.text.isEmpty { Text(entry.text).padding(10).background(BackplaneDesign.accent.opacity(0.18), in: RoundedRectangle(cornerRadius: 12)) }
                    if entry.sending { Text("Sending…").font(.caption2).foregroundStyle(.secondary) }
                    if entry.failed { Button("Not sent · Retry") { client.retry(thread, entry) }.font(.caption).foregroundStyle(.red) }
                }
            }.accessibilityIdentifier("data.entry.user")
        case "assistant":
            VStack(alignment: .leading, spacing: 4) {
                Text(markdown(entry.text)).textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading)
                if entry.live { Text("▍").foregroundStyle(.secondary) }
            }.accessibilityIdentifier("data.entry.assistant")
        case "tool":
            HStack(spacing: 8) {
                if entry.tool?.state == "running" { ProgressView().controlSize(.small) }
                else { Image(systemName: entry.tool?.state == "failed" ? "xmark.circle" : "checkmark.circle").foregroundStyle(entry.tool?.state == "failed" ? .red : .secondary) }
                Text(entry.tool?.name ?? "Tool").font(.footnote.weight(.semibold))
                Text(entry.tool?.summary ?? entry.text).font(.footnote).foregroundStyle(.secondary).lineLimit(2)
            }.accessibilityIdentifier("data.entry.tool")
        case "ask":
            DataAskCard(client: client, thread: thread, entry: entry)
        case "error":
            Label(entry.text, systemImage: "exclamationmark.triangle").font(.footnote).foregroundStyle(.red)
        default:
            Text(entry.text).font(.footnote).foregroundStyle(.secondary).frame(maxWidth: .infinity, alignment: .leading)
        }
    }
    private func markdown(_ s: String) -> AttributedString {
        (try? AttributedString(markdown: s, options: .init(allowsExtendedAttributes: false, interpretedSyntax: .inlineOnlyPreservingWhitespace))) ?? AttributedString(s)
    }
}

struct DataAskCard: View {
    let client: DataClient
    let thread: String
    let entry: DEntry
    @State private var answer = ""
    var body: some View {
        let ask = entry.ask
        VStack(alignment: .leading, spacing: 10) {
            Label(ask?.question ?? entry.text, systemImage: "questionmark.bubble").font(.subheadline.weight(.semibold))
            if ask?.isOpen == true {
                if (ask?.options ?? []).isEmpty {
                    HStack {
                        TextField("Your answer", text: $answer).textFieldStyle(.roundedBorder)
                        Button("Send") { Task { await client.answer(thread, ask: ask!.id, option: answer) } }.disabled(answer.isEmpty)
                    }
                } else {
                    HStack { ForEach(ask!.options) { o in
                        Button(o.label) { Task { await client.answer(thread, ask: ask!.id, option: o.id) } }
                            .buttonStyle(.borderedProminent).tint(o.id == "deny" || o.id == "decline" ? .red : BackplaneDesign.accent)
                            .accessibilityIdentifier("data.ask." + o.id)
                    } }
                }
            } else if let a = ask?.answered { Text("Answered: \(a)").font(.footnote).foregroundStyle(.secondary) }
        }
        .padding(12).frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.orange.opacity(0.14), in: RoundedRectangle(cornerRadius: 10))
        .accessibilityIdentifier("data.entry.ask")
    }
}

// MARK: Improvements

struct DataImprovementsView: View {
    let client: DataClient
    @Environment(\.openURL) private var openURL
    @State private var text = ""
    @State private var project = ""
    @State private var sent = ""

    var body: some View {
        Form {
            Section {
                TextEditor(text: $text).frame(minHeight: 140).accessibilityIdentifier("improvements.text")
            } header: { Text("What would you like to improve?") } footer: {
                Text("A public GitHub issue draft on BackplaneWorks/Backplane opens in your browser, filled with this text. You review it and press Submit there; nothing is sent from here. Do not include private information.")
            }
            Section {
                Button("Open a GitHub issue draft") { openURL(issueURL) }
                    .disabled(text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty).accessibilityIdentifier("improvements.github")
            }
            if !client.projects.isEmpty {
                Section {
                    Picker("Project", selection: $project) { ForEach(client.projects) { Text($0.title).tag($0.id) } }
                    Button("Send to an agent thread") {
                        let p = project.isEmpty ? (client.projects.first?.id ?? "") : project
                        Task { if (try? await client.createThread(project: p, text: "Improvement idea for this app:\n\n" + text)) != nil { sent = "Thread started in Threads."; text = "" } }
                    }
                    .disabled(text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || client.conn != .online).accessibilityIdentifier("improvements.thread")
                    if !sent.isEmpty { Text(sent).font(.footnote).foregroundStyle(.secondary) }
                } footer: { Text("Starts a thread on your hub in the chosen project with this text.") }
            }
        }
        .navigationTitle("Improvements")
    }
    private var issueURL: URL {
        let info = Bundle.main.infoDictionary
        let v = "\(info?["CFBundleShortVersionString"] as? String ?? "?") (\(info?["CFBundleVersion"] as? String ?? "?"))"
        let title = String(text.split(separator: "\n").first ?? "Improvement").prefix(80)
        var c = URLComponents(string: "https://github.com/BackplaneWorks/Backplane/issues/new")!
        c.queryItems = [URLQueryItem(name: "title", value: String(title)),
                        URLQueryItem(name: "body", value: text + "\n\n---\nBackplane iOS \(v), iOS \(UIDevice.current.systemVersion)")]
        return c.url!
    }
}

// MARK: Settings: this phone's own settings first, then the hub

struct DataSettingsView: View {
    @Bindable var client: DataClient
    @Bindable private var settings = AppSettings.shared
    @State private var confirmUnpair = false

    var body: some View {
        Form {
            Section {
                Picker("Appearance", selection: $settings.appearance) {
                    Text("System").tag("system"); Text("Light").tag("light"); Text("Dark").tag("dark")
                }.pickerStyle(.segmented)
                Picker("Text size", selection: $settings.textSize) {
                    Text("System").tag("system")
                    ForEach(AppSettings.textSteps, id: \.tag) { Text($0.tag.uppercased()).tag($0.tag) }
                }.accessibilityIdentifier("settings.textsize")
                Picker("Open on", selection: $settings.defaultTab) {
                    Text("Threads").tag(""); Text("Improvements").tag("improvements"); Text("Settings").tag("settings")
                }
                Toggle("Alerts on this phone", isOn: $settings.alerts).accessibilityIdentifier("settings.alerts")
                    .onChange(of: settings.alerts) { _, on in if on { Task { await client.askNotificationPermission() } } }
                Toggle("Alert sound", isOn: $settings.alertSound).disabled(!settings.alerts)
                Toggle("Haptics", isOn: $settings.haptics)
            } header: { Text("On this phone") } footer: {
                Text("Stored on this phone only; never sent to a hub. Alerts tell you when a turn ends or asks while the app is still running; they are not push notifications.")
            }
            Section {
                LabeledContent("Hub", value: client.hubName.isEmpty ? "Not paired" : client.hubName)
                LabeledContent("Status", value: client.conn.rawValue.capitalized)
                if !client.failure.isEmpty, client.conn != .online { Text(client.failure).font(.caption).foregroundStyle(.secondary) }
                Button("Reconnect") { client.connect() }
                Button("Forget this hub", role: .destructive) { confirmUnpair = true }
            } header: { Text("Hub") } footer: { Text("Everything above this line is this phone's; these rows describe the hub it is paired with.") }
            Section("About") {
                let i = Bundle.main.infoDictionary
                LabeledContent("Version", value: "\(i?["CFBundleShortVersionString"] as? String ?? "?") (\(i?["CFBundleVersion"] as? String ?? "?"))")
            }
        }
        .navigationTitle("Settings")
        .confirmationDialog("Forget this hub?", isPresented: $confirmUnpair, titleVisibility: .visible) {
            Button("Forget", role: .destructive) { client.unpair() }
        } message: { Text("This phone's saved copy of its threads is removed. Pair again to come back.") }
    }
}

// a picture of a message: this phone's copy when it has one, else fetched once from the hub (attachment.get) and kept
struct DataAttachmentImage: View {
    let client: DataClient
    let attachment: DAttachment
    @State private var image: UIImage?
    @State private var failed = false
    @State private var full = false
    var body: some View {
        Group {
            if let image { Image(uiImage: image).resizable().scaledToFit().frame(maxWidth: 220, maxHeight: 220).clipShape(RoundedRectangle(cornerRadius: 10)).onTapGesture { full = true } }
            else if failed { Label(attachment.name, systemImage: "photo").font(.footnote).foregroundStyle(.secondary) }
            else { ProgressView().frame(width: 80, height: 80) }
        }
        .task(id: attachment.id) {
            if let d = await client.attachmentData(attachment.id), let u = UIImage(data: d) { image = u } else { failed = true }
        }
        .fullScreenCover(isPresented: $full) {
            ZStack(alignment: .topTrailing) {
                Color.black.ignoresSafeArea()
                if let image { Image(uiImage: image).resizable().scaledToFit() }
                Button { full = false } label: { Image(systemName: "xmark.circle.fill").font(.title).foregroundStyle(.white) }.padding()
            }
        }
        .accessibilityIdentifier("data.entry.image")
    }
}
