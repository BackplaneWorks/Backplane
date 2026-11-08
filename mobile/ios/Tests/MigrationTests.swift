// The 0.14 import's card (Backplane/Migration.swift) against frames in the hub's shapes (present/onboard.rs import_card). Compiled with Migration.swift alone,
// outside the app: scripts/mobile/ios-unit-tests.sh.
import Foundation

var failed = 0, passed = 0
func check(_ ok: Bool, _ name: String, _ got: Any) { if ok { passed += 1 } else { failed += 1; print("FAIL \(name): got \(got)") } }
func json(_ s: String) -> Any? { try? JSONSerialization.jsonObject(with: Data(s.utf8)) }

let running = json(#"""
{"t":"migrating","state":"running","phase":"writing","what":"Writing your threads","done":120000,"total":300000,"pct":41.6,"eta_s":95,"projects":[20,50],"threads":[2000,5000],"current":"Board rev B",
 "card":{"state":"running","title":"Bringing your 0.14 work across…","body":"Writing your threads. Backplane works meanwhile, and your 0.14 folder is only read.","pct":42,
 "lines":["20 of 50 projects · 2,000 of 5,000 threads · 120,000 of 300,000 events","Now: Board rev B","About 2 min left in this step."],"buttons":[]}}
"""#)
let done = json(#"""
{"t":"migrating","state":"done","card":{"state":"done","title":"Your 0.14 work is here","body":"50 projects and 5,000 threads came across.","pct":100,"lines":["It took 230 s."],
 "buttons":[{"label":"Continue","action":"ob-import","value":"ack:1762620000","on":true},{"label":"Import with keys","action":"ob-import","value":"run-secrets","on":false}]}}
"""#)
let failedFrame = json(#"""
{"t":"migrating","state":"failed","card":{"state":"failed","title":"The import stopped","body":"The disk is full. Your 0.14 data is untouched.","pct":null,"lines":[],
 "buttons":[{"label":"Retry","action":"ob-import","value":"retry","on":true},{"label":"Not now","action":"ob-import","value":"later","on":false}]}}
"""#)

var m = DMigration()
check(m.card == nil && m.canClose, "nothing at first", m)
m.heard(json(#"{"t":"hello","migrating":null}"#).flatMap { ($0 as? [String: Any])?["migrating"] })
check(m.card == nil, "hello migrating null: no banner", m)

m.heard(running)
check(m.card?.title == "Bringing your 0.14 work across…" && m.card?.running == true, "running: the hub's title", m.card as Any)
check(m.card?.pct == 42 && m.card?.fill == 0.42, "the bar from the card's pct", m.card?.fill as Any)
check(m.card?.short == ["20 of 50 projects · 2,000 of 5,000 threads · 120,000 of 300,000 events", "Now: Board rev B"], "the banner: the first two lines", m.card?.short as Any)
check(m.card?.lines.count == 3 && m.card?.buttons.isEmpty == true, "the card: every line, no buttons", m.card as Any)
check(!m.canClose, "no dismiss while it runs", m.canClose)
m.answered()
check(m.card != nil, "an answer while it runs keeps it (a Retry's first frame came before its reply)", m)

m.heard(done)
check(m.card?.state == "done" && m.card?.fill == 1 && m.canClose, "done: full bar, can be answered", m.card as Any)
check(m.card?.buttons.map(\.label) == ["Continue", "Import with keys"] && m.card?.buttons.first?.value == "ack:1762620000" && m.card?.buttons.first?.on == true, "done: the hub's buttons", m.card?.buttons as Any)
m.answered()
check(m.card == nil, "Continue answered: gone", m)

m.heard(failedFrame)
check(m.card?.state == "failed" && m.card?.pct == nil && m.card?.fill == nil && m.card?.short.isEmpty == true, "failed: no bar, no lines", m.card as Any)
check(m.card?.buttons.map(\.value) == ["retry", "later"], "failed: Retry and Not now", m.card?.buttons as Any)
m.answered()
check(m.card == nil, "Not now answered: gone", m)

check(DMigCard.of(json(#"{"state":"done","card":null}"#)) == nil, "a state with no card to draw: nothing", "")
check(DMigCard.of(json(#"{"card":{"state":"running","title":"","pct":3}}"#)) == nil, "no title: nothing", "")
check(DMigCard.of(json(#"{"card":{"state":"running","title":"t","pct":140}}"#))?.fill == 1, "pct beyond 100 fills the bar", "")
check(DMigCard.of(json(#"{"card":{"state":"running","title":"t","pct":0,"buttons":[{"label":"","action":"x"}]}}"#))?.buttons.isEmpty == true, "a button with no label is not drawn", "")

print("migration: \(passed) passed, \(failed) failed"); exit(failed == 0 ? 0 : 1)
