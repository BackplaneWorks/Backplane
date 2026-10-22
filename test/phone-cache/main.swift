import Foundation

func require(_ yes: @autoclosure () -> Bool, _ message: String) {
    if !yes() { fputs("FAIL: \(message)\n", stderr); exit(1) }
}
let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
defer { try? FileManager.default.removeItem(at: root) }
let file = root.appendingPathComponent("screen.json")
let cache = ScreenCache(file: file)
let links = ["https://hub.test:3787/#token=abcdef"]
func reply(_ entries: [[String: Any]]) -> String {
    let screen: [String: Any] = ["hub": "hub.test:3787", "sel": "hub.test:3787|selected", "online": true,
      "projects": [], "bots": [], "rooms": [], "hubs": [],
      "thread": ["id": "selected", "draft": "private draft", "sending": ["pending send"], "entries": entries, "live": [["tag": "p", "kids": [["text": "current-stream-marker"]]]]]]
    return String(decoding: try! JSONSerialization.data(withJSONObject: ["screen": screen, "cmds": [["type": "notify"]]]), as: UTF8.self)
}
func entry(_ id: Int, _ body: String) -> [String: Any] {
    ["id": String(id), "kind": "assistant", "text": body, "blocks": [["tag": "p", "kids": [["text": body]]]]]
}
let large = reply((0..<160).map { entry($0, String(repeating: "older text ", count: 2000) + " marker-\($0)") })
let start = Date()
cache.save(large, links: links, generation: 1)
require(cache.size() > 0, "long-history screen persists instead of silent size miss")
require(cache.size() <= 512 * 1024, "presentation has bounded disk/decode size")
let equivalent = ["https://hub.test:3787/?token=abcdef#irrelevant"]
guard let data = cache.load(links: equivalent) else { require(false, "equivalent canonical hub credentials restore"); exit(1) }
let screen = try JSONSerialization.jsonObject(with: data) as! [String: Any]
let thread = screen["thread"] as! [String: Any]
let entries = thread["entries"] as! [[String: Any]]
require((entries.last?["text"] as? String)?.contains("marker-159") == true, "latest current text survives bound")
require(String(decoding: try! JSONSerialization.data(withJSONObject: thread["live"]!), as: UTF8.self).contains("current-stream-marker"), "in-progress reply survives process reopen")
require(thread["draft"] as? String == "", "cache never restores draft")
require((thread["sending"] as? [Any])?.isEmpty == true, "cache never revives pending sends")
require(screen["cmds"] == nil && screen["online"] as? Bool == false, "cache never restores effects or live status")
require(cache.load(links: ["https://hub.test:3787/#token=bbbbbb"]) == nil, "changed account credential rejected")
require(cache.load(links: links, profile: "other-install") == nil, "different profile rejected")
let restarted = ScreenCache(file: file)
require(restarted.load(links: links) != nil, "fresh process restores persisted screen without engine or network")
cache.clear(through: 3)
cache.save(reply([entry(1, "stale")]), links: links, generation: 2)
require(cache.size() == 0, "retired selection generation cannot write after clear")
cache.save(reply([entry(2, "newer")]), links: links, generation: 4)
require(cache.load(links: links) != nil, "new generation can persist")
// message text is never cut: a 40 KB reply comes back whole, from its first character
let long = "START-OF-LONG " + String(repeating: "forty kilobytes of reply text ", count: 1400) + " END-OF-LONG"
require(long.utf8.count > 40000, "the test message is over 40 KB")
let wholeCache = ScreenCache(file: root.appendingPathComponent("whole.json"))
wholeCache.save(reply([entry(1, "before"), entry(2, long)]), links: links, generation: 1)
guard let wd = wholeCache.load(links: links) else { require(false, "a 40 KB reply is cached"); exit(1) }
let wt = (try JSONSerialization.jsonObject(with: wd) as! [String: Any])["thread"] as! [String: Any]
let we = wt["entries"] as! [[String: Any]]
require(we.last?["text"] as? String == long, "a 40 KB reply is cached whole")
require(we.last?["partial"] == nil, "a whole reply is not marked partial")
require(String(decoding: try! JSONSerialization.data(withJSONObject: we.last!["blocks"]!), as: UTF8.self).contains("START-OF-LONG"), "its blocks start at its start")
// one entry over the whole limit is dropped whole, never cut; the others stay whole
let huge = "HUGE-START " + String(repeating: "x", count: 600 * 1024) + " HUGE-END"
let dropCache = ScreenCache(file: root.appendingPathComponent("drop.json"))
dropCache.save(reply([entry(1, huge), entry(2, "newest whole reply")]), links: links, generation: 1)
guard let dd = dropCache.load(links: links) else { require(false, "the screen is still cached when one entry is too big"); exit(1) }
let ds = String(decoding: dd, as: UTF8.self)
require(!ds.contains("HUGE-END") && !ds.contains("HUGE-START"), "an entry over the limit is dropped whole, no part of it kept")
require(ds.contains("newest whole reply"), "the newer entry stays whole")
// a streamed reply alone over the limit goes whole too
let bigLive: [String: Any] = ["hub": "hub.test:3787", "sel": "hub.test:3787|selected", "online": true, "projects": [], "hubs": [],
  "thread": ["id": "selected", "entries": [entry(1, "kept entry")], "live": [["tag": "p", "kids": [["text": "LIVE-START " + String(repeating: "y", count: 600 * 1024)]]]]]]
let liveCache = ScreenCache(file: root.appendingPathComponent("live.json"))
liveCache.save(String(decoding: try! JSONSerialization.data(withJSONObject: ["screen": bigLive]), as: UTF8.self), links: links, generation: 1)
guard let ld = liveCache.load(links: links) else { require(false, "the screen is cached when the live reply is too big"); exit(1) }
let ls = String(decoding: ld, as: UTF8.self)
require(!ls.contains("LIVE-START") && !ls.contains("yyyyyyyy"), "a live reply over the limit is dropped whole")
require(ls.contains("kept entry"), "the entries stay when the live reply goes")
print("PASS native persisted-screen helper; long-history bytes \(large.utf8.count), elapsed \(Date().timeIntervalSince(start)*1000) ms")
