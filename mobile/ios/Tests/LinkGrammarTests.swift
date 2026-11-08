import Foundation

// swiftc DataLinkGrammar.swift main.swift (scripts/mobile/ios-link-grammar-test.sh)
var failed = 0
func check(_ ok: Bool, _ what: String) { if !ok { failed += 1; print("FAIL", what) } }
func kinds(_ t: String) -> [String] {
    dLinkRuns(t).compactMap { run in run.url.map { u in (dPathFrom(u) != nil ? "path:" + dPathFrom(u)! : u.absoluteString) + "|" + run.text } }
}

check(kinds("see src/x.rs:12 now") == ["path:src/x.rs:12|src/x.rs:12"], "path with line \(kinds("see src/x.rs:12 now"))")
check(kinds("in (src/x.rs).") == ["path:src/x.rs|src/x.rs"], "path in parens")
check(kinds("/home/h/proj/src/a.kt:3:4,") == ["path:/home/h/proj/src/a.kt:3:4|/home/h/proj/src/a.kt:3:4"], "absolute path with line and column")
check(kinds("see README.md.") == ["path:README.md|README.md"], "known file")
check(kinds("src/x.rs#L40-L50") == ["path:src/x.rs#L40-L50|src/x.rs#L40-L50"], "line anchor")
check(kinds("and/or TCP/IP 1/2 e.g. i.e. v1.2.3 a.b") == [], "no prose \(kinds("and/or TCP/IP 1/2 e.g. i.e. v1.2.3 a.b"))")
check(kinds("snake_case_word and foo/bar") == [], "no word")
check(kinds("mail a.b@example.com, ok") == ["mailto:a.b@example.com|a.b@example.com"], "email")
check(kinds("x me@x") == [], "not an email")
check(kinds("call +1 555 123 4567 now") == ["tel:+15551234567|+1 555 123 4567"], "intl phone \(kinds("call +1 555 123 4567 now"))")
check(kinds("call (555) 123-4567.") == ["tel:5551234567|(555) 123-4567"], "nanp phone \(kinds("call (555) 123-4567."))")
check(kinds("ids 1700000000-1 and 20261107") == [], "no number soup")
check(kinds("visit https://example.com/a_(b)). ok") == ["https://example.com/a_(b)|https://example.com/a_(b)"], "web \(kinds("visit https://example.com/a_(b)). ok"))")
check(kinds("(backplane://thread/lab~t9)") == ["backplane://thread/lab~t9|backplane://thread/lab~t9"], "backplane link")
check(kinds("backplane://conv/room%3Aabc") == ["backplane://conv/room%3Aabc|backplane://conv/room%3Aabc"], "conv link")
check(dPathFrom(dPathURL("a b/c.rs:5")!) == "a b/c.rs:5", "path url round trip")
if failed == 0 { print("link grammar: ok") }
exit(failed == 0 ? 0 : 1)
