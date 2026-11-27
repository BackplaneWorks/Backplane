// SolidCraftWebPolicy & SolidCraftBundleSupport unit tests.
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

// Bundle manifest & local server support checks
let manifestJSON = """
{"version":"0.50.0","js":"solidcraft-web-abc.js","wasm":"solidcraft-web-abc_bg.wasm","css":"loading-111.css","wasmSize":216659986,"jsSize":198390}
"""
let manifest = SolidCraftBundleSupport.parseManifest(Data(manifestJSON.utf8))
check(manifest != nil, "valid manifest parses")
check(manifest?.wasmSize == 216659986, "manifest preserves wasmSize")

let badManifest = "{\"version\":\"0.50.0\",\"js\":\"../evil.js\",\"wasm\":\"x.wasm\",\"css\":\"\",\"wasmSize\":1,\"jsSize\":1}"
check(SolidCraftBundleSupport.parseManifest(Data(badManifest.utf8)) == nil, "traversal in manifest rejected")

check(SolidCraftBundleSupport.isSafeRelativePath("fonts/BIZUDPGothic-Regular.ttf"), "nested font path allowed")
check(!SolidCraftBundleSupport.isSafeRelativePath("../secret"), "parent traversal blocked")
check(!SolidCraftBundleSupport.isSafeRelativePath("sw.js"), "upstream sw.js blocked")
check(!SolidCraftBundleSupport.isSafeRelativePath(".hidden"), "hidden path blocked")

check(SolidCraftBundleSupport.acceptsGzip("gzip, deflate, br"), "gzip accepted")
check(!SolidCraftBundleSupport.acceptsGzip("br, gzip;q=0"), "gzip;q=0 rejected")

if let m = manifest {
    let html = SolidCraftBundleSupport.renderIndexHTML(m)
    let boot = SolidCraftBundleSupport.renderBootJS(m)
    check(html.contains("./solidcraft-web-abc.js") && html.contains("./bp-boot.js") && !html.contains("solidcraft.io"), "index html self-contained")
    check(boot.contains("__bpSolidCraftBootStarted = true") && boot.contains("bp:solidcraft:stage"), "boot js has watchdog flag and stage reporting")
    check(boot.contains("createSyncAccessHandle") && boot.contains("X-Uncompressed-Length"), "boot js has OPFS worker fallback and progress header")
    check(boot.contains("patchWritableProto") && boot.contains("byteOffset"), "boot js patches WebKit FileSystemWritableFileStream slice bug")
    check(boot.contains("./files/get") && boot.contains("./save"), "boot js uses relative proxy endpoints")
}

check(SolidCraftBundleSupport.progressPercent(done: 50, total: 200) == 25, "progress percent computes 25")
check(SolidCraftBundleSupport.progressPercent(done: 300, total: 200) == 100, "progress percent clamps to 100")
check(SolidCraftBundleSupport.progressPercent(done: 10, total: 0) == nil, "zero total gives nil")

let built = SolidCraftBundleSupport.buildHubURL(hubBase: URL(string: "https://desk.tail.ts.net:8451")!, route: "/files/get", rawQuery: "thread=t1&path=mech%2Fbody.sc")
check(built?.absoluteString == "https://desk.tail.ts.net:8451/files/get?thread=t1&path=mech%2Fbody.sc", "hub proxy URL built cleanly", built?.absoluteString as Any)

print("SolidCraftWebPolicyTests: \(passed) passed, \(failed) failed")
if failed > 0 { exit(1) }
