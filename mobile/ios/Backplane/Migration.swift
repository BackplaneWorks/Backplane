import Foundation

// A 0.14 home coming across (docs/PHONE-DATA.md, docs/IMPORT-0.14.md "Progress"): hello `migrating` and the `migrating` frames carry the card the hub drew,
// words and all. This only reads it and decides what is shown; the app adds no words of its own.
struct DMigButton: Equatable { let label, action, value: String; let on: Bool }
struct DMigCard: Equatable {
    let state: String          // running | done | failed
    let title, body: String
    let pct: Int?              // 0-100; none on a failure
    let lines: [String]
    let buttons: [DMigButton]
    var running: Bool { state == "running" }
    /// the bar's fill, 0...1; none when the hub gave no pct
    var fill: Double? { pct.map { min(max(Double($0) / 100, 0), 1) } }
    /// the banner's lines: the first two
    var short: [String] { Array(lines.prefix(2)) }

    /// hello `migrating` or a `migrating` frame: its card, or none (null, or a state with no card to draw)
    static func of(_ v: Any?) -> DMigCard? {
        guard let o = v as? [String: Any], let c = o["card"] as? [String: Any] else { return nil }
        let title = c["title"] as? String ?? ""
        guard !title.isEmpty else { return nil }
        let pct = (c["pct"] as? NSNumber).map { $0.doubleValue }.map { Int($0.rounded()) }
        let lines = (c["lines"] as? [Any] ?? []).compactMap { $0 as? String }
        let buttons = (c["buttons"] as? [[String: Any]] ?? []).map {
            DMigButton(label: $0["label"] as? String ?? "", action: $0["action"] as? String ?? "", value: $0["value"] as? String ?? "", on: $0["on"] as? Bool ?? false)
        }.filter { !$0.label.isEmpty && !$0.action.isEmpty }
        return DMigCard(state: c["state"] as? String ?? (o["state"] as? String ?? ""), title: title, body: c["body"] as? String ?? "", pct: pct, lines: lines, buttons: buttons)
    }
}

/// What the app holds of it: the newest card until the person answers it. Pure, so the rules have a test (Tests/MigrationTests.swift).
struct DMigration: Equatable {
    private(set) var card: DMigCard?
    /// a frame or a hello: the hub's newest word replaces what was shown (null on hello: nothing is coming across)
    mutating func heard(_ v: Any?) { card = DMigCard.of(v) }
    /// the person can put it away only by one of its buttons, and never while it runs
    var canClose: Bool { card.map { !$0.running } ?? true }
    /// a button the hub answered ok: the card is answered. A run it started (Retry, Import with keys) shows itself again with its next frame
    mutating func answered() { if canClose { card = nil } }
}
