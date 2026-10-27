import Foundation

// Numeric-only opt-in measurements. A stage measures one interval: callers
// take a token before the work and end it after that work finishes. Queue wait
// ends at worker entry; invoke excludes decode; display ends at the UI callback.
// Times use monotonic uptime nanoseconds, sizes are bytes. No token identifies
// a hub, navigation, request, payload, or credential.
final class PerformanceTrace: @unchecked Sendable {
    enum Stage: Int, Sendable {
        case queueWait = 1, context, invoke, decode, cache, discovery, modelReady, display
    }
    enum Operation: Int, Sendable {
        case startup = 1, loadPresentation, connection, receive, flush, navigation, viewerRestore
        case local, reading, pulse, screen, other, foreground
    }
    enum Outcome: Int, Sendable { case success = 0, skipped, failed, stale }
    struct Record: Equatable, Sendable {
        let stage: Stage
        let operation: Operation
        let startedNanoseconds: UInt64
        let endedNanoseconds: UInt64
        let durationNanoseconds: UInt64
        let bytes: UInt64
        let outcome: Outcome
    }
    typealias Sink = @Sendable (Record) -> Void
    typealias Clock = @Sendable () -> UInt64

    private let lock = NSLock()
    private let enabled: Bool
    private let clock: Clock
    private let sink: Sink?
    private var records: [Record] = []
    static let capacity = 256
    static let shared = PerformanceTrace(sink: { r in
        NSLog("BPPerf %d %d %llu %llu %llu %d", r.stage.rawValue, r.operation.rawValue,
              r.startedNanoseconds, r.endedNanoseconds, r.bytes, r.outcome.rawValue)
    })

    // Default configuration cannot enable instrumentation in a release build
    // or on a physical device. Sink is optional and receives only numeric data.
    convenience init(sink: Sink? = nil) {
        #if DEBUG && targetEnvironment(simulator)
        let enabled = ProcessInfo.processInfo.environment["BACKPLANE_PERFORMANCE"] == "1"
        #else
        let enabled = false
        #endif
        self.init(testEnabled: enabled, sink: sink)
    }

    // Explicit configuration is for a standalone Foundation test harness.
    init(testEnabled: Bool, clock: @escaping Clock = { DispatchTime.now().uptimeNanoseconds }, sink: Sink? = nil) {
        enabled = testEnabled
        self.clock = clock
        self.sink = sink
    }

    func begin() -> UInt64? { enabled ? clock() : nil }

    func end(_ stage: Stage, operation: Operation, started: UInt64?, bytes: @autoclosure () -> UInt64 = 0, outcome: Outcome = .success) {
        guard enabled, let started else { return }
        let now = clock()
        let record = Record(stage: stage, operation: operation,
                            startedNanoseconds: started, endedNanoseconds: now,
                            durationNanoseconds: now >= started ? now - started : 0,
                            bytes: bytes(), outcome: outcome)
        lock.lock()
        if records.count == Self.capacity { records.removeFirst() }
        records.append(record)
        lock.unlock()
        sink?(record)
    }

    // A copy in insertion order. Reading the trace never drains it.
    func snapshot() -> [Record] {
        guard enabled else { return [] }
        lock.lock()
        defer { lock.unlock() }
        return records
    }
}
