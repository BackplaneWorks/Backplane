import Foundation

// Local cover ownership and a deferred close are separate from the server's
// cached viewer value. The adapter must fence the actual async send as well.
struct ViewerCloseIntent {
    struct Owner: Equatable {
        let hub, nav: String
        let pairing: UInt64
    }
    struct Source: Equatable {
        let kind, key, layers: String
    }
    struct Token: Equatable {
        let owner: Owner
        let source: Source
        let presentation: UInt64
    }

    private var serial: UInt64 = 0
    private(set) var current: Token?
    private(set) var pending: Token?
    private(set) var locallyClosed = false

    // Call for a new explicit open, even if it names the same source. A
    // restored presentation also gets a token before its cover is shown.
    mutating func open(owner: Owner, source: Source) -> Token {
        serial += 1
        let token = Token(owner: owner, source: source, presentation: serial)
        current = token
        pending = nil
        locallyClosed = false
        return token
    }

    // The cover setter carries the token of the presentation being dismissed;
    // a delayed setter belonging to an old cover cannot dismiss a new one.
    @discardableResult mutating func close(_ token: Token) -> Bool {
        guard current == token, !locallyClosed else { return false }
        locallyClosed = true
        pending = token
        return true
    }

    // Check on the same serialized executor immediately before enqueue/send.
    // An offline close remains pending until the same owner/source is ready.
    func canSend(_ token: Token, owner: Owner, source: Source, ready: Bool) -> Bool {
        ready && locallyClosed && current == token && pending == token && token.owner == owner && token.source == source
    }

    // Call only after the adapter has safely enqueued the matching close.
    mutating func sent(_ token: Token) {
        if current == token && pending == token { pending = nil }
    }

    // Selection changes, hub changes and pairing replacement retire both the
    // local presentation and any deferred close; no remote write is sent.
    mutating func cancel() {
        current = nil
        pending = nil
        locallyClosed = false
    }
}
