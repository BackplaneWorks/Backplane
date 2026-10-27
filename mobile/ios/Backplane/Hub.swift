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
        guard let u = URLComponents(string: s), let host = u.host, !host.isEmpty,
              let scheme = u.scheme?.lowercased(), ["http", "https", "ws", "wss"].contains(scheme) else { return nil }
        let fragment = URLComponents(string: "?" + (u.fragment ?? ""))?.queryItems ?? []
        let token = (fragment + (u.queryItems ?? [])).first { $0.name == "token" }?.value
        var w = URLComponents()
        w.scheme = scheme == "https" || scheme == "wss" ? "wss" : "ws"
        w.host = host
        w.port = u.port
        w.path = "/ws"
        if let token { w.queryItems = [URLQueryItem(name: "token", value: token)] }
        return w.url
    }
}

// Only typed, secret-free causes cross into the UI. Foundation error text and
// userInfo can contain the full socket URL (and its pairing credential).
enum ConnectionFailure: Equatable {
    case timeout, network, dns, tls, authentication, connection, invalidAddress, invalidResponse, view
    case http(Int)

    var reason: String {
        switch self {
        case .timeout: "timeout"
        case .network: "network"
        case .dns: "dns"
        case .tls: "tls"
        case .authentication: "authentication"
        case .connection: "connection"
        case .invalidAddress: "address"
        case .invalidResponse: "response"
        case .view: "view"
        case .http: "http"
        }
    }

    var code: String { if case .http(let code) = self { return String(code) }; return "" }

    static func classify(_ error: Error) -> ConnectionFailure {
        let e = error as NSError
        guard e.domain == NSURLErrorDomain else { return .connection }
        switch e.code {
        case NSURLErrorTimedOut: return .timeout
        case NSURLErrorCannotFindHost, NSURLErrorDNSLookupFailed: return .dns
        case NSURLErrorNotConnectedToInternet, NSURLErrorCannotConnectToHost, NSURLErrorNetworkConnectionLost,
             NSURLErrorDataNotAllowed, NSURLErrorInternationalRoamingOff: return .network
        case NSURLErrorSecureConnectionFailed, NSURLErrorServerCertificateHasBadDate,
             NSURLErrorServerCertificateUntrusted, NSURLErrorServerCertificateHasUnknownRoot,
             NSURLErrorServerCertificateNotYetValid, NSURLErrorClientCertificateRejected,
             NSURLErrorClientCertificateRequired, NSURLErrorAppTransportSecurityRequiresSecureConnection: return .tls
        case NSURLErrorUserAuthenticationRequired: return .authentication
        default: return .connection
        }
    }

    static func response(_ status: Int) -> ConnectionFailure {
        status == 401 || status == 403 ? .authentication : .http(status)
    }
}

// The socket to the hub, reconnecting with backoff like host.js.
@MainActor
final class Hub {
    // What to do on each (re)connect: open this address (it carries
    // since/origin, so the hub sends only what this app has not seen), try
    // again after the backoff (the hub is down or restarting), or stop.
    enum Next { case go(URL), later(ConnectionFailure), stop }
    private let url: () async -> Next
    private let onConnecting: () -> Void
    private let onOpen: () -> Void
    private let onMessage: (Data) -> Void
    private let onClose: (ConnectionFailure) -> Void
    private var task: URLSessionWebSocketTask?
    private var backoff: Double = 0.25
    private var retry: DispatchWorkItem?
    private var stopped = true
    private var discovering = false
    private var discoveryStarted = Date()
    private var generation = 0
    // When a frame (a heartbeat included) last arrived. The hub sends one
    // every 15 s (core/desk.bend's Alive.every); a link silent for more
    // than Alive.limit (40 s) is dead, whatever TCP says, and is dropped
    // and reconnected from the client's sequence.
    private static let aliveLimit: TimeInterval = 40
    private var heard = Date()
    private var watch: Timer?

    init(url: @escaping () async -> Next, onConnecting: @escaping () -> Void, onOpen: @escaping () -> Void, onMessage: @escaping (Data) -> Void,
         onClose: @escaping (ConnectionFailure) -> Void) {
        self.url = url
        self.onConnecting = onConnecting
        self.onOpen = onOpen
        self.onMessage = onMessage
        self.onClose = onClose
    }

    func start() {
        guard stopped else { return }
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
        lost(.timeout)
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
            onClose(.connection)
            connect()
        } else if let r = retry {
            r.cancel()
            retry = nil
            connect()
        } else if !discovering || Date().timeIntervalSince(discoveryStarted) > quiet {
            // Discovery may have parked on an older hub without view support.
            // Recheck when the app returns or the network changes: the hub
            // may have been upgraded. A suspended discovery may also be stale;
            // its generation fence prevents its eventual answer from opening.
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
        discovering = false
        task?.cancel(with: .goingAway, reason: nil)
        task = nil
    }

    // A presentation can time out while heartbeats keep TCP healthy. Replace
    // that session through the normal backoff rather than waiting forever for
    // the silence watchdog. The bridge drops old pending actions on reconnect;
    // only the selected view is opened again.
    func reconnect(_ failure: ConnectionFailure) {
        guard !stopped, let t = task else { return }
        t.cancel(with: .goingAway, reason: nil)
        lost(failure)
    }

    // A greeting proves transport, not that the selected view can load. Keep
    // backing off across rejected views; reset only after the app accepts one.
    func ready() { backoff = 0.25 }

    // every frame is binary CBOR, both ways
    func send(_ data: Data) {
        guard let t = task else { return }
        let gen = generation
        t.send(.data(data)) { [weak self] error in
            guard let error else { return }
            Task { @MainActor in
                guard let self, !self.stopped, self.generation == gen, self.task === t else { return }
                self.lost(.classify(error))
            }
        }
    }

    private func connect() {
        guard !stopped else { return }
        generation += 1
        let gen = generation
        discovering = true
        discoveryStarted = Date()
        onConnecting()
        retry?.cancel()
        retry = nil
        Task {
            let next = await url()
            guard gen == generation, !stopped else { return }
            discovering = false
            let u: URL
            switch next {
            case .go(let to): u = to
            case .later(let failure): lost(failure); return
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
                        self.onOpen()
                    }
                    if case .data(let d) = m { self.onMessage(d) }
                    self.read(t, gen, first: false)
                case .failure(let error):
                    let status = (t.response as? HTTPURLResponse)?.statusCode
                    self.lost(status.map { $0 == 101 ? ConnectionFailure.classify(error) : .response($0) } ?? .classify(error))
                }
            }
        }
    }

    private func lost(_ failure: ConnectionFailure) {
        task?.cancel(with: .goingAway, reason: nil)
        task = nil
        discovering = false
        generation += 1
        onClose(failure)
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
