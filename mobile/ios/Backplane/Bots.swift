import SwiftUI

// Bots (docs/bots.md): the cats and rooms under the projects, the forms
// that make them, and a bot's, a room's or a remote bot's screen. All of
// it is the screen's (src/mobile/bots.bend); these views only draw it and
// send actions back.

private let sep = "\u{1f}"

// how a mood looks next to a cat
func moodTint(_ mood: String) -> Color {
    switch mood {
    case "wait": .orange
    case "work", "talk": .green
    case "think": .blue
    case "error": .red
    case "idle": .mint
    default: .secondary
    }
}

struct MoodDot: View {
    let mood: String

    var body: some View {
        Circle().fill(moodTint(mood)).frame(width: 7, height: 7)
    }
}

// a cat by its key: its rig, once the bridge has given it
struct BotCat: View {
    let model: AppModel
    let key: String
    let size: CGFloat

    var body: some View {
        CatView(rig: model.cats[key] ?? [], size: size)
            .task(id: key) { await model.cat(key) }
    }
}

// The list
// --------

struct BotsSection: View {
    let model: AppModel
    let screen: Screen

    var body: some View {
        Section {
            ForEach(screen.bots) { b in
                Button { model.open(nav: b.nav ?? "", action: b.act ?? (b.remote ? "remote" : "bot"), value: b.id, title: b.name, cat: b.cat, kind: b.remote ? "remote" : "bot") } label: {
                    HStack(spacing: 12) {
                        BotCat(model: model, key: b.cat, size: 36)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(b.name).foregroundStyle(.primary)
                            HStack(spacing: 5) {
                                MoodDot(mood: b.mood)
                                Text([b.note, b.peer, b.machine ?? ""].filter { !$0.isEmpty }.joined(separator: " · "))
                                    .lineLimit(1)
                            }
                            .font(.caption)
                            .foregroundStyle(.secondary)
                        }
                        Spacer()
                        Image(systemName: "chevron.right").font(.caption).foregroundStyle(.tertiary)
                    }
                }
                .opacity(b.mood == "away" ? 0.5 : 1)
            }
            if let b = screen.boardEntry {
                Button(b.label) { model.act("tb-open", screen.hub + "|") }
            }
            ForEach(screen.rooms) { r in
                Button { model.open(nav: r.nav ?? "", action: "room", value: r.id, title: r.name, kind: "room") } label: {
                    HStack(spacing: 12) {
                        Image(systemName: "bubble.left.and.bubble.right").frame(width: 36).foregroundStyle(.secondary)
                        Text(r.name).foregroundStyle(.primary)
                        Spacer()
                        Label("\(r.members)", systemImage: "person.2").font(.caption).foregroundStyle(.secondary)
                        Image(systemName: "chevron.right").font(.caption).foregroundStyle(.tertiary)
                    }
                }
            }
        } header: {
            HStack {
                Text("Bots")
                Spacer()
                Menu {
                    // with several hubs, a bot or room is made on the one chosen
                    if screen.hubs.count > 1 {
                        ForEach(screen.hubs, id: \.key) { h in
                            Menu(h.name) {
                                Button("New bot", systemImage: "cat") { model.act("bot-new", h.key + "|") }
                                Button("New room", systemImage: "bubble.left.and.bubble.right") { model.act("room-new", h.key + "|") }
                            }
                        }
                    } else {
                        Button("New bot", systemImage: "cat") { model.act("bot-new") }
                        Button("New room", systemImage: "bubble.left.and.bubble.right") { model.act("room-new") }
                    }
                } label: {
                    Image(systemName: "plus")
                }
                .accessibilityLabel("New bot or room")
            }
        }
    }
}

// Forms
// -----

struct NewBotSheet: View {
    let model: AppModel
    let form: NewBot
    @State private var name = ""
    @State private var persona = ""

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    TextField(form.hint, text: $name)
                        .onChange(of: name) { _, t in model.field("name", t) }
                    TextField("Persona", text: $persona, axis: .vertical)
                        .lineLimit(3...10)
                        .onChange(of: persona) { _, t in model.field("persona", t) }
                }
                Section {
                    Picker("Provider", selection: Binding(get: { form.provider }, set: { model.act("bfield", "provider" + sep + $0) })) {
                        ForEach(form.providers, id: \.provider) { Text($0.label).tag($0.provider) }
                    }
                }
            }
            .navigationTitle("New bot")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { model.act("form-close", "@bnew") } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Create") { model.act("bot-create") }
                        .disabled(name.trimmingCharacters(in: .whitespaces).isEmpty)
                }
            }
        }
        .onAppear { name = form.name; persona = form.persona }
    }
}

