import Foundation

// Minimal CBOR fixture encoder: only the types that these plot packets use.
private func head(_ major: UInt8, _ n: Int) -> Data {
    precondition(n >= 0 && n < 256)
    return n < 24 ? Data([(major << 5) | UInt8(n)]) : Data([(major << 5) | 24, UInt8(n)])
}
private func cbor(_ value: Any) -> Data {
    if let n = value as? Int { return n >= 0 ? head(0, n) : head(1, -1 - n) }
    if let s = value as? String { let bytes = Data(s.utf8); return head(3, bytes.count) + bytes }
    if let bytes = value as? Data { return head(2, bytes.count) + bytes }
    if let array = value as? [Any] { return array.reduce(head(4, array.count)) { $0 + cbor($1) } }
    if let object = value as? [String: Any] {
        return object.keys.sorted().reduce(head(5, object.count)) { $0 + cbor($1) + cbor(object[$1]!) }
    }
    preconditionFailure("unsupported fixture value")
}
private func plot(_ key: String, _ chunks: [Any]) -> Data {
    cbor(["t": "plot", "key": key, "cs": chunks])
}
private func chunk(_ layer: Int) -> [String: Any] { ["l": layer, "c": 1, "a": 255, "d": Data()] }

@main struct PlotStoreTests {
    @MainActor static func awaitFrame(_ store: PlotStore, key: String) async -> PlotFrame {
        for _ in 0..<200 {
            if let frame = store.frame, frame.key == key { return frame }
            try? await Task.sleep(for: .milliseconds(10))
        }
        fputs("FAIL timed out waiting for plot frame\n", stderr); exit(1)
    }
    static func check(_ condition: @autoclosure () -> Bool, _ name: String) {
        guard condition() else { fputs("FAIL \(name)\n", stderr); exit(1) }
        print("PASS \(name)")
    }
    @MainActor static func main() async {
        let store = PlotStore()
        store.receive(plot("seed", [chunk(3)]))
        let seed = await awaitFrame(store, key: "seed")
        check(seed.none.isEmpty && seed.chunks.count == 1 && seed.chunks[0].layer == 3, "valid initial chunk")
        store.receive(plot("reference", [0]))
        let valid = await awaitFrame(store, key: "reference")
        check(valid.none.isEmpty && valid.chunks.count == 1 && valid.chunks[0].layer == 3, "valid held reference")

        store.receive(plot("negative", [-1]))
        let negative = await awaitFrame(store, key: "negative")
        check(!negative.none.isEmpty && negative.chunks.isEmpty, "negative reference rejected atomically")

        store.receive(plot("seed2", [chunk(4)]))
        _ = await awaitFrame(store, key: "seed2")
        store.receive(plot("past-end", [1]))
        let pastEnd = await awaitFrame(store, key: "past-end")
        check(!pastEnd.none.isEmpty && pastEnd.chunks.isEmpty, "reference equal to held count rejected")

        store.receive(plot("seed3", [chunk(5)]))
        _ = await awaitFrame(store, key: "seed3")
        store.receive(plot("partial", [chunk(6), 1]))
        let partial = await awaitFrame(store, key: "partial")
        check(!partial.none.isEmpty && partial.chunks.isEmpty, "mixed valid and unresolved chunks rejected atomically")
        store.receive(plot("cleared", [0]))
        let cleared = await awaitFrame(store, key: "cleared")
        check(!cleared.none.isEmpty && cleared.chunks.isEmpty, "invalid frame clears held references")

        store.receive(plot("late", [chunk(7)]))
        store.reset()
        try? await Task.sleep(for: .milliseconds(200))
        check(store.frame == nil, "reset rejects late asynchronous decode")
        store.receive(plot("after-reset", [chunk(8)]))
        let fresh = await awaitFrame(store, key: "after-reset")
        check(fresh.none.isEmpty && fresh.chunks.count == 1 && fresh.chunks[0].layer == 8, "new plot works after reset")
    }
}
