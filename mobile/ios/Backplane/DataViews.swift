import SwiftUI
import UIKit
import PhotosUI

// The phone app's screens for data/1 (docs/PHONE-DATA.md): native SwiftUI over typed values from DataClient, drawn as the desktop draws them (DataStyle.swift):
// one list of projects and threads, one thread with its runs folded and the composer under it, this phone's settings. The same screens as the Android app's DataViews.kt.

struct DPage: Hashable { var name: String }

struct DataRoot: View {
    @Bindable var client: DataClient
    @Bindable private var settings = AppSettings.shared
    @Environment(\.scenePhase) private var phase
    @State private var path = NavigationPath()

    var body: some View {
        Group {
            if client.links.isEmpty || client.needsRepair {
                NavigationStack {
                    PairView(tag: "en", link: "", status: client.pairingError, repair: client.needsRepair) { client.pair($0) }
                        .safeAreaInset(edge: .bottom) {
                            // refused or not, a paired phone can still look at the hub
                            if !client.links.isEmpty { NavigationLink { DataHubView(client: client) } label: { Text("Hub") }.buttonStyle(DButton()).padding(12).accessibilityIdentifier("repair.hub") }
                        }
                }
            } else {
                NavigationStack(path: $path) { DataThreadsView(client: client, path: $path) }
            }
        }
        .overlay { if let l = client.askLink { AskPairView(client: client, link: l) } }
        .tint(BackplaneDesign.accent)
        .preferredColorScheme(settings.colorScheme)
        .modifier(TextSizeModifier(size: settings.dynamicType))
        .task {
            if !client.links.isEmpty, client.conn == .offline { client.connect() }
            #if DEBUG
            // a check without hands: BACKPLANE_SELECT opens a thread, or a page by @name
            if let t = ProcessInfo.processInfo.environment["BACKPLANE_SELECT"], !t.isEmpty { if t.hasPrefix("@") { path.append(DPage(name: String(t.dropFirst()))) } else { path.append(t) } }
            // a link as if opened from outside, through the same route as onOpenURL: a simulator cannot answer iOS's own "Open in …?" before it
            if let s = ProcessInfo.processInfo.environment["BACKPLANE_OPEN_URL"], let u = URL(string: s) { try? await Task.sleep(for: .seconds(1)); route(u) }
            #endif
        }
        .onChange(of: phase) { _, p in if p == .active { client.foreground() } }
        .onChange(of: client.pendingThread) { _, id in if let id { path.append(id); client.pendingThread = nil } }
        .onOpenURL { route($0) }
        .onChange(of: client.pushThread, initial: true) { _, t in if !t.isEmpty { var p = NavigationPath(); p.append(t); path = p; client.pushThread = "" } }
        // a newly paired (or forgotten) hub starts again on the list
        .onChange(of: client.links.isEmpty) { _, _ in path = NavigationPath() }
    }

    /// This app's links, its one handler: backplane://thread/<id> opens that thread; any other (a pairing link) asks first (DataClient.opened).
    private func route(_ url: URL) {
        guard url.scheme == "backplane" || url.scheme == AppIdentity.scheme else { return }
        if url.host == "thread" { if let id = url.pathComponents.dropFirst().first { path.append(id) } } else { client.opened(url) }
    }
}

// MARK: threads

struct DataThreadsView: View {
    @Bindable var client: DataClient
    @Binding var path: NavigationPath
    @State private var addingProject = false
    @State private var newThread: String?       // a project id
    @State private var query = ""
    @State private var renaming: DThread?
    @State private var deleting: DThread?
    @State private var projectMenu: DProject?
    @State private var renamingProject: DProject?
    @State private var badgeProject: DProject?
    @State private var projectTitle = ""
    @State private var shut = Set(UserDefaults.standard.stringArray(forKey: "data.projects.shut") ?? [])
    @State private var settledOpen: Set<String> = []

    private var botThreads: Set<String> { Set(client.bots.map(\.thread)) }
    private func threads(_ p: DProject) -> [DThread] { let b = botThreads; return client.threads.filter { $0.project == p.id && !$0.archived && !b.contains($0.id) } }
    private func toggle(_ p: DProject) {
        if shut.contains(p.id) { shut.remove(p.id) } else { shut.insert(p.id) }
        UserDefaults.standard.set(Array(shut), forKey: "data.projects.shut")
    }

    var body: some View {
        VStack(spacing: 0) {
            DataHubBanner(client: client)
            if client.conn != .online { DataConnectionBanner(client: client) }
            HStack(spacing: 8) {
                if client.has("search") {
                    TextField("Search projects", text: $query).dmono(13).textInputAutocapitalization(.never).autocorrectionDisabled()
                        .padding(.horizontal, 10).frame(height: 36).dbox().accessibilityIdentifier("data.search")
                } else { Spacer() }
                Button { addingProject = true } label: { Image(systemName: "plus").frame(width: 36, height: 36).foregroundStyle(BackplaneDesign.dim) }
                    .accessibilityLabel("Add project").accessibilityIdentifier("data.project.add")
            }.padding(.horizontal, 10).padding(.vertical, 8)
            DLine()
            if client.has("search"), !query.trimmingCharacters(in: .whitespaces).isEmpty {
                DataSearchResults(client: client, query: query) { path.append($0) }
            } else { list }
        }
        .background(BackplaneDesign.panel)
        .navigationBarTitleDisplayMode(.inline)
        .toolbarBackground(BackplaneDesign.panel, for: .navigationBar).toolbarBackground(.visible, for: .navigationBar)
        .toolbar {
            ToolbarItem(placement: .topBarLeading) {
                HStack(spacing: 8) { BrandMark(size: 20, conn: client.conn == .online); Text("Backplane").dmono(14, bold: true).foregroundStyle(BackplaneDesign.text) }.fixedSize()
            }.dflat()
            ToolbarItem(placement: .topBarTrailing) {
                Button { path.append(DPage(name: "settings")) } label: { Image(systemName: "gearshape").foregroundStyle(BackplaneDesign.dim) }
                    .accessibilityLabel("Settings").accessibilityIdentifier("data.settings")
            }.dflat()
        }
        .navigationDestination(for: String.self) { DataThreadView(client: client, id: $0) }
        .navigationDestination(for: DRoom.self) { DataRoomView(client: client, room: $0) }
        .navigationDestination(for: DViewerRoute.self) { r in if let t = client.threads.first(where: { $0.id == r.id }) { DataViewerView(client: client, thread: t) } }
        .navigationDestination(for: DPage.self) { p in if p.name == "settings" { DataSettingsView(client: client) } else if p.name == "hub" { DataHubView(client: client) } else { DataImprovementsView(client: client) } }
        .modifier(DataThreadDialogs(client: client, renaming: $renaming, deleting: $deleting))
        .alert("Rename project", isPresented: Binding(get: { renamingProject != nil }, set: { if !$0 { renamingProject = nil } })) {
            TextField("Title", text: $projectTitle)
            Button("Rename") { if let p = renamingProject { Task { await client.projectAct(p.id, "rename", title: projectTitle) } } }
            Button("Cancel", role: .cancel) {}
        }
        .confirmationDialog("Remove “\(projectMenu?.title ?? "")” and its threads from the list?", isPresented: Binding(get: { projectMenu != nil }, set: { if !$0 { projectMenu = nil } }), titleVisibility: .visible) {
            Button("Remove project", role: .destructive) { if let p = projectMenu { Task { await client.projectAct(p.id, "remove") } } }
            Button("Keep it", role: .cancel) {}
        }
        .sheet(isPresented: $addingProject) { DataNewProjectSheet(client: client) }
        .sheet(item: $badgeProject) { DataBadgeSheet(client: client, project: $0) }
        .sheet(item: Binding(get: { newThread.map { Wrapped(id: $0) } }, set: { newThread = $0?.id })) { w in
            DataNewThreadSheet(client: client, project: w.id) { path.append($0) }
        }
    }