struct NewRoomSheet: View {
    let model: AppModel
    let form: NewRoom
    @State private var name = ""

    var body: some View {
        NavigationStack {
            Form {
                TextField("Name", text: $name)
                    .onChange(of: name) { _, t in model.field("rname", t) }
                Section("Members") {
                    ForEach(form.picks, id: \.id) { p in
                        Button { model.act("room-pick", p.id) } label: {
                            HStack {
                                Text(p.name).foregroundStyle(.primary)
                                Spacer()
                                if p.on { Image(systemName: "checkmark") }
                            }
                        }
                    }
                }
            }
            .navigationTitle("New room")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { model.act("form-close", "@rnew") } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") { model.act("room-save") }.disabled(!form.picks.contains { $0.on })
                }
            }
        }
        .onAppear { name = form.name }
    }
}

// The screens
// -----------

struct BotDestination: View {
    let model: AppModel
    let bot: BotView
    var live = true
    var cachedThread: ThreadView?
    var cachedPlane: PlaneModel?

    var body: some View {
        switch bot.kind {
        case "bot": BotScreen(model: model, bot: bot, live: live, cachedThread: cachedThread, cachedPlane: cachedPlane)
        case "room": TalkScreen(model: model, bot: bot, field: "rpost", send: "room-post")
        default: TalkScreen(model: model, bot: bot, field: "rtext", send: "remote-send")
        }
    }
}

// a cat, its name and how it is
private struct BotHeader: View {
    let model: AppModel
    let bot: BotView
    let size: CGFloat

    var body: some View {
        HStack(spacing: 12) {
            BotCat(model: model, key: bot.cat ?? "", size: size)
                .saturation(bot.mood == "away" ? 0 : 1)
            VStack(alignment: .leading, spacing: 2) {
                Text(bot.title ?? bot.name).font(.title3.weight(.semibold)).lineLimit(1)
                HStack(spacing: 5) {
                    MoodDot(mood: bot.mood ?? "")
                    Text([bot.note ?? "", bot.title == nil ? bot.peer ?? "" : ""].filter { !$0.isEmpty }.joined(separator: " · ")).lineLimit(1)
                }
                .font(.subheadline)
                .foregroundStyle(.secondary)
            }
            Spacer(minLength: 0)
        }
    }
}

struct BotScreen: View {
    let model: AppModel
    let bot: BotView
    var live = true
    var cachedThread: ThreadView?
    var cachedPlane: PlaneModel?

    var body: some View {
        content
            .safeAreaInset(edge: .top, spacing: 0) {
                VStack(spacing: 6) {
                    BotHeader(model: model, bot: bot, size: 64).padding(.horizontal)
                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack(spacing: 0) {
                            ForEach(bot.tabs ?? [], id: \.id) { t in
                                let on = t.id == bot.tab
                                Button { model.act("bot-tab", t.id) } label: {
                                    Text(t.label)
                                        .font(.subheadline.weight(on ? .semibold : .regular))
                                        .padding(.horizontal, 12).padding(.vertical, 8)
                                        .overlay(alignment: .bottom) {
                                            Rectangle().fill(on ? Color.accentColor : .clear).frame(height: 2)
                                        }
                                }
                                .foregroundStyle(on ? .primary : .secondary)
                            }
                        }
                        .padding(.horizontal, 4)
                    }
                    Divider()
                }
                .padding(.top, 8)
                .background(.bar)
            }
    }

    @ViewBuilder private var content: some View {
        switch bot.tab ?? "chat" {
        case "status":
            StatusTab(model: model, plane: live ? model.screen.flatMap { model.plane($0) } : cachedPlane).navigationTitle(bot.name).navigationBarTitleDisplayMode(.inline)
        case "space":
            if let p = bot.page {
                SpacePage(model: model, page: p).navigationTitle(bot.name).navigationBarTitleDisplayMode(.inline)
            } else if let s = bot.space {
                SpaceView(space: s) { model.act($0, $1) }.navigationTitle(bot.name).navigationBarTitleDisplayMode(.inline)
            }
        case "browser":
            BrowserTab(model: model, browser: bot.browser).navigationTitle(bot.name).navigationBarTitleDisplayMode(.inline)
        case "memory":
            MemoryTab(model: model, items: bot.memory ?? []).navigationTitle(bot.name).navigationBarTitleDisplayMode(.inline)
        case "routines":
            RoutinesTab(model: model, items: bot.routines ?? [], form: bot.routine).navigationTitle(bot.name).navigationBarTitleDisplayMode(.inline)
        case "hooks":
            HooksTab(model: model, bot: bot).navigationTitle(bot.name).navigationBarTitleDisplayMode(.inline)
        case "settings":
            if let s = bot.settings { SettingsTab(model: model, bot: bot, settings: s).navigationTitle(bot.name).navigationBarTitleDisplayMode(.inline) }
        default:
            if let t = live ? model.screen?.thread : cachedThread { ThreadScreen(model: model, thread: t, live: live) }
        }
    }
}

