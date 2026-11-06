import Foundation
import SwiftUI
import UIKit
import UserNotifications

// The phone's whole link to a hub, docs/PHONE-DATA.md (data/1): one WebSocket of plain JSON data, typed values in, typed values out, a disk cache
// that paints at launch, deltas for the lists and the open thread only. No server-drawn views, no bridge, no engine.
@MainActor
@Observable
final class DataClient {
    /// A thread a link in a message asked to open (backplane://thread/<id>); the root screen takes it.
    var pendingThread: String?
    static let shared = DataClient()
    enum Conn: String { case offline, connecting, online }

    // what is shown
    private(set) var conn: Conn = .offline
    private(set) var needsRepair = false
    private(set) var failure = ""
    private(set) var projects: [DProject] = []
    private(set) var threads: [DThread] = []
    private(set) var entries: [String: [DEntry]] = [:]
    private(set) var older: [String: String] = [:]
    private(set) var loadedThreads: Set<String> = []
    private(set) var listsLoaded = false
    let plots = PlotStore()
    let voice = VoiceModel()
    private(set) var shot = 0
    private(set) var shotBot = ""
    private(set) var bots: [DBot] = []
    private(set) var rooms: [DRoom] = []
    // what the hub says it can do beyond the base of data/1 (hello.features); a hub that names none takes no pictures
    private(set) var features: Set<String> = []
    // hello `notice`, newer from /hub/status: the banner
    private(set) var notice = HubNotice()
    var canSendImages: Bool { features.contains("images") }
    var pairingError = ""
    var pairingBusy = false
    private(set) var links: [String] = PairingStore.load()
    var drafts: [String: String] = (UserDefaults.standard.dictionary(forKey: "data.drafts") as? [String: String]) ?? [:]

    // the hub being used (one in this build)
    var link: String? { links.first }
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
    @ObservationIgnored private var saveTask: Task<Void, Never>?
    @ObservationIgnored private var listNext: String?

    init() {
        #if DEBUG || BP_MEASURE
        if let l = ProcessInfo.processInfo.environment["BACKPLANE_LINK"] { links = l.split(separator: " ").map(String.init) }
        #endif
        loadCache()
        voice.client = self
    }

    // MARK: pairing
    func pair(_ text: String) {
        if let invite = Pairing.invitation(text) {
            guard !pairingBusy else { return }
            pairingBusy = true; pairingError = "Connecting…"
            Task {
                defer { pairingBusy = false }
                do { let saved = try await Pairing.exchange(invite, name: UIDevice.current.name); pairingError = ""; adopt(saved) }
                catch { pairingError = (error as NSError).domain == "BackplanePairing" ? error.localizedDescription : Pairing.message("hub_unreachable") }
            }
            return
        }
        if Pairing.malformedInvitation(text) { pairingError = "Enter the complete invitation link or a valid eight-character code."; return }
        adopt(text.trimmingCharacters(in: .whitespacesAndNewlines))
    }
    private func adopt(_ link: String) {
        guard Pairing.key(link) != nil else { pairingError = "Enter the hub's address or pairing link."; return }
        let changed = Pairing.key(links.first ?? "") != Pairing.key(link)
        links = [link]
        PairingStore.save(links)
        needsRepair = false
        if changed { reset() }
        connect()
    }
    func unpair() {
        links = []; PairingStore.save(links); reset(); close(); conn = .offline
    }
    private func reset() {
        notice = HubNotice()
        projects = []; threads = []; entries = [:]; older = [:]; loadedThreads = []; listsLoaded = false; open = nil
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
    }
    // the app came back to the front: a socket that died while suspended is replaced at once
    func foreground() {
        guard link != nil else { return }
        if conn != .online || Date().timeIntervalSince(heard) > 20 { connect() }
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
        guard link != nil, !needsRepair else { return }
        let wait = backoff
        backoff = min(backoff * 2, 4)
        reconnectTask = Task { [weak self] in
            try? await Task.sleep(for: .seconds(wait))
            guard !Task.isCancelled else { return }
            self?.connect()
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
            conn = .online; failure = ""; backoff = 0.25; plots.reset()
            Task { await self.synchronise(g) }
        case "reply":
            guard let id = o["id"] as? Int ?? (o["id"] as? Double).map(Int.init), let k = calls.removeValue(forKey: id) else { return }
            if o["ok"] as? Bool == true {
                k.resume(returning: (try? JSONSerialization.data(withJSONObject: o["result"] ?? [:])) ?? Data("{}".utf8))
            } else if let e = o["error"] as? [String: Any] {
                k.resume(throwing: DError(code: e["code"] as? String ?? "ERROR", message: e["message"] as? String ?? ""))
            } else { k.resume(throwing: DError(code: "ERROR", message: o["error"] as? String ?? "The hub refused")) }
        case "d": push(o)
        case "voice": voice.frame(o)
        case "shot": shotBot = o["bot"] as? String ?? ""; shot = (o["n"] as? Int) ?? ((o["n"] as? Double).map(Int.init) ?? 0)
        default: break
        }
    }

