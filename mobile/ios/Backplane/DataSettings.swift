import SwiftUI

// Settings: a top-level page of groups, each opening its own page (Accounts & Usage, Hubs, Agents, Machines & devices, Notifications, Appearance, Updates,
// Improvements), and a search over every setting's name that opens the page it is on. Every setting of this phone keeps its key (AppSettings) and its
// accessibility id; only where it is drawn moved. Accounts & Usage is the accounts page (DataAccounts.swift) with a bar a limit window.

/// A page under Settings (DPage name); Accounts & Usage and Improvements are pages of their own ("accounts", "improvements").
enum DSettingsPage: String, CaseIterable {
    case hubs = "settings.hubs", agents = "settings.agents", machines = "settings.machines", notifications = "settings.notifications",
         appearance = "settings.appearance", updates = "settings.updates"
}

/// One row of the top level: where it goes, its icon, its title, and whether this hub has it.
private struct DSettingsGroup: Identifiable {
    let id: String; let icon: String; let title: String; let page: DPage
}

/// One setting by name, for the search: its words and the page it is on.
private struct DSettingsEntry: Identifiable {
    let title: String; let page: DPage; let group: String
    var id: String { page.name + "/" + title }
}

struct DataSettingsView: View {
    @Bindable var client: DataClient
    @Bindable private var settings = AppSettings.shared
    @State private var query = ""
    @State private var accounts: DAccountsPage?

