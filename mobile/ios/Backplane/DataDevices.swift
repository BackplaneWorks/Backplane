import SwiftUI

// Settings, Devices (feature `devices`): the hub's rows (devices.page, native/host/src/net/devicespage.rs), drawn by the rows view Voice and Machines use, plus the
// pairing code's QR. The hub decides: the address chips, the pairing link with its QR, the local network switch, the paired devices and open links with Revoke.
// A press goes to devices.page.act {action, value, state} and its rows replace these. `state` is opaque: the last one goes back with every call, none to another hub
// (the same as the Android app's DataDevices.kt).
@MainActor
@Observable
final class DevicesModel {
    private(set) var rows: [DVoiceRow] = []
    private(set) var error = ""
    @ObservationIgnored private var hub = ""
    @ObservationIgnored private var state = ""
    @ObservationIgnored weak var client: DataClient?

    /// The state to send to `h`: none when the phone has moved to another hub since.
    private func params(_ h: String, _ p: [String: Any] = [:]) -> [String: Any] {
        if h != hub { hub = h; state = ""; rows = []; error = "" }
        var p = p; if !state.isEmpty { p["state"] = state }; return p
    }
    /// A reply from `h`: dropped when the phone is on another hub now; a reply without a state keeps the last.
    private func took(_ h: String, _ r: DDevicesPage) {
        guard h == hub else { return }
        if let s = r.state { state = s }
        rows = DVoiceRow.parse(r.rows); error = ""
    }
    func load() async {
        guard let client else { return }
        let h = client.hubName
        do { took(h, try await client.call("devices.page", params(h), as: DDevicesPage.self)) }
        catch { if h == hub { self.error = error.localizedDescription } }
    }
    func act(action: String, value: String) async -> String {
        guard let client else { return "" }
        let h = client.hubName
        do { took(h, try await client.call("devices.page.act", params(h, ["action": action, "value": value]), as: DDevicesPage.self)); return "" }
        catch { return error.localizedDescription }
    }
}

struct DDevicesPage: Decodable {
    var rows: [[String: Any]]
    var state: String?
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: K.self)
        rows = (try? c.decode(DJSON.self, forKey: .rows))?.value as? [[String: Any]] ?? []
        state = try? c.decode(String.self, forKey: .state)
    }
    enum K: String, CodingKey { case rows, state }
}

struct DataDevicesSection: View {
    @Bindable var client: DataClient
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            DPageRows(title: "Devices", idPrefix: "devices", rows: client.devices.rows, empty: "Not connected") { _, b, _ in
                await client.devices.act(action: b.action, value: b.value)
            }
            if !client.devices.error.isEmpty { Text(client.devices.error).dmono(11).foregroundStyle(BackplaneDesign.bad) }
        }
        .task(id: "\(client.conn)|\(client.hubName)") { if client.conn == .online { await client.devices.load() } }
    }
}

/// A code to scan: one string of "0"/"1" a module row ("1" dark), drawn dark on white with four modules of margin, square, as wide as it may be.
struct DQRCode: View {
    let rows: [String]
    var body: some View {
        let n = rows.map(\.count).max() ?? 0
        Canvas { ctx, size in
            let side = min(size.width, size.height); let m = side / CGFloat(n + 8)
            ctx.fill(Path(CGRect(x: 0, y: 0, width: side, height: side)), with: .color(.white))
            for (y, r) in rows.enumerated() {
                for (x, ch) in r.enumerated() where ch == "1" {
                    ctx.fill(Path(CGRect(x: CGFloat(x + 4) * m, y: CGFloat(y + 4) * m, width: m + 0.5, height: m + 0.5)), with: .color(.black))
                }
            }
        }
        .aspectRatio(1, contentMode: .fit).frame(maxWidth: 320)
    }
}
