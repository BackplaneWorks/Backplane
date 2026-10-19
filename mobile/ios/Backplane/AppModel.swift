import Foundation
import Observation
import UIKit

// Feeds every paired hub's socket and the user's actions to the Bend
// client, and shows whatever screen it answers. Runs its commands (send,
// copy, scroll, notify) and keeps the Live Activity in step with the
// screen's island. Each hub is keyed by its host:port (Pairing.key).
@MainActor
@Observable
final class AppModel {
    private let engine = Engine()
    // the client's state, kept for the next launch (StateStore)
    private let kept = StateStore()
    @ObservationIgnored private let recentScreen = ScreenCache()
    @ObservationIgnored private var cachedPresentation = false
    @ObservationIgnored private var cacheGeneration: UInt64 = 0
    @ObservationIgnored private var cacheTask: Task<Void, Never>?
    @ObservationIgnored private var lastScreenJSON: String?
    @ObservationIgnored private var earlyActions: [(String, String)] = []
    @ObservationIgnored private var wantsUsage = false
    @ObservationIgnored private var readingPages: [String: (seen: String, count: Int, away: Bool)] = [:]
    var contentReady: Bool { ready && !cachedPresentation }
    // something changed since the state was last kept
    @ObservationIgnored private var dirty = false
    @ObservationIgnored private var hubs: [String: Hub] = [:]
    // push tokens by kind, sent again to a hub paired later
    @ObservationIgnored private var tokens: [String: String] = [:]
    // each hub's Live Activity token, sent again when that hub reconnects
    @ObservationIgnored private var hubTokens: [String: String] = [:]
    @ObservationIgnored private let notifier = Notifier()
    @ObservationIgnored private var island: IslandController?
    @ObservationIgnored private var ready = false

    // the pairing links, one per hub, in the order they were paired
    private(set) var links: [String] = UserDefaults.standard.stringArray(forKey: "links")
        ?? UserDefaults.standard.string(forKey: "link").map { [$0] } ?? []
    private(set) var screen: Screen?
    // the last status plane the hub sent, drawn while a screen carries none (the
    // hub builds the plane only for the list and the Status tab, so going back
    // from a thread shows this one at once)
    private(set) var lastUber: UberCard?
    private(set) var lastPlane: PlaneModel?

    // the card and the plane to draw: the screen's, else the last it carried
    func uber(_ s: Screen) -> UberCard? { s.plane != nil ? s.uber : (s.uber == nil ? nil : lastUber ?? s.uber) }
    func plane(_ s: Screen) -> PlaneModel? { s.plane ?? (s.uber == nil ? nil : lastPlane) }
    // the board viewer's plots, which come straight from the socket
    let plots = PlotStore()
    // the composer's text, owned here so typing never waits on Bend
    var composer = ""
    private(set) var scrolls = 0
    // an entry of the open thread to show (a "jump"): n counts the jumps, so
    // the same entry shown twice scrolls twice
    private(set) var jump: Jump?
    // a jump waiting for a screen that holds its entry (the jump opens folds
    // and pages first), and how many more screens it waits
    @ObservationIgnored private var jumping: (id: String, left: Int)?
    // each hub's socket as this phone sees it (HubsView, the list's pill)
    let conn = HubConnections()
    private(set) var sendFailure: String?
    @ObservationIgnored private var requestDeadline: Double?
    // the last screen of each thread shown, by its id: a thread tapped
    // again shows at once while its fresh screen is made
    private(set) var seen: [String: ThreadView] = [:]
    private(set) var seenBots: [String: BotView] = [:]
    private(set) var seenPlanes: [String: PlaneModel] = [:]
    @ObservationIgnored private var botOrder: [String] = []
    private(set) var destination: TapDestination?
    // the page the stack was last moved to by the person: until a screen
    // shows it, screens of other selections (answers to older actions)
    // leave the stack alone, so a tap then a swipe back never bounces;
    // given up after 3 s (a page that never comes, a dropped screen), so a
    // notification's select still moves the stack
    @ObservationIgnored private var awaitedNav: String?
    @ObservationIgnored private var awaitedAt = Date.distantPast
    @ObservationIgnored private var pushed = Date.distantPast
    #if DEBUG
    @ObservationIgnored private var tapStart: (nav: String, at: UInt64, first: Int?, full: Int?)?
    private(set) var tapTimes: [String] = []
    #endif
    @ObservationIgnored private var seenOrder: [String] = []
    // messages sent from this phone the client has not answered yet: shown
    // at once as sending, dropped when the answer (with its own sending
    // row) is on screen
    private(set) var outgoing: [Outgoing] = []
    // the navigation stack's path: moved at once by a tap or a swipe back,
    // and by the screen only when its selection changes, so a screen that
    // answers an older action never pulls a thread back open
    // Each tab has its own stack, kept across tab switches; `path` is the
    // one of the tab on screen (the pages of a tab are Bend's nav keys, or
    // "@hub:<key>" for a hub's settings page, which is the app's alone)
    private(set) var stacks: [AppTab: [String]] = [:]
    private(set) var tab: AppTab = .threads
    private(set) var path: [String] {
        get { stacks[tab] ?? [] }
        set { stacks[tab] = newValue }
    }
    // the tab a page was opened in by a tap (a routine opens its bot in the
    // Routines tab, not the Bots tab a bot's page would otherwise go to)
    @ObservationIgnored private var opened: (nav: String, tab: AppTab)?
    // the first screen of this launch chooses the tab and carries the hub's
    // theme and thread list into App settings
    @ObservationIgnored private var started = false
    @ObservationIgnored private var sentView = false
    let settings = AppSettings.shared
    // the hub settings page: each hub's last rows, on disk beside the kept state
    private(set) var hubSettings: [String: Settings] = [:]
    // Clear cached state was pressed: nothing is kept again until the next launch
    @ObservationIgnored private var cleared = false
    @ObservationIgnored private var shownSel = ""
    // the bot, room or remote bot last tapped ("bot", "room", "remote" and
    // its row's id), until a screen shows it: it opens then even when the
    // selection matches the last screen's (a swipe back whose own screen
    // was dropped, or never came, before the tap)
    @ObservationIgnored private var tapped: String?
    // when the stack last popped, and a push waiting for that pop's
    // animation (about half a second) to end: SwiftUI drops a push made
    // during it, and its stack then shows the list while path says the
    // page is open
    @ObservationIgnored private var popped = Date.distantPast
    @ObservationIgnored private var pending: [String]?
    // a new thread's page was opened while the hub makes it
    @ObservationIgnored private var shownMaking = false
    // the hub whose plots are drawn: frames from any other are dropped
    @ObservationIgnored private var shownHub = ""
    var active = true
    // drafts sent to Bend and not yet answered: until then a screen may
    // carry an older draft than the one on screen
    @ObservationIgnored private var typing = 0
    #if DEBUG
    // headless checks: SIMCTL_CHILD_BACKPLANE_SELECT=<thread id> opens it
    @ObservationIgnored private var opening = ProcessInfo.processInfo.environment["BACKPLANE_SELECT"]
    // and SIMCTL_CHILD_BACKPLANE_VIEW=board (or schematic) opens its viewer
    @ObservationIgnored private var viewing = ProcessInfo.processInfo.environment["BACKPLANE_VIEW"]
    // and SIMCTL_CHILD_BACKPLANE_ACTS="diff;;term-toggle=$SIZE;;…" runs
    // actions two seconds apart once the thread is open
    @ObservationIgnored private var acting = ProcessInfo.processInfo.environment["BACKPLANE_ACTS"]
    // and SIMCTL_CHILD_BACKPLANE_BOT=<bot or room name> opens it (then ACTS run)
    @ObservationIgnored private var botting = ProcessInfo.processInfo.environment["BACKPLANE_BOT"]
    #endif