    private func groups() -> [[DSettingsGroup]] {
        var top: [DSettingsGroup] = []
        if client.has("accounts") { top.append(DSettingsGroup(id: "settings.accounts", icon: "person.2", title: Tr.t("Accounts & Usage"), page: DPage(name: "accounts"))) }
        top.append(DSettingsGroup(id: "settings.hubs", icon: "server.rack", title: Tr.t("Hubs"), page: DPage(name: DSettingsPage.hubs.rawValue)))
        var work: [DSettingsGroup] = []
        if client.has("mcpsettings") || client.has("voice") || client.has("cats") || client.bots.contains(where: { $0.delete }) { work.append(DSettingsGroup(id: "settings.agents", icon: "cpu", title: Tr.t("Agents"), page: DPage(name: DSettingsPage.agents.rawValue))) }
        if client.has("machines") || client.has("devices") { work.append(DSettingsGroup(id: "settings.machines", icon: "desktopcomputer", title: Tr.t("Machines & devices"), page: DPage(name: DSettingsPage.machines.rawValue))) }
        let phone = [DSettingsGroup(id: "settings.notifications", icon: "bell", title: Tr.t("Notifications"), page: DPage(name: DSettingsPage.notifications.rawValue)),
                     DSettingsGroup(id: "settings.appearance", icon: "textformat", title: Tr.t("Appearance"), page: DPage(name: DSettingsPage.appearance.rawValue))]
        let about = [DSettingsGroup(id: "settings.updates", icon: "arrow.down.circle", title: Tr.t("Updates"), page: DPage(name: DSettingsPage.updates.rawValue)),
                     DSettingsGroup(id: "settings.improve", icon: "lightbulb", title: Tr.t("Improvements"), page: DPage(name: "improvements"))]
        return [top, work, phone, about].filter { !$0.isEmpty }
    }
    /// Every setting by name, on the page it is drawn on.
    private func entries() -> [DSettingsEntry] {
        let g = Dictionary(groups().flatMap { $0 }.map { ($0.page.name, $0.title) }, uniquingKeysWith: { a, _ in a })
        func e(_ t: String, _ p: String) -> DSettingsEntry? { g[p].map { DSettingsEntry(title: t, page: DPage(name: p), group: $0) } }
        let (hubs, agents, machines, notify, look, updates) = (DSettingsPage.hubs.rawValue, DSettingsPage.agents.rawValue, DSettingsPage.machines.rawValue,
                                                               DSettingsPage.notifications.rawValue, DSettingsPage.appearance.rawValue, DSettingsPage.updates.rawValue)
        var all: [DSettingsEntry?] = [
            e(Tr.t("Accounts"), "accounts"), e(Tr.t("Usage"), "accounts"),
            e(Tr.t("Hubs"), hubs), e(Tr.t("Hub"), hubs), e(Tr.t("Add a hub"), hubs), e(Tr.t("Reconnect"), hubs),
            e(Tr.t("Alerts on this phone"), notify), e(Tr.t("Alert sound"), notify), e(Tr.t("Haptics"), notify), e(Tr.t("Quiet hours"), notify),
            e(Tr.k("settings.lang"), look), e(Tr.t("Appearance"), look), e(Tr.t("Text size"), look), e(Tr.k("ios.viewerbg"), look),
            e(Tr.t("Updates"), updates), e(Tr.t("Improvements"), "improvements"),
        ]
        if client.has("notifysettings") { all.append(e(Tr.t("Notifications"), notify)) }
        if client.has("ring") { all.append(e(Tr.t("Ring"), notify)) }
        if client.has("cats") { all += [e(Tr.t("Your uberbot"), agents), e(Tr.t("Name"), agents), e(Tr.t("Colour"), agents), e(Tr.t("Cat"), agents)] }
        if client.bots.contains(where: { $0.delete }) { all += [e(Tr.k("bots.settings.head"), agents), e(Tr.t("Delete bot"), agents)] }
        if client.has("mcpsettings") { all.append(e(Tr.t("MCP servers"), agents)) }
        if client.has("voice") { all.append(e(Tr.t("Voice"), agents)) }
        if client.has("machines") { all.append(e(Tr.t("Machines"), machines)) }
        if client.has("devices") { all.append(e(Tr.t("Devices"), machines)) }
        return all.compactMap { $0 }
    }
    private var found: [DSettingsEntry] {
        let q = query.trimmingCharacters(in: .whitespaces)
        return q.isEmpty ? [] : entries().filter { $0.title.localizedCaseInsensitiveContains(q) || $0.group.localizedCaseInsensitiveContains(q) }
    }
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                HStack(spacing: 8) {
                    Image(systemName: "magnifyingglass").font(.system(size: 12)).foregroundStyle(BackplaneDesign.faint)
                    TextField(Tr.t("Search settings"), text: $query).dmono(13).textInputAutocapitalization(.never).autocorrectionDisabled().accessibilityIdentifier("settings.search")
                    if !query.isEmpty { Button { query = "" } label: { Image(systemName: "xmark").font(.system(size: 11)).foregroundStyle(BackplaneDesign.faint) }.accessibilityLabel(Tr.t("Clear")) }
                }.padding(.horizontal, 10).frame(height: 38).dbox()
                if !query.trimmingCharacters(in: .whitespaces).isEmpty {
                    let f = found
                    if f.isEmpty { Text(Tr.t("No matches")).dmono(12).foregroundStyle(BackplaneDesign.faint) }
                    VStack(spacing: 0) {
                        ForEach(f) { x in
                            NavigationLink(value: x.page) {
                                HStack { Text(x.title).dmono(13).foregroundStyle(BackplaneDesign.text); Spacer(); Text(x.group).dmono(11).foregroundStyle(BackplaneDesign.faint)
                                    Image(systemName: "chevron.right").font(.system(size: 11)).foregroundStyle(BackplaneDesign.faint) }
                                    .padding(.horizontal, 12).frame(minHeight: 44).contentShape(Rectangle())
                            }.buttonStyle(.plain).accessibilityIdentifier("settings.found." + x.id)
                            if x.id != f.last?.id { DLine() }
                        }
                    }.dbox()
                } else {
                    ForEach(Array(groups().enumerated()), id: \.offset) { _, rows in
                        VStack(spacing: 0) {
                            ForEach(rows) { r in
                                NavigationLink(value: r.page) { row(r) }.buttonStyle(.plain).accessibilityIdentifier(r.id)
                                if r.id != rows.last?.id { DLine().padding(.leading, 52) }
                            }
                        }.dbox()
                    }
                    let i = Bundle.main.infoDictionary
                    Text(Tr.ios_version(version: i?["CFBundleShortVersionString"] as? String ?? "?", build: i?["CFBundleVersion"] as? String ?? "?")).dmono(11).foregroundStyle(BackplaneDesign.faint)
                        .frame(maxWidth: .infinity)
                }
            }.padding(16)
        }
        .scrollDismissesKeyboard(.immediately)
        .background(BackplaneDesign.background)
        .dataBar(Tr.t("Settings"))
        .task(id: client.conn) { if client.has("accounts") { accounts = try? await client.call("accounts.page") } }
    }
    private func row(_ r: DSettingsGroup) -> some View {
        HStack(spacing: 12) {
            Image(systemName: r.icon).font(.system(size: 14)).foregroundStyle(BackplaneDesign.accent).frame(width: 28, height: 28).background(BackplaneDesign.raised)
            VStack(alignment: .leading, spacing: 4) {
                Text(r.title).dmono(14).foregroundStyle(BackplaneDesign.text)
                summary(r)
            }
            Spacer(minLength: 6)
            Image(systemName: "chevron.right").font(.system(size: 11)).foregroundStyle(BackplaneDesign.faint)
        }.padding(.horizontal, 12).padding(.vertical, 10).frame(minHeight: 52).contentShape(Rectangle())
    }
    /// A row's one line of what it holds now: the fullest window of any account, the hub in use, this phone's choices.
    @ViewBuilder private func summary(_ r: DSettingsGroup) -> some View {
        switch r.id {
        case "settings.accounts":
            if let p = accounts {
                let rows = (p.own ?? []) + p.providers.flatMap { $0.accounts }
                let all = p.people.map { $0.flatMap { $0.logins.flatMap { $0.bars } } } ?? rows.flatMap { $0.bars ?? [] }
                if let b = all.max(by: { $0.used < $1.used }) { DUsageBar(bar: b, compact: true) }
                else { Text(Tr.ios_settings_accounts_n(n: p.people?.count ?? rows.count)).dmono(11).foregroundStyle(BackplaneDesign.faint) }
            }
        case "settings.hubs":
            Text(client.hubName.isEmpty ? Tr.t(client.conn.rawValue) : "\(client.hubName) · \(Tr.t(client.conn.rawValue))").dmono(11).foregroundStyle(BackplaneDesign.faint).lineLimit(1)
        case "settings.notifications":
            Text(Tr.t(settings.alerts ? "Alerts on" : "Alerts off")).dmono(11).foregroundStyle(BackplaneDesign.faint)
        default: EmptyView()
        }
    }
}

