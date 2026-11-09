import SwiftUI
import UIKit

// Settings, Accounts (feature "accounts"; docs/PHONE-DATA.md): the hub's Claude and Codex accounts, and their sign-in from this phone. The hub runs the provider's own sign-in
// and decides every word and button here (native/host/src/settings/accounts_phone.rs); the phone opens the sign-in page in the browser and, for Claude, sends back the code
// that page shows. No password is ever typed here. The same screens as the Android app's DataAccounts.kt.

struct DAccountsPage: Decodable { var providers: [DAccProvider]; var error: String; var note: String }
struct DAccProvider: Decodable, Identifiable { var provider: String; var title: String; var add: String; var accounts: [DAccRow]; var id: String { provider } }
struct DAccRow: Decodable, Identifiable { var id: String; var provider: String; var label: String; var email: String; var state: String; var line: String; var tone: String; var limits: String; var hold: String }
struct DAccReply: Decodable { var account: DAccount }
struct DAccount: Decodable {
    var id: String; var provider: String; var title: String; var label: String; var email: String; var plan: String; var state: String; var line: String; var tone: String
    var message: String; var limits: [String]; var hold: String; var signin: DSignIn?; var actions: [DAccAction]; var remove: String; var loopback: Bool?
}
struct DSignIn: Decodable { var attempt: String; var state: String; var url: String; var code: String; var paste: Bool; var text: String; var error: String; var busy: Bool; var done: Bool }
struct DAccAction: Decodable, Hashable { var label: String; var action: String; var danger: Bool }

func dTone(_ t: String) -> Color { t == "ok" ? BackplaneDesign.ok : t == "bad" ? BackplaneDesign.bad : t == "warn" ? BackplaneDesign.warn : BackplaneDesign.faint }

/// A page of this screen: an account by id, `start` when an alert's tap opened it to sign in again.
struct DAccountRoute: Hashable { var id: String; var start = false }

struct DataAccountsView: View {
    @Bindable var client: DataClient
    @State private var page: DAccountsPage?
    @State private var failure = ""
    @State private var adding: DAccProvider?
    @State private var name = ""
    @State private var opened: DAccountRoute?

    private func load() async {
        do { page = try await client.call("accounts.page"); failure = "" } catch { failure = error.localizedDescription }
    }
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                if let p = page {
                    ForEach(p.providers) { pv in
                        DHeading(text: pv.title)
                        if pv.accounts.isEmpty { Text(Tr.t("No accounts")).dmono(12).foregroundStyle(BackplaneDesign.faint) }
                        ForEach(pv.accounts) { a in
                            Button { opened = DAccountRoute(id: a.id) } label: { row(a) }.buttonStyle(.plain).accessibilityIdentifier("accounts.row." + a.id)
                        }
                        Button(Tr.t(pv.add)) {
                            // Google asks no name: the sign-in starts at once, its own listener first (GoogleLoop)
                            if pv.provider == "google" { Task { await add(pv, "") } } else { name = ""; adding = pv }
                        }.buttonStyle(DButton()).accessibilityIdentifier("accounts.add." + pv.provider)
                        DLine()
                    }
                    if !p.error.isEmpty { Text(Tr.t(p.error)).dmono(11).foregroundStyle(BackplaneDesign.bad) }
                    Text(Tr.t(p.note)).dmono(11).foregroundStyle(BackplaneDesign.dim)
                } else if !failure.isEmpty { Text(Tr.t(failure)).dmono(12).foregroundStyle(BackplaneDesign.bad) }
                else { DLoading(text: Tr.t("Asking the hub…")) }
            }.padding(16)
        }
        .background(BackplaneDesign.background)
        .dataBar(Tr.t("Accounts"))
        .refreshable { await load() }
        .task(id: client.conn) { await load() }
        .navigationDestination(item: $opened) { r in DataAccountView(client: client, route: r, moved: { opened = DAccountRoute(id: $0) }).id(r.id) }
        .onChange(of: opened) { _, o in if o == nil { Task { await load() } } }
        .alert(Tr.t(adding.map { $0.add } ?? ""), isPresented: Binding(get: { adding != nil }, set: { if !$0 { adding = nil } })) {
            TextField(Tr.t("Name (Work, Personal)"), text: $name)
            Button(Tr.t("Add and sign in")) {
                guard let pv = adding else { return }
                Task { await add(pv, name) }
            }
            Button(Tr.t("Cancel"), role: .cancel) {}
        }
    }
    /// An account added: Google's with its loopback listener up first and its address sent as `redirect` (no listener: none sent, the paste path works as is).
    private func add(_ pv: DAccProvider, _ label: String) async {
        var p: [String: Any] = ["action": "add", "provider": pv.provider, "label": label]
        let google = pv.provider == "google"
        let port = google ? await GoogleLoop.shared.listen() : nil
        if let port { p["redirect"] = GoogleLoopLogic.redirect(port) }
        do {
            let r: DAccReply = try await client.call("account.act", p)
            if port != nil { GoogleLoop.shared.profile = r.account.id }
            opened = DAccountRoute(id: r.account.id)
        } catch { GoogleLoop.shared.stop(); failure = error.localizedDescription }
        await load()
    }
    private func row(_ a: DAccRow) -> some View {
        HStack(alignment: .top, spacing: 10) {
            DDot(color: dTone(a.tone), size: 8).padding(.top, 5)
            VStack(alignment: .leading, spacing: 3) {
                HStack { Text(a.label).dmono(14, bold: true).foregroundStyle(BackplaneDesign.text).lineLimit(1); Spacer(); Text(Tr.t(a.line)).dmono(11).foregroundStyle(dTone(a.tone)) }
                if !a.email.isEmpty { Text(a.email).dmono(11).foregroundStyle(BackplaneDesign.dim).lineLimit(1) }
                if !a.limits.isEmpty { Text(Tr.t(a.limits)).dmono(11).foregroundStyle(BackplaneDesign.faint).lineLimit(2) }
                if !a.hold.isEmpty { Text(Tr.t(a.hold)).dmono(11).foregroundStyle(BackplaneDesign.warn) }
            }
            Image(systemName: "chevron.right").font(.system(size: 11)).foregroundStyle(BackplaneDesign.faint).padding(.top, 4)
        }.padding(10).frame(maxWidth: .infinity, alignment: .leading).dbox().contentShape(Rectangle())
    }
}

