// GoogleLoopLogic (Backplane/GoogleLoopLogic.swift; the same table as Android's GoogleLoopTest). scripts/mobile/ios-unit-tests.sh
import Foundation

var failed = 0, passed = 0
func check(_ ok: Bool, _ name: String, _ got: Any = "") { if ok { passed += 1 } else { failed += 1; print("FAIL \(name): got \(got)") } }
let r = GoogleLoopLogic.redirect(41234)
let enc = "http%3A%2F%2F127.0.0.1%3A41234%2Foauth%2Fgoogle"
check(r == "http://127.0.0.1:41234/oauth/google", "redirect", r)
check(GoogleLoopLogic.matches("https://accounts.google.com/o/oauth2/v2/auth?client_id=x&redirect_uri=\(enc)&state=s", r), "match")
check(GoogleLoopLogic.matches("https://accounts.google.com/a?redirect_uri=\(enc)", r), "match last")
check(!GoogleLoopLogic.matches("https://accounts.google.com/a?redirect_uri=http%3A%2F%2F127.0.0.1%3A41235%2Foauth%2Fgoogle", r), "other port")
check(!GoogleLoopLogic.matches("https://accounts.google.com/a?redirect_uri=\(enc)%2Fx", r), "longer path")
check(!GoogleLoopLogic.matches("https://accounts.google.com/a?x=redirect_uri%3D\(enc)", r), "nested")
check(!GoogleLoopLogic.matches("https://accounts.google.com/a", r), "none")
check(!GoogleLoopLogic.matches("", r), "empty")
check(GoogleLoopLogic.requestPath("GET /oauth/google?code=4%2Fabc&state=s HTTP/1.1\r\nHost: x\r\n") == "/oauth/google?code=4%2Fabc&state=s", "get")
check(GoogleLoopLogic.requestPath("POST /oauth/google?x=1 HTTP/1.1") == nil, "post")
check(GoogleLoopLogic.requestPath("") == nil, "empty line")
check(GoogleLoopLogic.requestPath("GET") == nil, "short")
check(GoogleLoopLogic.isCatch("/oauth/google?code=x&state=y"), "catch")
check(!GoogleLoopLogic.isCatch("/oauth/google"), "no query")
check(!GoogleLoopLogic.isCatch("/oauth/google?"), "empty query")
check(!GoogleLoopLogic.isCatch("/favicon.ico"), "favicon")
check(!GoogleLoopLogic.isCatch("/x/oauth/google?code=1"), "prefix")
check(!GoogleLoopLogic.isCatch(nil), "nil")
check(GoogleLoopLogic.wants("google:new", loopback: false), "google id")
check(GoogleLoopLogic.wants("x:1", loopback: true), "flag")
check(!GoogleLoopLogic.wants("claude:work", loopback: false), "claude")
print("\(passed) passed, \(failed) failed")
exit(failed == 0 ? 0 : 1)
