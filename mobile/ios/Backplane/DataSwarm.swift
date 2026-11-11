import SwiftUI

// Swarms on the phone (docs/SWARM.md, Client data and UI; feature `swarm`): a project's row for each live swarm, the swarm's screen (members, state,
// the board as one feed, a composer) and the New swarm sheet. What to show is the hub's; the few choices made here are SwarmModel.swift's pure functions.

/// A member's or a post's colour, as the hub gives it ("#rrggbb").
private func dSwarmColor(_ css: String) -> Color { Color(css: css) ?? BackplaneDesign.faint }
/// A member's state as a dot: the thread's own colours (dStateColor), a waiting member like an asking thread.
private func dMemberDot(_ state: String) -> Color {
    switch state { case "working": BackplaneDesign.ok; case "waiting", "failed": BackplaneDesign.bad; default: BackplaneDesign.faint }
}

/// One swarm on its project's list: its name, state and member count.
struct DataSwarmRow: View {
    let row: DSwarmRow
    let open: () -> Void
    var body: some View {
        Button(action: open) {
            HStack(spacing: 8) {
                Image(systemName: "circle.hexagongrid").font(.system(size: 12)).foregroundStyle(row.state == "paused" ? BackplaneDesign.warn : BackplaneDesign.accent).frame(width: 16)
                Text(row.name).dmono(13).foregroundStyle(BackplaneDesign.text).lineLimit(1)
                Spacer(minLength: 4)
                Text(Tr.t(row.state)).dmono(11).foregroundStyle(row.state == "paused" ? BackplaneDesign.warn : BackplaneDesign.faint)
                Text(Tr.ios_swarm_members(n: row.members)).dmono(11).foregroundStyle(BackplaneDesign.faint)
            }.padding(.leading, 30).padding(.trailing, 12).frame(minHeight: 40).contentShape(Rectangle())
        }.buttonStyle(.plain).dataRow().accessibilityIdentifier("data.swarm.row." + row.id)
    }
}

/// The swarm's screen: member chips, the goal and state with its controls, the board, a composer.
struct DataSwarmView: View {
    let client: DataClient
    let route: DSwarmRoute
    @State private var board = DSwarmBoard()
    @State private var loaded = false
    @State private var draft = ""
    @State private var error = ""
    @State private var busy = false
    @State private var stopping = false
    @State private var adding = false
    @State private var removing: DSwarmMember?
    private var state: String { board.room.state.isEmpty ? "active" : board.room.state }

    var body: some View {
        VStack(spacing: 0) {
            chips
            DLine()
            head
            DLine()
            feed
            if !Swarm.ended(state) { DLine(); composer }
        }
        .background(BackplaneDesign.background)
        .dataBar(board.room.name.isEmpty ? route.name : board.room.name)
        .task { await read(all: true) }
        .onChange(of: client.swarmMoved[route.id] ?? 0) { _, _ in Task { await read() } }
        .onChange(of: client.conn) { _, c in if c == .online { Task { await read(all: true) } } }
        .confirmationDialog(Tr.k("ios.swarm.stop.ask"), isPresented: $stopping, titleVisibility: .visible) {
            Button(Tr.t("Stop"), role: .destructive) { act("stop") }.accessibilityIdentifier("data.swarm.stop.yes")
            Button(Tr.t("Cancel"), role: .cancel) {}
        }
        .confirmationDialog(Tr.ios_swarm_remove_ask(name: removing?.name ?? ""), isPresented: Binding(get: { removing != nil }, set: { if !$0 { removing = nil } }), titleVisibility: .visible) {
            Button(Tr.t("Remove"), role: .destructive) { if let m = removing { act("remove", ["thread": m.thread]) } }.accessibilityIdentifier("data.swarm.remove.yes")
            Button(Tr.t("Cancel"), role: .cancel) {}
        }
        .sheet(isPresented: $adding) { DataSwarmAddSheet(client: client, room: route.id) { Task { await read() } } }
    }