    // this install's id, part of every message id (a resend is stored once)
    private static var cid: String {
        if let c = UserDefaults.standard.string(forKey: "cid") { return c }
        let c = String(format: "%08x", UInt32.random(in: 0 ... UInt32.max))
        UserDefaults.standard.set(c, forKey: "cid")
        return c
    }

    // a thread's draft, written at once (a crash loses nothing typed); "" forgets it
    private static func keep(_ thread: String, _ text: String) {
        let d = UserDefaults.standard
        var o = (try? JSONSerialization.jsonObject(with: Data((d.string(forKey: "drafts") ?? "{}").utf8))) as? [String: String] ?? [:]
        o[thread] = text.isEmpty ? nil : text
        if let data = try? JSONSerialization.data(withJSONObject: o), let s = String(data: data, encoding: .utf8) {
            d.set(s, forKey: "drafts")
        }
    }

    private static var env: String {
        #if DEBUG
        "sandbox"
        #else
        "production"
        #endif
    }

    init() {
        engine.onFlush = { [weak self] out in
            Task { @MainActor [weak self] in self?.apply(out) }
        }
        #if DEBUG
        if let l = ProcessInfo.processInfo.environment["BACKPLANE_LINK"] { links = l.split(separator: " ").map(String.init) }
        #endif
        if let data = recentScreen.load(links: links), let cached = try? JSONDecoder().decode(Screen.self, from: data) {
            screen = cached
            cachedPresentation = true
            lastUber = cached.uber
            lastPlane = cached.plane
            if let thread = cached.thread, !cached.sel.isEmpty { remember(cached.sel, thread) }
            if let tab = AppTab(rawValue: settings.defaultTab), tab != .settings { self.tab = tab }
            else { tab = cached.uber == nil ? .threads : .home }
        }
        notifier.open = { [weak self] id in self?.act("select", id) }
        NotificationCenter.default.addObserver(forName: .NSSystemTimeZoneDidChange, object: nil, queue: .main) { [weak self] _ in
            self?.act("tz-report", TimeZone.current.identifier)
        }
        NotificationCenter.default.addObserver(forName: NSLocale.currentLocaleDidChangeNotification, object: nil, queue: .main) { [weak self] _ in
            Task { @MainActor [weak self] in
                guard let self else { return }
                await self.languageLocal(self.engine)
            }
        }
        notifier.viewing = { [weak self] id in self?.active == true && self?.tab != .settings && self?.screen?.sel == id }
        if let d = try? Data(contentsOf: kept.settingsFile), let m = try? JSONDecoder().decode([String: Settings].self, from: d) { hubSettings = m }
        let e = engine
        Task {
            apply(await e.start(Self.cid, UserDefaults.standard.string(forKey: "drafts") ?? "{}"))
            apply(await e.local("view", settings.threadList))
            await languageLocal(e)
            // the state kept at the last launch: the screen shows at once, and
            // each hub then sends only what came since
            if let text = kept.load() {
                apply(await e.load(text))
                apply(await e.local("view", settings.threadList))
                await languageLocal(e)
                for l in links { if let k = Pairing.key(l) { apply(await e.offline(k)) } }
            }
            ready = true
            island = IslandController { [weak self] kind, token, hub in self?.register(kind, token, hub: hub) }
            connect()
            #if DEBUG
            // headless checks pair from the environment: no permission prompt over the screen
            if ProcessInfo.processInfo.environment["BACKPLANE_LINK"] == nil, !links.isEmpty { notifier.setUp() }
            #else
            if !links.isEmpty { notifier.setUp() }
            #endif
        }
        Task {
            while true {
                try? await Task.sleep(for: .seconds(30))
                if ready { apply(await e.tick(Int(Date().timeIntervalSince1970))) }
                keep()
            }
        }
    }

