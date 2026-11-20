// ReplyDraft (which messages offer Reply, the strip's line, the send's replyTo). Compiled with Backplane/ReplyDraft.swift alone, outside the app:
// scripts/mobile/ios-unit-tests.sh (CI's ios job).
import Foundation

var failed = 0, passed = 0
func check(_ ok: Bool, _ name: String, _ got: Any) { if ok { passed += 1 } else { failed += 1; print("FAIL \(name): got \(got)") } }

let a = ReplyDraft.of(id: "msg_A:0:aa", kind: "assistant", text: "\n  All done:  \n- merged")
check(a == DReplyRef(id: "msg_A:0:aa", text: "\n  All done:  \n- merged", role: "assistant"), "an answer offers Reply with its whole text", a as Any)
check(ReplyDraft.of(id: "m1", kind: "user", text: "mine")?.role == "user", "the person's own message too", "")
check(ReplyDraft.of(id: "t1", kind: "tool", text: "Read") == nil, "a tool call does not", "")
check(ReplyDraft.of(id: "m2", kind: "assistant", text: "  \n ") == nil, "nothing written: none", "")
check(ReplyDraft.of(id: "local:c1", kind: "user", text: "sending") == nil, "not yet on the hub: none", "")
check(ReplyDraft.of(id: "live:t1", kind: "assistant", text: "stream", live: true) == nil, "a reply still streaming: none", "")
check(ReplyDraft.firstLine(a!.text) == "All done:", "the strip's line is the first that is not blank", ReplyDraft.firstLine(a!.text))
check(ReplyDraft.firstLine(String(repeating: "é", count: 300)).count == 200, "200 characters at most", "")
check(ReplyDraft.who(a!) == "Agent" && ReplyDraft.who(DReplyRef(id: "m", text: "t", role: "user")) == "You", "the sender", "")
check(ReplyDraft.param(a!) == ["id": "msg_A:0:aa", "text": "\n  All done:  \n- merged"], "replyTo carries the id and the text as shown", ReplyDraft.param(a!))
check(ReplyDraft.chip(a!) == ("msg_A:0:aa", "All done:"), "the sent bubble's chip", ReplyDraft.chip(a!))
check(ReplyDraft.swipe(dx: 80, dy: 10), "a swipe right replies", "")
check(!ReplyDraft.swipe(dx: 40, dy: 0) && !ReplyDraft.swipe(dx: 80, dy: 60) && !ReplyDraft.swipe(dx: -90, dy: 0), "short, scrolling or leftward: no", "")
// a reply waits in the outbox like any message, with what it answers
let o = try! JSONDecoder().decode(DReplyRef.self, from: JSONEncoder().encode(a!)); check(o == a!, "it round-trips", o)

print("ReplyDraft: \(passed) passed, \(failed) failed")
exit(failed == 0 ? 0 : 1)