    /// The members, side by side: colour, name, state; a tap opens the member's thread, a long press offers Remove.
    private var chips: some View {
        let edit = Swarm.canEditMembers(state, count: board.members.filter { $0.state != "removed" }.count)
        return ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 6) {
                ForEach(board.members) { m in
                    NavigationLink(value: m.thread) {
                        HStack(spacing: 6) {
                            Rectangle().fill(dSwarmColor(m.color)).frame(width: 4, height: 18)
                            Text(m.name).dmono(12, bold: true).foregroundStyle(m.state == "removed" ? BackplaneDesign.faint : BackplaneDesign.text).lineLimit(1)
                            DDot(color: dMemberDot(m.state), size: 7)
                        }.padding(.horizontal, 8).frame(height: 34).dbox(fill: BackplaneDesign.panel)
                    }
                    .buttonStyle(.plain).disabled(m.thread.isEmpty)
                    .accessibilityLabel(m.name + ", " + Tr.t(m.state)).accessibilityIdentifier("data.swarm.member." + m.name)
                    .contextMenu { if edit.remove && !m.thread.isEmpty && m.state != "removed" { Button(Tr.t("Remove"), systemImage: "minus.circle", role: .destructive) { removing = m } } }
                }
                if edit.add {
                    Button { adding = true } label: { Image(systemName: "plus").font(.system(size: 13)).foregroundStyle(BackplaneDesign.dim).frame(width: 34, height: 34).dbox(fill: BackplaneDesign.panel) }
                        .buttonStyle(.plain).accessibilityLabel(Tr.k("ios.swarm.add")).accessibilityIdentifier("data.swarm.add")
                }
            }.padding(.horizontal, 10).padding(.vertical, 8)
        }.background(BackplaneDesign.panel)
    }

    /// The goal, the state (with why it paused) and Pause/Resume and Stop.
    private var head: some View {
        VStack(alignment: .leading, spacing: 6) {
            if !board.room.goal.isEmpty { Text(board.room.goal).dmono(13).foregroundStyle(BackplaneDesign.text).fixedSize(horizontal: false, vertical: true).accessibilityIdentifier("data.swarm.goal") }
            HStack(spacing: 8) {
                DDot(color: state == "active" ? BackplaneDesign.ok : state == "paused" ? BackplaneDesign.warn : BackplaneDesign.faint, size: 7)
                Text(board.room.reason.isEmpty ? Tr.t(state) : Tr.t(state) + " · " + Tr.t(board.room.reason)).dmono(12).foregroundStyle(BackplaneDesign.dim).lineLimit(1).accessibilityIdentifier("data.swarm.state")
                Spacer(minLength: 4)
                ForEach(Swarm.actions(state), id: \.self) { a in
                    Button(Tr.t(a == "pause" ? "Pause" : a == "resume" ? "Resume" : "Stop")) { if a == "stop" { stopping = true } else { act(a) } }
                        .buttonStyle(DButton(danger: a == "stop")).disabled(busy).accessibilityIdentifier("data.swarm.act." + a)
                }
            }
            if !error.isEmpty { Text(Tr.t(error)).dmono(12).foregroundStyle(BackplaneDesign.bad) }
        }.padding(.horizontal, 12).padding(.vertical, 8).frame(maxWidth: .infinity, alignment: .leading).background(BackplaneDesign.panel)
    }

    private var feed: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 8) {
                    if !loaded { DLoading(text: Tr.t(client.conn == .online ? "Loading the thread…" : "Connecting to the hub…")) }
                    ForEach(board.posts) { p in DataSwarmPost(post: p, claims: board.claims).id(p.id) }
                }.padding(.horizontal, 12).padding(.vertical, 10)
            }
            .defaultScrollAnchor(.bottom)
            .onChange(of: board.posts.last?.id) { _, id in if let id { withAnimation { proxy.scrollTo(id, anchor: .bottom) } } }
        }
    }

    private var composer: some View {
        HStack(alignment: .bottom, spacing: 8) {
            TextField(Tr.k("ios.swarm.post"), text: $draft, axis: .vertical).dmono(13).lineLimit(1...6).padding(10).dbox(BackplaneDesign.accent).accessibilityIdentifier("data.swarm.composer")
            Button(Tr.t("Post")) { send() }.buttonStyle(DButton(primary: true))
                .disabled(draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty).accessibilityIdentifier("data.swarm.send")
        }.padding(12).background(BackplaneDesign.panel)
    }

    /// The board: whole at first and after a reconnect, else only what came after the newest post shown (`since`).
    private func read(all: Bool = false) async {
        do {
            let r = try await client.swarmGet(route.id, since: all ? "" : Swarm.since(board.posts))
            let posts = all ? r.posts : Swarm.merge(board.posts, r.posts)
            board = r; board.posts = posts; loaded = true
        } catch { if !loaded { self.error = error.localizedDescription } }
    }
    private func send() {
        let t = draft.trimmingCharacters(in: .whitespacesAndNewlines); guard !t.isEmpty else { return }
        draft = ""; error = ""
        Task {
            do { try await client.swarmPost(route.id, t); await read() } catch { draft = t; self.error = error.localizedDescription }
        }
    }
    private func act(_ a: String, _ extra: [String: Any] = [:]) {
        busy = true; error = ""
        Task {
            do { try await client.swarmAct(route.id, a, extra) } catch { self.error = error.localizedDescription }
            busy = false; await read()
        }
    }
}

