import Foundation
import SwiftUI
import CryptoKit
import UIKit
import UserNotifications

// The phone's whole link to a hub, docs/PHONE-DATA.md (data/1): one WebSocket of plain JSON data, typed values in, typed values out, a disk cache
// that paints at launch, deltas for the lists and the open thread only. No server-drawn views, no bridge, no engine.
@MainActor
@Observable
final class DataClient {
    /// A thread a link in a message asked to open (backplane://thread/<id>); the root screen takes it.
    var pendingThread: String?
    /// Just paired: once the hub's bots are in, its uberbot's chat opens (phones have no setup; docs/ALWAYS-ON.md, D9).
    private var uberAfterPair = false
    /// a tapped link the hub resolved (DataLinks.swift): the root screen opens it; a link that opens nothing leaves a short note
    var linkTarget: DLinkTarget?
    var linkNote = ""
    /// a project a link named: the list opens its group and scrolls to it
    var focusProject = ""
    #if DEBUG
    /// the link the last tap handed on (the UI tests read it: a system handler cannot be observed from the app)
    var lastLink = ""
    #endif
    static let shared = DataClient()
    enum Conn: String { case offline, connecting, online }

    // what is shown
    private(set) var conn: Conn = .offline
    private(set) var needsRepair = false { didSet { if needsRepair { logPair("screen", "re-pair") } } }
    private(set) var failure = ""
    private(set) var projects: [DProject] = []
    private(set) var threads: [DThread] = []
    private(set) var entries: [String: [DEntry]] = [:]
    private(set) var older: [String: String] = [:]
    private(set) var loadedThreads: Set<String> = []
    private(set) var listsLoaded = false
    // the projects whose "Settled (N)" row is open, kept on the device; their settled threads are read from the hub when it opens (feature `settled`)
    private(set) var settledOpen = Set(UserDefaults.standard.stringArray(forKey: "data.settled.open") ?? [])
    private(set) var settledMore: [String: String] = [:]   // a project's next page of settled threads
    private(set) var settledBusy: Set<String> = []
    let plots = PlotStore()
    let voice = VoiceModel()
    let machines = MachinesModel()
    let notify = NotifyModel()
    let ring = RingModel()
    let devices = DevicesModel()
    let terms = TermStore()
    private(set) var shot = 0
    private(set) var shotBot = ""
    private(set) var bots: [DBot] = []
    private(set) var work: [String: DWork] = [:]
    // a subagent's own view (feature subview): read whole, then pushed whole as it moves; one open at a time
    private(set) var subView: DSubView?
    private(set) var subGone = false
    private var subOpen: DSubRoute?
    private var subSub = 0
    private(set) var locks: [DLock] = []
    private(set) var rooms: [DRoom] = []
    // swarms (feature swarm, docs/SWARM.md): every project's from swarm.list, and how often each room's `swarm` push came (the open swarm screen reads again when its own moves)
    private(set) var swarms: [DSwarmRow] = []
    private(set) var swarmMoved: [String: Int] = [:]
    // what the hub says it can do beyond the base of data/1 (hello.features); a hub that names none takes no pictures
    private(set) var features: Set<String> = []
    // hello `notice`, newer from /hub/status: the banner
    private(set) var notice = HubNotice()
    // a 0.14 home coming across (hello `migrating`, then `migrating` frames): the hub's card, until one of its buttons answers it (Migration.swift)
    private(set) var migration = DMigration()
    /// the card is open over the screen (a tap on its banner)
    var migrationOpen = false
    private(set) var migrationBusy = false
    private(set) var migrationError = ""
    // the GitHub star suggestion (feature "star"): shown while the hub says so, gone once decided on any device
    private(set) var star: DStarOffer?
    // the one-time telemetry notice (feature "telemetryNotice"): shown while the hub says so
    private(set) var telemetryNotice: DTelemetryNotice?
    /// the notice's Settings link: the root view opens Settings and sets it back
    var openSettings = false
    var canSendImages: Bool { features.contains("images") }
    // the composer's own commands (hello.commands): the hub's table; none of them is ever a message
    struct Word: Equatable { let word, label, desc: String }
    private(set) var commands: [Word] = []
    struct Btw: Equatable { let q: String; var a: String }
    private(set) var btw: [String: Btw] = [:]
    struct SecretAsk: Decodable, Equatable { let name: String; let purpose: String }
    private(set) var secretAsks: [String: [SecretAsk]] = [:]
    var pairingError = "" { didSet { logPair("status", pairingError) } }
    var pairingBusy = false
    private(set) var links: [String] = PairingStore.load()
    var drafts: [String: String] = (UserDefaults.standard.dictionary(forKey: "data.drafts") as? [String: String]) ?? [:]
    // the strip over the composer (feature queue, docs/PHONE-DATA.md `thread.queue`): the hub's rows per thread, and this phone's own outbox (Outbox.swift):
    // what was written while the socket was down, kept on disk and sent in order with its clientId once connected
    private(set) var queues: [String: [DQueueRow]] = [:]
    private(set) var outbox = Outbox()
    /// a thread's last refusal or failed press in the strip, in the hub's words
    private(set) var queueError: [String: String] = [:]
    /// text put back into a thread's draft from outside the composer (Edit, a refusal): the open composer reads `drafts` again when `rev` moves
    struct DraftBack: Equatable { var thread = ""; var rev = 0 }
    private(set) var draftBack = DraftBack()

    // a tapped notification: the thread to open and the approval to land on. The views follow these
    var pushThread = ""
    var pushAccount = ""
    var pendingAsk = ""
    // the hub's quiet hours, as push.state says them
    private(set) var quiet = ""
    private(set) var quietUrgent = false

    // the hub being used: the first of the paired ones (HubList), or the home hub while that one cannot be reached (failover)
    var link: String? { onHome ? homeLink ?? links.first : links.first }
    /// the home hub (docs/pairing.md, Home hub): added by its offer, used while the hub in use cannot be reached
    private(set) var homeKey = UserDefaults.standard.string(forKey: "data.home") ?? ""
    private(set) var onHome = false
    private var homeLink: String? { homeKey.isEmpty ? nil : links.first { Pairing.key($0) == homeKey } }
    /// the key of the hub connected to now
    var usingKey: String { link.flatMap { Pairing.key($0) } ?? "" }
    /// the pairing screen is open over a paired hub, to add another (Settings → Add a hub)
    var adding = false
    var hubName: String { link.flatMap { Pairing.key($0) } ?? "" }

    @ObservationIgnored private var task: URLSessionWebSocketTask?
    @ObservationIgnored private var gen = 0
    @ObservationIgnored private var nextId = 1
    @ObservationIgnored private var calls: [Int: CheckedContinuation<Data, Error>] = [:]
    @ObservationIgnored private var backoff = 0.25
    @ObservationIgnored private var reconnectTask: Task<Void, Never>?
    @ObservationIgnored private var watchdog: Task<Void, Never>?
    @ObservationIgnored private var heard = Date()
    @ObservationIgnored private var listSub = 0
    @ObservationIgnored private var threadSub = 0
    @ObservationIgnored private var open: String?
    @ObservationIgnored private var openBot: DBot?   // the bot whose chat is open, as last listed (a home move follows it)
    @ObservationIgnored private var saveTask: Task<Void, Never>?
    @ObservationIgnored private var listNext: String?
    @ObservationIgnored private var pushToken: String?
    @ObservationIgnored private var downSince: Date?
    @ObservationIgnored private var probing = false
    @ObservationIgnored private var homeWatch: Task<Void, Never>?
    @ObservationIgnored private var offersDone: Set<String> = []
    @ObservationIgnored private var offerBusy = false

    init() {
        #if DEBUG || BP_MEASURE
        if let l = ProcessInfo.processInfo.environment["BACKPLANE_LINK"] { links = l.split(separator: " ").map(String.init) }
        #endif
        loadCache()
        loadOutbox()
        voice.client = self
        machines.client = self
        notify.client = self
        ring.client = self
        devices.client = self
        terms.client = self
        #if DEBUG
        // a simulator has no taps: what the form's Pair and the question's buttons call, from the environment
        let env = ProcessInfo.processInfo.environment
        if let t = env["BACKPLANE_PAIR_TEXT"], !t.isEmpty { Task { @MainActor in try? await Task.sleep(for: .seconds(1)); self.pair(t) } }
        #endif
    }

