import SwiftUI

// Settings, Machines: the hub's rows (machines.page), drawn by the same rows view as Voice. A press goes to machines.page.act {action, value, fields}.
@MainActor
@Observable
final class MachinesModel {
    private(set) var rows: [DVoiceRow] = []
    @ObservationIgnored weak var client: DataClient?

    func load() async {
        guard let client, let r: DVoicePage = try? await client.call("machines.page") else { return }
        rows = DVoiceRow.parse(r.rows)
    }
    /// A press on a row's button; returns the hub's error for the row, if any.
    func act(action: String, value: String, fields: [String: String]) async -> String {
        guard let client else { return "" }
        do {
            _ = try await client.call("machines.page.act", ["action": action, "value": value, "fields": fields], as: DEmpty.self)
            await load()
            return ""
        } catch { await load(); return error.localizedDescription }
    }
}

struct DataMachinesSection: View {
    @Bindable var client: DataClient
    var body: some View {
        DPageRows(title: Tr.t("Machines"), idPrefix: "machines", rows: client.machines.rows, empty: Tr.t("Not connected")) { _, b, fields in
            await client.machines.act(action: b.action, value: b.value, fields: fields)
        }
        .task(id: client.conn) { if client.conn == .online { await client.machines.load() } }
    }
}