// the bot's page, as the hub's latest frame (fetched again when n moves)
private struct BrowserTab: View {
    let model: AppModel
    let browser: BotBrowser?
    @State private var image: UIImage?

    private var url: URL? {
        browser.flatMap { model.hubURL($0.url, query: [URLQueryItem(name: "n", value: $0.n)]) }
    }

    var body: some View {
        ScrollView {
            if let image {
                Image(uiImage: image).resizable().scaledToFit().frame(maxWidth: .infinity)
            } else {
                ContentUnavailableView("No page yet", systemImage: "globe")
            }
        }
        .task(id: url) {
            guard let u = url, let (d, r) = try? await URLSession.shared.data(from: u),
                  (r as? HTTPURLResponse)?.statusCode == 200, let i = UIImage(data: d) else { return }
            image = i
        }
    }
}

private struct MemoryTab: View {
    let model: AppModel
    let items: [BotMemory]

    var body: some View {
        List {
            ForEach(items, id: \.key) { m in
                VStack(alignment: .leading, spacing: 4) {
                    HStack(spacing: 6) {
                        Text(m.key).font(.subheadline.weight(.semibold)).lineLimit(1)
                        Text(m.kind).font(.caption2.monospaced())
                            .padding(.horizontal, 5).padding(.vertical, 1)
                            .background(.quaternary, in: .rect(cornerRadius: 2))
                        Spacer()
                        Text(m.updated).font(.caption).foregroundStyle(.secondary)
                    }
                    Text(m.text).font(.subheadline)
                    if !m.tags.isEmpty { Text(m.tags).font(.caption).foregroundStyle(.secondary) }
                }
                .swipeActions { Button("Forget", role: .destructive) { model.act("mem-forget", m.key) } }
            }
        }
        .overlay { if items.isEmpty { ContentUnavailableView("Nothing remembered", systemImage: "brain") } }
    }
}

private struct RoutinesTab: View {
    let model: AppModel
    let items: [BotRoutine]
    let form: RoutineForm?

    var body: some View {
        List {
            Button { model.act("routine-edit", "") } label: { Label("New routine", systemImage: "plus") }
            ForEach(items, id: \.id) { r in
                HStack(alignment: .top) {
                    VStack(alignment: .leading, spacing: 3) {
                        Text(r.name).font(.subheadline.weight(.semibold))
                        Text(r.when).font(.caption.monospaced()).foregroundStyle(.secondary)
                        Text(r.prompt).font(.caption).lineLimit(2)
                        if !r.last.isEmpty { Text("last " + r.last).font(.caption2).foregroundStyle(.secondary) }
                    }
                    Spacer()
                    Toggle("On", isOn: Binding(get: { r.on }, set: { _ in model.act("routine-toggle", r.id) })).labelsHidden()
                }
                .contentShape(.rect)
                .onTapGesture { model.act("routine-edit", r.id) }
                .swipeActions {
                    Button("Delete", role: .destructive) { model.act("routine-delete", r.id) }
                    Button("Run") { model.act("routine-run", r.id) }.tint(.indigo)
                }
            }
        }
        .sheet(isPresented: Binding(get: { form?.open ?? false }, set: { if !$0 { model.act("form-close", "@onew") } })) {
            if let f = model.screen?.bot?.routine { RoutineSheet(model: model, form: f) }
        }
    }
}

private struct RoutineSheet: View {
    let model: AppModel
    let form: RoutineForm
    @State private var name = ""
    @State private var cron = ""
    @State private var prompt = ""

