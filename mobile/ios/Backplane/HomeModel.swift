import Foundation

// The phone's home (docs/ALWAYS-ON.md section 8 on a phone): three tabs over the hub's sections (present/sections.rs through data/1
// `sidebar.sections`). Inbox is what needs the person, what runs and what they put off; Crew is Bub, the bots and the rooms; Bench is the
// projects. The hub decides what is in each section; this file only says which tab draws it and how a row's status reads. No SwiftUI here:
// HomeModelTests compiles it alone. The same rules as the Android app's HomeModel.kt.

enum HomeTab: String, CaseIterable { case inbox, crew, bench }

/// One span a row can be snoozed for, the hub's (`snooze`): `value` goes back as it came (`sidebar.act` `row-snooze`).
struct DSnoozeOpt: Decodable, Equatable, Identifiable {
    var label: String; var value: String
    var id: String { value }
    enum CodingKeys: String, CodingKey { case label, value }
    init(label: String, value: String) { self.label = label; self.value = value }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        label = (try? c.decode(String.self, forKey: .label)) ?? ""; value = (try? c.decode(String.self, forKey: .value)) ?? ""
    }
}

/// A row's status as a word with an icon: `label` is the hub's English word (the catalog translates it), `tone` one of bad, run, wait, ok.
struct HomePill: Equatable { var label: String; var tone: String; var symbol: String }

enum Home {
    /// The sections of the Inbox tab, in the hub's order.
    static let inbox: Set<String> = ["needs", "running", "chats", "snoozed"]
    /// The place of the chats that belong to no project (the hub's `sidebar::CHATS`): `thread.create` with it starts one, in a folder of its own.
    static let chats = "chats"
    /// Whether the Inbox says All clear: nothing waits and nothing runs. The chats and the snoozed rows stay listed under it.
    static func allClear(_ sections: [String]) -> Bool { !sections.contains { $0 != "snoozed" && $0 != chats } }
    static func tab(section id: String, kind: String) -> HomeTab? {
        if kind == "bub" { return .crew }
        if inbox.contains(id) { return .inbox }
        return kind == "projects" ? .bench : (kind == "threads" ? .inbox : nil)
    }
    /// The hub's `badge` (sections.rs) for a hub that sends no `label`: the same table, so an older hub's rows read alike.
    static func badge(word: String, want: String) -> (label: String, tone: String) {
        switch (word, want) {
        case ("needs", "approval"): return ("Approve", "bad")
        case ("needs", "sign-in"): return ("Sign in", "bad")
        case ("needs", "failed"): return ("Failed", "bad")
        case ("needs", "stopped"): return ("Stopped", "bad")
        case ("needs", _): return ("Input", "bad")
        case ("failed", _): return ("Failed", "bad")
        case ("cut", _), ("interrupted", _): return ("Stopped", "bad")
        case ("running", _): return ("Working", "run")
        case ("monitoring", _), ("limit", _): return ("Waiting", "wait")
        case ("done", _): return ("Done", "ok")
        default: return ("", "")
        }
    }
    /// The SF Symbol beside a status word.
    static func symbol(_ label: String) -> String {
        switch label {
        case "Approve": return "exclamationmark"
        case "Input": return "questionmark"
        case "Sign in": return "key.fill"
        case "Failed": return "xmark"
        case "Stopped": return "stop.fill"
        case "Working": return "arrow.triangle.2.circlepath"
        case "Waiting": return "clock"
        case "Done": return "checkmark"
        default: return "circle"
        }
    }
    /// A row's pill: the hub's `label` and `tone`, else what its `word` and `want` say; nil for a row with nothing to say (idle).
    static func pill(label: String, tone: String, word: String, want: String) -> HomePill? {
        let b = label.isEmpty ? badge(word: word, want: want) : (label: label, tone: tone.isEmpty ? "wait" : tone)
        return b.label.isEmpty ? nil : HomePill(label: b.label, tone: b.tone, symbol: symbol(b.label))
    }
    /// The hub's span value of a row's first snooze option with this suffix ("1h", "tomorrow", "1w"); nil when the row cannot be snoozed.
    static func span(_ opts: [DSnoozeOpt], _ suffix: String) -> String? { opts.first { $0.value.hasSuffix("|" + suffix) }?.value }
}
