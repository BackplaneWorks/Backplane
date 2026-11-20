import Foundation

// The desktop's Reply on the phone (docs/PHONE-DATA.md, feature `reply`): which messages offer it, what the composer's strip shows, and what the
// send carries. The hub builds the `<in_reply_to msg=… chars=…>` block from `replyTo {id, text}` (bots/reply.rs `wrap`), so the agent reads what it
// reads from the desktop. Pure: no UI, no socket (test: Tests/ReplyDraftTests.swift; scripts/mobile/ios-unit-tests.sh). Android: ReplyDraft.kt.

/// A reply being written: the message answered (its id and its text as shown) and who wrote it ("user" or "assistant").
struct DReplyRef: Codable, Equatable {
    var id: String
    var text: String
    var role: String
}

enum ReplyDraft {
    /// The message a Reply on this entry answers; nil where there is none to offer: not a message, nothing written, or not yet on the hub.
    static func of(id: String, kind: String, text: String, live: Bool = false) -> DReplyRef? {
        guard kind == "user" || kind == "assistant", !live, !id.isEmpty, !id.hasPrefix("local:"), !id.hasPrefix("live:"),
              !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return nil }
        return DReplyRef(id: id, text: text, role: kind)
    }
    /// The first line that is not blank, 200 characters at most: the strip's line, and the quote on the bubble (the hub's own, data/mod.rs).
    static func firstLine(_ t: String) -> String {
        String((t.split(separator: "\n", omittingEmptySubsequences: false).map { $0.trimmingCharacters(in: .whitespaces) }.first { !$0.isEmpty } ?? "").prefix(200))
    }
    /// The sender's word on the strip, in English (the view passes it through Tr.t).
    static func who(_ r: DReplyRef) -> String { r.role == "user" ? "You" : "Agent" }
    /// `message.send`'s `replyTo`.
    static func param(_ r: DReplyRef) -> [String: String] { ["id": r.id, "text": r.text] }
    /// The chip a sent reply shows until the hub's entry replaces it.
    static func chip(_ r: DReplyRef) -> (id: String, quote: String) { (r.id, firstLine(r.text)) }
    /// A swipe right far enough, and more right than down, is a Reply.
    static func swipe(dx: Double, dy: Double) -> Bool { dx >= 60 && abs(dy) < dx / 2 }
}
