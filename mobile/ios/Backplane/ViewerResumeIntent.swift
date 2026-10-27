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
    mutating func claim(current: ViewerCloseIntent.Token?, epoch: UInt64, ready: Bool, allowed: Set<String>) -> ViewerCloseIntent.Token? {
        guard let saved = pending else { return nil }
        guard current == saved.token else { pending = nil; return nil }
        guard ready, epoch > saved.fromEpoch else { return nil }
        guard allowed.contains(saved.token.source.kind) else { pending = nil; return nil }
        pending = nil
        return saved.token
    }

    static func matchesSource(_ token: ViewerCloseIntent.Token, accepted: ViewerCloseIntent.Source) -> Bool {
        token.source == accepted
    }

    // A normal view(kind) can reconstruct only a project's default source.
    // File, sheet, history and variant details need a separate restore path.
    static func canRestoreDefault(_ source: ViewerCloseIntent.Source) -> Bool {
        func root(_ key: String, kind: Substring) -> Substring? {
            let fields = key.split(separator: "|", maxSplits: 2, omittingEmptySubsequences: false)
            guard fields.count == 3, fields[0] == kind, !fields[1].isEmpty, fields[2].isEmpty else { return nil }
            return fields[1]
        }
        switch source.kind {
        case "board":
            return root(source.key, kind: "0") != nil && source.key == source.layers
        case "schematic":
            return root(source.key, kind: "1") != nil && source.key == source.layers
        case "3d":
            guard let modelRoot = root(source.key, kind: "2"), let boardRoot = root(source.layers, kind: "0") else { return false }
            return modelRoot == boardRoot
        default:
            return false
        }
    }
}
