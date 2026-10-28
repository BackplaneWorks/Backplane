import Foundation

@main
struct NavigationResumeTests {
    static func main() async {
        func check(_ value: @autoclosure () -> Bool, _ label: String) {
            precondition(value(), label); print("ok \(label)")
        }
        var state = NavigationResumeIntent()
        let a = "a.invalid:3787", b = "b.invalid:3787", tab = "threads"
        let bNav = a + "|thread-b"
        let first = state.capture(hub: a, pairing: 7, tab: tab, action: .select, value: bNav, nav: bNav)!
        let issued = state.claim(first, hub: a, pairing: 7, tab: tab, epoch: 1)!
        check(issued.action == .select && issued.value == bNav && issued.nav == bNav, "initial dispatch retains exact requested B")
        check(state.claim(first, hub: a, pairing: 7, tab: tab, epoch: 1) == nil, "at most once in epoch")
        check(!state.accept(first, hub: a, pairing: 7, tab: tab, epoch: 1, nav: a + "|thread-a", fresh: true), "old A cannot acknowledge B")
        let replay = state.claim(first, hub: a, pairing: 7, tab: tab, epoch: 2)!
        check(replay == first && replay.value == bNav, "new epoch replays B rather than root or A")
        check(state.claim(first, hub: a, pairing: 7, tab: tab, epoch: 1) == nil, "retired epoch cannot reclaim after reconnect")
        check(!state.accept(first, hub: a, pairing: 7, tab: tab, epoch: 1, nav: bNav, fresh: true), "late old epoch cannot acknowledge")
        check(!state.accept(first, hub: a, pairing: 7, tab: tab, epoch: 2, nav: bNav, fresh: false), "unfresh picture cannot acknowledge")
        check(!state.accept(first, hub: b, pairing: 7, tab: tab, epoch: 2, nav: bNav, fresh: true), "other hub cannot acknowledge")
        check(!state.current(first, hub: a, pairing: 8, tab: tab), "pairing replacement invalidates current guard")
        check(!state.current(first, hub: a, pairing: 7, tab: "home"), "tab switch invalidates current guard")
        check(state.accept(first, hub: a, pairing: 7, tab: tab, epoch: 2, nav: bNav, fresh: true), "matching fresh current presentation clears")
        check(!state.current(first, hub: a, pairing: 7, tab: tab), "accepted intent cannot replay")

        let old = state.capture(hub: a, pairing: 7, tab: tab, action: .select, value: bNav, nav: bNav)!
        _ = state.claim(old, hub: a, pairing: 7, tab: tab, epoch: 3)
        let root = state.capture(hub: a, pairing: 7, tab: tab, action: .select, value: "", nav: "")!
        check(state.latest == root, "latest exposes superseding token")
        check(!state.current(old, hub: a, pairing: 7, tab: tab), "new root supersedes old navigation")
        check(state.claim(root, hub: a, pairing: 7, tab: tab, epoch: 3)?.value == "", "root intent retains empty value")
        check(!state.accept(old, hub: a, pairing: 7, tab: tab, epoch: 3, nav: bNav, fresh: true), "old completion cannot clear newer root")
        check(state.accept(root, hub: a, pairing: 7, tab: tab, epoch: 3, nav: "", fresh: true), "fresh root may complete")

        let rejected = state.capture(hub: a, pairing: 7, tab: "bots", action: .uber, value: "raw-bot", nav: bNav)!
        _ = state.claim(rejected, hub: a, pairing: 7, tab: "bots", epoch: 4)
        let firstClaim = state.claimID!
        check(state.rejected(rejected, hub: a, pairing: 7, tab: "bots", epoch: 4, claim: firstClaim), "explicit rejection releases claim")
        check(state.claim(rejected, hub: a, pairing: 7, tab: "bots", epoch: 3) == nil, "rejection retains epoch watermark")
        check(!state.accept(rejected, hub: a, pairing: 7, tab: "bots", epoch: 4, nav: bNav, fresh: true), "rejected claim cannot acknowledge")
        check(state.claim(rejected, hub: a, pairing: 7, tab: "bots", epoch: 4) != nil, "current epoch can retry after explicit rejection")
        check(!state.rejected(rejected, hub: a, pairing: 7, tab: "bots", epoch: 4, claim: firstClaim), "late rejection cannot release same-epoch retry")
        _ = state.claim(rejected, hub: a, pairing: 7, tab: "bots", epoch: 5)
        check(!state.rejected(rejected, hub: a, pairing: 7, tab: "bots", epoch: 4, claim: firstClaim), "late rejection cannot release newer epoch")
        check(state.claim(rejected, hub: a, pairing: 7, tab: "bots", epoch: 5) == nil, "newer claim remains held")

        let prime = state.capture(hub: a, pairing: 7, tab: "bots", action: .uber, value: "raw", nav: bNav)!
        check(!state.beginPrime(prime, hub: a, pairing: 7, tab: "bots", epoch: 6), "prime requires explicit rejection")
        _ = state.claim(prime, hub: a, pairing: 7, tab: "bots", epoch: 6)
        check(state.rejected(prime, hub: a, pairing: 7, tab: "bots", epoch: 6, claim: state.claimID!), "prime rejection releases claim")
        check(state.beginPrime(prime, hub: a, pairing: 7, tab: "bots", epoch: 6), "begin prime after rejection")
        check(state.claim(prime, hub: a, pairing: 7, tab: "bots", epoch: 6) == nil, "prime blocks same epoch claim")
        check(!state.finishPrime(prime, hub: b, pairing: 7, tab: "bots", epoch: 6), "wrong owner cannot finish prime")
        check(state.finishPrime(prime, hub: a, pairing: 7, tab: "bots", epoch: 6), "finish prime after output applied")
        check(!state.beginPrime(prime, hub: a, pairing: 7, tab: "bots", epoch: 6), "prime at most once per epoch")
        check(state.claim(prime, hub: a, pairing: 7, tab: "bots", epoch: 6) != nil, "finish permits same epoch retry")
        _ = state.claim(prime, hub: a, pairing: 7, tab: "bots", epoch: 7)
        check(state.rejected(prime, hub: a, pairing: 7, tab: "bots", epoch: 7, claim: state.claimID!), "new epoch rejection")
        check(state.beginPrime(prime, hub: a, pairing: 7, tab: "bots", epoch: 7), "new epoch permits prime")
        check(state.claim(prime, hub: a, pairing: 7, tab: "bots", epoch: 8) != nil, "new epoch retires old prime")
        check(!state.finishPrime(prime, hub: a, pairing: 7, tab: "bots", epoch: 7), "old finish cannot affect newer epoch")
        check(state.rejected(prime, hub: a, pairing: 7, tab: "bots", epoch: 8, claim: state.claimID!), "newer claim can be rejected")
        check(state.beginPrime(prime, hub: a, pairing: 7, tab: "bots", epoch: 8), "newer prime begins")
        check(!state.finishPrime(prime, hub: a, pairing: 7, tab: "bots", epoch: 7), "old finish cannot clear newer prime")
        check(state.claim(prime, hub: a, pairing: 7, tab: "bots", epoch: 8) == nil, "newer prime remains blocked")
        let replacement = state.capture(hub: a, pairing: 7, tab: "bots", action: .bot, value: "other", nav: bNav)!
        check(!state.finishPrime(prime, hub: a, pairing: 7, tab: "bots", epoch: 8), "old token cannot finish replacement")
        check(state.claim(replacement, hub: a, pairing: 7, tab: "bots", epoch: 8) != nil, "replacement is not blocked by old prime")

        for action in [NavigationResumeIntent.Action.bot, .room, .remote, .uber, .uberBot] {
            let token = state.capture(hub: a, pairing: 9, tab: "bots", action: action, value: "qualified-value", nav: "expected-nav")!
            let retried = state.claim(token, hub: a, pairing: 9, tab: "bots", epoch: 4)!
            check(retried.action == action && retried.value == "qualified-value", "navigation action survives reconnect: \(action.rawValue)")
        }
        let retired = state.capture(hub: a, pairing: 9, tab: tab, action: .select, value: bNav, nav: bNav)!
        state.cancel()
        check(state.latest == nil, "retirement removes latest token")
        check(state.claim(retired, hub: a, pairing: 9, tab: tab, epoch: 5) == nil, "account or navigation retirement cancels")
        check(state.capture(hub: "", pairing: 9, tab: tab, action: .select, value: "", nav: "") == nil, "unowned intent rejected")
        await controlledPrimeOrdering()
    }

