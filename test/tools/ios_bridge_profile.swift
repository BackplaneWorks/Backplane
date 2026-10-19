import Foundation
import JavaScriptCore

// Run against a copied kept state, never a live hub. App actions are
// evaluated but their send commands are retained as evidence, not executed.
setbuf(stdout, nil)
let args = CommandLine.arguments
precondition(args.count == 3, "BRIDGE STATE")
let c = JSContext()!
c.exceptionHandler = { _, e in fputs("JSC ERROR \(e?.toString() ?? "")\n", stderr) }
c.evaluateScript(try String(contentsOfFile: args[1], encoding: .utf8))
let b = c.objectForKeyedSubscript("Backplane")!
var screen: [String: Any] = [:]
@discardableResult func call(_ name: String, _ args: [Any]) -> [String: Any] {
    let t = DispatchTime.now().uptimeNanoseconds
    let text = b.invokeMethod(name, withArguments: args)?.toString() ?? ""
    let j = (try? JSONSerialization.jsonObject(with: Data(text.utf8))) as? [String: Any] ?? [:]
    if let s = j["screen"] as? [String: Any] { screen = s }
    print("PROFILE \(name) ms=\(Double(DispatchTime.now().uptimeNanoseconds-t)/1e6) bytes=\(text.utf8.count) entries=\((screen["thread"] as? [String: Any])?["entries"] as? [Any] != nil ? ((screen["thread"] as! [String: Any])["entries"] as! [Any]).count : 0)")
    return j
}
call("start", ["ios-profile", "{}"])
call("load", [try String(contentsOfFile: args[2], encoding: .utf8)])
let uber = screen["uber"] as? [String: Any]
let bot = ((screen["bots"] as? [[String: Any]]) ?? []).first { ($0["name"] as? String ?? "").contains("Bub") }
guard let row = uber ?? bot, let id = row["id"] as? String else { fatalError("no Bub/Uberbot fixture") }
let action = uber == nil ? (row["act"] as? String ?? "bot") : "uber"
for _ in 0..<3 {
    call("act", ["select", ""])
    call("act", [action, id])
    call("screen", [])
    call("quiet", ["draft", "ios copied-state delivery probe"])
    let out = call("act", ["send", ""])
    print("PROFILE send commands=\((out["cmds"] as? [Any])?.count ?? 0)")
}
