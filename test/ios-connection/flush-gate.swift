import Foundation

final class GenerationRecorder: @unchecked Sendable {
    private let lock = NSLock()
    private var values: Set<UInt64> = []
    func add(_ value: UInt64) { lock.withLock { _ = values.insert(value) } }
    func snapshot() -> Set<UInt64> { lock.withLock { values } }
}

@main struct FlushGateTests {
    static func main() {
        func check(_ condition: @autoclosure () -> Bool, _ message: String) {
            guard condition() else { fputs("FAIL: \(message)\n", stderr); exit(1) }
            print("PASS \(message)")
        }
        let gate = PairingFlushGate()
        let old = gate.stamp()
        check(old == 0, "initial flush is stamped")
        let first = gate.invalidate()
        check(first == 1 && gate.stamp() == nil, "pairing change suspends flush immediately")
        let second = gate.invalidate()
        check(!gate.resume(first) && gate.stamp() == nil, "older reconciliation cannot resume a newer change")
        check(gate.resume(second) && gate.stamp() == second, "current reconciliation resumes with a new stamp")
        check(old != gate.stamp(), "in-flight old output carries a stale stamp")
        check(!gate.resume(second), "duplicate resume does not schedule a second release")

        let concurrent = PairingFlushGate()
        let recorder = GenerationRecorder()
        DispatchQueue.concurrentPerform(iterations: 1000) { _ in
            recorder.add(concurrent.invalidate())
        }
        let generations = recorder.snapshot()
        check(generations.count == 1000 && generations.max() == 1000, "concurrent invalidations have distinct ordered generations")
        check(!concurrent.resume(999) && concurrent.resume(1000), "only latest concurrent generation can resume")
    }
}
