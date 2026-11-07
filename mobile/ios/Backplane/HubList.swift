import Foundation

// The hubs this phone is paired with, and where an invitation is tried. Pure Foundation, so Tests/HubListTests.swift compiles it alone
// (scripts/mobile/ios-hub-list-test.sh). Android's HubList.kt is the same, and test/mobile/hubs_cases.json holds both to the same answers.
// The first link is the hub in use.
enum HubList {
    /// scheme://host[:port] of an http(s) address, or nil: string rules, not a URL parser's, so both phones agree.
    static func origin(_ link: String) -> String? {
        let s = link.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let at = s.range(of: "://"), at.lowerBound != s.startIndex else { return nil }
        let scheme = s[..<at.lowerBound].lowercased()
        let rest = s[at.upperBound...]
        let auth = rest.prefix { $0 != "/" && $0 != "?" && $0 != "#" }
        guard scheme == "http" || scheme == "https", !auth.isEmpty, !auth.contains("@") else { return nil }
        return scheme + "://" + auth
    }
    /// host:port of an http(s) or ws(s) link
    static func authority(_ link: String) -> String? {
        var s = link.trimmingCharacters(in: .whitespacesAndNewlines)
        if s.hasPrefix("ws://") { s = "http://" + s.dropFirst(5) } else if s.hasPrefix("wss://") { s = "https://" + s.dropFirst(6) }
        return origin(s).map { String($0[$0.range(of: "://")!.upperBound...]) }
    }

    /// A newly paired hub goes first; an older pairing with the same hub is replaced, every other hub is kept.
    static func adopt(_ links: [String], _ link: String, key: (String) -> String?) -> [String] { let k = key(link); return [link] + links.filter { key($0) != k } }
    /// Switching to a paired hub moves it first; an unknown key changes nothing.
    static func use(_ links: [String], _ k: String, key: (String) -> String?) -> [String] {
        guard let hit = links.first(where: { key($0) == k }) else { return links }
        return [hit] + links.filter { key($0) != k }
    }
    static func forget(_ links: [String], _ k: String, key: (String) -> String?) -> [String] { links.filter { key($0) != k } }

    /// The addresses to try an invitation at, in order: the link's own, then each other one its `routes` field names.
    static func routes(_ text: String) -> [String] {
        var out: [String] = []
        if let o = origin(text) { out.append(o) }
        let frag = text.firstIndex(of: "#").map { String(text[text.index(after: $0)...]) } ?? ""
        for kv in frag.split(separator: "&", omittingEmptySubsequences: true) where kv.hasPrefix("routes=") {
            let raw = String(kv.dropFirst("routes=".count)).replacingOccurrences(of: "+", with: " ")
            guard let json = raw.removingPercentEncoding, let data = json.data(using: .utf8),
                  let xs = (try? JSONSerialization.jsonObject(with: data)) as? [Any] else { continue }
            for x in xs { if let s = x as? String, let o = origin(s), !out.contains(o) { out.append(o) } }
        }
        return out
    }
    static func loopback(_ origin: String) -> Bool {
        guard let r = origin.range(of: "://") else { return false }
        let h = origin[r.upperBound...]
        return h.hasPrefix("localhost") || h.hasPrefix("127.") || h.hasPrefix("[::1]")
    }
    /// Every address is the hub's own computer: a phone that opens it reaches itself, never the hub.
    static func local(_ routes: [String]) -> Bool { !routes.isEmpty && routes.allSatisfy(loopback) }

    /// What the phone says of an invitation that names only its hub's computer.
    static let localText = "This invitation names only its hub's own computer (127.0.0.1), so this phone cannot reach it. Start the hub with Tailscale on, or on an address this phone can reach, then make a new invitation."
}