/// One post of the board: the member's colour, its tags, a claim's files (marked while a peer's lock keeps it waiting); the hub's own posts grey, a held one dimmed,
/// a synthesis in a box of its own.
struct DataSwarmPost: View {
    let post: DSwarmPost
    let claims: [DSwarmClaim]
    var body: some View {
        let look = Swarm.look(post)
        Group {
            switch look {
            case .hub:
                Text(post.text).dmono(11).foregroundStyle(BackplaneDesign.faint).frame(maxWidth: .infinity, alignment: .leading).padding(.vertical, 2)
            case .synthesis:
                VStack(alignment: .leading, spacing: 6) {
                    HStack(spacing: 6) { DHeading(text: Tr.t("synthesis")); Text(who).dmono(11, bold: true).foregroundStyle(dSwarmColor(post.color)) }
                    DLinkText(text: post.text)
                }.padding(12).frame(maxWidth: .infinity, alignment: .leading).dbox(BackplaneDesign.accent, fill: BackplaneDesign.panel).padding(.vertical, 8)
            default:
                HStack(alignment: .top, spacing: 8) {
                    Rectangle().fill(post.from == "you" && post.thread.isEmpty ? BackplaneDesign.accent : dSwarmColor(post.color)).frame(width: 3)
                    VStack(alignment: .leading, spacing: 3) {
                        HStack(spacing: 6) {
                            Text(who).dmono(11, bold: true).foregroundStyle(post.thread.isEmpty ? BackplaneDesign.accent : dSwarmColor(post.color)).lineLimit(1)
                            ForEach(Swarm.labels(post), id: \.self) { t in Text(Tr.t(t)).dmono(10).foregroundStyle(BackplaneDesign.dim).padding(.horizontal, 4).frame(height: 16).dbox(fill: BackplaneDesign.raised) }
                            Spacer(minLength: 0)
                            Text(dAgo(Swarm.ms(post.at))).dmono(10).foregroundStyle(BackplaneDesign.faint)
                        }
                        DLinkText(text: post.text)
                        if !post.files.isEmpty { files }
                        if look == .held { Text(Tr.ios_swarm_held(why: Tr.t(post.held))).dmono(10).foregroundStyle(BackplaneDesign.faint) }
                    }
                }.fixedSize(horizontal: false, vertical: true).opacity(look == .held ? 0.5 : 1)
            }
        }.accessibilityIdentifier("data.swarm.post." + post.id)
    }
    private var who: String { post.from.isEmpty || (post.from == "you" && post.thread.isEmpty) ? Tr.t("you") : post.from }
    private var files: some View {
        let waiting = Swarm.waiting(post, claims), open = Swarm.open(post, claims)
        return VStack(alignment: .leading, spacing: 3) {
            ForEach(post.files, id: \.self) { f in
                HStack(spacing: 6) {
                    Text(f).dmono(11).foregroundStyle(open ? BackplaneDesign.text : BackplaneDesign.faint).lineLimit(1).truncationMode(.head)
                    if waiting { Text(Tr.t("waiting")).dmono(10, bold: true).foregroundStyle(BackplaneDesign.warn) }
                }.padding(.horizontal, 6).frame(height: 22).dbox(waiting ? BackplaneDesign.warn : BackplaneDesign.line, fill: BackplaneDesign.panel)
            }
        }
    }
}