    func call<T: Decodable>(_ m: String, _ p: [String: Any] = [:], as: T.Type = T.self) async throws -> T {
        guard conn != .offline, let task else { throw DError(code: "OFFLINE", message: "Offline") }
        let id = nextId; nextId += 1
        let msg = try JSONSerialization.data(withJSONObject: ["id": id, "m": m, "p": p])
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
            async let p = loadProjects(g), t = loadThreads(g)
            _ = try await (p, t)
            await loadBots()
            await voice.resync()
            if let open { try await subscribeThread(open, g); await refreshThread(open) }
        } catch { if g == gen { failure = error.localizedDescription } }
    }
    func refresh() async { guard conn == .online else { return }; let g = gen; _ = try? await loadProjects(g); _ = try? await loadThreads(g) }
    private func loadProjects(_ g: Int) async throws {
        let r: DProjectsPage = try await call("projects.list", ["limit": 100]); guard g == gen else { return }
        projects = r.projects.sorted { $0.updatedAt > $1.updatedAt }; listsLoaded = true; scheduleSave()
    }
    private func loadThreads(_ g: Int) async throws {
        let r: DThreadsPage = try await call("threads.list", ["limit": 200]); guard g == gen else { return }
        threads = r.threads.sorted { $0.updatedAt > $1.updatedAt }; listNext = r.next; listsLoaded = true; scheduleSave()
    }

    // opening a thread: what the cache has shows now; the hub's newest page replaces it
    func openThread(_ id: String) {
        open = id
        guard conn == .online else { return }
        let g = gen
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
        func decode<T: Decodable>(_ key: String, _ type: T.Type) -> T? {
            guard let v = o[key], let d = try? JSONSerialization.data(withJSONObject: v) else { return nil }
            return try? JSONDecoder().decode(T.self, from: d)
        }
        switch k {
        case "thread": if let t = decode("thread", DThread.self) { upsert(t) }
        case "thread.removed":
            if let id = o["id"] as? String { threads.removeAll { $0.id == id }; entries[id] = nil; scheduleSave() }
        case "project.removed":
            if let id = o["id"] as? String { projects.removeAll { $0.id == id }; scheduleSave() }
        case "project":
            if let p = decode("project", DProject.self) {
                if let i = projects.firstIndex(where: { $0.id == p.id }) { projects[i] = p } else { projects.append(p) }
                projects.sort { $0.updatedAt > $1.updatedAt }; scheduleSave()
            }
        case "entry":
            if let tid = o["thread"] as? String, let e = decode("entry", DEntry.self) { put(e, in: tid) }
        case "text":
            if let tid = o["thread"] as? String, let eid = o["entry"] as? String, let add = o["append"] as? String,
               var l = entries[tid], let i = l.firstIndex(where: { $0.id == eid }) {
                l[i].text += add; l[i].live = true; entries[tid] = l
            }
        case "notify":
            if let tid = o["thread"] as? String, let kind = o["kind"] as? String { notifyLocal(tid, kind, title: o["title"] as? String ?? "", body: o["body"] as? String ?? "") }
        case "turn":
            if let tid = o["thread"] as? String, let s = o["state"] as? String, let i = threads.firstIndex(where: { $0.id == tid }) {
                let was = threads[i].state
                threads[i].state = s
                if !features.contains("notify"), was == "working" || was == "asking", s == "idle" || s == "asking" || s == "failed" { alert(threads[i], s) }
            }
        default: break
        }
    }
    private func upsert(_ t: DThread) {
        if let i = threads.firstIndex(where: { $0.id == t.id }) {
            let was = threads[i].state
            threads[i] = t
            if !features.contains("notify"), (was == "working"), t.state == "idle" || t.state == "asking" || t.state == "failed" { alert(t, t.state) }
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
    func send(_ tid: String, _ text: String, images: [Data] = []) {
        let text = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty || !images.isEmpty else { return }
        let cid = UUID().uuidString
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
        } catch {
            if var l = entries[tid], let i = l.firstIndex(where: { $0.clientId == cid }) { l[i].sending = false; l[i].failed = true; entries[tid] = l }
        }
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

    // MARK: data/1 additions (features actions, search, models, mentions, bots, notify; the hub's native/host/src/data/ext.rs)
    func has(_ f: String) -> Bool { features.contains(f) }
    func threadAct(_ id: String, _ action: String, title: String = "") async {
        var p: [String: Any] = ["thread": id, "action": action]; if !title.isEmpty { p["title"] = title }
        do {
            let r: DThreadOpt = try await call("thread.act", p)
            if action == "delete" { threads.removeAll { $0.id == id }; entries[id] = nil; scheduleSave() }
            else if let t = r.thread { upsert(t) }
        } catch { failure = error.localizedDescription }
    }
    func badgeList() async -> DBadgeList? { try? await call("badge.list") }
    func projectAct(_ id: String, _ action: String, title: String = "", value: String = "") async {
        var p: [String: Any] = ["project": id, "action": action]; if !title.isEmpty { p["title"] = title }; if !value.isEmpty { p["value"] = value }
        do {
            _ = try await call("project.act", p, as: DEmpty.self)
            if action == "remove" { projects.removeAll { $0.id == id }; threads.removeAll { $0.project == id }; scheduleSave() } else { await refresh() }
        } catch { failure = error.localizedDescription }
    }
    func search(_ q: String) async -> DSearch? { try? await call("search", ["q": q]) }
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
        bots = r.bots; rooms = r.rooms
    }
    func roomPosts(_ room: String) async -> [DPost] { ((try? await call("room.posts", ["room": room, "limit": 60]) as DPosts)?.posts) ?? [] }
    func roomPost(_ room: String, _ text: String) async throws { _ = try await call("room.post", ["room": room, "text": text, "clientId": UUID().uuidString], as: DEmpty.self) }

    // MARK: alerts: while the app still runs (iOS keeps the socket a short while in the background), a turn that ended or asks tells the person
    private func alert(_ t: DThread, _ state: String) {
        guard AppSettings.shared.alerts, UIApplication.shared.applicationState != .active else { return }
        let n = UNMutableNotificationContent()
        n.title = t.title
        n.body = state == "asking" ? "Waiting for your answer" : state == "failed" ? "The turn failed" : (t.preview.isEmpty ? "Finished" : t.preview)
        if AppSettings.shared.alertSound { n.sound = .default }
        UNUserNotificationCenter.current().add(UNNotificationRequest(identifier: "turn-" + t.id, content: n, trigger: nil))
    }
    // the hub's own `notify` push (a turn ended, failed, or an ask opened): a local notification while the app is not on screen
    private func notifyLocal(_ tid: String, _ kind: String, title: String, body: String) {
        guard AppSettings.shared.alerts, UIApplication.shared.applicationState != .active else { return }
        let n = UNMutableNotificationContent()
        n.title = title.isEmpty ? "Backplane" : title
        n.body = kind == "ask" ? "Waiting for your answer" + (body.isEmpty ? "" : ": " + body) : kind == "fail" ? "The turn failed" : "Finished"
        n.userInfo = ["thread": tid]
        if AppSettings.shared.alertSound { n.sound = .default }
        UNUserNotificationCenter.current().add(UNNotificationRequest(identifier: "turn-" + tid, content: n, trigger: nil))
    }
    func askNotificationPermission() async {
        _ = try? await UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound, .badge])
    }

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
