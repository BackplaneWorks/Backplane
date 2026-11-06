import Foundation

// client's whole state (Backplane.save()), kept between launches:
// loading it shows every hub's log at once instead of folding it again,
// and each hub then sends only what came since. The file is named for the
// types the state is made of (bridge.js ends with their hash,
// scripts/state-key.py): a new build with the same types keeps it, one
// with other types starts afresh rather than load a state it may not read.
// A bridge.js without the key names it by its whole text.
struct StateStore: Sendable {
    private let dir: URL
    private let name: String

    init() {
        dir = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let source = Bundle.main.url(forResource: "bridge", withExtension: "js")!
        let handle = try? FileHandle(forReadingFrom: source)
        let end = (try? handle?.seekToEnd()) ?? 0
        try? handle?.seek(toOffset: end > 256 ? end - 256 : 0)
        let tail = (try? handle?.readToEnd()) ?? Data()
        try? handle?.close()
        if let key = StateStore.key(tail) {
            name = "state-\(key).json"
        } else {
            var h: UInt64 = 1469598103934665603
            let sourceBytes = (try? Data(contentsOf: source)) ?? Data()
            for b in sourceBytes { h = (h ^ UInt64(b)) &* 1099511628211 }
            name = String(format: "state-%016llx.json", h)
        }
        // Preserve incompatible state files until the replacement is safely
        // written. A schema change must not erase the previous cache at launch.
        if !FileManager.default.fileExists(atPath: dir.appendingPathComponent(name).path),
           ((try? FileManager.default.contentsOfDirectory(atPath: dir.path)) ?? []).contains(where: { $0.hasPrefix("state-") && $0 != name }) {
            NSLog("Backplane state-cache miss_type_key")
        }
    }

    // the key bridge.js ends with (globalThis.BackplaneStateKey = "…")
    private static func key(_ src: Data) -> String? {
        let tail = String(decoding: src.suffix(200), as: UTF8.self)
        guard let r = tail.range(of: "BackplaneStateKey = \"") else { return nil }
        let key = tail[r.upperBound...].prefix { $0.isHexDigit }
        return key.isEmpty ? nil : String(key)
    }

    func load() -> String? {
        guard let t = try? String(contentsOf: dir.appendingPathComponent(name), encoding: .utf8), !t.isEmpty else { NSLog("Backplane state-cache miss_absent_or_io"); return nil }
        return t
    }

    // the kept state's size on disk (App settings, Cache)
    func size() -> Int {
        ((try? FileManager.default.attributesOfItem(atPath: dir.appendingPathComponent(name).path)[.size]) as? Int) ?? 0
    }

    // forgotten: the next launch fetches each hub's log again
    func clear() {
        try? FileManager.default.removeItem(at: dir.appendingPathComponent(name))
    }

    // the hub settings page's last rows, by hub (kept as JSON beside the state)
    var settingsFile: URL { dir.appendingPathComponent("hub-settings.json") }

    func save(_ text: String) {
        guard !text.isEmpty else { return }
        try? Data(text.utf8).write(to: dir.appendingPathComponent(name), options: .atomic)
    }
}
