import Foundation

// Shared by the main actor and the engine worker. A flush takes its stamp
// before entering JS; an identity change makes that stamp stale immediately.
final class PairingFlushGate: @unchecked Sendable {
    private let lock = NSLock()
    private var generation: UInt64 = 0
    private var suspended = false

    func invalidate() -> UInt64 {
        lock.withLock {
            generation &+= 1
            suspended = true
            return generation
        }
    }

    func stamp() -> UInt64? {
        lock.withLock { suspended ? nil : generation }
    }

    // An older reconcile cannot release a newer identity change.
    func resume(_ expected: UInt64) -> Bool {
        lock.withLock {
            guard suspended, generation == expected else { return false }
            suspended = false
            return true
        }
    }
}
