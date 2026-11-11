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
                    PairView(link: "", status: client.pairingError, repair: client.needsRepair) { client.pair($0) }
                        .safeAreaInset(edge: .bottom) {
                            // refused or not, a paired phone can still look at the hub
                            if !client.links.isEmpty {
                                VStack(alignment: .leading, spacing: 6) {
                                    NavigationLink { DataHubView(client: client) } label: { Text(Tr.t("Hub")) }.buttonStyle(DButton()).accessibilityIdentifier("repair.hub")
                                    // revoked here: the other paired hubs are one tap away
                                    ForEach(client.links.dropFirst().compactMap { Pairing.key($0) }, id: \.self) { k in
                                        Button(Tr.ios_hub_use(hub: k)) { client.useHub(k) }.buttonStyle(DButton()).accessibilityIdentifier("repair.use.\(k)")
                                    }
                                }.padding(12)
                            }
                        }
                }
            } else {
                NavigationStack(path: $path) { DataThreadsView(client: client, path: $path) }
                    .sheet(isPresented: $client.adding) {
                        NavigationStack {
                            PairView(link: "", status: client.pairingError, adding: true) { client.pair($0) }
                                .toolbar { ToolbarItem(placement: .cancellationAction) { Button(Tr.t("Cancel")) { client.adding = false; client.pairingError = "" }.accessibilityIdentifier("pair.cancel") } }
                        }
                    }
                    .sheet(isPresented: Binding(get: { client.migrationOpen && client.migration.card != nil }, set: { client.migrationOpen = $0 })) { DataMigrationCard(client: client) }
            }
        }
        .overlay { if let l = client.askLink { AskPairView(client: client, link: l) } }
        .tint(BackplaneDesign.accent)
        .preferredColorScheme(settings.colorScheme)
        .modifier(TextSizeModifier(size: settings.dynamicType))
        // the app's language (Settings, Language), for SwiftUI's own controls and dates too; a change draws everything again at once
        .environment(\.locale, Tr.locale)
        .environment(\.backplaneLanguage, Tr.lang)
        .task {
            if !client.links.isEmpty, client.conn == .offline { client.connect() }
            #if DEBUG
            // a check without hands: BACKPLANE_SELECT opens a thread, or a page by @name
            if let t = ProcessInfo.processInfo.environment["BACKPLANE_SELECT"], !t.isEmpty { if t.hasPrefix("@") { path.append(DPage(name: String(t.dropFirst()))) } else if t.hasPrefix("viewer:") { let id = String(t.dropFirst(7)); path.append(id); path.append(DViewerRoute(id: id)) } else { path.append(t) } }
            // a link as if opened from outside, through the same route as onOpenURL: a simulator cannot answer iOS's own "Open in …?" before it
            if let s = ProcessInfo.processInfo.environment["BACKPLANE_OPEN_URL"], let u = URL(string: s) { try? await Task.sleep(for: .seconds(1)); route(u) }
            #endif
        }
        .onChange(of: phase) { _, p in
            if p == .background { client.terms.pause() }
            if p == .active { client.terms.resume(); client.foreground() }
        }
        .fullScreenCover(item: Binding(get: { client.terms.fullscreen }, set: { client.terms.fullscreen = $0 })) { TermFullView(client: client, route: $0) }
        .onChange(of: client.pendingThread, initial: true) { _, id in if let id { path.append(id); client.pendingThread = nil } }
        .onChange(of: client.linkTarget) { _, t in if let t { follow(t); client.linkTarget = nil } }
        .overlay(alignment: .bottom) { DLinkNote(client: client) }
        .onOpenURL { route($0) }
        .onChange(of: client.pushThread, initial: true) { _, t in if !t.isEmpty { var p = NavigationPath(); p.append(t); path = p; client.pushThread = "" } }
        // an alert that a sign-in expired: Settings, Accounts, that account, its sign-in started
        .onChange(of: client.pushAccount, initial: true) { _, a in if !a.isEmpty { var p = NavigationPath(); p.append(DPage(name: "settings")); p.append(DPage(name: "accounts")); p.append(DAccountRoute(id: a, start: true)); path = p; client.pushAccount = "" } }
        // a newly paired (or forgotten) hub starts again on the list
        .onChange(of: client.links.isEmpty) { _, _ in path = NavigationPath() }
        .onChange(of: client.link) { _, _ in path = NavigationPath() }
    }

    /// A link the hub resolved: its screen, inside the app. A thread or file is pushed over what is open; a project is the list at its group; a room is its page.
    private func follow(_ t: DLinkTarget) {
        switch t.kind {
        case .thread: path.append(t.id)
        case .file: path.append(DViewerRoute(id: t.id, file: t.path, line: t.line))
        case .project: path = NavigationPath(); client.focusProject = t.id
        case .room:
            Task {
                if !client.rooms.contains(where: { $0.id == t.id }) { await client.loadBots() }
                if let r = client.rooms.first(where: { $0.id == t.id }) { path.append(r) } else { client.say("That room is not on this hub.") }
            }
        }
    }

    /// This app's links, its one handler: backplane://thread/<id> opens that thread, backplane://account/<id> that account's sign-in; any other (a pairing link) asks first (DataClient.opened).
    private func route(_ url: URL) {
        guard url.scheme == "backplane" || url.scheme == AppIdentity.scheme else { return }
        // (a thread link from outside: the hub says which thread it is, so `desktop~t9` and a link id both open the one on that machine)
        if url.host == "thread" { client.openLink(url, thread: "") }
        else if url.host == "google" { return }   // the end of a Google sign-in (GoogleLoop): not a pairing link
        else if url.host == "account" { if let id = url.pathComponents.dropFirst().first { client.pushAccount = id } } else { client.opened(url) }
    }
}

// MARK: threads

struct DataThreadsView: View {
    @Bindable var client: DataClient
    @Binding var path: NavigationPath
    @State private var addingProject = false
    @State private var newThread: String?       // a project id
    @State private var pickNew: DProject?       // a project on more than one machine: where the new thread starts
    @State private var query = ""
    @State private var renaming: DThread?
    @State private var deleting: DThread?
    @State private var projectMenu: DProject?
    @State private var renamingProject: DProject?
    @State private var badgeProject: DProject?
    @State private var projectTitle = ""
    @State private var shut = Set(UserDefaults.standard.stringArray(forKey: "data.projects.shut") ?? [])
    @State private var treeOpen = Set(UserDefaults.standard.stringArray(forKey: "data.tree.open") ?? []) // parents whose subthreads show
    @State private var flash = ""   // the project a link just opened

    @State private var newSwarm: String?        // a project id: the New swarm sheet
    /// the threads a project's list leaves out: bots' and, on a hub with swarms, the swarms' members (the swarm's row stands for them)
    private var botThreads: Set<String> {
        let b = Set(client.bots.map(\.thread))
        return client.has("swarm") ? b.union(Swarm.members(client.threads, swarm: \.swarm, id: \.id)) : b
    }
    private func toggle(_ p: DProject) {
        if shut.contains(p.id) { shut.remove(p.id) } else { shut.insert(p.id) }
        UserDefaults.standard.set(Array(shut), forKey: "data.projects.shut")
    }

    var body: some View {
        VStack(spacing: 0) {
            DataHubBanner(client: client)
            DataMigrationBanner(client: client)
            DataStarBanner(client: client)
            if client.conn != .online { DataConnectionBanner(client: client) }
            HStack(spacing: 8) {
                if client.has("search") {
                    TextField(Tr.t("Search projects and threads"), text: $query).dmono(13).textInputAutocapitalization(.never).autocorrectionDisabled()
                        .padding(.horizontal, 10).frame(height: 36).dbox().accessibilityIdentifier("data.search")
                } else { Spacer() }
                Button { addingProject = true } label: { Image(systemName: "plus").frame(width: 36, height: 36).foregroundStyle(BackplaneDesign.dim) }
                    .accessibilityLabel(Tr.t("Add project")).accessibilityIdentifier("data.project.add")
            }.padding(.horizontal, 10).padding(.vertical, 8)
            DataMachineChips(client: client)
            DLine()
            if !client.locks.isEmpty { DataLocksStrip(client: client, path: $path) }
            if client.has("search"), !query.trimmingCharacters(in: .whitespaces).isEmpty {
                DataSearchResults(client: client, query: query, open: { path.append($0) }, openAt: { path.append($0) })
            } else { list }
        }
        .background(BackplaneDesign.panel)
        .navigationBarTitleDisplayMode(.inline)
        .toolbarBackground(BackplaneDesign.panel, for: .navigationBar).toolbarBackground(.visible, for: .navigationBar)
        .toolbar {
            ToolbarItem(placement: .topBarLeading) {
                Menu {
                    ForEach(client.links.dropFirst().compactMap { Pairing.key($0) }, id: \.self) { k in
                        Button(k) { client.useHub(k) }.accessibilityIdentifier("data.hub.use.\(k)")
                    }
                    if client.has("terms") { Button(Tr.t("Terminals")) { path.append(DTermsRoute(project: "")) }.accessibilityIdentifier("data.hub.terms") }
                    Button(Tr.t("Add a hub")) { client.pairingError = ""; client.adding = true }.accessibilityIdentifier("data.hub.add")
                } label: {
                    HStack(spacing: 8) {
                        BrandMark(size: 20, conn: client.conn == .online)
                        Text(client.hubName.isEmpty ? "Backplane" : client.hubName).dmono(14, bold: true).foregroundStyle(BackplaneDesign.text).lineLimit(1)
                        Image(systemName: "chevron.down").font(.system(size: 9)).foregroundStyle(BackplaneDesign.dim)
                    }.fixedSize()
                }.accessibilityIdentifier("data.hub")
            }.dflat()
            ToolbarItem(placement: .topBarTrailing) {
                Button { path.append(DPage(name: "settings")) } label: { Image(systemName: "gearshape").foregroundStyle(BackplaneDesign.dim) }
                    .accessibilityLabel(Tr.t("Settings")).accessibilityIdentifier("data.settings")
            }.dflat()
        }
        .navigationDestination(for: String.self) { DataThreadView(client: client, id: $0).environment(\.dataSubThread, client.has("subview") ? $0 : "") }
        .navigationDestination(for: DSubRoute.self) { DataSubView(client: client, route: $0) }
        .navigationDestination(for: DThreadAt.self) { DataThreadView(client: client, id: $0.id, at: $0.msg).environment(\.dataSubThread, client.has("subview") ? $0.id : "") }
        .navigationDestination(for: DRoom.self) { DataRoomView(client: client, room: $0) }
        .navigationDestination(for: DSwarmRoute.self) { DataSwarmView(client: client, route: $0) }
        .navigationDestination(for: DViewerRoute.self) { r in
            if let t = client.threads.first(where: { $0.id == r.id }) { DataViewerView(client: client, thread: t, openFile: r.file, line: r.line) }
            else { DLoading(text: Tr.t(client.conn == .online ? "Loading the thread…" : "Connecting to the hub…")) } // never a blank page while the thread list comes
        }
        .navigationDestination(for: DPage.self) { p in if p.name == "settings" { DataSettingsView(client: client) } else if p.name == "hub" { DataHubView(client: client) } else if p.name == "accounts" { DataAccountsView(client: client) } else { DataImprovementsView(client: client) } }
        .navigationDestination(for: DAccountRoute.self) { DataAccountView(client: client, route: $0) }
        .navigationDestination(for: DTermsRoute.self) { DataTermsView(client: client, project: $0.project) }
        .modifier(DataThreadDialogs(client: client, renaming: $renaming, deleting: $deleting))
        .alert(Tr.t("Rename project"), isPresented: Binding(get: { renamingProject != nil }, set: { if !$0 { renamingProject = nil } })) {
            TextField(Tr.t("Title"), text: $projectTitle)
            Button(Tr.t("Rename")) { if let p = renamingProject { Task { await client.projectAct(p.id, "rename", title: projectTitle) } } }
            Button(Tr.t("Cancel"), role: .cancel) {}
        }
        .confirmationDialog(Tr.ios_project_remove_ask(title: projectMenu?.title ?? ""), isPresented: Binding(get: { projectMenu != nil }, set: { if !$0 { projectMenu = nil } }), titleVisibility: .visible) {
            Button(Tr.t("Remove project"), role: .destructive) { if let p = projectMenu { Task { await client.projectAct(p.id, "remove") } } }
            Button(Tr.t("Keep it"), role: .cancel) {}
        }
        .confirmationDialog(pickNew?.title ?? "", isPresented: Binding(get: { pickNew != nil }, set: { if !$0 { pickNew = nil } }), titleVisibility: .visible) {
            ForEach(pickNew?.newOn ?? [], id: \.value) { n in Button(dAway(Tr.t(n.label), n.away)) { newThread = n.value; pickNew = nil }.accessibilityIdentifier("data.thread.new.on." + n.value) }
        }
        .sheet(isPresented: $addingProject) { DataNewProjectSheet(client: client) }
        .sheet(item: Binding(get: { newSwarm.map { Wrapped(id: $0) } }, set: { newSwarm = $0?.id })) { w in
            DataNewSwarmSheet(client: client, project: w.id) { path.append($0) }
        }
        .sheet(item: $badgeProject) { DataBadgeSheet(client: client, project: $0) }
        .sheet(item: Binding(get: { newThread.map { Wrapped(id: $0) } }, set: { newThread = $0?.id })) { w in
            DataNewThreadSheet(client: client, project: w.id) { path.append($0) }
        }
    }

