import Foundation

// Raw room/remote routes do not identify an owning hub. On an explicit
// account-set change, retire all of them rather than guess their owner.
struct PresentationRetirement {
    private let hub: String?

    // nil retires only the raw routes, e.g. when adding a new hub.
    init(hub: String?) {
        self.hub = hub.flatMap { $0.isEmpty ? nil : $0 }
    }

    func retires(_ route: String) -> Bool {
        if route.hasPrefix("@room:") || route.hasPrefix("@remote:") { return true }
        guard let hub else { return false }
        return route.hasPrefix(hub + "|") || route == "@hub:" + hub
    }

    func keeping<Value>(_ values: [String: Value]) -> [String: Value] {
        values.filter { !retires($0.key) }
    }

    func keeping(_ routes: [String]) -> [String] {
        routes.filter { !retires($0) }
    }
}
