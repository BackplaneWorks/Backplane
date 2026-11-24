import SwiftUI

// Settings, Ring (feature `ring`, docs/ring.md): the hub's rows (ring.page), drawn by the rows view Voice and Notifications use: the Pebble Index ring's Set up, the
// webhook's URL and header to copy or share into the Pebble app on this phone, when the test event came, and the recordings with Play. Copy, share and Play are the
// phone's own (DPageLocal); What happened (link-open) opens the thread here; the rest goes to ring.page.act {action, value, fields}, whose answer is the page again.
enum RingLogic {
    static func hasPendingWork(_ rows: [DVoiceRow]) -> Bool {
        rows.contains { r in
            r.note.contains("being heard now") || r.note.contains("waiting to be heard") || r.note.contains("waiting to go to Pebble")
        }
    }
}

@MainActor
@Observable
final class RingModel {
    private(set) var rows: [DVoiceRow] = []
    private(set) var error = ""
    @ObservationIgnored weak var client: DataClient?

    func load() async {
        guard let client else { return }
        do {
            let r: DVoicePage = try await client.call("ring.page")
            rows = DVoiceRow.parse(r.rows)
            error = ""
        } catch {
            self.error = error.localizedDescription
        }
    }
    /// A press on a row's button; returns the hub's error for the row, if any.
    func act(action: String, value: String, fields: [String: String]) async -> String {
        guard let client else { return "" }
        if action == "link-open" { if let u = URL(string: value) { client.openLink(u, thread: "") }; return "" }
        do {
            let r: DVoicePage = try await client.call("ring.page.act", ["action": action, "value": value, "fields": fields])
            rows = DVoiceRow.parse(r.rows); error = ""; return ""
        } catch { await load(); return error.localizedDescription }
    }
}

struct DataRingSection: View {
    @Bindable var client: DataClient
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            if !client.ring.error.isEmpty { Text(Tr.t(client.ring.error)).dmono(12).foregroundStyle(BackplaneDesign.bad) }
            DPageRows(title: Tr.t("Ring"), idPrefix: "ring", rows: client.ring.rows, empty: Tr.t("Not connected")) { _, b, fields in
                await client.ring.act(action: b.action, value: b.value, fields: fields)
            }
        }
        .task(id: client.conn) { if client.conn == .online { await client.ring.load() } }
        .task(id: "\(client.conn.rawValue)|\(RingLogic.hasPendingWork(client.ring.rows))") {
            while !Task.isCancelled, client.conn == .online, RingLogic.hasPendingWork(client.ring.rows) {
                try? await Task.sleep(for: .seconds(2))
                guard !Task.isCancelled, client.conn == .online else { break }
                await client.ring.load()
            }
        }
    }
}