    private var list: some View {
        ScrollViewReader { proxy in listBody
            .onChange(of: client.focusProject, initial: true) { _, id in
                guard !id.isEmpty else { return }
                shut.remove(id); UserDefaults.standard.set(Array(shut), forKey: "data.projects.shut")
                flash = id; client.focusProject = ""
                Task { try? await Task.sleep(nanoseconds: 150_000_000); withAnimation { proxy.scrollTo("project." + id, anchor: .top) }; try? await Task.sleep(nanoseconds: 2_500_000_000); if flash == id { flash = "" } }
            }
        }
    }
    private var listBody: some View {
        List {
            if client.projects.isEmpty && client.listsLoaded {
                VStack(alignment: .leading, spacing: 10) {
                    Text(Tr.t("No projects yet")).dmono(14, bold: true)
                    Text(Tr.t("A project is a folder on your hub's computer. Add one to start a thread.")).dmono(12).foregroundStyle(BackplaneDesign.dim)
                    Button(Tr.t("Add a project")) { addingProject = true }.buttonStyle(DButton(primary: true)).accessibilityIdentifier("data.project.add.empty")
                }.padding(14).dataRow()
            }
            ForEach(client.bots.filter { $0.uber && !$0.thread.isEmpty }) { b in botRow(b) }
            ForEach(client.projects) { p in
                projectHeader(p)
                if !shut.contains(p.id) {
                    if client.has("swarm") { ForEach(Swarm.listed(client.swarms, project: p.id)) { r in DataSwarmRow(row: r) { path.append(DSwarmRoute(id: r.id, name: r.name)) } } }
                    let g = ThreadGroups.splitNested(client.threads, project: p.id, hidden: botThreads)
                    let n = ThreadGroups.count(hub: p.settledCount, loaded: g.settled.count)
                    ForEach(ThreadGroups.tree(g.active, all: g.all, open: treeOpen), id: \.t.id) { treeRow($0) }
                    if g.active.isEmpty && n == 0 && !(client.has("swarm") && !Swarm.listed(client.swarms, project: p.id).isEmpty) { Text(Tr.t("No threads")).dmono(12).foregroundStyle(BackplaneDesign.faint).padding(.leading, 34).frame(height: 32).dataRow() }
                    if n > 0 {
                        let open = client.settledOpen.contains(p.id)
                        Button { client.setSettled(open: !open, p.id) } label: {
                            HStack(spacing: 6) { Image(systemName: open ? "chevron.down" : "chevron.right").font(.system(size: 10)); Text(Tr.t(ThreadGroups.title(n))).dmono(11); Spacer() }
                                .foregroundStyle(BackplaneDesign.faint).padding(.leading, 30).frame(height: 32).contentShape(Rectangle())
                        }.buttonStyle(.plain).dataRow().accessibilityIdentifier("data.settled." + p.id)
                        if open {
                            ForEach(ThreadGroups.tree(g.settled, all: g.all, open: treeOpen), id: \.t.id) { treeRow($0) }
                            if client.settledMore[p.id] != nil {
                                Button { Task { await client.loadSettled(p.id, more: true) } } label: { Text(Tr.t("More")).dmono(11).foregroundStyle(BackplaneDesign.faint).padding(.leading, 34).frame(height: 32) }
                                    .buttonStyle(.plain).dataRow().accessibilityIdentifier("data.settled.more." + p.id)
                            }
                        }
                    }
                }
            }
            // Conversation history: the old Scratch chats. Its row opens or shuts the list; a chat that wants attention always has its row; nothing starts here
            if client.history.count > 0 {
                let open = client.historyOpen
                Button { client.setHistory(open: !open) } label: {
                    HStack(spacing: 8) {
                        Image(systemName: open ? "chevron.down" : "chevron.right").font(.system(size: 11)).foregroundStyle(BackplaneDesign.dim).frame(width: 14)
                        Text(Tr.t(ThreadGroups.historyTitle(client.history.count)).uppercased()).dmono(12).foregroundStyle(BackplaneDesign.dim).lineLimit(1); Spacer(minLength: 0)
                    }.padding(.leading, 10).frame(height: 40).contentShape(Rectangle())
                }.buttonStyle(.plain).dataRow().accessibilityIdentifier("data.history")
                ForEach(ThreadGroups.history(live: client.history.live, rows: client.history.threads, open: open)) { threadRow($0) }
                if open && client.history.next != nil {
                    Button { Task { await client.loadHistory(more: true) } } label: { Text(Tr.t("More")).dmono(11).foregroundStyle(BackplaneDesign.faint).padding(.leading, 34).frame(height: 32) }
                        .buttonStyle(.plain).dataRow().accessibilityIdentifier("data.history.more")
                }
            }
            let bots = client.bots.filter { !$0.uber && !$0.thread.isEmpty }
            if !bots.isEmpty { DHeading(text: Tr.t("Bots")).padding(.leading, 12).padding(.top, 12).padding(.bottom, 4).dataRow(); ForEach(bots) { botRow($0) } }
            if !client.rooms.isEmpty {
                DHeading(text: Tr.t("Rooms")).padding(.leading, 12).padding(.top, 12).padding(.bottom, 4).dataRow()
                ForEach(client.rooms) { r in
                    Button { path.append(r) } label: { sideRow("#", r.name, "room", r.mark) }.buttonStyle(.plain).dataRow().accessibilityIdentifier("data.room." + r.id)
                }
            }
        }
        .listStyle(.plain).scrollContentBackground(.hidden).environment(\.defaultMinListRowHeight, 1)
        .refreshable { await client.refresh() }
    }