/// Settings, Agents: Bots. This hub's bots the hub offers Delete bot for (`bots.list` rows with `delete`; never the uberbot), each with Delete bot,
/// asked first with the hub's words (`ask`: the bot by name, what goes with it).
struct DataBotsSection: View {
    @Bindable var client: DataClient
    @State private var ask: DBot?
    var body: some View {
        let bots = client.bots.filter { $0.delete }
        VStack(alignment: .leading, spacing: 10) {
            if !bots.isEmpty {
                DHeading(text: Tr.k("bots.settings.head"))
                ForEach(bots) { b in
                    HStack(spacing: 8) {
                        Text(b.name).dmono(13).lineLimit(1)
                        Spacer()
                        Button(Tr.t("Delete bot"), role: .destructive) { ask = b }.buttonStyle(DButton()).accessibilityIdentifier("settings.bot.delete.\(b.id)")
                    }
                }
                DLine()
            }
        }
        .task(id: client.conn) { if client.conn == .online { await client.loadBots() } }
        .botDeleteDialog(client: client, bot: $ask)
    }
}

extension View {
    /// Delete bot's confirm (a long press on a bot row, or Settings' Bots): the hub's words, then `bot.delete`.
    func botDeleteDialog(client: DataClient, bot: Binding<DBot?>) -> some View {
        let a = bot.wrappedValue.map(DBotAsk.of)
        return confirmationDialog(a?.title ?? "", isPresented: Binding(get: { bot.wrappedValue != nil }, set: { if !$0 { bot.wrappedValue = nil } }), titleVisibility: .visible) {
            Button(a?.yes ?? Tr.t("Delete bot"), role: .destructive) { if let b = bot.wrappedValue { Task { await client.botDelete(b.id) } } }
            Button(Tr.t("Cancel"), role: .cancel) {}
        } message: { if let body = a?.body, !body.isEmpty { Text(body) } }
    }
}

/// A page under Settings: the same sections as before, grouped.
struct DataSettingsPageView: View {
    @Bindable var client: DataClient
    let page: DSettingsPage
    @Bindable private var settings = AppSettings.shared
    @State private var forgetKey: String?

