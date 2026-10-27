import Foundation

// A reconnect can reopen only the exact still-visible presentation captured
// before the connection was retired. No wire command is produced here.
struct ViewerResumeIntent {
    private struct Pending {
        let token: ViewerCloseIntent.Token
        let fromEpoch: UInt64
    }
    private var pending: Pending?

    mutating func capture(_ token: ViewerCloseIntent.Token, fromEpoch: UInt64) {
        guard !token.source.kind.isEmpty else { pending = nil; return }
        pending = Pending(token: token, fromEpoch: fromEpoch)
    }

    // Lets the caller retain the displayed token/data while an accepted
    // reconnect snapshot temporarily has no viewer.
    func preserves(_ current: ViewerCloseIntent.Token?) -> Bool {
        guard let pending else { return false }
        return current == pending.token
    }

    mutating func cancel() { pending = nil }

    // Pass nil for a locally closed or no-longer-current presentation.
    // A mismatched owner, source or presentation retires the old request.
    mutating func claim(current: ViewerCloseIntent.Token?, epoch: UInt64, ready: Bool, allowed: Set<String>) -> String? {
        guard let saved = pending else { return nil }
        guard current == saved.token else { pending = nil; return nil }
        guard ready, epoch > saved.fromEpoch else { return nil }
        guard allowed.contains(saved.token.source.kind) else { pending = nil; return nil }
        pending = nil
        return saved.token.source.kind
    }
}
