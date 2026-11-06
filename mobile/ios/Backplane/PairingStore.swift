import Foundation
import Security

// The pairing links (one per hub, each with its hub's credential) in the
// Keychain, readable after the first unlock and never synced or backed up
// to another device. Builds before this kept them in UserDefaults ("links",
// before that one "link"): the first load moves them here and removes them
// there only once the Keychain holds them.
enum PairingStore {
    private static let service = (Bundle.main.bundleIdentifier ?? "dev.backplane.mobile") + ".pairing"
    private static let account = "links"

    private static var query: [String: Any] {
        [kSecClass as String: kSecClassGenericPassword,
         kSecAttrService as String: service,
         kSecAttrAccount as String: account]
    }

    static func load() -> [String] {
        let d = UserDefaults.standard
        // left there by an older build, or by a save the Keychain refused:
        // newer than what the Keychain holds
        if let old = d.stringArray(forKey: "links") ?? d.string(forKey: "link").map({ [$0] }) {
            if save(old) {
                d.removeObject(forKey: "links")
                d.removeObject(forKey: "link")
            }
            return old
        }
        var q = query
        q[kSecReturnData as String] = true
        q[kSecMatchLimit as String] = kSecMatchLimitOne
        var out: CFTypeRef?
        let status = SecItemCopyMatching(q as CFDictionary, &out)
        if status != errSecSuccess, status != errSecItemNotFound { NSLog("pairing: keychain read %d", Int(status)) }
        guard status == errSecSuccess, let data = out as? Data,
              let links = try? JSONDecoder().decode([String].self, from: data) else { return [] }
        return links
    }

    // false when the Keychain refused (the links then stay where they were)
    @discardableResult static func save(_ links: [String]) -> Bool {
        guard let data = try? JSONEncoder().encode(links) else { return false }
        let update: [String: Any] = [kSecValueData as String: data,
                                     kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly]
        var status = SecItemUpdate(query as CFDictionary, update as CFDictionary)
        if status == errSecItemNotFound {
            var add = query
            for (k, v) in update { add[k] = v }
            status = SecItemAdd(add as CFDictionary, nil)
        }
        if status != errSecSuccess { NSLog("pairing: keychain %d", Int(status)) }
        return status == errSecSuccess
    }
}
