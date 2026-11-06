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

    static func token(_ link: String) -> String? {
        socket(link).flatMap { URLComponents(url: $0, resolvingAgainstBaseURL: false) }?.queryItems?.first { $0.name == "token" }?.value
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
        if s.hasPrefix("backplane://") || s.hasPrefix(AppIdentity.scheme + "://") {
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
    // every 15 s; a link silent for more
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

// Disposable invitation material is exchanged before AppModel changes its
// saved links. Failed/expired invites never replace a working pairing.
extension Pairing {
    final class NoPairRedirect: NSObject, URLSessionTaskDelegate, @unchecked Sendable {
        func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse,
                        newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void) { completionHandler(nil) }
    }
    struct Invitation {
        let origin: URL
        let secret: String
    }
    static func invitation(_ text: String) -> Invitation? {
        var raw = text.trimmingCharacters(in: .whitespacesAndNewlines)
        if raw.hasPrefix("backplane://") || raw.hasPrefix(AppIdentity.scheme + "://") { raw = URLComponents(string: raw)?.queryItems?.first(where: { $0.name == "url" })?.value ?? "" }
        guard var c = URLComponents(string: raw), ["https", "http"].contains(c.scheme), c.host != nil,
              c.user == nil, c.password == nil else { return nil }
        let f = URLComponents(string: "?" + (c.fragment ?? ""))?.queryItems ?? []
        guard let secret = f.first(where: { $0.name == "invite" || $0.name == "code" })?.value,
              secret.range(of: "^(?:[a-f0-9]{32}|[A-Z2-9]{8})$", options: .regularExpression) != nil else { return nil }
        c.path = ""; c.query = nil; c.fragment = nil
        guard let origin = c.url else { return nil }
        return Invitation(origin: origin, secret: secret)
    }
    static func malformedInvitation(_ text: String) -> Bool {
        var raw = text.trimmingCharacters(in: .whitespacesAndNewlines)
        if raw.hasPrefix("backplane://") || raw.hasPrefix(AppIdentity.scheme + "://") { raw = URLComponents(string: raw)?.queryItems?.first(where: { $0.name == "url" })?.value ?? "" }
        let fragment = URLComponents(string: raw)?.fragment ?? ""
        let fields = URLComponents(string: "?" + fragment)?.queryItems ?? []
        return fields.contains { $0.name == "invite" || $0.name == "code" }
    }
    /// A short code on its own: it names no hub, and must not be taken for an address.
    static func bareCode(_ text: String) -> Bool { text.trimmingCharacters(in: .whitespacesAndNewlines).range(of: "^[A-Z2-9]{4}[- ]?[A-Z2-9]{4}$", options: .regularExpression) != nil }
    static func codeLink(address: String, code: String) -> String? {
        let code = code.replacingOccurrences(of: " ", with: "").replacingOccurrences(of: "-", with: "").uppercased()
        guard code.range(of: "^[A-Z2-9]{8}$", options: .regularExpression) != nil,
              var c = URLComponents(string: address.contains("://") ? address : "http://" + address),
              ["http", "https"].contains(c.scheme), c.host != nil, c.user == nil, c.password == nil else { return nil }
        c.path = "/pair";c.query = nil;c.fragment = "code=" + code
        return c.string
    }
    static func message(_ error: String) -> String {
        switch error {
        case "wrong_code": "That code is not correct. Check it and try again."
        case "expired_code": "This invitation expired. Create another in Settings → Devices."
        case "used_code": "This invitation has already been used. Create a new one."
        case "revoked_code": "This invitation was revoked. Create a new one in Settings → Devices."
        case "rate_limited": "Too many attempts. Wait a minute and try again."
        case "storage_unavailable": "The hub could not save this pairing. Try again later."
        case "pairing_required": "Re-pair this device with a new invitation."
        default: "Hub unreachable. Check its address, LAN or tailnet connection."
        }
    }
    static func exchange(_ invite: Invitation, name: String) async throws -> String {
        var request = URLRequest(url: invite.origin.appendingPathComponent("auth/enroll"))
        request.httpMethod = "POST";request.timeoutInterval = 8
        request.setValue("Bearer " + invite.secret, forHTTPHeaderField: "Authorization")
        request.setValue("native", forHTTPHeaderField: "X-Backplane-Client")
        request.setValue(name.unicodeScalars.filter { $0.isASCII && $0.value >= 32 }.map(String.init).joined(), forHTTPHeaderField: "X-Backplane-Name")
        let config = URLSessionConfiguration.ephemeral
        config.httpShouldSetCookies = false;config.urlCache = nil
        let session = URLSession(configuration: config, delegate: NoPairRedirect(), delegateQueue: nil);defer { session.invalidateAndCancel() }
        let (data, response) = try await session.data(for: request)
        let json = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] ?? [:]
        guard (response as? HTTPURLResponse)?.statusCode == 200, let token = json["token"] as? String,
              token.range(of: "^[a-f0-9]{32}$", options: .regularExpression) != nil else {
            throw NSError(domain: "BackplanePairing", code: 1, userInfo: [NSLocalizedDescriptionKey: message(json["error"] as? String ?? "hub_unreachable")])
        }
        return invite.origin.absoluteString.trimmingCharacters(in: CharacterSet(charactersIn: "/")) + "/#token=" + token
    }
}