/// A member row: provider, model (from `models.list`) and an optional role.
private struct DataSwarmMemberRow: View {
    @Binding var member: DSwarmMemberDraft
    let models: [String: [DModelRow]]
    let index: Int
    let remove: (() -> Void)?
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 6) {
                Menu {
                    ForEach(Swarm.providers(models), id: \.self) { p in Button(p) { member.provider = p; member.model = "" } }
                } label: { pick(member.provider.isEmpty ? Tr.t("Provider") : member.provider) }.accessibilityIdentifier("data.swarm.member.provider.\(index)")
                Menu {
                    Button(Tr.t("default")) { member.model = "" }
                    ForEach(models[member.provider] ?? [], id: \.value) { m in Button(m.label) { member.model = m.value }.disabled(!m.ok) }
                } label: { pick(member.model.isEmpty ? Tr.t("default") : (models[member.provider]?.first { $0.value == member.model }?.label ?? member.model)) }.accessibilityIdentifier("data.swarm.member.model.\(index)")
                if let remove {
                    Button(action: remove) { Image(systemName: "minus").font(.system(size: 12)).foregroundStyle(BackplaneDesign.dim).frame(width: 36, height: 36).dbox(fill: BackplaneDesign.panel) }
                        .buttonStyle(.plain).accessibilityLabel(Tr.t("Remove")).accessibilityIdentifier("data.swarm.member.remove.\(index)")
                }
            }
            TextField(Tr.k("ios.swarm.role"), text: $member.role).dmono(12).textInputAutocapitalization(.never).autocorrectionDisabled().padding(.horizontal, 8).frame(height: 34).dbox()
                .accessibilityIdentifier("data.swarm.member.role.\(index)")
        }
    }
    private func pick(_ s: String) -> some View {
        HStack(spacing: 4) { Text(s).dmono(12).foregroundStyle(BackplaneDesign.text).lineLimit(1); Spacer(minLength: 0); Image(systemName: "chevron.down").font(.system(size: 9)).foregroundStyle(BackplaneDesign.dim) }
            .padding(.horizontal, 8).frame(maxWidth: .infinity, minHeight: 36).dbox(fill: BackplaneDesign.panel)
    }
}