    // MARK: pairing
    /// What a simulator check reads of pairing: statuses and hub addresses, never a link's secret. Debug builds only.
    private func logPair(_ what: String, _ text: String) {
        #if DEBUG
        NSLog("BPPAIR %@ %@", what, text)
        #endif
    }
    /// A pairing link that came from outside the app: nothing happens until the person says so.
    var askLink: String? { didSet { if let l = askLink { logPair("ask", Pairing.invitation(l)?.origin.absoluteString ?? Pairing.key(l) ?? "") } } }
    func opened(_ url: URL) {
        let s = url.absoluteString
        guard s.hasPrefix("backplane://") || s.hasPrefix(AppIdentity.scheme + "://"), Pairing.key(s) != nil || Pairing.invitation(s) != nil else { return }
        askLink = s
    }
    func pair(_ text: String) {
        if let invite = Pairing.invitation(text) {
            guard !pairingBusy else { return }
            pairingBusy = true; pairingError = "Connecting…"
            Task {
                defer { pairingBusy = false }
                if HubList.local(invite.routes) { pairingError = HubList.localText; return }
                do { let saved = try await Pairing.exchange(invite, name: UIDevice.current.name); pairingError = ""; adopt(saved) }
                catch { pairingError = (error as NSError).domain == "BackplanePairing" ? error.localizedDescription : Pairing.message("hub_unreachable") }
            }
            return
        }
        if Pairing.bareCode(text) { pairingError = "Enter the hub's address with the short code."; return }
        if Pairing.malformedInvitation(text) { pairingError = "Enter the complete invitation link or a valid eight-character code."; return }
        adopt(text.trimmingCharacters(in: .whitespacesAndNewlines))
    }
    private func adopt(_ link: String) {
        guard Pairing.key(link) != nil else { pairingError = "Enter the hub's address or pairing link."; return }
        let changed = Pairing.key(links.first ?? "") != Pairing.key(link)
        links = HubList.adopt(links, link, key: Pairing.key)
        PairingStore.save(links)
        needsRepair = false; adding = false; uberAfterPair = true
        if changed { reset() }
        connect()
    }
    /// Switch to another paired hub: it goes first, this phone's copy of the last one's threads is dropped, and the socket moves.
    func useHub(_ key: String) {
        guard Pairing.key(link ?? "") != key else { return }
        let next = HubList.use(links, key, key: Pairing.key)
        guard next != links else { return }
        close(); leaveHome(); links = next; PairingStore.save(links); reset(); loadOutbox()
        needsRepair = false; pairingError = ""; failure = ""
        connect()
    }
    /// Forget one hub; when it was the one in use, the next paired hub is used (none left: the pairing screen).
    func forgetHub(_ key: String) {
        let current = Pairing.key(link ?? "") == key
        try? FileManager.default.removeItem(at: Self.outboxURL(key))
        links = HubList.forget(links, key, key: Pairing.key); PairingStore.save(links)
        if key == homeKey { setHome(""); if onHome { leaveHome(); close(); reset(); loadOutbox(); connect() } }
        guard current else { return }
        close(); reset(); loadOutbox(); needsRepair = false; pairingError = ""
        if link != nil { connect() } else { conn = .offline }
    }
    /// Forget every hub.
    func unpair() {
        for l in links { if let k = Pairing.key(l) { try? FileManager.default.removeItem(at: Self.outboxURL(k)) } }
        links = []; adding = false; PairingStore.save(links); setHome(""); leaveHome(); reset(); close(); conn = .offline; outbox = Outbox()
    }
    private func reset() {
        notice = HubNotice(); star = nil; telemetryNotice = nil; migration = DMigration(); migrationOpen = false; migrationError = ""
        dueThreads = []; dueProjects = []
        projects = []; threads = []; entries = [:]; older = [:]; loadedThreads = []; listsLoaded = false; open = nil; queues = [:]; queueError = [:]
        try? FileManager.default.removeItem(at: cacheURL)
    }

    // MARK: the Hub screen: plain HTTP from the pairing alone, whatever the socket is doing
    func hubHttp() -> HubHttp? {
        guard let link, let base = Pairing.http(link, path: "", token: false) else { return nil }
        return HubHttp(base: base.absoluteString, token: Pairing.token(link))
    }
    func heardNotice(_ n: HubNotice) { notice = n }