    /// `mark`: a bot or room of a linked machine names its machine, as its threads do
    private func sideRow(_ sigil: String, _ name: String, _ detail: String, _ mark: DMark? = nil) -> some View {
        HStack(spacing: 8) {
            Text(sigil).dmono(13, bold: true).foregroundStyle(BackplaneDesign.accent).frame(width: 16)
            Text(name).dmono(13).foregroundStyle(BackplaneDesign.text).lineLimit(1); Spacer()
            if let m = mark { Text(dAway(m.short, m.away)).dmono(11).foregroundStyle(BackplaneDesign.faint) }
            Text(Tr.t(detail)).dmono(11).foregroundStyle(BackplaneDesign.faint)
        }.padding(.horizontal, 12).frame(minHeight: 40).contentShape(Rectangle())
    }
    private func botRow(_ b: DBot) -> some View {
        Button { path.append(b.thread) } label: { sideRow("@", b.name, b.uber ? "uberbot" : "bot", b.mark) }.buttonStyle(.plain).dataRow().accessibilityIdentifier("data.bot." + b.id)
    }
    private func projectHeader(_ p: DProject) -> some View {
        HStack(spacing: 8) {
            Button { toggle(p) } label: {
                HStack(spacing: 8) {
                    Image(systemName: shut.contains(p.id) ? "chevron.right" : "chevron.down").font(.system(size: 11)).foregroundStyle(BackplaneDesign.dim).frame(width: 14)
                    // a project named Backplane: the app's own logo (the hub's `backplane`, which no badge sheet offers)
                    if p.icon == "backplane" { BrandMark(size: 13) } else { Image(systemName: dataBadgeSymbol(p.icon)).font(.system(size: 13)).foregroundStyle(Color(css: p.css) ?? BackplaneDesign.ok) }
                    Text(p.title.uppercased()).dmono(12).foregroundStyle(BackplaneDesign.dim).lineLimit(1)
                    // another machine's project says where it is ("on desktop"); one that is also on a linked machine says so ("also on desktop")
                    let where_ = p.mark.map { dAway(Tr.t($0.label), $0.away) } ?? Tr.t(p.also)
                    if !where_.isEmpty { Text(where_).dmono(11).foregroundStyle(BackplaneDesign.faint).lineLimit(1).accessibilityIdentifier("data.project.where." + p.id) }
                    if p.waiting > 0 { Text("\(p.waiting)").dmono(11, bold: true).foregroundStyle(BackplaneDesign.bad) }
                    Spacer(minLength: 0)
                }.frame(height: 40).contentShape(Rectangle())
            }.buttonStyle(.plain).accessibilityIdentifier("data.project." + p.id)
            if client.has("terms") {
                Button { path.append(DTermsRoute(project: p.id)) } label: { Image(systemName: "terminal").font(.system(size: 13)).foregroundStyle(BackplaneDesign.dim).frame(width: 36, height: 40).contentShape(Rectangle()) }
                    .buttonStyle(.plain).accessibilityLabel(Tr.ios_project_terms(project: p.title)).accessibilityIdentifier("data.terms." + p.id)
            }
            if client.has("swarm") && p.mark == nil {
                Button { newSwarm = p.id } label: { Image(systemName: "circle.hexagongrid").font(.system(size: 13)).foregroundStyle(BackplaneDesign.dim).frame(width: 36, height: 40).contentShape(Rectangle()) }
                    .buttonStyle(.plain).accessibilityLabel(Tr.ios_swarm_new_in(project: p.title)).accessibilityIdentifier("data.swarm.new." + p.id)
            }
            Button { if p.newOn.count > 1 { pickNew = p } else { newThread = p.id } } label: { Image(systemName: "plus").font(.system(size: 14)).foregroundStyle(BackplaneDesign.dim).frame(width: 40, height: 40).contentShape(Rectangle()) }
                .buttonStyle(.plain).accessibilityLabel(Tr.ios_project_newthread(project: p.title)).accessibilityIdentifier("data.thread.new." + p.id)
        }
        .padding(.leading, 10).padding(.trailing, 2).background(flash == p.id ? BackplaneDesign.accent.opacity(0.18) : Color.clear).id("project." + p.id).dataRow()
        .contextMenu { if client.has("actions") { Button(Tr.t("Rename"), systemImage: "pencil") { projectTitle = p.title; renamingProject = p }; Button(Tr.t("Badge"), systemImage: "paintpalette") { badgeProject = p }; Button(Tr.t("Remove project"), systemImage: "trash", role: .destructive) { projectMenu = p } } }
    }
    private func treeRow(_ r: ThreadGroups.Node<DThread>) -> some View {
        threadRow(r.t, depth: r.depth, kids: r.kids)
    }
    private func threadRow(_ t: DThread, depth: Int = 0, kids: Int = 0) -> some View {
        let on = treeOpen.contains(t.id)
        return Button { path.append(t.id) } label: { DataThreadRow(thread: t, depth: depth, kids: kids, open: on) {
            if on { treeOpen.remove(t.id) } else { treeOpen.insert(t.id) }
            UserDefaults.standard.set(Array(treeOpen), forKey: "data.tree.open")
        } }
            .buttonStyle(.plain).dataRow()
            .contextMenu { if client.has("actions") { DataThreadActions(client: client, thread: t, renaming: $renaming, deleting: $deleting) } }
            .swipeActions(edge: .trailing, allowsFullSwipe: false) {
                if client.has("actions") {
                    Button(Tr.t(t.settled ? "Unsettle" : "Settle")) { Task { await client.threadAct(t.id, t.settled ? "unsettle" : "settle") } }.tint(BackplaneDesign.ok)
                    Button(Tr.t("Archive")) { Task { await client.threadAct(t.id, "archive") } }.tint(BackplaneDesign.faint)
                }
            }
            .onAppear { client.prefetch(t.id) }
            .accessibilityIdentifier("data.thread.row." + t.id)
    }
    struct Wrapped: Identifiable { var id: String }
}

// Who holds each lock, on the home list; a tap opens the holder's thread.
struct DataLocksStrip: View {
    @Bindable var client: DataClient
    @Binding var path: NavigationPath
    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(Tr.t("LOCKS")).dmono(11).foregroundStyle(BackplaneDesign.dim)
            ForEach(client.locks) { l in
                Button { if !l.thread.isEmpty { path.append(l.thread) } } label: {
                    HStack(spacing: 8) { Text(l.name).dmono(12).lineLimit(1); Text(Tr.t(l.line)).dmono(11).foregroundStyle(BackplaneDesign.dim).lineLimit(1); Spacer(); if l.waiting > 0 { Text(Tr.ios_lock_waiting(n: l.waiting)).dmono(11).foregroundStyle(BackplaneDesign.dim) } }
                }.accessibilityIdentifier("data.lock.\(l.name)")
            }
        }.padding(.horizontal, 12).padding(.vertical, 6).background(BackplaneDesign.panel)
    }
}

struct DataConnectionBanner: View {
    let client: DataClient
    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 8) {
                DDot(color: client.conn == .connecting ? BackplaneDesign.warn : BackplaneDesign.bad, size: 8)
                VStack(alignment: .leading, spacing: 2) {
                    Text(client.conn == .connecting ? Tr.ios_conn_connecting(hub: client.hubName) : Tr.t("Offline · showing what this phone has saved")).dmono(12)
                    if client.conn == .offline, !client.failure.isEmpty { Text(Tr.t(client.failure)).dmono(11).foregroundStyle(BackplaneDesign.dim).lineLimit(2) }
                }
                Spacer()
                NavigationLink(value: DPage(name: "hub")) { Text(Tr.t("Hub")) }.dmono(12, bold: true).foregroundStyle(BackplaneDesign.accent).accessibilityIdentifier("conn.hub")
                if client.conn == .offline { Button(Tr.t("Retry")) { client.connect() }.dmono(12, bold: true).foregroundStyle(BackplaneDesign.accent) }
            }.padding(.horizontal, 12).padding(.vertical, 8).background(BackplaneDesign.raised)
            DLine()
        }.accessibilityIdentifier("data.connection.banner")
    }
}

/// The machine switcher over the lists: a chip each for every machine, this one and each linked one, in the hub's words (nothing with no linked machine).
struct DataMachineChips: View {
    let client: DataClient
    var body: some View {
        if !client.chips.isEmpty {
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 6) {
                    ForEach(client.chips) { c in
                        Button { client.pickMachine(c.value) } label: {
                            Text(dAway(Tr.t(c.label), c.away)).dmono(11).foregroundStyle(c.on ? BackplaneDesign.accent : BackplaneDesign.dim).padding(.horizontal, 10).frame(height: 28)
                                .overlay(Rectangle().stroke(c.on ? BackplaneDesign.accent : BackplaneDesign.faint.opacity(0.5), lineWidth: 1)).opacity(c.away && !c.on ? 0.55 : 1)
                        }.buttonStyle(.plain).accessibilityLabel(Tr.t(c.hint.isEmpty ? c.label : c.hint)).accessibilityIdentifier("data.machine." + (c.value.isEmpty ? "all" : c.value))
                    }
                }.padding(.horizontal, 10).padding(.bottom, 8)
            }.accessibilityIdentifier("data.machines")
        }
    }
}

struct DataThreadRow: View {
    let thread: DThread
    var depth = 0, kids = 0, open = false
    var toggle: () -> Void = {}
    var body: some View {
        HStack(alignment: .top, spacing: 8) {
            // a parent's chevron (left of its dot) shows or hides its subagents and subthreads; a child sits a step in
            HStack(alignment: .top, spacing: 0) {
                Group {
                    if kids > 0 {
                        Image(systemName: open ? "chevron.down" : "chevron.right").font(.system(size: 10)).foregroundStyle(BackplaneDesign.faint)
                            .frame(width: 22, height: 22).contentShape(Rectangle()).highPriorityGesture(TapGesture().onEnded { toggle() })
                            .accessibilityLabel(open ? Tr.ios_kids_hide(n: kids) : Tr.ios_kids_show(n: kids)).accessibilityAddTraits(.isButton).accessibilityIdentifier("data.thread.kids." + thread.id)
                    } else { Color.clear.frame(width: 22, height: 22) }
                }.padding(.top, -2)
                DStatusDot(thread: thread).padding(.top, 5).frame(width: 14)
            }
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 6) {
                    if thread.pinned { Image(systemName: "pin.fill").font(.system(size: 9)).foregroundStyle(BackplaneDesign.accent) }
                    Text(thread.title.isEmpty ? Tr.t("New thread") : thread.title).dmono(13).foregroundStyle(BackplaneDesign.text).lineLimit(1)
                }
                if !thread.preview.isEmpty { Text(thread.preview.replacingOccurrences(of: "**", with: "").replacingOccurrences(of: "`", with: "")).dmono(11).foregroundStyle(BackplaneDesign.faint).lineLimit(1) }
            }
            Spacer(minLength: 0)
            if kids > 0 && !open { Text("\(kids)").dmono(11).foregroundStyle(BackplaneDesign.faint) }
            if thread.waiting > 0 { Text("\(thread.waiting)").dmono(11, bold: true).foregroundStyle(BackplaneDesign.bad) }
            // another machine's thread: its machine's short name before the age, quiet; dimmer with "away" when that machine did not answer
            if let m = thread.mark { Text(dAway(m.short, m.away)).dmono(11).foregroundStyle(BackplaneDesign.faint).accessibilityIdentifier("data.thread.mark." + thread.id) }
            Text(dAgo(thread.updatedAt)).dmono(11).foregroundStyle(BackplaneDesign.faint)
        }
        .padding(.leading, 4 + 16 * CGFloat(min(depth, 4))).padding(.trailing, 12).padding(.vertical, 8).opacity(thread.mark?.away == true ? 0.5 : thread.settled ? 0.55 : 1).contentShape(Rectangle())
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
                TextField(Tr.t("Folder on the hub's computer"), text: $path).dmono(13).textInputAutocapitalization(.never).autocorrectionDisabled()
                    .padding(.horizontal, 10).frame(height: 40).dbox().padding(12).accessibilityIdentifier("data.project.path")
                if !error.isEmpty { Text(Tr.t(error)).dmono(12).foregroundStyle(BackplaneDesign.bad).padding(.horizontal, 12).padding(.bottom, 8) }
                DLine()
                List {
                    if let d = dirs {
                        if d.parent != d.path { row("..", "arrow.up.left", d.parent) }
                        ForEach(d.dirs, id: \.self) { n in row(n, "folder", d.path.hasSuffix("/") ? d.path + n : d.path + "/" + n) }
                    }
                }.listStyle(.plain).scrollContentBackground(.hidden).environment(\.defaultMinListRowHeight, 1)
            }
            .background(BackplaneDesign.panel)
            .dataBar(Tr.t("Add project"), back: false)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button(Tr.t("Cancel")) { dismiss() }.dmono(13) }.dflat()
                ToolbarItem(placement: .confirmationAction) {
                    Button(Tr.t("Add")) { Task { await add() } }.dmono(13, bold: true).disabled(busy || path.trimmingCharacters(in: .whitespaces).isEmpty).accessibilityIdentifier("data.project.add.confirm")
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
    private var owner: DProject? { client.projects.first { $0.id == project || $0.newOn.contains { $0.value == project } } }
    // where it runs (D6): the hub's choice first ("runs on desktop"); the line opens the machine picker when the project is on more than one
    @State private var target = ""
    @State private var pickRun = false
    private var runTo: String { target.isEmpty ? project : target }
    private var whereLine: String {
        guard let o = owner else { return "" }
        let chosen = o.newOn.first(where: { $0.value == runTo })
        if !o.runsOn.isEmpty && (chosen == nil || chosen == o.newOn.first) { return o.runsOn }
        if let w = chosen, o.newOn.count > 1 || o.mark != nil { return w.label }
        return ""
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text(owner?.title ?? Tr.t("New thread")).dmono(14, bold: true).lineLimit(1); Spacer()
                Button(Tr.t("Cancel")) { dismiss() }.dmono(13).foregroundStyle(BackplaneDesign.dim)
            }
            if !whereLine.isEmpty {
                Button { if (owner?.newOn.count ?? 0) > 1 { pickRun = true } } label: { Text(Tr.t(whereLine) + ((owner?.newOn.count ?? 0) > 1 ? " ▾" : "")).dmono(11).foregroundStyle(BackplaneDesign.faint) }
                    .buttonStyle(.plain).accessibilityIdentifier("data.thread.new.where")
                    .confirmationDialog(owner?.title ?? "", isPresented: $pickRun) { ForEach(owner?.newOn ?? [], id: \.value) { n in Button(dAway(Tr.t(n.label), n.away)) { target = n.value } } }
            }
            ZStack(alignment: .topLeading) {
                TextEditor(text: $text).dmono(13).scrollContentBackground(.hidden).padding(6).accessibilityIdentifier("data.thread.new.text")
                if text.isEmpty { Text(Tr.t("What should it do?")).dmono(13).foregroundStyle(BackplaneDesign.faint).padding(.horizontal, 11).padding(.vertical, 14).allowsHitTesting(false) }
            }.frame(minHeight: 120).dbox(BackplaneDesign.accent)
            if !error.isEmpty { Text(Tr.t(error)).dmono(12).foregroundStyle(BackplaneDesign.bad) }
            HStack(spacing: 6) {
                if client.has("picker") {
                    Button { picking = true } label: { Text(chip.isEmpty ? " " : chip).dmono(12).foregroundStyle(BackplaneDesign.text).lineLimit(1).padding(.horizontal, 10).frame(maxWidth: .infinity, minHeight: 40, alignment: .leading).dbox(fill: BackplaneDesign.panel) }
                        .buttonStyle(.plain).accessibilityLabel(Tr.ios_model_label(model: chip)).accessibilityIdentifier("data.thread.new.model")
                } else { Spacer() }
                Button(Tr.t(busy ? "Starting…" : "Start")) { Task { await start() } }.buttonStyle(DButton(primary: true)).disabled(busy).accessibilityIdentifier("data.thread.new.start")
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
            let t = try await client.createThread(project: runTo, text: text, choice: choice)
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

/// A side question and its answer, until closed.
private struct DataBtwCard: View {
    let btw: DataClient.Btw
    let close: () -> Void
    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(verbatim: "btw · \(btw.q)").font(.caption).foregroundStyle(.secondary).lineLimit(1)
                Spacer()
                Button(action: close) { Image(systemName: "xmark") }.buttonStyle(.plain).accessibilityLabel(Tr.t("Close")).accessibilityIdentifier("thread.btw.close")
            }
            ScrollView { Text(btw.a == "Thinking…" ? Tr.t(btw.a) : btw.a).font(.footnote).frame(maxWidth: .infinity, alignment: .leading).textSelection(.enabled) }.frame(maxHeight: 220)
        }
        .padding(.horizontal).padding(.vertical, 8).background(BackplaneDesign.panel).accessibilityElement(children: .contain).accessibilityIdentifier("thread.btw")
    }
}

