import Foundation

// Read-navigation intent survives a socket reconnect until the matching
// presentation is accepted. This value does not dispatch or serialize work.
struct NavigationResumeIntent {
    enum Action: String {
        case select, bot, room, remote, uber
        case uberBot = "uber-bot"
    }

    struct Token: Equatable {
        let nonce: UInt64
        let hub: String
        let pairing: UInt64
        let tab: String
        let action: Action
        let value: String
        let nav: String
    }

    private var serial: UInt64 = 0
    private var intent: Token?
    private var issuedEpoch: UInt64?
    private var issued = false
    private var claimSerial: UInt64 = 0
    private(set) var claimID: UInt64?
    private var primeEpoch: UInt64?
    private var primeInFlight = false

    // The caller still has to check current ownership and claim the epoch.
    var latest: Token? { intent }

    // An explicit newer navigation always supersedes the old one, including
    // selecting the empty root. The caller supplies its already-resolved hub.
    @discardableResult
    mutating func capture(hub: String, pairing: UInt64, tab: String,
                          action: Action, value: String, nav: String) -> Token? {
        serial &+= 1
        intent = nil
        issuedEpoch = nil
        issued = false
        claimID = nil
        primeEpoch = nil
        primeInFlight = false
        guard !hub.isEmpty, !tab.isEmpty else { return nil }
        let next = Token(nonce: serial, hub: hub, pairing: pairing, tab: tab,
                         action: action, value: value, nav: nav)
        intent = next
        return next
    }

    // Recheck after each await and immediately before dispatch/applying an
    // effect; token possession alone never grants current owner authority.
    func current(_ token: Token, hub: String, pairing: UInt64, tab: String) -> Bool {
        intent == token && token.hub == hub && token.pairing == pairing && token.tab == tab
    }

    // Claim before the original dispatch and before reconnect replay. A claim
    // cannot be repeated within an epoch, even if dispatch later fails.
    mutating func claim(_ token: Token, hub: String, pairing: UInt64,
                        tab: String, epoch: UInt64) -> Token? {
        guard current(token, hub: hub, pairing: pairing, tab: tab),
              issuedEpoch.map({ epoch > $0 || (epoch == $0 && !issued) }) ?? true,
              !(primeInFlight && primeEpoch == epoch) else { return nil }
        if let prior = primeEpoch, epoch > prior { primeInFlight = false }
        issuedEpoch = epoch
        issued = true
        claimSerial &+= 1
        claimID = claimSerial
        return token
    }

    // Call only on an explicit local rejection before issue or retention.
    // The epoch watermark stays in place, excluding older completions.
    @discardableResult
    mutating func rejected(_ token: Token, hub: String, pairing: UInt64,
                           tab: String, epoch: UInt64, claim: UInt64) -> Bool {
        guard current(token, hub: hub, pairing: pairing, tab: tab),
              issued, issuedEpoch == epoch, claimID == claim else { return false }
        issued = false
        claimID = nil
        return true
    }

    // After explicit rejection, hold same-epoch claims until the caller has
    // applied the owned select output. A newer epoch may supersede this stage.
    mutating func beginPrime(_ token: Token, hub: String, pairing: UInt64,
                             tab: String, epoch: UInt64) -> Bool {
        guard current(token, hub: hub, pairing: pairing, tab: tab),
              issuedEpoch == epoch, !issued,
              primeEpoch.map({ epoch > $0 }) ?? true else { return false }
        primeEpoch = epoch
        primeInFlight = true
        return true
    }

    @discardableResult
    mutating func finishPrime(_ token: Token, hub: String, pairing: UInt64,
                              tab: String, epoch: UInt64) -> Bool {
        guard current(token, hub: hub, pairing: pairing, tab: tab),
              issuedEpoch == epoch, primeEpoch == epoch, primeInFlight else { return false }
        primeInFlight = false
        return true
    }

    // Only the issued epoch's fresh, adopted exact owner and nav retire the
    // request. Stale replies and other hubs never acknowledge this intent.
    @discardableResult
    mutating func accept(_ token: Token, hub: String, pairing: UInt64, tab: String,
                         epoch: UInt64, nav: String, fresh: Bool) -> Bool {
        guard fresh, current(token, hub: hub, pairing: pairing, tab: tab),
              issued, issuedEpoch == epoch, token.nav == nav else { return false }
        intent = nil
        issuedEpoch = nil
        issued = false
        claimID = nil
        primeEpoch = nil
        primeInFlight = false
        return true
    }

    mutating func cancel() {
        intent = nil
        issuedEpoch = nil
        issued = false
        claimID = nil
        primeEpoch = nil
        primeInFlight = false
    }
}
