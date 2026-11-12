import SwiftUI

// The Hub screen (docs/PHONE-DATA.md): native, on plain HTTP, reachable with the socket down. What it shows is HubRescue's; the buttons are the hub's. The same as Android's HubScreen.kt.
struct DataHubView: View {
    let client: DataClient
    @Environment(\.scenePhase) private var phase
    @State private var http: HubHttp?
    @State private var fetch: HubFetch = .loading
    @State private var asking: HubAction?
    @State private var sending = false
    @State private var answer = ""
    @State private var progress = ""
    @State private var waitFrom: Date?
    @State private var moved = false

    private struct Wait: Equatable { var from: Date?; var active: Bool }
    private var busy: Bool { sending || waitFrom != nil }

    private func load() async {
        let f = await http?.status() ?? .unreachable
        fetch = f
        if case .got(let s) = f, let n = s.notice { client.heardNotice(n) }
    }
    private func send(_ a: HubAction) {
        sending = true; answer = ""; progress = ""
        Task {
            let after = HubRescue.after(await http?.control(a.id) ?? .unreachable)
            answer = after.text; sending = false
            if after.poll { moved = false; waitFrom = Date() }
        }
    }
    // ends with the screen, pauses with the app; the three minutes keep counting from the send
    private func poll() async {
        guard let from = waitFrom, phase == .active else { return }
        while !Task.isCancelled {
            try? await Task.sleep(for: .milliseconds(HubRescue.pollMs))
            if Task.isCancelled { return }
            let h = await http?.health()
            if Task.isCancelled { return }
            let s = HubRescue.step(h, elapsedMs: Int64(Date().timeIntervalSince(from) * 1000), moved: moved)
            moved = s.moved; progress = s.text
            if s.done { await load(); waitFrom = nil; return }
        }
    }

    var body: some View {
        let panel = HubRescue.panel(fetch)
        ScrollView {
            VStack(alignment: .leading, spacing: 10) {
                Text(client.hubName).dmono(12).foregroundStyle(BackplaneDesign.dim).lineLimit(1)
                Text(Tr.t(panel.text)).dmono(14).foregroundStyle(BackplaneDesign.text).accessibilityIdentifier("hub.text")
                ForEach(panel.actions) { a in
                    Button { asking = a } label: { Text(Tr.t(a.label)).frame(maxWidth: .infinity, alignment: .leading) }
                        .buttonStyle(DButton()).disabled(busy).accessibilityIdentifier("hub.action." + a.id)
                }
                if !answer.isEmpty { Text(Tr.t(answer)).dmono(13).foregroundStyle(BackplaneDesign.text).accessibilityIdentifier("hub.answer") }
                if !progress.isEmpty { Text(Tr.t(progress)).dmono(13).foregroundStyle(BackplaneDesign.dim).accessibilityIdentifier("hub.progress") }
                if fetch != .loading {
                    Button(Tr.t("Refresh")) { fetch = .loading; Task { await load() } }.buttonStyle(DButton()).disabled(busy).accessibilityIdentifier("hub.refresh")
                }
                ForEach(Array(panel.incidents.enumerated()), id: \.offset) { _, i in
                    DLine()
                    Text(dStamp(Int(i.atMs))).dmono(11).foregroundStyle(BackplaneDesign.faint)
                    Text(Tr.t([i.what, i.why, i.did].filter { !$0.isEmpty }.joined(separator: " · "))).dmono(12).foregroundStyle(BackplaneDesign.dim)
                }
            }.padding(16).frame(maxWidth: .infinity, alignment: .leading)
        }
        .background(BackplaneDesign.background)
        .dataBar(Tr.t("Hub"))
        .task {
            http = client.hubHttp(); await load()
            #if DEBUG
            // a check without hands: BACKPLANE_HUB_ACT names an action to ask about, !name sends it as if confirmed
            if let t = ProcessInfo.processInfo.environment["BACKPLANE_HUB_ACT"], let a = HubRescue.panel(fetch).actions.first(where: { $0.id == t.replacingOccurrences(of: "!", with: "") }) {
                if t.hasPrefix("!") { send(a) } else { asking = a }
            }
            #endif
        }
        .task(id: Wait(from: waitFrom, active: phase == .active)) { await poll() }
        .alert(Tr.t(asking?.label ?? "") + "?", isPresented: Binding(get: { asking != nil }, set: { if !$0 { asking = nil } }), presenting: asking) { a in
            Button(Tr.t(a.label)) { send(a) }
            Button(Tr.t("Cancel"), role: .cancel) {}
        } message: { a in if !a.text.isEmpty { Text(Tr.t(a.text)) } }
    }
}

