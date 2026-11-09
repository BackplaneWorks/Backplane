// BrowserInput against test/mobile/browser_input_cases.json (the table Android's BrowserInputTest reads). Compiled with Backplane/BrowserInput.swift alone:
// scripts/mobile/ios-unit-tests.sh. The table's path is the first argument.
import Foundation

var failed = 0, passed = 0
func check(_ ok: Bool, _ name: String, _ got: Any) { if ok { passed += 1 } else { failed += 1; print("FAIL \(name): got \(got)") } }
let path = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "browser_input_cases.json"
guard let data = FileManager.default.contents(atPath: path), let t = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else { print("no table at \(path)"); exit(2) }
func cases(_ k: String) -> [[String: Any]] { let a = t[k] as? [[String: Any]] ?? []; check(!a.isEmpty, "\(k) has cases", a.count); return a }
for c in cases("keys") {
    let text = c["text"] as! String, g = BrowserInput.keys(c["old"] as? String ?? BrowserInput.sentinel, text).map { [$0.0, $0.1] }
    check(g == (c["want"] as! [[String]]), c["name"] as! String, g)
    check(BrowserInput.next(text) == c["next"] as! String, (c["name"] as! String) + " (next)", BrowserInput.next(text).unicodeScalars.count)
}
for c in cases("scroll") {
    let n = { (k: String) in (c[k] as! NSNumber).doubleValue }
    let g = BrowserInput.scroll(n("dx"), n("dy"), image: n("image"), shown: n("shown")); let w = c["want"] as! [Int]
    check(g.0 == w[0] && g.1 == w[1], c["name"] as! String, g)
}
print("\(passed) passed, \(failed) failed")
exit(failed == 0 ? 0 : 1)
