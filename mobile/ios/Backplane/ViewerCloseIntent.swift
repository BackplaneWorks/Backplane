import Foundation

// Local cover ownership and a deferred close are separate from the server's
// cached viewer value. The adapter must fence the actual async send as well.
struct ViewerCloseIntent {
    struct Owner: Equatable {
        let hub, nav: String
        let pairing: UInt64
    }
    struct Source: Equatable, Codable {
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
        let token = reserve(owner: owner, source: source)
        accept(token)
        return token
    }

    // Reserve an explicit request without showing old data as that request.
    mutating func reserve(owner: Owner, source: Source) -> Token {
        serial += 1
        return Token(owner: owner, source: source, presentation: serial)
    }
    mutating func accept(_ token: Token) {
        current = token
        pending = nil
        locallyClosed = false
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

// The Engine checks this on its worker immediately before invoking Bend.
// A newer open also invalidates an older close that was already queued.
final class ViewerDispatchGate: @unchecked Sendable {
    private let lock = NSLock()
    private var token: ViewerCloseIntent.Token?
    private var closing = false
    func install(_ token: ViewerCloseIntent.Token?, closing: Bool = false) {
        lock.withLock { self.token = token; self.closing = closing }
    }
    func allows(_ token: ViewerCloseIntent.Token, value: String) -> Bool {
        lock.withLock { self.token == token && self.closing == value.isEmpty }
    }
    func perform<T>(_ token: ViewerCloseIntent.Token, value: String, _ operation: () -> T) -> T? {
        lock.withLock {
            guard self.token == token && self.closing == value.isEmpty else { return nil }
            return operation()
        }
    }
}

// A close remains local across process death. Pairing identity is a digest,
// never a credential; a replacement account cannot reuse this preference.
struct ViewerClosedStamp: Codable {
    let identity, hub, nav: String
    let source: ViewerCloseIntent.Source
    private static let key = "viewer.closed.v1"
    func matches(_ owner: ViewerCloseIntent.Owner, identity: String?) -> Bool {
        identity == self.identity && owner.hub == hub && owner.nav == nav
    }
    func save(in defaults: UserDefaults = .standard) {
        if let data = try? JSONEncoder().encode(self) { defaults.set(data, forKey: Self.key) }
    }
    static func load(from defaults: UserDefaults = .standard) -> Self? {
        defaults.data(forKey: key).flatMap { try? JSONDecoder().decode(Self.self, from: $0) }
    }
    static func clear(in defaults: UserDefaults = .standard) { defaults.removeObject(forKey: key) }
}