/// The masked box of a chat: `/secret`, or an agent's request. The value is held here and leaves once, as `secret.set`; it is never a draft.
private struct DataSecretBox: View {
    @Bindable var client: DataClient
    let thread: String
    let name0: String
    let asked: DataClient.SecretAsk?
    let shut: () -> Void
    @State private var name = ""
    @State private var value = ""
    @State private var err = ""
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(name0.isEmpty ? Tr.t("A secret for this chat. Agents use it by name and never see the value.") : Tr.ios_secret_named(name: name0)).font(.footnote)
            if let p = asked?.purpose, !p.isEmpty { Text(p).font(.caption).foregroundStyle(.secondary) }
            if name0.isEmpty { TextField(Tr.t("Name"), text: $name).textFieldStyle(.roundedBorder).autocorrectionDisabled().textInputAutocapitalization(.never).accessibilityIdentifier("thread.secret.name") }
            SecureField(Tr.t("Paste the value"), text: $value).textFieldStyle(.roundedBorder).accessibilityIdentifier("thread.secret.value")
            if !err.isEmpty { Text(Tr.t(err)).font(.caption).foregroundStyle(.red) }
            HStack {
                Button(Tr.t("Save")) {
                    Task { do { try await client.saveSecret(thread, name: (name0.isEmpty ? name : name0).trimmingCharacters(in: .whitespaces), value: value, purpose: asked?.purpose ?? ""); value = ""; shut() }
                        catch { err = (error as? DError)?.message ?? "The hub refused the secret" } }
                }.disabled((name0.isEmpty && name.isEmpty) || value.isEmpty).accessibilityIdentifier("thread.secret.save")
                Button(Tr.t("Cancel")) { value = ""; shut() }.accessibilityIdentifier("thread.secret.cancel")
                if let a = asked { Button(Tr.t("Not needed")) { Task { await client.dismissSecret(thread, name: a.name); value = ""; shut() } } }
            }
        }
        .padding(12).background(BackplaneDesign.panel).accessibilityElement(children: .contain).accessibilityIdentifier("thread.secret")
    }
}

struct DataThreadView: View {
    @Bindable var client: DataClient
    let id: String
    @State private var text = ""
    @State private var pending: [Data] = []
    @State private var picked: [PhotosPickerItem] = []
    @State private var photoLibrary = false
    @State private var camera = false
    @State private var drawing = false
    @State private var notice = ""
    // the masked box: a name ("" for a new one) while open; the value stays in the box until Save
    @State private var box: String?
    @State private var renaming: DThread?
    @State private var deleting: DThread?
    @State private var modelSheet = false
    @State private var noChoice = DNewThreadChoice()
    @State private var openRuns: Set<String> = []
    @State private var atEnd = true
    @State private var order: DOrder?
    @State private var buy: DBuy?          // Buy it, when the hub offers it for this thread
    @State private var buying = false
    @State private var finding = false     // Search in thread
    @State private var findGo: String?     // the message to go to (Search in thread, the find bar, a search hit), read older pages until it is held
    @State private var mark: String?       // the message marked there, until the find bar closes
    @State private var findOpen = false    // the find bar (feature find)
    @State private var findQ = ""
    @State private var findRes: DFind?
    @State private var findAt: Int?        // the current match in findRes.matches (ThreadFind)
    @FocusState private var findFocus: Bool
    private let at: String?                // opened at this message (a search hit)
    @State private var caret: (at: Int, rev: Int)?
    @Environment(\.dismiss) private var dismiss

    init(client: DataClient, id: String, at: String? = nil) { self.client = client; self.id = id; self.at = at; client.touch(id) }
    /// the find bar's current match: marked, and gone to
    private func findGoTo() { guard let i = findAt, let r = findRes, r.matches.indices.contains(i) else { return }; mark = r.matches[i].msg; findGo = r.matches[i].msg }
    private func findStep(_ by: Int) { findAt = ThreadFind.step(findAt, count: findRes?.matches.count ?? 0, by: by); findGoTo() }
    private func findClose() { findOpen = false; findFocus = false; findQ = ""; findRes = nil; findAt = nil; mark = nil }

    private var thread: DThread? { client.threads.first { $0.id == id } }
    /// Buy it: the hub makes the buy thread, which opens (as a link to it would)
    private func startBuy() {
        guard !buying else { return }
        buying = true
        Task { do { let t = try await client.buy(id); if !t.isEmpty { client.linkTarget = DLinkTarget(kind: .thread, id: t) } } catch { notice = error.localizedDescription }; buying = false }
    }
    // a message the hub holds in the queue shows in the strip over the composer, not in the conversation (the row's msg is its entry)
    private var list: [DEntry] {
        let all = client.entries[id] ?? []; let q = client.queues[id] ?? []
        guard !q.isEmpty else { return all }
        let held = Set(q.map(\.msg)); return all.filter { !held.contains($0.id) && !($0.clientId.map(held.contains) ?? false) }
    }
    private var working: Bool { thread?.state == "working" || thread?.state == "asking" }
    private var chip: String { guard let t = thread else { return "" }; return t.chip.isEmpty ? "\(t.provider) / \(t.model.isEmpty ? Tr.t("default") : t.model)" : t.chip }

