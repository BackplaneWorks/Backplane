import Foundation

// The hub's restarts on the connection strip (docs/PHONE-DATA.md, "Restarts (feature `restarting`)"): pure, the strip only draws `line`.
//   hub.restarting: its text, in place of the usual offline / reconnecting look while there is no hub, for 5 minutes at most; a new hello drops it.
//   hello.hubBack: its text once, for 6 seconds, as the app joins.
enum HubRestart {
    /// a note the hub gave and when this phone heard it (this phone's clock, not the hub's `at`)
    struct Note: Equatable { var text: String; var heard: Date }
    /// what the connection strip shows
    enum Line: Equatable {
        case none           // connected, nothing to say
        case usual          // the usual offline / reconnecting look
        case note(String)   // the hub's words (English; the strip translates them) in its place
    }
    static let restartingFor: TimeInterval = 5 * 60
    static let backFor: TimeInterval = 6

    /// `restarting`: the last hub.restarting heard since the last hello; `back`: the hubBack shown as this connection joined.
    static func line(restarting: Note?, back: Note?, online: Bool, now: Date) -> Line {
        if online {
            if let b = back, now < b.heard.addingTimeInterval(backFor) { return .note(b.text) }
            return .none
        }
        if let r = restarting, now < r.heard.addingTimeInterval(restartingFor) { return .note(r.text) }
        return .usual
    }

    /// When `line` changes next with nothing else happening: the one timer's deadline (nil: no timer).
    static func next(restarting: Note?, back: Note?, online: Bool, now: Date) -> Date? {
        let end = online ? back.map { $0.heard.addingTimeInterval(backFor) } : restarting.map { $0.heard.addingTimeInterval(restartingFor) }
        return end.flatMap { $0 > now ? $0 : nil }
    }

    /// The text of a hub.restarting frame (nil when it has none).
    static func restarting(_ o: [String: Any]) -> String? {
        guard let t = o["text"] as? String, !t.isEmpty else { return nil }
        return t
    }

    /// hello.hubBack: "" or a JSON string {reason, text, at}. Its text and `at` (what makes it shown once), nil when there is nothing to show.
    static func back(_ v: Any?) -> (text: String, at: String)? {
        guard let s = v as? String, !s.isEmpty, let d = s.data(using: .utf8),
              let o = (try? JSONSerialization.jsonObject(with: d)) as? [String: Any],
              let t = o["text"] as? String, !t.isEmpty else { return nil }
        let at = (o["at"] as? NSNumber)?.stringValue ?? (o["at"] as? String) ?? t
        return (t, at)
    }
}
