import Foundation

@main struct PresentationIntentTests {
    static func main() {
        func check(_ condition: @autoclosure () -> Bool, _ name: String) {
            guard condition() else { fputs("FAIL \(name)\n", stderr); exit(1) }
            print("PASS \(name)")
        }
        let original = PresentationIntent.Context(hub: "a", selection: "t", nav: "n", epoch: 1, pairing: 1)
        var held = PresentationIntent()
        held.offer("view", "board", context: original)
        held.offer("view", "3d", context: original)
        check(held.take(context: original, ready: false) == nil, "not ready retains latest intent")
        let once = held.take(context: original, ready: true)
        check(once?.0 == "view" && once?.1 == "3d", "latest viewer intent wins")
        check(held.take(context: original, ready: true) == nil, "intent delivered once")
        for changed in [
            PresentationIntent.Context(hub: "b", selection: "t", nav: "n", epoch: 1, pairing: 1),
            PresentationIntent.Context(hub: "a", selection: "other", nav: "n", epoch: 1, pairing: 1),
            PresentationIntent.Context(hub: "a", selection: "t", nav: "other", epoch: 1, pairing: 1),
            PresentationIntent.Context(hub: "a", selection: "t", nav: "n", epoch: 2, pairing: 1),
            PresentationIntent.Context(hub: "a", selection: "t", nav: "n", epoch: 1, pairing: 2)
        ] {
            held.offer("bot-tab", "space", context: original)
            check(held.take(context: changed, ready: false) == nil && held.take(context: original, ready: true) == nil, "changed owner or generation cancels")
        }
        held.offer("send", "ignored", context: original)
        check(held.take(context: original, ready: true) == nil, "mutating actions cannot be queued")
        held.offer("bot-tab", "space", context: original)
        held.clear()
        check(held.take(context: original, ready: true) == nil, "explicit cancellation drops intent")
    }
}