    // a new hub, or a new link to one already paired (it replaces the old)
    func pair(_ text: String) {
        let l = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let k = Pairing.key(l) else { return }
        links = links.filter { Pairing.key($0) != k } + [l]
        save()
        notifier.setUp()
        connect()
    }

    func unpair(_ key: String) {
        cacheGeneration += 1
        recentScreen.clear(through: cacheGeneration)
        links = links.filter { Pairing.key($0) != key }
        save()
        connect()
    }

    private func save() {
        UserDefaults.standard.set(links, forKey: "links")
        UserDefaults.standard.removeObject(forKey: "link")
    }

    // an APNs token for this device's alerts (from the app delegate)
    func registered(_ token: Data) {
        register("alert", token.map { String(format: "%02x", $0) }.joined())
    }

    // a token with a hub (a Live Activity's) goes to that hub alone
    private func register(_ kind: String, _ token: String, hub: String? = nil) {
        let bundle = Bundle.main.bundleIdentifier ?? ""
        if let h = hub {
            hubTokens[h] = token
            run { await $0.register(at: h, kind, token, env: Self.env, bundle: bundle) }
            return
        }
        tokens[kind] = token
        run { await $0.register(kind, token, env: Self.env, bundle: bundle) }
    }

    // one socket per paired hub: new hubs connect, unpaired ones hang up
    private func connect() {
        guard ready else { return }
        var keyed: [String: String] = [:]
        var keys: [String] = []
        for l in links {
            if let k = Pairing.key(l), keyed[k] == nil { keyed[k] = l; keys.append(k) }
        }
        for (k, h) in hubs where keyed[k] == nil {
            h.stop()
            hubs[k] = nil
        }
        let e = engine
        let fresh = keys.contains { hubs[$0] == nil }
        Task {
            apply(await e.hubs(keys))
            apply(await e.local("view", settings.threadList))
            await languageLocal(e)
            apply(await e.local("usage", wantsUsage ? "1" : ""))
            for (thread, page) in readingPages {
                apply(await e.reading(thread, seen: page.seen, count: page.count, away: page.away))
            }
            for (k, l) in keyed where hubs[k] == nil { open(k, l) }
            // a hub paired later learns this phone's push tokens too
            if fresh {
                for (kind, token) in tokens { register(kind, token) }
                for (hub, token) in hubTokens where keyed[hub] != nil { register("activity", token, hub: hub) }
            }
        }
    }

    private func open(_ key: String, _ link: String) {
        let e = engine
        let h = Hub(url: { [weak self] in
                guard let self else { return nil }
                let epoch = String(self.conn[key]?.epoch ?? 0)
                self.apply(await e.connection(key, epoch))
                let r = try? JSONDecoder().decode(Resume.self, from: Data(await e.resume(key).utf8))
                return Pairing.socket(link, since: r?.since ?? "0", origin: r?.origin ?? "")
            },
            onConnecting: { [weak self] in
                self?.conn[key, default: HubConn()].phase = .connecting
                self?.conn[key, default: HubConn()].epoch += 1
            },
            onOpen: { [weak self] in
                if self?.shownHub == key { self?.plots.reset() }
                self?.conn[key, default: HubConn()].phase = .syncing
                let epoch = String(self?.conn[key]?.epoch ?? 0)
                self?.run { await $0.online(key, true, epoch: epoch) }
                // where the person is: every hub keeps the phone's zone (routines follow it)
                self?.act("tz-report", TimeZone.current.identifier)
            },
            // plots go straight to the viewer, never through the Bend client,
            // and only the hub in focus draws
            onMessage: { [weak self] d in
                if PlotStore.isPlot(d) {
                    if self?.shownHub == key { self?.plots.receive(d) }
                } else {
                    self?.received(key, d)
                }
            },
            onClose: { [weak self] in
                self?.conn[key, default: HubConn()].phase = .offline
                let epoch = String(self?.conn[key]?.epoch ?? 0)
                self?.run { await $0.online(key, false, epoch: epoch) }
            })
        hubs[key] = h
        h.start()
    }

