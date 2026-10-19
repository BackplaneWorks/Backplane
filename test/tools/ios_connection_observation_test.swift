import Foundation
import Observation

private final class Count: @unchecked Sendable {
    private let lock = NSLock()
    private var n = 0
    func increment() { lock.withLock { n += 1 } }
    var value: Int { lock.withLock { n } }
}

// Uses the actual connection store, without a hub, native renderer or mocks.
@main private struct Test {
    @MainActor static func main() {
        let c = HubConnections()
        let key = "one:3787"
        c[key, default: HubConn()].phase = .syncing
        c[key, default: HubConn()].epoch = 7
        let statusChanges = Count()
        withObservationTracking { _ = c[key]?.phase; _ = c[key]?.stage } onChange: { statusChanges.increment() }
        for n in 0..<1000 { c.received(key, bytes: 64, at: Date(timeIntervalSince1970: Double(n))) }
        precondition(statusChanges.value == 0, "stream traffic invalidated status-dependent UI")
        precondition(c.traffic(key).bytes == 64000)
        precondition(c.traffic(key).heard == Date(timeIntervalSince1970: 999))
        precondition(c[key]?.epoch == 7)
        c[key, default: HubConn()].phase = .online
        precondition(statusChanges.value == 1, "online transition must update controls immediately")
        let heardChanges = Count()
        withObservationTracking { _ = c.traffic(key).heard } onChange: { heardChanges.increment() }
        c.received(key, bytes: 128, at: Date(timeIntervalSince1970: 1000))
        precondition(heardChanges.value == 1, "hub details must retain traffic observation")
        precondition(c.traffic(key).bytes == 64000, "catch-up count only grows while catching up")
        let disconnects = Count()
        withObservationTracking { _ = c[key]?.phase } onChange: { disconnects.increment() }
        c[key, default: HubConn()].phase = .offline
        precondition(disconnects.value == 1, "offline status must update immediately")
        precondition(c.traffic("two:3787").heard == nil)
        print("PASS 1000 off-screen frames do not invalidate connection-status UI; traffic, epoch and online/offline remain exact")
    }
}
