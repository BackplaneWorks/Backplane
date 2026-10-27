import Foundation

@main struct ViewerCloseIntentTests {
    static func main() {
        func check(_ condition: @autoclosure () -> Bool, _ name: String) {
            guard condition() else { fputs("FAIL \(name)\n", stderr); exit(1) }
            print("PASS \(name)")
        }
        let owner = ViewerCloseIntent.Owner(hub: "hub", nav: "hub|thread", pairing: 1)
        let source = ViewerCloseIntent.Source(kind: "board", key: "board-key", layers: "layers")
        var state = ViewerCloseIntent()
        let first = state.open(owner: owner, source: source)
        check(state.close(first) && state.locallyClosed, "close immediately hides local cover")
        check(!state.canSend(first, owner: owner, source: source, ready: false), "offline close stays deferred")
        check(state.canSend(first, owner: owner, source: source, ready: true), "same presentation can close when ready")
        check(!state.canSend(first, owner: .init(hub: "other", nav: owner.nav, pairing: 1), source: source, ready: true), "another hub cannot receive close")
        check(!state.canSend(first, owner: .init(hub: owner.hub, nav: "other", pairing: 1), source: source, ready: true), "another selection cannot receive close")
        check(!state.canSend(first, owner: .init(hub: owner.hub, nav: owner.nav, pairing: 2), source: source, ready: true), "replacement pairing cannot receive close")
        check(!state.canSend(first, owner: owner, source: .init(kind: "3d", key: source.key, layers: source.layers), ready: true), "changed source cannot receive close")
        state.sent(first)
        check(!state.canSend(first, owner: owner, source: source, ready: true) && state.locallyClosed, "sent once and stays locally hidden")
        let second = state.open(owner: owner, source: source)
        check(first != second && !state.locallyClosed, "same-source reopen has a new presentation")
        check(!state.close(first) && !state.locallyClosed, "late old dismissal cannot close reopened viewer")
        check(state.close(second), "current presentation closes")
        state.cancel()
        check(!state.canSend(second, owner: owner, source: source, ready: true) && state.current == nil, "navigation cancels pending close")
        let third = state.open(owner: owner, source: source)
        _ = state.close(third)
        _ = state.open(owner: owner, source: .init(kind: "3d", key: "new", layers: ""))
        check(!state.canSend(third, owner: owner, source: source, ready: true), "new source cancels old pending close")
    }
}
