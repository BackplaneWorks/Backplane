import Foundation

// Files of the hub's machine on the phone (docs/file-links.md; the Android app's FileFetch.kt is the same rules): which tapped link is a file the hub
// serves, the address it is fetched from, and how a download that stopped goes on. Pure, Foundation only, so mobile/ios/Tests/FileFetchTests.swift runs it.

/// A file an assistant entry names that the hub serves (data/1 feature `fileLinks`): `open` is "view" (Backplane's viewers draw it) or "download".
struct DFileLink: Codable, Equatable {
    var path: String; var name: String; var size: Int64; var open: String; var project: String; var absPath: String
    enum CodingKeys: String, CodingKey { case path, name, size, open, project, absPath }
    init(path: String, name: String, size: Int64, open: String, project: String = "", absPath: String = "") {
        self.path = path; self.name = name; self.size = size; self.open = open; self.project = project; self.absPath = absPath
    }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        path = (try? c.decode(String.self, forKey: .path)) ?? ""; name = (try? c.decode(String.self, forKey: .name)) ?? ""
        size = (try? c.decode(Int64.self, forKey: .size)) ?? 0; open = (try? c.decode(String.self, forKey: .open)) ?? ""
        project = (try? c.decode(String.self, forKey: .project)) ?? ""; absPath = (try? c.decode(String.self, forKey: .absPath)) ?? ""
        if open.isEmpty { open = "download" }
    }
}

enum FileFetch {
    // what the hub's share_route writes: the token, then the name with everything but [A-Za-z0-9._~-] percent-encoded (never `\`, `?`, `#`, `..`)
    private static let share = try! NSRegularExpression(pattern: "^/s/[0-9A-Fa-f]{48}/(?:[A-Za-z0-9._~-]|%[0-9A-Fa-f]{2})+$")
    private static let lineTail = try! NSRegularExpression(pattern: "(?::\\d{1,6}){1,2}$|#L\\d{1,6}(?:-L?\\d{1,6})?$")
    private static func whole(_ s: String) -> NSRange { NSRange(location: 0, length: (s as NSString).length) }

    /// A share's address (`/s/<token>/<name>`, written in a message): fetched as it is, the token its own credential.
    static func shareRoute(_ p: String) -> String? {
        var t = p.trimmingCharacters(in: .whitespaces)
        if let h = t.firstIndex(of: "#") { t = String(t[..<h]) }
        if let q = t.firstIndex(of: "?") { t = String(t[..<q]) }
        guard share.firstMatch(in: t, range: whole(t)) != nil, ![".", ".."].contains(t.split(separator: "/").last.map(String.init) ?? "") else { return nil }
        return t
    }
    /// A share's route carries its own credential: the device's token is not sent with it.
    static func isShare(_ route: String) -> Bool { route.hasPrefix("/s/") }
    /// The address of a route on the hub at `base`: a share's route as written (it is already percent-encoded), any other path encoded here.
    static func url(base: URL, route: String) -> URL? {
        guard var c = URLComponents(url: base, resolvingAgainstBaseURL: false) else { return nil }
        if isShare(route) { guard shareRoute(route) != nil else { return nil }; c.percentEncodedPath = route } else { c.path = route }
        return c.url
    }
    /// A path as a link carries it, without `file://`, fragment, query, or the line it points at (`a.rs:12`, `a.rs:3:4`, `a.rs#L4-L9`).
    static func bare(_ p: String) -> String {
        var s = p.trimmingCharacters(in: .whitespaces)
        if s.count >= 7, s.prefix(7).lowercased() == "file://" {
            var rest = String(s.dropFirst(7))
            if rest.hasPrefix("localhost/") { rest = String(rest.dropFirst("localhost".count)) }
            if let hash = rest.firstIndex(of: "#") { rest = String(rest[..<hash]) }
            if let q = rest.firstIndex(of: "?") { rest = String(rest[..<q]) }
            s = rest.removingPercentEncoding ?? rest
            if s.range(of: "^/[A-Za-z]:/", options: .regularExpression) != nil { s = String(s.dropFirst()) }
        }
        return lineTail.stringByReplacingMatches(in: s, range: whole(s), withTemplate: "")
    }
    /// The file of `files` a tapped path names, in the message's spelling (or resolved `absPath`).
    static func find(_ p: String, in files: [DFileLink]) -> DFileLink? {
        let b = bare(p)
        return files.first { $0.path == p || $0.path == b || bare($0.path) == b || (!$0.absPath.isEmpty && bare($0.absPath) == b) }
    }
    /// A thread of another machine (`<link>~<id>`) is named on the request; this hub's own thread is not.
    static func far(_ thread: String) -> Bool { thread.contains("~") }
    /// `/file`'s query.
    static func fileQuery(path: String, thread: String) -> [URLQueryItem] {
        [URLQueryItem(name: "path", value: path)] + (far(thread) ? [URLQueryItem(name: "thread", value: thread)] : [])
    }
    /// The name a share saves as: its last part, percent-decoding undone.
    static func shareName(_ route: String) -> String { let l = route.split(separator: "/").last.map(String.init) ?? route; return l.removingPercentEncoding ?? l }
    /// A name safe to be a file's: no folder in it.
    static func safeName(_ n: String) -> String {
        let t = n.replacingOccurrences(of: "/", with: "_").replacingOccurrences(of: "\\", with: "_").trimmingCharacters(in: .whitespaces)
        return t.isEmpty || t == "." || t == ".." ? "download" : t
    }

