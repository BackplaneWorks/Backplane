import Foundation
import JavaScriptCore

// One thread with a deep stack: the hub's log arrives as one message and
// Bend walks it recursively. A dispatch queue's 512 KB overflows on a real
// log, the call throws, and the app keeps its empty first screen.
private final class Worker: Thread, @unchecked Sendable {
    private let cond = NSCondition()
    private var people: [() -> Void] = []
    private var background: [() -> Void] = []

    var personWaiting: Bool { cond.withLock { !people.isEmpty } }

    override init() {
        super.init()
        name = "bend"
        stackSize = 64 << 20
        qualityOfService = .userInitiated
        start()
    }

    func async(person: Bool, _ f: @escaping () -> Void) {
        cond.lock()
        if person { people.append(f) } else { background.append(f) }
        cond.signal()
        cond.unlock()
    }

    override func main() {
        while true {
            cond.lock()
            while people.isEmpty && background.isEmpty { cond.wait() }
            let f = people.isEmpty ? background.removeFirst() : people.removeFirst()
            cond.unlock()
            f()
        }
    }
}

// The Bend client (bridge.js) in JavaScriptCore, on one thread of its own.
// Every call answers {"screen": ..., "cmds": [...]} as a string.
final class Engine: @unchecked Sendable {
    private let queue = Worker()
    private var ctx: JSContext?

    private func context() -> JSContext {
        if let ctx { return ctx }
        let c = JSContext()!
        c.exceptionHandler = { _, e in NSLog("bridge.js: %@", e?.toString() ?? "?") }
        let url = Bundle.main.url(forResource: "bridge", withExtension: "js")!
        c.evaluateScript(try! String(contentsOf: url, encoding: .utf8), withSourceURL: url)
        ctx = c
        return c
    }

    // calls queued whose answer carries a screen: while one is, the screen
    // before it is out of date before it could be drawn, so it is skipped
    private let lock = NSLock()
    private var personAhead = 0
    private var backgroundAhead = 0
    // a screen was dropped that way: the next recv builds one even for a
    // frame that changes nothing on screen (bridge.js's owed)
    private var owe = false
    // Background streaming screens have a 250 ms window. An owed screen
    // flushes even if no further frame arrives; person calls bypass it.
    private static let window: UInt64 = 250_000_000
    private static let burst: UInt64 = 1_000_000_000
    private var built: UInt64 = 0
    private var flushPending = false
    var onFlush: (@Sendable (Out?) -> Void)?

    private func scheduleFlush() {
        let delay: UInt64? = lock.withLock {
            guard !flushPending else { return nil }
            flushPending = true
            let elapsed = DispatchTime.now().uptimeNanoseconds &- built
            return elapsed >= Self.window ? 0 : Self.window - elapsed
        }
        guard let delay else { return }
        DispatchQueue.global(qos: .userInitiated).asyncAfter(deadline: .now() + .nanoseconds(Int(delay))) { [weak self] in
            guard let self else { return }
            self.queue.async(person: false) { self.flushReady() }
        }
    }

    // Runs only on the engine thread. bridge.flush builds nothing unless
    // a visible change was withheld or a real screen was dropped.
    private func flushReady() {
        let ready = lock.withLock {
            flushPending = false
            return DispatchTime.now().uptimeNanoseconds &- built >= Self.window
        }
        guard ready, !queue.personWaiting else { scheduleFlush(); return }
        let began = DispatchTime.now().uptimeNanoseconds
        let forced = lock.withLock { owe }
        let text = context().objectForKeyedSubscript("Backplane")?.invokeMethod("flush", withArguments: [forced])?.toString() ?? "{}"
        let out = Out.decode(text, screen: true)
        guard out?.screen != nil else { onFlush?(out); return }
        let stale = queue.personWaiting
        lock.withLock { built = began; owe = stale }
        if stale { scheduleFlush() }
        onFlush?(stale ? Out.decode(text, screen: false) : out)
    }

