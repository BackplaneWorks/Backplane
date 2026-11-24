// The home's rules (Backplane/HomeModel.swift) against the hub's sections (native/host/src/present/sections.rs `build`, `badge`, `snooze_of`).
// Compiled with HomeModel.swift alone, outside the app: scripts/mobile/ios-unit-tests.sh.
import Foundation

var failed = 0, passed = 0
func check(_ ok: Bool, _ name: String, _ got: Any = "") { if ok { passed += 1 } else { failed += 1; print("FAIL \(name): got \(got)") } }

// which tab draws a section: the hub's ids and kinds
check(Home.tab(section: "needs", kind: "threads") == .inbox && Home.tab(section: "running", kind: "threads") == .inbox && Home.tab(section: "snoozed", kind: "threads") == .inbox, "Needs you, Running and Snoozed are the Inbox")
for id in ["pinned", "active", "group:owner:github.com/acme", "all", "quiet"] { check(Home.tab(section: id, kind: "projects") == .bench, "a section of projects is the Bench", id) }
check(Home.tab(section: "bub", kind: "bub") == .crew, "Bub's row is the Crew's")
check(Home.tab(section: "later", kind: "threads") == .inbox, "a list of threads a newer hub adds is in the Inbox, never lost")
check(Home.tab(section: "x", kind: "chart") == nil, "an unknown kind is not drawn")
// Chats (the threads in no project) are a list of threads: the Inbox, and they do not end All clear
check(Home.tab(section: "chats", kind: "threads") == .inbox && Home.chats == "chats", "Chats are in the Inbox")
check(Home.allClear([]) && Home.allClear(["chats", "snoozed"]) && !Home.allClear(["needs", "chats"]) && !Home.allClear(["running"]), "All clear with only chats and snoozed rows")

// the status pill: the hub's label and tone as they come
check(Home.pill(label: "Approve", tone: "bad", word: "needs", want: "approval") == HomePill(label: "Approve", tone: "bad", symbol: "exclamationmark"), "the hub's label and tone")
check(Home.pill(label: "", tone: "", word: "idle", want: "") == nil, "an idle thread says nothing")
// an older hub sends no label: sections.rs `badge`'s table
let table: [(String, String, String, String)] = [("needs", "approval", "Approve", "bad"), ("needs", "sign-in", "Sign in", "bad"), ("needs", "question", "Input", "bad"), ("failed", "failed", "Failed", "bad"),
    ("cut", "stopped", "Stopped", "bad"), ("interrupted", "stopped", "Stopped", "bad"), ("running", "", "Working", "run"), ("monitoring", "", "Waiting", "wait"), ("limit", "", "Waiting", "wait"), ("done", "", "Done", "ok")]
for (word, want, label, tone) in table {
    let p = Home.pill(label: "", tone: "", word: word, want: want)
    check(p?.label == label && p?.tone == tone, "\(word)/\(want) reads \(label)", p as Any)
}
// a swarm's row in Needs you has no label; its `want` says why
check(Home.pill(label: "", tone: "", word: "needs", want: "failed")?.label == "Failed", "a swarm whose member failed")
// every word the hub can say has its own icon
let labels = ["Approve", "Input", "Sign in", "Failed", "Stopped", "Working", "Waiting", "Done"]
check(Set(labels.map(Home.symbol)).count == labels.count && !labels.map(Home.symbol).contains("circle"), "one icon per word", labels.map(Home.symbol))

// snooze options decode as the hub sends them, and their values go back untouched
let opts = (try? JSONDecoder().decode([DSnoozeOpt].self, from: Data(#"[{"label":"1 hour","value":"t9|1h"},{"label":"Tomorrow","value":"t9|tomorrow"},{"label":"1 week","value":"t9|1w"}]"#.utf8))) ?? []
check(opts.map(\.label) == ["1 hour", "Tomorrow", "1 week"] && opts.first?.value == "t9|1h", "the hub's spans", opts)
check(Home.span(opts, "tomorrow") == "t9|tomorrow" && Home.span(opts, "1y") == nil && Home.span([], "1h") == nil, "a span by its suffix")
check(((try? JSONDecoder().decode([DSnoozeOpt].self, from: Data("[{}]".utf8))) ?? []).first?.value == "", "missing fields are empty")

print("home-model: \(passed) passed, \(failed) failed")
exit(failed == 0 ? 0 : 1)
