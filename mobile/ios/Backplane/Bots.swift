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

// Forms
// -----

struct NewBotSheet: View {
    @Environment(\.backplaneLanguage) private var lang
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
                    TextField(NativeMessages.bots_persona(lang), text: $persona, axis: .vertical)
                        .lineLimit(3...10)
                        .onChange(of: persona) { _, t in model.field("persona", t) }
                }
                Section {
                    Picker(NativeMessages.native_bot_provider(lang), selection: Binding(get: { form.provider }, set: { model.act("bfield", "provider" + sep + $0) })) {
                        ForEach(form.providers, id: \.provider) { Text($0.label).tag($0.provider) }
                    }
                }
            }
            .navigationTitle(NativeMessages.bots_new(lang))
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button(NativeMessages.common_cancel(lang)) { model.act("form-close", "@bnew") } }
                ToolbarItem(placement: .confirmationAction) {
                    Button(NativeMessages.common_create(lang)) { model.act("bot-create") }
                        .disabled(name.trimmingCharacters(in: .whitespaces).isEmpty)
                }
            }
        }
        .onAppear { name = form.name; persona = form.persona }
    }
}

struct NewRoomSheet: View {
    @Environment(\.backplaneLanguage) private var lang
    let model: AppModel
    let form: NewRoom
    @State private var name = ""