    var body: some View {
        VStack(spacing: 0) {
            DataHubBanner(client: client)
            DataMigrationBanner(client: client)
            DataStarBanner(client: client)
            if client.conn != .online { DataConnectionBanner(client: client) }
            stateStrip
            if let m = thread?.mark { Text(dAway(Tr.ios_machine_on(machine: m.name), m.away)).dmono(11).foregroundStyle(BackplaneDesign.faint).frame(maxWidth: .infinity, alignment: .leading).padding(.horizontal, 14).padding(.vertical, 3).accessibilityIdentifier("data.thread.machine") }
            if findOpen { findBar }
            if let w = client.work[id], !w.rows.isEmpty { DataWorkPanel(client: client, thread: id, work: w) }
            ScrollViewReader { proxy in
                ScrollView {
                    // A VStack, not a LazyVStack: once a message with a picture and words was sent, the lazy stack's placement never settled
                    // (the main thread at 100% in LazySubviewPlacements, no view of ours re-run, the screen frozen). A page is 40 entries with
                    // tool runs folded, so the rows are few; older pages come only when asked, by the row below, never by a row appearing.
                    VStack(alignment: .leading, spacing: 8) {
                        // a tap, as on Android: in a VStack every row appears at once, so an onAppear here would pull the whole history page by page
                        if client.older[id] != nil {
                            Button { client.loadOlder(id) } label: { Text(Tr.t("load earlier")).dmono(12).foregroundStyle(BackplaneDesign.dim).frame(maxWidth: .infinity, minHeight: 36) }
                                .buttonStyle(.plain).accessibilityIdentifier("data.thread.older")
                        }
                        ForEach(DataFold.rows(list, running: working)) { r in
                            switch r {
                            case .entry(let e): DataEntryRow(client: client, thread: id, entry: e)
                                // the message a find or a search hit went to
                                .background { if mark == e.id { Rectangle().fill(BackplaneDesign.accent.opacity(0.12)).overlay(alignment: .leading) { Rectangle().fill(BackplaneDesign.accent).frame(width: 2) }.padding(-4).accessibilityElement().accessibilityLabel(Tr.k("ios.thread.marked")).accessibilityIdentifier("data.thread.marked") } }
                                .id(e.id)
                            case .fold(let f): DataFoldRow(fold: f, open: Binding(get: { openRuns.contains(f.id) }, set: { if $0 { openRuns.insert(f.id) } else { openRuns.remove(f.id) } })).id("fold:" + f.id)
                            }
                        }
                        if list.isEmpty && client.loadedThreads.contains(id) { Text(Tr.t("Start a conversation.")).dmono(13).foregroundStyle(BackplaneDesign.dim).padding(.top, 40).frame(maxWidth: .infinity, alignment: .leading) }
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
                // Search in thread: to the message, loading earlier pages until it is here (a few at most)
                .task(id: findGo) {
                    guard let m = findGo else { return }
                    for _ in 0..<30 {
                        if list.contains(where: { $0.id == m }) { atEnd = false; mark = m; withAnimation { proxy.scrollTo(m, anchor: .center) }; break }
                        if client.older[id] == nil { notice = "That message is no longer in this thread."; break }
                        client.loadOlder(id); try? await Task.sleep(for: .milliseconds(400))
                    }
                    findGo = nil
                }
            }
            if let order { DataOrderCard(order: order) }
            if let m = thread?.mark { Text(dAway(Tr.t(m.runs), m.away)).dmono(11).foregroundStyle(BackplaneDesign.faint).frame(maxWidth: .infinity, alignment: .leading).padding(.horizontal, 14).padding(.top, 4).accessibilityIdentifier("data.thread.runs") }
            DataMentionBar(client: client, text: text) { text = $0; client.setDraft(id, $0) }
            DVoiceStrip(client: client, thread: id, draft: text)
            if let b = client.btw[id] { DataBtwCard(btw: b) { client.closeBtw(id) } }
            let asked = client.secretAsks[id]?.first
            if box != nil || asked != nil {
                DataSecretBox(client: client, thread: id, name0: box ?? asked?.name ?? "", asked: (box == nil || asked?.name == box) ? asked : nil) { box = nil }.id((box ?? "") + (asked?.name ?? ""))
            }
            let queued = client.queueRows(id)
            if !queued.isEmpty || client.queueError[id] != nil { DataQueueStrip(client: client, thread: id, rows: queued) }
            composer
        }
        // Edit in the strip, or a refusal: the text is back in the draft
        // (the caret goes to its end; negative revs, so they never meet dictation's)
        .onChange(of: client.draftBack) { _, b in if b.thread == id { text = client.drafts[id] ?? ""; caret = (text.utf16.count, -b.rev) } }
        .onChange(of: client.voice.caret.rev) { _, rev in
            // the hub's dictation wrote this thread's draft: show it and put the caret where it said
            let c = client.voice.caret
            guard c.thread == id else { return }
            let new = client.drafts[id] ?? ""
            text = new
            caret = (VoiceEdit.utf16(new, caret: c.at), rev)
        }
        .task(id: "\(list.count)|\(thread?.state ?? "")|\(client.conn == .online)") { if client.has("order"), let o = try? await client.order(id) { order = o } else if !client.has("order") { order = nil } }
        .task(id: "\(thread?.state ?? "")|\(client.conn == .online)") { if client.conn == .online, thread?.state != "working" { buy = await client.buyOf(id) } }
        .background(BackplaneDesign.background)
        .dataBar(thread?.title.isEmpty == false ? thread!.title : Tr.t("Thread"))
        // Buy it when there is something to buy, the thread's menu, the viewers in the corner (the person's order, 11-04)
        .toolbar {
            if let b = buy {
                ToolbarItem(placement: .topBarTrailing) {
                    Button { startBuy() } label: { Image(systemName: "cart").foregroundStyle(BackplaneDesign.accent) }.disabled(buying).accessibilityLabel(Tr.t(b.label)).accessibilityIdentifier("data.thread.buy")
                }.dflat()
            }
            if let t = thread, client.has("actions") {
                ToolbarItem(placement: .topBarTrailing) {
                    Menu {
                        if let b = buy { Button(Tr.t(b.label), systemImage: "cart") { startBuy() } }
                        // the find bar when the hub has `find`; else the older sheet of hits
                        if client.has("find") { Button(Tr.k("ios.find.bar"), systemImage: "magnifyingglass") { findOpen = true; findFocus = true } }
                        else if client.has("search") { Button(Tr.t("Search in thread"), systemImage: "magnifyingglass") { finding = true } }
                        DataThreadActions(client: client, thread: t, renaming: $renaming, deleting: $deleting)
                    } label: { Image(systemName: "ellipsis").foregroundStyle(BackplaneDesign.dim) }
                        .accessibilityLabel(Tr.t("Thread actions")).accessibilityIdentifier("data.thread.more")
                }.dflat()
            }
            if client.has("viewers") {
                ToolbarItem(placement: .topBarTrailing) {
                    NavigationLink(value: DViewerRoute(id: id)) { Image(systemName: "square.grid.2x2").foregroundStyle(BackplaneDesign.dim) }.accessibilityLabel(Tr.t("Viewer")).accessibilityIdentifier("data.thread.viewer")
                }.dflat()
            }
        }
        .sheet(isPresented: $finding) { DataFindInThread(client: client, thread: id) { m in finding = false; findGo = m } }
        .modifier(DataThreadDialogs(client: client, renaming: $renaming, deleting: $deleting, deleted: { dismiss() }))
        .sheet(isPresented: $modelSheet) { DataPickerSheet(client: client, thread: id, choice: $noChoice) }
        // full screen, not a sheet: a pen stroke downward must not swipe it away
        .fullScreenCover(isPresented: $drawing) { DataPadView { add([$0]) } }
        .photosPicker(isPresented: $photoLibrary, selection: $picked, maxSelectionCount: max(1, 4 - pending.count), matching: .images)
        .fullScreenCover(isPresented: $camera) { DCamera { d in camera = false; if let d { add([d]) } }.ignoresSafeArea() }
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
        .task(id: at) { if let at { try? await Task.sleep(for: .milliseconds(300)); findGo = at } }
        // the find bar: the hub's `find` a moment after typing stops, the newest match first
        .task(id: findOpen ? findQ : "") {
            guard findOpen, ThreadFind.asks(findQ) else { findRes = nil; findAt = nil; return }
            try? await Task.sleep(for: .milliseconds(250)); if Task.isCancelled { return }
            let r = await client.find(id, findQ.trimmingCharacters(in: .whitespacesAndNewlines)); if Task.isCancelled { return }
            findRes = r; findAt = ThreadFind.first(count: r?.matches.count ?? 0); if findAt == nil { mark = nil }; findGoTo()
        }
        .task(id: id) { await client.refreshSecretAsks(id) }
    }

    // find in this thread: the words, "n of total", previous and next (older, newer), close (which clears the mark)
    private var findBar: some View {
        VStack(spacing: 0) {
            HStack(spacing: 4) {
                TextField(Tr.k("ios.find.bar"), text: $findQ).dmono(13).textInputAutocapitalization(.never).autocorrectionDisabled().focused($findFocus).submitLabel(.search)
                    .onSubmit { findStep(-1) }
                    .padding(.horizontal, 8).frame(height: 32).dbox().accessibilityIdentifier("data.find.bar")
                Text(Tr.t(ThreadFind.label(findAt, total: findRes?.total, more: findRes?.more ?? false))).dmono(11).foregroundStyle(BackplaneDesign.dim).fixedSize().accessibilityIdentifier("data.find.count")
                let none = (findRes?.matches.isEmpty ?? true)
                Button { findStep(-1) } label: { Image(systemName: "chevron.up").frame(width: 32, height: 32) }.disabled(none).accessibilityLabel(Tr.k("ios.find.prev")).accessibilityIdentifier("data.find.prev")
                Button { findStep(1) } label: { Image(systemName: "chevron.down").frame(width: 32, height: 32) }.disabled(none).accessibilityLabel(Tr.k("ios.find.next")).accessibilityIdentifier("data.find.next")
                Button { findClose() } label: { Image(systemName: "xmark").frame(width: 32, height: 32) }.accessibilityLabel(Tr.k("ios.find.close")).accessibilityIdentifier("data.find.close")
            }.foregroundStyle(BackplaneDesign.dim).padding(.horizontal, 10).padding(.vertical, 4).background(BackplaneDesign.panel)
            DLine()
        }
    }

    // the desktop's strip under the bar: STATE, then the tools a thread always has
    private var stateStrip: some View {
        VStack(spacing: 0) {
            HStack(spacing: 6) {
                DHeading(text: Tr.t("State"))
                if let t = thread { DStatusDot(thread: t, size: 6); Text(Tr.t(dStatusWord(t))).dmono(11).foregroundStyle(BackplaneDesign.text).accessibilityIdentifier("data.thread.state") }
                Spacer()
                if let t = thread, client.has("actions") {
                    strip(Tr.t(t.pinned ? "Unpin" : "Pin"), on: t.pinned) { Task { await client.threadAct(id, t.pinned ? "unpin" : "pin") } }
                    strip(Tr.t(t.settled ? "Unsettle" : "Settle"), on: t.settled) { Task { await client.threadAct(id, t.settled ? "unsettle" : "settle") } }
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
                if !notice.isEmpty { Text(Tr.t(notice)).dmono(12).foregroundStyle(BackplaneDesign.warn).accessibilityIdentifier("data.images.notice") }
                if !pending.isEmpty {
                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack(spacing: 8) {
                            ForEach(Array(pending.enumerated()), id: \.offset) { i, d in
                                ZStack(alignment: .topTrailing) {
                                    if let u = UIImage(data: d) { Image(uiImage: u).resizable().scaledToFill().frame(width: 64, height: 64).clipped().overlay(Rectangle().strokeBorder(BackplaneDesign.line, lineWidth: 1)) }
                                    Button { pending.remove(at: i) } label: { Image(systemName: "xmark").font(.system(size: 10, weight: .bold)).foregroundStyle(BackplaneDesign.text).frame(width: 20, height: 20).background(BackplaneDesign.raised) }
                                        .accessibilityLabel(Tr.t("Remove picture"))
                                }
                            }
                        }
                    }.accessibilityIdentifier("data.pending.images")
                }
                UIKitField(text: text, placeholder: Tr.t("Ask anything: @bot, >thread, %project, #room, &model, $skill, /btw"), editable: true,
                           onChange: { text = $0; client.setDraft(id, $0) },
                           onSend: { send() }, onPasteImages: { imgs in add(imgs.compactMap { $0.jpegData(compressionQuality: 0.95) }) },
                           font: UIFontMetrics(forTextStyle: .body).scaledFont(for: Mono.uiFont(size: 14)), caret: caret)
                    .padding(.horizontal, 10).padding(.vertical, 9).frame(minHeight: 44, alignment: .topLeading).dbox(BackplaneDesign.accent)
                    .accessibilityLabel(Tr.t("Message"))
                HStack(spacing: 6) {
                    if client.has("picker") || thread?.chip.isEmpty == false {
                        Button { modelSheet = true } label: { Text(chip).dmono(12).foregroundStyle(BackplaneDesign.text).lineLimit(1).padding(.horizontal, 10).frame(maxWidth: .infinity, minHeight: 40, alignment: .leading).dbox(fill: BackplaneDesign.panel) }
                            .buttonStyle(.plain).accessibilityLabel(Tr.ios_model_label(model: chip)).accessibilityIdentifier("data.thread.model")
                    } else { Spacer(minLength: 0) }
                    if client.has("voice") {
                        let on = client.voice.listening(id)
                        Button { micPress() } label: {
                            Image(systemName: on ? "mic.fill" : "mic").font(.system(size: 15)).foregroundStyle(on ? BackplaneDesign.bad : BackplaneDesign.dim).frame(width: 40, height: 40).dbox(fill: BackplaneDesign.panel)
                        }
                        .buttonStyle(.plain).accessibilityLabel(Tr.t(on ? "Stop dictation" : "Dictate")).accessibilityIdentifier("data.mic")
                    }
                    // Attach: a photo from the library, or one taken now with the camera
                    Menu {
                        Button(Tr.t("Choose a photo"), systemImage: "photo") { photoLibrary = true }
                        if UIImagePickerController.isSourceTypeAvailable(.camera) { Button(Tr.t("Take a photo"), systemImage: "camera") { camera = true } }
                    } label: {
                        Image(systemName: "paperclip").font(.system(size: 15)).foregroundStyle(BackplaneDesign.dim).frame(width: 40, height: 40).dbox(fill: BackplaneDesign.panel)
                    }
                    .disabled(!client.canSendImages || pending.count >= 4)
                    .accessibilityLabel(Tr.t(client.canSendImages ? "Attach pictures" : "Attach pictures (this hub does not take pictures yet)")).accessibilityIdentifier("data.attach")
                    Button { if client.canSendImages { drawing = true } else { notice = "This hub does not take pictures yet: update it, then draw again." } } label: {
                        Image(systemName: "pencil.tip").font(.system(size: 15)).foregroundStyle(client.canSendImages ? BackplaneDesign.dim : BackplaneDesign.faint).frame(width: 40, height: 40).dbox(fill: BackplaneDesign.panel)
                    }
                    .disabled(pending.count >= 4)
                    .accessibilityLabel(Tr.t("Draw")).accessibilityIdentifier("data.draw")
                    let blank = text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && pending.isEmpty
                    if working && blank {
                        Button(Tr.t("Stop")) { Task { await client.interrupt(id) } }.buttonStyle(DButton(danger: true)).accessibilityLabel(Tr.t("Stop")).accessibilityIdentifier("thread.send")
                    } else {
                        Button(Tr.t(client.commandOf(text)?.label ?? "Send")) { send() }.buttonStyle(DButton(primary: true)).disabled(blank).accessibilityIdentifier("thread.send")
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
    // a command of the composer: the hub runs it; a refusal leaves the draft and says why
    private func run(_ t: String) {
        notice = ""
        Task {
            do {
                let r = try await client.runCommand(id, t.trimmingCharacters(in: .whitespacesAndNewlines))
                switch r.kind {
                case "edit": text = r.text ?? ""; client.setDraft(id, text)
                case "secret": box = r.name ?? ""; text = ""; client.setDraft(id, "")
                case "intent": notice = "Intent saved"; text = ""; client.setDraft(id, "")
                default: text = ""; client.setDraft(id, "")
                }
            } catch { notice = (error as? DError)?.message ?? "That could not be done." }
        }
    }
    private func send() {
        let t = text
        guard !t.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || !pending.isEmpty else { return }
        if client.commandOf(t) != nil { run(t); return }
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
// The thread's background work: a head that folds, its rows, and the Release, Cancel or Stop a row offers. Open while any is at work.
/// What waits (the desktop's queue strip over the composer, docs/PHONE-DATA.md `thread.queue`): each row's tag and first line, and the buttons it has.
/// The hub's rows (Queued/Held with Steer, Edit, Remove; a thread elsewhere's Sending, Unsent, Refused with Edit and Remove) and this phone's own
/// Unsent ones (Outbox.swift) look the same; DataClient.queueAct sends each press where it belongs.
struct DataQueueStrip: View {
    @Bindable var client: DataClient
    let thread: String
    let rows: [DQueueRow]
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            ForEach(rows) { r in
                HStack(spacing: 8) {
                    Text(Tr.t(r.tag).uppercased()).dmono(10).foregroundStyle(r.tag == "Refused" ? BackplaneDesign.bad : BackplaneDesign.faint).accessibilityIdentifier("queue.tag")
                    Text(r.text).dmono(12).foregroundStyle(BackplaneDesign.text).lineLimit(2).frame(maxWidth: .infinity, alignment: .leading).accessibilityIdentifier("queue.text")
                    ForEach(r.buttons.filter { $0.action != "q-up" }, id: \.self) { b in
                        Button { Task { await client.queueAct(thread, b) } } label: {
                            Text(Tr.t(b.label)).dmono(12).foregroundStyle(b.action == "q-steer" ? BackplaneDesign.accent : b.action.hasSuffix("-remove") ? BackplaneDesign.bad : BackplaneDesign.dim)
                                .padding(.horizontal, 10).frame(minHeight: 30).overlay(Rectangle().strokeBorder(BackplaneDesign.line, lineWidth: 1)).contentShape(Rectangle())
                        }.buttonStyle(.plain).accessibilityIdentifier("queue.\(b.action)")
                    }
                }.accessibilityElement(children: .contain).accessibilityIdentifier("queue.row")
            }
            if let e = client.queueError[thread] { Text(Tr.t(e)).dmono(11).foregroundStyle(BackplaneDesign.bad).accessibilityIdentifier("queue.error") }
        }
        .padding(.horizontal, 14).padding(.vertical, 6).frame(maxWidth: .infinity, alignment: .leading).background(BackplaneDesign.panel)
        .overlay(alignment: .top) { DLine() }
        .accessibilityElement(children: .contain).accessibilityIdentifier("thread.queue")
    }
}

struct DataWorkPanel: View {
    @Bindable var client: DataClient
    let thread: String
    let work: DWork
    @State private var chosen: Bool?
    private var open: Bool { chosen ?? (work.live > 0) }
    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Button { chosen = !open } label: { HStack { Text(Tr.ios_work_head(word: Tr.t(work.word)).uppercased()).dmono(11).foregroundStyle(BackplaneDesign.dim); Spacer() } }
                .accessibilityIdentifier("data.work.head")
            if open {
                ForEach(work.rows, id: \.self) { r in
                    HStack(spacing: 8) {
                        // a subagent's row opens its own view (feature subview)
                        if !r.subKey.isEmpty && client.has("subview") {
                            NavigationLink(value: DSubRoute(thread: thread, key: r.subKey)) {
                                HStack(spacing: 6) {
                                    VStack(alignment: .leading, spacing: 1) {
                                        Text(r.title).dmono(12).lineLimit(1).foregroundStyle(BackplaneDesign.text)
                                        if let m = r.meta, !m.isEmpty { Text(m).dmono(11).foregroundStyle(BackplaneDesign.faint).lineLimit(1) }
                                        Text(r.doing.isEmpty ? Tr.t(r.kind) + " · " + Tr.t(r.state) : Tr.t(r.doing)).dmono(11).foregroundStyle(BackplaneDesign.dim).lineLimit(1)
                                    }
                                    Spacer(minLength: 0)
                                    Image(systemName: "chevron.right").font(.system(size: 11)).foregroundStyle(BackplaneDesign.accent)
                                }.contentShape(Rectangle())
                            }.buttonStyle(.plain).accessibilityIdentifier("data.work.sub.\(r.subKey)")
                        } else {
                        VStack(alignment: .leading, spacing: 1) {
                            Text(r.title).dmono(12).lineLimit(1)
                            Text(r.doing.isEmpty ? Tr.t(r.kind) + " · " + Tr.t(r.state) : Tr.t(r.doing)).dmono(11).foregroundStyle(BackplaneDesign.dim).lineLimit(1)
                        }
                        Spacer()
                        }
                        if let label = r.label, !label.isEmpty {
                            Button { client.workAct(thread, r) } label: { Text(Tr.t(label)).dmono(12) }.accessibilityIdentifier("data.work.\(r.act ?? "").\(r.value ?? "")")
                        }
                    }
                }
            }
        }
        .padding(.horizontal, 14).padding(.vertical, 6).background(BackplaneDesign.panel)
    }
}

struct DataFoldRow: View {
    let fold: DFold
    @Binding var open: Bool
    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Button { open.toggle() } label: {
                HStack(spacing: 8) {
                    Image(systemName: open ? "chevron.down" : "chevron.right").font(.system(size: 11)).foregroundStyle(BackplaneDesign.dim).frame(width: 12)
                    Text(Tr.t(fold.label)).dmono(12).foregroundStyle(BackplaneDesign.dim).lineLimit(1)
                    Spacer(minLength: 0)
                }.padding(.horizontal, 8).frame(minHeight: 36).overlay(Rectangle().strokeBorder(BackplaneDesign.line, lineWidth: 1)).contentShape(Rectangle())
            }
            .buttonStyle(.plain).accessibilityLabel(Tr.t(fold.label)).accessibilityValue(Tr.t(open ? "open" : "shut")).accessibilityIdentifier("data.fold." + fold.id)
            if open { ForEach(fold.items) { DataToolRow(entry: $0) }.padding(.leading, 8) }
        }.accessibilityIdentifier("data.entry.tool")
    }
}

struct DataToolRow: View {
    let entry: DEntry
    @State private var whole = false
    @Environment(\.dataSubThread) private var subThread
    private var name: String { entry.kind == "step" ? "" : entry.tool?.name ?? "tool" }
    private var what: String { entry.kind == "step" ? Tr.t(entry.label.isEmpty ? "Design change" : entry.label) : (entry.tool?.summary ?? "").isEmpty ? "" : entry.text }
    var body: some View {
        // an Agent call opens its subagent's own view (feature subview)
        if let sub = entry.tool?.sub, !sub.isEmpty, !subThread.isEmpty {
            NavigationLink(value: DSubRoute(thread: subThread, key: sub)) {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Text(name).dmono(12, bold: true).foregroundStyle(BackplaneDesign.text)
                    Text(what).dmono(12).foregroundStyle(BackplaneDesign.dim).lineLimit(2)
                    Spacer(minLength: 0)
                    Text(Tr.k("ios.handoff.open")).dmono(12).foregroundStyle(BackplaneDesign.accent); Image(systemName: "chevron.right").font(.system(size: 11)).foregroundStyle(BackplaneDesign.accent)
                }.padding(.vertical, 3).contentShape(Rectangle())
            }.buttonStyle(.plain).accessibilityIdentifier("data.tool.sub.\(sub)")
        } else {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            if !name.isEmpty { Text(name).dmono(12, bold: true).foregroundStyle(BackplaneDesign.text) }
            Text(what).dmono(12).foregroundStyle(BackplaneDesign.dim).lineLimit(whole ? nil : 3).textSelection(.enabled)
            Spacer(minLength: 0)
        }.padding(.vertical, 3).contentShape(Rectangle()).onTapGesture { whole.toggle() }
        }
    }
}

private let dClock: DateFormatter = { let f = DateFormatter(); f.dateFormat = "HH:mm"; return f }()
private let dDayEn: DateFormatter = { let f = DateFormatter(); f.locale = Locale(identifier: "en_US_POSIX"); f.dateFormat = "EEE d MMM HH:mm"; return f }()
private let dDayJa: DateFormatter = { let f = DateFormatter(); f.locale = Locale(identifier: "ja_JP"); f.dateFormat = "M月d日(EEE) HH:mm"; return f }()
/// A message's time: the clock today, the day too on another (in the app's language).
func dStamp(_ ms: Int) -> String {
    guard ms > 0 else { return "" }
    let d = Date(timeIntervalSince1970: Double(ms) / 1000)
    return (Calendar.current.isDateInToday(d) ? dClock : Tr.lang == "ja" ? dDayJa : dDayEn).string(from: d)
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
                        if !entry.text.isEmpty { DLinkText(text: entry.text, thread: thread).foregroundStyle(BackplaneDesign.text).lineSpacing(3) }
                    }.padding(.horizontal, 10).padding(.vertical, 8).dbox(fill: BackplaneDesign.raised)
                    if entry.sending { Text(Tr.t("Sending…")).dmono(11).foregroundStyle(BackplaneDesign.faint) }
                    else if entry.failed { Button(Tr.t("Not sent · Retry")) { client.retry(thread, entry) }.dmono(11).foregroundStyle(BackplaneDesign.bad) }
                    else { Text(dStamp(entry.at)).dmono(11).foregroundStyle(BackplaneDesign.faint) }
                }
            }.padding(.vertical, 4).accessibilityIdentifier("data.entry.user")
        case "assistant":
            VStack(alignment: .leading, spacing: 4) {
                DataMarkdown(text: entry.text, thread: thread)
                // the pictures the reply names (the hub lists them as attachments); tap for the full size
                ForEach(entry.attachments) { a in DataAttachmentImage(client: client, attachment: a) }
                if !entry.live { Text(dStamp(entry.at)).dmono(11).foregroundStyle(BackplaneDesign.faint) }
            }.padding(.vertical, 4).frame(maxWidth: .infinity, alignment: .leading).accessibilityIdentifier("data.entry.assistant")
        case "tool", "step":
            DataToolRow(entry: entry).accessibilityIdentifier("data.entry.tool")
        case "ask":
            DataAskCard(client: client, thread: thread, entry: entry)
        case "term" where client.has("terms") && entry.term != nil:
            DataTermCard(client: client, info: entry.term!)
        case "handoff" where entry.handoff != nil:
            DataHandoffCard(client: client, thread: thread, handoff: entry.handoff!)
        default:
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                if !entry.label.isEmpty { Text(Tr.t(entry.label)).dmono(12, bold: true) }
                DLinkText(text: Tr.t(entry.text), size: 12, thread: thread)
                Spacer(minLength: 0)
            }.foregroundStyle(entry.tone == "error" ? BackplaneDesign.bad : BackplaneDesign.dim).padding(.vertical, 3)
        }
    }
}

