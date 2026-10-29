@main struct PresentationReadinessChecks {
    static func main() {
        let oldScreen = "hub|old", requested = "hub|requested"
        let oldPredicate = requested == (Optional(requested) ?? oldScreen)
        precondition(oldPredicate, "The old predicate must reproduce false freshness")
        let checks: [(String, Bool)] = [
            ("awaited marker cannot freshen another displayed thread", !NavigationPresentationReadiness.matches(displayed: oldScreen, mark: requested, awaited: requested)),
            ("current target and matching marker become ready", NavigationPresentationReadiness.matches(displayed: requested, mark: requested, awaited: requested)),
            ("no awaited navigation accepts matching current picture", NavigationPresentationReadiness.matches(displayed: oldScreen, mark: oldScreen, awaited: nil)),
            ("old picture stays unready while a newer target waits", !NavigationPresentationReadiness.matches(displayed: oldScreen, mark: oldScreen, awaited: requested)),
            ("matching request cannot excuse an old marker", !NavigationPresentationReadiness.matches(displayed: requested, mark: oldScreen, awaited: requested)),
            ("same thread name on another hub is a different picture", !NavigationPresentationReadiness.matches(displayed: "other|requested", mark: requested, awaited: requested)),
            ("current root is ready without pending navigation", NavigationPresentationReadiness.matches(displayed: "", mark: "", awaited: nil)),
            ("explicit root waits for root picture", !NavigationPresentationReadiness.matches(displayed: requested, mark: requested, awaited: "")),
            ("explicit root completes at root picture", NavigationPresentationReadiness.matches(displayed: "", mark: "", awaited: ""))
        ]
        for (name, pass) in checks { precondition(pass, name); print("PASS \(name)") }
    }
}
