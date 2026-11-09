import Foundation

// The pure parts of Google's loopback sign-in (GoogleLoop.swift; docs/GOOGLE.md "Sign-in"; tests: mobile/ios/Tests/GoogleLoopTests.swift).
enum GoogleLoopLogic {
    static func redirect(_ port: UInt16) -> String { "http://127.0.0.1:\(port)/oauth/google" }

    /// Whether `link` is the hub's sign-in link and sends Google back to exactly `redirect`.
    static func matches(_ link: String, _ redirect: String) -> Bool {
        guard let u = URL(string: link), let q = URLComponents(url: u, resolvingAgainstBaseURL: false)?.queryItems else { return false }
        return q.first(where: { $0.name == "redirect_uri" })?.value == redirect
    }

    /// The target of a `GET` request line, else nil.
    static func requestPath(_ head: String) -> String? {
        let line = head.split(whereSeparator: { $0 == "\r" || $0 == "\n" }).first ?? ""
        let p = line.split(separator: " ")
        return p.count >= 2 && p[0] == "GET" ? String(p[1]) : nil
    }

    /// The one request that ends a sign-in: /oauth/google with a query.
    static func isCatch(_ path: String?) -> Bool {
        guard let path else { return false }
        return path.hasPrefix("/oauth/google?") && path.count > "/oauth/google?".count
    }

    /// Whether an account's sign-in is caught here: its view says so, or it is Google's.
    static func wants(_ id: String, loopback: Bool) -> Bool { loopback || id.hasPrefix("google:") }
}
