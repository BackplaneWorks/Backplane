import Foundation
import Observation

// Connection status changes update the chat's controls immediately. Transport
// counters have their own observation scope: off-screen stream traffic must
// not invalidate a chat merely because it reads the connection phase.
struct HubConn: Equatable {
    enum Phase { case connecting, offline, syncing, online }
    var phase: Phase = .connecting
    var stage = ""
    var detail: String?
    var epoch: UInt64 = 0
}

@MainActor @Observable
final class HubTraffic {
    var heard: Date?
    var bytes = 0
}

@MainActor @Observable
final class HubConnections {
    private var states: [String: HubConn] = [:]
    @ObservationIgnored private var counters: [String: HubTraffic] = [:]

    subscript(key: String) -> HubConn? {
        get { states[key] }
        set { if states[key] != newValue { states[key] = newValue } }
    }
    subscript(key: String, default value: @autoclosure () -> HubConn) -> HubConn {
        get { states[key] ?? value() }
        set { if states[key] != newValue { states[key] = newValue } }
    }
    func traffic(_ key: String) -> HubTraffic {
        if let value = counters[key] { return value }
        let value = HubTraffic()
        counters[key] = value
        return value
    }
    func received(_ key: String, bytes: Int, at: Date) {
        let value = traffic(key)
        value.heard = at
        if states[key]?.phase != .online { value.bytes += bytes }
    }
}
