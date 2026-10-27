import Foundation

// Draft authority is scoped to the qualified thread, not the visible screen.
struct ComposerDrafts {
    private struct Draft {
        var text: String
        var edit: UInt64
        var pending: UInt64?
    }

    private var drafts: [String: Draft] = [:]

    func text(for thread: String) -> String? { drafts[thread]?.text }
    func edit(for thread: String) -> UInt64 { drafts[thread]?.edit ?? 0 }
    func pending(for thread: String) -> UInt64? { drafts[thread]?.pending }

    @discardableResult
    mutating func receive(_ thread: String, _ text: String, edit: UInt64?) -> Bool {
        let revision = edit ?? 0
        if let current = drafts[thread] {
            guard current.pending == nil, revision >= current.edit else { return false }
        }
        drafts[thread] = Draft(text: text, edit: revision, pending: nil)
        return true
    }

    @discardableResult
    mutating func change(_ thread: String, _ text: String) -> UInt64 {
        let next = edit(for: thread) + 1
        drafts[thread] = Draft(text: text, edit: next, pending: next)
        return next
    }

    mutating func acknowledged(_ thread: String, edit: UInt64) {
        guard var current = drafts[thread], current.pending == edit else { return }
        current.pending = nil
        drafts[thread] = current
    }

    @discardableResult
    mutating func failed(_ thread: String, _ text: String, sentEdit: UInt64) -> UInt64? {
        let limit = sentEdit == UInt64.max ? UInt64.max : sentEdit + 1
        guard edit(for: thread) <= limit else { return nil }
        return change(thread, text)
    }

    mutating func forget(_ hub: String) {
        let prefix = hub + "|"
        drafts = drafts.filter { !$0.key.hasPrefix(prefix) }
    }
}