    /// The headers that ask for the rest of a file: none from nothing (or without the tag the first answer had: the start again), else the range and its tag.
    static func ask(have: Int64, etag: String) -> [String: String] { have <= 0 || etag.isEmpty ? [:] : ["Range": "bytes=\(have)-", "If-Range": etag] }

    enum Step: Equatable {
        /// Write from `from` (0: truncate the `.part` and start over; else append); `total` the whole file's length, -1 unknown.
        case write(from: Int64, total: Int64)
        /// Nothing more to fetch: the `.part` is the file.
        case done
        /// The `.part` is no good (a range that does not join it): drop it and ask for the whole file.
        case again
        /// The hub refused: show its own words when it sent some.
        case fail(String)
    }
    private static let contentRange = try! NSRegularExpression(pattern: "^bytes (\\d+)-(\\d+)/(\\d+|\\*)$")
    private static let unsatisfied = try! NSRegularExpression(pattern: "^bytes \\*/(\\d+)$")
    private static func group(_ r: NSRegularExpression, _ s: String, _ i: Int) -> String? {
        guard let m = r.firstMatch(in: s, range: whole(s)), let g = Range(m.range(at: i), in: s) else { return nil }
        return String(s[g])
    }
    /// The answer's status and headers made a Step. `have`: the bytes the `.part` holds and was asked to go on from.
    static func step(code: Int, have: Int64, contentRange cr: String?, contentLength: Int64, body: String = "") -> Step {
        let r = (cr ?? "").trimmingCharacters(in: .whitespaces)
        switch code {
        case 206:
            guard let a = group(contentRange, r, 1).flatMap({ Int64($0) }), a == have else { return .again }
            return .write(from: have, total: group(contentRange, r, 3).flatMap { Int64($0) } ?? -1)
        case 200: return .write(from: 0, total: contentLength)
        case 416:
            if let t = group(unsatisfied, r, 1).flatMap({ Int64($0) }), t == have { return .done }
            return .again
        default:
            let t = String(body.trimmingCharacters(in: .whitespacesAndNewlines).prefix(200))
            return .fail(t.isEmpty ? "HTTP \(code)" : t)
        }
    }
    /// Whether a `.part` of `have` bytes is the whole file.
    static func complete(have: Int64, total: Int64) -> Bool { total >= 0 && have == total }
    /// How long to wait before try `n` (1, 2, …) after a dropped connection, in seconds.
    static func backoff(_ n: Int) -> Double { min(30, Double(1 << min(max(n - 1, 0), 5))) }
    static let tries = 6
}
