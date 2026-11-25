import Foundation
import Observation
import Security
import UIKit

// Enterprise push: when the person's hub has no APNs key of its own, their Enterprise server holds it. The phone signs in there with an account token (bpe_u_...)
// and registers its APNs token directly; the hub hands alerts to the server (native/host/src/push/enterprise.rs). The token lives in the Keychain, readable after the
// first unlock and never synced or backed up; the server's URL and the account's email are not secret and stay in UserDefaults.
enum EnterpriseStore {
    private static let service = (Bundle.main.bundleIdentifier ?? "dev.backplane.mobile") + ".enterprise"
    private static let account = "token"
    private static let apnsAccount = "last_apns_token"

    private static func query(_ acct: String) -> [String: Any] {
        [kSecClass as String: kSecClassGenericPassword,
         kSecAttrService as String: service,
         kSecAttrAccount as String: acct]
    }

    static func load(_ acct: String = account) -> String? {
        var q = query(acct)
        q[kSecReturnData as String] = true
        q[kSecMatchLimit as String] = kSecMatchLimitOne
        var out: CFTypeRef?
        let status = SecItemCopyMatching(q as CFDictionary, &out)
        if status != errSecSuccess, status != errSecItemNotFound { NSLog("enterprise: keychain read %d", Int(status)) }
        guard status == errSecSuccess, let data = out as? Data, let s = String(data: data, encoding: .utf8), !s.isEmpty else { return nil }
        return s
    }

    @discardableResult static func save(_ value: String, account acct: String = account) -> Bool {
        let q = query(acct)
        let update: [String: Any] = [kSecValueData as String: Data(value.utf8),
                                     kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly]
        var status = SecItemUpdate(q as CFDictionary, update as CFDictionary)
        if status == errSecItemNotFound {
            var add = q
            for (k, v) in update { add[k] = v }
            status = SecItemAdd(add as CFDictionary, nil)
        }
        if status != errSecSuccess { NSLog("enterprise: keychain %d", Int(status)) }
        return status == errSecSuccess
    }

    static func delete() {
        SecItemDelete(query(account) as CFDictionary)
        SecItemDelete(query(apnsAccount) as CFDictionary)
    }

    static func loadLastApnsToken() -> String? { load(apnsAccount) }
    @discardableResult static func saveLastApnsToken(_ token: String) -> Bool { save(token, account: apnsAccount) }
}

@MainActor
@Observable
final class EnterprisePush {
    static let shared = EnterprisePush()

    private static let urlKey = "app.enterprise.url"
    private static let emailKey = "app.enterprise.email"

    private(set) var baseURL: String
    private(set) var accountEmail: String
    private(set) var signedIn: Bool
    var busy = false
    var lastError = ""
    var lastStatus = ""

    private init() {
        let d = UserDefaults.standard
        baseURL = d.string(forKey: Self.urlKey) ?? ""
        accountEmail = d.string(forKey: Self.emailKey) ?? ""
        signedIn = !baseURL.isEmpty && EnterpriseStore.load() != nil
    }

    /// https:// (or http:// for loopback tests) with no trailing slash, else nil.
    static func normalize(_ s: String) -> String? {
        var u = s.trimmingCharacters(in: .whitespacesAndNewlines)
        while u.hasSuffix("/") { u.removeLast() }
        guard let parsed = URL(string: u), let host = parsed.host?.lowercased(), !host.isEmpty else { return nil }
        if u.hasPrefix("https://") { return u }
        if u.hasPrefix("http://"), host == "127.0.0.1" || host == "localhost" || host == "::1" { return u }
        return nil
    }

    private func request(_ base: String, _ path: String, token: String, method: String = "GET", body: [String: Any]? = nil) -> URLRequest? {
        guard let url = URL(string: base + path) else { return nil }
        var r = URLRequest(url: url, timeoutInterval: 15)
        r.httpMethod = method
        r.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        if let body {
            r.setValue("application/json", forHTTPHeaderField: "Content-Type")
            r.httpBody = try? JSONSerialization.data(withJSONObject: body)
        }
        return r
    }

    /// Checks the token against the server and keeps it. nil on success, else what to tell the person.
    func signIn(urlString: String, token: String) async -> String? {
        guard let base = Self.normalize(urlString) else { return Tr.t("The server address must start with https://") }
        let t = token.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !t.isEmpty else { return Tr.t("Enter your account token") }
        guard let req = request(base, "/v1/me", token: t) else { return Tr.t("The server address is not valid") }
        busy = true; defer { busy = false }
        do {
            let (data, resp) = try await URLSession.shared.data(for: req)
            let code = (resp as? HTTPURLResponse)?.statusCode ?? 0
            switch code {
            case 200:
                let o = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
                let email = o?["email"] as? String ?? ""
                guard EnterpriseStore.save(t) else { return Tr.t("The token could not be saved on this phone") }
                UserDefaults.standard.set(base, forKey: Self.urlKey)
                UserDefaults.standard.set(email, forKey: Self.emailKey)
                baseURL = base; accountEmail = email; signedIn = true; lastError = ""; lastStatus = ""
                return nil
            case 401, 403: return Tr.t("That token was not accepted")
            case 429: return Tr.t("Too many tries, wait a moment")
            default: return Tr.t("The server answered") + " (\(code))"
            }
        } catch { return error.localizedDescription }
    }

    /// Takes this phone off the server (best effort), then forgets the account.
    func signOut(apnsToken: String?) async {
        let effectiveToken = apnsToken ?? EnterpriseStore.loadLastApnsToken()
        if let effectiveToken, !baseURL.isEmpty, let t = EnterpriseStore.load(),
           let req = request(baseURL, "/v1/push/devices/unregister", token: t, method: "POST", body: ["token": effectiveToken]) {
            _ = try? await URLSession.shared.data(for: req)
        }
        EnterpriseStore.delete()
        UserDefaults.standard.removeObject(forKey: Self.urlKey)
        UserDefaults.standard.removeObject(forKey: Self.emailKey)
        baseURL = ""; accountEmail = ""; signedIn = false; lastError = ""; lastStatus = ""
    }

    /// Tells the server this phone's APNs token (and whether alerts are muted). Nothing when not signed in.
    func registerDevice(token: String, env: String, bundle: String, muted: Bool) async {
        guard !baseURL.isEmpty, let t = EnterpriseStore.load() else { return }
        if !signedIn { signedIn = true }
        let body: [String: Any] = ["token": token, "platform": "ios", "kind": "alert", "env": env, "bundle": bundle, "deviceName": UIDevice.current.name, "muted": muted]
        guard let req = request(baseURL, "/v1/push/devices", token: t, method: "POST", body: body) else { return }
        do {
            let (_, resp) = try await URLSession.shared.data(for: req)
            let code = (resp as? HTTPURLResponse)?.statusCode ?? 0
            if (200..<300).contains(code) {
                EnterpriseStore.saveLastApnsToken(token)
                lastError = ""; lastStatus = muted ? Tr.t("Registered, alerts off") : Tr.t("Registered")
            }
            else if code == 401 || code == 403 { lastStatus = ""; lastError = Tr.t("Sign-in expired or revoked") }
            else { lastStatus = ""; lastError = Tr.t("The server answered") + " (\(code))" }
        } catch { lastStatus = ""; lastError = error.localizedDescription }
    }
}