    @MainActor
    static func controlledPrimeOrdering() async {
        for scenario in ["matching", "coalesced", "newer-refresh", "stale-presentation"] {
            var state = NavigationResumeIntent()
            let hub = "a.invalid:3787", tab = "bots", nav = hub + "|b"
            let token = state.capture(hub: hub, pairing: 2, tab: tab, action: .uber, value: "b", nav: nav)!
            _ = state.claim(token, hub: hub, pairing: 2, tab: tab, epoch: 3)
            precondition(state.rejected(token, hub: hub, pairing: 2, tab: tab, epoch: 3, claim: state.claimID!))
            precondition(state.beginPrime(token, hub: hub, pairing: 2, tab: tab, epoch: 3))
            var resumePrime: CheckedContinuation<Void, Never>?
            var attempts = 0, dispatched = 0
            var hostReady = false
            var hostRevision = 12, presentedRevision = 11
            var applied = false
            var prime: Task<Void, Never>?
            await withCheckedContinuation { (started: CheckedContinuation<Void, Never>) in
                prime = Task { @MainActor in
                    // JS select has returned; its native completion is suspended.
                    await withCheckedContinuation {
                        resumePrime = $0
                        started.resume()
                    }
                    applied = true
                    precondition(state.finishPrime(token, hub: hub, pairing: 2, tab: tab, epoch: 3))
                    if state.claim(token, hub: hub, pairing: 2, tab: tab, epoch: 3) != nil {
                        precondition(applied)
                        attempts += 1
                        // Model only the host result; actual host/bridge tests
                        // separately exercise its serialized revision admission.
                        if hostReady && hostRevision == presentedRevision { dispatched += 1 }
                        else {
                            precondition(state.rejected(token, hub: hub, pairing: 2, tab: tab, epoch: 3, claim: state.claimID!))
                        }
                    }
                }
            }
            // Process the callback(s) before the prime await can resume.
            if scenario == "matching" || scenario == "coalesced" {
                hostReady = true; presentedRevision = 12
            } else if scenario == "newer-refresh" {
                hostReady = true; hostRevision = 13; presentedRevision = 13
                precondition(state.claim(token, hub: hub, pairing: 2, tab: tab, epoch: 3) == nil)
            }
            precondition(state.claim(token, hub: hub, pairing: 2, tab: tab, epoch: 3) == nil)
            resumePrime!.resume()
            await prime!.value
            precondition(attempts == 1)
            if scenario == "stale-presentation" {
                precondition(dispatched == 0)
                precondition(!state.beginPrime(token, hub: hub, pairing: 2, tab: tab, epoch: 3))
                hostReady = true; presentedRevision = hostRevision
                precondition(state.claim(token, hub: hub, pairing: 2, tab: tab, epoch: 3) != nil)
                dispatched += 1
            }
            precondition(dispatched == 1)
            precondition(state.claim(token, hub: hub, pairing: 2, tab: tab, epoch: 3) == nil)
            print("ok suspended prime continuation: \(scenario) dispatches once, stale presentation cannot authorize")
        }
    }
}