struct DataAccountView: View {
    @Bindable var client: DataClient
    let route: DAccountRoute
    /// A signed-in Google account is google:<id>: its own screen replaces google:new.
    var moved: (String) -> Void = { _ in }
    @Environment(\.openURL) private var openURL
    @Environment(\.dismiss) private var dismiss
    @State private var a: DAccount?
    @State private var failure = ""
    @State private var code = ""
    @State private var confirmRemove = false
    @State private var started = false
    @State private var copied = ""

    private func load() async {
        do { let r: DAccReply = try await client.call("account.view", ["profile": route.id]); a = r.account; failure = "" } catch { failure = error.localizedDescription }
    }
    private func act(_ action: String, _ extra: [String: Any] = [:]) async {
        var p: [String: Any] = ["profile": route.id, "action": action]; for (k, v) in extra { p[k] = v }
        if action == "remove" {
            do { let _: DAccountsPage = try await client.call("account.act", p); GoogleLoop.shared.stop(); dismiss() } catch { failure = error.localizedDescription }
            return
        }
        let port = action == "signin" && GoogleLoopLogic.wants(route.id, loopback: a?.loopback == true) ? await GoogleLoop.shared.listen() : nil
        if let port { p["redirect"] = GoogleLoopLogic.redirect(port) }
        do {
            let r: DAccReply = try await client.call("account.act", p); a = r.account; failure = ""; if action == "code" { code = "" }
            if port != nil { GoogleLoop.shared.profile = r.account.id.isEmpty ? route.id : r.account.id }
        } catch { if port != nil { GoogleLoop.shared.stop() }; failure = error.localizedDescription; await load() }
    }
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                if let a {
                    HStack(spacing: 8) { DDot(color: dTone(a.tone), size: 8); Text(Tr.t(a.line)).dmono(13, bold: true).foregroundStyle(dTone(a.tone)) }.accessibilityIdentifier("account.state")
                    if !a.email.isEmpty { Text(a.plan.isEmpty ? a.email : "\(a.email) · \(a.plan)").dmono(12).foregroundStyle(BackplaneDesign.dim) }
                    if !a.message.isEmpty { Text(Tr.t(a.message)).dmono(12).foregroundStyle(BackplaneDesign.dim) }
                    ForEach(a.limits, id: \.self) { Text(Tr.t($0)).dmono(12).foregroundStyle(BackplaneDesign.faint) }
                    if !a.hold.isEmpty { Text(Tr.t(a.hold)).dmono(12).foregroundStyle(BackplaneDesign.warn) }
                    if let s = a.signin { DLine(); signin(s) }
                    DLine()
                    ForEach(a.actions, id: \.self) { b in
                        Button(Tr.t(b.label)) { if b.action == "remove" { confirmRemove = true } else { Task { await act(b.action) } } }
                            .buttonStyle(DButton(primary: b.action == "signin" && a.state == "expired", danger: b.danger)).accessibilityIdentifier("account.act." + b.action)
                    }
                    if !failure.isEmpty { Text(Tr.t(failure)).dmono(12).foregroundStyle(BackplaneDesign.bad).accessibilityIdentifier("account.error") }
                } else if !failure.isEmpty { Text(Tr.t(failure)).dmono(12).foregroundStyle(BackplaneDesign.bad) }
                else { DLoading(text: Tr.t("Asking the hub…")) }
            }.padding(16).frame(maxWidth: .infinity, alignment: .leading)
        }
        .background(BackplaneDesign.background)
        .dataBar(a.map { "\($0.title) · \($0.label)" } ?? Tr.t("Account"))
        .task(id: client.conn) {
            await load()
            // opened from the alert: the sign-in starts at once, unless one is already under way or it works again
            if route.start, !started, let x = a, x.signin?.busy != true, x.state != "connected" { started = true; await act("signin") }
        }
        // Google's sign-in comes back here: the browser opens once with the hub's link; the caught address is sent and the view read again
        .task(id: a?.signin?.url ?? "") { if let u = a?.signin?.url, !u.isEmpty { GoogleLoop.shared.open(u) } }
        .onAppear {
            GoogleLoop.shared.onCatch = { who, address in
                Task {
                    let r: DAccReply? = try? await client.call("account.act", ["profile": who, "provider": "google", "action": "code", "code": address])
                    if let id = r?.account.id, id.hasPrefix("google:"), id != route.id { moved(id) } else { await load() }
                }
            }
        }
        // a sign-in under way: the hub is asked again until it is over (the only polling here, and only then)
        .task(id: a?.signin?.busy == true || a?.state == "connecting") {
            while a?.signin?.busy == true || a?.state == "connecting" { try? await Task.sleep(for: .seconds(1.5)); if Task.isCancelled { return }; await load() }
        }
        .confirmationDialog(Tr.t(a?.remove ?? ""), isPresented: $confirmRemove, titleVisibility: .visible) {
            Button(Tr.t("Remove"), role: .destructive) { Task { await act("remove") } }
        }
    }
    @ViewBuilder private func signin(_ s: DSignIn) -> some View {
        Text(Tr.t(s.text)).dmono(13).foregroundStyle(s.done ? BackplaneDesign.ok : BackplaneDesign.text).fixedSize(horizontal: false, vertical: true).accessibilityIdentifier("account.signin.text")
        if !s.url.isEmpty, let u = URL(string: s.url) {
            HStack(spacing: 6) {
                Button(Tr.t("Open sign-in page")) { openURL(u) }.buttonStyle(DButton(primary: true)).accessibilityIdentifier("account.signin.open")
                Button(Tr.t(copied == "link" ? "Copied" : "Copy link")) { UIPasteboard.general.string = s.url; copied = "link" }.buttonStyle(DButton())
            }
        }
        if !s.code.isEmpty {
            Text(s.code).font(.system(size: 28, weight: .bold, design: .monospaced)).foregroundStyle(BackplaneDesign.text).textSelection(.enabled).accessibilityIdentifier("account.signin.code")
            Button(Tr.t(copied == "code" ? "Copied" : "Copy code")) { UIPasteboard.general.string = s.code; copied = "code" }.buttonStyle(DButton())
        }
        if s.paste {
            TextField(Tr.t("Paste the code here"), text: $code).dmono(13).textInputAutocapitalization(.never).autocorrectionDisabled()
                .padding(.horizontal, 10).frame(height: 40).dbox().accessibilityIdentifier("account.signin.field")
            HStack(spacing: 6) {
                Button(Tr.t("Paste")) { code = UIPasteboard.general.string ?? code }.buttonStyle(DButton())
                Button(Tr.t("Send code")) { Task { await act("code", ["code": code.trimmingCharacters(in: .whitespacesAndNewlines)]) } }
                    .buttonStyle(DButton(primary: true)).disabled(code.trimmingCharacters(in: .whitespaces).isEmpty).accessibilityIdentifier("account.signin.send")
            }
        }
        if !s.error.isEmpty { Text(Tr.t(s.error)).dmono(12).foregroundStyle(BackplaneDesign.bad) }
    }
}
