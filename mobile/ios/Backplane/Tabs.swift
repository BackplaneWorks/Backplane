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
    @Environment(\.dynamicTypeSize) private var typeSize
    @Environment(\.horizontalSizeClass) private var horizontalSizeClass

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
                    .toolbar(horizontalSizeClass == .compact ? .hidden : .automatic, for: .tabBar)
                    .tabItem { Label(t.title(screen.locale ?? "en"), systemImage: t.icon) }
                    .badge(badge(t))
                    .tag(t)
            }
        }
    }

    private func tabButton(_ t: AppTab) -> some View {
        let selected = model.tab == t
        let count = badge(t)
        let title = t.title(screen.locale ?? "en")
        return Button { model.switchTab(t) } label: {
            VStack(spacing: 4) {
                HStack(spacing: 3) {
                    Image(systemName: t.icon).font(.body)
                    if count > 0 {
                        Text(String(count)).font(.caption2.weight(.bold).monospacedDigit())
                            .foregroundStyle(BackplaneDesign.onAccent)
                            .padding(.horizontal, 4).padding(.vertical, 1)
                            .background(BackplaneDesign.accent)
                    }
                }
                Text(title).font(.caption2.weight(selected ? .semibold : .regular))
                    .lineLimit(typeSize.isAccessibilitySize ? nil : 2)
                    .multilineTextAlignment(.center)
                    .foregroundStyle(selected ? BackplaneDesign.accentText : Color.primary)
            }
            .frame(maxWidth: .infinity, minHeight: 48)
            .padding(.horizontal, 3).padding(.vertical, 5)
            .foregroundStyle(selected ? BackplaneDesign.accent : Color.primary)
            .background(selected ? BackplaneDesign.raised : BackplaneDesign.panel, ignoresSafeAreaEdges: [])
            .overlay(alignment: .top) {
                if selected { Rectangle().fill(BackplaneDesign.accent).frame(height: 2) }
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(title)
        .accessibilityValue(count > 0 ? String(count) : "")
        .accessibilityAddTraits(selected ? .isSelected : [])
        .accessibilityIdentifier("tab." + t.rawValue)
    }

    private var tabStrip: some View {
        Group {
            if typeSize.isAccessibilitySize {
                ScrollView(.horizontal) {
                    HStack(spacing: 0) {
                        ForEach(AppTab.allCases) { t in tabButton(t).frame(minWidth: 90) }
                    }
                }
                .scrollIndicators(.hidden)
            } else {
                HStack(spacing: 0) {
                    ForEach(AppTab.allCases) { t in tabButton(t) }
                }
            }
        }
        .background(BackplaneDesign.panel)
        .overlay(alignment: .top) { Rectangle().fill(BackplaneDesign.line).frame(height: 1) }
        .background(BackplaneDesign.panel.ignoresSafeArea(edges: .bottom))
    }

    @ViewBuilder private var navigation: some View {
        if horizontalSizeClass == .compact {
            VStack(spacing: 0) {
                tabs.frame(maxWidth: .infinity, maxHeight: .infinity)
                tabStrip
            }
        } else if #available(iOS 18.0, *) {
            tabs.tabViewStyle(.sidebarAdaptable)
        } else {
            tabs
        }
    }

    var body: some View {
        navigation
        .tint(BackplaneDesign.accent)
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
    @Environment(\.backplaneLanguage) private var lang
    let model: AppModel
    let screen: Screen
    @State private var part = "bots"

    var body: some View {
        List {
            Section {
                Picker(NativeMessages.common_show(lang), selection: $part) {
                    Text(NativeMessages.tour_title_bots(lang)).tag("bots")
                    Text(NativeMessages.bots_rooms(lang)).tag("rooms")
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
                        .accessibilityIdentifier("bot.row." + b.id)
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
        .modifier(BackplaneListSurface())
        .overlay {
            if part == "bots" && screen.bots.isEmpty {
                ContentUnavailableView(NativeMessages.native_bot_empty(lang), systemImage: "pawprint", description: Text(NativeMessages.native_bot_create_hint(lang)))
            } else if part == "rooms" && screen.rooms.isEmpty {
                ContentUnavailableView(NativeMessages.native_room_empty(lang), systemImage: "bubble.left.and.bubble.right", description: Text(NativeMessages.native_bot_create_hint(lang)))
            }
        }
        .navigationTitle(NativeMessages.tour_title_bots(lang))
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Menu {
                    // with several hubs, a bot or room is made on the one chosen
                    if screen.hubs.count > 1 {
                        ForEach(screen.hubs, id: \.key) { h in
                            Menu(h.name) {
                                Button(NativeMessages.bots_new(lang), systemImage: "cat") { model.act("bot-new", h.key + "|") }
                                Button(NativeMessages.rooms_new(lang), systemImage: "bubble.left.and.bubble.right") { model.act("room-new", h.key + "|") }
                            }
                        }
                    } else {
                        Button(NativeMessages.bots_new(lang), systemImage: "cat") { model.act("bot-new") }
                        Button(NativeMessages.rooms_new(lang), systemImage: "bubble.left.and.bubble.right") { model.act("room-new") }
                    }
                } label: {
                    Image(systemName: "plus")
                }
                .accessibilityLabel(NativeMessages.native_bot_create_menu(lang))
            }
        }
    }
}

// Routines
// --------

struct RoutinesPage: View {
    @Environment(\.backplaneLanguage) private var lang
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
        [r.state == "done" ? (r.doneText ?? "") : r.sub, r.zone ?? "", r.ago.isEmpty ? "" : NativeMessages.native_routine_last(lang, r.ago)].filter { !$0.isEmpty }.joined(separator: " · ")
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
        .modifier(BackplaneListSurface())
        .overlay {
            if rows.isEmpty {
                ContentUnavailableView(NativeMessages.native_routines_empty(lang), systemImage: "clock.arrow.circlepath", description: Text(NativeMessages.native_routines_note(lang)))
            }
        }
        .navigationTitle(NativeMessages.bots_tab_routines(lang))
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
    @State private var notificationsLoaded = false
    @State private var requestingNotifications = false
    @State private var cacheBytes = 0
    @State private var confirmClear = false

    private var tag: String { screen.locale ?? "en" }

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
        case .authorized, .provisional, .ephemeral: NativeMessages.settings_lang_on(tag)
        case .denied: NativeMessages.settings_lang_off(tag)
        default: NativeMessages.native_settings_notasked(tag)
        }
    }

    private var about: String {
        let i = Bundle.main.infoDictionary
        return "\(i?["CFBundleShortVersionString"] as? String ?? "?") (\(i?["CFBundleVersion"] as? String ?? "?"))"
    }

    var body: some View {
        Form {
            Section(NativeMessages.settings_appearance(tag)) {
                Picker(NativeMessages.settings_appearance(tag), selection: $settings.appearance) {
                    Text(NativeMessages.native_settings_system(tag)).tag("system")
                    Text(NativeMessages.settings_theme_light(tag)).tag("light")
                    Text(NativeMessages.settings_theme_dark(tag)).tag("dark")
                }
                .pickerStyle(.segmented)
                if let row = screen.lang?.rows.first {
                    Picker(NativeMessages.settings_lang(tag), selection: Binding(get: { settings.lang }, set: { model.language($0) })) {
                        ForEach(row.buttons, id: \.value) { Text($0.label).tag($0.value) }
                    }
                    .pickerStyle(.segmented)
                }
                if let row = screen.lang?.rows.dropFirst().first, let button = row.buttons.first {
                    Toggle(row.label, isOn: Binding(get: { button.on }, set: { _ in model.act(button.action, button.value) }))
                }
            }
            Section {
                Picker(NativeMessages.settings_sidebar(tag), selection: listBinding) {
                    Text(NativeMessages.tour_title_projects(tag)).tag("projects")
                    Text(NativeMessages.settings_sidebar_active(tag)).tag("active")
                }
                .pickerStyle(.segmented)
                Picker(NativeMessages.native_settings_opens(tag), selection: $settings.defaultTab) {
                    Text(NativeMessages.settings_lang_auto(tag)).tag("")
                    ForEach(AppTab.allCases.filter { $0 != .settings }) { Text($0.title(tag)).tag($0.rawValue) }
                }
                Toggle(NativeMessages.native_settings_haptics(tag), isOn: $settings.haptics)
            } header: {
                Text(NativeMessages.native_settings_behaviour(tag))
            } footer: {
                Text(NativeMessages.native_settings_behaviour_ios(tag))
            }
            Section {
                HStack {
                    Text(NativeMessages.native_settings_notifications(tag))
                    Spacer()
                    if notificationsLoaded { Text(notificationWord).foregroundStyle(.secondary) }
                }
                if notificationsLoaded && notifications == .notDetermined {
                    Button(tag == "ja" ? "通知を有効にする" : "Enable notifications") {
                        requestingNotifications = true
                        Task {
                            await model.enableNotifications()
                            notifications = await UNUserNotificationCenter.current().notificationSettings().authorizationStatus
                            requestingNotifications = false
                        }
                    }
                    .disabled(requestingNotifications)
                    .accessibilityIdentifier("settings.notifications.enable")
                } else if notificationsLoaded {
                    Button(NativeMessages.native_settings_system_open(tag)) {
                        if let u = URL(string: UIApplication.openNotificationSettingsURLString) { UIApplication.shared.open(u) }
                    }
                }
                // what the hub alerts for: its settings, from the last screen that carried them
                if let r = notifyRows { HubSettingsRows(model: model, rows: r, notify: true) }
            } header: {
                Text(NativeMessages.native_settings_notifications(tag))
            } footer: {
                if screen.hubs.count > 1, let h = notifyHub { Text(NativeMessages.native_settings_notify_hub(tag, h.name)) }
            }
            Section(NativeMessages.native_usage_machines(tag)) {
                ForEach(screen.usageMachines ?? []) { machine in
                    VStack(alignment: .leading, spacing: 8) {
                        HStack {
                            Text(machine.name).font(.headline)
                            Spacer()
                            Text(HubDot.word(model.conn[machine.key]?.phase ?? (machine.online ? .online : .offline), tag))
                                .foregroundStyle(.secondary)
                        }
                        Text(machine.key).font(.caption).foregroundStyle(.secondary)
                        ForEach(Array(machine.rows.enumerated()), id: \.offset) { _, row in
                            VStack(alignment: .leading, spacing: 3) {
                                Text(row.label)
                                if !row.note.isEmpty { Text(row.note).font(.caption).foregroundStyle(.secondary) }
                                ForEach(row.buttons, id: \.self) { button in
                                    Button(button.label) { model.act(button.action, machine.key + "|" + button.value) }
                                        .disabled(model.conn[machine.key].map { $0.phase == .offline || $0.phase == .connecting } ?? !machine.online)
                                }
                            }
                        }
                    }
                    .accessibilityIdentifier("usage.machine." + machine.key)
                }
            }
            Section(NativeMessages.native_settings_hubs(tag)) {
                NavigationLink(value: "@hubs") {
                    HStack {
                        Text(NativeMessages.native_settings_hubs(tag))
                        Spacer()
                        Text("\(screen.hubs.count)").foregroundStyle(.secondary)
                    }
                }
                ForEach(screen.hubs, id: \.key) { h in
                    NavigationLink(value: "@hub:" + h.key) {
                        Text(screen.hubs.count > 1 ? NativeMessages.native_settings_hub_named(tag, h.name) : NativeMessages.native_settings_hub(tag))
                    }
                }
            }
            Section {
                HStack {
                    Text(NativeMessages.native_settings_kept(tag))
                    Spacer()
                    Text(ByteCountFormatter.string(fromByteCount: Int64(cacheBytes), countStyle: .file)).foregroundStyle(.secondary)
                }
                Button(NativeMessages.native_settings_cache_clear(tag), role: .destructive) { confirmClear = true }
            } header: {
                Text(NativeMessages.native_settings_cache(tag))
            } footer: {
                Text(NativeMessages.native_settings_cache_next(tag))
            }
            Section(NativeMessages.settings_tour(tag)) {
                Button(NativeMessages.settings_tour_show(tag)) { model.act("ob-open") }
            }
            Section(NativeMessages.native_settings_about(tag)) {
                HStack {
                    Text("Backplane")
                    Spacer()
                    Text(about).foregroundStyle(.secondary)
                }
            }
        }
        .navigationTitle(NativeMessages.palette_settings(tag))
        .task {
            notifications = await UNUserNotificationCenter.current().notificationSettings().authorizationStatus
            notificationsLoaded = true
        }
        .onAppear {
            model.usage(true)
            cacheBytes = model.cacheSize()
            // ask the hub for its settings in the background; the rows above
            // show the last ones meanwhile, and a select (a tab change) closes it
            if let h = notifyHub { model.act("hub-settings", h.key + "|on") }
        }
        .onDisappear { model.usage(false) }
        .confirmationDialog(NativeMessages.native_settings_cache_ask(tag), isPresented: $confirmClear, titleVisibility: .visible) {
            Button(NativeMessages.native_settings_cache_clear(tag), role: .destructive) { model.clearCache(); cacheBytes = 0 }
            Button(NativeMessages.common_cancel(tag), role: .cancel) {}
        }
    }
}

// The hub's settings (the rows Bend sends for Settings), on a page of their
// own: it shows at once with the rows last seen (kept on disk), asks the hub
// in the background and takes the fresh rows in place; with none yet, the
// page's shape as placeholders. Leaving it closes the hub's Settings.
struct HubSettingsPage: View {
    @Environment(\.backplaneLanguage) private var lang
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
                            Text(NativeMessages.native_settings_placeholder_name(lang))
                            Text(NativeMessages.native_settings_placeholder_hint(lang)).font(.footnote)
                            HStack { Text(NativeMessages.native_settings_placeholder_option(lang)); Text(NativeMessages.native_settings_placeholder_option(lang)); Text(NativeMessages.native_settings_placeholder_option(lang)) }
                        }
                        .padding(.vertical, 4)
                    }
                }
                .redacted(reason: .placeholder)
                .allowsHitTesting(false)
            }
        }
        .navigationTitle(NativeMessages.native_settings_hub_named(lang, name))
        .navigationBarTitleDisplayMode(.inline)
        .onAppear { model.act("hub-settings", key + "|on") }
        .onDisappear { model.act("hub-settings", key + "|off") }
    }
}
