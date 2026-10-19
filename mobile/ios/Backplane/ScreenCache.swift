import Foundation
import CryptoKit

// A native presentation cache survives a Bend type-key change. It never
// supplies replay state, commands, a cursor or a composer draft.
final class ScreenCache: @unchecked Sendable {
    private let file: URL
    private let lock = NSLock()
    private var generation: UInt64 = 0

    init(file: URL? = nil) {
        let dir = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        self.file = file ?? dir.appendingPathComponent("recent-screen-v1.json")
    }
    private static func identity(_ links: [String]) -> String {
        SHA256.hash(data: Data(links.sorted().joined(separator: "\n").utf8)).map { String(format: "%02x", $0) }.joined()
    }
    static func prepare(_ text: String, links: [String]) -> Data? {
        guard let root = try? JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Any],
              var screen = root["screen"] as? [String: Any] else { return nil }
        screen["online"] = false
        if var thread = screen["thread"] as? [String: Any] {
            thread["draft"] = ""
            thread["sending"] = []
            screen["thread"] = thread
        }
        for key in ["hubs", "usageMachines"] {
            if let rows = screen[key] as? [[String: Any]] {
                screen[key] = rows.map { row in var r = row; r["online"] = false; return r }
            }
        }
        return try? JSONSerialization.data(withJSONObject: ["version": 1, "identity": identity(links), "screen": screen])
    }
    func save(_ text: String, links: [String], generation: UInt64) {
        guard let data = Self.prepare(text, links: links), data.count <= 2 << 20 else { return }
        lock.withLock {
            guard generation > self.generation else { return }
            self.generation = generation
            #if os(iOS)
            try? data.write(to: file, options: [.atomic, .completeFileProtectionUntilFirstUserAuthentication])
            #else
            try? data.write(to: file, options: .atomic)
            #endif
        }
    }
    func load(links: [String]) -> Data? {
        lock.withLock {
            guard let size = (try? FileManager.default.attributesOfItem(atPath: file.path)[.size]) as? Int,
                  size <= 2 << 20, let data = try? Data(contentsOf: file),
                  let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                  object["version"] as? Int == 1, object["identity"] as? String == Self.identity(links),
                  let screen = object["screen"] else { return nil }
            return try? JSONSerialization.data(withJSONObject: screen)
        }
    }
    func clear(through generation: UInt64) {
        lock.withLock {
            self.generation = max(self.generation, generation)
            try? FileManager.default.removeItem(at: file)
        }
    }
    func size() -> Int {
        lock.withLock { ((try? FileManager.default.attributesOfItem(atPath: file.path)[.size]) as? Int) ?? 0 }
    }
}
