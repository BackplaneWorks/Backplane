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
        let claimed = resume.claim(current: token, epoch: 11, ready: true, allowed: allowed)
        check(claimed == token, "new epoch claims full captured source and owner")
        check(ViewerResumeIntent.matchesSource(token, accepted: source), "same complete source accepted")
        // Hs.bar.k emits an enabled, nonnil bar at position zero even when
        // there are no history steps. This real default must remain resumable.
        check(ViewerResumeIntent.liveHistory(at: 0, playing: false, diff: false, comparing: false, menuOpen: false), "present live history bar permits default restore")
        check(!ViewerResumeIntent.liveHistory(at: 1, playing: false, diff: false, comparing: false, menuOpen: false), "saved history step cannot restore as live")
        check(!ViewerResumeIntent.liveHistory(at: 0, playing: false, diff: false, comparing: true, menuOpen: false), "comparison at zero cannot restore as live")
        check(!ViewerResumeIntent.liveHistory(at: 0, playing: false, diff: true, comparing: false, menuOpen: false), "history diff cannot restore as live")
        check(!ViewerResumeIntent.liveHistory(at: 0, playing: true, diff: false, comparing: false, menuOpen: false), "playing history cannot restore as live")
        check(!ViewerResumeIntent.liveHistory(at: 0, playing: false, diff: false, comparing: false, menuOpen: true), "version menu cannot restore as live")
        check(!ViewerResumeIntent.matchesSource(token, accepted: .init(kind: "board", key: "different-file", layers: "layers")), "different file is not restored")
        check(!ViewerResumeIntent.matchesSource(token, accepted: .init(kind: "board", key: "key", layers: "different-sheet")), "different layer or history source is not restored")
        check(!ViewerResumeIntent.matchesSource(token, accepted: .init(kind: "3d", key: "key", layers: "layers")), "different kind is not restored")
        func sourceFixture(_ kind: String, _ key: String, _ layers: String) -> ViewerCloseIntent.Source { .init(kind: kind, key: key, layers: layers) }
        check(ViewerResumeIntent.canRestoreDefault(sourceFixture("board", "0|/project|", "0|/project|")), "default board is restorable")
        check(ViewerResumeIntent.canRestoreDefault(sourceFixture("schematic", "1|/project|", "1|/project|")), "default schematic is restorable")
        check(ViewerResumeIntent.canRestoreDefault(sourceFixture("3d", "2|/project|", "0|/project|")), "default 3d is restorable")
        for suffix in ["file.kicad_pcb", "#sheet", "history:abc"] {
            check(!ViewerResumeIntent.canRestoreDefault(sourceFixture("board", "0|/project|" + suffix, "0|/project|" + suffix)), "nondefault suffix rejected")
        }
        check(!ViewerResumeIntent.canRestoreDefault(sourceFixture("board", "0||", "0||")), "empty project root rejected")
        check(!ViewerResumeIntent.canRestoreDefault(sourceFixture("3d", "2|/project|", "0|/other|")), "cross-root layers rejected")
        check(!ViewerResumeIntent.canRestoreDefault(sourceFixture("3d", "3|/project|", "0|/project|")), "bare variant rejected")
        check(!ViewerResumeIntent.canRestoreDefault(sourceFixture("3d", "4|/project|", "0|/project|")), "full variant rejected")
        check(!ViewerResumeIntent.canRestoreDefault(sourceFixture("mech", "4|/project|", "")), "mechanical source rejected")
        check(!ViewerResumeIntent.canRestoreDefault(sourceFixture("board", "0|/project|", "0|/other|")), "board layer mismatch rejected")
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