    // a frame from hub key: the first after a connect carries what this
    // phone missed, and the hub is up to date once it is folded in
    private func received(_ key: String, _ d: Data) {
        dirty = true
        conn.received(key, bytes: d.count, at: Date())
        let e = engine
        let person = e.priorityFrame(d)
        let epoch = conn[key]?.epoch ?? 0
        Task {
            let result = await e.recv(key, d.base64EncodedString(), epoch: String(epoch), person: person)
            guard conn[key]?.epoch == epoch else { return }
            apply(result)
        }
    }

    private func run(_ f: @escaping (Engine) async -> Out?) {
        let e = engine
        Task { apply(await f(e)) }
    }

    func act(_ action: String, _ value: String = "") {
        // a web address a settings row offers (Antigravity's sign-in):
        // opened in the browser here, never sent to the engine
        if action == "open-url" {
            if let u = URL(string: value), u.scheme == "https" || u.scheme == "http" { UIApplication.shared.open(u) }
            return
        }
        if !ready || cachedPresentation {
            earlyActions.append((action, value))
            return
        }
        if Self.opens.contains(action) { tapped = value }
        run { await $0.act(action, value) }
    }

    // where the hub in focus serves a path ("/img?path=…"), with its token
    func web(_ path: String) -> URL? {
        guard let s = screen, let l = links.first(where: { Pairing.key($0) == s.hub }) ?? links.first else { return nil }
        return Pairing.web(l, path)
    }

    // a file for the next message: one POST /attach to the thread's hub with
    // the file as the body (no base64, no pieces through the engine); the
    // hub's answer goes back as "attach-end", as an attach.put's would
    func attach(_ data: Data, name: String) {
        guard !data.isEmpty, let s = screen, let named = s.thread?.id,
              let l = links.first(where: { Pairing.key($0) == s.hub }) ?? links.first else { return }
        // the hub's own id: over several hubs the screen names it "<hub>|<id>"
        let thread = named.split(separator: "|").last.map(String.init) ?? named
        let key = String(format: "%08x", UInt32.random(in: 0 ... UInt32.max))
        guard let url = Pairing.http(l, path: "/attach", query: [URLQueryItem(name: "thread", value: thread),
                                                                 URLQueryItem(name: "key", value: key), URLQueryItem(name: "name", value: name)]) else { return }
        var req = URLRequest(url: url, timeoutInterval: 600)
        req.httpMethod = "POST"
        req.setValue("application/octet-stream", forHTTPHeaderField: "Content-Type")
        // the engine decides: one over the cap it refuses, and nothing goes
        let up = (try? JSONSerialization.data(withJSONObject: ["name": name, "size": data.count] as [String: Any])).flatMap { String(data: $0, encoding: .utf8) } ?? ""
        act("attach-up", up)
        guard data.count <= s.thread?.cap ?? 10_485_760 else { return }
        Task {
            var answer = ""
            do {
                let (body, _) = try await URLSession.shared.upload(for: req, from: data)
                answer = String(data: body, encoding: .utf8) ?? ""
            } catch {
                answer = ""
            }
            let answered = (try? JSONSerialization.jsonObject(with: Data(answer.utf8))) is [String: Any]
            if !answered {
                let o: [String: Any] = ["uiError": "upload.network", "thread": thread, "upload": true]
                answer = (try? JSONSerialization.data(withJSONObject: o)).flatMap { String(data: $0, encoding: .utf8) } ?? ""
            }
            act("attach-end", answer)
        }
    }

    // Every row opens through this point; Bend's nav is the destination key.
    func open(nav: String, action: String, value: String, title: String, cat: String = "", kind: String = "thread") {
        // a row from an older bridge has no nav: its page opens when its screen comes
        guard !nav.isEmpty else { act(action, value); return }
        guard path.last != nav, pending?.last != nav,
              Date().timeIntervalSince(pushed) >= 0.35 else { return }
        pushed = Date()
        let dest = destination(kind: kind, action: action)
        opened = (nav, dest)
        if dest != tab { switchTab(dest) }
        let rig = cat.isEmpty ? (screen?.bots.first(where: { $0.nav == nav })?.cat ?? (screen?.uber?.nav == nav ? screen?.uber?.cat : nil) ?? "") : cat
        destination = TapDestination(nav: nav, title: title, cat: rig, kind: kind)
        awaitedNav = nav
        awaitedAt = Date()
        #if DEBUG
        tapStart = (nav, DispatchTime.now().uptimeNanoseconds, nil, nil)
        #endif
        show([nav])
        act(action, value)
    }

    func navigate(_ p: [String]) {
        if p.count < path.count { popped = Date() }
        path = p
        // a hub's settings page is the app's own: nothing to select
        if tab == .settings { return }
        destination = nil
        awaitedNav = p.last ?? ""
        awaitedAt = Date()
        #if DEBUG
        tapStart = nil
        #endif
        tapped = nil
        pending = nil
        act("select", p.last ?? "")
    }

    // Tabs
    // ----

    // a page that is the app's alone (a hub's settings), not a Bend selection
    static func local(_ nav: String) -> Bool { nav.hasPrefix("@hub") || nav == making }