/// A `/give` received: the way back, the summary, the note, and what the work touched, each reference opening what it names (the hub's `link.resolve`).
struct DataHandoffCard: View {
    let client: DataClient
    let thread: String
    let handoff: DHandoff
    private func open(_ r: DHandoffRef) {
        let file = r.kind == "file" || r.kind == "board"
        let text = file ? dPathScheme + ":" + (r.url.addingPercentEncoding(withAllowedCharacters: .urlPathAllowed) ?? r.url) : r.url
        if let u = URL(string: text) { client.openLink(u, thread: r.thread) }
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Button { if let u = URL(string: "backplane://thread/\(handoff.from)") { client.openLink(u, thread: thread) } } label: { Text(Tr.ios_handoff_from(title: handoff.title)).dmono(11).foregroundStyle(BackplaneDesign.accent).lineLimit(1) }
                .accessibilityIdentifier("handoff.back")
            Text(handoff.summary).dmono(13).foregroundStyle(BackplaneDesign.text).accessibilityIdentifier("handoff.summary")
            if !handoff.note.isEmpty { Text(Tr.ios_handoff_note(note: handoff.note)).dmono(12).foregroundStyle(BackplaneDesign.dim) }
            ForEach(Array(handoff.refs.enumerated()), id: \.offset) { i, r in
                Button { open(r) } label: {
                    HStack(spacing: 8) {
                        Text(r.kind == "thread" ? "▸" : r.kind == "message" ? "“" : r.kind == "board" ? "▦" : "·").dmono(12, bold: true).foregroundStyle(BackplaneDesign.accent)
                        Text(r.label).dmono(12).foregroundStyle(BackplaneDesign.text).lineLimit(1)
                        Spacer(minLength: 0)
                    }.padding(.horizontal, 8).padding(.vertical, 6).dbox(fill: BackplaneDesign.panel)
                }.buttonStyle(.plain).accessibilityIdentifier("handoff.ref.\(i)")
            }
        }.padding(10).frame(maxWidth: .infinity, alignment: .leading).dbox(fill: BackplaneDesign.raised).padding(.vertical, 4).accessibilityElement(children: .contain).accessibilityIdentifier("handoff.card")
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
            DHeading(text: Tr.t(ask?.isOpen == true ? "Asks" : "Asked"))
            Text(ask?.question ?? entry.text).dmono(13).foregroundStyle(BackplaneDesign.text).textSelection(.enabled)
            if ask?.isOpen == true {
                if (ask?.options ?? []).isEmpty {
                    HStack(spacing: 6) {
                        TextField(Tr.t("Your answer"), text: $answer).dmono(13).padding(.horizontal, 10).frame(height: 40).dbox()
                        Button(Tr.t("Send")) { Task { await client.answer(thread, ask: ask!.id, option: answer) } }.buttonStyle(DButton(primary: true)).disabled(answer.isEmpty)
                    }
                } else {
                    VStack(alignment: .leading, spacing: 6) { ForEach(ask!.options) { o in
                        Button { Task { await client.answer(thread, ask: ask!.id, option: o.id) } } label: { Text(Tr.t(o.label)).frame(maxWidth: .infinity, alignment: .leading) }
                            .buttonStyle(DButton(primary: o.id == "accept", danger: o.id == "deny" || o.id == "decline"))
                            .accessibilityIdentifier("data.ask." + o.id)
                    } }
                }
            } else if let a = ask?.answered { Text(Tr.ios_ask_answered(answer: Tr.t(a))).dmono(12).foregroundStyle(BackplaneDesign.dim) }
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
    // where it goes: the hub searches every machine's projects and says the last one chosen (improvements.projects); a pick is remembered there
    @State private var q = ""
    @State private var rows: [DImpProject] = []

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                DHeading(text: Tr.t("What would you like to improve?"))
                TextEditor(text: $text).dmono(13).scrollContentBackground(.hidden).padding(6).frame(minHeight: 140).dbox(BackplaneDesign.accent).accessibilityIdentifier("improvements.text")
                Text(Tr.t("A public GitHub issue draft on BackplaneWorks/Backplane opens in your browser, filled with this text. You review it and press Submit there; nothing is sent from here. Do not include private information.")).dmono(11).foregroundStyle(BackplaneDesign.dim)
                Button(Tr.t("Open a GitHub issue draft")) { openURL(issueURL) }.buttonStyle(DButton())
                    .disabled(text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty).accessibilityIdentifier("improvements.github")
                if !client.projects.isEmpty || !rows.isEmpty {
                    DLine()
                    DHeading(text: Tr.t("Project"))
                    TextField(Tr.t("Search projects…"), text: $q).dmono(13).textInputAutocapitalization(.never).autocorrectionDisabled()
                        .padding(.horizontal, 10).frame(height: 36).dbox().accessibilityIdentifier("improvements.search")
                    if rows.isEmpty { Text(Tr.t("No project matches.")).dmono(12).foregroundStyle(BackplaneDesign.dim) }
                    ForEach(rows.prefix(12)) { r in
                        Button { project = r.id; Task { _ = try? await client.call("improvements.project", ["project": r.id], as: DEmpty.self) } } label: {
                            Text((r.id == project ? "● " : "○ ") + r.title + (r.short.isEmpty ? "" : " · \(r.short)")).dmono(13).foregroundStyle(BackplaneDesign.text).frame(maxWidth: .infinity, alignment: .leading)
                        }.accessibilityIdentifier("improvements.project.\(r.id)")
                    }
                    Button(Tr.t("Send to an agent thread")) {
                        let p = project
                        Task { if (try? await client.createThread(project: p, text: "Improvement idea for this app:\n\n" + text)) != nil { sent = "Thread started in Threads."; text = "" } }
                    }
                    .buttonStyle(DButton(primary: true))
                    .disabled(text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || client.conn != .online || project.isEmpty).accessibilityIdentifier("improvements.thread")
                    if !sent.isEmpty { Text(Tr.t(sent)).dmono(12).foregroundStyle(BackplaneDesign.dim) }
                    Text(Tr.t("Starts a thread on your hub in the chosen project with this text.")).dmono(11).foregroundStyle(BackplaneDesign.dim)
                }
            }.padding(16)
        }
        .background(BackplaneDesign.background)
        .dataBar(Tr.t("Improvements"))
        .task(id: q) {
            if !q.isEmpty { try? await Task.sleep(for: .milliseconds(250)) }
            guard let r = try? await client.call("improvements.projects", ["q": q], as: DImpProjects.self) else { return }
            rows = r.projects
            if project.isEmpty { project = r.project.isEmpty ? (r.projects.first?.id ?? "") : r.project }
        }
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
    @State private var forgetKey: String?

    private func onOff(_ title: String, _ on: Binding<Bool>, id: String) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            DHeading(text: title)
            DChoices(options: [(label: Tr.t("on"), value: true), (label: Tr.t("off"), value: false)], value: on).accessibilityIdentifier(id)
        }
    }
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                DHeading(text: Tr.k("settings.lang"))
                DChoices(options: [(label: Tr.k("settings.lang.auto"), value: "auto"), (label: Tr.k("settings.lang.en"), value: "en"), (label: Tr.k("settings.lang.ja"), value: "ja")], value: $settings.lang).accessibilityIdentifier("settings.lang")
                Text(Tr.k("settings.lang.note")).dmono(11).foregroundStyle(BackplaneDesign.dim)
                DHeading(text: Tr.t("Appearance"))
                DChoices(options: [(label: Tr.t("system"), value: "system"), (label: Tr.t("light"), value: "light"), (label: Tr.t("dark"), value: "dark")], value: $settings.appearance)
                DataViewerBackgroundSection(settings: settings)
                DHeading(text: Tr.t("Text size"))
                DChoices(options: [(label: Tr.t("system"), value: "system")] + AppSettings.textSteps.map { (label: $0.tag, value: $0.tag) }, value: $settings.textSize).accessibilityIdentifier("settings.textsize")
                onOff(Tr.t("Alerts on this phone"), $settings.alerts, id: "settings.alerts")
                    .onChange(of: settings.alerts) { _, on in Task { if on { await client.enablePush() }; await client.pushMute(); if client.has("notifysettings") { await client.notify.load() } } }
                if settings.alerts { onOff(Tr.t("Alert sound"), $settings.alertSound, id: "settings.sound") }
                onOff(Tr.t("Haptics"), $settings.haptics, id: "settings.haptics")
                Text(Tr.t("Off: only what you let through reaches this phone. Alerts come from your uberbot.")).dmono(11).foregroundStyle(BackplaneDesign.dim)
                // an older hub has no Notifications page: its quiet hours and urgent switch are drawn here
                if client.has("push") && !client.has("notifysettings") { DLine(); DataQuietSection(client: client) }
                DLine()
                DHeading(text: Tr.t("Hubs"))
                let keys = client.links.compactMap { Pairing.key($0) }
                ForEach(keys.indices, id: \.self) { i in
                    let k = keys[i]
                    // the hub connected to now: the one in use, or the home hub while that one cannot be reached
                    let on = k == client.usingKey
                    HStack(spacing: 8) {
                        if on { DDot(color: client.conn == .online ? BackplaneDesign.ok : client.conn == .connecting ? BackplaneDesign.warn : BackplaneDesign.bad, size: 8) }
                        Text(k).dmono(13).lineLimit(1)
                        if k == client.homeKey { Text(Tr.t("Home")).dmono(11).foregroundStyle(BackplaneDesign.dim).accessibilityIdentifier("settings.hub.home.\(k)") }
                        Spacer()
                        if on { Text(Tr.t(client.conn.rawValue)).dmono(11).foregroundStyle(BackplaneDesign.dim) }
                        else if i == 0, client.onHome { Text(Tr.t("Cannot be reached")).dmono(11).foregroundStyle(BackplaneDesign.dim) }
                        else if i > 0 { Button(Tr.t("Use")) { client.useHub(k) }.buttonStyle(DButton()).accessibilityIdentifier("settings.hub.use.\(k)") }
                        Button(Tr.t("Forget")) { forgetKey = k }.buttonStyle(DButton(danger: true)).accessibilityIdentifier("settings.hub.forget.\(k)")
                    }
                }
                if client.links.isEmpty { Text(Tr.t("Not paired")).dmono(13) }
                if !client.failure.isEmpty, client.conn != .online { Text(Tr.t(client.failure)).dmono(11).foregroundStyle(BackplaneDesign.dim) }
                HStack(spacing: 6) {
                    Button(Tr.t("Reconnect")) { client.connect() }.buttonStyle(DButton())
                    Button(Tr.t("Add a hub")) { client.pairingError = ""; client.adding = true }.buttonStyle(DButton()).accessibilityIdentifier("settings.hub.add")
                }
                NavigationLink(value: DPage(name: "hub")) { Text(Tr.t("Hub")) }.buttonStyle(DButton()).accessibilityIdentifier("settings.hub")
                if client.has("accounts") { NavigationLink(value: DPage(name: "accounts")) { Text(Tr.t("Accounts")) }.buttonStyle(DButton()).accessibilityIdentifier("settings.accounts") }
                DLine()
                DataUpdateSection(client: client)
                DLine()
                if client.has("voice") { DataVoiceSection(client: client); DLine() }
                if client.has("devices") { DataDevicesSection(client: client); DLine() }
                if client.has("machines") { DataMachinesSection(client: client); DLine() }
                if client.has("notifysettings") { DataNotifySection(client: client); DLine() }
                if client.has("ring") { DataRingSection(client: client); DLine() }
                NavigationLink(value: DPage(name: "improvements")) { Text(Tr.t("Improvements")) }.buttonStyle(DButton()).accessibilityIdentifier("settings.improve")
                DLine()
                let i = Bundle.main.infoDictionary
                Text(Tr.ios_version(version: i?["CFBundleShortVersionString"] as? String ?? "?", build: i?["CFBundleVersion"] as? String ?? "?")).dmono(11).foregroundStyle(BackplaneDesign.faint)
            }.padding(16)
        }
        .background(BackplaneDesign.background)
        .dataBar(Tr.t("Settings"))
        .confirmationDialog(Tr.ios_hub_forget_ask(hub: forgetKey ?? Tr.t("this hub")), isPresented: Binding(get: { forgetKey != nil }, set: { if !$0 { forgetKey = nil } }), titleVisibility: .visible) {
            Button(Tr.t("Forget"), role: .destructive) { if let k = forgetKey { client.forgetHub(k) }; forgetKey = nil }
        } message: { Text(Tr.t("This phone's saved copy of its threads is removed. Pair again to come back.")) }
    }
}

