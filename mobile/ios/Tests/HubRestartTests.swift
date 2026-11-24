// HubRestart (Backplane/HubRestart.swift): what the connection strip shows around a hub restart, and when it changes next.
// Compiled with HubRestart.swift alone, outside the app (as scripts/mobile/ios-unit-tests.sh runs the others).
import Foundation

var failed = 0, passed = 0
func check(_ ok: Bool, _ name: String, _ got: Any = "") { if ok { passed += 1 } else { failed += 1; print("FAIL \(name): got \(got)") } }

let t0 = Date(timeIntervalSince1970: 1_000_000)
let build = HubRestart.Note(text: "Restarting Backplane… (new build 2c25023cc9)", heard: t0)
let crash = HubRestart.Note(text: "Backplane is back after it stopped unexpectedly", heard: t0)
func line(_ r: HubRestart.Note?, _ b: HubRestart.Note?, online: Bool, at s: TimeInterval) -> HubRestart.Line { HubRestart.line(restarting: r, back: b, online: online, now: t0.addingTimeInterval(s)) }
func next(_ r: HubRestart.Note?, _ b: HubRestart.Note?, online: Bool, at s: TimeInterval) -> Date? { HubRestart.next(restarting: r, back: b, online: online, now: t0.addingTimeInterval(s)) }

// no restart said: the usual look
check(line(nil, nil, online: false, at: 0) == .usual, "a drop with no frame: usual")
check(line(nil, nil, online: true, at: 0) == .none, "connected: nothing")
check(next(nil, nil, online: false, at: 0) == nil, "nothing said: no timer")

// hub.restarting: its words while there is no hub, for 5 minutes
check(line(build, nil, online: true, at: 0) == .none, "heard while still connected: nothing yet")
check(line(build, nil, online: false, at: 1) == .note(build.text), "socket down: the hub's words", line(build, nil, online: false, at: 1))
check(line(build, nil, online: false, at: 299) == .note(build.text), "still within 5 minutes")
check(line(build, nil, online: false, at: 300) == .usual, "5 minutes with no hub: the usual look")
check(line(build, nil, online: false, at: 3600) == .usual, "long after: the usual look")
check(next(build, nil, online: false, at: 1) == t0.addingTimeInterval(300), "one timer, at the 5 minutes")
check(next(build, nil, online: false, at: 300) == nil, "past it: no timer")
check(next(build, nil, online: true, at: 1) == nil, "connected: the restarting note needs no timer")

// hello.hubBack: its words once connected, for 6 seconds
check(line(nil, crash, online: true, at: 0) == .note(crash.text), "joined after a crash: the hub's words")
check(line(nil, crash, online: true, at: 5.9) == .note(crash.text), "within 6 seconds")
check(line(nil, crash, online: true, at: 6) == .none, "after 6 seconds: nothing")
check(next(nil, crash, online: true, at: 1) == t0.addingTimeInterval(6), "one timer, at the 6 seconds")
check(line(nil, crash, online: false, at: 1) == .usual, "dropped again: the usual look, not the back note")

// parsing
check(HubRestart.restarting(["t": "hub.restarting", "reason": "stop", "text": "Backplane stopped", "at": 7]) == "Backplane stopped", "a frame's text")
check(HubRestart.restarting(["t": "hub.restarting", "text": ""]) == nil, "an empty text: no note")
check(HubRestart.back("") == nil, "hubBack empty: nothing")
check(HubRestart.back(nil) == nil, "an older hub: nothing")
check(HubRestart.back("not json") == nil, "not JSON: nothing")
let b = HubRestart.back(#"{"reason":"crash","text":"Backplane is back after it stopped unexpectedly","at":1763600000000}"#)
check(b?.text == crash.text && b?.at == "1763600000000", "hubBack's text and at", String(describing: b))

print("hub-restart: \(passed) passed, \(failed) failed")
exit(failed == 0 ? 0 : 1)
