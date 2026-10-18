import SwiftUI
import UserNotifications

// The tab bar (ios-snappy): Home, Threads, Bots, Routines, Settings. Each tab
// has its own stack, kept by AppModel, and its own scroll position (the
// tabs stay alive); Bend has one selection, which AppModel re-selects when a
// tab comes up. Every view here is an existing one moved, or a list over
// what the screen already carries.
struct AppTabs: View {
    @Bindable var model: AppModel
    let screen: Screen
    @Binding var pairing: Bool

    private func badge(_ t: AppTab) -> Int {
        switch t {
        case .home: screen.tabs?.home?.badge ?? 0
        case .bots: screen.tabs?.bots?.badge ?? 0
        case .routines: screen.tabs?.routines?.badge ?? 0
        default: 0
        }
    }

    private func stack(_ t: AppTab) -> some View {
        NavigationStack(path: Binding(get: { model.stacks[t] ?? [] }, set: { if model.tab == t { model.navigate($0) } })) {
            root(t)
                .navigationDestination(for: String.self) { id in
                    if id == "@hubs" {
                        HubsView(model: model, screen: model.screen ?? screen)
                    } else if id.hasPrefix("@hub:") {
                        HubSettingsPage(model: model, key: String(id.dropFirst(5)))
                    } else {
                        ThreadDestination(model: model, id: id)
                    }
                }
        }
    }

    @ViewBuilder private func root(_ t: AppTab) -> some View {
        switch t {
        case .home: HomeTab(model: model, screen: screen)
        case .threads: ProjectsView(model: model, screen: screen, pairing: $pairing)
        case .bots: BotsTab(model: model, screen: screen)
        case .routines: RoutinesPage(model: model, screen: screen)
        case .settings: AppSettingsTab(model: model, screen: screen)
        }
    }

    private var tabs: some View {
        TabView(selection: Binding(get: { model.tab }, set: { model.switchTab($0) })) {
            ForEach(AppTab.allCases) { t in
                stack(t)
                    .tabItem { Label(t.title, systemImage: t.icon) }
                    .badge(badge(t))
                    .tag(t)
            }
        }
    }

    var body: some View {
        // (iPad, iOS 18: the same tabs as a sidebar)
        Group {
            if #available(iOS 18.0, *) {
                tabs.tabViewStyle(.sidebarAdaptable)
            } else {
                tabs
            }
        }
        .sheet(isPresented: Binding(get: { screen.newBot != nil }, set: { if !$0 { model.act("form-close", "@bnew") } })) {
            if let f = model.screen?.newBot { NewBotSheet(model: model, form: f) }
        }
        .sheet(isPresented: Binding(get: { screen.newRoom != nil }, set: { if !$0 { model.act("form-close", "@rnew") } })) {
            if let f = model.screen?.newRoom { NewRoomSheet(model: model, form: f) }
        }
    }
}

// Bots
// ----

struct BotsTab: View {
    let model: AppModel
    let screen: Screen
    @State private var part = "bots"