// the 3D viewers' background (Mech and the 3D board share it), on this phone: auto, light, dark, or a colour of one's own
struct DataViewerBackgroundSection: View {
    @Bindable var settings: AppSettings
    private var colour: Binding<Color> {
        Binding(get: {
            let (r, g, b) = ViewerBackdrop.unit(ViewerBackdrop.parse(settings.viewerBgCustom) ?? ViewerBackdrop.defaultCustom)
            return Color(.sRGB, red: Double(r), green: Double(g), blue: Double(b))
        }, set: { c in
            var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
            guard UIColor(c).getRed(&r, green: &g, blue: &b, alpha: &a) else { return }
            func byte(_ x: CGFloat) -> UInt32 { UInt32((min(max(x, 0), 1) * 255).rounded()) }
            settings.viewerBgCustom = ViewerBackdrop.hex(byte(r) << 16 | byte(g) << 8 | byte(b))
            settings.viewerBg = "custom"
        })
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            DHeading(text: Tr.k("ios.viewerbg"))
            DChoices(options: [(label: Tr.k("ios.viewerbg.auto"), value: "auto"), (label: Tr.k("settings.theme.light"), value: "light"),
                               (label: Tr.k("settings.theme.dark"), value: "dark"), (label: Tr.k("ios.viewerbg.custom"), value: "custom")],
                     value: $settings.viewerBg).accessibilityIdentifier("settings.viewerbg")
            ColorPicker(Tr.k("ios.viewerbg.color"), selection: colour, supportsOpacity: false)
                .dmono(13).accessibilityIdentifier("settings.viewerbg.color")
            Text(Tr.k("ios.viewerbg.note")).dmono(11).foregroundStyle(BackplaneDesign.dim)
        }
    }
}

