import Foundation

// Main-actor fixed window. The callback reads the owner's current snapshot;
// the window never retains a screen, hub, pairing link or profile.
@MainActor
final class CacheSaveWindow {
    typealias Cancel = @MainActor () -> Void
    // A scheduler must enqueue its callback, never invoke it synchronously.
    typealias Schedule = @MainActor (@escaping @MainActor () -> Void) -> Cancel
    private let schedule: Schedule
    private let flush: @MainActor () -> Void
    private var serial: UInt64 = 0
    private var pending: UInt64?
    private var cancelWake: Cancel?

    init(schedule: @escaping Schedule = { wake in
        let task = Task { @MainActor in
            do { try await Task.sleep(for: .milliseconds(300)) } catch { return }
            guard !Task.isCancelled else { return }
            wake()
        }
        return { task.cancel() }
    }, flush: @escaping @MainActor () -> Void) {
        self.schedule = schedule
        self.flush = flush
    }

    // Further offers share the first deadline. The callback reads the latest
    // accepted snapshot when that deadline fires.
    func offer() {
        guard pending == nil else { return }
        serial &+= 1
        let ticket = serial
        pending = ticket
        cancelWake = schedule { [weak self] in self?.wake(ticket) }
    }

    func cancel() {
        pending = nil
        let cancel = cancelWake
        cancelWake = nil
        cancel?()
    }

    private func wake(_ ticket: UInt64) {
        guard pending == ticket else { return }
        pending = nil
        cancelWake = nil
        flush()
    }
}
