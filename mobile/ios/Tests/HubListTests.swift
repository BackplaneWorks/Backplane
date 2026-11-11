// HubList against test/mobile/hubs_cases.json (the table Android's HubListTest reads). Compiled with Backplane/HubList.swift alone, outside the app:
// scripts/mobile/ios-hub-list-test.sh. The table's path is the first argument.
import Foundation

var failed = 0, passed = 0
func check(_ ok: Bool, _ name: String, _ got: Any) { if ok { passed += 1 } else { failed += 1; print("FAIL \(name): got \(got)") } }
let path = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "hubs_cases.json"
guard let data = FileManager.default.contents(atPath: path), let t = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else { print("no table at \(path)"); exit(2) }
func cases(_ k: String) -> [[String: Any]] { let a = t[k] as? [[String: Any]] ?? []; check(!a.isEmpty, "\(k) has cases", a.count); return a }
func strs(_ v: Any?) -> [String] { v as? [String] ?? [] }
let key: (String) -> String? = { HubList.authority($0) }

for c in cases("adopt") { let g = HubList.adopt(strs(c["links"]), c["link"] as! String, key: key); check(g == strs(c["want"]), c["name"] as! String, g) }
for c in cases("added") { let g = HubList.added(strs(c["links"]), c["link"] as! String, key: key); check(g == strs(c["want"]), c["name"] as! String, g) }
for c in cases("use") { let g = HubList.use(strs(c["links"]), c["key"] as! String, key: key); check(g == strs(c["want"]), c["name"] as! String, g) }
for c in cases("forget") { let g = HubList.forget(strs(c["links"]), c["key"] as! String, key: key); check(g == strs(c["want"]), c["name"] as! String, g) }
for c in cases("routes") {
    let g = HubList.routes(c["text"] as! String)
    check(g == strs(c["want"]) && HubList.local(g) == (c["local"] as! Bool), c["name"] as! String, (g, HubList.local(g)))
}
for c in cases("offer") {
    let w = c["want"] as! [String: String]
    let want: HubList.Plan = switch w["plan"]! { case "known": .known(w["key"]!); case "same": .same(key: w["key"]!, link: w["link"]!); case "take": .take(w["key"]!); default: .none }
    let tok = c["token"] as! String
    let g = HubList.plan(c["offer"] as! String, links: strs(c["links"]), token: tok.isEmpty ? nil : tok, done: Set(strs(c["done"])), key: key)
    check(g == want, c["name"] as! String, g)
}
for c in cases("failover") {
    let g = HubList.onHome(main: c["main"] as! String, home: c["home"] as! String, now: c["onHome"] as! Bool, down: (c["down"] as! NSNumber).doubleValue, homeUp: c["homeUp"] as! Bool, mainUp: c["mainUp"] as! Bool)
    check(g == (c["want"] as! Bool), c["name"] as! String, g)
}
print("\(passed) passed, \(failed) failed")
exit(failed == 0 ? 0 : 1)
