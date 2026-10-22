import Foundation

// A pairing link as the desktop shows it (http://host:3787/#token=abc)
// becomes the hub's socket address (ws://host:3787/ws?token=abc).
enum Pairing {

    static func viewSocket(_ link: String) -> URL? {
        guard let u = socket(link), var c = URLComponents(url: u, resolvingAgainstBaseURL: false) else { return nil }
        c.queryItems = (c.queryItems ?? []) + [URLQueryItem(name: "since", value: "0"), URLQueryItem(name: "origin", value: ""),
            URLQueryItem(name: "enc", value: "cbor"), URLQueryItem(name: "view", value: "1"), URLQueryItem(name: "mobile", value: "1")]
        return c.url
    }

    // the hub's key: host:port, the same for every link to it
    static func key(_ link: String) -> String? {
        guard let u = socket(link), let host = u.host() else { return nil }
        return u.port.map { "\(host):\($0)" } ?? host
    }

    // where the hub serves path ("/img?path=…"), over http(s), with the link's token
    static func web(_ link: String, _ path: String) -> URL? {
        guard let w = socket(link), var c = URLComponents(url: w, resolvingAgainstBaseURL: false),
              let p = URLComponents(string: path) else { return nil }
        c.scheme = w.scheme == "wss" ? "https" : "http"
        c.path = p.path
        c.percentEncodedQuery = [p.percentEncodedQuery, c.percentEncodedQuery].compactMap { $0 }.joined(separator: "&")
        return c.url
    }

    // an HTTP address on the hub (its token along, unless token is false)
    static func http(_ link: String, path: String, query: [URLQueryItem] = [], token: Bool = true) -> URL? {
        guard let w = socket(link), var c = URLComponents(url: w, resolvingAgainstBaseURL: false) else { return nil }
        c.scheme = c.scheme == "wss" ? "https" : "http"
        c.path = path
        let q = (token ? (c.queryItems ?? []).filter { $0.name == "token" } : []) + query
        c.queryItems = q.isEmpty ? nil : q
        return c.url
    }

    static func socket(_ link: String) -> URL? {
        var s = link.trimmingCharacters(in: .whitespacesAndNewlines)
        if s.isEmpty { return nil }
        if s.hasPrefix("backplane://") {
            guard let q = URLComponents(string: s)?.queryItems?.first(where: { $0.name == "url" })?.value else { return nil }
            s = q
        }
        if !s.contains("://") { s = "http://" + s }
        guard let u = URLComponents(string: s), let host = u.host else { return nil }
        let rest = (u.fragment ?? "") + "&" + (u.query ?? "")
        let token = rest.range(of: "token=[0-9a-f]+", options: .regularExpression).map { String(rest[$0].dropFirst(6)) }
        var w = URLComponents()
        w.scheme = u.scheme == "https" ? "wss" : "ws"
        w.host = host
        w.port = u.port
        w.path = "/ws"
        if let token { w.queryItems = [URLQueryItem(name: "token", value: token)] }
        return w.url
    }
}

// The socket to the hub, reconnecting with backoff like host.js.
@MainActor
final class Hub {
    // What to do on each (re)connect: open this address (it carries
    // since/origin, so the hub sends only what this app has not seen), try
    // again after the backoff (the hub is down or restarting), or stop.
    enum Next { case go(URL), later, stop }
    private let url: () async -> Next
    private let onConnecting: () -> Void
    private let onOpen: () -> Void
    private let onMessage: (Data) -> Void
    private let onClose: () -> Void
    private var task: URLSessionWebSocketTask?
    private var backoff: Double = 0.25
    private var retry: DispatchWorkItem?
    private var stopped = false
    private var generation = 0
    // When a frame (a heartbeat included) last arrived. The hub sends one
    // every 15 s (core/desk.bend's Alive.every); a link silent for more
    // than Alive.limit (40 s) is dead, whatever TCP says, and is dropped
    // and reconnected from the client's sequence.
    private static let aliveLimit: TimeInterval = 40
    private var heard = Date()
    private var watch: Timer?

    init(url: @escaping () async -> Next, onConnecting: @escaping () -> Void, onOpen: @escaping () -> Void, onMessage: @escaping (Data) -> Void,
         onClose: @escaping () -> Void) {
        self.url = url
        self.onConnecting = onConnecting
        self.onOpen = onOpen
        self.onMessage = onMessage
        self.onClose = onClose
    }

    func start() {
        stopped = false
        heard = Date()
        watch?.invalidate()
        watch = Timer.scheduledTimer(withTimeInterval: 5, repeats: true) { [weak self] _ in
            Task { @MainActor in self?.check() }
        }
        connect()
    }

    private func check() {
        guard !stopped, let t = task, Date().timeIntervalSince(heard) > Hub.aliveLimit else { return }
        t.cancel(with: .goingAway, reason: nil)
        lost()
    }

    // Try now rather than after the backoff: the network came back or the
    // app is in front again. A socket quiet for `quiet` seconds (it may have
    // died while the app was suspended) is replaced.
    func wake(quiet: TimeInterval = 3) {
        guard !stopped else { return }
        backoff = 0.25
        if let t = task {
            guard Date().timeIntervalSince(heard) > quiet else { return }
            t.cancel(with: .goingAway, reason: nil)
            task = nil
            generation += 1
            onClose()
            connect()
        } else if let r = retry {
            r.cancel()
            retry = nil
            connect()
        }
    }

    func stop() {
        stopped = true
        retry?.cancel()
        retry = nil
        watch?.invalidate()
        watch = nil
        generation += 1
        task?.cancel(with: .goingAway, reason: nil)
        task = nil
    }

    // every frame is binary CBOR, both ways
    func send(_ data: Data) {
        task?.send(.data(data)) { _ in }
    }

    private func connect() {
        guard !stopped else { return }
        generation += 1
        let gen = generation
        onConnecting()
        retry?.cancel()
        retry = nil
        Task {
            let next = await url()
            guard gen == generation, !stopped else { return }
            let u: URL
            switch next {
            case .go(let to): u = to
            case .later: lost(); return
            case .stop: return
            }
            let t = URLSession.shared.webSocketTask(with: u)
            // the first frame is the whole log (megabytes on a busy hub);
            // past the 1 MB default every receive fails and it reconnects forever
            t.maximumMessageSize = 64 << 20
            task = t
            heard = Date()
            t.resume()
            // the first message proves the socket is up (the hub greets at once)
            read(t, gen, first: true)
        }
    }

    private func read(_ t: URLSessionWebSocketTask, _ gen: Int, first: Bool) {
        t.receive { [weak self] r in
            Task { @MainActor in
                guard let self, gen == self.generation else { return }
                switch r {
                case .success(let m):
                    self.heard = Date()
                    if first {
                        self.backoff = 0.25
                        self.onOpen()
                    }
                    if case .data(let d) = m { self.onMessage(d) }
                    self.read(t, gen, first: false)
                case .failure:
                    self.lost()
                }
            }
        }
    }

    private func lost() {
        task = nil
        generation += 1
        onClose()
        guard !stopped else { return }
        // equal jitter: after a deploy the phones spread out, none at zero
        let wait = backoff / 2 + Double.random(in: 0...(backoff / 2))
        backoff = min(backoff * 2, 3)
        let r = DispatchWorkItem { [weak self] in
            guard let self else { return }
            self.retry = nil
            self.connect()
        }
        retry?.cancel()
        retry = r
        DispatchQueue.main.asyncAfter(deadline: .now() + wait, execute: r)
    }
}
