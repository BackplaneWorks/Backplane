// WireJSON against the frames that crashed TestFlight 202611071658 (SIGABRT: NSJSONSerialization raised inside DataClient.push on
// {"k":"work","subs":null}). Compiled with Backplane/WireJSON.swift and Backplane/DataModels.swift (and ThreadGroups.swift, which it needs) alone: scripts/mobile/ios-unit-tests.sh.
// Before the fix each of these lines aborted the process; now each answers nil or JSON.
import Foundation

var failed = 0, passed = 0
func check(_ ok: Bool, _ name: String, _ got: Any) { if ok { passed += 1 } else { failed += 1; print("FAIL \(name): got \(got)") } }
func frame(_ s: String) -> [String: Any] { (try! JSONSerialization.jsonObject(with: Data(s.utf8))) as! [String: Any] }
func text(_ d: Data?) -> String { d.map { String(decoding: $0, as: UTF8.self) } ?? "nil" }

// the crash itself: a thread's work ended, the hub pushed subs: null
let ended = frame(#"{"t":"d","k":"work","thread":"t1","subs":null}"#)
check(WireJSON.decode(ended["subs"], as: DWork.self) == nil, "work ended: subs null is no work", "")
let working = frame(#"{"t":"d","k":"work","thread":"t1","subs":{"word":"work","live":1,"rows":[{"title":"Build","kind":"work","state":"running","doing":"cargo"}]}}"#)
check(WireJSON.decode(working["subs"], as: DWork.self)?.rows.first?.title == "Build", "work running decodes", "")
check(WireJSON.decode(frame(#"{"locks":null}"#)["locks"], as: [DLock].self) == nil, "locks null", "")
check(WireJSON.decode(nil, as: DWork.self) == nil, "key missing", "")

// every top-level kind JSONSerialization raises on
check(text(WireJSON.data(NSNull())) == "null", "null fragment", text(WireJSON.data(NSNull())))
check(text(WireJSON.data("a\"b")) == #""a\"b""#, "string fragment", text(WireJSON.data("a\"b")))
check(text(WireJSON.data(3)) == "3", "number fragment", text(WireJSON.data(3)))
check(text(WireJSON.data(true)) == "true", "bool fragment", text(WireJSON.data(true)))
check(WireJSON.data(Double.nan) == nil, "NaN", "")
check(WireJSON.data(["x": Double.infinity]) == nil, "nested infinity", "")
check(WireJSON.data(["when": Date()]) == nil, "nested non-JSON", "")
check(WireJSON.data(Date()) == nil, "top-level non-JSON", "")
check(WireJSON.decode("text", as: DWork.self) == nil, "string where an object belongs", "")
check(WireJSON.decode(7, as: [DLock].self) == nil, "number where an array belongs", "")

// what was fine stays the same: a call's request and an object result
check(text(WireJSON.data(["id": 1, "m": "thread.work", "p": ["thread": "t1"]] as [String: Any])).contains(#""m":"thread.work""#), "request", "")
check(WireJSON.data(frame(#"{"a":[1,null,"x"]}"#)) != nil, "nested null in an object", "")

print("wire-json: \(passed) passed, \(failed) failed")
exit(failed == 0 ? 0 : 1)
