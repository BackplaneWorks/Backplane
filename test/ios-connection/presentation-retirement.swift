import Foundation

@main
struct PresentationRetirementTests {
    static func main() {
        func check(_ condition: @autoclosure () -> Bool, _ label: String) {
            precondition(condition(), label)
            print("ok \(label)")
        }
        let a = "a.invalid:3787", b = "b.invalid:3787"
        let rawRoom = "@room:shared", rawRemote = "@remote:shared"
        let aThread = a + "|thread", bThread = b + "|thread"
        let aHub = "@hub:" + a, bHub = "@hub:" + b
        let snapshots = [rawRoom: "private room", rawRemote: "private remote",
                         aThread: "a", bThread: "b"]
        let stack = ["@hubs", rawRoom, aThread, rawRemote, aHub, bThread, bHub]
        let retirement = PresentationRetirement(hub: a)

        // Exact old AppModel filters: these demonstrate the defect that the
        // new boundary predicate covers, without invoking actual pairing.
        let oldSnapshots = snapshots.filter { !$0.key.hasPrefix(a + "|") }
        let oldStack = stack.filter { !$0.hasPrefix(a + "|") && $0 != "@hub:" + a }
        check(oldSnapshots[rawRoom] != nil && oldSnapshots[rawRemote] != nil && oldStack.contains(rawRoom),
              "old prefix-only policy retains raw private snapshots and route")

        let remaining = retirement.keeping(snapshots)
        check(remaining.count == 1 && remaining[bThread] == "b", "replace or unpair removes raw and target snapshots")
        check(retirement.keeping(stack) == ["@hubs", bThread, bHub], "retire target stack and preserve other and global routes")
        check(retirement.retires(rawRoom) && retirement.retires(rawRemote), "raw intents retire despite ambiguous owner")
        check(retirement.retires(aThread) && retirement.retires(aHub), "target-qualified intent and settings retire")
        check(!retirement.retires(bThread) && !retirement.retires(bHub) && !retirement.retires("@hubs"), "other owner and global intents survive")
        check(!retirement.retires(a + ".other|thread") && !retirement.retires("@hub:" + a + ".other"), "hub matching observes exact boundary")

        let added = PresentationRetirement(hub: nil)
        check(added.keeping(stack) == ["@hubs", aThread, aHub, bThread, bHub], "add hub conservatively removes only raw routes")
        check(added.keeping(snapshots) == [aThread: "a", bThread: "b"], "unknown raw collision drops while qualified owners survive")
        check(PresentationRetirement(hub: "").keeping(stack) == added.keeping(stack), "empty hub cannot retire unrelated qualified routes")
    }
}
