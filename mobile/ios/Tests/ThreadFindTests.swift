// ThreadFind (the find bar's stepping and the snippets' bold words) against hand-made cases. Compiled with Backplane/ThreadFind.swift alone,
// outside the app: scripts/mobile/ios-unit-tests.sh (CI's ios job).
import Foundation

var failed = 0, passed = 0
func check(_ ok: Bool, _ name: String, _ got: Any) { if ok { passed += 1 } else { failed += 1; print("FAIL \(name): got \(got)") } }

// an answer: the newest match (the last, matches are oldest first); nothing matched: none
check(ThreadFind.first(count: 5) == 4, "first is the newest", ThreadFind.first(count: 5) as Any)
check(ThreadFind.first(count: 0) == nil, "no first when nothing matched", ThreadFind.first(count: 0) as Any)

// previous (-1, older) and next (+1, newer), wrapping at both ends
check(ThreadFind.step(4, count: 5, by: -1) == 3, "previous", ThreadFind.step(4, count: 5, by: -1) as Any)
check(ThreadFind.step(0, count: 5, by: -1) == 4, "previous wraps to the newest", ThreadFind.step(0, count: 5, by: -1) as Any)
check(ThreadFind.step(4, count: 5, by: 1) == 0, "next wraps to the oldest", ThreadFind.step(4, count: 5, by: 1) as Any)
check(ThreadFind.step(1, count: 5, by: 1) == 2, "next", ThreadFind.step(1, count: 5, by: 1) as Any)
check(ThreadFind.step(0, count: 1, by: 1) == 0 && ThreadFind.step(0, count: 1, by: -1) == 0, "one match stays", "")
check(ThreadFind.step(nil, count: 3, by: 1) == 2, "no current: the newest", ThreadFind.step(nil, count: 3, by: 1) as Any)
check(ThreadFind.step(7, count: 3, by: -1) == 2, "a current past fewer results: the newest", ThreadFind.step(7, count: 3, by: -1) as Any)
check(ThreadFind.step(0, count: 0, by: 1) == nil, "nothing to step through", ThreadFind.step(0, count: 0, by: 1) as Any)

// the label
check(ThreadFind.label(4, total: 5) == "5 of 5", "label newest", ThreadFind.label(4, total: 5))
check(ThreadFind.label(0, total: 5) == "1 of 5", "label oldest", ThreadFind.label(0, total: 5))
check(ThreadFind.label(nil, total: 0) == "0 of 0", "label nothing", ThreadFind.label(nil, total: 0))
check(ThreadFind.label(nil, total: nil) == "", "label before an answer", ThreadFind.label(nil, total: nil))
check(ThreadFind.label(999, total: 1000, more: true) == "1000 of 1000+", "label capped", ThreadFind.label(999, total: 1000, more: true))

// asks: two characters or more
check(!ThreadFind.asks("q") && !ThreadFind.asks(" q ") && ThreadFind.asks("qu"), "asks", "")

// spans: the query's words in bold, any case, every place, several words
func bold(_ s: [(String, Bool)]) -> String { s.map { $0.1 ? "[\($0.0)]" : $0.0 }.joined() }
check(bold(ThreadFind.spans("the Quasar and the quasar", "quasar")) == "the [Quasar] and the [quasar]", "bold every place, any case", bold(ThreadFind.spans("the Quasar and the quasar", "quasar")))
check(bold(ThreadFind.spans("red quasars glow", "QUA glow")) == "red [qua]sars [glow]", "bold inside a word, two words", bold(ThreadFind.spans("red quasars glow", "QUA glow")))
check(bold(ThreadFind.spans("nothing here", "zebra")) == "nothing here", "no word: plain", bold(ThreadFind.spans("nothing here", "zebra")))
check(bold(ThreadFind.spans("…Café crème", "café")) == "…[Café] crème", "accents", bold(ThreadFind.spans("…Café crème", "café")))
check(ThreadFind.spans("", "x").isEmpty && bold(ThreadFind.spans("abc", "  ")) == "abc", "empty text or query", "")

print("thread-find: \(passed) passed, \(failed) failed")
exit(failed == 0 ? 0 : 1)