/// The hub's own word that something is off (hello `notice`), one line; a tap opens the Hub screen.
struct DataHubBanner: View {
    let client: DataClient
    var body: some View {
        if !client.notice.text.isEmpty {
            NavigationLink(value: DPage(name: "hub")) {
                VStack(spacing: 0) {
                    HStack(spacing: 8) {
                        DDot(color: client.notice.warn ? BackplaneDesign.warn : BackplaneDesign.faint, size: 8)
                        Text(Tr.t(client.notice.text)).dmono(12).foregroundStyle(BackplaneDesign.text).lineLimit(1)
                        Spacer(minLength: 0)
                    }.padding(.horizontal, 12).padding(.vertical, 6).background(BackplaneDesign.raised)
                    DLine()
                }
            }.buttonStyle(.plain).accessibilityIdentifier("hub.banner")
        }
    }
}

/// Alerts, offered once the person sees why (DataClient.alertsAsk): one line; Allow alerts shows the system's question, Not now hides it until the next launch.
struct DataAlertsBanner: View {
    let client: DataClient
    var body: some View {
        if client.alertsAsk && client.conn == .online {
            VStack(spacing: 0) {
                HStack(spacing: 8) {
                    Text(Tr.t("Bub can alert you when a thread needs you.")).dmono(12).foregroundStyle(BackplaneDesign.text).lineLimit(2)
                    Spacer(minLength: 0)
                    Button(Tr.t("Allow alerts")) { Task { await client.enablePush(ask: true) } }.buttonStyle(DButton(primary: true)).accessibilityIdentifier("alerts.allow")
                    Button(Tr.t("Not now")) { client.alertsAsk = false }.dmono(12).foregroundStyle(BackplaneDesign.dim).accessibilityIdentifier("alerts.later")
                }.padding(.horizontal, 12).padding(.vertical, 6).background(BackplaneDesign.raised)
                DLine()
            }
        }
    }
}

/// The one-time telemetry notice (DataClient.telemetryNotice), one line: Turn off turns telemetry off, Settings opens this app's Settings, ✕ closes it.
/// "shown" goes once it is really on screen: drawn while the app is active.
struct DataTelemetryBanner: View {
    let client: DataClient
    @Environment(\.scenePhase) private var phase
    var body: some View {
        if let n = client.telemetryNotice {
            VStack(spacing: 0) {
                HStack(spacing: 8) {
                    Text(Tr.t(n.text)).dmono(12).foregroundStyle(BackplaneDesign.text).lineLimit(3)
                    Spacer(minLength: 0)
                    Button(n.off.isEmpty ? Tr.t("Turn off") : Tr.t(n.off)) { client.telemetryAct("off") }
                        .buttonStyle(DButton(primary: true)).accessibilityIdentifier("telemetry.off")
                    Button(n.settings.isEmpty ? Tr.t("Settings") : Tr.t(n.settings)) { client.openSettings = true }
                        .dmono(12).foregroundStyle(BackplaneDesign.dim).accessibilityIdentifier("telemetry.settings")
                    Button { client.telemetryAct("close") } label: { Image(systemName: "xmark").font(.system(size: 12)).foregroundStyle(BackplaneDesign.dim).frame(width: 28, height: 28) }
                        .accessibilityLabel(Tr.t("Close")).accessibilityIdentifier("telemetry.close")
                }.padding(.horizontal, 12).padding(.vertical, 6).background(BackplaneDesign.raised)
                DLine()
            }
            .task(id: "\(n.token)|\(phase == .active)") { if phase == .active { client.telemetryShown(n.token) } }
        }
    }
}

/// The GitHub star suggestion (DataClient.star): a small card under the top bar with the hub's title, the repository, why stars help, a Star on
/// GitHub (opens the page, decides open), Later (a week, decides later) and Don't ask again (decides dismiss). It waits for a pause (DStarOffer.due).
struct DataStarBanner: View {
    let client: DataClient
    @Environment(\.openURL) private var openURL
    var body: some View {
        if let s = client.star, DStarOffer.due(s, running: client.threads.contains { $0.state == "working" || $0.state == "asking" }, typing: client.drafts.values.contains { !$0.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }) {
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 8) {
                    Image(systemName: "star.fill").font(.system(size: 14)).foregroundStyle(BackplaneDesign.accent)
                    Text(Tr.t(s.text)).dmono(13, bold: true).foregroundStyle(BackplaneDesign.text)
                }
                Text(s.place).dmono(11).foregroundStyle(BackplaneDesign.faint)
                if let w = s.why, !w.isEmpty { Text(Tr.t(w)).dmono(12).foregroundStyle(BackplaneDesign.dim).fixedSize(horizontal: false, vertical: true) }
                Button { if let u = URL(string: s.url) { openURL(u) }; client.starDecide("open") } label: {
                    HStack(spacing: 8) { Image(systemName: "star.fill"); Text(Tr.t(s.label.isEmpty ? "Star on GitHub" : s.label)).dmono(13, bold: true) }
                        .frame(maxWidth: .infinity, minHeight: 34).foregroundStyle(BackplaneDesign.onAccent).background(BackplaneDesign.accent)
                }.buttonStyle(.plain).accessibilityIdentifier("star.open")
                HStack(spacing: 8) {
                    Button(Tr.t("Later")) { client.starDecide("later") }.buttonStyle(DButton()).frame(maxWidth: .infinity).accessibilityIdentifier("star.later")
                    Button(Tr.t("Don't ask again")) { client.starDecide("dismiss") }.buttonStyle(DButton()).frame(maxWidth: .infinity).accessibilityIdentifier("star.dismiss")
                }
            }.padding(12).background(BackplaneDesign.raised).overlay(Rectangle().stroke(BackplaneDesign.accent, lineWidth: 1)).padding(8)
        }
    }
}