    private var list: some View {
        List {
            if client.projects.isEmpty && client.listsLoaded {
                VStack(alignment: .leading, spacing: 10) {
                    Text("No projects yet").dmono(14, bold: true)
                    Text("A project is a folder on your hub's computer. Add one to start a thread.").dmono(12).foregroundStyle(BackplaneDesign.dim)
                    Button("Add a project") { addingProject = true }.buttonStyle(DButton(primary: true)).accessibilityIdentifier("data.project.add.empty")
                }.padding(14).dataRow()
            }
            ForEach(client.bots.filter { $0.uber && !$0.thread.isEmpty }) { b in botRow(b) }
            ForEach(client.projects) { p in
                projectHeader(p)
                if !shut.contains(p.id) {
                    let all = threads(p)
                    let active = all.filter { !$0.settled }.sorted { $0.pinned && !$1.pinned }
                    let settled = all.filter(\.settled)
                    ForEach(active) { threadRow($0) }
                    if all.isEmpty { Text("No threads").dmono(12).foregroundStyle(BackplaneDesign.faint).padding(.leading, 34).frame(height: 32).dataRow() }
                    if !settled.isEmpty {
                        let open = settledOpen.contains(p.id)
                        Button { if open { settledOpen.remove(p.id) } else { settledOpen.insert(p.id) } } label: {
                            HStack(spacing: 6) { Image(systemName: open ? "chevron.down" : "chevron.right").font(.system(size: 10)); Text("Settled \(settled.count)").dmono(11); Spacer() }
                                .foregroundStyle(BackplaneDesign.faint).padding(.leading, 30).frame(height: 32).contentShape(Rectangle())
                        }.buttonStyle(.plain).dataRow().accessibilityIdentifier("data.settled." + p.id)
                        if open { ForEach(settled) { threadRow($0) } }
                    }
                }
            }
            let bots = client.bots.filter { !$0.uber && !$0.thread.isEmpty }
            if !bots.isEmpty { DHeading(text: "Bots").padding(.leading, 12).padding(.top, 12).padding(.bottom, 4).dataRow(); ForEach(bots) { botRow($0) } }
            if !client.rooms.isEmpty {
                DHeading(text: "Rooms").padding(.leading, 12).padding(.top, 12).padding(.bottom, 4).dataRow()
                ForEach(client.rooms) { r in
                    Button { path.append(r) } label: { sideRow("#", r.name, "room") }.buttonStyle(.plain).dataRow().accessibilityIdentifier("data.room." + r.id)
                }
            }
        }
        .listStyle(.plain).scrollContentBackground(.hidden).environment(\.defaultMinListRowHeight, 1)
        .refreshable { await client.refresh() }
    }

    private func sideRow(_ sigil: String, _ name: String, _ detail: String) -> some View {
        HStack(spacing: 8) {
            Text(sigil).dmono(13, bold: true).foregroundStyle(BackplaneDesign.accent).frame(width: 16)
            Text(name).dmono(13).foregroundStyle(BackplaneDesign.text).lineLimit(1); Spacer()
            Text(detail).dmono(11).foregroundStyle(BackplaneDesign.faint)
        }.padding(.horizontal, 12).frame(minHeight: 40).contentShape(Rectangle())
    }
    private func botRow(_ b: DBot) -> some View {
        Button { path.append(b.thread) } label: { sideRow("@", b.name, b.uber ? "uberbot" : "bot") }.buttonStyle(.plain).dataRow().accessibilityIdentifier("data.bot." + b.id)
    }
    private func projectHeader(_ p: DProject) -> some View {
        HStack(spacing: 8) {
            Button { toggle(p) } label: {
                HStack(spacing: 8) {
                    Image(systemName: shut.contains(p.id) ? "chevron.right" : "chevron.down").font(.system(size: 11)).foregroundStyle(BackplaneDesign.dim).frame(width: 14)
                    Image(systemName: dataBadgeSymbol(p.icon)).font(.system(size: 13)).foregroundStyle(Color(css: p.css) ?? BackplaneDesign.ok)
                    Text(p.title.uppercased()).dmono(12).foregroundStyle(BackplaneDesign.dim).lineLimit(1)
                    if p.waiting > 0 { Text("\(p.waiting)").dmono(11, bold: true).foregroundStyle(BackplaneDesign.bad) }
                    Spacer(minLength: 0)
                }.frame(height: 40).contentShape(Rectangle())
            }.buttonStyle(.plain).accessibilityIdentifier("data.project." + p.id)
            Button { newThread = p.id } label: { Image(systemName: "plus").font(.system(size: 14)).foregroundStyle(BackplaneDesign.dim).frame(width: 40, height: 40).contentShape(Rectangle()) }
                .buttonStyle(.plain).accessibilityLabel("New thread in \(p.title)").accessibilityIdentifier("data.thread.new." + p.id)
        }
        .padding(.leading, 10).padding(.trailing, 2).dataRow()
        .contextMenu { if client.has("actions") { Button("Rename", systemImage: "pencil") { projectTitle = p.title; renamingProject = p }; Button("Badge", systemImage: "paintpalette") { badgeProject = p }; Button("Remove project", systemImage: "trash", role: .destructive) { projectMenu = p } } }
    }
    private func threadRow(_ t: DThread) -> some View {
        Button { path.append(t.id) } label: { DataThreadRow(thread: t) }
            .buttonStyle(.plain).dataRow()
            .contextMenu { if client.has("actions") { DataThreadActions(client: client, thread: t, renaming: $renaming, deleting: $deleting) } }
            .swipeActions(edge: .trailing, allowsFullSwipe: false) {
                if client.has("actions") {
                    Button(t.settled ? "Unsettle" : "Settle") { Task { await client.threadAct(t.id, t.settled ? "unsettle" : "settle") } }.tint(BackplaneDesign.ok)
                    Button("Archive") { Task { await client.threadAct(t.id, "archive") } }.tint(BackplaneDesign.faint)
                }
            }
            .onAppear { client.prefetch(t.id) }
            .accessibilityIdentifier("data.thread.row." + t.id)
    }
    struct Wrapped: Identifiable { var id: String }
}

