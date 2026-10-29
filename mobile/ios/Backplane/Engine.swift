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
            // This thread lives for the app's lifetime. Drain temporary
            // Objective-C/JavaScriptCore values after each completed task,
            // including large wire arguments and rendered screen strings.
            autoreleasepool { f() }
        }
    }
}

// The Bend client (bridge.js) in JavaScriptCore, on one thread of its own.
// Every call answers {"screen": ..., "cmds": [...]} as a string.
final class Engine: @unchecked Sendable {
    private let queue = Worker()
    private let priority = WirePriority()
    private let pairingFlush = PairingFlushGate()
    private let navigationFlush = PairingFlushGate()
    private let viewerDispatch = ViewerDispatchGate()
    private var ctx: JSContext?

    private func context() -> JSContext {
        if let ctx { return ctx }
        let started = PerformanceTrace.shared.begin()
        defer { PerformanceTrace.shared.end(.context, operation: .startup, started: started) }
        let c = JSContext()!
        c.exceptionHandler = { _, e in NSLog("bridge.js: %@", e?.toString() ?? "?") }
        let url = Bundle.main.url(forResource: "bridge", withExtension: "js")!
        c.evaluateScript(try! String(contentsOf: url, encoding: .utf8), withSourceURL: url)
        c.objectForKeyedSubscript("Backplane")?.invokeMethod("publicationAck", withArguments: [])
        c.objectForKeyedSubscript("Backplane")?.invokeMethod("nativeSidebar", withArguments: [1])
        if let text = c.objectForKeyedSubscript("Backplane")?.invokeMethod("headerKeys", withArguments: [])?.toString() {
            priority.install(text)
        }
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
    var onFlush: (@Sendable (Out?, UInt64) -> Void)?

    // Called synchronously when the person's pairing changes. Reconciliation
    // releases this generation only after retired state has been removed.
    func invalidatePairing() -> UInt64 { pairingFlush.invalidate() }
    func resumePairing(_ generation: UInt64) {
        if pairingFlush.resume(generation) { scheduleFlush() }
    }

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
        // While reconciliation runs, leave bridge queues untouched. The
        // matching resume schedules another attempt, even if this was the last.
        guard let generation = pairingFlush.stamp() else { return }
        let began = DispatchTime.now().uptimeNanoseconds
        let forced = lock.withLock { owe }
        let bridge = context().objectForKeyedSubscript("Backplane")
        let invoked = PerformanceTrace.shared.begin()
        let text = bridge?.invokeMethod("flush", withArguments: [forced])?.toString() ?? "{}"
        PerformanceTrace.shared.end(.invoke, operation: .flush, started: invoked, bytes: UInt64(text.utf8.count))
        let out = decode(text, screen: true, operation: .flush)
        guard out?.screen != nil else { onFlush?(out, generation); return }
        let stale = queue.personWaiting
        lock.withLock { built = began; owe = stale }
        if stale { scheduleFlush() }
        let result = stale ? decode(text, screen: false, operation: .flush) : out
        onFlush?(stale || pairingFlush.stamp() != generation ? result : published(result), generation)
    }

    // On the engine worker, acknowledge only a decoded screen that is not
    // being discarded. The bridge then exposes any on-demand follow-up in
    // this same Out's deadline; quiet and discarded screens remain pending.
    private func published(_ candidate: Out?) -> Out? {
        guard var out = candidate, out.screen != nil, pairingFlush.stamp() != nil else { return candidate }
        if let value = context().objectForKeyedSubscript("Backplane")?.invokeMethod("published", withArguments: []),
           !value.isNull, !value.isUndefined {
            let deadline = value.toDouble()
            if deadline.isFinite { out.deadline = deadline }
        }
        return out
    }

    // Enqueue captured editor work synchronously with the main-actor edit.
    // An async Task created for each keystroke could reorder draft/send calls.
    private func enqueue<T>(_ name: String, _ args: [Any], person: Bool = false, late: (() -> [Any])? = nil, authorize: ((() -> String) -> String?)? = nil, _ finish: @escaping (String) -> T, completion: @escaping (T) -> Void) {
        let queued = PerformanceTrace.shared.begin()
        let operation = Self.performanceOperation(name)
        queue.async(person: person) {
            PerformanceTrace.shared.end(.queueWait, operation: operation, started: queued)
            let b = self.context().objectForKeyedSubscript("Backplane")!
            let t0 = DispatchTime.now().uptimeNanoseconds
            let invoke = {
                let arguments = late?() ?? args
                let started = PerformanceTrace.shared.begin()
                let text = b.invokeMethod(name, withArguments: arguments)?.toString() ?? "{}"
                PerformanceTrace.shared.end(.invoke, operation: operation, started: started, bytes: UInt64(text.utf8.count))
                return text
            }
            let text: String
            if let authorize {
                guard let allowed = authorize(invoke) else { completion(finish("null")); return }
                text = allowed
            } else { text = invoke() }
            let ms = (DispatchTime.now().uptimeNanoseconds - t0) / 1_000_000
            if ms > 100 { NSLog("Backplane slow %@: %llu ms", name, ms) }
            completion(finish(text))
        }
    }
    private static func performanceOperation(_ name: String) -> PerformanceTrace.Operation {
        switch name {
        case "start": .startup
        case "loadPresentation": .loadPresentation
        case "connection", "online", "hubs": .connection
        case "recv": .receive
        case "flush": .flush
        case "act", "navigation": .navigation
        case "viewerAction": .viewerRestore
        case "local": .local
        case "reading": .reading
        case "pulse", "tick": .pulse
        case "screen": .screen
        default: .other
        }
    }

