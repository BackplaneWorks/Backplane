import SwiftUI

// Settings, Browser: the hub's rows (browser.page), drawn by the same rows view as Notifications: the start page a new or empty Browser tab opens, with Save,
// Reset to default and Blank. The hub owns the setting and checks the address; a press goes to browser.page.act {action, value, fields} and the page is read again.
@MainActor
@Observable
final class BrowserStartModel {
    private(set) var rows: [DVoiceRow] = []
    @ObservationIgnored weak var client: DataClient?

    func load() async {
        guard let client, let r: DVoicePage = try? await client.call("browser.page") else { return }
        rows = DVoiceRow.parse(r.rows)
    }
    /// A press on a row's button; returns the hub's error for the row, if any (an address that is not http or https).
    func act(action: String, value: String, fields: [String: String]) async -> String {
        guard let client else { return "" }
        do {
            _ = try await client.call("browser.page.act", ["action": action, "value": value, "fields": fields], as: DEmpty.self)
            await load()
            return ""
        } catch { await load(); return error.localizedDescription }
    }
}

struct DataBrowserStartSection: View {
    @Bindable var client: DataClient
    var body: some View {
        DPageRows(title: Tr.t("Browser"), idPrefix: "browser", rows: client.browserStart.rows, empty: Tr.t("Not connected")) { _, b, fields in
            await client.browserStart.act(action: b.action, value: b.value, fields: fields)
        }
        .task(id: client.conn) { if client.conn == .online { await client.browserStart.load() } }
    }
}