    // where a tap on a row opens its page. From Home a thread goes to
    // Threads and a bot, room or routine to Bots (the uberbot's own card
    // stays on Home); any other tab keeps its page in its own stack.
    private func destination(kind: String, action: String) -> AppTab {
        guard tab == .home else { return tab }
        switch kind {
        case "thread": return .threads
        case "bot": return action == "uber" ? .home : .bots
        case "room", "remote", "routine": return .bots
        default: return tab
        }
    }

    // where a page selected from elsewhere (a notification, a link, the
    // island, a link in a thread) opens: a bot's thread, a room or a
    // remote bot in Bots, the uberbot's on Home, any other thread in Threads
    // (Bend says which: the screen's bot, and the uber card's sel)
    private func destination(_ s: Screen, _ nav: String) -> AppTab {
        if let o = opened, o.nav == nav { return o.tab }
        if nav == Self.making { return tab }
        if s.bot != nil { return s.uber?.sel == true ? .home : .bots }
        return .threads
    }

    // the tab on screen changes: it shows its own stack at once, and Bend
    // selects what that stack ends with (the page is cached, so it draws
    // now and goes live when its screen comes); an empty stack selects nothing
    func switchTab(_ t: AppTab) {
        guard t != tab else { return }
        tab = t
        let top = path.last(where: { !Self.local($0) }) ?? ""
        pending = nil
        awaitedNav = top
        awaitedAt = Date()
        shownSel = top
        tapped = nil
        act("select", top)
    }

    // a thread's screen kept for the next time it is opened (the last 24)
    private func remember(_ sel: String, _ t: ThreadView) {
        seen[sel] = t
        if seenOrder.last != sel {
            seenOrder.removeAll { $0 == sel }
            seenOrder.append(sel)
            if seenOrder.count > 24 { seen[seenOrder.removeFirst()] = nil }
        }
    }

    private func rememberBot(_ nav: String, _ b: BotView) {
        seenBots[nav] = b
        botOrder.removeAll { $0 == nav }
        botOrder.append(nav)
        if botOrder.count > 24 {
            let old = botOrder.removeFirst()
            seenBots[old] = nil
            seenPlanes[old] = nil
        }
    }

    // the cats' rigs by key ("look:mood"), each asked of the bridge once
    private(set) var cats: [String: [CatPart]] = [:]
    @ObservationIgnored private var asked: Set<String> = []

    // catalog texts for native parts, by key (spec 4.1); read again when the
    // language changes, which shows in the screen's language row
    private(set) var texts: [String: String] = [:]
    @ObservationIgnored private var wanted: Set<String> = []
    @ObservationIgnored private var langLabel: String?

    // the text for a key; its English (fallback) until the bridge answers
    func t(_ key: String, _ fallback: String) -> String { texts[key] ?? fallback }

    func want(_ key: String) async {
        wanted.insert(key)
        texts[key] = await engine.t(key)
    }

    private func refreshTexts() {
        let keys = wanted
        Task { for k in keys { texts[k] = await engine.t(k) } }
    }

    func cat(_ key: String) async {
        guard !key.isEmpty, !asked.contains(key) else { return }
        asked.insert(key)
        cats[key] = catRig(Data(await engine.cat(key).utf8))
    }

    // a bot form's field ("bfield"), typed: Bend keeps it without a new
    // screen, the field on screen already shows it
    func field(_ name: String, _ text: String) {
        run { await $0.quiet("bfield", name + "\u{1f}" + text) }
    }

    // an address on the hub in focus (a bot's browser frame, a webhook)
    func hubURL(_ path: String, query: [URLQueryItem] = [], token: Bool = true) -> URL? {
        guard let s = screen, let l = links.first(where: { Pairing.key($0) == s.hub }) else { return nil }
        return Pairing.http(l, path: path, query: query, token: token)
    }

    // the actions that open a bot's, a room's or a remote bot's page, and
    // the uberbot's card ("uber": the uberbot is not in the bots list, so
    // its card is the only way in; without it here, a swipe back whose own
    // screen was dropped left the card unable to open it again)
    static let opens: Set<String> = ["bot", "room", "remote", "uber", "uber-bot"]

    // whether the screen shows what was tapped (the row's id is its hub's
    // key and the bot's id, the screen's bot its id alone) while the stack
    // does not
    private func shows(tapped s: Screen, _ nav: String) -> Bool {
        guard let t = tapped, let b = s.bot, !nav.isEmpty, t == b.id || t.hasSuffix("|" + b.id) else { return false }
        tapped = nil
        return (pending ?? path) != [nav]
    }

    #if DEBUG
    private func tapShown(_ nav: String) {
        guard var t = tapStart, t.nav == nav, t.first == nil else { return }
        t.first = Int((DispatchTime.now().uptimeNanoseconds - t.at) / 1_000_000)
        tapStart = t
        tapReport()
    }

    private func tapFull(_ nav: String) {
        guard var t = tapStart, t.nav == nav, t.full == nil else { return }
        t.full = Int((DispatchTime.now().uptimeNanoseconds - t.at) / 1_000_000)
        tapStart = t
        tapReport()
    }