    private func decode(_ text: String, screen: Bool, operation: PerformanceTrace.Operation) -> Out? {
        let started = PerformanceTrace.shared.begin()
        let result = Out.decode(text, screen: screen)
        PerformanceTrace.shared.end(.decode, operation: operation, started: started,
                                    bytes: UInt64(text.utf8.count), outcome: result == nil ? .failed : .success)
        return result
    }

    private func call<T>(_ name: String, _ args: [Any], person: Bool = false, late: (() -> [Any])? = nil, authorize: ((() -> String) -> String?)? = nil, _ finish: @escaping (String) -> T) async -> T {
        await withCheckedContinuation { k in
            enqueue(name, args, person: person, late: late, authorize: authorize, finish, completion: { k.resume(returning: $0) })
        }
    }

    // Commands are always delivered. Screens become stale only for newer
    // person work, never because hub frames wait behind a tap.
    private func enqueueOut(_ name: String, _ args: [Any], screen: Bool = true, person: Bool = false, quiet: Bool = false, authorize: ((() -> String) -> String?)? = nil, completion: @escaping (Out?) -> Void) {
        if screen { lock.withLock { if person { personAhead += 1 } else { backgroundAhead += 1 } } }
        var withheld = false
        var began: UInt64 = 0
        var carriedOwe = false
        let late: (() -> [Any])? = quiet ? {
            self.lock.withLock {
                began = DispatchTime.now().uptimeNanoseconds
                // a streaming frame builds at most one screen per window;
                // a burst (a cold join, catching up) one a second, as
                // before, while more frames wait behind this one
                let age = began &- self.built
                withheld = self.queue.personWaiting || age < Self.window || (self.backgroundAhead > 1 && age < Self.burst)
                let o = self.owe
                carriedOwe = o
                self.owe = false
                return args + [withheld, o]
            }
        } : nil
        enqueue(name, args, person: person, late: late, authorize: authorize, { text in
            if text == "null" {
                self.lock.withLock { if screen { if person { self.personAhead -= 1 } else { self.backgroundAhead -= 1 } } }
                return nil
            }
            // (bridge.js writes the screen first: a null one is cheap to see)
            let hasScreen = screen && !text.hasPrefix("{\"screen\":null")
            let metadata = hasScreen ? nil : self.decode(text, screen: false, operation: Self.performanceOperation(name))
            let stale = self.lock.withLock {
                if screen { if person { self.personAhead -= 1 } else { self.backgroundAhead -= 1 } }
                if metadata?.inert == true {
                    self.owe = self.owe || carriedOwe
                    return false
                }
                let drop = hasScreen && (person ? self.personAhead > 0 : self.queue.personWaiting)
                if hasScreen {
                    if !person { self.built = began == 0 ? DispatchTime.now().uptimeNanoseconds : began }
                    self.owe = drop
                }
                return drop
            }
            if metadata?.inert != true && (withheld || stale) { self.scheduleFlush() }
            let result = metadata ?? self.decode(text, screen: !stale && screen, operation: Self.performanceOperation(name))
            return stale ? result : self.published(result)
        }, completion: completion)
    }

    private func out(_ name: String, _ args: [Any], screen: Bool = true, person: Bool = false, quiet: Bool = false, authorize: ((() -> String) -> String?)? = nil) async -> Out? {
        await withCheckedContinuation { k in
            enqueueOut(name, args, screen: screen, person: person, quiet: quiet, authorize: authorize, completion: { k.resume(returning: $0) })
        }
    }

    func draftAtNow(_ thread: String, text: String, edit: UInt64, generation: UInt64, completion: @escaping (Out?) -> Void) {
        enqueueOut("draftAt", [thread, text, edit], screen: false, person: true,
                   authorize: { self.pairingFlush.perform(generation, $0) }, completion: completion)
    }
    func sendAtNow(_ thread: String, action: String, text: String, edit: UInt64, generation: UInt64, completion: @escaping (Out?) -> Void) {
        enqueueOut("sendAt", [thread, action, text, edit], person: true,
                   authorize: { self.pairingFlush.perform(generation, $0) }, completion: completion)
    }
    func composerActNow(_ thread: String, action: String, value: String, generation: UInt64, completion: @escaping (Out?) -> Void) {
        enqueueOut("composerAction", [thread, action, value], person: true,
                   authorize: { self.pairingFlush.perform(generation, $0) }, completion: completion)
    }

