// Appended to the exact Worker definition extracted from Engine.swift.
// Copied state only: returned commands are hashed and never executed.
import CryptoKit
import Darwin

setbuf(stdout, nil)
setbuf(stderr, nil)
let args = CommandLine.arguments
precondition(args.count == 3, "BRIDGE COPIED_STATE")
private let worker = Worker()
final class ProfileState: @unchecked Sendable {
    var context: JSContext?
    var bridge: JSValue?
    var result: [String: Any] = [:]
}
let state = ProfileState()
let done = DispatchSemaphore(value: 0)
DispatchQueue.global().asyncAfter(deadline: .now() + 45) {
    fputs("TIMEOUT 45s total Worker lifetime diagnostic\n", stderr)
    exit(124)
}
worker.async(person: true) {
    let c = JSContext()!
    c.exceptionHandler = { _, e in
        fputs("JS_EXCEPTION \(e?.toString() ?? "unknown")\n", stderr)
        exit(1)
    }
    c.evaluateScript("Date.now = () => 1792389000000;")
    c.evaluateScript(try! String(contentsOfFile: args[1], encoding: .utf8))
    state.context = c
    state.bridge = c.objectForKeyedSubscript("Backplane")!
    done.signal()
}
func wait() {
    if done.wait(timeout: .now() + 45) == .timedOut {
        fputs("TIMEOUT actual Worker task; process terminates all owned threads\n", stderr)
        exit(124)
    }
}
wait()
@discardableResult func call(_ name: String, _ arguments: [Any]) -> [String: Any] {
    worker.async(person: true) {
        let began = DispatchTime.now().uptimeNanoseconds
        let text = state.bridge!.invokeMethod(name, withArguments: arguments)!.toString()!
        let digest = SHA256.hash(data: Data(text.utf8)).map { String(format: "%02x", $0) }.joined()
        print("END Backplane.\(name) ms=\(Double(DispatchTime.now().uptimeNanoseconds-began)/1e6) bytes=\(text.utf8.count) sha=\(digest)")
        state.result = (try? JSONSerialization.jsonObject(with: Data(text.utf8))) as? [String: Any] ?? [:]
        done.signal()
    }
    wait()
    return state.result
}
call("start", ["isolated-jsc-diagnostic", "{}"])
let loaded = call("load", [try String(contentsOfFile: args[2], encoding: .utf8)])
let screen = loaded["screen"] as! [String: Any]
let thread = screen["thread"] as! [String: Any]
let id = thread["id"] as! String
call("online", [String(id.split(separator: "|", maxSplits: 1)[0]), true, false])
call("tick", [Int(Date().timeIntervalSince1970)])
for i in 0..<30 {
    call("reading", [id, thread["readingSeen"] as? String ?? "", (thread["entries"] as? [Any])?.count ?? 0, true])
    call("tick", [1792389001])
    call("flush", [true])
    var usage = rusage()
    getrusage(RUSAGE_SELF, &usage)
    print("LIFETIME iteration=\(i) actual_worker=true peak_bytes=\(usage.ru_maxrss)")
}
print("PASS actual production Worker lifetime diagnostic; no RPC execution")
