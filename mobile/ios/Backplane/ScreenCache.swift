import Foundation
import CryptoKit

// Presentation only: independent of Bend's kept-state type key. Never used
// as replay state, an applied cursor, commands, pending sends or drafts.
final class ScreenCache: @unchecked Sendable {
    static let limit = 512 * 1024
    private let file: URL
    private let lock = NSLock()
    private var generation: UInt64 = 0
    private let report: @Sendable (String) -> Void

    init(file: URL? = nil, report: @escaping @Sendable (String) -> Void = { NSLog("Backplane screen-cache %@", $0) }) {
        let dir = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        self.file = file ?? dir.appendingPathComponent("recent-screen-v1.json")
        self.report = report
    }
    static func identity(_ links: [String], profile: String) -> String? {
        let normalized = links.compactMap { Pairing.socket($0)?.absoluteString }
        guard !links.isEmpty, normalized.count == links.count else { return nil }
        return SHA256.hash(data: Data(([profile] + normalized.sorted()).joined(separator: "\n").utf8))
            .map { String(format: "%02x", $0) }.joined()
    }
    // Only materialized activity display fields are neutralized. A cached
    // body, owner, route, blocked state or settled state keeps its identity.
    private static func offlineActivity(_ input: [String: Any]) -> [String: Any] {
        var screen = input
        func rows(_ input: [[String: Any]]) -> [[String: Any]] {
            input.map { row in
                var row = row
                let status = row["status"] as? String ?? ""
                if ["active", "running", "watching", "cancelling", "working", "monitoring"].contains(status)
                    || (status.isEmpty && row["state"] as? String == "run") { row["status"] = "inactive" }
                return row
            }
        }
        func plane(_ input: [String: Any]) -> [String: Any] {
            var plane = input
            plane["working"] = 0
            for key in ["need", "run", "idle", "bots", "routines", "locks"] {
                if let values = plane[key] as? [[String: Any]] {
                    plane[key] = values.map { row in
                        var row = row
                        let state = row["state"] as? String ?? ""
                        if ["active", "working", "work", "talk", "think"].contains(state) {
                            row["state"] = state == "active" ? "inactive" : "idle"
                        }
                        return row
                    }
                }
            }
            return plane
        }
        if var thread = screen["thread"] as? [String: Any] {
            let phase = thread["phase"] as? String ?? ""
            if ["active", "running", "watching", "cancelling", "working", "monitoring"].contains(phase)
                || (phase.isEmpty && thread["state"] as? String == "run") { thread["phase"] = "inactive" }
            thread["working"] = ""
            screen["thread"] = thread
        }
        if let projects = screen["projects"] as? [[String: Any]] {
            screen["projects"] = projects.map { project in
                var project = project
                for key in ["threads", "snoozed", "settled", "archived"] {
                    if let values = project[key] as? [[String: Any]] { project[key] = rows(values) }
                }
                return project
            }
        }
        if let active = screen["active"] as? [[String: Any]] { screen["active"] = rows(active) }
        if let value = screen["plane"] as? [String: Any] { screen["plane"] = plane(value) }
        if var bot = screen["bot"] as? [String: Any], let value = bot["plane"] as? [String: Any] {
            bot["plane"] = plane(value); screen["bot"] = bot
        }
        if var bot = screen["bot"] as? [String: Any], var mail = bot["mailroom"] as? [String: Any] {
            mail["online"] = false
            if var body = mail["body"] as? [String: Any] { body["draft"] = ""; body["sendState"] = ""; mail["body"] = body }
            bot["mailroom"] = mail; screen["bot"] = bot
        }
        if var uber = screen["uber"] as? [String: Any] { uber.removeValue(forKey: "counts"); screen["uber"] = uber }
        return screen
    }
    static func prepare(_ text: String, links: [String], profile: String = "") -> Data? {
        guard let identity = identity(links, profile: profile),
              let root = try? JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Any],
              var screen = root["screen"] as? [String: Any],
              let hub = screen["hub"] as? String, links.contains(where: { Pairing.key($0) == hub }) else { return nil }
        screen = offlineActivity(screen)
        screen["online"] = false
        // Dialogs and live activity state are never replayed from a picture.
        for key in ["rowMenu", "deleting", "removing", "renaming", "newBot", "newRoom", "tour", "folders", "find"] { screen.removeValue(forKey: key) }
        screen["making"] = false
        if var thread = screen["thread"] as? [String: Any] {
            thread["draft"] = ""; thread["sending"] = []; thread["attaching"] = []; thread["uploading"] = ""
            // Keep the current page bounded regardless of how many older pages
            // were opened. A message's text is never cut: one too big for the
            // cache is dropped whole below (the size limit), never shown in part.
            let all = thread["entries"] as? [[String: Any]] ?? []
            thread["earlier"] = (thread["earlier"] as? Int ?? 0) + max(0, all.count - 40)
            thread["entries"] = Array(all.suffix(40))
            for key in ["diff", "term", "embs", "asks", "skills", "parent", "subs"] { thread.removeValue(forKey: key) }
            screen["thread"] = thread
        }
        for key in ["hubs", "usageMachines"] {
            if let rows = screen[key] as? [[String: Any]] {
                screen[key] = rows.map { row in var r = row; r["online"] = false; return r }
            }
        }
        func encode() -> Data? { try? JSONSerialization.data(withJSONObject: ["version": 2, "identity": identity, "screen": screen]) }
        // Offscreen catalogs must not evict the selected conversation.
        var data = encode()
        if (data?.count ?? 0) > limit {
            for key in ["projects", "bots", "rooms", "active", "found", "usageMachines"] { screen[key] = [] }
            screen.removeValue(forKey: "plane")
            data = encode()
        }
        while (data?.count ?? 0) > limit, var t = screen["thread"] as? [String: Any],
              let entries = t["entries"] as? [[String: Any]], entries.count > 1 {
            let keep = max(1, entries.count / 2)
            t["entries"] = Array(entries.suffix(keep))
            t["earlier"] = (t["earlier"] as? Int ?? 0) + entries.count - keep
            screen["thread"] = t
            data = encode()
        }
        // still too big: the streamed reply, then the one entry left, go whole
        if (data?.count ?? 0) > limit, var t = screen["thread"] as? [String: Any], !((t["live"] as? [Any]) ?? []).isEmpty {
            t["live"] = []; t.removeValue(forKey: "livePartial"); screen["thread"] = t
            data = encode()
        }
        if (data?.count ?? 0) > limit, var t = screen["thread"] as? [String: Any], let entries = t["entries"] as? [[String: Any]], !entries.isEmpty {
            t["entries"] = []; t["earlier"] = (t["earlier"] as? Int ?? 0) + entries.count; screen["thread"] = t
            data = encode()
        }
        guard let data, data.count <= limit else { return nil }
        return data
    }
    func save(_ text: String, links: [String], profile: String = "", generation: UInt64) {
        guard let data = Self.prepare(text, links: links, profile: profile) else { report("save_shape_or_size"); return }
        lock.withLock {
            guard generation > self.generation else { return }
            do {
                #if os(iOS)
                try data.write(to: file, options: [.atomic, .completeFileProtectionUntilFirstUserAuthentication])
                #else
                try data.write(to: file, options: .atomic)
                #endif
                self.generation = generation
            } catch { report("save_io") }
        }
    }
    func load(links: [String], profile: String = "") -> Data? {
        lock.withLock {
            guard FileManager.default.fileExists(atPath: file.path) else { report("miss_absent"); return nil }
            guard let size = (try? FileManager.default.attributesOfItem(atPath: file.path)[.size]) as? Int,
                  size > 0, size <= Self.limit else { report("miss_size"); return nil }
            guard let data = try? Data(contentsOf: file) else { report("miss_io"); return nil }
            guard let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { report("miss_decode"); return nil }
            guard object["version"] as? Int == 2 else { report("miss_schema"); return nil }
            guard let identity = Self.identity(links, profile: profile), object["identity"] as? String == identity else { report("miss_identity"); return nil }
            guard let screen = object["screen"] as? [String: Any], let hub = screen["hub"] as? String,
                  links.contains(where: { Pairing.key($0) == hub }),
                  let out = try? JSONSerialization.data(withJSONObject: Self.offlineActivity(screen)) else { report("miss_shape"); return nil }
            report("hit")
            return out
        }
    }
    func clear(through generation: UInt64) {
        lock.withLock {
            self.generation = max(self.generation, generation)
            try? FileManager.default.removeItem(at: file)
        }
    }
    func size() -> Int { lock.withLock { ((try? FileManager.default.attributesOfItem(atPath: file.path)[.size]) as? Int) ?? 0 } }
}