    func navigationPermitted(_ pairing: UInt64) -> Bool { pairingFlush.stamp() == pairing }
    func invalidateNavigation() -> UInt64 { navigationFlush.invalidate() }
    @discardableResult func resumeNavigation(_ generation: UInt64) -> Bool {
        navigationFlush.stamp() == generation || navigationFlush.resume(generation)
    }
    func navigation(_ action: String, _ value: String, hub: String, epoch: String, pairing: UInt64, generation: UInt64, presentedNav: String? = nil) async -> Out? {
        await out("navigation", [action, value, hub, epoch, presentedNav.map { $0 as Any } ?? NSNull()], person: true, authorize: { operation in
            self.pairingFlush.perform(pairing) {
                self.navigationFlush.perform(generation, operation) ?? "null"
            }
        })
    }

    func start(_ cid: String, _ drafts: String) async -> Out? { await out("start", [cid, drafts]) }
    // a choice of this device that Bend draws by ("view": "projects" or "active")
    func local(_ key: String, _ value: String) async -> Out? { await out("local", [key, value], person: true) }
    func reading(_ thread: String, seen: String, count: Int, away: Bool) async -> Out? {
        await out("reading", [thread, seen, count, away], screen: false, person: true)
    }
    func viewport(_ thread: String, id: String, offset: Double, first: String, last: String) async -> Out? {
        await out("viewport", [thread, id, offset, first, last], screen: false, person: true)
    }
    func hubs(_ keys: [String]) async -> Out? { await out("hubs", [keys]) }
    func forgetHub(_ key: String) async -> Out? { await out("forgetHub", [key], screen: false, person: true) }
    func connection(_ key: String, _ epoch: String, mode: String? = nil) async -> Out? {
        await out("connection", mode.map { [key, epoch, $0] } ?? [key, epoch], screen: false, person: true)
    }
    func register(_ kind: String, _ token: String, env: String, bundle: String) async -> Out? {
        await out("register", ["ios", token, kind, "", env, bundle], screen: false)
    }
    func register(at hub: String, _ kind: String, _ token: String, env: String, bundle: String) async -> Out? {
        await out("registerAt", [hub, "ios", token, kind, "", env, bundle], screen: false)
    }
    func screen() async -> Out? { await out("screen", [], person: true) }
    func priorityFrame(_ data: Data) -> Bool { priority.person(data) }
    func recv(_ key: String, _ text: String, epoch: String, person: Bool = false) async -> Out? { await out("recv", [key, text, epoch], person: person, quiet: true) }
    func pulse() async -> Out? { await out("pulse", [], screen: false, person: true) }
    // the whole client state as text (StateStore), and the state loaded back
    func savePresentation() async -> String { await call("savePresentation", [], person: true) { $0 } }
    func loadPresentation(_ text: String) async -> Out? { await out("loadPresentation", [text], screen: false, person: true) }
    func save() async -> String { await call("save", []) { $0 } }
    func load(_ text: String) async -> Out? { await out("load", [text]) }
    func act(_ action: String, _ value: String) async -> Out? {
        let answer = await out("act", [action, value], person: true)
        return answer
    }
    func viewerToken(_ token: ViewerCloseIntent.Token?, closing: Bool = false) { viewerDispatch.install(token, closing: closing) }
    func viewerAct(_ value: String, token: ViewerCloseIntent.Token) async -> Out? {
        await out("viewerAction", [token.owner.hub, token.owner.nav, Int(token.presentation), value], person: true,
                  authorize: { self.viewerDispatch.perform(token, value: value, $0) })
    }
    func quiet(_ action: String, _ value: String) async -> Out? { await out("quiet", [action, value], screen: false, person: true) }
    func projectSearchQuery(scope: Int, edit: Int, text: String) async -> Out? {
        await out("projectSearchQuery", [scope, edit, text], person: true)
    }
    func projectSearchEnd(scope: Int) async -> Out? {
        await out("projectSearchEnd", [scope], screen: false, person: true)
    }
    func projectSearchGo(scope: Int, edit: Int, text: String, project: String) async -> Out? {
        await out("projectSearchGo", [scope, edit, text, project], person: true)
    }
    // the secret box's Save: the value goes in one request built by Bend
    // (bridge.js's Backplane.secret), never as an action or a form field
    func secret(_ packed: String) async -> Out? { await out("secret", [packed], person: true) }
    func online(_ key: String, _ b: Bool, epoch: String = "") async -> Out? { await out("online", [key, b, false, epoch], person: true) }
    // a hub marked offline with no screen of its own (after load())
    func offline(_ key: String) async -> Out? { await out("online", [key, false, true], screen: false) }
    func tick(_ now: Int) async -> Out? { await out("tick", [now]) }
    func reflowDefaults() async -> String { await call("reflowDefaults", [], person: true) { $0 } }
    func reflowAlloys() async -> String { await call("reflowAlloys", [], person: true) { $0 } }
    func reflow(_ profile: String, alloy: String) async -> String { await call("reflow", [profile, alloy], person: true) { $0 } }
    // a catalog text by key, in the device's language
    func t(_ key: String) async -> String { await call("t", [key], person: true) { $0 } }
    // a cat's rig for its key ("look:mood")
    func cat(_ key: String) async -> String { await call("cat", [key], person: true) { $0 } }
}
