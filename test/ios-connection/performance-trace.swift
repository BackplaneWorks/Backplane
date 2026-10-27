import Foundation

private final class Counter: @unchecked Sendable {
    private let lock = NSLock()
    private var n = 0
    func add() { lock.lock(); n += 1; lock.unlock() }
    func value() -> Int { lock.lock(); defer { lock.unlock() }; return n }
}

@main struct PerformanceTraceTests {
    static func main() {
        func check(_ value: @autoclosure () -> Bool, _ name: String) {
            guard value() else { fputs("FAIL \(name)\n", stderr); exit(1) }
            print("PASS \(name)")
        }
        let calls = Counter()
        let disabled = PerformanceTrace(testEnabled: false, clock: { calls.add(); return 100 }, sink: { _ in calls.add() })
        let absent = disabled.begin()
        disabled.end(.invoke, operation: .startup, started: 1, bytes: { calls.add(); return 1 }())
        check(absent == nil && calls.value() == 0 && disabled.snapshot().isEmpty, "disabled avoids clock, sink and records")

        let sink = Counter()
        let trace = PerformanceTrace(testEnabled: true, clock: { 200 }, sink: { _ in sink.add() })
        trace.end(.queueWait, operation: .receive, started: 125, bytes: 123, outcome: .stale)
        let r = trace.snapshot().first
        check(r?.stage == .queueWait && r?.operation == .receive && r?.startedNanoseconds == 125 && r?.endedNanoseconds == 200 && r?.durationNanoseconds == 75 && r?.bytes == 123 && r?.outcome == .stale, "fixed fields and correlated monotonic timestamps")
        trace.end(.decode, operation: .receive, started: nil)
        check(sink.value() == 1, "missing token does not emit")
        trace.end(.context, operation: .startup, started: 201)
        check(trace.snapshot().last?.durationNanoseconds == 0, "backwards test clock saturates")
        for n in 0..<300 { trace.end(.invoke, operation: .startup, started: 200, bytes: UInt64(n)) }
        let bounded = trace.snapshot()
        check(bounded.count == 256 && bounded.first?.bytes == 44 && bounded.last?.bytes == 299, "ring retains newest 256 in order")

        let concurrent = PerformanceTrace(testEnabled: true, clock: { 200 })
        DispatchQueue.concurrentPerform(iterations: 1024) { n in
            concurrent.end(.decode, operation: .receive, started: 100, bytes: UInt64(n))
        }
        let result = concurrent.snapshot()
        check(result.count == 256 && result.allSatisfy { $0.durationNanoseconds == 100 && $0.stage == .decode }, "concurrent recording stays bounded and valid")
        check(PerformanceTrace().begin() == nil, "ordinary nonsimulator build disabled")
    }
}
