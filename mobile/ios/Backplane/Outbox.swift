import Foundation

// The phone's own outbox (docs/PHONE-DATA.md, `thread.queue`): a message written while the app's socket is down waits here, on disk, not in the
// conversation. The queue strip draws it as an `Unsent` row with Edit and Remove; once connected the app sends each, oldest first, with its clientId
// (message.send with a clientId posts once, so a send cut off and repeated is one message) and drops it when the hub answers.
// Pure: no UI, no socket (test: Tests/OutboxTests.swift, an XCTest; scripts/mobile/ios-outbox-test.sh). Android: Outbox.kt.

/// A strip row in the hub's shape (`thread.queue` rows): `{msg, tag, text, buttons:[{label, action, value}]}`.
struct DQueueButton: Codable, Equatable, Hashable {
    var label: String
    var action: String
    var value: String
    enum CodingKeys: String, CodingKey { case label, action, value }
    init(label: String, action: String, value: String) { self.label = label; self.action = action; self.value = value }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        label = (try? c.decode(String.self, forKey: .label)) ?? ""; action = (try? c.decode(String.self, forKey: .action)) ?? ""; value = (try? c.decode(String.self, forKey: .value)) ?? ""
    }
}
struct DQueueRow: Codable, Equatable, Hashable, Identifiable {
    var msg: String
    var tag: String
    var text: String
    var buttons: [DQueueButton]
    var id: String { msg }
    enum CodingKeys: String, CodingKey { case msg, tag, text, buttons }
    init(msg: String, tag: String, text: String, buttons: [DQueueButton]) { self.msg = msg; self.tag = tag; self.text = text; self.buttons = buttons }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        msg = (try? c.decode(String.self, forKey: .msg)) ?? ""; tag = (try? c.decode(String.self, forKey: .tag)) ?? ""; text = (try? c.decode(String.self, forKey: .text)) ?? ""
        buttons = (try? c.decode([DQueueButton].self, forKey: .buttons)) ?? []
    }
}

struct Outbox: Codable, Equatable {
    struct Item: Codable, Equatable { var thread: String; var text: String; var clientId: String }
    /// oldest first: the order they were written, and the order they go
    private(set) var items: [Item] = []

    /// The buttons of the app's own rows: handled on the phone, never sent to `queue.act`.
    static let edit = "u-edit", remove = "u-remove"
    static let tag = "Unsent"

    init(items: [Item] = []) { self.items = items }

    /// Keep a message. An empty text, or a clientId already kept, changes nothing.
    mutating func push(thread: String, text: String, clientId: String) {
        let t = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !t.isEmpty, !clientId.isEmpty, !items.contains(where: { $0.clientId == clientId }) else { return }
        items.append(Item(thread: thread, text: t, clientId: clientId))
    }
    /// Take one out (Remove, or Edit, whose text goes back into the draft); nil when it is not here.
    @discardableResult mutating func remove(_ clientId: String) -> Item? {
        guard let i = items.firstIndex(where: { $0.clientId == clientId }) else { return nil }
        return items.remove(at: i)
    }
    /// The hub took it (or refused it): it leaves the outbox.
    mutating func sent(_ clientId: String) { remove(clientId) }
    /// What to send next: the oldest of all threads.
    var next: Item? { items.first }
    var isEmpty: Bool { items.isEmpty }
    func has(_ thread: String) -> Bool { items.contains { $0.thread == thread } }

    /// A thread's rows for the strip, oldest first, in the hub's row shape.
    func rows(_ thread: String) -> [DQueueRow] {
        items.filter { $0.thread == thread }.map {
            DQueueRow(msg: $0.clientId, tag: Self.tag, text: Self.firstLine($0.text),
                      buttons: [DQueueButton(label: "Edit", action: Self.edit, value: $0.clientId), DQueueButton(label: "Remove", action: Self.remove, value: $0.clientId)])
        }
    }
    /// The hub's own words for a row (session/composer.rs `queue_json`): the first line that is not blank, 200 characters at most.
    static func firstLine(_ t: String) -> String {
        String((t.split(separator: "\n", omittingEmptySubsequences: false).map { $0.trimmingCharacters(in: .whitespaces) }.first { !$0.isEmpty } ?? "").prefix(200))
    }
    /// The draft with a message's text put back: the text alone, or after what is there, a blank line between.
    static func draft(_ draft: String, adding text: String) -> String {
        let d = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        return d.isEmpty ? text : draft.replacingOccurrences(of: "\\s+$", with: "", options: .regularExpression) + "\n\n" + text
    }

    // MARK: disk
    func encoded() -> Data { (try? JSONEncoder().encode(self)) ?? Data("{\"items\":[]}".utf8) }
    /// What was saved; an unreadable file is an empty outbox.
    static func decoded(_ d: Data?) -> Outbox { d.flatMap { try? JSONDecoder().decode(Outbox.self, from: $0) } ?? Outbox() }
}