    var body: some View {
        NavigationStack {
            Form {
                TextField(NativeMessages.bots_name(lang), text: $name)
                    .onChange(of: name) { _, t in model.field("rname", t) }
                Section(NativeMessages.native_room_members(lang)) {
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
            .navigationTitle(NativeMessages.rooms_new(lang))
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button(NativeMessages.common_cancel(lang)) { model.act("form-close", "@rnew") } }
                ToolbarItem(placement: .confirmationAction) {
                    Button(NativeMessages.common_save(lang)) { model.act("room-save") }.disabled(!form.picks.contains { $0.on })
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

    private var mailWaiting: Bool {
        MailroomAction.waiting(live: live, contentReady: model.contentReady, loading: bot.mailroomLoading == true)
    }

    var body: some View {
        // Give the chat an explicitly smaller viewport. A top safe-area
        // inset around its UIKit timeline can displace the bottom composer
        // below the app's tab strip instead of reducing the timeline height.
        VStack(spacing: 0) {
                VStack(spacing: 6) {
                    BotHeader(model: model, bot: bot, size: 64).padding(.horizontal)
                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack(spacing: 0) {
                            ForEach(bot.tabs ?? [], id: \.id) { t in
                                let on = t.id == (bot.navTab ?? bot.tab)
                                Button { model.act("bot-tab", t.id) } label: {
                                    HStack(spacing: 4) {
                                        Text(t.label)
                                        if t.id == "mail", mailWaiting { ProgressView().controlSize(.mini) }
                                    }
                                        .font(.subheadline.weight(on ? .semibold : .regular))
                                        .padding(.horizontal, 12).padding(.vertical, 8)
                                        .overlay(alignment: .bottom) {
                                            Rectangle().fill(on ? Color.accentColor : .clear).frame(height: 2)
                                        }
                                }
                                .foregroundStyle(on ? .primary : .secondary)
                                .disabled(!live || (t.id == "mail" && mailWaiting))
                                .accessibilityAddTraits(on ? .isSelected : [])
                                .accessibilityIdentifier("bot.tab." + t.id)
                            }
                        }
                        .padding(.horizontal, 4)
                    }
                    if let sections = bot.subtabs, !sections.isEmpty {
                        ScrollView(.horizontal, showsIndicators: false) {
                            HStack(spacing: 0) {
                                ForEach(sections, id: \.id) { t in
                                    let on = t.id == bot.tab
                                    Button { model.act("bot-tab", t.id) } label: {
                                        Text(t.label)
                                            .font(.subheadline.weight(on ? .semibold : .regular))
                                            .padding(.horizontal, 12).padding(.vertical, 8)
                                    }
                                    .foregroundStyle(on ? .primary : .secondary)
                                    .disabled(!live || (t.id == "mail" && mailWaiting))
                                    .accessibilityAddTraits(on ? .isSelected : [])
                                    .accessibilityIdentifier("bot.section." + t.id)
                                }
                            }.padding(.horizontal, 4)
                        }
                    }
                    Divider()
                }
                .padding(.top, 8)
                .background(.bar)
            content.frame(maxWidth: .infinity, maxHeight: .infinity)
        }
            .toolbar {
                if model.mailroomReturn != nil, (bot.tab ?? "chat") != "chat" {
                    ToolbarItem(placement: .topBarLeading) {
                        Button { model.backToMailroom() } label: {
                            Label(NativeMessages.native_mailroom_title(model.screen?.locale ?? "en"), systemImage: "chevron.left")
                        }.accessibilityIdentifier("mailroom.return")
                            .disabled(!live)
                    }
                }
            }
            .accessibilityElement(children: .contain)
            .accessibilityIdentifier("bot.screen." + bot.id)
    }

    @ViewBuilder private var content: some View {
        switch bot.tab ?? "chat" {
        case "mail":
            if let mailroom = bot.mailroom {
                MailroomScreen(model: model, mailroom: mailroom)
                    .navigationTitle(bot.name).navigationBarTitleDisplayMode(.inline)
            } else {
                ProgressView(NativeMessages.viewer_loading(model.screen?.locale ?? "en"))
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        case "status":
            StatusTab(model: model, plane: live ? model.screen.flatMap { model.plane($0) } : cachedPlane).navigationTitle(bot.name).navigationBarTitleDisplayMode(.inline)
        case "space":
            if let p = bot.page {
                SpacePage(model: model, page: p).navigationTitle(bot.name).navigationBarTitleDisplayMode(.inline)
            } else if let s = bot.space {
                SpaceView(space: s) { model.act($0, $1) }.navigationTitle(bot.name).navigationBarTitleDisplayMode(.inline)
            }
        case "browser":
            BrowserTab(model: model, browser: bot.browser, name: bot.name).navigationTitle(bot.name).navigationBarTitleDisplayMode(.inline)
        case "tasks":
            TasksTab(model: model, tasks: bot.tasks).navigationTitle(bot.name).navigationBarTitleDisplayMode(.inline)
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

private struct MemoryTab: View {
    @Environment(\.backplaneLanguage) private var lang
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
                .swipeActions { Button(NativeMessages.bots_memory_forget(lang), role: .destructive) { model.act("mem-forget", m.key) } }
            }
        }
        .overlay { if items.isEmpty { ContentUnavailableView(NativeMessages.native_bot_memory_empty(lang), systemImage: "brain") } }
    }
}

// a routine's kind chip icon: a bell for a reminder, a clock for a one-time
// routine, else repeat
private func routineIcon(_ kind: String?) -> String {
    switch kind {
    case "reminder": return "bell"
    case "once": return "clock"
    default: return "repeat"
    }
}

// "Next Tue 14 Oct 09:30 EEST · in 3h", with the routine's zone when it is not the person's
private func routineNext(_ lang: String, _ next: String, _ rel: String?, _ zone: String?) -> String {
    NativeMessages.routine_next(lang, next, (rel ?? "").isEmpty ? "none" : "some", rel ?? "", zone == nil || zone == "your time" ? "none" : "some", zone ?? "")
}

private struct RoutinesTab: View {
    @Environment(\.backplaneLanguage) private var lang
    let model: AppModel
    let items: [BotRoutine]
    let form: RoutineForm?

    var body: some View {
        List {
            Button { model.act("routine-edit", "") } label: { Label(NativeMessages.routine_new(lang), systemImage: "plus") }
            ForEach(items, id: \.id) { r in
                HStack(alignment: .top) {
                    VStack(alignment: .leading, spacing: 3) {
                        Text(r.name).font(.subheadline.weight(.semibold))
                        HStack(spacing: 6) {
                            // kind chip: Repeats, Once, or Reminder with a bell
                            Label(r.label ?? NativeMessages.sched_kind_repeat(lang), systemImage: routineIcon(r.kind))
                                .font(.caption2).padding(.horizontal, 5).padding(.vertical, 1)
                                .background(Color.secondary.opacity(0.15))
                            Text(r.when).font(.caption.monospaced()).foregroundStyle(.secondary)
                        }
                        if r.done == true {
                            Text(r.doneText ?? NativeMessages.tour_done(lang)).font(.caption).foregroundStyle(.secondary)
                        } else if let n = r.next, !n.isEmpty {
                            Text(routineNext(lang, n, r.rel, r.zone))
                                .font(.caption).foregroundStyle(.secondary)
                        }
                        Text(r.prompt).font(.caption).lineLimit(2)
                    }
                    Spacer()
                    if r.done != true {
                        Toggle(NativeMessages.settings_lang_on(lang), isOn: Binding(get: { r.on }, set: { _ in model.act("routine-toggle", r.id) })).labelsHidden()
                    }
                }
                .opacity(r.done == true ? 0.55 : 1)
                .contentShape(.rect)
                .onTapGesture { model.act("routine-edit", r.id) }
                .swipeActions {
                    Button(NativeMessages.common_delete(lang), role: .destructive) { model.act("routine-delete", r.id) }
                    if r.done != true { Button(NativeMessages.common_run(lang)) { model.act("routine-run", r.id) }.tint(.indigo) }
                }
            }
        }
        .sheet(isPresented: Binding(get: { form?.open ?? false }, set: { if !$0 { model.act("form-close", "@onew") } })) {
            if let f = model.screen?.bot?.routine { RoutineSheet(model: model, form: f) }
        }
    }
}

private struct RoutineSheet: View {
    @Environment(\.backplaneLanguage) private var lang
    let model: AppModel
    let form: RoutineForm
    @State private var name = ""
    @State private var cron = ""
    @State private var prompt = ""

    var body: some View {
        NavigationStack {
            Form {
                TextField(NativeMessages.bots_name(lang), text: $name).onChange(of: name) { _, t in model.field("oname", t) }
                Section {
                    TextField(NativeMessages.native_routine_examples(lang), text: $cron)
                        .font(.body.monospaced())
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .onChange(of: cron) { _, t in model.field("ocron", t) }
                } header: {
                    Text(NativeMessages.native_routine_timefield(lang))
                } footer: {
                    Text((form.hint ?? "").isEmpty ? NativeMessages.native_routine_hint(lang) : form.hint!)
                }
                Section(NativeMessages.native_routine_prompt(lang)) {
                    TextField(NativeMessages.native_routine_what(lang), text: $prompt, axis: .vertical)
                        .lineLimit(3...12)
                        .onChange(of: prompt) { _, t in model.field("oprompt", t) }
                }
            }
            .navigationTitle(form.id.isEmpty ? NativeMessages.routine_new(lang) : NativeMessages.native_routine_title(lang))
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button(NativeMessages.common_cancel(lang)) { model.act("form-close", "@onew") } }
                ToolbarItem(placement: .confirmationAction) {
                    Button(NativeMessages.common_save(lang)) { model.act("routine-save") }.disabled(cron.trimmingCharacters(in: .whitespaces).isEmpty)
                }
            }
        }
        .onAppear { name = form.name; cron = form.cron; prompt = form.prompt }
    }
}

private struct HooksTab: View {
    @Environment(\.backplaneLanguage) private var lang
    let model: AppModel
    let bot: BotView
    @State private var name = ""

    var body: some View {
        List {
            Section {
                HStack {
                    TextField(NativeMessages.bots_name(lang), text: $name).onChange(of: name) { _, t in model.field("hname", t) }
                    Button(NativeMessages.common_add(lang)) { model.act("hook-new"); name = "" }
                }
            }
            if let s = bot.secret, !s.isEmpty {
                Section {
                    Text(s).font(.caption.monospaced()).textSelection(.enabled)
                    Button(NativeMessages.common_copy(lang), systemImage: "doc.on.doc") { model.act("copy", s) }
                } header: {
                    Text(NativeMessages.native_hook_secret(lang))
                } footer: {
                    Text(NativeMessages.native_hook_once(lang))
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
                    .contextMenu { Button(NativeMessages.native_hook_copyurl(lang), systemImage: "doc.on.doc") { model.act("copy", url) } }
                    .swipeActions { Button(NativeMessages.hooks_revoke(lang), role: .destructive) { model.act("hook-revoke", h.id) } }
                }
            }
        }
    }
}

private struct SettingsTab: View {
    @Environment(\.backplaneLanguage) private var lang
    let model: AppModel
    let bot: BotView
    let settings: BotSettings
    @State private var persona = ""
    @State private var gid = ""
    @State private var gsecret = ""
    @State private var gpaste = ""
    @State private var gsig = ""
    @State private var purl = ""
    @State private var join = ""
    @State private var deleting = false

    var body: some View {
        Form {
            Section(NativeMessages.bots_persona(lang)) {
                TextField(NativeMessages.bots_persona(lang), text: $persona, axis: .vertical)
                    .lineLimit(4...14)
                    .onChange(of: persona) { _, t in model.field("persona", t) }
                Button(NativeMessages.common_save(lang)) { model.act("bot-edit") }.disabled(persona == settings.persona)
            }
            Section {
                if !settings.google.status.isEmpty { Text(settings.google.status).font(.subheadline) }
                ForEach(settings.google.accounts, id: \.self) { a in
                    VStack(alignment: .leading, spacing: 6) {
                        HStack {
                            Text(a).lineLimit(1)
                            Spacer()
                            Button(NativeMessages.google_signature(lang)) { model.act("google", "sig-edit:" + a) }
                                .buttonStyle(.borderless)
                            Button(NativeMessages.agy_signout(lang), role: .destructive) { model.act("google", "signout:" + a) }
                                .buttonStyle(.borderless)
                        }
                        if settings.google.sigFor == a {
                            TextField(NativeMessages.google_signature_placeholder(lang, a), text: $gsig, axis: .vertical)
                                .lineLimit(2...6)
                                .onChange(of: gsig) { _, t in model.field("gsig", t) }
                                .onAppear { gsig = settings.google.sigText }
                            HStack {
                                Button(NativeMessages.common_save(lang)) { model.act("google", "sig-save") }.buttonStyle(.borderless)
                                Button(NativeMessages.common_cancel(lang)) { model.act("google", "sig-cancel") }.buttonStyle(.borderless)
                            }
                        }
                    }
                }
                Button(settings.google.accounts.isEmpty ? NativeMessages.google_connect(lang) : NativeMessages.agy_account_add(lang)) { GoogleLoop.shared.start { model.act($0, $1) } }
                DisclosureGroup(NativeMessages.google_client_own(lang)) {
                    TextField(NativeMessages.native_google_client_id(lang), text: $gid)
                        .textInputAutocapitalization(.never).autocorrectionDisabled()
                        .onChange(of: gid) { _, t in model.field("gid", t) }
                    SecureField(NativeMessages.google_client_secret(lang), text: $gsecret)
                        .onChange(of: gsecret) { _, t in model.field("gsecret", t) }
                    Button(NativeMessages.google_client_save(lang)) { model.act("google", "configure") }
                }
                if let u = URL(string: settings.google.url), !settings.google.url.isEmpty {
                    Link(NativeMessages.google_open(lang), destination: u)
                    TextField(NativeMessages.native_google_redirect(lang), text: $gpaste)
                        .textInputAutocapitalization(.never).autocorrectionDisabled()
                        .onChange(of: gpaste) { _, t in model.field("gpaste", t) }
                    Button(NativeMessages.common_finish(lang)) { model.act("google", "finish") }
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
                    .swipeActions { Button(NativeMessages.machines_unlink(lang), role: .destructive) { model.act("peer-revoke", p.id) } }
                }
                HStack {
                    TextField(NativeMessages.native_machine_address(lang), text: $purl)
                        .textInputAutocapitalization(.never).autocorrectionDisabled().keyboardType(.URL)
                        .onChange(of: purl) { _, t in model.field("purl", t) }
                    Button(NativeMessages.machines_invite(lang)) { model.act("peer-invite") }
                }
                if !settings.invite.isEmpty {
                    Text(settings.invite).font(.caption.monospaced()).textSelection(.enabled)
                    Button(NativeMessages.native_machine_invite_copy(lang), systemImage: "doc.on.doc") { model.act("copy", settings.invite) }
                }
                HStack {
                    TextField(NativeMessages.native_machine_invite_paste(lang), text: $join)
                        .textInputAutocapitalization(.never).autocorrectionDisabled()
                        .onChange(of: join) { _, t in model.field("invite", t) }
                    Button(NativeMessages.bots_invite_join(lang)) { model.act("peer-join"); join = "" }
                }
            } header: {
                Text(NativeMessages.bots_machines(lang))
            }
            Section {
                Button(NativeMessages.bots_delete(lang), role: .destructive) { deleting = true }
            }
        }
        .onChange(of: settings.google.url) { _, u in GoogleLoop.shared.open(u) }
        .onAppear {
            persona = settings.personaField.isEmpty ? settings.persona : settings.personaField
            gid = settings.google.gid
            gsecret = settings.google.gsecret
            purl = settings.purl
        }
        .confirmationDialog(NativeMessages.native_bot_delete_ask(lang, bot.name), isPresented: $deleting, titleVisibility: .visible) {
            Button(NativeMessages.common_delete(lang), role: .destructive) { model.act("bot-delete", bot.id) }
        }
    }
}

// a room, or the conversation with a bot on a linked machine: posts and a
// composer (field is its "bfield", send the action that posts it)
struct TalkScreen: View {
    @Environment(\.backplaneLanguage) private var lang
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
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
                                .contextMenu { Button(NativeMessages.common_copy(lang), systemImage: "doc.on.doc") { model.act("copy", p.text) } }
                        }
                        .frame(maxWidth: .infinity, alignment: p.mine ? .trailing : .leading)
                    }
                    Color.clear.frame(height: 1).id("end")
                }
                .padding()
            }
            .defaultScrollAnchor(.bottom)
            .scrollDismissesKeyboard(.interactively)
            .onChange(of: bot.posts?.count) { withAnimation(reduceMotion ? nil : .default) { proxy.scrollTo("end", anchor: .bottom) } }
            .onChange(of: model.scrolls) { proxy.scrollTo("end", anchor: .bottom) }
        }
        .overlay { if (bot.posts ?? []).isEmpty { ContentUnavailableView(NativeMessages.native_bot_messages_empty(lang), systemImage: "bubble.left.and.bubble.right") } }
        .safeAreaInset(edge: .top, spacing: 0) {
            if bot.kind == "remote" {
                BotHeader(model: model, bot: bot, size: 48).padding(.horizontal).padding(.vertical, 8).background(.bar)
            }
        }
        .safeAreaInset(edge: .bottom) {
            HStack(alignment: .bottom, spacing: 8) {
                TextField(bot.kind == "room" ? NativeMessages.native_room_post(lang) : NativeMessages.native_bot_message(lang), text: $text, axis: .vertical)
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
                .accessibilityLabel(NativeMessages.composer_send(lang))
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
                        Button(NativeMessages.room_delete(lang), systemImage: "trash", role: .destructive) { model.act("room-delete", bot.id) }
                    } label: {
                        Image(systemName: "ellipsis.circle")
                    }
                }
            }
        }
        .onAppear { text = bot.draft ?? "" }
    }
}