struct DataConnectionBanner: View {
    let client: DataClient
    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 8) {
                DDot(color: client.conn == .connecting ? BackplaneDesign.warn : BackplaneDesign.bad, size: 8)
                VStack(alignment: .leading, spacing: 2) {
                    Text(client.conn == .connecting ? "Connecting to \(client.hubName)…" : "Offline · showing what this phone has saved").dmono(12)
                    if client.conn == .offline, !client.failure.isEmpty { Text(client.failure).dmono(11).foregroundStyle(BackplaneDesign.dim).lineLimit(2) }
                }
                Spacer()
                NavigationLink(value: DPage(name: "hub")) { Text("Hub") }.dmono(12, bold: true).foregroundStyle(BackplaneDesign.accent).accessibilityIdentifier("conn.hub")
                if client.conn == .offline { Button("Retry") { client.connect() }.dmono(12, bold: true).foregroundStyle(BackplaneDesign.accent) }
            }.padding(.horizontal, 12).padding(.vertical, 8).background(BackplaneDesign.raised)
            DLine()
        }.accessibilityIdentifier("data.connection.banner")
    }
}

struct DataThreadRow: View {
    let thread: DThread
    var body: some View {
        HStack(alignment: .top, spacing: 8) {
            DDot(color: dStateColor(thread.state)).padding(.top, 6).frame(width: 14)
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 6) {
                    if thread.pinned { Image(systemName: "pin.fill").font(.system(size: 9)).foregroundStyle(BackplaneDesign.accent) }
                    Text(thread.title.isEmpty ? "New thread" : thread.title).dmono(13).foregroundStyle(BackplaneDesign.text).lineLimit(1)
                }
                if !thread.preview.isEmpty { Text(thread.preview.replacingOccurrences(of: "**", with: "").replacingOccurrences(of: "`", with: "")).dmono(11).foregroundStyle(BackplaneDesign.faint).lineLimit(1) }
            }
            Spacer(minLength: 0)
            if thread.waiting > 0 { Text("\(thread.waiting)").dmono(11, bold: true).foregroundStyle(BackplaneDesign.bad) }
            Text(dAgo(thread.updatedAt)).dmono(11).foregroundStyle(BackplaneDesign.faint)
        }
        .padding(.leading, 26).padding(.trailing, 12).padding(.vertical, 8).opacity(thread.settled ? 0.55 : 1).contentShape(Rectangle())
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

    private func row(_ name: String, _ icon: String, _ to: String) -> some View {
        Button { Task { await go(to) } } label: {
            HStack(spacing: 10) { Image(systemName: icon).font(.system(size: 13)).foregroundStyle(BackplaneDesign.dim).frame(width: 18); Text(name).dmono(13).foregroundStyle(BackplaneDesign.text).lineLimit(1); Spacer() }
                .padding(.horizontal, 14).frame(minHeight: 40).contentShape(Rectangle())
        }.buttonStyle(.plain).dataRow()
    }
    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 0) {
                TextField("Folder on the hub's computer", text: $path).dmono(13).textInputAutocapitalization(.never).autocorrectionDisabled()
                    .padding(.horizontal, 10).frame(height: 40).dbox().padding(12).accessibilityIdentifier("data.project.path")
                if !error.isEmpty { Text(error).dmono(12).foregroundStyle(BackplaneDesign.bad).padding(.horizontal, 12).padding(.bottom, 8) }
                DLine()
                List {
                    if let d = dirs {
                        if d.parent != d.path { row("..", "arrow.up.left", d.parent) }
                        ForEach(d.dirs, id: \.self) { n in row(n, "folder", d.path.hasSuffix("/") ? d.path + n : d.path + "/" + n) }
                    }
                }.listStyle(.plain).scrollContentBackground(.hidden).environment(\.defaultMinListRowHeight, 1)
            }
            .background(BackplaneDesign.panel)
            .dataBar("Add project", back: false)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() }.dmono(13) }.dflat()
                ToolbarItem(placement: .confirmationAction) {
                    Button("Add") { Task { await add() } }.dmono(13, bold: true).disabled(busy || path.trimmingCharacters(in: .whitespaces).isEmpty).accessibilityIdentifier("data.project.add.confirm")
                }.dflat()
            }
            .task { await go(nil) }
        }.dataSheet()
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
    let started: (String) -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var text = ""
    @State private var error = ""
    @State private var busy = false
    @State private var choice = DNewThreadChoice()
    @State private var chip = ""
    @State private var picking = false

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text(client.projects.first { $0.id == project }?.title ?? "New thread").dmono(14, bold: true).lineLimit(1); Spacer()
                Button("Cancel") { dismiss() }.dmono(13).foregroundStyle(BackplaneDesign.dim)
            }
            ZStack(alignment: .topLeading) {
                TextEditor(text: $text).dmono(13).scrollContentBackground(.hidden).padding(6).accessibilityIdentifier("data.thread.new.text")
                if text.isEmpty { Text("What should it do?").dmono(13).foregroundStyle(BackplaneDesign.faint).padding(.horizontal, 11).padding(.vertical, 14).allowsHitTesting(false) }
            }.frame(minHeight: 120).dbox(BackplaneDesign.accent)
            if !error.isEmpty { Text(error).dmono(12).foregroundStyle(BackplaneDesign.bad) }
            HStack(spacing: 6) {
                if client.has("picker") {
                    Button { picking = true } label: { Text(chip.isEmpty ? " " : chip).dmono(12).foregroundStyle(BackplaneDesign.text).lineLimit(1).padding(.horizontal, 10).frame(maxWidth: .infinity, minHeight: 40, alignment: .leading).dbox(fill: BackplaneDesign.panel) }
                        .buttonStyle(.plain).accessibilityLabel("Model: " + chip).accessibilityIdentifier("data.thread.new.model")
                } else { Spacer() }
                Button(busy ? "Starting…" : "Start") { Task { await start() } }.buttonStyle(DButton(primary: true)).disabled(busy).accessibilityIdentifier("data.thread.new.start")
            }
            Spacer(minLength: 0)
        }
        .padding(14).background(BackplaneDesign.panel)
        .presentationDetents([.medium, .large]).dataSheet()
        .sheet(isPresented: $picking) { DataPickerSheet(client: client, thread: nil, choice: $choice) }
        .task(id: choice) { if client.has("picker") { chip = (try? await client.picker(thread: nil, choice))?.chip ?? chip } }
    }
    private func start() async {
        busy = true; defer { busy = false }
        do {
            let t = try await client.createThread(project: project, text: text, choice: choice)
            dismiss(); started(t.id)
        } catch { self.error = error.localizedDescription }
    }
}

