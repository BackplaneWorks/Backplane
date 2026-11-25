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
        EnterprisePushSection(client: client)
    }
}

/// Alerts through the person's Enterprise server, for a hub with no APNs key of its own: sign in with an account token, and this phone registers there.
struct EnterprisePushSection: View {
    @Bindable var client: DataClient
    private let ep = EnterprisePush.shared
    @State private var url = ""
    @State private var token = ""
    @State private var err = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            DHeading(text: Tr.t("Enterprise alerts"))
            if ep.signedIn { signedIn } else { form }
            let shown = err.isEmpty ? ep.lastError : err
            if !shown.isEmpty { Text(shown).dmono(11).foregroundStyle(BackplaneDesign.bad).accessibilityIdentifier("notify.enterprise.error") }
        }
        .padding(.top, 8)
    }

    @ViewBuilder private var form: some View {
        TextField("https://enterprise.example.com", text: $url).dmono(13).textInputAutocapitalization(.never).autocorrectionDisabled().keyboardType(.URL)
            .padding(.horizontal, 10).frame(height: 38).dbox().accessibilityIdentifier("notify.enterprise.url")
        SecureField("bpe_u_…", text: $token).dmono(13).textInputAutocapitalization(.never).autocorrectionDisabled()
            .padding(.horizontal, 10).frame(height: 38).dbox().accessibilityIdentifier("notify.enterprise.token")
        Button(Tr.t("Sign in")) { signIn() }.buttonStyle(DButton(primary: true))
            .disabled(ep.busy || url.trimmingCharacters(in: .whitespaces).isEmpty || token.trimmingCharacters(in: .whitespaces).isEmpty)
            .accessibilityIdentifier("notify.enterprise.signin")
    }

    @ViewBuilder private var signedIn: some View {
        VStack(alignment: .leading, spacing: 4) {
            if !ep.accountEmail.isEmpty { Text(ep.accountEmail).dmono(13).foregroundStyle(BackplaneDesign.text).accessibilityIdentifier("notify.enterprise.email") }
            Text(ep.baseURL).dmono(11).foregroundStyle(BackplaneDesign.dim).lineLimit(1).truncationMode(.middle).accessibilityIdentifier("notify.enterprise.server")
            if !ep.lastStatus.isEmpty { Text(ep.lastStatus).dmono(11).foregroundStyle(BackplaneDesign.faint).accessibilityIdentifier("notify.enterprise.status") }
        }
        Button(Tr.t("Sign out")) { Task { await ep.signOut(apnsToken: client.apnsToken) } }.buttonStyle(DButton(primary: false))
            .disabled(ep.busy).accessibilityIdentifier("notify.enterprise.signout")
    }

    private func signIn() {
        Task {
            err = await ep.signIn(urlString: url, token: token) ?? ""
            guard err.isEmpty else { return }
            token = ""; url = ""
            if client.apnsToken != nil { await client.registerEnterprise() } else { await client.enablePush(ask: true) }
        }
    }
}