    // MARK: connection
    func connect() {
        guard let link, let base = Pairing.socket(link), var c = URLComponents(url: base, resolvingAgainstBaseURL: false) else { return }
        c.queryItems = (c.queryItems ?? []) + [URLQueryItem(name: "data", value: "1"), URLQueryItem(name: "enc", value: "json")]
        guard let url = c.url else { return }
        close()
        gen += 1
        let g = gen
        conn = .connecting
        let t = URLSession.shared.webSocketTask(with: url)
        t.maximumMessageSize = 16 << 20
        task = t
        heard = Date()
        t.resume()
        receive(t, g)
        watchdog = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(10))
                guard let self, g == self.gen else { return }
                if Date().timeIntervalSince(self.heard) > 40 { self.lost(g, "silent"); return }
            }
        }
    }
    private func close() {
        gen += 1
        watchdog?.cancel(); reconnectTask?.cancel()
        task?.cancel(with: .goingAway, reason: nil); task = nil
        for (_, k) in calls { k.resume(throwing: DError(code: "OFFLINE", message: "Offline")) }
        calls = [:]
        listSub = 0; threadSub = 0
        terms.disconnected()
    }
    // the app came back to the front: a socket that died while suspended is replaced at once
    func foreground() {
        guard link != nil else { return }
        if conn != .online || Date().timeIntervalSince(heard) > 20 { connect() } else { Task { await syncPush() } }
    }
    private func receive(_ t: URLSessionWebSocketTask, _ g: Int) {
        t.receive { [weak self] r in
            Task { @MainActor [weak self] in
                guard let self, g == self.gen else { return }
                switch r {
                case .success(let m):
                    self.heard = Date()
                    switch m {
                    case .string(let s): self.handle(Data(s.utf8), g)
                    case .data(let d): if PlotStore.isPlot(d) { self.plots.receive(d) } else { self.handle(d, g) }
                    @unknown default: break
                    }
                    self.receive(t, g)
                case .failure(let e):
                    let status = (t.response as? HTTPURLResponse)?.statusCode ?? 0
                    if status == 401 || status == 403 { self.needsRepair = true }
                    self.lost(g, status == 0 ? (e as NSError).localizedDescription : "HTTP \(status)")
                }
            }
        }
    }
    private func lost(_ g: Int, _ why: String) {
        guard g == gen else { return }
        failure = why
        close()
        conn = .offline
        if downSince == nil { downSince = Date() }
        if onHome, needsRepair { needsRepair = false; leaveHome(); reset(); loadOutbox(); connect(); return }   // the home refused this phone: back to the hub chosen
        guard link != nil, !needsRepair else { return }
        failover()
        let wait = backoff
        backoff = min(backoff * 2, 4)
        reconnectTask = Task { [weak self] in
            try? await Task.sleep(for: .seconds(wait))
            guard !Task.isCancelled else { return }
            self?.connect()
        }
    }

    // MARK: the home hub (docs/pairing.md, Home hub): its offer, carried out with no dialog, and failing over to it
    private func setHome(_ k: String) { homeKey = k; UserDefaults.standard.set(k, forKey: "data.home") }
    private func leaveHome() { onHome = false; homeWatch?.cancel(); homeWatch = nil; downSince = nil }
    private func heardOffer(_ text: String) {
        guard !onHome, !offerBusy, let o = HubList.offer(text) else { return }
        let plan = HubList.plan(text, links: links, token: link.flatMap(Pairing.token), done: offersDone, key: Pairing.key)
        logPair("offer", "\(o.url) \(String(describing: plan).prefix { $0 != "(" })")   // never the plan itself: same holds the token
        switch plan {
        case .none: return
        case .known(let k): if o.why == "home" { setHome(k) }; offerDone(o.id, true)
        case .same(let k, let l): offerBusy = true; Task { await self.tryOffer(o, k) { try await Self.takes(l); return l } }
        case .take(let k): offerBusy = true; Task {
            await self.tryOffer(o, k) {
                let r: DOfferLink = try await self.call("hub.offer.take", ["id": o.id])
                guard let invite = Pairing.invitation(r.link) else { throw DError(code: "BAD_LINK", message: "not a pairing link") }
                return try await Pairing.exchange(invite, name: UIDevice.current.name)
            }
        }
        }
    }
    private struct DOfferLink: Decodable { let link: String }
    /// a few tries with a longer wait each time; the hub added second (the one in use stays), the home when the offer says so, then hub.offer.done
    private func tryOffer(_ o: HubList.Offer, _ k: String, _ get: () async throws -> String) async {
        defer { offerBusy = false }
        for n in 0..<3 {
            if n > 0 { try? await Task.sleep(for: .seconds(Double(2 << n))) }
            do {
                let l = try await get()
                links = HubList.added(links, l, key: Pairing.key); PairingStore.save(links)
                if o.why == "home" { setHome(Pairing.key(l) ?? k) }
                logPair("added", o.url)
                offerDone(o.id, true); return
            } catch { logPair("offer-retry", error.localizedDescription) }
        }
        offerDone(o.id, false)
    }
    private func offerDone(_ id: String, _ ok: Bool) {
        offersDone.insert(id)
        Task { let _: DEmpty? = try? await call("hub.offer.done", ["id": id, "ok": ok]) }
    }
    /// the same device token at the offered hub: kept only once that hub takes it
    private static func takes(_ link: String) async throws {
        guard let url = Pairing.http(link, path: "/client/capabilities", token: false), let t = Pairing.token(link) else { throw URLError(.badURL) }
        var r = URLRequest(url: url); r.timeoutInterval = 8
        r.setValue("Bearer " + t, forHTTPHeaderField: "Authorization"); r.setValue("native", forHTTPHeaderField: "X-Backplane-Client")
        let (_, resp) = try await URLSession(configuration: .ephemeral).data(for: r)
        guard (resp as? HTTPURLResponse)?.statusCode == 200 else { throw URLError(.userAuthenticationRequired) }
    }
    /// whether a hub answers at all (/hello needs no credential)
    private static func answers(_ link: String) async -> Bool {
        guard let url = Pairing.http(link, path: "/hello", token: false) else { return false }
        var r = URLRequest(url: url); r.timeoutInterval = 4
        return ((try? await URLSession(configuration: .ephemeral).data(for: r))?.1 as? HTTPURLResponse)?.statusCode == 200
    }
    /// the hub in use has not answered for a while: the home, when it answers (HubList.onHome decides)
    private func failover() {
        guard !onHome, !probing, let home = homeLink, let main = links.first, let since = downSince else { return }
        let down = Date().timeIntervalSince(since)
        guard HubList.onHome(main: Pairing.key(main) ?? "", home: homeKey, now: false, down: down, homeUp: true, mainUp: false) else { return }
        probing = true
        Task {
            defer { probing = false }
            let up = await Self.answers(home)
            guard !onHome, HubList.onHome(main: Pairing.key(main) ?? "", home: homeKey, now: false, down: down, homeUp: up, mainUp: false) else { return }
            logPair("home", homeKey)
            close(); onHome = true; reset(); loadOutbox(); connect()
            homeWatch = Task { [weak self] in
                while !Task.isCancelled {
                    try? await Task.sleep(for: .seconds(10))
                    guard let self, self.onHome, let m = self.links.first else { return }
                    let back = await Self.answers(m)
                    if !HubList.onHome(main: Pairing.key(m) ?? "", home: self.homeKey, now: true, down: 0, homeUp: true, mainUp: back) {
                        self.logPair("back", Pairing.key(m) ?? "")
                        self.close(); self.leaveHome(); self.reset(); self.loadOutbox(); self.connect(); return
                    }
                }
            }
        }
    }

    // MARK: frames
    private func handle(_ data: Data, _ g: Int) {
        guard let o = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any], let t = o["t"] as? String else { return }
        switch t {
        case "hello":
            guard (o["proto"] as? String)?.hasPrefix("data/") == true else { failure = "This hub does not speak the phone data protocol (update it)."; lost(g, failure); return }
            features = Set((o["features"] as? [String]) ?? [])
            notice = HubRescue.notice(o["notice"] as? [String: Any])
            heardMigration(o["migrating"])
            commands = ((o["commands"] as? [[String: Any]]) ?? []).map { Word(word: $0["word"] as? String ?? "", label: $0["label"] as? String ?? "", desc: $0["desc"] as? String ?? "") }
            conn = .online; failure = ""; backoff = 0.25; plots.reset()
            if !onHome { downSince = nil }
            Task { await self.synchronise(g) }
            heardOffer(o["hubOffer"] as? String ?? "")
        case "reply":
            guard let id = o["id"] as? Int ?? (o["id"] as? Double).map(Int.init), let k = calls.removeValue(forKey: id) else { return }
            if o["ok"] as? Bool == true {
                k.resume(returning: o["result"].flatMap { $0 is NSNull ? nil : WireJSON.data($0) } ?? Data("{}".utf8))
            } else if let e = o["error"] as? [String: Any] {
                k.resume(throwing: DError(code: e["code"] as? String ?? "ERROR", message: e["message"] as? String ?? ""))
            } else { k.resume(throwing: DError(code: "ERROR", message: o["error"] as? String ?? "The hub refused")) }
        case "d": push(o)
        case "voice": voice.frame(o)
        case "plot": plots.receiveJSON(o) // why a view has nothing to show (the plots themselves come as binary frames)
        case "star": star = DStarOffer.of(o["offer"])
        case "telemetry.notice": telemetryNotice = DTelemetryNotice.of(o["notice"])
        case "migrating": heardMigration(o)
        case "info": if let v = (o["info"] as? [String: Any])?["hub.offer"] as? String { heardOffer(v) }
        case "shot": shotBot = o["bot"] as? String ?? ""; shot = (o["n"] as? Int) ?? ((o["n"] as? Double).map(Int.init) ?? 0)
        default: break
        }
    }

    func call<T: Decodable>(_ m: String, _ p: [String: Any] = [:], as: T.Type = T.self) async throws -> T {
        guard conn != .offline, let task else { throw DError(code: "OFFLINE", message: "Offline") }
        let id = nextId; nextId += 1
        guard let msg = WireJSON.data(["id": id, "m": m, "p": p]) else { throw DError(code: "BAD_PARAMS", message: "Not JSON: \(m)") }
        let data: Data = try await withCheckedThrowingContinuation { k in
            calls[id] = k
            task.send(.string(String(decoding: msg, as: UTF8.self))) { [weak self] e in
                if let e { Task { @MainActor in self?.calls.removeValue(forKey: id)?.resume(throwing: e) } }
            }
            Task { [weak self] in
                try? await Task.sleep(for: .seconds(25))
                self?.calls.removeValue(forKey: id)?.resume(throwing: DError(code: "TIMEOUT", message: "The hub did not answer"))
            }
        }
        return try JSONDecoder().decode(T.self, from: data)
    }

    // MARK: sync
    private func synchronise(_ g: Int) async {
        do {
            let s: DSubReply = try await call("subscribe", ["topic": "lists"]); guard g == gen else { return }; listSub = s.sub
            Task { await self.flushOutbox(g) }
            async let p = loadProjects(g), t = loadThreads(g), h: Void = loadHistory()
            _ = try await (p, t, h)
            await loadSections()   // the sidebar's sections (feature sections): the list draws them once they come
            await loadBots()
            await loadSwarms()
            await voice.resync()
            await enablePush(); await registerPush(); await syncPush()
            await loadLocks()
            await loadStar()
            await loadTelemetryNotice()
            terms.resync()
            if let open { try await subscribeThread(open, g); await refreshThread(open) }
            if let r = subOpen { subSub = 0; await readSub(r, g) }
        } catch { if g == gen { failure = error.localizedDescription } }
    }
    func refresh() async { guard conn == .online else { return }; let g = gen; _ = try? await loadProjects(g); _ = try? await loadThreads(g); await loadLocks(); await loadHistory(); await loadSwarms(); await loadSections() }
    // MARK: The sidebar's sections (feature sections, docs/ALWAYS-ON.md section 8): Needs you, Pinned, Active, the groups, Quiet, as the hub presents them
    private(set) var sections: [DSection] = []
    private(set) var groupsTo: [DGroupTo] = []
    private(set) var sectionsOpen = Set(UserDefaults.standard.stringArray(forKey: "data.sections.open") ?? [])
    func loadSections() async {
        guard has("sections"), let r: DSections = try? await call("sidebar.sections", ["open": Array(sectionsOpen)]) else { return }
        sections = r.sections; groupsTo = r.groupsTo
    }
    func toggleSection(_ toggle: String) {
        if sectionsOpen.contains(toggle) { sectionsOpen.remove(toggle) } else { sectionsOpen.insert(toggle) }
        UserDefaults.standard.set(Array(sectionsOpen), forKey: "data.sections.open"); Task { await loadSections() }
    }
    func sidebarAct(_ action: String, _ value: String) async { _ = try? await call("sidebar.act", ["action": action, "value": value]) as DEmpty; await loadSections() }
    func projectPage(_ key: String, done: Bool) async -> DProjectPage? {
        guard has("sections"), let r: DSections = try? await call("sidebar.sections", ["open": Array(sectionsOpen), "page": key, "pageDone": done]) else { return nil }
        return r.page
    }
    private var sectionsDue = false
    /// a thread or project moved (a push): ask for the sections again, once for a burst
    private func sectionsMoved() { guard has("sections"), !sectionsDue else { return }; sectionsDue = true; Task { try? await Task.sleep(nanoseconds: 400_000_000); sectionsDue = false; await loadSections() } }
    // MARK: Conversation history (feature history): the old Scratch chats, found and opened, never a place to start one
    private(set) var history = DHistory()
    private(set) var historyOpen = UserDefaults.standard.bool(forKey: "data.history.open")
    func setHistory(open on: Bool) { historyOpen = on; UserDefaults.standard.set(on, forKey: "data.history.open") }
    func loadHistory(more: Bool = false) async {
        guard has("history") else { return }
        if more && history.next == nil { return }
        var q: [String: Any] = ["limit": 50]; if more, let c = history.next { q["before"] = c }
        guard var r: DHistory = try? await call("history.list", q) else { return }
        if more { r.threads = history.threads + r.threads }
        history = r
    }
    private var historyDue = false
    /// a chat of the group moved (a push): ask for the group again, once for a burst
    private func historyMoved() { guard !historyDue else { return }; historyDue = true; Task { try? await Task.sleep(nanoseconds: 300_000_000); historyDue = false; await loadHistory() } }
    func loadStar() async { guard has("star"), let r: DStarReply = try? await call("star.offer") else { return }; star = r.offer?.valid }
    /// open (the banner also opens the page) or dismiss: final, the banner goes at once
    func starDecide(_ action: String) {
        guard let s = star else { return }; star = nil
        Task { let _: DEmpty? = try? await call("star.nudge", ["token": s.token, "action": action]) }
    }
    func loadTelemetryNotice() async { guard has("telemetryNotice"), let r: DTelemetryNoticeReply = try? await call("telemetry.notice") else { return }; telemetryNotice = r.notice?.valid }
    @ObservationIgnored private var noticeShown = Set<String>()   // tokens whose "shown" went this app session
    /// The strip is on screen: "shown" once per token per app session (it lets the hub start sending).
    func telemetryShown(_ token: String) {
        guard !token.isEmpty, noticeShown.insert(token).inserted else { return }
        Task { do { let _: DEmpty = try await call("telemetry.notice.act", DTelemetryNotice.act(token, "shown")) } catch { noticeShown.remove(token) } }
    }
    /// close (✕) or off (turns telemetry off on the hub): the strip goes at once
    func telemetryAct(_ action: String) {
        guard let n = telemetryNotice else { return }; telemetryNotice = nil
        Task { let _: DEmpty? = try? await call("telemetry.notice.act", DTelemetryNotice.act(n.token, action)) }
    }
    private func heardMigration(_ v: Any?) {
        migration.heard(v)
        if migration.card == nil { migrationOpen = false }
        #if DEBUG
        // a simulator has no taps: BACKPLANE_MIGRATION_OPEN=1 opens the card as it comes, BACKPLANE_MIGRATION_PRESS=<label> presses that button once it is shown
        let env = ProcessInfo.processInfo.environment
        if env["BACKPLANE_MIGRATION_OPEN"] == "1", migration.card != nil { migrationOpen = true }
        if let l = env["BACKPLANE_MIGRATION_PRESS"], let b = migration.card?.buttons.first(where: { $0.label == l }), !migrationBusy {
            Task { try? await Task.sleep(for: .seconds(4)); self.migrationAct(b) }
        }
        NSLog("BPMIG %@ %@", migration.card?.state ?? "none", String(migration.card?.pct ?? -1))
        #endif
    }
    /// one of the card's buttons, as setup's: onboard.act {action, value}. The hub's ok answers the card; a run it starts shows itself with its next frame
    func migrationAct(_ b: DMigButton) {
        guard !migrationBusy else { return }
        migrationBusy = true; migrationError = ""
        Task {
            defer { migrationBusy = false }
            do {
                let _: DEmpty = try await call("onboard.act", ["action": b.action, "value": b.value])
                migration.answered()
                if migration.card == nil { migrationOpen = false }
            } catch { migrationError = (error as? DError)?.message ?? error.localizedDescription }
        }
    }
    func loadLocks() async { guard has("work"), let r: DLocks = try? await call("locks.list") else { return }; locks = r.locks }
    private func loadWork(_ id: String) async { guard has("work"), let r: DWorkReply = try? await call("thread.work", ["thread": id]) else { return }; work[id] = r.subs }
    // a row's button; what it changes comes back as a push
    func workAct(_ id: String, _ row: DWorkRow) { Task { _ = try? await call("work.act", ["thread": id, "act": row.act ?? "", "value": row.value ?? ""], as: DEmpty.self) } }
    /// The machine switcher (docs/PHONE-DATA.md, Your machines): the chips the hub sends with the lists (none with no linked machine), and the one in force.
    private(set) var chips: [DChip] = []
    private(set) var machine = ""
    func pickMachine(_ v: String) { guard v != machine else { return }; machine = v; Task { await refresh() } }
    private func machineQ(_ q: [String: Any]) -> [String: Any] { var q = q; if !machine.isEmpty { q["machine"] = machine }; return q }
    private func takeChips(_ c: [DChip]?) { guard let c else { return }; chips = c; if let on = c.first(where: \.on), on.value != machine { machine = on.value } }
    private func loadProjects(_ g: Int) async throws {
        let r: DProjectsPage = try await call("projects.list", machineQ(["limit": 100])); guard g == gen else { return }; takeChips(r.machines)
        flushLists()   // the deltas that came before the page are older than it
        projects = r.projects.sorted { $0.updatedAt > $1.updatedAt }; listsLoaded = true; scheduleSave()
    }
    private func loadThreads(_ g: Int) async throws {
        // a hub with `settled` sends the live threads alone, so settled ones never push them off the page; the settled ones come per project when its row opens
        let split = has("settled")
        var q: [String: Any] = ["limit": 200]; if split { q["settled"] = false }
        let r: DThreadsPage = try await call("threads.list", machineQ(q)); guard g == gen else { return }; takeChips(r.machines)
        flushLists()
        threads = r.threads.sorted { $0.updatedAt > $1.updatedAt } + (split ? threads.filter { old in old.settled && !r.threads.contains(where: { $0.id == old.id }) } : [])
        listNext = r.next; listsLoaded = true; scheduleSave()
        if split { for p in settledOpen { await loadSettled(p, g: g) } }
        await parentsOf(g)
    }
    /// The parents of subagents and subthreads the list left out (a settled one): read one by one, so a child sits under its parent, as on the desktop. A few at most.
    private func parentsOf(_ g: Int) async {
        let have = Set(threads.map(\.id))
        var seen = Set<String>()
        for pid in threads.map(\.parent) where !pid.isEmpty && !have.contains(pid) && seen.insert(pid).inserted && seen.count <= 20 {
            guard let r: DEntriesPage = try? await call("thread.entries", ["thread": pid, "limit": 1]), g == gen, let t = r.thread else { continue }
            if !threads.contains(where: { $0.id == t.id }) { threads.append(t) }
        }
        scheduleSave()
    }
    func setSettled(open on: Bool, _ project: String) {
        if on { settledOpen.insert(project) } else { settledOpen.remove(project) }
        UserDefaults.standard.set(Array(settledOpen), forKey: "data.settled.open")
        if on, has("settled"), conn == .online { let g = gen; Task { await loadSettled(project, g: g) } }
    }
    // the first page of a project's settled threads, or the next one
    func loadSettled(_ project: String, more: Bool = false, g: Int? = nil) async {
        let g = g ?? gen
        guard has("settled"), !settledBusy.contains(project) else { return }
        if more && settledMore[project] == nil { return }
        settledBusy.insert(project); defer { settledBusy.remove(project) }
        var q: [String: Any] = ["project": project, "settled": true, "limit": 50]; if more, let c = settledMore[project] { q["before"] = c }
        guard let r: DThreadsPage = try? await call("threads.list", machineQ(q)), g == gen else { return }
        flushLists()
        for t in r.threads { if let i = threads.firstIndex(where: { $0.id == t.id }) { threads[i] = t } else { threads.append(t) } }
        settledMore[project] = r.next; scheduleSave()
    }

    // opening a thread: what the cache has shows now; the hub's newest page replaces it
    func openThread(_ id: String) {
        open = id
        guard conn == .online else { return }
        let g = gen
        if has("push") { Task { _ = try? await self.call("push.seen", ["thread": id], as: DEmpty.self); await self.syncPush() } }
        // the subscribe and the newest page go out back to back on the one socket (it keeps their order): no round trip between them
        Task {
            async let sub: Void? = try? await self.subscribeThread(id, g)
            await refreshThread(id)
            _ = await sub
        }
    }

    // MARK: cache-first: what the list shows is read ahead, so opening it paints from memory
    @ObservationIgnored private var prefetched: [String: Date] = [:]
    @ObservationIgnored private var prefetchBusy = 0
    @ObservationIgnored private var touched: [String: UInt64] = [:]
    // a finger went down on a thread: read it now (and remember when, for the open timing)
    func touch(_ id: String) {
        if touched[id].map({ DispatchTime.now().uptimeNanoseconds - $0 > 1_000_000_000 }) ?? true { touched[id] = DispatchTime.now().uptimeNanoseconds }
        prefetch(id)
    }
    // (the thread screen is made at the tap: DataThreadView.init calls touch, onAppear + one run loop later is "shown")
    func prefetch(_ id: String) {
        guard conn == .online, id != open, prefetchBusy < 4, prefetched[id].map({ Date().timeIntervalSince($0) > 60 }) ?? true else { return }
        prefetched[id] = Date(); prefetchBusy += 1
        let g = gen
        Task {
            defer { prefetchBusy -= 1 }
            guard let r: DEntriesPage = try? await call("thread.entries", ["thread": id, "limit": 40]), g == gen, id != open else { return }
            let pending = (entries[id] ?? []).filter { $0.sending || $0.failed }
            entries[id] = r.entries + pending
            older[id] = r.next
            loadedThreads.insert(id)
        }
    }
    // the thread's screen is up: how long since the touch (or the open call), and whether it had a saved copy to show
    func shown(_ id: String, hadCopy: Bool) {
        #if DEBUG || BP_MEASURE
        let t0 = touched.removeValue(forKey: id) ?? DispatchTime.now().uptimeNanoseconds
        NSLog("Backplane open cached=%@ ms=%.1f", String(hadCopy), Double(DispatchTime.now().uptimeNanoseconds - t0) / 1e6)
        #endif
    }
    func openSub(_ r: DSubRoute) {
        closeSub(); subOpen = r; subGone = false
        let g = gen; Task { await readSub(r, g) }
    }
    private func readSub(_ r: DSubRoute, _ g: Int) async {
        guard let s: DSubReply = try? await call("subscribe", ["topic": "sub", "thread": r.thread, "key": r.key]) else { return }
        guard g == gen, subOpen == r else { _ = try? await call("unsubscribe", ["sub": s.sub], as: DEmpty.self); return }
        subSub = s.sub
        guard let v: DSubViewReply = try? await call("thread.sub", ["thread": r.thread, "key": r.key]), subOpen == r else { return }
        subView = v.view; subGone = v.view == nil
    }
    func closeSub() {
        subOpen = nil; subView = nil; subGone = false
        let s = subSub; subSub = 0
        if s != 0, conn == .online { Task { _ = try? await call("unsubscribe", ["sub": s], as: DEmpty.self) } }
    }
    func closeThread(_ id: String) {
        if open == id { open = nil }
        let sub = threadSub; threadSub = 0
        if sub != 0, conn == .online { Task { _ = try? await call("unsubscribe", ["sub": sub], as: DEmpty.self) } }
    }
    private func subscribeThread(_ id: String, _ g: Int) async throws {
        let s: DSubReply = try await call("subscribe", ["topic": "thread", "thread": id]); guard g == gen else { return }
        threadSub = s.sub
    }
    private func refreshThread(_ id: String) async {
        let g = gen
        guard let r: DEntriesPage = try? await call("thread.entries", ["thread": id, "limit": 40]), g == gen else { return }
        // keep what is still being sent
        let pending = (entries[id] ?? []).filter { $0.sending || $0.failed }
        entries[id] = r.entries + pending.filter { p in !r.entries.contains { $0.clientId != nil && $0.clientId == p.clientId } }
        older[id] = r.next
        loadedThreads.insert(id)
        if let t = r.thread { upsert(t) }
        scheduleSave()
        await loadQueue(id)
        await loadWork(id)
    }
    func loadOlder(_ id: String) {
        guard let cursor = older[id], conn == .online else { return }
        older[id] = nil // one page in flight
        let g = gen
        Task {
            guard let r: DEntriesPage = try? await call("thread.entries", ["thread": id, "limit": 40, "before": cursor]), g == gen else { older[id] = cursor; return }
            let have = Set((entries[id] ?? []).map(\.id))
            entries[id] = r.entries.filter { !have.contains($0.id) } + (entries[id] ?? [])
            older[id] = r.next
        }
    }

    // MARK: pushes
    private func push(_ o: [String: Any]) {
        guard let k = o["k"] as? String else { return }
        if k == "btw", let th = o["thread"] as? String, let q = o["q"] as? String, btw[th]?.q == q { btw[th] = Btw(q: q, a: o["a"] as? String ?? ""); return }
        if k == "term" { terms.push(o); return }
        if k == "bots" { Task { await loadBots() }; return } // the bots or the home moved: read them again (an open uberbot chat follows it)
        if k == "secret" { let th = o["thread"] as? String ?? ""; Task { await refreshSecretAsks(th) }; return }
        // a null (`"subs":null` when a thread's work ends) or any non-JSON value decodes to nil instead of raising
        func decode<T: Decodable>(_ key: String, _ type: T.Type) -> T? { WireJSON.decode(o[key], as: type) }
        // a row that changed is held for one batch (flushLists): a burst (a 0.14 import writes thousands) is one sort and one redraw, not one per row,
        // so the socket's frames never queue up behind the list. Anything else first applies what is held, so the order stays the hub's
        if k == "thread", let t = decode("thread", DThread.self) { dueThreads.append(t); dueLists(); return }
        if k == "project", let p = decode("project", DProject.self) { dueProjects.append(p); dueLists(); return }
        flushLists()
        switch k {
        case "thread.removed":
            if let id = o["id"] as? String { threads.removeAll { $0.id == id }; entries[id] = nil; scheduleSave() }
        case "project.removed":
            if let id = o["id"] as? String { projects.removeAll { $0.id == id }; scheduleSave() }
        case "entry":
            if let tid = o["thread"] as? String, let e = decode("entry", DEntry.self) { put(e, in: tid); if e.kind == "user", queues[tid] != nil { Task { await self.loadQueue(tid) } } }
        case "work":
            if let tid = o["thread"] as? String { work[tid] = decode("subs", DWork.self) }
        case "sub":
            if let tid = o["thread"] as? String, let key = o["key"] as? String, subOpen == DSubRoute(thread: tid, key: key), let v = decode("view", DSubView.self) { subView = v; subGone = false }
        case "queue":
            // the strip's rows changed with no line in the log (a message on its way to another machine): ask again
            if let tid = o["thread"] as? String { Task { await self.loadQueue(tid) } }
        case "locks":
            locks = decode("locks", [DLock].self) ?? []
        case "text":
            if let tid = o["thread"] as? String, let eid = o["entry"] as? String, let add = o["append"] as? String,
               var l = entries[tid], let i = l.firstIndex(where: { $0.id == eid }) {
                l[i].text += add; l[i].live = true; entries[tid] = l
            }
        case "swarm":
            if let r = o["room"] as? String { swarmMoved[r, default: 0] += 1; swarmsMoved() }
        case "notify":
            notified(o)
        case "clear":
            Task { await self.cleared(o["keys"] as? [String] ?? [], badge: o["badge"] as? Int) }
        case "turn":
            if let tid = o["thread"] as? String, let s = o["state"] as? String, let i = threads.firstIndex(where: { $0.id == tid }) {
                threads[i].state = s
                if tid == open { Task { await self.loadQueue(tid) } }
            }
        default: break
        }
    }
    @ObservationIgnored private var dueThreads: [DThread] = []
    @ObservationIgnored private var dueProjects: [DProject] = []
    @ObservationIgnored private var dueTask: Task<Void, Never>?
    private func dueLists() {
        guard dueTask == nil else { return }
        dueTask = Task { [weak self] in try? await Task.sleep(for: .milliseconds(200)); self?.flushLists() }
    }
    /// the held rows, in the order they came: one sort and one change of each list
    func flushLists() {
        dueTask?.cancel(); dueTask = nil
        if !dueThreads.isEmpty {
            let due = dueThreads; dueThreads = []
            var ts = threads; var at = Dictionary(ts.enumerated().map { ($1.id, $0) }, uniquingKeysWith: { a, _ in a })
            for t in due { if let i = at[t.id] { ts[i] = t } else { at[t.id] = ts.count; ts.append(t) } }
            threads = ts.sorted { $0.updatedAt > $1.updatedAt }
            if due.contains(where: \.history) { historyMoved() }
            sectionsMoved()
            scheduleSave()
        }
        if !dueProjects.isEmpty {
            let due = dueProjects; dueProjects = []
            var ps = projects; var at = Dictionary(ps.enumerated().map { ($1.id, $0) }, uniquingKeysWith: { a, _ in a })
            for p in due { if let i = at[p.id] { ps[i] = p } else { at[p.id] = ps.count; ps.append(p) } }
            projects = ps.sorted { $0.updatedAt > $1.updatedAt }
            scheduleSave()
        }
    }
    private func upsert(_ t: DThread) {
        flushLists()
        if let i = threads.firstIndex(where: { $0.id == t.id }) {
            threads[i] = t
        } else { threads.append(t) }
        threads.sort { $0.updatedAt > $1.updatedAt }
        scheduleSave()
    }
    private func put(_ e: DEntry, in tid: String) {
        var l = entries[tid] ?? []
        // my own message: the hub's entry carries my clientId, or an id equal to it
        let mine = l.firstIndex(where: { $0.sending && ($0.clientId == e.clientId && e.clientId != nil || $0.clientId == e.id) })
        if let i = mine { l[i] = e }
        else if let r = e.replaces, let i = l.firstIndex(where: { $0.id == r }) {
            // the stream ended: the final message takes the live entry's place (and drops any other copy of itself)
            l[i] = e
            var seen = false
            l = l.filter { x in if x.id == e.id { if seen { return false }; seen = true }; return true }
        }
        else if let i = l.firstIndex(where: { $0.id == e.id }) { l[i] = e }
        else { l.append(e) }
        entries[tid] = l
        scheduleSave()
    }

    // MARK: actions
    /// A message: message.send at once, whatever the thread is doing (the hub puts it into a running turn). With the socket down, or while older
    /// unsent ones wait, it goes to the outbox instead (a message with pictures is not kept there: it stays in the conversation with Retry).
    func send(_ tid: String, _ text: String, images: [Data] = []) {
        let text = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty || !images.isEmpty else { return }
        let cid = UUID().uuidString
        queueError[tid] = nil
        if images.isEmpty && (conn != .online || !outbox.isEmpty) {
            outbox.push(thread: tid, text: text, clientId: cid); saveOutbox()
            setDraft(tid, "")
            if conn == .online { let g = gen; Task { await flushOutbox(g) } }
            return
        }
        var local = DEntry(id: "local:" + cid, kind: "user", text: text, at: Int(Date().timeIntervalSince1970 * 1000), clientId: cid)
        local.sending = true; local.localImages = images
        entries[tid, default: []].append(local)
        setDraft(tid, "")
        Task { await deliver(tid, text, cid, images) }
    }
    func retry(_ tid: String, _ entry: DEntry) {
        guard let cid = entry.clientId, var l = entries[tid], let i = l.firstIndex(where: { $0.id == entry.id }) else { return }
        l[i].failed = false; l[i].sending = true; entries[tid] = l
        Task { await deliver(tid, entry.text, cid, entry.localImages) }
    }
    private func deliver(_ tid: String, _ text: String, _ cid: String, _ images: [Data]) async {
        do {
            var p: [String: Any] = ["thread": tid, "text": text, "clientId": cid]
            if !images.isEmpty {
                p["attachments"] = images.enumerated().map { ["name": "photo-\($0.offset + 1).jpg", "mime": "image/jpeg", "data": $0.element.base64EncodedString()] }
            }
            let r: DEntryReply = try await call("message.send", p)
            // the pictures sent stay on this phone under the hub's ids: no fetch for what we just sent
            for (i, a) in r.entry.attachments.enumerated() where i < images.count { Self.attachmentCache.setObject(images[i] as NSData, forKey: a.id as NSString) }
            put(r.entry, in: tid)
            await loadQueue(tid)
        } catch {
            if images.isEmpty, var l = entries[tid], let i = l.firstIndex(where: { $0.clientId == cid }) {
                l.remove(at: i); entries[tid] = l
                // the socket went: into the outbox, sent again (same clientId, posted once) when connected
                if Self.socketGone(error) { outbox.push(thread: tid, text: text, clientId: cid); saveOutbox() }
                // the hub said no: its words, and the text back into the draft
                else { refused(tid, text, error) }
                return
            }
            if var l = entries[tid], let i = l.firstIndex(where: { $0.clientId == cid }) { l[i].sending = false; l[i].failed = true; entries[tid] = l }
        }
    }
    /// The call never got the hub's answer (no socket, cut, or no reply): not a refusal, so the message is kept and sent again.
    nonisolated static func socketGone(_ e: Error) -> Bool { guard let d = e as? DError else { return true }; return d.code == "OFFLINE" || d.code == "TIMEOUT" }
    private func refused(_ tid: String, _ text: String, _ error: Error) {
        queueError[tid] = (error as? DError)?.message ?? error.localizedDescription
        backToDraft(tid, text)
    }
    private func backToDraft(_ tid: String, _ text: String) {
        setDraft(tid, Outbox.draft(drafts[tid] ?? "", adding: text))
        draftBack = DraftBack(thread: tid, rev: draftBack.rev + 1)
    }

    // MARK: the strip over the composer (feature queue) and this phone's outbox
    /// What the strip shows: the hub's rows while connected (they are stale otherwise), then this phone's unsent ones.
    func queueRows(_ tid: String) -> [DQueueRow] { (conn == .online ? queues[tid] ?? [] : []) + outbox.rows(tid) }
    func loadQueue(_ tid: String) async {
        guard has("queue"), let r: DQueueReply = try? await call("thread.queue", ["thread": tid]) else { return }
        queues[tid] = r.rows.isEmpty ? nil : r.rows
    }
    /// A row's button. The outbox's own (u-edit, u-remove) stay on the phone; the hub's (q-*, o-*) go to queue.act, and q-edit / o-edit answer the text for the draft.
    func queueAct(_ tid: String, _ b: DQueueButton) async {
        queueError[tid] = nil
        switch b.action {
        case Outbox.edit: if let it = outbox.remove(b.value) { saveOutbox(); backToDraft(tid, it.text) }
        case Outbox.remove: outbox.remove(b.value); saveOutbox()
        default:
            do {
                let r: DQueueActReply = try await call("queue.act", ["thread": tid, "action": b.action, "value": b.value])
                if (b.action == "q-edit" || b.action == "o-edit"), let t = r.text, !t.isEmpty { backToDraft(tid, t) }
            } catch { queueError[tid] = (error as? DError)?.message ?? error.localizedDescription }
            await loadQueue(tid)
        }
    }
    @ObservationIgnored private var flushing = false
    /// Connected again: each unsent message, oldest first, with its clientId; each leaves the outbox when the hub answers. A cut stops it (the next connect goes on).
    private func flushOutbox(_ g: Int) async {
        guard !flushing else { return }
        flushing = true; defer { flushing = false }
        while g == gen, conn == .online, let it = outbox.next {
            do {
                let r: DEntryReply = try await call("message.send", ["thread": it.thread, "text": it.text, "clientId": it.clientId])
                outbox.sent(it.clientId); saveOutbox()
                put(r.entry, in: it.thread)
                await loadQueue(it.thread)
            } catch {
                if Self.socketGone(error) { return }
                outbox.sent(it.clientId); saveOutbox()
                refused(it.thread, it.text, error)
            }
        }
    }
    nonisolated static func outboxURL(_ hub: String) -> URL {
        let d = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        try? FileManager.default.createDirectory(at: d, withIntermediateDirectories: true)
        let safe = String(hub.map { $0.isLetter || $0.isNumber || $0 == "-" || $0 == "." ? $0 : "_" })
        return d.appendingPathComponent("outbox-" + safe + ".json")
    }
    private func loadOutbox() {
        guard let hub = link.flatMap({ Pairing.key($0) }) else { outbox = Outbox(); return }
        outbox = Outbox.decoded(try? Data(contentsOf: Self.outboxURL(hub)))
    }
    private func saveOutbox() {
        guard let hub = link.flatMap({ Pairing.key($0) }) else { return }
        try? outbox.encoded().write(to: Self.outboxURL(hub), options: .atomic)
    }

    // MARK: pictures
    nonisolated(unsafe) static let attachmentCache = NSCache<NSString, NSData>()
    func attachmentData(_ id: String) async -> Data? {
        if let d = Self.attachmentCache.object(forKey: id as NSString) { return d as Data }
        guard let r: DAttachmentData = try? await call("attachment.get", ["id": id]), let d = Data(base64Encoded: r.data) else { return nil }
        Self.attachmentCache.setObject(d as NSData, forKey: id as NSString)
        return d
    }
    // a picture chosen or pasted: the longest side at most 2048 px, JPEG at 0.8 (about a megabyte at most; the hub takes 8 MB)
    nonisolated static func prepared(_ data: Data) -> Data? {
        guard let image = UIImage(data: data) else { return nil }
        let longest = max(image.size.width, image.size.height) * image.scale
        let factor = longest > 2048 ? 2048 / longest : 1
        let size = CGSize(width: image.size.width * image.scale * factor, height: image.size.height * image.scale * factor)
        let fmt = UIGraphicsImageRendererFormat(); fmt.scale = 1; fmt.opaque = true
        let out = UIGraphicsImageRenderer(size: size, format: fmt).image { _ in UIColor.white.setFill(); UIRectFill(CGRect(origin: .zero, size: size)); image.draw(in: CGRect(origin: .zero, size: size)) }
        return out.jpegData(compressionQuality: 0.8)
    }
    func answer(_ tid: String, ask: String, option: String) async {
        do {
            _ = try await call("ask.answer", ["thread": tid, "ask": ask, "answer": option], as: DEmpty.self)
            // the card shows the answer at once; the hub's own push (same entry id, state answered) confirms it
            if var l = entries[tid], let i = l.firstIndex(where: { $0.ask?.id == ask }) {
                l[i].ask?.state = "answered"; l[i].ask?.answered = option; entries[tid] = l
            }
        } catch { failure = error.localizedDescription }
    }
    func interrupt(_ tid: String) async { _ = try? await call("turn.interrupt", ["thread": tid], as: DEmpty.self) }
    func addProject(path: String, title: String = "") async throws -> DProject {
        var p: [String: Any] = ["path": path]; if !title.isEmpty { p["title"] = title }
        let r: DProjectReply = try await call("project.add", p)
        flushLists()
        if let i = projects.firstIndex(where: { $0.id == r.project.id }) { projects[i] = r.project } else { projects.insert(r.project, at: 0) }
        return r.project
    }
    func createThread(project: String, text: String, choice: DNewThreadChoice = DNewThreadChoice()) async throws -> DThread {
        var p: [String: Any] = ["project": project, "clientId": UUID().uuidString, "permission": "ask"]
        for (k, v) in choice.params where (v as? String)?.isEmpty == false { p[k] = v }
        let t = text.trimmingCharacters(in: .whitespacesAndNewlines); if !t.isEmpty { p["text"] = t }
        let r: DThreadReply = try await call("thread.create", p)
        upsert(r.thread)
        return r.thread
    }
    func dirs(_ path: String?) async throws -> DDirs { try await call("dirs.list", path.map { ["path": $0] } ?? [:]) }
    func setDraft(_ tid: String, _ text: String) {
        voice.typed(tid, old: drafts[tid] ?? "", new: text)
        putDraft(tid, text)
    }
    /// A draft the hub's dictation wrote (the mark was already placed by it).
    func putDraft(_ tid: String, _ text: String) {
        drafts[tid] = text.isEmpty ? nil : text
        UserDefaults.standard.set(drafts, forKey: "data.drafts")
    }

    // MARK: the composer's own commands (feature commands): the hub runs them, the draft stays until it said yes
    struct CommandResult: Decodable { let kind: String; var q: String?; var name: String?; var text: String? }
    private struct SecretAsks: Decodable { let asks: [SecretAsk] }
    /// The command this draft is, by the hub's own rule (session/composer.rs command_is): the word alone, or followed by a space or a new line.
    func commandOf(_ draft: String) -> Word? {
        let lead = String(draft.drop(while: { $0.isWhitespace }))
        return commands.first { w in
            guard lead.hasPrefix(w.word) else { return false }
            let r = lead.dropFirst(w.word.count)
            return r.allSatisfy(\.isWhitespace) || r.first == " " || r.first == "\n"
        }
    }
    /// Runs it; throws the hub's plain words when it cannot. `kind` says what became of it.
    func runCommand(_ tid: String, _ text: String) async throws -> CommandResult {
        let r: CommandResult = try await call("composer.run", ["thread": tid, "text": text])
        if r.kind == "btw" { btw[tid] = Btw(q: r.q ?? "", a: "Thinking…") }
        return r
    }
    func closeBtw(_ tid: String) { btw[tid] = nil }
    func saveSecret(_ tid: String, name: String, value: String, purpose: String) async throws {
        let _: DEmpty = try await call("secret.set", ["name": name, "value": value, "purpose": purpose, "thread": tid])
        await refreshSecretAsks(tid)
    }
    func dismissSecret(_ tid: String, name: String) async { let _: DEmpty? = try? await call("secret.dismiss", ["name": name]); await refreshSecretAsks(tid) }
    func refreshSecretAsks(_ tid: String) async {
        guard has("commands"), !tid.isEmpty, let r: SecretAsks = try? await call("secret.asks", ["thread": tid]) else { return }
        secretAsks[tid] = r.asks
    }

    // MARK: data/1 additions (features actions, search, models, mentions, bots, notify; the hub's native/host/src/data/ext.rs)
    func has(_ f: String) -> Bool { features.contains(f) }
    func threadAct(_ id: String, _ action: String, title: String = "") async {
        var p: [String: Any] = ["thread": id, "action": action]; if !title.isEmpty { p["title"] = title }
        do {
            let r: DThreadOpt = try await call("thread.act", p)
            flushLists()
            if action == "delete" { threads.removeAll { $0.id == id }; entries[id] = nil; scheduleSave() }
            else if let t = r.thread { upsert(t) }
        } catch { failure = error.localizedDescription }
    }
    func badgeList() async -> DBadgeList? { try? await call("badge.list") }
    func projectAct(_ id: String, _ action: String, title: String = "", value: String = "") async {
        var p: [String: Any] = ["project": id, "action": action]; if !title.isEmpty { p["title"] = title }; if !value.isEmpty { p["value"] = value }
        do {
            _ = try await call("project.act", p, as: DEmpty.self)
            flushLists()
            if action == "remove" { projects.removeAll { $0.id == id }; threads.removeAll { $0.project == id }; scheduleSave() } else { await refresh() }
        } catch { failure = error.localizedDescription }
    }
    func search(_ q: String) async -> DSearch? { try? await call("search", ["q": q]) }
    func buyOf(_ thread: String) async -> DBuy? { guard has("buy") else { return nil }; return (try? await call("thread.buy", ["thread": thread], as: DBuyReply.self))?.buy }
    func buy(_ thread: String) async throws -> String { try await call("order.buy", ["thread": thread], as: DThreadRef.self).thread }
    func find(_ thread: String, _ q: String) async -> DFind? { try? await call("find", ["thread": thread, "q": q]) }
    func searchIn(_ thread: String, _ q: String) async -> [DHit] { (try? await call("search", ["q": q, "thread": thread, "limit": 60], as: DHits.self))?.messages ?? [] }
    func order(_ thread: String) async throws -> DOrder? { (try await call("thread.order", ["thread": thread]) as DOrderReply).order }
    func picker(thread: String?, _ choice: DNewThreadChoice = DNewThreadChoice()) async throws -> DPicker {
        var p = choice.params; if let thread { p["thread"] = thread }
        return (try await call("thread.picker", p) as DPickerReply).picker
    }
    func pick(_ id: String, action: String, value: String) async throws {
        let r: DThreadOpt = try await call("thread.pick", ["thread": id, "action": action, "value": value])
        if let t = r.thread { upsert(t) }
    }
    func mentions(_ q: String, kind: String) async -> [DMention] {
        ((try? await call("mentions.list", ["q": q, "kind": kind, "limit": 8]) as DMentions)?.items) ?? []
    }
    func loadBots() async {
        guard has("bots"), let r: DDirectory = try? await call("bots.list") else { return }
        if let o = open, let b = bots.first(where: { $0.thread == o }) { openBot = b }
        bots = r.bots; rooms = r.rooms
        if uberAfterPair, let u = bots.first(where: { $0.uber && !$0.thread.isEmpty }) { uberAfterPair = false; pendingThread = u.thread }
        // the uberbot's chat follows a home move: the same row, the same thread, now drawn from the machine that holds it
        if let o = open { if let b = bots.first(where: { $0.thread == o }) { openBot = b } else if let to = DBot.followed(o, openBot, r.bots) { openBot = nil
            // the chat is re-pointed once its row is in: until then it would draw a thread with no name (Nov 11: "Thread" for two seconds)
            if threads.contains(where: { $0.id == to }) { pushThread = to } else { let g = gen; Task { try? await self.loadThreads(g); self.pushThread = to } } } }
    }
    func roomPosts(_ room: String) async -> [DPost] { ((try? await call("room.posts", ["room": room, "limit": 60]) as DPosts)?.posts) ?? [] }
    func roomPost(_ room: String, _ text: String) async throws { _ = try await call("room.post", ["room": room, "text": text, "clientId": UUID().uuidString], as: DEmpty.self) }

    // MARK: swarms (feature swarm, docs/SWARM.md Client data)
    func loadSwarms() async { guard has("swarm"), let r: DSwarmList = try? await call("swarm.list", ["project": ""]) else { return }; swarms = r.swarms }
    @ObservationIgnored private var swarmsDue = false
    /// a board, state or member turn changed: the list's rows again, once for a burst
    private func swarmsMoved() { guard !swarmsDue else { return }; swarmsDue = true; Task { try? await Task.sleep(nanoseconds: 400_000_000); swarmsDue = false; await loadSwarms() } }
    func swarmGet(_ room: String, since: String = "") async throws -> DSwarmBoard {
        var p: [String: Any] = ["room": room]; if !since.isEmpty { p["since"] = since }
        return try await call("swarm.get", p)
    }
    func swarmPost(_ room: String, _ text: String) async throws { _ = try await call("swarm.post", ["room": room, "text": text, "clientId": UUID().uuidString], as: DEmpty.self) }
    func swarmAct(_ room: String, _ action: String, _ extra: [String: Any] = [:]) async throws {
        _ = try await call("swarm.act", extra.merging(["room": room, "action": action]) { _, b in b }, as: DEmpty.self)
        await loadSwarms()
    }
    func swarmCreate(_ p: [String: Any]) async throws -> String {
        let r: DSwarmCreated = try await call("swarm.create", p)
        await loadSwarms(); return r.room
    }
    func modelsList() async -> [String: [DModelRow]] { guard has("models"), let r: DModelsList = try? await call("models.list") else { return [:] }; return r.models }

    // MARK: alerts: only what the hub's gate lets through reaches here (native/host/src/push): an APNs push, or the same on the socket for a phone with no token
    private func notified(_ o: [String: Any]) {
        guard pushToken == nil, UIApplication.shared.applicationState != .active else { return }
        guard AppSettings.shared.alerts || o["through"] as? Bool == true else { return }
        let n = UNMutableNotificationContent()
        n.title = (o["title"] as? String ?? "").isEmpty ? "Backplane" : o["title"] as? String ?? ""
        n.body = o["body"] as? String ?? ""
        let thread = o["thread"] as? String ?? "", key = o["key"] as? String ?? ""
        n.threadIdentifier = thread
        n.userInfo = ["thread": thread, "ask": o["ask"] as? String ?? "", "key": key]
        if let b = o["badge"] as? Int { n.badge = NSNumber(value: b) }
        if AppSettings.shared.alertSound { n.sound = .default }
        UNUserNotificationCenter.current().add(UNNotificationRequest(identifier: key, content: n, trigger: nil))
    }
    /// A clear from the hub: what was dealt with leaves this phone, by its key.
    func cleared(_ keys: [String], badge: Int?) async {
        let c = UNUserNotificationCenter.current()
        if let badge { try? await c.setBadgeCount(badge) }
        let gone = await c.deliveredNotifications().filter { keys.contains($0.request.content.userInfo["key"] as? String ?? "") }.map(\.request.identifier)
        if !gone.isEmpty { c.removeDeliveredNotifications(withIdentifiers: gone) }
    }
    private struct PushState: Decodable { var keys: [String]; var badge: Int; var quiet: String; var urgent: Bool }
    /// What is waiting, on connect and on coming to the front: the badge, and whatever was dealt with while this phone slept leaves it.
    func syncPush() async {
        guard has("push"), let s = try? await call("push.state", as: PushState.self) else { return }
        quiet = s.quiet; quietUrgent = s.urgent
        let c = UNUserNotificationCenter.current()
        try? await c.setBadgeCount(s.badge)
        let stale = await c.deliveredNotifications().filter { !s.keys.contains($0.request.content.userInfo["key"] as? String ?? "") }.map(\.request.identifier)
        if !stale.isEmpty { c.removeDeliveredNotifications(withIdentifiers: stale) }
    }
    /// Alerts not decided yet: DataAlertsBanner says why and offers them; the system's question comes only from its Allow alerts. Not now hides it until the next launch.
    var alertsAsk = false
    /// Launch, and once a hub can use it: take a token when alerts are allowed; when they are not decided yet, offer them (DataAlertsBanner) instead of asking at once.
    func enablePush(ask: Bool = false) async {
        guard has("push") else { return }
        let c = UNUserNotificationCenter.current()
        var status = await c.notificationSettings().authorizationStatus
        if status == .notDetermined, AppSettings.shared.alerts {
            if !ask { alertsAsk = true; return }
            alertsAsk = false
            _ = try? await c.requestAuthorization(options: [.alert, .sound, .badge])
            status = await c.notificationSettings().authorizationStatus
        }
        if status == .authorized || status == .provisional || status == .ephemeral { UIApplication.shared.registerForRemoteNotifications() }
    }
    func tokenGot(_ token: String) { pushToken = token; Task { await registerPush() } }
    /// The name the hub's Notifications page gives this phone (native/host/src/push/page.rs `handle`: twelve hex digits of the token's SHA-256); empty with no token.
    var pushHandle: String { pushToken.map { SHA256.hash(data: Data($0.utf8)).prefix(6).map { String(format: "%02x", $0) }.joined() } ?? "" }
    private func registerPush() async {
        guard has("push"), let t = pushToken else { return }
        #if DEBUG
        let env = "sandbox"
        #else
        let env = "production"
        #endif
        _ = try? await call("device.register", ["token": t, "platform": "ios", "kind": "alert", "env": env, "bundle": Bundle.main.bundleIdentifier ?? ""], as: DEmpty.self)
        await pushMute()
    }
    /// This phone's switch: off, only what the person let through reaches it.
    func pushMute() async {
        guard has("push"), let t = pushToken else { return }
        _ = try? await call("device.prefs", ["token": t, "mute": !AppSettings.shared.alerts], as: DEmpty.self)
    }
    func setQuiet(_ spec: String) async { quiet = spec; _ = try? await call("push.prefs", ["notify.quiet": spec], as: DEmpty.self) }
    func setQuietUrgent(_ on: Bool) async { quietUrgent = on; _ = try? await call("push.prefs", ["notify.quiet.urgent": on ? "on" : "off"], as: DEmpty.self) }
    /// A tapped notification opens its thread, at its approval when it has one.
    func openPush(thread: String, ask: String, link: String = "", hub: String = "") {
        // a push from another hub on the list (its url in `hub`): that hub first, then what it links to
        if let k = Pairing.key(hub), !k.isEmpty, k != usingKey, links.contains(where: { Pairing.key($0) == k }) {
            if k == homeKey, links.first.flatMap(Pairing.key) != k { if !onHome { close(); onHome = true; reset(); loadOutbox(); connect() } }
            else { useHub(k) }
        }
        // a link the hub put on it opens that instead: "account:<id>" is that account's sign-in (an expired sign-in's alert)
        if link.hasPrefix("account:") { pushAccount = String(link.dropFirst(8)); return }
        guard !thread.isEmpty else { return }
        pendingAsk = ask; pushThread = thread
    }
    var viewing: String? { open }

    // MARK: cache (paints at launch, replaced as soon as the hub answers)
    private struct Cache: Codable { var hub: String; var projects: [DProject]; var threads: [DThread]; var entries: [String: [DEntry]]; var older: [String: String] }
    private var cacheURL: URL {
        let d = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        try? FileManager.default.createDirectory(at: d, withIntermediateDirectories: true)
        return d.appendingPathComponent("data-cache-v1.json")
    }
    private func loadCache() {
        guard let hub = link.flatMap({ Pairing.key($0) }), let d = try? Data(contentsOf: cacheURL),
              let c = try? JSONDecoder().decode(Cache.self, from: d), c.hub == hub else { return }
        projects = c.projects; threads = c.threads; entries = c.entries; older = c.older; listsLoaded = !c.threads.isEmpty || !c.projects.isEmpty
    }
    private func scheduleSave() {
        saveTask?.cancel()
        saveTask = Task { [weak self] in
            try? await Task.sleep(for: .seconds(1))
            guard !Task.isCancelled, let self, let hub = self.link.flatMap({ Pairing.key($0) }) else { return }
            // the ten most recent threads' newest entries
            let recent = Set(self.threads.prefix(10).map(\.id))
            var e: [String: [DEntry]] = [:]
            for (k, v) in self.entries where recent.contains(k) { e[k] = Array(v.filter { !$0.sending && !$0.failed }.suffix(40)) }
            let c = Cache(hub: hub, projects: self.projects, threads: self.threads, entries: e, older: self.older.filter { recent.contains($0.key) })
            let url = self.cacheURL
            await Task.detached(priority: .utility) { if let d = try? JSONEncoder().encode(c) { try? d.write(to: url, options: .atomic) } }.value
        }
    }
}
