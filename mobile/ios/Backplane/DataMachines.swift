import SwiftUI

// Settings, Machines: the hub's rows (machines.page), drawn by the same rows view as Voice. A press goes to machines.page.act {action, value, fields}.
@MainActor
@Observable
final class MachinesModel {
    private(set) var rows: [DVoiceRow] = []
    /// The hub says the page changes by itself now (a sign-in on another machine runs): read again every 2 s while it does.
    private(set) var busy = false
    @ObservationIgnored weak var client: DataClient?

    func load() async {
        guard let client, let r: DMachinesPage = try? await client.call("machines.page") else { return }
        rows = DVoiceRow.parse(r.rows); busy = r.busy
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
        // while a sign-in there runs, its link, code and screen change without a press: the page is read again every 2 s, as the desktop redraws
        .task(id: "\(client.conn)-\(client.machines.busy)") {
            while client.conn == .online, client.machines.busy, !Task.isCancelled {
                try? await Task.sleep(nanoseconds: 2_000_000_000)
                if Task.isCancelled { break }
                await client.machines.load()
            }
        }
    }
}

/// `machines.page`: the rows, and whether they change by themselves now.
struct DMachinesPage: Decodable {
    var rows: [[String: Any]]
    var busy: Bool
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: K.self)
        rows = (try c.decode(DJSON.self, forKey: .rows)).value as? [[String: Any]] ?? []
        busy = (try? c.decode(Bool.self, forKey: .busy)) ?? false
    }
    enum K: String, CodingKey { case rows, busy }
}
