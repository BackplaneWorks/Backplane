import Foundation

@MainActor
final class ManualClock {
    struct Item { let at: Int; let wake: @MainActor () -> Void }
    var now = 0
    var items: [Item] = []
    func schedule(_ wake: @escaping @MainActor () -> Void) -> CacheSaveWindow.Cancel {
        items.append(Item(at: now + 300, wake: wake))
        // Deliberately deliver canceled callbacks: ticket fencing must hold.
        return {}
    }
    func advance(to time: Int) {
        now = time
        let due = items.filter { $0.at <= time }
        items.removeAll { $0.at <= time }
        for item in due { item.wake() }
    }
}

@main
struct CacheWindowTests {
    @MainActor static func main() throws {
        func check(_ ok: @autoclosure () -> Bool, _ label: String) {
            precondition(ok(), label)
            print("ok \(label)")
        }
        let clock = ManualClock()
        var latest = 0
        var saved: [Int] = []
        let window = CacheSaveWindow(schedule: clock.schedule) { saved.append(latest) }
        latest = 1; window.offer()
        clock.advance(to: 100); latest = 2; window.offer()
        clock.advance(to: 200); latest = 3; window.offer()
        clock.advance(to: 299); check(saved.isEmpty, "no early flush")
        clock.advance(to: 300); check(saved == [3], "continuous offers flush latest at first deadline")
        latest = 4; window.offer()
        clock.advance(to: 600); check(saved == [3, 4], "later offer opens next window")
        latest = 5; window.offer()
        clock.advance(to: 700); window.cancel(); latest = 6; window.offer()
        clock.advance(to: 900); check(saved == [3, 4], "retired wake cannot flush new owner")
        latest = 7; window.offer()
        clock.advance(to: 1000); check(saved == [3, 4, 7], "retired wake cannot clear newer window")

        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let store = ScreenCache(file: dir.appendingPathComponent("cache.json"), report: { _ in })
        let links = ["http://example.invalid:3787/"]
        func snapshot(_ name: String) -> String {
            "{\"screen\":{\"hub\":\"example.invalid:3787\",\"sel\":\"\(name)\"}}"
        }
        store.save(snapshot("old"), links: links, profile: "one", generation: 1)
        check(store.load(links: links, profile: "one") != nil, "real cache accepts matching identity")
        check(store.load(links: links, profile: "two") == nil, "real cache rejects other profile")
        check(store.load(links: ["http://other.invalid:3787/"], profile: "one") == nil, "real cache rejects other hub")
        store.clear(through: 2)
        store.save(snapshot("retired"), links: links, profile: "one", generation: 1)
        check(store.load(links: links, profile: "one") == nil, "retired save cannot resurrect cleared cache")
        store.save(snapshot("replacement"), links: links, profile: "one", generation: 3)
        store.save(snapshot("old"), links: links, profile: "one", generation: 1)
        let loaded = store.load(links: links, profile: "one")!
        let json = try JSONSerialization.jsonObject(with: loaded) as! [String: Any]
        check(json["sel"] as? String == "replacement", "late old save cannot replace newer cache")
    }
}