    var body: some View {
        NavigationStack {
            Form {
                TextField("Name", text: $name).onChange(of: name) { _, t in model.field("oname", t) }
                Section {
                    TextField("0 9 * * 1-5", text: $cron)
                        .font(.body.monospaced())
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .onChange(of: cron) { _, t in model.field("ocron", t) }
                } header: {
                    Text("When")
                } footer: {
                    Text("Cron, @daily, every 15m, weekdays 08:00")
                }
                Section("Prompt") {
                    TextField("What to do", text: $prompt, axis: .vertical)
                        .lineLimit(3...12)
                        .onChange(of: prompt) { _, t in model.field("oprompt", t) }
                }
            }
            .navigationTitle(form.id.isEmpty ? "New routine" : "Routine")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { model.act("form-close", "@onew") } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") { model.act("routine-save") }.disabled(cron.trimmingCharacters(in: .whitespaces).isEmpty)
                }
            }
        }
        .onAppear { name = form.name; cron = form.cron; prompt = form.prompt }
    }
}

private struct HooksTab: View {
    let model: AppModel
    let bot: BotView
    @State private var name = ""

    var body: some View {
        List {
            Section {
                HStack {
                    TextField("Name", text: $name).onChange(of: name) { _, t in model.field("hname", t) }
                    Button("Add") { model.act("hook-new"); name = "" }
                }
            }
            if let s = bot.secret, !s.isEmpty {
                Section {
                    Text(s).font(.caption.monospaced()).textSelection(.enabled)
                    Button("Copy", systemImage: "doc.on.doc") { model.act("copy", s) }
                } header: {
                    Text("Secret")
                } footer: {
                    Text("Shown once.")
                }
            }
            Section {
                ForEach(bot.hooks ?? [], id: \.id) { h in
                    let url = model.hubURL(h.path, token: false)?.absoluteString ?? h.path
                    VStack(alignment: .leading, spacing: 3) {
                        Text(h.name).font(.subheadline.weight(.semibold))
                        Text(url).font(.caption.monospaced()).lineLimit(1).truncationMode(.middle).textSelection(.enabled)
                        Text(h.last.isEmpty ? "\(h.count)" : "\(h.count) · " + h.last).font(.caption2).foregroundStyle(.secondary)
                    }
                    .contextMenu { Button("Copy URL", systemImage: "doc.on.doc") { model.act("copy", url) } }
                    .swipeActions { Button("Revoke", role: .destructive) { model.act("hook-revoke", h.id) } }
                }
            }
        }
    }
}

private struct SettingsTab: View {
    let model: AppModel
    let bot: BotView
    let settings: BotSettings
    @State private var persona = ""
    @State private var gid = ""
    @State private var gsecret = ""
    @State private var gpaste = ""
    @State private var purl = ""
    @State private var join = ""
    @State private var deleting = false

    var body: some View {
        Form {
            Section("Persona") {
                TextField("Persona", text: $persona, axis: .vertical)
                    .lineLimit(4...14)
                    .onChange(of: persona) { _, t in model.field("persona", t) }
                Button("Save") { model.act("bot-edit") }.disabled(persona == settings.persona)
            }
            Section {
                if !settings.google.status.isEmpty { Text(settings.google.status).font(.subheadline) }
                ForEach(settings.google.accounts, id: \.self) { a in
                    HStack {
                        Text(a).lineLimit(1)
                        Spacer()
                        Button("Sign out", role: .destructive) { model.act("google", "signout:" + a) }
                            .buttonStyle(.borderless)
                    }
                }
                Button(settings.google.accounts.isEmpty ? "Connect Google" : "Add account") { GoogleLoop.shared.start { model.act($0, $1) } }
                DisclosureGroup("Own client") {
                    TextField("Client ID (Desktop app)", text: $gid)
                        .textInputAutocapitalization(.never).autocorrectionDisabled()
                        .onChange(of: gid) { _, t in model.field("gid", t) }
                    SecureField("Client secret", text: $gsecret)
                        .onChange(of: gsecret) { _, t in model.field("gsecret", t) }
                    Button("Save client") { model.act("google", "configure") }
                }
                if let u = URL(string: settings.google.url), !settings.google.url.isEmpty {
                    Link("Open Google sign-in", destination: u)
                    TextField("Redirected URL", text: $gpaste)
                        .textInputAutocapitalization(.never).autocorrectionDisabled()
                        .onChange(of: gpaste) { _, t in model.field("gpaste", t) }
                    Button("Finish") { model.act("google", "finish") }
                }
            } header: {
                Text("Google")
            }
            Section {
                ForEach(settings.peers, id: \.id) { p in
                    VStack(alignment: .leading, spacing: 2) {
                        Text(p.name)
                        Text(p.url).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                    }
                    .swipeActions { Button("Unlink", role: .destructive) { model.act("peer-revoke", p.id) } }
                }
                HStack {
                    TextField("This hub's address", text: $purl)
                        .textInputAutocapitalization(.never).autocorrectionDisabled().keyboardType(.URL)
                        .onChange(of: purl) { _, t in model.field("purl", t) }
                    Button("Invite") { model.act("peer-invite") }
                }
                if !settings.invite.isEmpty {
                    Text(settings.invite).font(.caption.monospaced()).textSelection(.enabled)
                    Button("Copy invite", systemImage: "doc.on.doc") { model.act("copy", settings.invite) }
                }
                HStack {
                    TextField("Paste an invite", text: $join)
                        .textInputAutocapitalization(.never).autocorrectionDisabled()
                        .onChange(of: join) { _, t in model.field("invite", t) }
                    Button("Join") { model.act("peer-join"); join = "" }
                }
            } header: {
                Text("Machines")
            }
            Section {
                Button("Delete bot", role: .destructive) { deleting = true }
            }
        }
        .onChange(of: settings.google.url) { _, u in GoogleLoop.shared.open(u) }
        .onAppear {
            persona = settings.personaField.isEmpty ? settings.persona : settings.personaField
            gid = settings.google.gid
            gsecret = settings.google.gsecret
            purl = settings.purl
        }
        .confirmationDialog("Delete \(bot.name)?", isPresented: $deleting, titleVisibility: .visible) {
            Button("Delete", role: .destructive) { model.act("bot-delete", bot.id) }
        }
    }
}