// MARK: one thread

// The list's one bottom anchor (it opens at the end and keeps the end in view as rows grow), and on iOS 18 whether the reader is at the end.
private struct DStick: ViewModifier {
    @Binding var atEnd: Bool
    func body(content: Content) -> some View {
        if #available(iOS 18, *) {
            content.defaultScrollAnchor(.bottom)
                .onScrollGeometryChange(for: Bool.self) { g in g.contentOffset.y + g.containerSize.height >= g.contentSize.height - 80 } action: { _, v in if atEnd != v { atEnd = v } }
        } else { content.defaultScrollAnchor(.bottom) }
    }
}

struct DataThreadView: View {
    @Bindable var client: DataClient
    let id: String
    @State private var text = ""
    @State private var pending: [Data] = []
    @State private var picked: [PhotosPickerItem] = []
    @State private var drawing = false
    @State private var notice = ""
    @State private var renaming: DThread?
    @State private var deleting: DThread?
    @State private var modelSheet = false
    @State private var noChoice = DNewThreadChoice()
    @State private var openRuns: Set<String> = []
    @State private var atEnd = true
    @State private var order: DOrder?
    @State private var caret: (at: Int, rev: Int)?
    @Environment(\.dismiss) private var dismiss

    init(client: DataClient, id: String) { self.client = client; self.id = id; client.touch(id) }

    private var thread: DThread? { client.threads.first { $0.id == id } }
    private var list: [DEntry] { client.entries[id] ?? [] }
    private var working: Bool { thread?.state == "working" || thread?.state == "asking" }
    private var chip: String { guard let t = thread else { return "" }; return t.chip.isEmpty ? "\(t.provider) / \(t.model.isEmpty ? "default" : t.model)" : t.chip }

