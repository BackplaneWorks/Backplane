// A retired navigation can share a bridge drain with another hub. Only
// positively attributed effects for an unaffected paired hub may survive.
enum NavigationCommandPolicy {
    static func owner(type: String, hub: String?, thread: String?) -> String? {
        if type == "send" { return hub.flatMap { $0.isEmpty ? nil : $0 } }
        guard ["keep", "draft-kept", "send-issued", "send-failed"].contains(type),
              let thread, let divider = thread.firstIndex(of: "|"), divider != thread.startIndex else { return nil }
        return String(thread[..<divider])
    }

    static func preserves(type: String, hub: String?, thread: String?, retiredHub: String, paired: Set<String>) -> Bool {
        guard let owner = owner(type: type, hub: hub, thread: thread) else { return false }
        return owner != retiredHub && paired.contains(owner)
    }
}
