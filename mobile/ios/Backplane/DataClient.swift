import Foundation
import SwiftUI
import UIKit
import UserNotifications

// The phone's whole link to a hub, docs/gpui/PHONE-DATA.md (data/1): one WebSocket of plain JSON data, typed values in, typed values out, a disk cache
// that paints at launch, deltas for the lists and the open thread only. No server-drawn views, no bridge, no engine.
@MainActor
@Observable
final class DataClient {
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
        projects = []; threads = []; entries = [:]; older = [:]; loadedThreads = []; listsLoaded = false; open = nil
        try? FileManager.default.removeItem(at: cacheURL)
    }

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
                    case .data(let d): self.handle(d, g)
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
            conn = .online; failure = ""; backoff = 0.25
            Task { await self.synchronise(g) }
        case "reply":
            guard let id = o["id"] as? Int ?? (o["id"] as? Double).map(Int.init), let k = calls.removeValue(forKey: id) else { return }
            if o["ok"] as? Bool == true {
                k.resume(returning: (try? JSONSerialization.data(withJSONObject: o["result"] ?? [:])) ?? Data("{}".utf8))
            } else if let e = o["error"] as? [String: Any] {
                k.resume(throwing: DError(code: e["code"] as? String ?? "ERROR", message: e["message"] as? String ?? ""))
            } else { k.resume(throwing: DError(code: "ERROR", message: o["error"] as? String ?? "The hub refused")) }
        case "d": push(o)
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
        Task {
            do { try await subscribeThread(id, g); await refreshThread(id) } catch {}
        }
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
        case "turn":
            if let tid = o["thread"] as? String, let s = o["state"] as? String, let i = threads.firstIndex(where: { $0.id == tid }) {
                let was = threads[i].state
                threads[i].state = s
                if was == "working" || was == "asking", s == "idle" || s == "asking" || s == "failed" { alert(threads[i], s) }
            }
        default: break
        }
    }
    private func upsert(_ t: DThread) {
        if let i = threads.firstIndex(where: { $0.id == t.id }) {
            let was = threads[i].state
            threads[i] = t
            if (was == "working"), t.state == "idle" || t.state == "asking" || t.state == "failed" { alert(t, t.state) }
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
    func send(_ tid: String, _ text: String) {
        let text = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return }
        let cid = UUID().uuidString
        var local = DEntry(id: "local:" + cid, kind: "user", text: text, at: Int(Date().timeIntervalSince1970 * 1000), clientId: cid)
        local.sending = true
        entries[tid, default: []].append(local)
        setDraft(tid, "")
        Task { await deliver(tid, text, cid) }
    }
    func retry(_ tid: String, _ entry: DEntry) {
        guard let cid = entry.clientId, var l = entries[tid], let i = l.firstIndex(where: { $0.id == entry.id }) else { return }
        l[i].failed = false; l[i].sending = true; entries[tid] = l
        Task { await deliver(tid, entry.text, cid) }
    }
    private func deliver(_ tid: String, _ text: String, _ cid: String) async {
        do {
            let r: DEntryReply = try await call("message.send", ["thread": tid, "text": text, "clientId": cid])
            put(r.entry, in: tid)
        } catch {
            if var l = entries[tid], let i = l.firstIndex(where: { $0.clientId == cid }) { l[i].sending = false; l[i].failed = true; entries[tid] = l }
        }
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
    func createThread(project: String, text: String) async throws -> DThread {
        var p: [String: Any] = ["project": project, "clientId": UUID().uuidString, "permission": "ask"]
        let t = text.trimmingCharacters(in: .whitespacesAndNewlines); if !t.isEmpty { p["text"] = t }
        let r: DThreadReply = try await call("thread.create", p)
        upsert(r.thread)
        return r.thread
    }
    func dirs(_ path: String?) async throws -> DDirs { try await call("dirs.list", path.map { ["path": $0] } ?? [:]) }
    func setDraft(_ tid: String, _ text: String) {
        drafts[tid] = text.isEmpty ? nil : text
        UserDefaults.standard.set(drafts, forKey: "data.drafts")
    }

    // MARK: alerts: while the app still runs (iOS keeps the socket a short while in the background), a turn that ended or asks tells the person
    private func alert(_ t: DThread, _ state: String) {
        guard AppSettings.shared.alerts, UIApplication.shared.applicationState != .active else { return }
        let n = UNMutableNotificationContent()
        n.title = t.title
        n.body = state == "asking" ? "Waiting for your answer" : state == "failed" ? "The turn failed" : (t.preview.isEmpty ? "Finished" : t.preview)
        if AppSettings.shared.alertSound { n.sound = .default }
        UNUserNotificationCenter.current().add(UNNotificationRequest(identifier: "turn-" + t.id, content: n, trigger: nil))
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