    var body: some View {
        VStack(spacing: 0) {
            DataHubBanner(client: client)
            if client.conn != .online { DataConnectionBanner(client: client) }
            stateStrip
            ScrollViewReader { proxy in
                ScrollView {
                    // A VStack, not a LazyVStack: once a message with a picture and words was sent, the lazy stack's placement never settled
                    // (the main thread at 100% in LazySubviewPlacements, no view of ours re-run, the screen frozen). A page is 40 entries with
                    // tool runs folded, so the rows are few; older pages come only when asked, by the row below, never by a row appearing.
                    VStack(alignment: .leading, spacing: 8) {
                        // a tap, as on Android: in a VStack every row appears at once, so an onAppear here would pull the whole history page by page
                        if client.older[id] != nil {
                            Button { client.loadOlder(id) } label: { Text("load earlier").dmono(12).foregroundStyle(BackplaneDesign.dim).frame(maxWidth: .infinity, minHeight: 36) }
                                .buttonStyle(.plain).accessibilityIdentifier("data.thread.older")
                        }
                        ForEach(DataFold.rows(list, running: working)) { r in
                            switch r {
                            case .entry(let e): DataEntryRow(client: client, thread: id, entry: e).id(e.id)
                            case .fold(let f): DataFoldRow(fold: f, open: Binding(get: { openRuns.contains(f.id) }, set: { if $0 { openRuns.insert(f.id) } else { openRuns.remove(f.id) } })).id("fold:" + f.id)
                            }
                        }
                        if list.isEmpty && client.loadedThreads.contains(id) { Text("Start a conversation.").dmono(13).foregroundStyle(BackplaneDesign.dim).padding(.top, 40).frame(maxWidth: .infinity, alignment: .leading) }
                        if thread?.state == "working" { DWorkingRow() }
                        Color.clear.frame(height: 1).id("end")
                    }
                    .padding(.horizontal, 14).padding(.top, 10).padding(.bottom, 6)
                }
                .modifier(DStick(atEnd: $atEnd))
                .scrollDismissesKeyboard(.interactively)
                .onChange(of: list.last?.id) { _, _ in if atEnd { proxy.scrollTo("end", anchor: .bottom) } }
                .onChange(of: list.count) { _, _ in if client.pendingAsk.isEmpty, atEnd { proxy.scrollTo("end", anchor: .bottom) }; toAsk(proxy) }
                .onAppear { toAsk(proxy) }
                .onChange(of: list.last?.text.count) { _, _ in if atEnd { proxy.scrollTo("end", anchor: .bottom) } }
            }
            if let order { DataOrderCard(order: order) }
            DataMentionBar(client: client, text: text) { text = $0; client.setDraft(id, $0) }
            DVoiceStrip(client: client, thread: id, draft: text)
            composer
        }
        .onChange(of: client.voice.caret.rev) { _, rev in
            // the hub's dictation wrote this thread's draft: show it and put the caret where it said
            let c = client.voice.caret
            guard c.thread == id else { return }
            let new = client.drafts[id] ?? ""
            text = new
            caret = (VoiceEdit.utf16(new, caret: c.at), rev)
        }
        .task(id: "\(list.count)|\(thread?.state ?? "")|\(client.conn == .online)") { if client.has("order"), let o = try? await client.order(id) { order = o } else if !client.has("order") { order = nil } }
        .background(BackplaneDesign.background)
        .dataBar(thread?.title.isEmpty == false ? thread!.title : "Thread")
        .toolbar {
            if client.has("viewers") {
                ToolbarItem(placement: .topBarTrailing) {
                    NavigationLink(value: DViewerRoute(id: id)) { Image(systemName: "square.grid.2x2").foregroundStyle(BackplaneDesign.dim) }.accessibilityLabel("Viewer").accessibilityIdentifier("data.thread.viewer")
                }.dflat()
            }
            if let t = thread, client.has("actions") {
                ToolbarItem(placement: .topBarTrailing) {
                    Menu { DataThreadActions(client: client, thread: t, renaming: $renaming, deleting: $deleting) } label: { Image(systemName: "ellipsis").foregroundStyle(BackplaneDesign.dim) }
                        .accessibilityLabel("Thread actions").accessibilityIdentifier("data.thread.more")
                }.dflat()
            }
        }
        .modifier(DataThreadDialogs(client: client, renaming: $renaming, deleting: $deleting, deleted: { dismiss() }))
        .sheet(isPresented: $modelSheet) { DataPickerSheet(client: client, thread: id, choice: $noChoice) }
        // full screen, not a sheet: a pen stroke downward must not swipe it away
        .fullScreenCover(isPresented: $drawing) { DataPadView { add([$0]) } }
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
            #if DEBUG
            // BACKPLANE_SHOW: picker, fold (either, comma-separated)
            let show = ProcessInfo.processInfo.environment["BACKPLANE_SHOW"] ?? ""
            if show.contains("picker") { modelSheet = true }
            if show.contains("fold") { DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) { for r in DataFold.rows(client.entries[id] ?? [], running: false) { if case .fold(let f) = r { openRuns.insert(f.id) } } } }
            #endif
        }
        .onDisappear { client.closeThread(id) }
    }

    // the desktop's strip under the bar: STATE, then the tools a thread always has
    private var stateStrip: some View {
        VStack(spacing: 0) {
            HStack(spacing: 6) {
                DHeading(text: "State")
                if let t = thread { DDot(color: dStateColor(t.state)); Text(dStateWord(t.state)).dmono(11).foregroundStyle(BackplaneDesign.text).accessibilityIdentifier("data.thread.state") }
                Spacer()
                if let t = thread, client.has("actions") {
                    strip(t.pinned ? "Unpin" : "Pin", on: t.pinned) { Task { await client.threadAct(id, t.pinned ? "unpin" : "pin") } }
                    strip(t.settled ? "Unsettle" : "Settle", on: t.settled) { Task { await client.threadAct(id, t.settled ? "unsettle" : "settle") } }
                }
            }.padding(.leading, 14).frame(height: 32).background(BackplaneDesign.panel)
            DLine()
        }
    }
    private func strip(_ label: String, on: Bool, _ run: @escaping () -> Void) -> some View {
        Button(action: run) { Text(label).dmono(11).foregroundStyle(on ? BackplaneDesign.accent : BackplaneDesign.dim).padding(.horizontal, 10).frame(maxHeight: .infinity).overlay(alignment: .leading) { Rectangle().fill(BackplaneDesign.line).frame(width: 1) }.contentShape(Rectangle()) }.buttonStyle(.plain)
    }

    // the desktop's composer: the box in accent, under it the model chip, attach and Send
    private var composer: some View {
        VStack(spacing: 0) {
            DLine()
            VStack(alignment: .leading, spacing: 8) {
                if !notice.isEmpty { Text(notice).dmono(12).foregroundStyle(BackplaneDesign.warn).accessibilityIdentifier("data.images.notice") }
                if !pending.isEmpty {
                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack(spacing: 8) {
                            ForEach(Array(pending.enumerated()), id: \.offset) { i, d in
                                ZStack(alignment: .topTrailing) {
                                    if let u = UIImage(data: d) { Image(uiImage: u).resizable().scaledToFill().frame(width: 64, height: 64).clipped().overlay(Rectangle().strokeBorder(BackplaneDesign.line, lineWidth: 1)) }
                                    Button { pending.remove(at: i) } label: { Image(systemName: "xmark").font(.system(size: 10, weight: .bold)).foregroundStyle(BackplaneDesign.text).frame(width: 20, height: 20).background(BackplaneDesign.raised) }
                                        .accessibilityLabel("Remove picture")
                                }
                            }
                        }
                    }.accessibilityIdentifier("data.pending.images")
                }
                UIKitField(text: text, placeholder: "Ask anything: @bot, >thread, %project, #room, &model, $skill, /btw", editable: true,
                           onChange: { text = $0; client.setDraft(id, $0) },
                           onSend: { send() }, onPasteImages: { imgs in add(imgs.compactMap { $0.jpegData(compressionQuality: 0.95) }) },
                           font: UIFontMetrics(forTextStyle: .body).scaledFont(for: Mono.uiFont(size: 14)), caret: caret)
                    .padding(.horizontal, 10).padding(.vertical, 9).frame(minHeight: 44, alignment: .topLeading).dbox(BackplaneDesign.accent)
                    .accessibilityLabel("Message")
                HStack(spacing: 6) {
                    if client.has("picker") || thread?.chip.isEmpty == false {
                        Button { modelSheet = true } label: { Text(chip).dmono(12).foregroundStyle(BackplaneDesign.text).lineLimit(1).padding(.horizontal, 10).frame(maxWidth: .infinity, minHeight: 40, alignment: .leading).dbox(fill: BackplaneDesign.panel) }
                            .buttonStyle(.plain).accessibilityLabel("Model: " + chip).accessibilityIdentifier("data.thread.model")
                    } else { Spacer(minLength: 0) }
                    if client.has("voice") {
                        let on = client.voice.listening(id)
                        Button { micPress() } label: {
                            Image(systemName: on ? "mic.fill" : "mic").font(.system(size: 15)).foregroundStyle(on ? BackplaneDesign.bad : BackplaneDesign.dim).frame(width: 40, height: 40).dbox(fill: BackplaneDesign.panel)
                        }
                        .buttonStyle(.plain).accessibilityLabel(on ? "Stop dictation" : "Dictate").accessibilityIdentifier("data.mic")
                    }
                    PhotosPicker(selection: $picked, maxSelectionCount: max(0, 4 - pending.count), matching: .images) {
                        Image(systemName: "paperclip").font(.system(size: 15)).foregroundStyle(BackplaneDesign.dim).frame(width: 40, height: 40).dbox(fill: BackplaneDesign.panel)
                    }
                    .disabled(!client.canSendImages)
                    .accessibilityLabel(client.canSendImages ? "Attach pictures" : "Attach pictures (this hub does not take pictures yet)").accessibilityIdentifier("data.attach")
                    Button { if client.canSendImages { drawing = true } else { notice = "This hub does not take pictures yet: update it, then draw again." } } label: {
                        Image(systemName: "pencil.tip").font(.system(size: 15)).foregroundStyle(client.canSendImages ? BackplaneDesign.dim : BackplaneDesign.faint).frame(width: 40, height: 40).dbox(fill: BackplaneDesign.panel)
                    }
                    .disabled(pending.count >= 4)
                    .accessibilityLabel("Draw").accessibilityIdentifier("data.draw")
                    let blank = text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && pending.isEmpty
                    if working && blank {
                        Button("Stop") { Task { await client.interrupt(id) } }.buttonStyle(DButton(danger: true)).accessibilityLabel("Stop").accessibilityIdentifier("thread.send")
                    } else {
                        Button("Send") { send() }.buttonStyle(DButton(primary: true)).disabled(blank).accessibilityIdentifier("thread.send")
                    }
                }
            }
            .padding(.horizontal, 12).padding(.vertical, 8)
        }
        .background(BackplaneDesign.background)
    }
    private func micPress() {
        let at = ComposerHandle.view.map { VoiceEdit.chars(text, utf16: $0.selectedRange.location) } ?? VoiceEdit.count(text)
        client.voice.press(id, draft: text, caret: at)
    }
    // a tapped notification lands on its approval, once it is loaded
    private func toAsk(_ proxy: ScrollViewProxy) {
        let e = "ask:" + client.pendingAsk
        guard !client.pendingAsk.isEmpty, list.contains(where: { $0.id == e }) else { return }
        client.pendingAsk = ""
        DispatchQueue.main.async { proxy.scrollTo(e, anchor: .center) }
    }
    private func send() {
        let t = text
        guard !t.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || !pending.isEmpty else { return }
        let images = pending
        text = ""; pending = []; atEnd = true
        client.send(id, t, images: images)
        client.voice.sent(id, draft: "")
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

/// A run of tool calls as one line (the desktop's fold row): a tap opens it, each call a line of its own.
struct DataFoldRow: View {
    let fold: DFold
    @Binding var open: Bool
    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Button { open.toggle() } label: {
                HStack(spacing: 8) {
                    Image(systemName: open ? "chevron.down" : "chevron.right").font(.system(size: 11)).foregroundStyle(BackplaneDesign.dim).frame(width: 12)
                    Text(fold.label).dmono(12).foregroundStyle(BackplaneDesign.dim).lineLimit(1)
                    Spacer(minLength: 0)
                }.padding(.horizontal, 8).frame(minHeight: 36).overlay(Rectangle().strokeBorder(BackplaneDesign.line, lineWidth: 1)).contentShape(Rectangle())
            }
            .buttonStyle(.plain).accessibilityLabel(fold.label).accessibilityValue(open ? "open" : "shut").accessibilityIdentifier("data.fold." + fold.id)
            if open { ForEach(fold.items) { DataToolRow(entry: $0) }.padding(.leading, 8) }
        }.accessibilityIdentifier("data.entry.tool")
    }
}