// the hub's updates: the hub says what to show (update.status); this draws it
struct DataUpdateSection: View {
    @Bindable var client: DataClient
    @State private var status: DUpdate?
    private func load(_ m: String, _ p: [String: Any] = [:]) async { if let r: DUpdate = try? await client.call(m, p) { status = r } }
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            DHeading(text: Tr.t("Updates"))
            if let u = status {
                Text(verbatim: "Backplane \(u.current)").dmono(13)
                Text(Tr.t(u.line)).dmono(11).foregroundStyle(BackplaneDesign.dim).accessibilityIdentifier("settings.update.line")
                DChoices(options: [(label: Tr.t("stable"), value: "stable"), (label: Tr.t("beta"), value: "beta")],
                         value: Binding(get: { u.channel }, set: { c in Task { await load("update.channel", ["channel": c]) } })).accessibilityIdentifier("settings.update.channel")
                if u.canCheck {
                    Button(Tr.t("Check now")) { Task { await load("update.check"); try? await Task.sleep(for: .seconds(3)); await load("update.status") } }.buttonStyle(DButton()).accessibilityIdentifier("settings.update.check")
                }
            } else { Text(Tr.t("Not connected")).dmono(11).foregroundStyle(BackplaneDesign.dim) }
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
            Text(order.name.isEmpty ? Tr.t("Order").uppercased() : Tr.ios_order_named(name: order.name).uppercased()).dmono(12, bold: true).foregroundStyle(BackplaneDesign.text)
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 6) {
                    ForEach(Array(order.steps.enumerated()), id: \.offset) { i, s in
                        if i > 0 { Text("›").dmono(11).foregroundStyle(BackplaneDesign.faint) }
                        switch s.mark {
                        case "done": Text("✓ " + Tr.t(s.label)).dmono(11).foregroundStyle(BackplaneDesign.dim)
                        case "on": Text(Tr.t(s.label)).dmono(11, bold: true).foregroundStyle(BackplaneDesign.accentText)
                        default: Text(Tr.t(s.label)).dmono(11).foregroundStyle(BackplaneDesign.faint)
                        }
                    }
                }
            }
            Text(Tr.t(order.line)).dmono(12).foregroundStyle(order.waiting ? BackplaneDesign.dim : BackplaneDesign.text)
            if !order.total.isEmpty { Text(Tr.t(order.total)).dmono(14, bold: true).foregroundStyle(BackplaneDesign.text) }
            if !order.customs.isEmpty { Text(Tr.t(order.customs)).dmono(11).foregroundStyle(BackplaneDesign.dim) }
            if let u = order.payURL { Button(Tr.t(order.label.isEmpty ? "Buy now" : order.label)) { openURL(u) }.buttonStyle(DButton(primary: true)).accessibilityIdentifier("thread.order.pay") }
        }
        .padding(10).frame(maxWidth: .infinity, alignment: .leading).background(BackplaneDesign.panel).overlay(Rectangle().strokeBorder(BackplaneDesign.line, lineWidth: 1))
        .padding(.horizontal, 12).padding(.vertical, 6).accessibilityElement(children: .contain).accessibilityIdentifier("thread.order") // a container: Buy now keeps its own identifier
    }
}

// A link from outside the app names the hub it would pair with; Cancel comes first.
struct AskPairView: View {
    let client: DataClient
    let link: String
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(Tr.t("Pair with this hub?")).font(.title2.weight(.semibold))
            Text(verbatim: Pairing.invitation(link)?.origin.absoluteString ?? Pairing.key(link) ?? "").font(.system(.body, design: .monospaced))
            Text(Tr.t("A link opened this. Pair only with a hub you know.")).font(.footnote).foregroundStyle(.secondary)
            HStack(spacing: 8) {
                Button(Tr.t("Cancel")) { client.askLink = nil }.buttonStyle(PairActionStyle()).accessibilityIdentifier("pair.ask.cancel")
                Button(Tr.t("Pair")) { client.askLink = nil; client.pair(link) }.buttonStyle(.bordered).buttonBorderShape(.roundedRectangle(radius: 0)).accessibilityIdentifier("pair.ask.yes")
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

/// A machine's word with " · away" when it does not answer (the hub's `away`), in the language on show.
func dAway(_ label: String, _ away: Bool) -> String { away ? Tr.ios_machine_away(label: label) : label }
