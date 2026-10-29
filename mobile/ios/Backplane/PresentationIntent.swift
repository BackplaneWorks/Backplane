// One latest read-navigation intent, bound to the presentation that was
// actually shown when tapped. It is never a queue of mutations.
struct PresentationIntent {
    struct Context: Equatable {
        let hub, selection, nav: String
        let epoch, pairing: UInt64
    }
    private struct Pending {
        let action, value: String
        let context: Context
    }
    private var pending: Pending?

    mutating func offer(_ action: String, _ value: String, context: Context) {
        guard action == "view" || action == "bot-tab" else { return }
        pending = Pending(action: action, value: value, context: context)
    }
    mutating func clear() { pending = nil }
    mutating func take(context: Context?, ready: Bool) -> (String, String)? {
        guard let saved = pending else { return nil }
        guard saved.context == context else { pending = nil; return nil }
        guard ready else { return nil }
        pending = nil
        return (saved.action, saved.value)
    }
}
