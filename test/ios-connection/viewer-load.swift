import Foundation

@main struct ViewerLoadStateTests {
    static func main() {
        func check(_ condition: @autoclosure () -> Bool, _ name: String) {
            guard condition() else { fputs("FAIL \(name)\n", stderr); exit(1) }
            print("PASS \(name)")
        }
        let id = ViewerLoadState.Identity(hub: "hub", navigation: "hub|thread", kind: "3d", key: "mesh", layers: "board", pairing: 1, epoch: 2, presentation: 3)
        var load = ViewerLoadState(timeoutMilliseconds: 30_000)
        let first = load.start(id, geometry: true, mesh: true, now: 1_000)
        check(load.status(first, now: 30_999) == .loading, "healthy connection cannot extend missing plot deadline")
        check(load.status(first, now: 31_000) == .timedOut, "missing geometry reaches deadline")
        load.geometryReady(first)
        check(load.status(first, now: 31_001) == .timedOut, "base geometry alone does not satisfy mesh")
        load.meshReady(first)
        check(load.status(first, now: 31_002) == .ready, "late current mesh can complete")
        let retry = load.start(id, geometry: true, mesh: true, now: 40_000)
        load.geometryReady(first); load.meshReady(first)
        check(load.status(first, now: 100_000) == .idle && load.status(retry, now: 40_001) == .loading, "old completion and timer cannot satisfy retry")
        load.geometryReady(retry)
        check(load.status(retry, now: 40_002) == .loading, "base without mesh remains loading")
        load.reset()
        load.meshReady(retry)
        check(load.status(retry, now: 100_000) == .idle, "close cancels pending deadline")
        let nextID = ViewerLoadState.Identity(hub: "hub", navigation: "hub|other", kind: "board", key: "", layers: "other", pairing: 1, epoch: 2, presentation: 4)
        let next = load.start(nextID, geometry: true, mesh: false, now: 100_000)
        load.geometryReady(retry)
        check(load.status(next, now: 100_001) == .loading, "late old owner cannot satisfy new source")
        load.geometryReady(next)
        check(load.status(next, now: 100_002) == .ready, "board completes without mesh")
    }
}