struct DataToolRow: View {
    let entry: DEntry
    @State private var whole = false
    private var name: String { entry.kind == "step" ? "" : entry.tool?.name ?? "tool" }
    private var what: String { entry.kind == "step" ? (entry.label.isEmpty ? "Design change" : entry.label) : (entry.tool?.summary ?? "").isEmpty ? "" : entry.text }
    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            if !name.isEmpty { Text(name).dmono(12, bold: true).foregroundStyle(BackplaneDesign.text) }
            Text(what).dmono(12).foregroundStyle(BackplaneDesign.dim).lineLimit(whole ? nil : 3).textSelection(.enabled)
            Spacer(minLength: 0)
        }.padding(.vertical, 3).contentShape(Rectangle()).onTapGesture { whole.toggle() }
    }
}

private let dClock: DateFormatter = { let f = DateFormatter(); f.dateFormat = "HH:mm"; return f }()
private let dDay: DateFormatter = { let f = DateFormatter(); f.dateFormat = "EEE d MMM HH:mm"; return f }()
/// A message's time: the clock today, the day too on another.
func dStamp(_ ms: Int) -> String {
    guard ms > 0 else { return "" }
    let d = Date(timeIntervalSince1970: Double(ms) / 1000)
    return (Calendar.current.isDateInToday(d) ? dClock : dDay).string(from: d)
}

struct DataEntryRow: View {
    let client: DataClient
    let thread: String
    let entry: DEntry

