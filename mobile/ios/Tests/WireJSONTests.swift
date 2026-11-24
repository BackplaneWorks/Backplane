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

// the one-time telemetry notice (feature telemetryNotice): its words as the hub sends them, null or empty is none, the act carries {token, action}
let tn = DTelemetryNotice.of(frame(#"{"token":"t1","text":"Backplane sends anonymous usage counts.","off":"Turn off","settings":"Settings"}"#))
check(tn == DTelemetryNotice(token: "t1", text: "Backplane sends anonymous usage counts.", off: "Turn off", settings: "Settings"), "telemetry notice", String(describing: tn))
check(DTelemetryNotice.of(frame(#"{"t":"telemetry.notice","notice":null}"#)["notice"]) == nil, "telemetry notice frame null", "")
check(DTelemetryNotice.of(frame(#"{"token":"","text":"x","off":"Turn off","settings":"Settings"}"#)) == nil, "telemetry notice no token", "")
check(DTelemetryNotice.of(frame(#"{"token":"t","text":"","off":"Turn off","settings":"Settings"}"#)) == nil, "telemetry notice no text", "")
check(WireJSON.decode(frame(#"{"notice":null}"#)["notice"], as: DTelemetryNotice.self) == nil, "telemetry.notice reply null", "")
check(WireJSON.decode(frame(#"{"notice":{"token":"t1","text":"x","off":"Turn off","settings":"Settings"}}"#)["notice"], as: DTelemetryNotice.self)?.valid?.settings == "Settings", "telemetry.notice reply decodes", "")
for a in ["shown", "close", "off"] {
    let p = DTelemetryNotice.act("t1", a)
    check(p.count == 2 && p["token"] as? String == "t1" && p["action"] as? String == a, "telemetry act \(a)", p)
}

// the review bar (feature reviewmode, #1305): the hub's words and the value its button sends; nothing to draw is no bar
let rb = try! JSONDecoder().decode(DReviewReply.self, from: Data(#"{"review":{"on":true,"text":"Read-only: this request looked like a review, so Backplane refuses changes to the design files.","button":"Allow edits","tip":"t","value":false}}"#.utf8)).review
check(rb == DReviewBar(on: true, text: "Read-only: this request looked like a review, so Backplane refuses changes to the design files.", button: "Allow edits", tip: "t", value: false) && rb!.drawn, "review bar on", String(describing: rb))
let ro = try! JSONDecoder().decode(DReviewReply.self, from: Data(#"{"review":{"on":false,"text":"Edits allowed","button":"Read-only again","value":true}}"#.utf8)).review
check(ro?.value == true && ro?.on == false, "review bar off: pressed again, back on", String(describing: ro))
check(try! JSONDecoder().decode(DReviewReply.self, from: Data(#"{"review":null}"#.utf8)).review == nil, "no review: no bar", "")
check(try! JSONDecoder().decode(DReviewReply.self, from: Data(#"{"review":{"on":true}}"#.utf8)).review?.drawn == false, "nothing to draw", "")

// Thread plan mode + Proposed plan ask head
let thPlan = try! JSONDecoder().decode(DThread.self, from: Data(#"{"id":"t1","project":"p","title":"T","state":"idle","updatedAt":1,"pinned":false,"pinOrder":0,"settled":false,"settledAt":0,"archived":false,"provider":"claude","model":"opus","effort":"high","runtime":"","chip":"","preview":"","waiting":0,"parent":"","history":false,"status":"","swarm":"","planMode":{"phase":"armed","interaction":"default","once":true,"scope":"once","label":"Plan (1 turn)","pending":false,"executing":false}}"#.utf8))
check(thPlan.planMode?.phase == "armed" && thPlan.planMode?.scope == "once", "DThread decodes planMode", "")
check(DPlanMode.nextCycle(for: thPlan.planMode!) == .always, "once -> always cycle", "")
let thOmitted = try! JSONDecoder().decode(DThread.self, from: Data(#"{"id":"t2","project":"p","title":"T","state":"idle","updatedAt":1,"pinned":false,"pinOrder":0,"settled":false,"settledAt":0,"archived":false,"provider":"claude","model":"opus","effort":"","runtime":"","chip":"","preview":"","waiting":0,"parent":"","history":false,"status":"","swarm":""}"#.utf8))
check(DPlanMode.effective(for: thOmitted)?.phase == "off", "Claude thread with omitted planMode defaults to off", "")
let askPlan = try! JSONDecoder().decode(DAsk.self, from: Data(#"{"id":"a1","state":"open","kind":"plan","head":"Proposed plan","question":"Do step 1","options":[],"revise":true}"#.utf8))
check(askPlan.head == "Proposed plan" && askPlan.revise && askPlan.kind == "plan", "DAsk decodes head & revise", "")

// Commerce lead_days + estimate_warnings across DOrder, DBuy, DProjectCard
let ord = try! JSONDecoder().decode(DOrder.self, from: Data(#"{"name":"Main","steps":[],"line":"Quoted","waiting":false,"total":"$27.20","customs":"","pay":"","label":"Buy now","lead_days":9,"estimate_warnings":"thin board"}"#.utf8))
check(ord.leadDays == 9 && ord.estimateWarnings == "thin board", "DOrder decodes lead_days & estimate_warnings", ord)
let ordPlain = try! JSONDecoder().decode(DOrder.self, from: Data(#"{"name":"Main","steps":[],"line":"Reply","waiting":false,"lead_days":-2}"#.utf8))
check(ordPlain.leadDays == 0 && ordPlain.estimateWarnings == "", "DOrder clamps negative/omitted fields", ordPlain)
let buy = try! JSONDecoder().decode(DBuy.self, from: Data(#"{"label":"Buy it","tip":"t","thread":"t1","lead_days":5,"estimate_warnings":"fragile"}"#.utf8))
check(buy.leadDays == 5 && buy.estimateWarnings == "fragile", "DBuy decodes lead_days & estimate_warnings", buy)
let pcard = try! JSONDecoder().decode(DProjectCard.self, from: Data(#"{"goal":"Desk lamp","boards":["main"],"lead_days":7,"estimate_warnings":"warn1"}"#.utf8))
check(pcard.leadDays == 7 && pcard.estimateWarnings == "warn1", "DProjectCard decodes lead_days & estimate_warnings", pcard)

print("wire-json: \(passed) passed, \(failed) failed")
exit(failed == 0 ? 0 : 1)