    private func tapReport() {
        guard let t = tapStart, let first = t.first, let full = t.full else { return }
        NSLog("Backplane tap %@ first %d ms full %d ms", t.nav, first, full)
        tapTimes.append("\(t.nav) first \(first) ms full \(full) ms")
        if tapTimes.count > 24 { tapTimes.removeFirst() }
        tapStart = nil
    }
    #endif

    // the stack moved to p by a screen; a push waits out a pop's animation
    private func show(_ p: [String]) {
        let wait = popped.timeIntervalSinceNow + 0.8
        guard wait > 0, p.count > path.count else {
            pending = nil
            path = p
            #if DEBUG
            if let nav = p.last { tapShown(nav) }
            #endif
            return
        }
        pending = p
        DispatchQueue.main.asyncAfter(deadline: .now() + wait) { [weak self] in
            guard let self, self.pending == p else { return }
            self.pending = nil
            self.path = p
            #if DEBUG
            if let nav = p.last { self.tapShown(nav) }
            #endif
        }
    }

    // the page of a thread asked for and not yet made
    static let making = "@new"

    // what the stack shows: the thread selected, or a room or a bot on a
    // linked machine (a bot's own view is its thread's)
    static func nav(_ s: Screen) -> String {
        if let b = s.bot, b.kind != "bot" { return "@" + b.kind + ":" + b.id }
        return s.sel
    }

    // the composer's text sent now: it shows at once as sending and the
    // composer empties; the client's answer carries it on (its own sending
    // row, then the hub's message)
    func send(_ action: String = "send") {
        guard contentReady else { return }
        sendFailure = nil
        let text = composer.trimmingCharacters(in: .whitespacesAndNewlines)
        let thread = screen?.thread?.id ?? ""
        if !text.isEmpty { outgoing.append(Outgoing(thread: thread, text: text)) }
        composer = ""
        typing += 1
        let e = engine
        Task {
            let out = await e.act(action, "")
            typing -= 1
            apply(out, sent: text)
        }
    }

    func draft(_ text: String) {
        composer = text
        typing += 1
        let e = engine
        Task {
            let out = await e.quiet("draft", text)
            typing -= 1
            apply(out)
        }
    }

    // a URL the app was opened with: a pairing link, or a thread to show
    func open(_ url: URL) {
        guard url.scheme == "backplane" else { return }
        if url.host == "open" {
            let q = URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems ?? []
            let id = q.first { $0.name == "thread" }?.value ?? ""
            // (an older Live Activity's "pick=1" opens the app as it was)
            if !id.isEmpty { act("select", id) }
        } else if url.host != "google" {
            // (backplane://google ends a Google sign-in: GoogleLoop)
            pair(url.absoluteString)
        }
    }

    // the state written down when it changed (the app may be ended at any
    // time once in the background)
    func keep() {
        if let text = lastScreenJSON, !cleared {
            let generation = cacheGeneration, links = self.links, store = recentScreen
            Task.detached(priority: .utility) { store.save(text, links: links, generation: generation) }
        }
        guard dirty, ready, !cleared else { return }
        dirty = false
        let e = engine, k = kept
        Task { k.save(await e.save()) }
    }

    // the hub settings page's rows, kept as JSON beside the state
    private func keepHubSettings() {
        guard !cleared, let d = try? JSONEncoder().encode(hubSettings) else { return }
        try? d.write(to: kept.settingsFile, options: .atomic)
    }

    // App settings, Cache: the kept state's size, and forgetting it (the next
    // launch fetches each hub's log again; nothing is written again before)
    func cacheSize() -> Int { kept.size() + recentScreen.size() + ((try? Data(contentsOf: kept.settingsFile).count) ?? 0) }

    func clearCache() {
        cleared = true
        cacheTask?.cancel()
        lastScreenJSON = nil
        cacheGeneration += 1
        recentScreen.clear(through: cacheGeneration)
        kept.clear()
        try? FileManager.default.removeItem(at: kept.settingsFile)
        hubSettings = [:]
    }

    // the thread list's view, for Bend to draw (the device's own choice)
    private func languageLocal(_ e: Engine) async {
        apply(await e.local("lang-preview", settings.langPreview ? "1" : ""))
        apply(await e.local("lang", settings.lang))
        apply(await e.local("lang-os", Locale.preferredLanguages.first ?? ""))
    }

    func language(_ v: String) {
        settings.lang = v
        Task { await languageLocal(engine) }
    }

    func hold(_ away: Bool, thread: ThreadView) {
        let page = readingPages[thread.id]
        let seen = away && page?.away == true ? page!.seen : (thread.readingSeen ?? "-")
        let count = away && page?.away == true ? page!.count : thread.entries.count
        readingPages[thread.id] = (seen, count, away)
        guard ready else { return }
        run { await $0.reading(thread.id, seen: seen, count: count, away: away) }
    }

    func usage(_ visible: Bool) {
        wantsUsage = visible
        guard ready else { return }
        run { await $0.local("usage", visible ? "1" : "") }
    }

    func threadList(_ v: String) {
        settings.threadList = v
        run { await $0.local("view", v) }
    }

    func foreground(_ yes: Bool) {
        active = yes
        if !yes { keep() }
        if yes, let s = screen { island?.show(s.islands ?? [], foreground: true) }
    }