    var body: some View {
        switch entry.kind {
        case "user":
            HStack { Spacer(minLength: 32)
                VStack(alignment: .trailing, spacing: 3) {
                    VStack(alignment: .leading, spacing: 6) {
                        ForEach(Array(entry.localImages.enumerated()), id: \.offset) { _, d in
                            if let u = UIImage(data: d) { Image(uiImage: u).resizable().scaledToFit().frame(maxWidth: 220, maxHeight: 220).accessibilityIdentifier("data.entry.image") }
                        }
                        if entry.localImages.isEmpty { ForEach(entry.attachments) { a in DataAttachmentImage(client: client, attachment: a) } }
                        if !entry.text.isEmpty { Text(entry.text).dmono(13).foregroundStyle(BackplaneDesign.text).lineSpacing(3).textSelection(.enabled) }
                    }.padding(.horizontal, 10).padding(.vertical, 8).dbox(fill: BackplaneDesign.raised)
                    if entry.sending { Text("Sending…").dmono(11).foregroundStyle(BackplaneDesign.faint) }
                    else if entry.failed { Button("Not sent · Retry") { client.retry(thread, entry) }.dmono(11).foregroundStyle(BackplaneDesign.bad) }
                    else { Text(dStamp(entry.at)).dmono(11).foregroundStyle(BackplaneDesign.faint) }
                }
            }.padding(.vertical, 4).accessibilityIdentifier("data.entry.user")
        case "assistant":
            VStack(alignment: .leading, spacing: 4) {
                DataMarkdown(text: entry.text)
                if !entry.live { Text(dStamp(entry.at)).dmono(11).foregroundStyle(BackplaneDesign.faint) }
            }.padding(.vertical, 4).frame(maxWidth: .infinity, alignment: .leading).accessibilityIdentifier("data.entry.assistant")
        case "tool", "step":
            DataToolRow(entry: entry).accessibilityIdentifier("data.entry.tool")
        case "ask":
            DataAskCard(client: client, thread: thread, entry: entry)
        default:
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                if !entry.label.isEmpty { Text(entry.label).dmono(12, bold: true) }
                Text(entry.text).dmono(12).textSelection(.enabled)
                Spacer(minLength: 0)
            }.foregroundStyle(entry.tone == "error" ? BackplaneDesign.bad : BackplaneDesign.dim).padding(.vertical, 3)
        }
    }
}

struct DataAskCard: View {
    let client: DataClient
    let thread: String
    let entry: DEntry
    @State private var answer = ""
    var body: some View {
        let ask = entry.ask
        VStack(alignment: .leading, spacing: 8) {
            DHeading(text: ask?.isOpen == true ? "Asks" : "Asked")
            Text(ask?.question ?? entry.text).dmono(13).foregroundStyle(BackplaneDesign.text).textSelection(.enabled)
            if ask?.isOpen == true {
                if (ask?.options ?? []).isEmpty {
                    HStack(spacing: 6) {
                        TextField("Your answer", text: $answer).dmono(13).padding(.horizontal, 10).frame(height: 40).dbox()
                        Button("Send") { Task { await client.answer(thread, ask: ask!.id, option: answer) } }.buttonStyle(DButton(primary: true)).disabled(answer.isEmpty)
                    }
                } else {
                    VStack(alignment: .leading, spacing: 6) { ForEach(ask!.options) { o in
                        Button { Task { await client.answer(thread, ask: ask!.id, option: o.id) } } label: { Text(o.label).frame(maxWidth: .infinity, alignment: .leading) }
                            .buttonStyle(DButton(primary: o.id == "accept", danger: o.id == "deny" || o.id == "decline"))
                            .accessibilityIdentifier("data.ask." + o.id)
                    } }
                }
            } else if let a = ask?.answered { Text("Answered: \(a)").dmono(12).foregroundStyle(BackplaneDesign.dim) }
        }
        .padding(12).frame(maxWidth: .infinity, alignment: .leading)
        .dbox(ask?.isOpen == true ? BackplaneDesign.bad : BackplaneDesign.line, fill: BackplaneDesign.panel)
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
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                DHeading(text: "What would you like to improve?")
                TextEditor(text: $text).dmono(13).scrollContentBackground(.hidden).padding(6).frame(minHeight: 140).dbox(BackplaneDesign.accent).accessibilityIdentifier("improvements.text")
                Text("A public GitHub issue draft on BackplaneWorks/Backplane opens in your browser, filled with this text. You review it and press Submit there; nothing is sent from here. Do not include private information.").dmono(11).foregroundStyle(BackplaneDesign.dim)
                Button("Open a GitHub issue draft") { openURL(issueURL) }.buttonStyle(DButton())
                    .disabled(text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty).accessibilityIdentifier("improvements.github")
                if !client.projects.isEmpty {
                    DLine()
                    DHeading(text: "Project")
                    DChoices(options: client.projects.map { (label: $0.title, value: $0.id) }, value: Binding(get: { project.isEmpty ? (client.projects.first?.id ?? "") : project }, set: { project = $0 }))
                    Button("Send to an agent thread") {
                        let p = project.isEmpty ? (client.projects.first?.id ?? "") : project
                        Task { if (try? await client.createThread(project: p, text: "Improvement idea for this app:\n\n" + text)) != nil { sent = "Thread started in Threads."; text = "" } }
                    }
                    .buttonStyle(DButton(primary: true))
                    .disabled(text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || client.conn != .online).accessibilityIdentifier("improvements.thread")
                    if !sent.isEmpty { Text(sent).dmono(12).foregroundStyle(BackplaneDesign.dim) }
                    Text("Starts a thread on your hub in the chosen project with this text.").dmono(11).foregroundStyle(BackplaneDesign.dim)
                }
            }.padding(16)
        }
        .background(BackplaneDesign.background)
        .dataBar("Improvements")
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

    private func onOff(_ title: String, _ on: Binding<Bool>, id: String) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            DHeading(text: title)
            DChoices(options: [(label: "on", value: true), (label: "off", value: false)], value: on).accessibilityIdentifier(id)
        }
    }
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                DHeading(text: "Appearance")
                DChoices(options: [(label: "system", value: "system"), (label: "light", value: "light"), (label: "dark", value: "dark")], value: $settings.appearance)
                DHeading(text: "Text size")
                DChoices(options: [(label: "system", value: "system")] + AppSettings.textSteps.map { (label: $0.tag, value: $0.tag) }, value: $settings.textSize).accessibilityIdentifier("settings.textsize")
                onOff("Alerts on this phone", $settings.alerts, id: "settings.alerts")
                    .onChange(of: settings.alerts) { _, on in Task { if on { await client.enablePush() }; await client.pushMute() } }
                if settings.alerts { onOff("Alert sound", $settings.alertSound, id: "settings.sound") }
                onOff("Haptics", $settings.haptics, id: "settings.haptics")
                Text("Off: only what you let through reaches this phone. Alerts come from your uberbot.").dmono(11).foregroundStyle(BackplaneDesign.dim)
                if client.has("push") { DLine(); DataQuietSection(client: client) }
                DLine()
                DHeading(text: "Hub")
                HStack(spacing: 8) {
                    DDot(color: client.conn == .online ? BackplaneDesign.ok : client.conn == .connecting ? BackplaneDesign.warn : BackplaneDesign.bad, size: 8)
                    Text(client.hubName.isEmpty ? "Not paired" : client.hubName).dmono(13).lineLimit(1)
                    Spacer()
                    Text(client.conn.rawValue).dmono(11).foregroundStyle(BackplaneDesign.dim)
                }
                if !client.failure.isEmpty, client.conn != .online { Text(client.failure).dmono(11).foregroundStyle(BackplaneDesign.dim) }
                HStack(spacing: 6) {
                    Button("Reconnect") { client.connect() }.buttonStyle(DButton())
                    Button("Forget this hub") { confirmUnpair = true }.buttonStyle(DButton(danger: true))
                }
                NavigationLink(value: DPage(name: "hub")) { Text("Hub") }.buttonStyle(DButton()).accessibilityIdentifier("settings.hub")
                DLine()
                DataUpdateSection(client: client)
                DLine()
                if client.has("voice") { DataVoiceSection(client: client); DLine() }
                NavigationLink(value: DPage(name: "improvements")) { Text("Improvements") }.buttonStyle(DButton()).accessibilityIdentifier("settings.improve")
                DLine()
                let i = Bundle.main.infoDictionary
                Text("Version \(i?["CFBundleShortVersionString"] as? String ?? "?") (\(i?["CFBundleVersion"] as? String ?? "?"))").dmono(11).foregroundStyle(BackplaneDesign.faint)
            }.padding(16)
        }
        .background(BackplaneDesign.background)
        .dataBar("Settings")
        .confirmationDialog("Forget this hub?", isPresented: $confirmUnpair, titleVisibility: .visible) {
            Button("Forget", role: .destructive) { client.unpair() }
        } message: { Text("This phone's saved copy of its threads is removed. Pair again to come back.") }
    }
}