/// New swarm: topic, goal, seed and 2 to 8 members; Start sends `swarm.create` and opens the swarm.
struct DataNewSwarmSheet: View {
    let client: DataClient
    let project: String
    let started: (DSwarmRoute) -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var topic = ""
    @State private var goal = ""
    @State private var seed = ""
    @State private var members: [DSwarmMemberDraft] = []
    @State private var models: [String: [DModelRow]] = [:]
    @State private var busy = false
    @State private var error = ""
    private var owner: DProject? { client.projects.first { $0.id == project } }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 10) {
                HStack {
                    Text(Tr.k("ios.swarm.new")).dmono(14, bold: true).lineLimit(1)
                    if let o = owner { Text(o.title).dmono(12).foregroundStyle(BackplaneDesign.faint).lineLimit(1) }
                    Spacer()
                    Button(Tr.t("Cancel")) { dismiss() }.dmono(13).foregroundStyle(BackplaneDesign.dim)
                }
                TextField(Tr.k("ios.swarm.topic"), text: $topic).dmono(13).padding(.horizontal, 10).frame(height: 40).dbox(BackplaneDesign.accent).accessibilityIdentifier("data.swarm.new.topic")
                TextField(Tr.k("ios.swarm.goal"), text: $goal, axis: .vertical).dmono(13).lineLimit(1...3).padding(10).dbox().accessibilityIdentifier("data.swarm.new.goal")
                TextField(Tr.k("ios.swarm.seed"), text: $seed, axis: .vertical).dmono(13).lineLimit(3...8).padding(10).dbox().accessibilityIdentifier("data.swarm.new.seed")
                DHeading(text: Tr.t("Members")).padding(.top, 4)
                ForEach($members) { $m in
                    let i = members.firstIndex { $0.id == m.id } ?? 0, id = m.id
                    DataSwarmMemberRow(member: $m, models: models, index: i, remove: members.count > Swarm.minMembers ? { members.removeAll { $0.id == id } } : nil)
                    if i < members.count - 1 { DLine() }
                }
                if members.count < Swarm.maxMembers {
                    Button(Tr.k("ios.swarm.add")) { members.append(Swarm.draft(models)) }.buttonStyle(DButton()).accessibilityIdentifier("data.swarm.new.add")
                }
                if !error.isEmpty { Text(Tr.t(error)).dmono(12).foregroundStyle(BackplaneDesign.bad) }
                HStack {
                    Spacer()
                    Button(Tr.t(busy ? "Starting…" : "Start")) { Task { await start() } }.buttonStyle(DButton(primary: true))
                        .disabled(busy || !Swarm.canCreate(topic: topic, members: members)).accessibilityIdentifier("data.swarm.new.start")
                }
            }.padding(14)
        }
        .background(BackplaneDesign.panel)
        .presentationDetents([.large]).dataSheet()
        .task {
            models = await client.modelsList()
            if members.isEmpty { members = (0..<Swarm.minMembers).map { _ in Swarm.draft(models) } }
        }
    }
    private func start() async {
        busy = true; defer { busy = false }; error = ""
        do {
            let room = try await client.swarmCreate(Swarm.createParams(project: project, topic: topic, goal: goal, seed: seed, members: members))
            dismiss(); started(DSwarmRoute(id: room, name: topic.trimmingCharacters(in: .whitespacesAndNewlines)))
        } catch { self.error = error.localizedDescription }
    }
}

/// Add member: one member row; Add sends `swarm.act add`.
struct DataSwarmAddSheet: View {
    let client: DataClient
    let room: String
    let added: () -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var member = DSwarmMemberDraft()
    @State private var models: [String: [DModelRow]] = [:]
    @State private var busy = false
    @State private var error = ""
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text(Tr.k("ios.swarm.add")).dmono(14, bold: true); Spacer()
                Button(Tr.t("Cancel")) { dismiss() }.dmono(13).foregroundStyle(BackplaneDesign.dim)
            }
            DataSwarmMemberRow(member: $member, models: models, index: 0, remove: nil)
            if !error.isEmpty { Text(Tr.t(error)).dmono(12).foregroundStyle(BackplaneDesign.bad) }
            HStack {
                Spacer()
                Button(Tr.t("Add")) { Task { await add() } }.buttonStyle(DButton(primary: true)).disabled(busy || member.provider.isEmpty).accessibilityIdentifier("data.swarm.add.go")
            }
            Spacer(minLength: 0)
        }
        .padding(14).background(BackplaneDesign.panel)
        .presentationDetents([.medium]).dataSheet()
        .task { models = await client.modelsList(); if member.provider.isEmpty { member = Swarm.draft(models) } }
    }
    private func add() async {
        busy = true; defer { busy = false }; error = ""
        var p: [String: Any] = [:]; for (k, v) in Swarm.member(member) { p[k] = v }
        do { try await client.swarmAct(room, "add", p); dismiss(); added() } catch { self.error = error.localizedDescription }
    }
}
