import Foundation
import CryptoKit

// swiftc FileFetch.swift FileFetchRun.swift main.swift: the app's own download loop (FileFetchRun) against a live hub, through a relay that can cut every
// connection (scripts/mobile/tcp-relay.py). Env: BP_LIVE_URL (the big file's /file address through the relay), BP_LIVE_TOKEN, BP_LIVE_SHA256,
// BP_LIVE_CUT (the relay's control address), BP_LIVE_DENY (a /file address the hub refuses with 403). Rig: test/mobile/file-links-live.sh.
let env = ProcessInfo.processInfo.environment
func e(_ k: String) -> String { env[k] ?? "" }
var failed = 0
func check(_ ok: Bool, _ what: String) { if !ok { failed += 1; print("FAIL", what) } }
final class Log: @unchecked Sendable { let q = DispatchQueue(label: "log"); var v: [String] = []; func add(_ s: String) { q.sync { v.append(s) } }; var all: [String] { q.sync { v } } }
let log = Log()
FileFetchRun.heard = { r, c in log.add("\(r) \(c)") }
final class Cut: @unchecked Sendable { var done = false }
let cut = Cut()
func poke(_ u: String) { let s = DispatchSemaphore(value: 0); URLSession.shared.dataTask(with: URL(string: u)!) { _, _, _ in s.signal() }.resume(); s.wait() }
func sha(_ u: URL) -> String {
    let h = FileHandle(forReadingAtPath: u.path)!; var d = SHA256()
    while true { let b = h.readData(ofLength: 1 << 20); if b.isEmpty { break }; d.update(data: b) }
    return d.finalize().map { String(format: "%02x", $0) }.joined()
}
let dir = FileManager.default.temporaryDirectory.appendingPathComponent("bp-live-\(UUID().uuidString)")
try! FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
defer { try? FileManager.default.removeItem(at: dir) }
let sem = DispatchSemaphore(value: 0)
Task {
    let part = dir.appendingPathComponent("big.bin.part"), tag = dir.appendingPathComponent("etag")
    let out = await FileFetchRun.fetch(where: { FileFetchRun.Where(url: URL(string: e("BP_LIVE_URL"))!, token: e("BP_LIVE_TOKEN")) }, part: part, tagFile: tag, size: -1, offline: "offline", failed: "failed") { have, _ in
        // past 40 MB the relay drops the connection (and refuses new ones for a moment)
        if !cut.done && have > 40 << 20 { cut.done = true; poke(e("BP_LIVE_CUT") + "/cut"); DispatchQueue.global().asyncAfter(deadline: .now() + 1.5) { poke(e("BP_LIVE_CUT") + "/heal") } }
    }
    print("live requests:", log.all)
    check(out == .whole(part), "whole: \(out)")
    check(cut.done, "it was cut")
    check(log.all.first == "- 200", "the first ask is the whole file")
    check(log.all.dropFirst().contains { $0.hasPrefix("bytes=") && $0.hasSuffix(" 206") }, "a later ask went on with a 206")
    let got = sha(part); check(got == e("BP_LIVE_SHA256"), "sha256 \(got)")
    let n0 = log.all.count
    let deny = await FileFetchRun.fetch(where: { FileFetchRun.Where(url: URL(string: e("BP_LIVE_DENY"))!, token: e("BP_LIVE_TOKEN")) }, part: dir.appendingPathComponent("x.part"), tagFile: dir.appendingPathComponent("etag2"), size: -1, offline: "offline", failed: "failed")
    print("live refusal:", deny, Array(log.all.dropFirst(n0)))
    check(deny == .fail("that file is not shared"), "403 words: \(deny)")
    sem.signal()
}
sem.wait()
if failed == 0 { print("file fetch live: ok") }
exit(failed == 0 ? 0 : 1)
