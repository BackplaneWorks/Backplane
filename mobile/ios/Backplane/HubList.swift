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
    /// A hub an offer added goes second: the one in use stays in use; an older pairing with it is replaced.
    static func added(_ links: [String], _ link: String, key: (String) -> String?) -> [String] {
        let k = key(link); let rest = links.filter { key($0) != k }
        return rest.isEmpty ? [link] : [rest[0], link] + rest.dropFirst()
    }
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


    // MARK: the home hub (docs/pairing.md, Home hub): an offer to add it, and failing over to it
    struct Offer: Equatable { let id, name, url, why, credential: String }
    /// The hub's `hub.offer` (a JSON string, "" for none), or nil when it names no hub.
    static func offer(_ text: String) -> Offer? {
        guard let d = text.data(using: .utf8), let o = (try? JSONSerialization.jsonObject(with: d)) as? [String: Any],
              let id = o["id"] as? String, !id.isEmpty, let url = (o["url"] as? String).flatMap(origin) else { return nil }
        return Offer(id: id, name: o["name"] as? String ?? "", url: url, why: o["why"] as? String ?? "", credential: o["credential"] as? String ?? "")
    }
    enum Plan: Equatable { case none, known(String), same(key: String, link: String), take(String) }
    /// What an offer asks of this phone: nothing, a hub already paired (it becomes the home), the same token at the offered address, or a link to take and redeem.
    /// token is the one this phone holds for the hub in use; done the offers already answered.
    static func plan(_ text: String, links: [String], token: String?, done: Set<String>, key: (String) -> String?) -> Plan {
        guard let o = offer(text), !done.contains(o.id), let k = key(o.url) else { return .none }
        if let first = links.first, key(first) == k { return .none }
        if links.contains(where: { key($0) == k }) { return .known(k) }
        if o.credential == "same", let t = token, !t.isEmpty { return .same(key: k, link: o.url + "/#token=" + t) }
        return .take(k)
    }
    /// How long the hub in use may not answer before the home is used instead.
    static let failoverAfter: Double = 20
    /// Whether to be on the home hub: main is the hub in use, home the remembered one ("" for none), down how long main has not answered.
    static func onHome(main: String, home: String, now onHome: Bool, down: Double, homeUp: Bool, mainUp: Bool) -> Bool {
        guard !home.isEmpty, home != main else { return false }
        if onHome { return !mainUp }
        return down >= failoverAfter && homeUp
    }

    /// What the phone says of an invitation that names only its hub's computer.
    static let localText = "This invitation names only its hub's own computer (127.0.0.1), so this phone cannot reach it. Start the hub with Tailscale on, or on an address this phone can reach, then make a new invitation."
}
