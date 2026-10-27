import Foundation
import Dispatch

@main struct ViewerCloseIntentTests {
    static func main() {
        func check(_ condition: @autoclosure () -> Bool, _ name: String) {
            guard condition() else { fputs("FAIL \(name)\n", stderr); exit(1) }
            print("PASS \(name)")
        }
        let owner = ViewerCloseIntent.Owner(hub: "hub", nav: "hub|thread", pairing: 1)
        let source = ViewerCloseIntent.Source(kind: "board", key: "board-key", layers: "layers")
        var state = ViewerCloseIntent()
        let first = state.open(owner: owner, source: source)
        check(state.close(first) && state.locallyClosed, "close immediately hides local cover")
        check(!state.canSend(first, owner: owner, source: source, ready: false), "offline close stays deferred")
        check(state.canSend(first, owner: owner, source: source, ready: true), "same presentation can close when ready")
        check(!state.canSend(first, owner: .init(hub: "other", nav: owner.nav, pairing: 1), source: source, ready: true), "another hub cannot receive close")
        check(!state.canSend(first, owner: .init(hub: owner.hub, nav: "other", pairing: 1), source: source, ready: true), "another selection cannot receive close")
        check(!state.canSend(first, owner: .init(hub: owner.hub, nav: owner.nav, pairing: 2), source: source, ready: true), "replacement pairing cannot receive close")
        check(!state.canSend(first, owner: owner, source: .init(kind: "3d", key: source.key, layers: source.layers), ready: true), "changed source cannot receive close")
        state.sent(first)
        check(!state.canSend(first, owner: owner, source: source, ready: true) && state.locallyClosed, "sent once and stays locally hidden")
        let second = state.open(owner: owner, source: source)
        check(first != second && !state.locallyClosed, "same-source reopen has a new presentation")
        check(!state.close(first) && !state.locallyClosed, "late old dismissal cannot close reopened viewer")
        check(state.close(second), "current presentation closes")
        state.cancel()
        check(!state.canSend(second, owner: owner, source: source, ready: true) && state.current == nil, "navigation cancels pending close")
        let reserved = state.reserve(owner: owner, source: source)
        check(state.current == nil, "reserved open never publishes stale data")
        state.accept(reserved)
        check(state.current == reserved, "accepted correlated open publishes exact token")
        let gate = ViewerDispatchGate()
        gate.install(first)
        let queuedOldClose = { gate.allows(first, value: "") }
        gate.install(first, closing: true)
        check(!gate.allows(first, value: "board"), "close cancels an opening still queued for same token")
        gate.install(second)
        check(!queuedOldClose(), "worker rejects queued old close after newer reopen")
        check(gate.allows(second, value: "board"), "worker permits current explicit open")
        gate.install(second, closing: true)
        check(gate.allows(second, value: ""), "worker permits matching current close")
        gate.install(nil)
        check(!gate.allows(second, value: ""), "navigation invalidates queued worker close")
        final class Events: @unchecked Sendable {
            private let lock = NSLock()
            private var values: [String] = []
            func append(_ text: String) { lock.withLock { values.append(text) } }
            var snapshot: [String] { lock.withLock { values } }
        }
        let entered = DispatchSemaphore(value: 0), release = DispatchSemaphore(value: 0)
        let installing = DispatchSemaphore(value: 0), installed = DispatchSemaphore(value: 0), finished = DispatchSemaphore(value: 0)
        let events = Events()
        gate.install(first, closing: true)
        DispatchQueue.global().async {
            _ = gate.perform(first, value: "") {
                entered.signal()
                _ = release.wait(timeout: .now() + 2)
                events.append("old-dispatched")
            }
            finished.signal()
        }
        check(entered.wait(timeout: .now() + 2) == .success, "old dispatch reaches atomic boundary")
        DispatchQueue.global().async {
            installing.signal()
            gate.install(second)
            events.append("new-installed")
            installed.signal()
        }
        check(installing.wait(timeout: .now() + 2) == .success, "new intent races actual invocation")
        release.signal()
        check(finished.wait(timeout: .now() + 2) == .success && installed.wait(timeout: .now() + 2) == .success, "atomic dispatch race finishes")
        check(events.snapshot == ["old-dispatched", "new-installed"], "new install cannot cross authorized dispatch")
        check(gate.perform(first, value: "", { true }) == nil, "old dispatch is rejected after new install")
        let suite = "bp-viewer-close-test-" + UUID().uuidString
        let defaults = UserDefaults(suiteName: suite)!
        defer { defaults.removePersistentDomain(forName: suite) }
        ViewerClosedStamp(identity: "pair-a", hub: owner.hub, nav: owner.nav, source: source).save(in: defaults)
        let restored = ViewerClosedStamp.load(from: defaults)!
        check(restored.matches(owner, identity: "pair-a"), "local close survives process state reload")
        check(!restored.matches(owner, identity: "pair-b"), "replacement pairing cannot reuse persisted close")
        check(!restored.matches(.init(hub: owner.hub, nav: "other", pairing: 1), identity: "pair-a"), "persisted close cannot affect another thread")
        ViewerClosedStamp.clear(in: defaults)
        check(ViewerClosedStamp.load(from: defaults) == nil, "explicit open retires persisted close")
        let third = state.open(owner: owner, source: source)
        _ = state.close(third)
        _ = state.open(owner: owner, source: .init(kind: "3d", key: "new", layers: ""))
        check(!state.canSend(third, owner: owner, source: source, ready: true), "new source cancels old pending close")
    }
}
