// SolidCraftWebPolicy (Backplane/SolidCraftWebPolicy.swift) unit tests.
// Compiled with SolidCraftWebPolicy.swift alone: scripts/mobile/ios-unit-tests.sh.
import Foundation

var failed = 0, passed = 0
func check(_ ok: Bool, _ name: String, _ got: Any = "") {
    if ok { passed += 1 } else { failed += 1; print("FAIL \(name): got \(got)") }
}

let httpsBase = URL(string: "https://desktop.tail.ts.net:8451/solidcraft-web/?thread=t1")!
let httpsDefault = URL(string: "https://desktop.tail.ts.net/solidcraft-web/")!
let httpsExplicit443 = URL(string: "https://desktop.tail.ts.net:443/solidcraft-web/bp-boot.js")!
let httpDefault = URL(string: "http://100.64.84.41/solidcraft-web/")!
let httpExplicit80 = URL(string: "http://100.64.84.41:80/solidcraft-web/bp-boot.js")!

check(SolidCraftWebPolicy.normalizedPort(httpsBase) == 8451, "explicit port preserved")
check(SolidCraftWebPolicy.normalizedPort(httpsDefault) == 443, "https default port normalized to 443")
check(SolidCraftWebPolicy.normalizedPort(httpDefault) == 80, "http default port normalized to 80")

check(SolidCraftWebPolicy.isSameOrigin(URL(string: "https://DESKTOP.TAIL.TS.NET:8451/files/get")!, httpsBase), "same-origin matches case-insensitively")
check(SolidCraftWebPolicy.isSameOrigin(httpsExplicit443, httpsDefault), "https :443 matches default port")
check(SolidCraftWebPolicy.isSameOrigin(httpExplicit80, httpDefault), "http :80 matches default port")
check(!SolidCraftWebPolicy.isSameOrigin(URL(string: "https://desktop.tail.ts.net:8452/")!, httpsBase), "different port rejected")
check(!SolidCraftWebPolicy.isSameOrigin(URL(string: "http://desktop.tail.ts.net:8451/")!, httpsBase), "different scheme rejected")
check(!SolidCraftWebPolicy.isSameOrigin(URL(string: "https://evil.example:8451/")!, httpsBase), "different host rejected")

let req = SolidCraftWebPolicy.initialRequest(url: httpsBase, token: "abc123")
check(req.value(forHTTPHeaderField: "Authorization") == "Bearer abc123", "initialRequest sets Bearer header")
let noTokReq = SolidCraftWebPolicy.initialRequest(url: httpsBase, token: "")
check(noTokReq.value(forHTTPHeaderField: "Authorization") == nil, "empty token omits Authorization header")

let httpsProps = SolidCraftWebPolicy.cookieProperties(url: httpsBase, token: "tok1")
let httpsCookie = HTTPCookie(properties: httpsProps)
check(httpsCookie != nil, "https cookie created")
check(httpsCookie?.isSecure == true, "https cookie is Secure")
check(httpsCookie?.isHTTPOnly == true, "https cookie is HttpOnly")
check(httpsCookie?.value == "tok1", "https cookie holds token")

let httpProps = SolidCraftWebPolicy.cookieProperties(url: URL(string: "http://100.64.84.41:3797/solidcraft-web/")!, token: "tok2")
let httpCookie = HTTPCookie(properties: httpProps)
check(httpCookie != nil, "http cookie created")
check(httpCookie?.isSecure == false, "http cookie omits Secure")
check(httpCookie?.isHTTPOnly == true, "http cookie is HttpOnly")

let clearProps = SolidCraftWebPolicy.cookieProperties(url: httpsBase, token: "tok1", clear: true)
let clearCookie = HTTPCookie(properties: clearProps)
check(clearCookie?.value == "", "clear cookie empties value")
check((clearCookie?.expiresDate?.timeIntervalSince1970 ?? 1) == 0, "clear cookie expires immediately")

print("SolidCraftWebPolicyTests: \(passed) passed, \(failed) failed")
if failed > 0 { exit(1) }
