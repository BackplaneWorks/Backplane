import Foundation

@main struct ViewerResumeIntentTests {
    static func main() {
        func check(_ condition: @autoclosure () -> Bool, _ name: String) {
            guard condition() else { fputs("FAIL \(name)\n", stderr); exit(1) }
            print("PASS \(name)")
        }
        let owner = ViewerCloseIntent.Owner(hub: "h", nav: "h|thread", pairing: 4)
        let source = ViewerCloseIntent.Source(kind: "board", key: "key", layers: "layers")
        let token = ViewerCloseIntent.Token(owner: owner, source: source, presentation: 8)
        let allowed: Set<String> = ["board", "3d"]
        var resume = ViewerResumeIntent()
        resume.capture(token, fromEpoch: 10)
        check(resume.preserves(token), "preserve same visible presentation on empty reconnect screen")
        check(resume.claim(current: token, epoch: 9, ready: true, allowed: allowed) == nil, "old epoch cannot claim")
        check(resume.claim(current: token, epoch: 10, ready: true, allowed: allowed) == nil, "same epoch cannot claim")
        check(resume.claim(current: token, epoch: 11, ready: false, allowed: allowed) == nil && resume.preserves(token), "wait for fresh readiness")
        check(resume.claim(current: token, epoch: 11, ready: true, allowed: allowed) == "board", "new epoch claims captured kind")
        check(resume.claim(current: token, epoch: 11, ready: true, allowed: allowed) == nil && !resume.preserves(token), "claim occurs once")

        resume.capture(token, fromEpoch: 10)
        check(resume.claim(current: token, epoch: 11, ready: true, allowed: ["3d"]) == nil && !resume.preserves(token), "unavailable kind retires request")
        resume.capture(token, fromEpoch: 10); resume.cancel()
        check(resume.claim(current: token, epoch: 11, ready: true, allowed: allowed) == nil, "explicit close cancels")
        resume.capture(token, fromEpoch: 10)
        check(resume.claim(current: nil, epoch: 11, ready: true, allowed: allowed) == nil && !resume.preserves(token), "closed presentation cannot resume")

        let replacement = ViewerCloseIntent.Token(owner: .init(hub: "h", nav: "h|thread", pairing: 5), source: source, presentation: 8)
        resume.capture(token, fromEpoch: 10)
        check(resume.claim(current: replacement, epoch: 11, ready: true, allowed: allowed) == nil, "same address replacement pairing rejected")
        let newer = ViewerCloseIntent.Token(owner: owner, source: source, presentation: 9)
        resume.capture(token, fromEpoch: 10)
        check(resume.claim(current: newer, epoch: 11, ready: true, allowed: allowed) == nil, "new presentation invalidates old open")
        let other = ViewerCloseIntent.Token(owner: .init(hub: "other", nav: "other|thread", pairing: 4), source: source, presentation: 8)
        resume.capture(token, fromEpoch: 10)
        check(resume.claim(current: other, epoch: 11, ready: true, allowed: allowed) == nil, "hub or selection change invalidates")
    }
}
