import SwiftUI

// Settings, Notifications: the hub's rows (notify.page), drawn by the same rows view as Voice and Machines: which alerts reach the person, quiet hours, urgent asks, each
// phone's mute. The hub owns the settings and the rules; a press goes to notify.page.act {action, value, fields} and the page is read again.
@MainActor
@Observable
final class NotifyModel {
    private(set) var rows: [DVoiceRow] = []
    @ObservationIgnored weak var client: DataClient?

    func load() async {
        guard let client, let r: DVoicePage = try? await client.call("notify.page") else { return }
        rows = DVoiceRow.parse(r.rows)
    }
    /// A press on a row's button; returns the hub's error for the row, if any.
    func act(action: String, value: String, fields: [String: String]) async -> String {
        guard let client else { return "" }
        do {
            _ = try await client.call("notify.page.act", ["action": action, "value": value, "fields": fields], as: DEmpty.self)
            // this phone's own row is its Alerts switch too: keep the switch in step, or the next connect (registerPush -> pushMute) would undo the press
            if action == "notify-mute", let eq = value.firstIndex(of: "="), !client.pushHandle.isEmpty, String(value[..<eq]) == client.pushHandle {
                AppSettings.shared.alerts = value[value.index(after: eq)...] == "off"
            }
            await load()
            return ""
        } catch { await load(); return error.localizedDescription }
    }
}

struct DataNotifySection: View {
    @Bindable var client: DataClient
    var body: some View {
        DPageRows(title: Tr.t("Notifications"), idPrefix: "notify", rows: client.notify.rows, empty: Tr.t("Not connected")) { _, b, fields in
            await client.notify.act(action: b.action, value: b.value, fields: fields)
        }
        .task(id: client.conn) { if client.conn == .online { await client.notify.load() } }
    }
}