    var body: some View {
        List {
            Section {
                Picker("Show", selection: $part) {
                    Text("Bots").tag("bots")
                    Text("Rooms").tag("rooms")
                }
                .pickerStyle(.segmented)
                .listRowBackground(Color.clear)
            }
            if part == "bots" {
                Section {
                    ForEach(screen.bots) { b in
                        Button { model.open(nav: b.nav ?? "", action: b.act ?? (b.remote ? "remote" : "bot"), value: b.id, title: b.name, cat: b.cat, kind: b.remote ? "remote" : "bot") } label: {
                            HStack(spacing: 12) {
                                BotCat(model: model, key: b.cat, size: 36)
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(b.name).foregroundStyle(.primary)
                                    HStack(spacing: 5) {
                                        MoodDot(mood: b.mood)
                                        Text([b.note, b.peer, b.machine ?? ""].filter { !$0.isEmpty }.joined(separator: " · ")).lineLimit(1)
                                    }
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                                }
                                Spacer()
                                Image(systemName: "chevron.right").font(.caption).foregroundStyle(.tertiary)
                            }
                            .contentShape(Rectangle())
                        }
                        // plain: a List's button tints its whole label (names read as links)
                        .buttonStyle(.plain)
                        .opacity(b.mood == "away" ? 0.5 : 1)
                    }
                }
            } else {
                Section {
                    ForEach(screen.rooms) { r in
                        Button { model.open(nav: r.nav ?? "", action: "room", value: r.id, title: r.name, kind: "room") } label: {
                            HStack(spacing: 12) {
                                Image(systemName: "bubble.left.and.bubble.right").frame(width: 36).foregroundStyle(.secondary)
                                Text(r.name).foregroundStyle(.primary)
                                Spacer()
                                Label("\(r.members)", systemImage: "person.2").font(.caption).foregroundStyle(.secondary)
                                Image(systemName: "chevron.right").font(.caption).foregroundStyle(.tertiary)
                            }
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
        }
        .overlay {
            if part == "bots" && screen.bots.isEmpty {
                ContentUnavailableView("No bots yet", systemImage: "pawprint", description: Text("Make one with the plus button."))
            } else if part == "rooms" && screen.rooms.isEmpty {
                ContentUnavailableView("No rooms yet", systemImage: "bubble.left.and.bubble.right", description: Text("Make one with the plus button."))
            }
        }
        .navigationTitle("Bots")
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
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

// Routines
// --------

struct RoutinesPage: View {
    let model: AppModel
    let screen: Screen

    private static func icon(_ state: String) -> String {
        switch state {
        case "off": "pause.circle"
        case "done": "checkmark.circle"
        case "reminder": "bell"
        default: "clock.arrow.circlepath"
        }
    }

    private func when(_ r: RoutineRow) -> String {
        [r.state == "done" ? (r.doneText ?? "") : r.sub, r.zone ?? "", r.ago.isEmpty ? "" : "last " + r.ago + " ago"].filter { !$0.isEmpty }.joined(separator: " · ")
    }

    var body: some View {
        let rows = screen.tabs?.routines?.rows ?? []
        List {
            ForEach(rows) { r in
                Button { model.open(nav: r.nav ?? "", action: r.act, value: r.value, title: r.bot ?? r.title, kind: "routine") } label: {
                    HStack(spacing: 12) {
                        Image(systemName: Self.icon(r.state))
                            .frame(width: 24).foregroundStyle(r.state == "off" || r.state == "done" ? Color.secondary : PhaseColor.accent)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(r.title).foregroundStyle(.primary).lineLimit(1)
                            Text([r.bot ?? "", r.machine ?? ""].filter { !$0.isEmpty }.joined(separator: " · ")).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                            Text(when(r)).font(.caption).foregroundStyle(.secondary).lineLimit(2)
                        }
                        Spacer()
                        Image(systemName: "chevron.right").font(.caption).foregroundStyle(.tertiary)
                    }
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .opacity((r.away ?? false) || r.state == "off" || r.state == "done" ? 0.5 : 1)
            }
        }
        .overlay {
            if rows.isEmpty {
                ContentUnavailableView("No routines", systemImage: "clock.arrow.circlepath", description: Text("A bot's Routines tab makes them."))
            }
        }
        .navigationTitle("Routines")
    }
}

// Settings
// --------

// On this device, instant: nothing here waits on a hub. The hub's own
// settings are one row away, on a page that fills in as the hub answers.
struct AppSettingsTab: View {
    let model: AppModel
    let screen: Screen
    @Bindable private var settings = AppSettings.shared
    @State private var notifications: UNAuthorizationStatus = .notDetermined
    @State private var cacheBytes = 0
    @State private var confirmClear = false

    private var listBinding: Binding<String> {
        Binding(get: { settings.threadList.isEmpty ? (screen.view ?? "projects") : settings.threadList }, set: { model.threadList($0) })
    }

    // the hub whose notification policy is shown: the one in focus
    private var notifyHub: HubRow? { screen.hubs.first { $0.key == screen.hub } ?? screen.hubs.first }

    private var notifyRows: [SetRow]? {
        guard let h = notifyHub else { return nil }
        let r = (screen.hub == h.key ? screen.settings?.rows : nil) ?? model.hubSettings[h.key]?.rows
        return r
    }

    private var notificationWord: String {
        switch notifications {
        case .authorized, .provisional, .ephemeral: "On"
        case .denied: "Off"
        default: "Not asked"
        }
    }

    private var about: String {
        let i = Bundle.main.infoDictionary
        return "\(i?["CFBundleShortVersionString"] as? String ?? "?") (\(i?["CFBundleVersion"] as? String ?? "?"))"
    }

    var body: some View {
        Form {
            Section("Appearance") {
                Picker("Appearance", selection: $settings.appearance) {
                    Text("System").tag("system")
                    Text("Light").tag("light")
                    Text("Dark").tag("dark")
                }
                .pickerStyle(.segmented)
                if let row = screen.lang?.rows.first {
                    Picker(model.t("settings.lang", "Language"), selection: Binding(get: { settings.lang }, set: { model.language($0) })) {
                        ForEach(row.buttons, id: \.value) { Text($0.label).tag($0.value) }
                    }
                    .pickerStyle(.segmented)
                    .task { await model.want("settings.lang") }
                }
                if let row = screen.lang?.rows.dropFirst().first, let button = row.buttons.first {
                    Toggle(row.label, isOn: Binding(get: { button.on }, set: { _ in model.act(button.action, button.value) }))
                }
            }
            Section {
                Picker("Thread list", selection: listBinding) {
                    Text("Projects").tag("projects")
                    Text("Active").tag("active")
                }
                .pickerStyle(.segmented)
                Picker("Opens on", selection: $settings.defaultTab) {
                    Text("Automatic").tag("")
                    ForEach(AppTab.allCases.filter { $0 != .settings }) { Text($0.title).tag($0.rawValue) }
                }
                Toggle("Haptics", isOn: $settings.haptics)
            } header: {
                Text("Behaviour")
            } footer: {
                Text("Projects groups threads by project; Active lists what is going on, what needs you first. Automatic opens on Home with an uberbot, else Threads.")
            }
            Section {
                HStack {
                    Text("Notifications")
                    Spacer()
                    Text(notificationWord).foregroundStyle(.secondary)
                }
                Button("Open system settings") {
                    if let u = URL(string: UIApplication.openNotificationSettingsURLString) { UIApplication.shared.open(u) }
                }
                // what the hub alerts for: its settings, from the last screen that carried them
                if let r = notifyRows { HubSettingsRows(model: model, rows: r, notify: true) }
            } header: {
                Text("Notifications")
            } footer: {
                if screen.hubs.count > 1, let h = notifyHub { Text("Notify me for is a setting on \(h.name).") }
            }
            Section("Hubs") {
                NavigationLink(value: "@hubs") {
                    HStack {
                        Text("Hubs")
                        Spacer()
                        Text("\(screen.hubs.count)").foregroundStyle(.secondary)
                    }
                }
                ForEach(screen.hubs, id: \.key) { h in
                    NavigationLink(value: "@hub:" + h.key) {
                        Text(screen.hubs.count > 1 ? "Hub settings · \(h.name)" : "Hub settings")
                    }
                }
            }
            Section {
                HStack {
                    Text("Kept state")
                    Spacer()
                    Text(ByteCountFormatter.string(fromByteCount: Int64(cacheBytes), countStyle: .file)).foregroundStyle(.secondary)
                }
                Button("Clear cached state", role: .destructive) { confirmClear = true }
            } header: {
                Text("Cache")
            } footer: {
                Text("The next launch fetches each hub's history again.")
            }
            Section("Tour") {
                Button("Show the tour") { model.act("ob-open") }
            }
            Section("About") {
                HStack {
                    Text("Backplane")
                    Spacer()
                    Text(about).foregroundStyle(.secondary)
                }
            }
        }
        .navigationTitle("Settings")
        .task { notifications = await UNUserNotificationCenter.current().notificationSettings().authorizationStatus }
        .onAppear {
            cacheBytes = model.cacheSize()
            // ask the hub for its settings in the background; the rows above
            // show the last ones meanwhile, and a select (a tab change) closes it
            if let h = notifyHub { model.act("hub-settings", h.key + "|on") }
        }
        .confirmationDialog("Clear the cached state?", isPresented: $confirmClear, titleVisibility: .visible) {
            Button("Clear cached state", role: .destructive) { model.clearCache(); cacheBytes = 0 }
            Button("Cancel", role: .cancel) {}
        }
    }
}

// The hub's settings (the rows Bend sends for Settings), on a page of their
// own: it shows at once with the rows last seen (kept on disk), asks the hub
// in the background and takes the fresh rows in place; with none yet, the
// page's shape as placeholders. Leaving it closes the hub's Settings.
struct HubSettingsPage: View {
    let model: AppModel
    let key: String

    private var rows: [SetRow]? {
        if let s = model.screen, s.hub == key, let st = s.settings { return st.rows }
        return model.hubSettings[key]?.rows
    }

    private var name: String {
        model.screen?.hubs.first { $0.key == key }?.name ?? key
    }

    var body: some View {
        Form {
            if let r = rows {
                HubSettingsRows(model: model, rows: r)
            } else {
                ForEach(0..<7, id: \.self) { _ in
                    Section {
                        VStack(alignment: .leading, spacing: 8) {
                            Text("Setting name")
                            Text("A line of what the setting does for this hub.").font(.footnote)
                            HStack { Text("Option"); Text("Option"); Text("Option") }
                        }
                        .padding(.vertical, 4)
                    }
                }
                .redacted(reason: .placeholder)
                .allowsHitTesting(false)
            }
        }
        .navigationTitle("Hub settings · \(name)")
        .navigationBarTitleDisplayMode(.inline)
        .onAppear { model.act("hub-settings", key + "|on") }
        .onDisappear { model.act("hub-settings", key + "|off") }
    }
}