/// A 0.14 home coming across (DataClient.migration): the hub's title, its bar and the first two of its lines, as the notice banner; a tap opens the card.
/// It has no close: it goes when one of the card's buttons is answered.
struct DataMigrationBanner: View {
    let client: DataClient
    var body: some View {
        if let c = client.migration.card {
            Button { client.migrationOpen = true } label: {
                VStack(spacing: 0) {
                    VStack(alignment: .leading, spacing: 4) {
                        HStack(spacing: 8) {
                            DDot(color: dMigColor(c), size: 8)
                            Text(Tr.t(c.title)).dmono(12).foregroundStyle(BackplaneDesign.text).lineLimit(1)
                            Spacer(minLength: 0)
                        }
                        if let f = c.fill { DMigBar(fill: f) }
                        ForEach(Array(c.short.enumerated()), id: \.offset) { _, l in Text(Tr.t(l)).dmono(11).foregroundStyle(BackplaneDesign.dim).lineLimit(1) }
                    }.padding(.horizontal, 12).padding(.vertical, 6).frame(maxWidth: .infinity, alignment: .leading).background(BackplaneDesign.raised)
                    DLine()
                }
            }.buttonStyle(.plain).accessibilityIdentifier("migration.banner")
        }
    }
}

/// The whole card: title, body, bar, every line and the hub's buttons (onboard.act). While it runs it has no button; it closes back to the banner.
struct DataMigrationCard: View {
    let client: DataClient
    var body: some View {
        ScrollView {
            if let c = client.migration.card {
                VStack(alignment: .leading, spacing: 12) {
                    Text(Tr.t(c.title)).dmono(15, bold: true).foregroundStyle(BackplaneDesign.text).accessibilityIdentifier("migration.title")
                    if !c.body.isEmpty { Text(Tr.t(c.body)).dmono(13).foregroundStyle(BackplaneDesign.text).fixedSize(horizontal: false, vertical: true) }
                    if let f = c.fill { DMigBar(fill: f, height: 6) }
                    VStack(alignment: .leading, spacing: 4) {
                        ForEach(Array(c.lines.enumerated()), id: \.offset) { _, l in Text(Tr.t(l)).dmono(12).foregroundStyle(BackplaneDesign.dim).fixedSize(horizontal: false, vertical: true) }
                    }
                    if !c.buttons.isEmpty {
                        HStack(spacing: 8) {
                            ForEach(Array(c.buttons.enumerated()), id: \.offset) { _, b in
                                Button(Tr.t(b.label)) { client.migrationAct(b) }.buttonStyle(DButton(primary: b.on)).disabled(client.migrationBusy)
                                    .accessibilityIdentifier("migration.button.\(b.value)")
                            }
                        }
                    }
                    if !client.migrationError.isEmpty { Text(Tr.t(client.migrationError)).dmono(12).foregroundStyle(BackplaneDesign.warn) }
                }.padding(16).frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .presentationDetents([.medium, .large])
        .dataSheet()
        .accessibilityIdentifier("migration.card")
    }
}

/// The import's bar: the line colour, filled in accent to the hub's pct. Square; it moves only when a frame comes.
struct DMigBar: View {
    let fill: Double
    var height: CGFloat = 3
    var body: some View {
        GeometryReader { g in
            ZStack(alignment: .leading) {
                Rectangle().fill(BackplaneDesign.line)
                Rectangle().fill(BackplaneDesign.accent).frame(width: g.size.width * fill)
            }
        }.frame(height: height).accessibilityHidden(true)
    }
}
func dMigColor(_ c: DMigCard) -> Color { switch c.state { case "failed": BackplaneDesign.bad; case "done": BackplaneDesign.ok; default: BackplaneDesign.accent } }
