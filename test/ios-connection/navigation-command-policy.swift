@main struct NavigationCommandPolicyChecks {
    static func main() {
        let paired: Set<String> = ["A", "B"]
        func keeps(_ type: String, _ hub: String? = nil, _ thread: String? = nil) -> Bool {
            NavigationCommandPolicy.preserves(type: type, hub: hub, thread: thread, retiredHub: "B", paired: paired)
        }
        let checks: [(String, Bool)] = [
            ("stale same-hub view request is discarded", !keeps("send", "B")),
            ("unaffected paired hub request survives", keeps("send", "A")),
            ("unpaired request is discarded", !keeps("send", "C")),
            ("unscoped request is discarded", !keeps("send")),
            ("jump cannot survive retired navigation", !keeps("jump", "A", "A|t")),
            ("later action cannot survive retired navigation", !keeps("later", "A", "A|t")),
            ("copy cannot survive retired navigation", !keeps("copy", "A", "A|t")),
            ("same-hub draft effect is discarded", !keeps("draft-kept", nil, "B|t")),
            ("unaffected draft effect survives", keeps("draft-kept", nil, "A|t")),
            ("unqualified draft is discarded", !keeps("draft-kept", nil, "t")),
            ("empty owner is discarded", !keeps("keep", nil, "|t")),
            ("unaffected send receipt survives", keeps("send-issued", nil, "A|t"))
        ]
        for (name, pass) in checks { precondition(pass, name); print("PASS \(name)") }
    }
}
