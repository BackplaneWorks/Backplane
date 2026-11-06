import Foundation

// iOS's half of test/mobile/hub_cases.json, and HubHttp over real HTTP against test/mobile/hub_stub.py. Run by scripts/mobile/hub-check.sh (the same as Android's HubRescueTest).
let port = CommandLine.arguments[1], token = CommandLine.arguments[2], base = "http://127.0.0.1:\(port)"
var bad = 0, ran = 0
func same<T: Equatable>(_ name: String, _ got: T, _ want: T) { ran += 1; if got != want { print("FAIL \(name)\n  want \(want)\n  got  \(got)"); bad += 1 } }
func body(_ c: [String: Any]) -> Data {
    if let s = c["body"] as? String { return Data(s.utf8) }
    return try! JSONSerialization.data(withJSONObject: c["body"]!)
}
let cases = try JSONSerialization.jsonObject(with: Data(contentsOf: URL(fileURLWithPath: "hub_cases.json"))) as! [String: [[String: Any]]]

for c in cases["panel"]! {
    let n = c["name"] as! String, code = c["code"] as! Int
    let f = code == 0 ? HubFetch.unreachable : HubRescue.fetch(code: code, body: body(c))
    let p = HubRescue.panel(f)
    same(n, p.text, c["text"] as! String)
    same(n, p.actions.map { $0.id + ":" + $0.label }, c["actions"] as! [String])
    same(n, p.incidents.map(\.what), c["incidents"] as! [String])
    if let at = c["firstAtMs"] as? NSNumber { same(n, p.incidents[0].atMs, at.int64Value) }
    if let t = c["notice"] as? String, case .got(let s) = f { same(n, s.notice?.text, t) }
}
for c in cases["reply"]! {
    let code = c["code"] as! Int
    let r = code == 0 ? HubReply.unreachable : HubRescue.reply(code: code, body: body(c))
    same(c["name"] as! String, HubRescue.after(r), HubAfter(text: c["text"] as! String, poll: c["poll"] as! Bool))
}
for c in cases["step"]! {
    let code = c["code"] as! Int
    let h = code == 0 ? nil : HubRescue.health(body(c))
    same(c["name"] as! String, HubRescue.step(h, elapsedMs: (c["elapsed"] as! NSNumber).int64Value, moved: c["moved"] as! Bool),
         HubStep(text: c["text"] as! String, done: c["done"] as! Bool, moved: c["movedAfter"] as! Bool))
}
for c in cases["notice"]! {
    let n = HubRescue.notice(c["body"] as? [String: Any])
    same(c["name"] as! String, n.text, c["text"] as! String)
    same(c["name"] as! String, n.warn, c["warn"] as! Bool)
}
for h in [nil, HubHealth(status: "starting", safe: false), HubHealth(status: "hung", safe: false), HubHealth(status: "ok", safe: false), HubHealth(status: "down", safe: false)] {
    same("a wait never outlives the limit", HubRescue.step(h, elapsedMs: HubRescue.limitMs, moved: false).done, true)
}

// over real HTTP
func mode(_ m: String) async {
    var r = URLRequest(url: URL(string: base + "/__mode")!); r.httpMethod = "POST"; r.httpBody = try! JSONSerialization.data(withJSONObject: ["mode": m])
    _ = try! await URLSession.shared.data(for: r)
}
func seen() async -> [[String: Any]] {
    try! JSONSerialization.jsonObject(with: await URLSession.shared.data(from: URL(string: base + "/__seen")!).0) as! [[String: Any]]
}
let hub = HubHttp(base: base, token: token)

await mode("rescue")
same("rescue health", await hub.health(), HubHealth(status: "down", safe: false))
same("rescue status", HubRescue.panel(await hub.status()).text, "Hub is down: exit 101")
same("restart accepted", await hub.control("restart"), .accepted("restarting"))
var s = await seen()
same("routes", s.map { ($0["m"] as! String) + " " + ($0["path"] as! String) }, ["GET /healthz", "GET /hub/status", "POST /hub/control"])
same("healthz goes without the credential", s[0]["auth"] is NSNull, true)
same("status carries it", s[1]["auth"] as? String, "Bearer " + token)
same("control carries it", s[2]["auth"] as? String, "Bearer " + token)
same("the action", s[2]["body"] as? String, #"{"action":"restart"}"#)

await mode("up")
same("a refused rollback", await hub.control("rollback"), .refused("old has no remote control: going back to it from here would lock you out"))
await mode("limit")
same("the limit", HubRescue.after(await hub.control("restart")).poll, false)
await mode("noguard")
same("no guard", HubRescue.panel(await hub.status()).text, "Restart is not available from here.")
same("control's message", await hub.control("restart"), .refused("this hub runs without a guard: restart it at its machine"))

let wrong = HubHttp(base: base, token: "nope")
same("a wrong credential", await wrong.status(), .refused("this Backplane needs its pairing token"))

await mode("redirect")
same("a redirect is not followed", await hub.control("restart"), .unreachable)
same("nothing went on", (await seen()).count, 1)

await mode("hung")
let t = Date()
same("a hung hub", await HubHttp(base: base, token: nil, healthTimeout: 1, callTimeout: 2).health(), nil)
same("does not hold the request", Date().timeIntervalSince(t) < 4, true)
await mode("up")

let gone = HubHttp(base: "http://127.0.0.1:1", token: token, healthTimeout: 1, callTimeout: 1)
same("gone: status", await gone.status(), .unreachable)
same("gone: control", await gone.control("restart"), .unreachable)

print("\(ran) checks, \(bad) wrong"); exit(bad == 0 ? 0 : 1)
