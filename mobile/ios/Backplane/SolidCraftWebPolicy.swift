import Foundation

enum SolidCraftWebPolicy {
    /// Deterministic store ID for the SolidCraft Web persistent WKWebsiteDataStore
    /// (persists OPFS and the 216 MB .wasm HTTP cache without touching `.default()`).
    static let dataStoreID = UUID(uuidString: "7B6E3A54-4C92-4E2A-9D8F-2E1A0F6B3C12")!

    static func normalizedPort(_ url: URL) -> Int? {
        if let port = url.port { return port }
        switch url.scheme?.lowercased() {
        case "https": return 443
        case "http": return 80
        default: return nil
        }
    }

    static func isSameOrigin(_ url: URL, _ base: URL) -> Bool {
        guard let s1 = url.scheme?.lowercased(),
              let s2 = base.scheme?.lowercased(),
              let h1 = url.host?.lowercased(),
              let h2 = base.host?.lowercased(),
              !s1.isEmpty, !h1.isEmpty else { return false }
        return s1 == s2 && h1 == h2 && normalizedPort(url) == normalizedPort(base)
    }

    static func initialRequest(url: URL, token: String) -> URLRequest {
        var req = URLRequest(url: url)
        if !token.isEmpty {
            req.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        }
        return req
    }

    static func cookieProperties(url: URL, token: String, clear: Bool = false) -> [HTTPCookiePropertyKey: Any] {
        var props: [HTTPCookiePropertyKey: Any] = [
            .name: "bp_token",
            .value: clear ? "" : token,
            .domain: url.host ?? "",
            .path: "/",
            HTTPCookiePropertyKey("HttpOnly"): "TRUE",
        ]
        if url.scheme?.lowercased() == "https" {
            props[.secure] = "TRUE"
        }
        if clear {
            props[.expires] = Date(timeIntervalSince1970: 0)
        }
        return props
    }
}
