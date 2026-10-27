import Foundation

// The caller supplies a monotonic millisecond clock and starts a new ticket
// for every presentation, source change and explicit retry. A caller may
// continue one attempt across connection epochs without extending its budget.
struct ViewerLoadState {
    struct Identity: Equatable {
        let hub, navigation, kind, key, layers: String
        let pairing, epoch, presentation: UInt64
    }
    struct Ticket: Equatable {
        let identity: Identity
        fileprivate let serial: UInt64
    }
    enum Status: Equatable { case idle, loading, timedOut, ready }

    private var serial: UInt64 = 0
    private(set) var ticket: Ticket?
    private var started: UInt64 = 0
    private var needsGeometry = false
    private var needsMesh = false
    private var hasGeometry = false
    private var hasMesh = false
    private let timeoutMilliseconds: UInt64

    init(timeoutMilliseconds: UInt64 = 30_000) {
        self.timeoutMilliseconds = timeoutMilliseconds
    }

    @discardableResult
    mutating func start(_ identity: Identity, geometry: Bool, mesh: Bool, now: UInt64,
                        preservingDeadline: Bool = false) -> Ticket {
        if ticket == nil || !preservingDeadline { started = now }
        serial &+= 1
        let next = Ticket(identity: identity, serial: serial)
        ticket = next
        needsGeometry = geometry
        needsMesh = mesh
        hasGeometry = false
        hasMesh = false
        return next
    }

    mutating func reset() { ticket = nil }

    // A retired ticket has zero budget. A zero budget must time out without
    // another sleep; elapsed time never underflows if the clock moves back.
    func remainingMilliseconds(_ expected: Ticket, now: UInt64) -> UInt64 {
        guard ticket == expected else { return 0 }
        let elapsed = now >= started ? now - started : 0
        return elapsed >= timeoutMilliseconds ? 0 : timeoutMilliseconds - elapsed
    }

    mutating func geometryReady(_ expected: Ticket) {
        guard ticket == expected else { return }
        hasGeometry = true
    }

    mutating func meshReady(_ expected: Ticket) {
        guard ticket == expected else { return }
        hasMesh = true
    }

    func status(_ expected: Ticket, now: UInt64) -> Status {
        guard ticket == expected else { return .idle }
        if (!needsGeometry || hasGeometry) && (!needsMesh || hasMesh) { return .ready }
        if remainingMilliseconds(expected, now: now) == 0 { return .timedOut }
        return .loading
    }
}