// the hub's updates: the hub says what to show (update.status); this draws it
struct DataUpdateSection: View {
    @Bindable var client: DataClient
    @State private var status: DUpdate?
    private func load(_ m: String, _ p: [String: Any] = [:]) async { if let r: DUpdate = try? await client.call(m, p) { status = r } }
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            DHeading(text: "Updates")
            if let u = status {
                Text("Backplane \(u.current)").dmono(13)
                Text(u.line).dmono(11).foregroundStyle(BackplaneDesign.dim).accessibilityIdentifier("settings.update.line")
                DChoices(options: [(label: "stable", value: "stable"), (label: "beta", value: "beta")],
                         value: Binding(get: { u.channel }, set: { c in Task { await load("update.channel", ["channel": c]) } })).accessibilityIdentifier("settings.update.channel")
                if u.canCheck {
                    Button("Check now") { Task { await load("update.check"); try? await Task.sleep(for: .seconds(3)); await load("update.status") } }.buttonStyle(DButton()).accessibilityIdentifier("settings.update.check")
                }
            } else { Text("Not connected").dmono(11).foregroundStyle(BackplaneDesign.dim) }
        }
        .task(id: client.conn) { await load("update.status") }
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
            if let image { Image(uiImage: image).resizable().scaledToFit().frame(maxWidth: 220, maxHeight: 220).onTapGesture { full = true } }
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

/// A buy thread's Order card: the steps, the hub's line, the total and what it covers about customs, and Buy now (the hub's link, opened in the browser). As the Android app's OrderCard.
struct DataOrderCard: View {
    let order: DOrder
    @Environment(\.openURL) private var openURL
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(order.name.isEmpty ? "ORDER" : "ORDER · \(order.name)").dmono(12, bold: true).foregroundStyle(BackplaneDesign.text)
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 6) {
                    ForEach(Array(order.steps.enumerated()), id: \.offset) { i, s in
                        if i > 0 { Text("›").dmono(11).foregroundStyle(BackplaneDesign.faint) }
                        switch s.mark {
                        case "done": Text("✓ \(s.label)").dmono(11).foregroundStyle(BackplaneDesign.dim)
                        case "on": Text(s.label).dmono(11, bold: true).foregroundStyle(BackplaneDesign.accentText)
                        default: Text(s.label).dmono(11).foregroundStyle(BackplaneDesign.faint)
                        }
                    }
                }
            }
            Text(order.line).dmono(12).foregroundStyle(order.waiting ? BackplaneDesign.dim : BackplaneDesign.text)
            if !order.total.isEmpty { Text(order.total).dmono(14, bold: true).foregroundStyle(BackplaneDesign.text) }
            if !order.customs.isEmpty { Text(order.customs).dmono(11).foregroundStyle(BackplaneDesign.dim) }
            if let u = order.payURL { Button(order.label.isEmpty ? "Buy now" : order.label) { openURL(u) }.buttonStyle(DButton(primary: true)).accessibilityIdentifier("thread.order.pay") }
        }
        .padding(10).frame(maxWidth: .infinity, alignment: .leading).background(BackplaneDesign.panel).overlay(Rectangle().strokeBorder(BackplaneDesign.line, lineWidth: 1))
        .padding(.horizontal, 12).padding(.vertical, 6).accessibilityIdentifier("thread.order")
    }
}

// A link from outside the app names the hub it would pair with; Cancel comes first.
struct AskPairView: View {
    let client: DataClient
    let link: String
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Pair with this hub?").font(.title2.weight(.semibold))
            Text(verbatim: Pairing.invitation(link)?.origin.absoluteString ?? Pairing.key(link) ?? "").font(.system(.body, design: .monospaced))
            Text("A link opened this. Pair only with a hub you know.").font(.footnote).foregroundStyle(.secondary)
            HStack(spacing: 8) {
                Button("Cancel") { client.askLink = nil }.buttonStyle(PairActionStyle()).accessibilityIdentifier("pair.ask.cancel")
                Button("Pair") { client.askLink = nil; client.pair(link) }.buttonStyle(.bordered).buttonBorderShape(.roundedRectangle(radius: 0)).accessibilityIdentifier("pair.ask.yes")
            }
        }
        .padding(24).frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading).background(BackplaneDesign.background)
        #if DEBUG
        .task { // the simulator's answer, with no taps
            switch ProcessInfo.processInfo.environment["BACKPLANE_ASK"] {
            case "cancel": try? await Task.sleep(for: .seconds(3)); client.askLink = nil
            case "yes": try? await Task.sleep(for: .seconds(3)); client.askLink = nil; client.pair(link)
            default: break
            }
        }
        #endif
    }
}
