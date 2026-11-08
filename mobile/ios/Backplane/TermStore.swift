import Foundation
import Observation

// The phone's terminals (feature `terms`; docs/PHONE-DATA.md, Terminals). One TermScreen per terminal a view has asked for: its emulator (Vt), the run and offset it holds
// (TermHeld) and its state. While a card or the full screen is up the hub streams output to this socket (`term.watch`); off screen, or with the app in the background,
// `term.unwatch` and the last screen stays drawn. Redraws happen only when bytes (or a state) arrive: no timers, no polling.

@MainActor @Observable
final class TermScreen {
    let key: String
    @ObservationIgnored var vt = Vt(cols: 80, rows: 24)
    @ObservationIgnored var held: TermHeld?
    private(set) var state = ""
    private(set) var title = ""
    private(set) var missing = false     // the hub does not know it (anymore)
    private(set) var rev = 0             // changes with every redraw
    init(key: String, cols: Int = 80, rows: Int = 24) { self.key = key; vt = Vt(cols: cols, rows: rows) }
    var live: Bool { TermCard.live(state) }
    func touch() { rev &+= 1 }
    func set(state s: String) { if s != state { state = s; rev &+= 1 } }
    func set(title t: String) { if !t.isEmpty, t != title { title = t } }
    func set(missing m: Bool) { if m != missing { missing = m; rev &+= 1 } }
}

@MainActor @Observable
final class TermStore {
    @ObservationIgnored weak var client: DataClient?
    @ObservationIgnored private(set) var screens: [String: TermScreen] = [:]
    @ObservationIgnored private var wanted: [String: Int] = [:]     // views showing the terminal
    @ObservationIgnored private var watching: Set<String> = []      // watched on this socket
    @ObservationIgnored private var reading: Set<String> = []       // a read in flight
    @ObservationIgnored private var sent: [String: String] = [:]    // the last "cols x rows" asked of the pty
    @ObservationIgnored private var paused = false
    /// The terminal open full screen (the root screen presents it).
    var fullscreen: TermRoute?

    func screen(_ key: String, cols: Int = 80, rows: Int = 24) -> TermScreen {
        if let s = screens[key] { return s }
        let s = TermScreen(key: key, cols: cols, rows: rows); screens[key] = s; return s
    }
    func open(_ key: String, title: String) { fullscreen = TermRoute(key: key, title: title) }

    // MARK: watching
    /// A view shows this terminal: stream it.
    func want(_ key: String, cols: Int = 80, rows: Int = 24) {
        _ = screen(key, cols: cols, rows: rows)
        wanted[key, default: 0] += 1
        if wanted[key] == 1 { startWatch(key) }
    }
    /// The view is gone (off screen, or its screen closed).
    func unwant(_ key: String) {
        guard let n = wanted[key] else { return }
        if n > 1 { wanted[key] = n - 1; return }
        wanted[key] = nil
        if watching.remove(key) != nil { Task { _ = try? await client?.call("term.unwatch", ["key": key], as: DEmpty.self) } }
    }
    private func online() -> Bool { !paused && client?.conn == .online && client?.has("terms") == true }
    private func startWatch(_ key: String) {
        guard online(), !watching.contains(key), let s = screens[key] else { return }
        watching.insert(key)
        var p: [String: Any] = ["key": key]
        if let h = s.held { p["g"] = h.g; p["from"] = h.o }
        Task {
            do { let r: DTermRead = try await client!.call("term.watch", p); take(r, s) }
            catch {
                watching.remove(key)
                if let e = error as? DError, e.code != "OFFLINE", e.code != "TIMEOUT", !e.message.contains("too many") { s.set(missing: true) }
            }
        }
    }
    /// A new socket, or the app came back: ask again for everything still on screen, with what is held so nothing is lost.
    func resync() { watching = []; reading = []; for k in wanted.keys { startWatch(k) } }
    /// The app is in front again.
    func resume() { paused = false; resync() }
    /// The socket is gone: the hub forgot what this socket watched.
    func disconnected() { watching = []; reading = [] }
    /// The app went to the background: stop the stream (the held screens stay).
    func pause() {
        paused = true
        for k in watching { Task { _ = try? await client?.call("term.unwatch", ["key": k], as: DEmpty.self) } }
        watching = []
    }

