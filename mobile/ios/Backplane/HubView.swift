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
                Text(panel.text).dmono(14).foregroundStyle(BackplaneDesign.text).accessibilityIdentifier("hub.text")
                ForEach(panel.actions) { a in
                    Button { asking = a } label: { Text(a.label).frame(maxWidth: .infinity, alignment: .leading) }
                        .buttonStyle(DButton()).disabled(busy).accessibilityIdentifier("hub.action." + a.id)
                }
                if !answer.isEmpty { Text(answer).dmono(13).foregroundStyle(BackplaneDesign.text).accessibilityIdentifier("hub.answer") }
                if !progress.isEmpty { Text(progress).dmono(13).foregroundStyle(BackplaneDesign.dim).accessibilityIdentifier("hub.progress") }
                if fetch != .loading {
                    Button("Refresh") { fetch = .loading; Task { await load() } }.buttonStyle(DButton()).disabled(busy).accessibilityIdentifier("hub.refresh")
                }
                ForEach(Array(panel.incidents.enumerated()), id: \.offset) { _, i in
                    DLine()
                    Text(dStamp(Int(i.atMs))).dmono(11).foregroundStyle(BackplaneDesign.faint)
                    Text([i.what, i.why, i.did].filter { !$0.isEmpty }.joined(separator: " · ")).dmono(12).foregroundStyle(BackplaneDesign.dim)
                }
            }.padding(16).frame(maxWidth: .infinity, alignment: .leading)
        }
        .background(BackplaneDesign.background)
        .dataBar("Hub")
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
        .alert((asking?.label ?? "") + "?", isPresented: Binding(get: { asking != nil }, set: { if !$0 { asking = nil } }), presenting: asking) { a in
            Button(a.label) { send(a) }
            Button("Cancel", role: .cancel) {}
        } message: { a in if !a.text.isEmpty { Text(a.text) } }
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
                        Text(client.notice.text).dmono(12).foregroundStyle(BackplaneDesign.text).lineLimit(1)
                        Spacer(minLength: 0)
                    }.padding(.horizontal, 12).padding(.vertical, 6).background(BackplaneDesign.raised)
                    DLine()
                }
            }.buttonStyle(.plain).accessibilityIdentifier("hub.banner")
        }
    }
}
