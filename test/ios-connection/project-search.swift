import Foundation

@main struct ProjectSearchIntentTests {
    static func main() {
        func check(_ condition: @autoclosure () -> Bool, _ name: String) {
            guard condition() else { fputs("FAIL \(name)\n", stderr); exit(1) }
            print("PASS \(name)")
        }
        let owner = ProjectSearchIntent.Owner(generation: 1)
        let other = ProjectSearchIntent.Owner(generation: 2)
        var state = ProjectSearchIntent()
        let initial = state.begin(text: "", owner: owner)
        let a = state.edit("a", scope: initial.scope, owner: owner)!
        let ab = state.edit("ab", scope: initial.scope, owner: owner)!
        let committed = state.submit(scope: initial.scope, owner: owner)!
        check(committed == ab, "return captures latest complete query")
        check(state.accept(a, projects: ["stale"]) == nil, "older prefix cannot commit")
        state.disappear(scope: initial.scope)
        let fresh = state.accept(committed, projects: ["hub|fresh", "hub|later"])
        check(fresh?.project == "hub|fresh" && fresh?.query.text == "ab", "accepted fresh first row wins")
        check(state.accept(committed, projects: ["hub|fresh"]) == nil, "go dispatches once")

        let next = state.begin(text: "old", owner: owner)
        let waiting = state.submit(scope: next.scope, owner: owner)!
        let changed = state.edit("new", scope: next.scope, owner: owner)!
        check(changed.scope > next.scope && state.committed == waiting, "edit after return owns newer scope")
        state.disappear(scope: next.scope)
        check(state.editing == changed, "old disappearance cannot cancel newer field")
        check(state.accept(waiting, projects: []) == nil && state.editing == changed, "empty result retires go but retains edit")
        let newGo = state.submit(scope: changed.scope, owner: owner)!
        check(state.accept(newGo, projects: ["hub|new"])?.project == "hub|new", "return works after no match")

        let none = state.begin(text: "missing", owner: owner)
        let noGo = state.submit(scope: none.scope, owner: owner)!
        check(state.accept(noGo, projects: []) == nil, "no matches produce no go")
        let resumed = state.edit("found", scope: none.scope, owner: owner)
        check(resumed != nil && resumed!.scope > none.scope, "edit after rejected go resumes")
        state.cancel()

        let s = state.begin(text: "x", owner: owner)
        check(state.edit("wrong", scope: s.scope, owner: other) == nil, "different owner cannot edit")
        let pending = state.submit(scope: s.scope, owner: owner)!
        check(state.edit("wrong", scope: s.scope, owner: other) == nil, "different owner cannot revive submitted scope")
        state.cancel()
        check(state.accept(pending, projects: ["hub|x"]) == nil, "navigation or pairing cancellation prevents go")
        let last = state.begin(text: "clear", owner: owner)
        let clear = state.edit("", scope: last.scope, owner: owner)!
        check(clear.text.isEmpty, "empty clear is a full query")
        state.disappear(scope: last.scope)
        check(state.editing == nil, "visible field disappearance cancels editing")
    }
}