    // MARK: bytes
    /// A `{t:"d", k:"term"}` push.
    func push(_ o: [String: Any]) {
        guard let key = o["key"] as? String, let s = screens[key] else { return }
        let b = (o["b"] as? String).flatMap { Data(base64Encoded: $0) }.map { [UInt8]($0) } ?? []
        func int(_ k: String) -> Int? { (o[k] as? NSNumber)?.intValue }
        let reset = (o["reset"] as? [NSNumber])?.map(\.intValue)
        for a in TermStream.apply(held: s.held, g: int("g"), o: int("o"), bytes: b, reset: reset, state: o["state"] as? String) { perform(a, s) }
    }
    private func perform(_ a: TermAction, _ s: TermScreen) {
        switch a {
        case .none: break
        case .state(let st): s.set(state: st)
        case .feed(let bytes, let held): s.vt.feed(bytes); s.held = held; s.touch()
        case .snapshot(let c, let r, let bytes, let held):
            snapshot(s, cols: c, rows: r, bytes: bytes, held: held)
        case .reread: reread(s)
        }
    }
    /// A new screen of this size drawn by `bytes`; the scrollback stays when it is the same run.
    private func snapshot(_ s: TermScreen, cols: Int, rows: Int, bytes: [UInt8], held: TermHeld) {
        s.vt = s.held?.g == held.g ? Vt(cols: cols, rows: rows, keepingHistoryOf: s.vt) : Vt(cols: cols, rows: rows)
        s.vt.feed(bytes); s.held = held; s.touch()
    }
    private func reread(_ s: TermScreen) {
        guard online(), !reading.contains(s.key) else { return }
        reading.insert(s.key)
        var p: [String: Any] = ["key": s.key]
        if let h = s.held { p["g"] = h.g; p["from"] = h.o }
        Task {
            defer { reading.remove(s.key) }
            if let r: DTermRead = try? await client?.call("term.read", p) { take(r, s) }
        }
    }
    /// The answer of `term.read`/`term.watch`/`term.new`: the bytes after `from`, or a snapshot. A push may have moved the screen on meanwhile; only what is new is taken.
    func take(_ r: DTermRead, _ s: TermScreen) {
        s.set(missing: false); s.set(state: r.state); s.set(title: r.title)
        if let reset = r.reset, reset.count == 2 {
            if let h = s.held, h.g == r.g, h.o > r.o { return }
            snapshot(s, cols: reset[0], rows: reset[1], bytes: r.bytes, held: TermHeld(g: r.g, o: r.o)); return
        }
        guard let h = s.held, h.g == r.g else {
            // an answer to a read from nothing is a snapshot; this one is not: ask for one
            s.held = nil; reread(s); return
        }
        let b = r.bytes
        switch TermStream.merge(held: h.o, o: r.from ?? h.o, len: b.count) {
        case .take(let i): s.vt.feed(Array(b[i...])); s.held = TermHeld(g: r.g, o: (r.from ?? h.o) + b.count); s.touch()
        case .skip: break
        case .gap: s.held = nil; reread(s)
        }
    }

    // MARK: input
    func write(_ key: String, _ bytes: [UInt8]) {
        guard !bytes.isEmpty, let c = client else { return }
        let b = Data(bytes).base64EncodedString()
        Task { _ = try? await c.call("term.write", ["key": key, "b": b], as: DEmpty.self) }
    }
    /// The grid that fits the full screen changed.
    func resize(_ key: String, cols: Int, rows: Int) {
        let tag = "\(cols)x\(rows)"
        guard sent[key] != tag, let c = client else { return }
        sent[key] = tag
        Task { _ = try? await c.call("term.resize", ["key": key, "cols": cols, "rows": rows], as: DEmpty.self) }
    }
    /// The next time the full screen opens it tells the pty its size again.
    func forgetSize(_ key: String) { sent[key] = nil }
    func stop(_ key: String) { guard let c = client else { return }; Task { _ = try? await c.call("term.stop", ["key": key], as: DEmpty.self) } }

    // MARK: lists
    func list(project: String) async -> [DTermRow]? {
        guard let c = client else { return nil }
        var p: [String: Any] = [:]; if !project.isEmpty { p["project"] = project }
        return (try? await c.call("term.list", p, as: DTermList.self))?.terms
    }
    /// A new login shell (in the project's folder, or the hub's home); its key.
    func newShell(project: String, cols: Int, rows: Int) async throws -> String {
        guard let c = client else { throw DError(code: "OFFLINE", message: "Offline") }
        var p: [String: Any] = ["cols": cols, "rows": rows]; if !project.isEmpty { p["project"] = project }
        let r: DTermRead = try await c.call("term.new", p)
        let s = screen(r.key); take(r, s)
        return r.key
    }
}
