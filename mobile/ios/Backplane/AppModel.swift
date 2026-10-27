import Foundation
import Network
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
    // Display-only tint; temperatures and fractions come from the shared planner.
    var reflowThermal = SIMD4<Float>(0, 25, 0, 0)
    func reflowDefaults() async -> String { await engine.reflowDefaults() }
    func reflowAlloys() async -> String { await engine.reflowAlloys() }
    func reflow(_ profile: String, alloy: String) async -> String { await engine.reflow(profile, alloy: alloy) }
    // the client's state, kept for the next launch (StateStore)
    @ObservationIgnored private lazy var kept = StateStore()
    @ObservationIgnored private let recentScreen = ScreenCache()
    @ObservationIgnored private let presentationStore = PresentationStore()
    @ObservationIgnored private var presentationGeneration: UInt64 = 0
    private(set) var startup: [String: StartupPolicy] = [:]
    @ObservationIgnored private var thinReady: [String: (epoch: UInt64, nav: String)] = [:]
    @ObservationIgnored private var thinRequests: [String: ThinPresentationFence] = [:]
    @ObservationIgnored private var cachedPresentation = false
    @ObservationIgnored private var cachedReadiness = PresentationReadiness()
    @ObservationIgnored private var cacheGeneration: UInt64 = 0
    @ObservationIgnored private var cacheTask: Task<Void, Never>?
    @ObservationIgnored private var lastScreenJSON: String?
    @ObservationIgnored private var earlyActions: [(String, String)] = []
    @ObservationIgnored private var presentationIntent = PresentationIntent()
    private var viewerClose = ViewerCloseIntent()
    @ObservationIgnored private var viewerOpening: ViewerCloseIntent.Token?
    @ObservationIgnored private var viewerReceipt: (intent: UInt64, hub: String, epoch: UInt64)?
    private var viewerPublished: Viewer?
    @ObservationIgnored private var viewerCloseSending: ViewerCloseIntent.Token?
    @ObservationIgnored private var viewerClosedStamp = ViewerClosedStamp.load()
    @ObservationIgnored private var projectSearch = ProjectSearchIntent()
    @ObservationIgnored private var projectSearchOwner: UInt64 = 0
    @ObservationIgnored private var projectSearchGo: ProjectSearchIntent.Go?
    private(set) var projectSearchRevision = 0
    @ObservationIgnored private var localDrafts: [String: String] = [:]
    @ObservationIgnored private var restoredAt: [String: UInt64] = [:]
    @ObservationIgnored private var wantsUsage = false
    @ObservationIgnored private var readingPages: [String: (seen: String, count: Int, away: Bool)] = [:]
    var contentReady: Bool {
        guard ready, !cachedPresentation, let screen else { return false }
        if startup[screen.hub] == .compatibilityChosen { return conn[screen.hub]?.phase == .online && conn[screen.hub]?.stage == "live" }
        guard let mark = thinReady[screen.hub], mark.epoch == conn[screen.hub]?.epoch,
              conn[screen.hub]?.phase == .online else { return false }
        return mark.nav == (awaitedNav ?? Self.nav(screen))
    }
    // something changed since the state was last kept
    @ObservationIgnored private var dirty = false
    @ObservationIgnored private var hubs: [String: Hub] = [:]
    @ObservationIgnored private var connectGeneration: UInt64 = 0
    @ObservationIgnored private var pairingGeneration: UInt64 = 0
    @ObservationIgnored private var retiredPairings: [String: UInt64] = [:]
    @ObservationIgnored private var pendingPairingResets: Set<String> = []
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
    private(set) var composer = ""
    private var composerDrafts = ComposerDrafts()
    private var composerOwner = ""
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
    #if DEBUG
    @ObservationIgnored private var tapStart: (nav: String, at: UInt64, first: Int?, full: Int?)?
    private(set) var tapTimes: [String] = []
    #endif
    @ObservationIgnored private var seenOrder: [String] = []
    // messages sent from this phone the client has not answered yet: shown
    // at once as sending, dropped when the answer (with its own sending
    // row) is on screen
    private(set) var outgoing: [Outgoing] = []
    private(set) var mailroomReturn: MailroomReturnContext?
    @ObservationIgnored private var mailroomPositions: [MailroomPositionKey: TimelinePosition] = [:]
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
    #if DEBUG && targetEnvironment(simulator)
    private(set) var qualificationIncoming: [String: String] = [:]
    private(set) var qualificationDisconnected = UserDefaults.standard.bool(forKey: "qualification.connection.parked")
    var qualificationConnection: String {
        if qualificationDisconnected { return "parked" }
        if links.isEmpty { return "unpaired" }
        if contentReady { return "ready" }
        let key = screen?.hub ?? links.first.flatMap(Pairing.key) ?? ""
        return conn[key]?.phase == .offline ? "offline" : "connecting"
    }

    // IDs and typed state only. Never include links, drafts, message bodies,
    // Foundation errors or the presentation JSON in qualification output.
    var qualificationDetails: String {
        let key = screen?.hub ?? links.first.flatMap(Pairing.key) ?? ""
        let status = conn[key]
        let mark = thinReady[key]
        var values: [String: String] = [
            "hub": key, "socket": status.map { String(describing: $0.phase) } ?? "none",
            "epoch": status.map { String($0.epoch) } ?? "none",
            "failure": status?.failure?.reason ?? "none",
            "startup": startup[key].map { String(describing: $0) } ?? "none",
            "cached": String(cachedPresentation), "engineReady": String(ready),
            "contentReady": String(contentReady), "tab": tab.rawValue,
            "nav": screen.map(Self.nav) ?? "none", "awaited": awaitedNav ?? "none",
            "path": path.last ?? "none", "thread": screen?.thread?.id ?? "none",
            "bot": screen?.bot?.id ?? "none", "botTab": screen?.bot?.tab ?? "none",
            "viewer": screen?.thread?.viewer.open ?? "none",
            "viewerPresentation": viewerClose.current.map { String($0.presentation) } ?? "none",
            "viewerLocallyClosed": String(viewerClose.locallyClosed),
            "viewerClosePending": String(viewerClose.pending != nil),
            "viewerRequested": viewerOpening?.source.kind ?? "none",
            "viewerAcceptedIntent": viewerReceipt.map { String($0.intent) } ?? "none",
            "markEpoch": mark.map { String($0.epoch) } ?? "none", "markNav": mark?.nav ?? "none"
        ]
        for (key, value) in qualificationIncoming { values["incoming." + key] = value }
        guard let data = try? JSONSerialization.data(withJSONObject: values, options: [.sortedKeys]) else { return "unavailable" }
        return String(decoding: data, as: UTF8.self)
    }

    private func qualification(_ url: URL) -> Bool {
        guard url.scheme == "backplane", url.host == "qualification",
              let op = URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems?.first(where: { $0.name == "op" })?.value,
              ["disconnect", "reconnect"].contains(op) else { return false }
        qualificationDisconnected = op == "disconnect"
        UserDefaults.standard.set(qualificationDisconnected, forKey: "qualification.connection.parked")
        if qualificationDisconnected {
            connectGeneration &+= 1
            for (key, hub) in hubs {
                hub.stop()
                conn[key, default: HubConn()].epoch &+= 1
                conn[key, default: HubConn()].phase = .offline
                cachedReadiness.retire(key)
                thinReady[key] = nil
                thinRequests[key] = nil
                run { await $0.online(key, false, epoch: String(self.conn[key]?.epoch ?? 0)) }
            }
            hubs.removeAll()
            keep()
        } else { connect() }
        return true
    }
    #endif
    // when the app last left the front (a suspension may kill its sockets)
    @ObservationIgnored private var awayAt: Date?
    // network changes (Wi-Fi to cellular, back online): sockets are tried now
    @ObservationIgnored private let paths = NWPathMonitor()
    @ObservationIgnored private var lastPath: NWPath.Status?
    @ObservationIgnored private var lastInterfaces: [String] = []
    // drafts sent to Bend and not yet answered: until then a screen may
    // carry an older draft than the one on screen
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
        engine.onFlush = { [weak self] out, generation in
            Task { @MainActor [weak self] in self?.apply(out, generation: generation) }
        }
        #if DEBUG
        if let l = ProcessInfo.processInfo.environment["BACKPLANE_LINK"] { links = l.split(separator: " ").map(String.init) }
        #endif
        let cachedData = recentScreen.load(links: links, profile: Self.cid)
        let cachedScreen = cachedData.flatMap { try? JSONDecoder().decode(Screen.self, from: $0) }
        if cachedData != nil, cachedScreen == nil { NSLog("Backplane screen-cache miss_native_decode") }
        if let cached = cachedScreen {
            screen = cached
            reconcileViewer(cached)
            cachedPresentation = true
            let drafts = (try? JSONSerialization.jsonObject(with: Data((UserDefaults.standard.string(forKey: "drafts") ?? "{}").utf8))) as? [String: String] ?? [:]
            composerOwner = cached.thread?.id ?? ""
            if !composerOwner.isEmpty {
                _ = composerDrafts.receive(composerOwner, drafts[composerOwner] ?? cached.thread?.draft ?? "", edit: nil)
                composer = composerDrafts.text(for: composerOwner) ?? ""
            }
            lastUber = cached.uber
            lastPlane = cached.plane
            if let thread = cached.thread, !cached.sel.isEmpty { remember(cached.sel, thread) }
            let nav = Self.nav(cached)
            if !nav.isEmpty {
                tab = destination(cached, nav)
                stacks[tab] = [nav]
                shownSel = nav
                if let bot = cached.bot { rememberBot(nav, bot) }
            } else if let tab = AppTab(rawValue: settings.defaultTab), tab != .settings { self.tab = tab }
            else { tab = cached.uber == nil ? .threads : .home }
        }
        notifier.open = { [weak self] id in self?.act("select", id) }
        paths.pathUpdateHandler = { [weak self] path in
            let names = path.availableInterfaces.map(\.name)
            Task { @MainActor [weak self] in self?.pathChanged(path.status, names) }
        }
        paths.start(queue: .main)
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
        if let d = try? Data(contentsOf: Self.settingsFile), let m = try? JSONDecoder().decode([String: Settings].self, from: d) { hubSettings = m }
        let e = engine
        Task {
            apply(await e.start(Self.cid, UserDefaults.standard.string(forKey: "drafts") ?? "{}"))
            apply(await e.local("view", settings.threadList))
            await languageLocal(e)
            // Bounded choices only; a miss never opens the canonical StateStore.
            let paired = links, profile = Self.cid, store = presentationStore
            if let text = await Task.detached(priority: .userInitiated, operation: { store.load(links: paired, profile: profile) }).value, paired == links {
                let restored = await e.loadPresentation(text)
                if paired == links { apply(restored) }
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
                if ready {
                    let generation = pairingGeneration
                    apply(await e.tick(Int(Date().timeIntervalSince1970)), generation: generation)
                }
                keep()
            }
        }
        #if DEBUG && targetEnvironment(simulator)
        // CoreSimulator's pasteboard can be unavailable. A private launch
        // environment can exercise the normal pairing path without putting
        // the credential in URL command arguments or UI-test recordings.
        if let link = ProcessInfo.processInfo.environment["BACKPLANE_PAIR_LINK"] {
            pair(link)
        }
        #endif
    }

    // a new hub, or a new link to one already paired (it replaces the old)
    @discardableResult func pair(_ text: String) -> Bool {
        let l = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let k = Pairing.key(l) else { return false }
        endProjectSearch()
        cancelViewer()
        presentationIntent.clear()
        pairingGeneration = engine.invalidatePairing()
        retiredPairings[k] = pairingGeneration
        if links.contains(where: { Pairing.key($0) == k }) { forgetPresentation(k) }
        retireScreenCache()
        hubs.removeValue(forKey: k)?.stop()
        thinReady[k] = nil
        thinRequests[k] = nil
        cachedReadiness.retire(k)
        startup[k] = .discovering
        conn[k, default: HubConn()].epoch += 1
        conn[k, default: HubConn()].phase = .offline
        links = links.filter { Pairing.key($0) != k } + [l]
        save()
        notifier.setUp()
        connect()
        return true
    }

    func unpair(_ key: String) {
        endProjectSearch()
        cancelViewer()
        presentationIntent.clear()
        pairingGeneration = engine.invalidatePairing()
        retiredPairings[key] = pairingGeneration
        forgetPresentation(key)
        cachedReadiness.retire(key)
        thinReady[key] = nil
        thinRequests[key] = nil
        startup[key] = nil
        mailroomPositions = mailroomPositions.filter { $0.key.hub != key }
        if mailroomReturn?.hub == key { mailroomReturn = nil }
        retireScreenCache()
        links = links.filter { Pairing.key($0) != key }
        save()
        connect()
    }

    // An explicitly replaced pairing is a new account boundary, even when its
    // address is unchanged. Network reconnects preserve these choices instead.
    private func forgetPresentation(_ key: String) {
        if viewerClosedStamp?.hub == key {
            viewerClosedStamp = nil
            ViewerClosedStamp.clear()
        }
        pendingPairingResets.insert(key)
        let prefix = key + "|"
        seen = seen.filter { !$0.key.hasPrefix(prefix) }
        seenOrder.removeAll { $0.hasPrefix(prefix) }
        seenBots = seenBots.filter { !$0.key.hasPrefix(prefix) }
        seenPlanes = seenPlanes.filter { !$0.key.hasPrefix(prefix) }
        botOrder.removeAll { $0.hasPrefix(prefix) }
        localDrafts = localDrafts.filter { !$0.key.hasPrefix(prefix) }
        composerDrafts.forget(key)
        if composerOwner.hasPrefix(prefix) { composerOwner = ""; composer = "" }
        readingPages = readingPages.filter { !$0.key.hasPrefix(prefix) }
        earlyActions.removeAll { $0.1.hasPrefix(prefix) }
        outgoing.removeAll { $0.thread.hasPrefix(prefix) }
        hubSettings[key] = nil
        keepHubSettings()
        let defaults = UserDefaults.standard
        if let text = defaults.string(forKey: "drafts"),
           let drafts = try? JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: String],
           let data = try? JSONSerialization.data(withJSONObject: drafts.filter { !$0.key.hasPrefix(prefix) }),
           let filtered = String(data: data, encoding: .utf8) { defaults.set(filtered, forKey: "drafts") }
        for tab in Array(stacks.keys) { stacks[tab]?.removeAll { $0.hasPrefix(prefix) || $0 == "@hub:" + key } }
        if screen?.hub == key {
            earlyActions.removeAll()
            screen = nil
            composer = ""
            destination = nil
            awaitedNav = nil
            pending = nil
            shownSel = ""
            shownHub = ""
            lastUber = nil
            lastPlane = nil
            plots.reset()
        }
    }

    private func save() {
        UserDefaults.standard.set(links, forKey: "links")
        UserDefaults.standard.removeObject(forKey: "link")
    }

    // an APNs token for this device's alerts (from the app delegate)
    func enableNotifications() async { await notifier.enable() }

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
        connectGeneration &+= 1
        let generation = connectGeneration
        #if DEBUG && targetEnvironment(simulator)
        guard !qualificationDisconnected else { return }
        #endif
        guard ready else { return }
        var keyed: [String: String] = [:]
        var keys: [String] = []
        for l in links {
            if let k = Pairing.key(l), keyed[k] == nil { keyed[k] = l; keys.append(k) }
        }
        for (k, h) in hubs where keyed[k] == nil {
            cachedReadiness.retire(k)
            conn[k, default: HubConn()].phase = .offline
            conn[k, default: HubConn()].epoch += 1
            h.stop()
            hubs[k] = nil
        }
        let e = engine
        let fresh = keys.contains { hubs[$0] == nil }
        Task {
            guard generation == connectGeneration else { return }
            for key in pendingPairingResets.sorted() {
                _ = await e.forgetHub(key)
                guard generation == connectGeneration else { return }
                pendingPairingResets.remove(key)
            }
            let reconciled = await e.hubs(keys)
            guard generation == connectGeneration else { return }
            e.resumePairing(pairingGeneration)
            apply(reconciled)
            let view = await e.local("view", settings.threadList)
            guard generation == connectGeneration else { return }
            apply(view)
            await languageLocal(e)
            guard generation == connectGeneration else { return }
            let usage = await e.local("usage", wantsUsage ? "1" : "")
            guard generation == connectGeneration else { return }
            apply(usage)
            for (thread, page) in readingPages {
                let reading = await e.reading(thread, seen: page.seen, count: page.count, away: page.away)
                guard generation == connectGeneration else { return }
                apply(reading)
            }
            for (k, l) in keyed where hubs[k] == nil && links.contains(l) { open(k, l) }
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
                guard let self else { return Hub.Next.stop }
                let epoch = String(self.conn[key]?.epoch ?? 0)
                let pairing = self.pairingGeneration
                self.startup[key] = .discovering
                let found = await ViewCapability.find(link)
                guard self.links.contains(link), self.conn[key]?.epoch == UInt64(epoch) else { return .stop }
                guard pairing == self.pairingGeneration else { return .later(.connection) }
                // a hub that is down or restarting is asked again after the
                // backoff; only a hub that answers without views parks
                if case .retry(let failure) = found { return .later(failure) }
                guard found == .supported else {
                    self.startup[key] = .compatibilityRequired
                    self.conn[key, default: HubConn()].failure = nil
                    self.conn[key, default: HubConn()].phase = .offline
                    NSLog("Backplane view capability_required")
                    return .stop
                }
                self.startup[key] = .thin
                let connected = await e.connection(key, epoch, mode: "view-v1")
                guard self.links.contains(link), self.conn[key]?.epoch == UInt64(epoch) else { return .stop }
                self.apply(connected, generation: pairing)
                guard pairing == self.pairingGeneration else { return .later(.connection) }
                self.projectSearchRevision += 1
                if let pending = self.earlyActions.last(where: { $0.0 != "tz-report" && ($0.1.hasPrefix(key + "|") || ($0.1.isEmpty && self.screen?.hub == key)) }) {
                    self.earlyActions.removeAll { $0.0 == pending.0 && $0.1 == pending.1 }
                    let selectingPairing = self.pairingGeneration
                    let selected = await e.act(pending.0, pending.1)
                    guard self.links.contains(link), self.conn[key]?.epoch == UInt64(epoch) else { return .stop }
                    self.apply(selected, generation: selectingPairing)
                    guard selectingPairing == self.pairingGeneration else { return .later(.connection) }
                } else if let cached = self.screen, cached.hub == key {
                    let selectingPairing = self.pairingGeneration
                    let selected = await e.act("select", cached.sel)
                    guard self.links.contains(link), self.conn[key]?.epoch == UInt64(epoch) else { return .stop }
                    self.apply(selected, generation: selectingPairing)
                    guard selectingPairing == self.pairingGeneration else { return .later(.connection) }
                }
                return Pairing.viewSocket(link).map { Hub.Next.go($0) } ?? Hub.Next.stop
            },
            onConnecting: { [weak self] in
                self?.viewerCloseSending = nil
                self?.viewerReceipt = nil
                self?.endProjectSearch()
                if self?.screen?.hub == key { self?.presentationIntent.clear() }
                self?.cachedReadiness.retire(key)
                self?.thinReady[key] = nil
                self?.thinRequests[key] = nil
                self?.conn[key, default: HubConn()].phase = .connecting
                self?.conn[key, default: HubConn()].epoch += 1
            },
            onOpen: { [weak self] in
                self?.conn[key, default: HubConn()].failure = nil
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
            onClose: { [weak self] failure in
                if self?.screen?.hub == key { self?.presentationIntent.clear() }
                if self?.conn[key]?.failure != failure {
                    NSLog("Backplane connection retry cause=%@ http=%@", failure.reason, failure.code)
                }
                self?.conn[key, default: HubConn()].failure = failure
                self?.cachedReadiness.retire(key)
                self?.thinReady[key] = nil
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
        guard hubs[key] != nil, links.contains(where: { Pairing.key($0) == key }) else { return }
        dirty = true
        conn.received(key, bytes: d.count, at: Date())
        let e = engine
        let person = e.priorityFrame(d)
        let epoch = conn[key]?.epoch ?? 0
        let pairing = pairingGeneration
        Task {
            guard pairing == pairingGeneration else { return }
            let result = await e.recv(key, d.base64EncodedString(), epoch: String(epoch), person: person)
            guard pairing == pairingGeneration else { apply(result, generation: pairing); return }
            guard hubs[key] != nil, links.contains(where: { Pairing.key($0) == key }), conn[key]?.epoch == epoch else { return }
            for update in result?.sync ?? [] where update.hub == key {
                let thin = startup[key] == .thin
                observeThinPresenting(update, epoch: epoch)
                let presented = thin && currentThinReply(update, epoch: epoch)
                if presented {
                    let nav = awaitedNav ?? (cachedPresentation ? screen.map(Self.nav) : nil) ?? result?.screen.map(Self.nav) ?? screen.map(Self.nav) ?? ""
                    thinReady[key] = (epoch, nav)
                }
                if cachedPresentation, !thin || presented {
                    cachedReadiness.observe(hub: key, epoch: epoch, phase: update.phase)
                }
            }
            let needsFrame = cachedPresentation && result?.screen == nil && cachedReadiness.requestFrame { key in
                guard hubs[key] != nil, links.contains(where: { Pairing.key($0) == key }),
                      let status = conn[key], status.phase != .offline, status.phase != .connecting else { return nil }
                return status.epoch
            }
            apply(result)
            if startup[key] == .compatibilityChosen, cachedPresentation, screen?.hub == key, restoredAt[key] != epoch,
               result?.sync?.contains(where: { $0.hub == key && ($0.phase == "recent" || $0.phase == "live") }) == true {
                restoredAt[key] = epoch
                let selection = earlyActions.last(where: { $0.0 == "select" })?.1 ?? screen?.sel ?? ""
                let selectingPairing = pairingGeneration
                let selected = await e.act("select", selection)
                guard conn[key]?.epoch == epoch, hubs[key] != nil else { return }
                apply(selected, generation: selectingPairing)
                guard selectingPairing == pairingGeneration else { return }
            }
            if needsFrame { run { await $0.screen() } }
        }
    }

    private func run(_ f: @escaping (Engine) async -> Out?) {
        let e = engine
        let generation = pairingGeneration
        Task {
            guard generation == pairingGeneration else { return }
            let out = await f(e)
            apply(out, generation: generation)
        }
    }

    // what was typed in the secret box's value field (SetFieldView, a vault
    // field): held here only until Save, then handed to the engine once
    @ObservationIgnored var vaultText = ""

    func act(_ action: String, _ value: String = "") {
        if action == "view", value.isEmpty {
            if let token = viewerClose.current { closeViewer(token) }
            return
        }
        if action == "select" || Self.opens.contains(action) { cancelViewer() }
        let viewerRequest = action == "view" ? beginViewerOpen(value) : nil
        if action == "select" || Self.opens.contains(action) || (action == "proj-find" && value == "off") { endProjectSearch() }
        if action == "select" || Self.opens.contains(action) { presentationIntent.clear() }
        // the secret box's Save: the value joins the box's name, purpose and
        // thread and leaves in one secret.set request (Engine.secret)
        if action == "secret-save" {
            let v = vaultText
            vaultText = ""
            guard contentReady else { return }
            run { await $0.secret(value + "\u{1f}" + v) }
            return
        }
        // a web address a settings row offers (Antigravity's sign-in):
        // opened in the browser here, never sent to the engine
        if action == "open-url" {
            if let u = URL(string: value), u.scheme == "https" || u.scheme == "http" { UIApplication.shared.open(u) }
            return
        }
        if action == "select" {
            if value.isEmpty || links.contains(where: { value.hasPrefix((Pairing.key($0) ?? "") + "|") }) {
                awaitedNav = value
                awaitedAt = Date()
            }
        }
        if action == "select" || Self.opens.contains(action) { retireScreenCache() }
        if !contentReady {
            if (action == "view" || action == "bot-tab"), let s = screen,
               !cachedPresentation, !s.hub.isEmpty, startup[s.hub] == .thin,
               hubs[s.hub] != nil, let status = conn[s.hub], status.phase == .syncing || status.phase == .online,
               thinRequests[s.hub] != nil, (awaitedNav == nil || awaitedNav == Self.nav(s)),
               ((action == "view" && s.thread != nil) || (action == "bot-tab" && s.bot != nil)) {
                presentationIntent.offer(action, value, context: PresentationIntent.Context(hub: s.hub, selection: s.sel,
                    nav: Self.nav(s), epoch: status.epoch, pairing: pairingGeneration))
                return
            }
            guard ["select", "bot", "room", "remote", "uber", "uber-bot", "tz-report"].contains(action) else { return }
            earlyActions.removeAll { $0.0 == action }
            let targetHub = links.compactMap(Pairing.key).first { value.hasPrefix($0 + "|") } ?? screen?.hub
            let configured = targetHub.map { startup[$0] == .thin || startup[$0] == .compatibilityChosen } ?? false
            if ready, configured, action != "tz-report" {
                let e = engine
                let generation = pairingGeneration
                Task {
                    guard generation == pairingGeneration else { return }
                    let out: Out?
                    if let viewerRequest { out = await e.viewerAct(value, token: viewerRequest) }
                    else { out = await e.act(action, value) }
                    apply(out, generation: generation)
                    guard generation == pairingGeneration else { return }
                    dirty = true
                    keep()
                }
            } else {
                earlyActions.append((action, value))
            }
            return
        }
        if Self.opens.contains(action) { tapped = value }
        let navigation = action == "select" || Self.opens.contains(action)
        let e = engine
        let generation = pairingGeneration
        Task {
            guard generation == pairingGeneration else { return }
            let out: Out?
            if let viewerRequest { out = await e.viewerAct(value, token: viewerRequest) }
            else { out = await e.act(action, value) }
            apply(out, generation: generation)
            guard generation == pairingGeneration else { return }
            if navigation { dirty = true; keep() }
        }
    }

    func viewerChoices(for thread: String) -> [Choice] {
        guard !cachedPresentation, let s = screen, let current = s.thread, current.id == thread,
              Self.nav(s) == thread, path.last == thread, awaitedNav == nil || awaitedNav == thread,
              hubs[s.hub] != nil, links.contains(where: { Pairing.key($0) == s.hub }),
              let status = conn[s.hub], status.phase == .online || status.phase == .syncing else { return [] }
        return current.viewer.choices
    }
    func openViewer(_ kind: String, for thread: String) {
        // Recheck the actual owner at the tap boundary; a cached destination
        // must never send an action through another ready conversation.
        guard viewerChoices(for: thread).contains(where: { $0.value == kind }) else { return }
        act("view", kind)
    }

    private func viewerOwner(_ s: Screen) -> ViewerCloseIntent.Owner {
        .init(hub: s.hub, nav: Self.nav(s), pairing: pairingGeneration)
    }
    private func viewerSource(_ v: Viewer) -> ViewerCloseIntent.Source {
        .init(kind: v.open, key: v.key, layers: v.layers)
    }
    private func cancelViewer() {
        viewerClose.cancel()
        viewerOpening = nil
        viewerReceipt = nil
        viewerPublished = nil
        viewerCloseSending = nil
        engine.viewerToken(nil)
    }
    private func beginViewerOpen(_ kind: String) -> ViewerCloseIntent.Token? {
        guard let s = screen, let thread = s.thread, !s.hub.isEmpty else { return nil }
        viewerClosedStamp = nil
        ViewerClosedStamp.clear()
        let token = viewerClose.reserve(owner: viewerOwner(s), source: .init(kind: kind, key: thread.viewer.key, layers: thread.viewer.layers))
        viewerOpening = token
        viewerReceipt = nil
        viewerCloseSending = nil
        engine.viewerToken(token)
        return token
    }
    private func reconcileViewer(_ s: Screen) {
        guard let thread = s.thread else { cancelViewer(); return }
        let owner = viewerOwner(s), source = viewerSource(thread.viewer)
        if let requested = viewerOpening, requested.owner == owner {
            // A same-kind cached/refresh picture is not an acknowledgement of
            // this open. Receipt comes only from its accepted host operation.
            guard source.kind == requested.source.kind else { return }
            if startup[owner.hub] == .thin {
                guard let receipt = viewerReceipt, receipt.intent == requested.presentation,
                      receipt.hub == owner.hub, receipt.epoch == conn[owner.hub]?.epoch else { return }
            } else {
                guard startup[owner.hub] == .compatibilityChosen, !cachedPresentation,
                      conn[owner.hub]?.phase == .online, conn[owner.hub]?.stage == "live" else { return }
            }
            let accepted = ViewerCloseIntent.Token(owner: owner, source: source, presentation: requested.presentation)
            viewerClose.accept(accepted)
            viewerPublished = thread.viewer
            viewerOpening = nil
            viewerReceipt = nil
            engine.viewerToken(accepted)
            return
        }
        guard !source.kind.isEmpty else { cancelViewer(); return }
        if let current = viewerClose.current, current.owner == owner, viewerClose.locallyClosed {
            // Passive source/revision changes do not reopen a locally closed
            // panel. Conservatively retire its old deferred remote close.
            if current.source != source { viewerClose.sent(current); engine.viewerToken(nil) }
            return
        }
        viewerPublished = thread.viewer
        if viewerClose.current?.owner != owner || viewerClose.current?.source != source {
            let token = viewerClose.open(owner: owner, source: source)
            if let stamp = viewerClosedStamp, stamp.matches(owner, identity: ScreenCache.identity(links, profile: Self.cid)) {
                viewerClose.close(token)
                if stamp.source == source { engine.viewerToken(token, closing: true) }
                else { viewerClose.sent(token); engine.viewerToken(nil) }
            } else { engine.viewerToken(token) }
        }
    }
    func viewerData(for thread: String) -> Viewer? {
        guard viewerClose.current?.owner.nav == thread else { return nil }
        return viewerPublished
    }
    func visibleViewerToken(for thread: String) -> ViewerCloseIntent.Token? {
        guard !viewerClose.locallyClosed, let token = viewerClose.current, token.owner.nav == thread,
              let data = viewerData(for: thread), !data.open.isEmpty, token.source == viewerSource(data) else { return nil }
        return token
    }
    func closeViewer(_ token: ViewerCloseIntent.Token) {
        guard viewerClose.close(token) else { return }
        viewerOpening = nil
        viewerReceipt = nil
        presentationIntent.clear()
        if let identity = ScreenCache.identity(links, profile: Self.cid) {
            let stamp = ViewerClosedStamp(identity: identity, hub: token.owner.hub, nav: token.owner.nav, source: token.source)
            viewerClosedStamp = stamp
            stamp.save()
        }
        engine.viewerToken(token, closing: true)
        advanceViewerClose()
    }
    private func advanceViewerClose() {
        guard let token = viewerClose.pending, viewerCloseSending == nil, let s = screen, let thread = s.thread,
              viewerClose.canSend(token, owner: viewerOwner(s), source: viewerSource(thread.viewer), ready: contentReady) else { return }
        viewerCloseSending = token
        let generation = pairingGeneration, e = engine
        Task {
            guard generation == pairingGeneration, viewerClose.current == token, viewerClose.locallyClosed else {
                if viewerCloseSending == token { viewerCloseSending = nil }
                return
            }
            let out = await e.viewerAct("", token: token)
            apply(out, generation: generation)
            // A valid Out can mean merely queued/deferred. Keep the close
            // pending until an accepted empty viewer or an epoch retires it.
            if out == nil, viewerCloseSending == token { viewerCloseSending = nil }
        }
    }

    // Search input has a separate lifetime from mutations. A refresh can
    // coalesce edits, but must not lose the final complete query or clear.
    func beginProjectSearch(_ text: String) -> ProjectSearchIntent.Query {
        if showingProjectSearch,
           let current = projectSearch.editing ?? projectSearch.committed ?? projectSearchGo?.query,
           current.owner.generation == projectSearchOwner { return current }
        endProjectSearch()
        return projectSearch.begin(text: text, owner: .init(generation: projectSearchOwner))
    }

    func endProjectSearch(scope: UInt64? = nil) {
        if let scope {
            // View disappearance cannot cancel Return's committed selection.
            guard projectSearch.editing?.scope == scope else { return }
            projectSearch.disappear(scope: scope)
            run { await $0.projectSearchEnd(scope: Int(scope)) }
        } else {
            let scopes = Set([projectSearch.editing?.scope, projectSearch.committed?.scope, projectSearchGo?.query.scope].compactMap { $0 })
            projectSearch.cancel()
            projectSearchGo = nil
            projectSearchOwner += 1
            for scope in scopes { run { await $0.projectSearchEnd(scope: Int(scope)) } }
        }
    }

    private var showingProjectSearch: Bool {
        tab == .threads && path.isEmpty && screen?.search != nil && !links.isEmpty
    }

    private func sendProjectSearch(_ query: ProjectSearchIntent.Query) {
        let generation = pairingGeneration
        Task {
            guard generation == pairingGeneration, showingProjectSearch,
                  query.owner.generation == projectSearchOwner,
                  projectSearch.editing == query || projectSearch.committed == query else { return }
            if let hub = screen?.hub, startup[hub] == .compatibilityChosen {
                apply(await engine.act("proj-find-q", query.text), generation: generation)
            } else {
                apply(await engine.projectSearchQuery(scope: Int(query.scope), edit: Int(query.edit), text: query.text), generation: generation)
            }
        }
    }

    func searchProjects(_ text: String, scope: UInt64) -> UInt64? {
        guard showingProjectSearch else { return nil }
        let old = projectSearch.committed?.scope ?? projectSearchGo?.query.scope
        let owner = ProjectSearchIntent.Owner(generation: projectSearchOwner)
        let query: ProjectSearchIntent.Query?
        if projectSearch.editing == nil, projectSearchGo?.query.scope == scope {
            let initial = projectSearch.begin(text: text, owner: owner)
            query = projectSearch.edit(text, scope: initial.scope, owner: owner)
        } else if projectSearch.editing == nil, projectSearch.committed == nil, projectSearchGo == nil {
            // A reconnect can retire the lifetime while its field remains
            // on screen. The next visible edit starts a fresh owner scope.
            let initial = projectSearch.begin(text: text, owner: owner)
            query = projectSearch.edit(text, scope: initial.scope, owner: owner)
        } else { query = projectSearch.edit(text, scope: scope, owner: owner) }
        guard let query else { return nil }
        if let old, old != query.scope {
            projectSearchGo = nil
            run { await $0.projectSearchEnd(scope: Int(old)) }
        }
        sendProjectSearch(query)
        return query.scope
    }

    func resumeProjectSearch(_ text: String, scope: UInt64) -> UInt64? {
        guard projectSearch.committed == nil, projectSearchGo == nil else { return nil }
        return searchProjects(text, scope: scope)
    }

    func submitProjectSearch(_ text: String, scope: UInt64) {
        guard showingProjectSearch else { return }
        // Capture the final text even if Return arrives before onChange.
        var submitting = scope
        if projectSearch.editing?.text != text {
            guard let latest = searchProjects(text, scope: scope) else { return }
            submitting = latest
        }
        guard let current = projectSearch.editing,
              current.scope == submitting,
              let query = projectSearch.submit(scope: current.scope, owner: .init(generation: projectSearchOwner)) else { return }
        sendProjectSearch(query)
        advanceProjectSearch()
    }

    private func advanceProjectSearch() {
        guard showingProjectSearch, projectSearchGo == nil,
              let query = projectSearch.committed, query.owner.generation == projectSearchOwner else { return }
        if let hub = screen?.hub, startup[hub] == .thin, let go = projectSearch.forward(query) {
            projectSearchGo = go
            run { await $0.projectSearchGo(scope: Int(query.scope), edit: Int(query.edit), text: query.text, project: "") }
            return
        }
        guard contentReady, let hub = screen?.hub, startup[hub] == .compatibilityChosen,
              screen?.search?.query == query.text else { return }
        // The explicitly chosen compatibility lane keeps its existing
        // action path, after the accepted full query is visible.
        guard let go = projectSearch.accept(query, projects: screen?.projects.map(\.id) ?? []) else { return }
        projectSearchGo = go
        run { await $0.act("proj-go", go.project) }
    }

    // Synchronous acceptance for the real-drag latch. An inert cached page
    // may remain readable while its current presentation is being refreshed.
    func loadEarlier(_ thread: String) -> Bool {
        guard contentReady, screen?.thread?.id == thread else { return false }
        act("earlier", "")
        return true
    }

    func moreMessage(hub: String, thread: String, entry: String, partial: MessagePartial, live: Bool = false) {
        guard contentReady, startup[hub] == .thin, partial.canRequest, !entry.isEmpty,
              let current = screen, current.hub == hub, let shown = current.thread, shown.id == thread else { return }
        let actual = live ? shown.livePartial : shown.entries.first(where: { $0.id == entry })?.partial
        guard actual == partial, !live || actual?.id == entry,
              let data = try? JSONSerialization.data(withJSONObject: ["entryId": entry, "cursor": partial.cursor]),
              let value = String(data: data, encoding: .utf8) else { return }
        act("thin-message-more", value)
    }

    func mailroom(_ mailroom: MailroomModel, _ action: String, _ value: String = "",
                  draft: String? = nil, recipient: String? = nil, quiet: Bool = false) {
        guard contentReady, links.contains(where: { Pairing.key($0) == mailroom.hub }) else { return }
        let request = MailroomAction.request(mailroom, action, value, draft: draft, recipient: recipient)
        guard !request.isEmpty else { return }
        run { engine in
            if quiet { return await engine.quiet("mailroom-action", request) }
            return await engine.act("mailroom-action", request)
        }
    }

    func mailroomPosition(_ mailroom: MailroomModel, conversation: String) -> TimelinePosition {
        let list: MailroomListWindow? = conversation.isEmpty
            ? MailroomListWindow(filter: mailroom.filter, query: mailroom.query, first: mailroom.first) : nil
        let key = MailroomPositionKey(hub: mailroom.hub, origin: mailroom.origin ?? "",
                                      owner: mailroom.owner, conversation: conversation, list: list)
        if let position = mailroomPositions[key] { return position }
        // A list window change starts at its own beginning. Retain only the
        // current list window for this owner; conversation/source-return
        // anchors and other owners keep their existing positions.
        mailroomPositions = mailroomPositions.filter { old, _ in
            if old.hub == key.hub && old.origin != key.origin { return false }
            if list != nil && old.hub == key.hub && old.origin == key.origin &&
                old.owner == key.owner && old.conversation.isEmpty { return old.list == list }
            return true
        }
        let position = TimelinePosition()
        mailroomPositions[key] = position
        return position
    }

    func openMailroomSource(_ mailroom: MailroomModel, uri: String) {
        guard contentReady, let url = URL(string: uri), !uri.isEmpty else { return }
        if open(url, sourceHub: mailroom.hub) {
            mailroomReturn = MailroomReturnContext(hub: mailroom.hub, owner: mailroom.owner)
        }
    }

    func backToMailroom() {
        guard let back = mailroomReturn,
              links.contains(where: { Pairing.key($0) == back.hub }) else { return }
        // Existing bot navigation restores that hub's Mailv conversation and
        // filter; its owner-qualified native position survives view recreation.
        mailroomReturn = nil
        act("uber-bot", back.hub + "|" + back.owner + "|mail")
    }

    // where the hub in focus serves a path ("/img?path=…"), with its token
    func web(_ path: String) -> URL? {
        guard let s = screen, let l = links.first(where: { Pairing.key($0) == s.hub }) else { return nil }
        return Pairing.web(l, path)
    }

    // a file for the next message: one POST /attach to the thread's hub with
    // the file as the body (no base64, no pieces through the engine); the
    // hub's answer goes back as "attach-end", as an attach.put's would
    func attach(_ data: Data, name: String, for owner: String? = nil) {
        guard let owner = owner ?? screen?.thread?.id, canSendComposer(for: owner) else { return }
        guard !data.isEmpty, let s = screen, let named = s.thread?.id,
              let l = links.first(where: { Pairing.key($0) == s.hub }) else { return }
        let epoch = conn[s.hub]?.epoch
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
        actComposer("attach-up", up, for: owner)
        guard data.count <= s.thread?.cap ?? 10_485_760 else { return }
        Task {
            var answer = ""
            do {
                let (body, _) = try await URLSession.shared.upload(for: req, from: data)
                answer = String(data: body, encoding: .utf8) ?? ""
            } catch {
                answer = ""
            }
            // Finishing an upload never applies its result to a replacement
            // pairing or to a different conversation opened in the meantime.
            guard canEditComposer(for: owner), screen?.hub == s.hub, screen?.thread?.id == named,
                  conn[s.hub]?.epoch == epoch, links.contains(l) else { return }
            let answered = (try? JSONSerialization.jsonObject(with: Data(answer.utf8))) is [String: Any]
            if !answered {
                let o: [String: Any] = ["uiError": "upload.network", "thread": thread, "upload": true]
                answer = (try? JSONSerialization.data(withJSONObject: o)).flatMap { String(data: $0, encoding: .utf8) } ?? ""
            }
            actComposer("attach-end", answer, for: owner)
        }
    }

    // Every row opens through this point; Bend's nav is the destination key.
    func open(nav: String, action: String, value: String, title: String, cat: String = "", kind: String = "thread") {
        // a row from an older bridge has no nav: its page opens when its screen comes
        guard !nav.isEmpty else { act(action, value); return }
        guard path.last != nav, pending?.last != nav else { return }
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
        guard contentReady else { return }
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

    func composerText(for thread: ThreadView) -> String {
        composerDrafts.text(for: thread.id) ?? localDrafts[thread.id] ?? thread.draft
    }

    func canEditComposer(for thread: String) -> Bool {
        guard !thread.isEmpty, let s = screen, s.thread?.id == thread,
              Self.nav(s) == thread, path.last == thread,
              awaitedNav == nil || awaitedNav == thread,
              links.contains(where: { Pairing.key($0) == s.hub }) else { return false }
        if let bot = s.bot, bot.kind != "bot" || (bot.tab ?? "chat") != "chat" { return false }
        return true
    }
    func canSendComposer(for thread: String) -> Bool { canEditComposer(for: thread) && contentReady }
    func composerStatus(for thread: String) -> HubConn? {
        guard let split = thread.firstIndex(of: "|") else { return nil }
        return conn[String(thread[..<split])]
    }

    private func publishDraft(_ thread: String, text: String, edit: UInt64) {
        let generation = pairingGeneration
        engine.draftAtNow(thread, text: text, edit: edit, generation: generation) { [weak self] out in
            Task { @MainActor [weak self] in self?.apply(out, generation: generation) }
        }
    }
    func editComposer(_ text: String, for thread: String) {
        guard canEditComposer(for: thread) else { return }
        let edit = composerDrafts.change(thread, text)
        composerOwner = thread
        composer = text
        Self.keep(thread, text)
        // This private write never sends a network request, even while offline.
        localDrafts[thread] = text
        publishDraft(thread, text: text, edit: edit)
    }
    func sendComposer(_ action: String = "send", for thread: String) {
        guard canSendComposer(for: thread), ["send", "send-alt"].contains(action) else { return }
        let text = composerDrafts.text(for: thread) ?? ""
        guard !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
        sendFailure = nil
        let edit = composerDrafts.edit(for: thread), generation = pairingGeneration
        outgoing.append(Outgoing(thread: thread, text: text.trimmingCharacters(in: .whitespacesAndNewlines)))
        // Enqueue before clearing so the captured message precedes later edits.
        engine.sendAtNow(thread, action: action, text: text, edit: edit, generation: generation) { [weak self] out in
            Task { @MainActor [weak self] in
                guard let self else { return }
                if out == nil, generation == self.pairingGeneration { self.restoreComposer(thread, text: text, edit: edit) }
                self.apply(out, generation: generation)
            }
        }
        let clearedEdit = composerDrafts.change(thread, "")
        composer = ""
        localDrafts[thread] = ""
        Self.keep(thread, "")
        publishDraft(thread, text: "", edit: clearedEdit)
    }
    func interruptComposer(for thread: String) {
        guard canSendComposer(for: thread) else { return }
        let generation = pairingGeneration
        engine.sendAtNow(thread, action: "interrupt", text: "", edit: composerDrafts.edit(for: thread), generation: generation) { [weak self] out in
            Task { @MainActor [weak self] in self?.apply(out, generation: generation) }
        }
    }
    func actComposer(_ action: String, _ value: String = "", for thread: String) {
        guard action == "attach-end" ? canEditComposer(for: thread) : canSendComposer(for: thread) else { return }
        let generation = pairingGeneration
        engine.composerActNow(thread, action: action, value: value, generation: generation) { [weak self] out in
            Task { @MainActor [weak self] in self?.apply(out, generation: generation) }
        }
    }
    private func restoreComposer(_ thread: String, text: String, edit: UInt64) {
        outgoing.removeAll { $0.thread == thread && $0.text == text.trimmingCharacters(in: .whitespacesAndNewlines) }
        sendFailure = thread
        guard let next = composerDrafts.failed(thread, text, sentEdit: edit) else { return }
        localDrafts[thread] = text
        Self.keep(thread, text)
        if composerOwner == thread { composer = text }
        publishDraft(thread, text: text, edit: next)
    }
    // Existing completion helpers use the current owner's private editor.
    func send(_ action: String = "send") {
        guard let thread = screen?.thread?.id else { return }
        if action == "interrupt" { interruptComposer(for: thread) } else { sendComposer(action, for: thread) }
    }
    func draft(_ text: String) {
        guard let thread = screen?.thread?.id else { return }
        editComposer(text, for: thread)
    }

    // Both external URL entry and links tapped in a message use shared Bend
    // reference resolution, with the message's hub kept outside the URI.
    @discardableResult
    func open(_ url: URL, sourceHub: String? = nil) -> Bool {
        #if DEBUG && targetEnvironment(simulator)
        if qualification(url) { return true }
        #endif
        switch InternalURLRoute.route(url) {
        case let .reference(uri, origin):
            act("ref-open", InternalURLRoute.request(uri, origin: origin, sourceHub: sourceHub))
            return true
        case let .pair(link):
            pair(link)
            return true
        case .ignored: return true
        case .external: return false
        }
    }

    private func retireScreenCache() {
        cacheTask?.cancel()
        lastScreenJSON = nil
        cacheGeneration += 1
        recentScreen.clear(through: cacheGeneration)
        presentationGeneration += 1
        presentationStore.clear(through: presentationGeneration)
    }

    // the state written down when it changed (the app may be ended at any
    // time once in the background)
    func keep() {
        if let text = lastScreenJSON, !cleared {
            let generation = cacheGeneration, links = self.links, profile = Self.cid, store = recentScreen
            Task.detached(priority: .utility) { store.save(text, links: links, profile: profile, generation: generation) }
        }
        guard dirty, ready, !cleared else { return }
        dirty = false
        let e = engine
        presentationGeneration += 1
        let generation = presentationGeneration, paired = links, profile = Self.cid, store = presentationStore
        Task {
            let text = await e.savePresentation()
            guard generation == presentationGeneration, paired == links else { return }
            await Task.detached(priority: .utility) { store.save(text, links: paired, profile: profile, generation: generation) }.value
        }
    }

    private static var settingsFile: URL {
        FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0].appendingPathComponent("hub-settings.json")
    }

    func retryView(_ key: String) {
        guard links.contains(where: { Pairing.key($0) == key }), startup[key] == .compatibilityRequired else { return }
        retryConnection(key)
    }

    func retryConnection(_ key: String) {
        guard links.contains(where: { Pairing.key($0) == key }) else { return }
        endProjectSearch()
        hubs.removeValue(forKey: key)?.stop()
        thinReady[key] = nil
        thinRequests[key] = nil
        cachedReadiness.retire(key)
        startup[key] = .discovering
        conn[key, default: HubConn()].epoch &+= 1
        conn[key, default: HubConn()].phase = .connecting
        conn[key, default: HubConn()].failure = nil
        connect()
    }

    // the hub settings page's rows, kept as JSON beside the state
    private func keepHubSettings() {
        guard !cleared, let d = try? JSONEncoder().encode(hubSettings) else { return }
        try? d.write(to: Self.settingsFile, options: .atomic)
    }

    // App settings, Cache: the kept state's size, and forgetting it (the next
    // launch fetches each hub's log again; nothing is written again before)
    func cacheSize() -> Int { kept.size() + recentScreen.size() + ((try? Data(contentsOf: Self.settingsFile).count) ?? 0) }

    func clearCache() {
        mailroomPositions.removeAll()
        mailroomReturn = nil
        cleared = true
        cacheTask?.cancel()
        lastScreenJSON = nil
        cacheGeneration += 1
        recentScreen.clear(through: cacheGeneration)
        presentationGeneration += 1
        presentationStore.clear(through: presentationGeneration)
        kept.clear()
        try? FileManager.default.removeItem(at: Self.settingsFile)
        hubSettings = [:]
    }

    // the thread list's view, for Bend to draw (the device's own choice)
    private func languageLocal(_ e: Engine) async {
        let generation = pairingGeneration
        for (key, value) in [("mailroom-renderer", "ios-v1"), ("lang-preview", settings.langPreview ? "1" : ""), ("lang", settings.lang), ("lang-os", Locale.preferredLanguages.first ?? "")] {
            guard generation == pairingGeneration else { return }
            apply(await e.local(key, value), generation: generation)
        }
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

    private func pathChanged(_ status: NWPath.Status, _ interfaces: [String]) {
        defer { lastPath = status; lastInterfaces = interfaces }
        guard let was = lastPath, status == .satisfied, was != .satisfied || interfaces != lastInterfaces else { return }
        for h in hubs.values { h.wake(quiet: 3) }
    }

    func foreground(_ yes: Bool) {
        active = yes
        if !yes { keep(); awayAt = Date() }
        // back in front: a socket the suspension may have killed is replaced
        // at once, and a hub waiting out its backoff is tried now
        if yes {
            let away = awayAt.map { Date().timeIntervalSince($0) } ?? 0
            awayAt = nil
            for h in hubs.values { h.wake(quiet: away > 20 ? 3 : 40) }
        }
        if yes, contentReady, let s = screen { island?.show(s.islands ?? [], foreground: true) }
    }

    private func observeThinPresenting(_ update: SyncUpdate, epoch: UInt64) {
        guard startup[update.hub] == .thin, update.thinPresenting,
              conn[update.hub]?.epoch == epoch else { return }
        var fence = thinRequests[update.hub, default: ThinPresentationFence()]
        if fence.observe(update, epoch: epoch) {
            thinReady[update.hub] = nil
            cachedReadiness.retire(update.hub)
        }
        thinRequests[update.hub] = fence
    }

    private func currentThinReply(_ update: SyncUpdate, epoch: UInt64) -> Bool {
        update.thinPresented && thinRequests[update.hub]?.accepts(epoch: epoch, revision: update.requestRevision!) == true
    }

    private func apply(_ out: Out?, generation: UInt64) {
        guard generation == pairingGeneration else {
            // A retired flush may already have drained effects for another hub.
            // Keep only effects with a positive, still-valid owner. Navigation,
            // unqualified UI effects and retired credentials have no authority.
            let commands = out?.cmds.filter { command in
                let owner: String?
                if command.type == "send" { owner = command.hub }
                else if ["keep", "draft-kept", "send-issued", "send-failed"].contains(command.type), let thread = command.thread, let split = thread.firstIndex(of: "|") { owner = String(thread[..<split]) }
                else { owner = nil }
                guard let owner, links.contains(where: { Pairing.key($0) == owner }) else { return false }
                return (retiredPairings[owner] ?? 0) <= generation
            } ?? []
            if !commands.isEmpty { apply(Out(screen: nil, cmds: commands)) }
            return
        }
        apply(out)
    }

    private func apply(_ out: Out?, sent: String? = nil) {
        guard let o = out else {
            if let t = sent, let i = outgoing.firstIndex(where: { $0.text == t }) { outgoing.remove(at: i) }
            return
        }
        for update in o.sync ?? [] {
            let epoch = conn[update.hub]?.epoch ?? 0
            observeThinPresenting(update, epoch: epoch)
            if let intent = update.viewerIntent, currentThinReply(update, epoch: epoch) {
                viewerReceipt = (intent, update.hub, epoch)
            }
            if startup[update.hub] == .thin {
                if update.phase == "presented", !currentThinReply(update, epoch: epoch) { continue }
                if update.phase == "presenting",
                   (!update.thinPresenting || thinRequests[update.hub]?.accepts(epoch: epoch, revision: update.requestRevision ?? 0) != true || thinReady[update.hub] != nil) { continue }
            }
            // Timed expiry can reach apply through pulse, outside received.
            // It may retire a marker, but cannot establish new freshness.
            if update.phase == "recent" || update.phase == "failed" {
                cachedReadiness.retire(update.hub)
                thinReady[update.hub] = nil
                thinRequests[update.hub]?.invalidate()
            }
            var status = conn[update.hub, default: HubConn()]
            status.stage = update.phase
            status.detail = update.detail
            if (startup[update.hub] == .thin && currentThinReply(update, epoch: epoch)) || (startup[update.hub] == .compatibilityChosen && update.phase == "live") { status.phase = .online }
            else if update.phase == "failed" { status.phase = .offline }
            else { status.phase = .syncing }
            if conn[update.hub] != status { conn[update.hub] = status }
            if status.phase == .online { hubs[update.hub]?.ready() }
            if startup[update.hub] == .thin, update.phase == "failed" {
                hubs[update.hub]?.reconnect(.view)
            }
        }
        if requestDeadline != o.deadline {
            requestDeadline = o.deadline
            if let deadline = o.deadline {
                let delay = max(0, deadline / 1000 - Date().timeIntervalSince1970) + 0.01
                Task { [weak self] in
                    try? await Task.sleep(for: .seconds(delay))
                    guard let self, self.requestDeadline == deadline else { return }
                    let generation = self.pairingGeneration
                    self.apply(await self.engine.pulse(), generation: generation)
                }
            }
        }
        let fresh = o.sync?.contains { $0.hub == screen?.hub && ($0.phase == "presented" || $0.phase == "live") } == true
        let cacheFresh = cachedReadiness.accepts { key in
            guard let current = screen, let next = o.screen,
                  key == next.hub,
                  Self.nav(next) == (awaitedNav ?? Self.nav(current)),
                  (awaitedNav != nil || key == current.hub),
                  hubs[key] != nil, links.contains(where: { Pairing.key($0) == key }),
                  let status = conn[key], status.phase != .offline, status.phase != .connecting else { return nil }
            return status.epoch
        }
        let thinFresh = o.screen.map { next in
            guard startup[next.hub] == .thin else { return true }
            guard let mark = thinReady[next.hub], mark.epoch == conn[next.hub]?.epoch else { return false }
            guard !(o.sync ?? []).contains(where: { $0.hub == next.hub && ($0.phase == "presented" || $0.phase == "presenting") && !currentThinReply($0, epoch: mark.epoch) }) else { return false }
            return mark.nav == Self.nav(next) && (awaitedNav == nil || awaitedNav == Self.nav(next))
        } ?? false
        #if DEBUG && targetEnvironment(simulator)
        if let next = o.screen {
            qualificationIncoming = [
                "hub": next.hub, "nav": Self.nav(next), "thread": next.thread?.id ?? "none",
                "bot": next.bot?.id ?? "none", "viewer": next.thread?.viewer.open ?? "none", "thinFresh": String(thinFresh),
                "cacheAllowed": String(!cachedPresentation || cacheFresh),
                "paired": String(next.hub.isEmpty || links.contains { Pairing.key($0) == next.hub })
            ]
        }
        #endif
        if let s = o.screen, s.hub.isEmpty || links.contains(where: { Pairing.key($0) == s.hub }),
           (!cachedPresentation || cacheFresh), thinFresh {
            cachedPresentation = false
            cachedReadiness.clear()
            if let thread = s.thread {
                let previous = composerDrafts.text(for: thread.id)
                let adopted = composerDrafts.receive(thread.id, thread.draft, edit: o.composerEdit)
                // A hub completion (skills, suggestions, attachments) is a new
                // editor version too; it must not reuse an already sent edit.
                if adopted, let previous, previous != thread.draft, composerDrafts.edit(for: thread.id) > 0 {
                    let edit = composerDrafts.change(thread.id, thread.draft)
                    publishDraft(thread.id, text: thread.draft, edit: edit)
                }
                composerOwner = thread.id
                composer = composerDrafts.text(for: thread.id) ?? ""
                Self.keep(thread.id, composer)
            } else { composerOwner = ""; composer = "" }
            screen = s
            reconcileViewer(s)
            if let text = o.screenJSON, !cleared, awaitedNav == nil || awaitedNav == Self.nav(s) {
                lastScreenJSON = text
                cacheGeneration += 1
                let generation = cacheGeneration, links = self.links, profile = Self.cid, store = recentScreen
                cacheTask?.cancel()
                cacheTask = Task {
                    try? await Task.sleep(for: .milliseconds(300))
                    guard !Task.isCancelled else { return }
                    self.dirty = true
                    self.keep()
                    await Task.detached(priority: .utility) { store.save(text, links: links, profile: profile, generation: generation) }.value
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
                if Self.nav(s).isEmpty, path.isEmpty {
                    if let t = AppTab(rawValue: settings.defaultTab), t != .settings { tab = t } else { tab = s.uber == nil ? .threads : .home }
                }
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
            if contentReady { island?.show(s.islands ?? [], foreground: active) }
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
            case "keep":
                let thread = c.thread ?? ""
                Self.keep(thread, composerDrafts.text(for: thread) ?? c.text ?? "")
            case "draft-kept":
                if let thread = c.thread, let edit = c.edit {
                    composerDrafts.acknowledged(thread, edit: edit)
                    if composerDrafts.pending(for: thread) == nil { localDrafts[thread] = nil }
                }
            case "send-issued":
                if let i = outgoing.firstIndex(where: { $0.thread == c.thread && $0.text == c.text }) { outgoing.remove(at: i) }
            case "send-failed":
                let thread = c.thread ?? "", text = c.text ?? ""
                if c.action != "interrupt" { restoreComposer(thread, text: text, edit: c.edit ?? composerDrafts.edit(for: thread)) }
                else { sendFailure = thread }
            // while asleep the hub's push carries the alert instead
            case "notify": if active { notifier.post(thread: c.thread ?? "", key: c.key ?? "", title: c.title ?? "", body: c.body ?? "") }
            default: break
            }
        }
        advanceViewerClose()
        advanceProjectSearch()
        if contentReady, let thread = screen?.thread?.id, let text = localDrafts[thread], composerDrafts.pending(for: thread) != nil {
            localDrafts[thread] = nil
            publishDraft(thread, text: text, edit: composerDrafts.edit(for: thread))
        }
        let intentContext = screen.flatMap { s in conn[s.hub].map { status in
            PresentationIntent.Context(hub: s.hub, selection: s.sel, nav: Self.nav(s), epoch: status.epoch, pairing: pairingGeneration)
        } }
        if let (action, value) = presentationIntent.take(context: intentContext, ready: contentReady) { act(action, value) }
        if fresh || cacheFresh, contentReady, !earlyActions.isEmpty {
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
