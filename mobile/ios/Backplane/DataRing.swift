import SwiftUI

// Settings, Ring (feature `ring`, docs/ring.md): the hub's rows (ring.page), drawn by the rows view Voice and Notifications use: the Pebble Index ring's Set up, the
// webhook's URL and header to copy or share into the Pebble app on this phone, when the test event came, and the recordings with Play. Copy, share and Play are the
// phone's own (DPageLocal); What happened (link-open) opens the thread here; the rest goes to ring.page.act {action, value, fields}, whose answer is the page again.
@MainActor
@Observable
final class RingModel {
    private(set) var rows: [DVoiceRow] = []
    @ObservationIgnored weak var client: DataClient?

    func load() async {
        guard let client, let r: DVoicePage = try? await client.call("ring.page") else { return }
        rows = DVoiceRow.parse(r.rows)
    }
    /// A press on a row's button; returns the hub's error for the row, if any.
    func act(action: String, value: String, fields: [String: String]) async -> String {
        guard let client else { return "" }
        if action == "link-open" { if let u = URL(string: value) { client.openLink(u, thread: "") }; return "" }
        do {
            let r: DVoicePage = try await client.call("ring.page.act", ["action": action, "value": value, "fields": fields])
            rows = DVoiceRow.parse(r.rows); return ""
        } catch { await load(); return error.localizedDescription }
    }
}

struct DataRingSection: View {
    @Bindable var client: DataClient
    var body: some View {
        DPageRows(title: Tr.t("Ring"), idPrefix: "ring", rows: client.ring.rows, empty: Tr.t("Not connected")) { _, b, fields in
            await client.ring.act(action: b.action, value: b.value, fields: fields)
        }
        .task(id: client.conn) { if client.conn == .online { await client.ring.load() } }
    }
}