    private func onOff(_ title: String, _ on: Binding<Bool>, id: String) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            DHeading(text: title)
            DChoices(options: [(label: Tr.t("on"), value: true), (label: Tr.t("off"), value: false)], value: on).accessibilityIdentifier(id)
        }
    }
    private var title: String {
        switch page {
        case .hubs: Tr.t("Hubs")
        case .agents: Tr.t("Agents")
        case .machines: Tr.t("Machines & devices")
        case .notifications: Tr.t("Notifications")
        case .appearance: Tr.t("Appearance")
        case .updates: Tr.t("Updates")
        }
    }
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                switch page {
                case .appearance: appearance
                case .notifications: notifications
                case .hubs: hubs
                case .agents:
                    if client.has("cats") { DataUberSection(client: client); if client.has("mcpsettings") || client.has("voice") { DLine() } }
                    DataBotsSection(client: client)
                    if client.has("mcpsettings") { NavigationLink(value: DPage(name: "mcp")) { Text(Tr.t("MCP servers")) }.buttonStyle(DButton()).accessibilityIdentifier("settings.mcp") }
                    if client.has("voice") { if client.has("mcpsettings") { DLine() }; DataVoiceSection(client: client) }
                case .machines:
                    if client.has("machines") { DataMachinesSection(client: client) }
                    if client.has("devices") { if client.has("machines") { DLine() }; DataDevicesSection(client: client) }
                case .updates: DataUpdateSection(client: client)
                }
            }.padding(16).frame(maxWidth: .infinity, alignment: .leading)
        }
        .background(BackplaneDesign.background)
        .dataBar(title)
        .confirmationDialog(Tr.ios_hub_forget_ask(hub: forgetKey ?? Tr.t("this hub")), isPresented: Binding(get: { forgetKey != nil }, set: { if !$0 { forgetKey = nil } }), titleVisibility: .visible) {
            Button(Tr.t("Forget"), role: .destructive) { if let k = forgetKey { client.forgetHub(k) }; forgetKey = nil }
        } message: { Text(Tr.t("This phone's saved copy of its threads is removed. Pair again to come back.")) }
    }
    @ViewBuilder private var appearance: some View {
        DHeading(text: Tr.k("settings.lang"))
        DChoices(options: [(label: Tr.k("settings.lang.auto"), value: "auto"), (label: Tr.k("settings.lang.en"), value: "en"), (label: Tr.k("settings.lang.ja"), value: "ja"), (label: Tr.k("settings.lang.es"), value: "es")], value: $settings.lang).accessibilityIdentifier("settings.lang")
        Text(Tr.k("settings.lang.note")).dmono(11).foregroundStyle(BackplaneDesign.dim)
        DHeading(text: Tr.t("Appearance"))
        DChoices(options: [(label: Tr.t("system"), value: "system"), (label: Tr.t("light"), value: "light"), (label: Tr.t("dark"), value: "dark")], value: $settings.appearance)
        DHeading(text: Tr.t("Text size"))
        DChoices(options: [(label: Tr.t("system"), value: "system")] + AppSettings.textSteps.map { (label: $0.tag, value: $0.tag) }, value: $settings.textSize).accessibilityIdentifier("settings.textsize")
        DLine()
        DataViewerBackgroundSection(settings: settings)
    }
    @ViewBuilder private var notifications: some View {
        onOff(Tr.t("Alerts on this phone"), $settings.alerts, id: "settings.alerts")
            .onChange(of: settings.alerts) { _, on in Task { if on { await client.enablePush(ask: true) }; await client.pushMute(); if client.has("notifysettings") { await client.notify.load() } } }
        if settings.alerts { onOff(Tr.t("Alert sound"), $settings.alertSound, id: "settings.sound") }
        onOff(Tr.t("Haptics"), $settings.haptics, id: "settings.haptics")
        Text(Tr.t("Off: only what you let through reaches this phone. Alerts come from your uberbot.")).dmono(11).foregroundStyle(BackplaneDesign.dim)
        // an older hub has no Notifications page: its quiet hours and urgent switch are drawn here
        if client.has("push") && !client.has("notifysettings") { DLine(); DataQuietSection(client: client) }
        if client.has("notifysettings") { DLine(); DataNotifySection(client: client) }
        if client.has("ring") { DLine(); DataRingSection(client: client) }
    }
    @ViewBuilder private var hubs: some View {
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
    }
}

/// A limit window as a bar: its name, the share used filled in its tone, and the hub's words (used, when it resets). `compact`: one line, for the Settings row.
struct DUsageBar: View {
    let bar: DAccBar
    var compact = false
    var body: some View {
        let tone = dTone(bar.tone)
        let fill = GeometryReader { g in
            ZStack(alignment: .leading) {
                Rectangle().fill(BackplaneDesign.raised)
                Rectangle().fill(tone).frame(width: g.size.width * min(max(bar.used, 0), 100) / 100)
            }
        }.frame(height: compact ? 4 : 6).overlay(Rectangle().strokeBorder(BackplaneDesign.line, lineWidth: compact ? 0 : 1))
        if compact {
            HStack(spacing: 8) { Text(bar.name).dmono(11).foregroundStyle(BackplaneDesign.faint); fill.frame(maxWidth: 120); Text(verbatim: "\(Int(bar.used.rounded()))%").dmono(11).foregroundStyle(tone) }
        } else {
            VStack(alignment: .leading, spacing: 4) {
                HStack { Text(bar.name).dmono(11, bold: true).foregroundStyle(BackplaneDesign.dim); Spacer(); Text(Tr.t(bar.text)).dmono(11).foregroundStyle(bar.tone == "ok" ? BackplaneDesign.faint : tone).lineLimit(1) }
                fill
            }.accessibilityElement(children: .combine)
        }
    }
}
