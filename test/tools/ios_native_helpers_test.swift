import Foundation

let wire = WirePriority()
wire.install("{\"keys\":[\"t\",\"id\",\"items\"],\"words\":[\"mobile\",\"reply\",\"log\",\"changes\"]}")
func text(_ s: String) -> [UInt8] { let b = Array(s.utf8); precondition(b.count < 24); return [0x60 + UInt8(b.count)] + b }
func priority(_ bytes: [UInt8]) -> Bool { wire.person(Data(bytes)) }
precondition(priority([0xa1, 0, 0xc6, 0]), "dictionary greeting")
precondition(priority([0xa1] + text("t") + text("mobile")), "literal greeting")
precondition(priority([0xa2, 0, 0xc6, 1, 1, 0xc7] + text("4000000000001")), "reserved RPC reply")
precondition(!priority([0xa2, 0, 0xc6, 1, 1, 24, 42]), "ordinary replies remain FIFO")
precondition(!priority([0xa2, 0, 0xc6, 2, 2, 0x81] + text("mobile")), "history contents never grant priority")
precondition(!priority([0xa2, 0, 0xc6, 3, 2, 0x81] + text("4000000000001")), "changes remain FIFO")
precondition(!priority([]) && !priority([0xa2, 0]) && !priority([0xff]), "malformed prefix")
precondition(!priority([0xa1, 1, 0xc7] + text("-4000000000001")), "negative id")
precondition(!priority([0xa1, 1, 0xc7] + text("4000000000000junk")), "whole numeric id required")
let unknown = WirePriority()
precondition(!unknown.person(Data([0xa1, 0, 0xc6, 0])), "no dictionary assumptions before initialization")

let directory = FileManager.default.temporaryDirectory.appendingPathComponent("bp-ios-screen-cache-" + UUID().uuidString)
try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
defer { try? FileManager.default.removeItem(at: directory) }
let file = directory.appendingPathComponent("screen.json"), cache = ScreenCache(file: directory.appendingPathComponent("screen.json"))
let links = ["http://desk/#token=abc", "http://lap/#token=def"]
func screen(_ title: String, draft: String) -> String {
    "{\"screen\":{\"online\":true,\"thread\":{\"title\":\"\(title)\",\"draft\":\"\(draft)\",\"sending\":[\"already sent\"]},\"hubs\":[{\"key\":\"desk\",\"online\":true}],\"usageMachines\":[{\"key\":\"lap\",\"online\":true}]},\"cmds\":[{\"type\":\"send\",\"data\":\"never replay\"}]}"
}
func loaded() throws -> [String: Any] { try JSONSerialization.jsonObject(with: cache.load(links: links)!) as! [String: Any] }
cache.save(screen("Recent", draft: "sent text"), links: links, generation: 1)
let restored = try loaded(), thread = restored["thread"] as! [String: Any]
precondition(thread["draft"] as? String == "", "cached presentation never restores sent draft")
precondition((thread["sending"] as? [String])?.isEmpty == true)
precondition(restored["cmds"] == nil, "commands never cached")
precondition(restored["online"] as? Bool == false)
precondition((restored["usageMachines"] as? [[String: Any]])?.first?["online"] as? Bool == false)
precondition(cache.load(links: Array(links.reversed())) != nil)
precondition(cache.load(links: ["http://desk/#token=different"]) == nil, "pairing identity isolation")
cache.save(screen("Newer", draft: ""), links: links, generation: 3)
cache.save(screen("Old", draft: ""), links: links, generation: 2)
let newer = try loaded()
precondition((newer["thread"] as? [String: Any])?["title"] as? String == "Newer", "late writer cannot regress cache")
cache.clear(through: 4)
cache.save(screen("Old", draft: ""), links: links, generation: 3)
precondition(cache.load(links: links) == nil, "clear blocks pending old writers")
cache.save(screen("Next", draft: ""), links: links, generation: 5)
precondition(cache.load(links: links) != nil)
try Data("invalid".utf8).write(to: file)
precondition(cache.load(links: links) == nil)
print("PASS native wire priority and screen cache: FIFO history, reserved replies, malformed input, no draft/command restore, identity, ordered writes and clear")
