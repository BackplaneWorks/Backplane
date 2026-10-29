// Compile with the actual production EarlierPageGate.swift in the next coherent
// native workflow. Helper proof only; root actual gestures remain the UI gate.
@main struct EarlierPageGateCases {
    static func main() {
        var gate = EarlierPageGate(), calls = 0
        func accepted() -> Bool { calls += 1; return true }
        gate.configure(enabled: false)
        gate.beginDrag()
        precondition(!gate.request(24032, perform: accepted))
        precondition(calls == 0, "cached-unready gesture is not consumed or dispatched")
        gate.configure(enabled: true)
        precondition(calls == 0, "readiness alone never pages")
        precondition(!gate.request(24032) { false }, "declined callback does not latch")
        precondition(gate.request(24032, perform: accepted), "later real accepted gesture requests page")
        precondition(!gate.request(24032, perform: accepted), "same page has exactly one accepted dispatch")
        precondition(calls == 1)
        gate.configure(enabled: false)
        precondition(!gate.request(24032, perform: accepted))
        gate.configure(enabled: true)
        precondition(calls == 1, "reconnect/refresh never retries automatically")
        precondition(!gate.request(24000, perform: accepted), "reply/remaining change during same drag cannot request another page")
        precondition(calls == 1, "readiness and earlier count never reset accepted gesture")
        gate.beginDrag()
        precondition(calls == 1, "new drag beginning alone does not page")
        precondition(gate.request(24000, perform: accepted), "next real drag may request one bounded page")
        for earlier in [23968, 23936, 23904] {
            gate.configure(enabled: false)
            gate.configure(enabled: true)
            precondition(!gate.request(earlier, perform: accepted), "same drag/deceleration cannot cascade across replies")
        }
        precondition(!gate.request(0, perform: accepted))
        precondition(calls == 2)
        gate.configure(enabled: true, page: "older-head")
        precondition(gate.request(23904, perform: accepted), "a new displayed page permits continuing the real fling")
        precondition(!gate.request(23904, perform: accepted), "only one request for that page")
        gate.configure(enabled: false, page: "older-head")
        gate.configure(enabled: true, page: "older-head")
        precondition(!gate.request(23872, perform: accepted), "same-page reconnect/count changes cannot rearm")
        precondition(calls == 3)
        var fresh = EarlierPageGate()
        fresh.configure(enabled: true, page: "first")
        precondition(!fresh.request(100, perform: accepted), "a fresh ready page still needs a real drag")
        print("PASS production EarlierPageGate: acceptance, same-page deduplication, continuing-fling next page, no readiness-only paging")
    }
}