    // late: the arguments decided on the queue, as the call runs
    private func call<T>(_ name: String, _ args: [Any], person: Bool = false, late: (() -> [Any])? = nil, _ finish: @escaping (String) -> T) async -> T {
        await withCheckedContinuation { k in
            queue.async(person: person) {
                let b = self.context().objectForKeyedSubscript("Backplane")!
                let t0 = DispatchTime.now().uptimeNanoseconds
                let text = b.invokeMethod(name, withArguments: late?() ?? args)?.toString() ?? "{}"
                // a slow step of the Bend client, for finding what to make faster (as Android's)
                let ms = (DispatchTime.now().uptimeNanoseconds - t0) / 1_000_000
                if ms > 100 { NSLog("Backplane slow %@: %llu ms", name, ms) }
                k.resume(returning: finish(text))
            }
        }
    }

    // Commands are always delivered. Screens become stale only for newer
    // person work, never because hub frames wait behind a tap.
    private func out(_ name: String, _ args: [Any], screen: Bool = true, person: Bool = false, quiet: Bool = false) async -> Out? {
        if screen { lock.withLock { if person { personAhead += 1 } else { backgroundAhead += 1 } } }
        var withheld = false
        var began: UInt64 = 0
        let late: (() -> [Any])? = quiet ? {
            self.lock.withLock {
                began = DispatchTime.now().uptimeNanoseconds
                // a streaming frame builds at most one screen per window;
                // a burst (a cold join, catching up) one a second, as
                // before, while more frames wait behind this one
                let age = began &- self.built
                withheld = self.queue.personWaiting || age < Self.window || (self.backgroundAhead > 1 && age < Self.burst)
                let o = self.owe
                self.owe = false
                return args + [withheld, o]
            }
        } : nil
        return await call(name, args, person: person, late: late) { text in
            // (bridge.js writes the screen first: a null one is cheap to see)
            let hasScreen = screen && !text.hasPrefix("{\"screen\":null")
            let stale = self.lock.withLock {
                if screen { if person { self.personAhead -= 1 } else { self.backgroundAhead -= 1 } }
                let drop = hasScreen && (person ? self.personAhead > 0 : self.queue.personWaiting)
                if hasScreen {
                    if !person { self.built = began == 0 ? DispatchTime.now().uptimeNanoseconds : began }
                    self.owe = drop
                }
                return drop
            }
            if withheld || stale { self.scheduleFlush() }
            return Out.decode(text, screen: !stale)
        }
    }

    func start(_ cid: String, _ drafts: String) async -> Out? { await out("start", [cid, drafts]) }
    // a choice of this device that Bend draws by ("view": "projects" or "active")
    func local(_ key: String, _ value: String) async -> Out? { await out("local", [key, value], person: true) }
    func hubs(_ keys: [String]) async -> Out? { await out("hubs", [keys]) }
    func resume(_ key: String) async -> String { await call("resume", [key]) { $0 } }
    func register(_ kind: String, _ token: String, env: String, bundle: String) async -> Out? {
        await out("register", ["ios", token, kind, "", env, bundle], screen: false)
    }
    func register(at hub: String, _ kind: String, _ token: String, env: String, bundle: String) async -> Out? {
        await out("registerAt", [hub, "ios", token, kind, "", env, bundle], screen: false)
    }
    func screen() async -> Out? { await out("screen", [], person: true) }
    func recv(_ key: String, _ text: String) async -> Out? { await out("recv", [key, text], quiet: true) }
    // the whole client state as text (StateStore), and the state loaded back
    func save() async -> String { await call("save", []) { $0 } }
    func load(_ text: String) async -> Out? { await out("load", [text]) }
    func act(_ action: String, _ value: String) async -> Out? { await out("act", [action, value], person: true) }
    func quiet(_ action: String, _ value: String) async -> Out? { await out("quiet", [action, value], screen: false, person: true) }
    func online(_ key: String, _ b: Bool) async -> Out? { await out("online", [key, b]) }
    // a hub marked offline with no screen of its own (after load())
    func offline(_ key: String) async -> Out? { await out("online", [key, false, true], screen: false) }
    func tick(_ now: Int) async -> Out? { await out("tick", [now]) }
    // a catalog text by key, in the device's language
    func t(_ key: String) async -> String { await call("t", [key], person: true) { $0 } }
    // a cat's rig for its key ("look:mood")
    func cat(_ key: String) async -> String { await call("cat", [key], person: true) { $0 } }
}
