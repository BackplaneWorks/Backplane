// TermStream against the cases of the hub's native/host/src/term/stream.rs (`frames_merge_by_offset`) and the push rules of docs/PHONE-DATA.md. Compiled with
// Backplane/TermStream.swift alone, outside the app: scripts/mobile/ios-unit-tests.sh.
import Foundation

var failed = 0, passed = 0
func check(_ ok: Bool, _ name: String, _ got: Any) { if ok { passed += 1 } else { failed += 1; print("FAIL \(name): got \(got)") } }

// stream.rs: frames_merge_by_offset
check(TermStream.merge(held: 10, o: 10, len: 5) == .take(0), "merge: next frame", TermStream.merge(held: 10, o: 10, len: 5))
check(TermStream.merge(held: 10, o: 8, len: 5) == .take(2), "merge: overlap", TermStream.merge(held: 10, o: 8, len: 5))
check(TermStream.merge(held: 10, o: 3, len: 5) == .skip, "merge: held already", TermStream.merge(held: 10, o: 3, len: 5))
check(TermStream.merge(held: 10, o: 12, len: 5) == .gap, "merge: gap", TermStream.merge(held: 10, o: 12, len: 5))
check(TermStream.merge(held: 10, o: 10, len: 0) == .skip, "merge: empty", TermStream.merge(held: 10, o: 10, len: 0))

let held = TermHeld(g: 7, o: 10)
func ap(_ held: TermHeld?, g: Int? = 7, o: Int? = 10, _ b: String = "", reset: [Int]? = nil, state: String? = nil) -> [TermAction] {
    TermStream.apply(held: held, g: g, o: o, bytes: Array(b.utf8), reset: reset, state: state)
}
check(ap(held, "hello") == [.feed(Array("hello".utf8), held: TermHeld(g: 7, o: 15))], "bytes at the held offset are fed and held", ap(held, "hello"))
check(ap(held, o: 8, "abcde") == [.feed(Array("cde".utf8), held: TermHeld(g: 7, o: 13))], "overlap feeds only what is new", ap(held, o: 8, "abcde"))
check(ap(held, o: 3, "abcde") == [], "all held already is skipped", ap(held, o: 3, "abcde"))
check(ap(held, o: 12, "abcde") == [.reread], "a gap asks again", ap(held, o: 12, "abcde"))
check(ap(held, g: 8, "x") == [.reread], "another run asks again", ap(held, g: 8, "x"))
check(ap(held, g: nil, o: nil, "x") == [.reread], "no g asks again", ap(held, g: nil, o: nil, "x"))
check(ap(nil, "x") == [.reread], "bytes with nothing held ask", ap(nil, "x"))
check(ap(held, o: 10, reset: [80, 24], state: nil) == [.snapshot(cols: 80, rows: 24, [], held: TermHeld(g: 7, o: 10))], "a snapshot replaces and holds its o", ap(held, o: 10, reset: [80, 24]))
check(ap(held, g: 9, o: 500, "z", reset: [40, 10]) == [.snapshot(cols: 40, rows: 10, Array("z".utf8), held: TermHeld(g: 9, o: 500))], "a snapshot of another run is taken as it is", "")
check(ap(held, state: "done 0") == [.state("done 0")], "a state frame is only a state", ap(held, state: "done 0"))
check(ap(nil, state: "stopped") == [.state("stopped")], "a state with nothing held", ap(nil, state: "stopped"))
check(ap(held, g: 8, state: "running") == [.state("running"), .reread], "a state of another run also asks again", ap(held, g: 8, state: "running"))
check(ap(held, g: nil, o: nil, state: "stopped") == [.state("stopped")], "a state frame with no run is only a state", "")
check(ap(held, o: 10, "ab", reset: [80, 24], state: "running") == [.state("running"), .snapshot(cols: 80, rows: 24, Array("ab".utf8), held: TermHeld(g: 7, o: 10))], "the first push of a watch carries both", "")

print("term stream: \(passed) passed, \(failed) failed"); exit(failed == 0 ? 0 : 1)