    private func apply(_ out: Out?, sent: String? = nil) {
        guard let o = out else {
            if let t = sent, let i = outgoing.firstIndex(where: { $0.text == t }) { outgoing.remove(at: i) }
            return
        }
        for update in o.sync ?? [] {
            var status = conn[update.hub, default: HubConn()]
            status.stage = update.phase
            status.detail = update.detail
            if update.phase == "live" { status.phase = .online }
            else if update.phase == "failed" { status.phase = .offline }
            else { status.phase = .syncing }
            if conn[update.hub] != status { conn[update.hub] = status }
        }
        if requestDeadline != o.deadline {
            requestDeadline = o.deadline
            if let deadline = o.deadline {
                let delay = max(0, deadline / 1000 - Date().timeIntervalSince1970) + 0.01
                Task { [weak self] in
                    try? await Task.sleep(for: .seconds(delay))
                    guard let self, self.requestDeadline == deadline else { return }
                    self.apply(await self.engine.pulse())
                }
            }
        }
        let fresh = o.sync?.contains { $0.phase == "history" || $0.phase == "live" } == true
        if let s = o.screen, !cachedPresentation || fresh {
            cachedPresentation = false
            if typing == 0 { composer = s.thread?.draft ?? "" }
            screen = s
            if let text = o.screenJSON, !cleared {
                lastScreenJSON = text
                cacheGeneration += 1
                let generation = cacheGeneration, links = self.links, store = recentScreen
                cacheTask?.cancel()
                cacheTask = Task {
                    try? await Task.sleep(for: .milliseconds(300))
                    guard !Task.isCancelled else { return }
                    await Task.detached(priority: .utility) { store.save(text, links: links, generation: generation) }.value
                }
            }
            let label = s.lang?.rows.first?.label
            if label != langLabel { langLabel = label; refreshTexts() }
            if s.uber == nil { lastUber = nil; lastPlane = nil } else if s.plane != nil { lastUber = s.uber; lastPlane = s.plane }
            if let t = s.thread, !s.sel.isEmpty { remember(s.sel, t) }
            if s.hub != shownHub {
                shownHub = s.hub
                plots.reset()
            }
            if !started {
                started = true
                settings.migrate(theme: s.theme, view: s.view)
                if let t = AppTab(rawValue: settings.defaultTab), t != .settings { tab = t } else { tab = s.uber == nil ? .threads : .home }
                if !sentView { sentView = true; run { await $0.local("view", AppSettings.shared.threadList) } }
            }
            if let st = s.settings, !s.hub.isEmpty, hubSettings[s.hub] != st { hubSettings[s.hub] = st; keepHubSettings() }
            let nav = Self.nav(s)
            if let b = s.bot, !nav.isEmpty {
                rememberBot(nav, b)
                if let p = s.plane { seenPlanes[nav] = p }
            }
            let accepted = awaitedNav == nil || awaitedNav == nav || awaitedAt.timeIntervalSinceNow < -3
            if accepted {
                awaitedNav = nil
                if destination?.nav == nav { destination = nil }
                #if DEBUG
                tapFull(nav)
                #endif
                // Evaluate the tap even when nav changed (#68).
                let tappedPage = shows(tapped: s, nav)
                if nav != shownSel || tappedPage {
                    shownSel = nav
                    if nav.isEmpty {
                        // nothing selected: the pages of Bend's selection go, the app's own stay
                        let own = path.filter(Self.local)
                        if own != path { show(own) }
                    } else {
                        let dest = destination(s, nav)
                        if dest != tab { tab = dest }
                        show([nav])
                        if opened?.nav == nav { opened = nil }
                    }
                }
            }
            // New Thread: its page opens at once and waits there for the
            // hub's thread (the selection above replaces it); a failure
            // takes it away again
            let making = s.making ?? false
            if making != shownMaking {
                shownMaking = making
                if making { path = [Self.making] } else if path == [Self.making] { path = nav.isEmpty ? [] : [nav] }
            }
            island?.show(s.islands ?? [], foreground: active)
            #if DEBUG
            if let id = opening, let row = s.projects.lazy.flatMap(\.threads).first(where: { $0.id == id || $0.id.hasSuffix("|" + id) }) {
                opening = nil
                act("select", row.id)
            } else if let id = opening, !s.hub.isEmpty, !s.projects.isEmpty {
                // a thread not on show (a settled or archived shelf): by its hub
                opening = nil
                act("select", s.hub + "|" + id)
            }
            if let n = botting, let u = s.uber, u.name == n {
                botting = nil
                open(nav: u.nav ?? "", action: "uber", value: u.id, title: u.name, cat: u.cat, kind: "bot")
            } else if let n = botting, let b = s.bots.first(where: { $0.name == n }) {
                botting = nil
                open(nav: b.nav ?? "", action: b.act ?? (b.remote ? "remote" : "bot"), value: b.id, title: b.name, cat: b.cat, kind: b.remote ? "remote" : "bot")
            } else if let n = botting, let r = s.rooms.first(where: { $0.name == n }) {
                botting = nil
                open(nav: r.nav ?? "", action: "room", value: r.id, title: r.name, kind: "room")
            }
            if opening == nil, let v = viewing, let t = s.thread, !t.viewer.choices.isEmpty {
                viewing = nil
                act("view", v)
            }
            if opening == nil, botting == nil, let a = acting, !s.projects.isEmpty || s.bot != nil {
                acting = nil
                Task {
                    for var step in a.components(separatedBy: ";;") where !step.isEmpty {
                        // "!step": at once, not two seconds after the one before
                        if step.hasPrefix("!") { step.removeFirst() } else { try? await Task.sleep(for: .seconds(2)) }
                        let kv = step.split(separator: "=", maxSplits: 1).map(String.init)
                        if kv[0] == "@times" { NSLog("Backplane taps %@", tapTimes.joined(separator: "; ")); continue }
                        if kv[0] == "@open", kv.count > 1 {
                            let name = kv[1]
                            if let u = screen?.uber, u.name == name {
                                open(nav: u.nav ?? "", action: "uber", value: u.id, title: u.name, cat: u.cat, kind: "bot")
                            } else if let b = screen?.bots.first(where: { $0.name == name }) {
                                open(nav: b.nav ?? "", action: b.act ?? (b.remote ? "remote" : "bot"), value: b.id, title: b.name, cat: b.cat, kind: b.remote ? "remote" : "bot")
                            } else if let r = screen?.rooms.first(where: { $0.name == name }) {
                                open(nav: r.nav ?? "", action: "room", value: r.id, title: r.name, kind: "room")
                            }
                            continue
                        }
                        // "@tab=<home|threads|bots|routines|settings>": the tab bar's tap; "@push=<page>" pushes a page of this tab
                        if kv[0] == "@tab", kv.count > 1, let t = AppTab(rawValue: kv[1]) { switchTab(t); continue }
                        if kv[0] == "@push", kv.count > 1 { navigate(path + [kv[1]]); continue }
                        // "@back": what a swipe back does
                        if kv[0] == "@back" { navigate(Array(path.dropLast())); continue }
                        // "@attach": a noisy PNG big enough to go up in several pieces
                        if kv[0] == "@attach" {
                            let img = UIGraphicsImageRenderer(size: CGSize(width: 400, height: 400)).image { c in
                                for y in stride(from: 0, to: 400, by: 2) {
                                    for x in stride(from: 0, to: 400, by: 2) {
                                        UIColor(hue: .random(in: 0 ... 1), saturation: 0.8, brightness: 0.9, alpha: 1).setFill()
                                        c.fill(CGRect(x: x, y: y, width: 2, height: 2))
                                    }
                                }
                            }
                            if let d = img.pngData() { attach(d, name: "noise.png") }
                            continue
                        }
                        act(kv[0], kv.count > 1 ? kv[1].replacingOccurrences(of: "$SIZE", with: TermSheet.size()) : "")
                    }
                }
            }
            #endif
        }
        if o.screen != nil, let t = sent, let i = outgoing.firstIndex(where: { $0.text == t }) { outgoing.remove(at: i) }
        for c in o.cmds {
            switch c.type {
            case "send": if let d = Data(base64Encoded: c.data ?? "") { hubs[c.hub ?? ""]?.send(d) }
            case "copy": UIPasteboard.general.string = c.text ?? ""
            case "scroll": scrolls += 1
            // an action come due (the design history playing), sent as a tap would
            case "later":
                let action = c.action ?? "", value = c.value ?? "", ms = max(c.ms ?? 0, 0)
                if !action.isEmpty {
                    Task { [weak self] in
                        try? await Task.sleep(for: .milliseconds(Int(ms)))
                        self?.act(action, value)
                    }
                }
            case "jump": if let id = c.id, !id.isEmpty { jumping = (id, 4) }
            case "keep": Self.keep(c.thread ?? "", c.text ?? "")
            case "send-issued":
                if let i = outgoing.firstIndex(where: { $0.thread == c.thread && $0.text == c.text }) { outgoing.remove(at: i) }
            case "send-failed":
                if let i = outgoing.firstIndex(where: { $0.thread == c.thread && $0.text == c.text }) { outgoing.remove(at: i) }
                let thread = c.thread ?? "", text = c.text ?? ""
                sendFailure = thread
                if screen?.thread?.id != thread || composer.isEmpty {
                    Self.keep(thread, text)
                    if screen?.thread?.id == thread { composer = text }
                }
            // while asleep the hub's push carries the alert instead
            case "notify": if active { notifier.post(thread: c.thread ?? "", key: c.key ?? "", title: c.title ?? "", body: c.body ?? "") }
            default: break
            }
        }
        if fresh, contentReady, !earlyActions.isEmpty {
            let actions = earlyActions
            earlyActions.removeAll()
            for (action, value) in actions { act(action, value) }
        }
        if let j = jumping, let t = screen?.thread {
            if let e = t.entries.first(where: { $0.id == j.id || $0.id.hasSuffix("|" + j.id) }) {
                jumping = nil
                jump = Jump(id: e.id, n: (jump?.n ?? 0) + 1)
            } else if o.screen != nil {
                jumping = j.left > 1 ? (j.id, j.left - 1) : nil
            }
        }
    }
}

struct Jump: Equatable {
    let id: String
    let n: Int
}

struct Outgoing: Identifiable, Equatable {
    let id = UUID()
    let thread, text: String
}

struct TapDestination {
    let nav, title, cat, kind: String
}