// a room, or the conversation with a bot on a linked machine: posts and a
// composer (field is its "bfield", send the action that posts it)
struct TalkScreen: View {
    let model: AppModel
    let bot: BotView
    let field: String
    let send: String
    @State private var text = ""

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 12) {
                    ForEach(bot.posts ?? []) { p in
                        VStack(alignment: p.mine ? .trailing : .leading, spacing: 2) {
                            HStack(spacing: 6) {
                                Text(p.from).font(.caption.weight(.semibold))
                                Text(p.ago).font(.caption2).foregroundStyle(.secondary)
                            }
                            Text(p.text)
                                .padding(.horizontal, 12).padding(.vertical, 8)
                                .background(p.mine ? Color.accentColor.opacity(0.15) : Color(.secondarySystemBackground), in: .rect(cornerRadius: 6))
                                .contextMenu { Button("Copy", systemImage: "doc.on.doc") { model.act("copy", p.text) } }
                        }
                        .frame(maxWidth: .infinity, alignment: p.mine ? .trailing : .leading)
                    }
                    Color.clear.frame(height: 1).id("end")
                }
                .padding()
            }
            .defaultScrollAnchor(.bottom)
            .scrollDismissesKeyboard(.interactively)
            .onChange(of: bot.posts?.count) { withAnimation { proxy.scrollTo("end", anchor: .bottom) } }
            .onChange(of: model.scrolls) { proxy.scrollTo("end", anchor: .bottom) }
        }
        .overlay { if (bot.posts ?? []).isEmpty { ContentUnavailableView("No messages", systemImage: "bubble.left.and.bubble.right") } }
        .safeAreaInset(edge: .top, spacing: 0) {
            if bot.kind == "remote" {
                BotHeader(model: model, bot: bot, size: 48).padding(.horizontal).padding(.vertical, 8).background(.bar)
            }
        }
        .safeAreaInset(edge: .bottom) {
            HStack(alignment: .bottom, spacing: 8) {
                TextField(bot.kind == "room" ? "Post to the room" : "Message", text: $text, axis: .vertical)
                    .lineLimit(1...6)
                    .padding(.horizontal, 12).padding(.vertical, 8)
                    .background(Color(.secondarySystemBackground), in: .rect(cornerRadius: 8))
                    .onChange(of: text) { _, t in model.field(field, t) }
                Button {
                    model.act(send)
                    text = ""
                } label: {
                    Image(systemName: "arrow.up.circle.fill").font(.system(size: 32))
                }
                .disabled(text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                .accessibilityLabel("Send")
            }
            .padding(.horizontal).padding(.vertical, 8)
            .background(.bar)
        }
        .navigationTitle(bot.name)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            if bot.kind == "room" {
                ToolbarItem(placement: .principal) {
                    VStack(spacing: 0) {
                        Text(bot.name).font(.headline).lineLimit(1)
                        Text(bot.members ?? "").font(.caption2).foregroundStyle(.secondary).lineLimit(1)
                    }
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Menu {
                        Button("Delete room", systemImage: "trash", role: .destructive) { model.act("room-delete", bot.id) }
                    } label: {
                        Image(systemName: "ellipsis.circle")
                    }
                }
            }
        }
        .onAppear { text = bot.draft ?? "" }
    }
}
