import Foundation

// Restarting the hub from the phone (docs/PHONE-DATA.md, "The Hub screen"): plain HTTP to the paired address, so it works with the data socket down, the hub's views broken, or only its guard answering.
// HubRescue decides what each answer shows, a pure function held to test/mobile/hub_cases.json with Android's HubRescue.kt (scripts/mobile/hub-check.sh); HubHttp only carries the requests.
struct HubAction: Equatable, Identifiable { var id: String; var label: String; var text = "" }
struct HubIncident: Equatable { var atMs: Int64; var what: String; var why: String; var did: String }
struct HubNotice: Equatable { var text = ""; var tone = ""; var warn: Bool { tone == "warn" } }
struct HubStatus: Equatable { var hasGuard: Bool; var text: String; var actions: [HubAction]; var incidents: [HubIncident]; var notice: HubNotice? }
struct HubHealth: Equatable { var status: String; var safe: Bool }

enum HubFetch: Equatable { case loading, got(HubStatus), refused(String), unreachable }
enum HubReply: Equatable { case accepted(String), refused(String), credential(String), missing, unreachable }
struct HubPanel: Equatable { var text: String; var incidents: [HubIncident] = []; var actions: [HubAction] = [] }
struct HubAfter: Equatable { var text: String; var poll: Bool }
struct HubStep: Equatable { var text: String; var done: Bool; var moved: Bool }

enum HubRescue {
    static let pollMs: Int64 = 2_000
    static let limitMs: Int64 = 180_000
    // The old hub answers for a moment after its 202, and rescue the same. An ok or a down counts once the restart has shown (a gap, starting, hung) or after this long: a restart can fall between two polls.
    static let settleMs: Int64 = 10_000
    private static let restart = HubAction(id: "restart", label: "Restart hub")
    private static let pairing = "The hub refused this phone's pairing. Pair it again."
    private static func or(_ s: String, _ other: String) -> String { s.isEmpty ? other : s }

    static func panel(_ f: HubFetch) -> HubPanel {
        switch f {
        case .loading: HubPanel(text: "Asking the hub…")
        case .got(let s): s.hasGuard ? HubPanel(text: s.text, incidents: s.incidents, actions: s.actions)
            : HubPanel(text: "Restart is not available from here.", incidents: s.incidents, actions: [restart])
        case .refused(let t): HubPanel(text: or(t, pairing))
        case .unreachable: HubPanel(text: "The hub's status cannot be fetched.", actions: [restart])
        }
    }

    static func after(_ r: HubReply) -> HubAfter {
        switch r {
        case .accepted(let t): HubAfter(text: or(t, "Sent."), poll: true)
        case .refused(let t): HubAfter(text: or(t, "The hub refused."), poll: false)
        case .credential(let t): HubAfter(text: or(t, pairing), poll: false)
        case .missing: HubAfter(text: "This hub cannot do that yet (update it).", poll: false)
        case .unreachable: HubAfter(text: "The machine cannot be reached.", poll: false)
        }
    }

    /// One answer of /healthz (nil: none) after the request was accepted. `moved`: the restart has shown itself.
    static func step(_ h: HubHealth?, elapsedMs: Int64, moved: Bool) -> HubStep {
        let status = h?.status ?? ""
        let settled = status == "ok" || status == "degraded" || status == "down"
        let nowMoved = moved || !settled
        if settled, nowMoved || elapsedMs >= settleMs, let h { return HubStep(text: result(h), done: true, moved: nowMoved) }
        if elapsedMs >= limitMs { return HubStep(text: status.isEmpty || settled ? "No answer from the hub after 3 minutes." : "Hub still \(status) after 3 minutes.", done: true, moved: nowMoved) }
        return HubStep(text: status.isEmpty || settled ? "Waiting for the hub…" : "Hub is \(status)…", done: false, moved: nowMoved)
    }
    private static func result(_ h: HubHealth) -> String {
        switch h.status {
        case "down": "Hub is down."
        case "degraded": h.safe ? "Hub is up in safe mode, degraded." : "Hub is up, degraded."
        default: h.safe ? "Hub is up in safe mode." : "Hub is up."
        }
    }

    // MARK: the hub's answers
    private static func json(_ d: Data) -> [String: Any]? { (try? JSONSerialization.jsonObject(with: d)) as? [String: Any] }
    private static func str(_ o: [String: Any]?, _ k: String) -> String { o?[k] as? String ?? "" }

    static func notice(_ o: [String: Any]?) -> HubNotice { HubNotice(text: str(o, "text"), tone: str(o, "tone")) }
    static func fetch(code: Int, body: Data) -> HubFetch {
        let o = json(body)
        if code == 401 { return .refused(str(o, "text")) }
        guard (200..<300).contains(code), let o, o.keys.contains("guard") else { return .unreachable }
        let g = o["guard"] as? [String: Any]
        let actions = ((g?["actions"] as? [[String: Any]]) ?? []).map { a in HubAction(id: str(a, "id"), label: or(str(a, "label"), str(a, "id")), text: str(a, "text")) }
        // oldest first on the wire, `at` in seconds; newest first here
        let incidents = ((o["incidents"] as? [[String: Any]]) ?? []).map { i in
            HubIncident(atMs: ((i["at"] as? NSNumber)?.int64Value ?? 0) * 1000, what: str(i, "what"), why: str(i, "why"), did: str(i, "did"))
        }.reversed()
        return .got(HubStatus(hasGuard: g != nil, text: or(str(g, "text"), str(g, "state")), actions: actions, incidents: Array(incidents), notice: (o["notice"] as? [String: Any]).map(notice)))
    }
    static func reply(code: Int, body: Data) -> HubReply {
        let o = json(body), text = str(o, "text")
        if (200..<300).contains(code) { return o?["ok"] as? Bool == false ? .refused(text) : .accepted(text) }
        if code == 401 { return .credential(text) }
        if code == 409 || !text.isEmpty { return .refused(text) }
        return code == 404 ? .missing : .unreachable
    }
    // a 503 with a body is an answer like a 200: only the body counts
    static func health(_ body: Data) -> HubHealth? {
        guard let o = json(body) else { return nil }
        let s = str(o, "status")
        return s.isEmpty ? nil : HubHealth(status: s, safe: o["safe"] as? Bool ?? false)
    }
}

// the credential goes to the paired address and nowhere a redirect points
private final class HubNoRedirect: NSObject, URLSessionTaskDelegate {
    func urlSession(_ s: URLSession, task: URLSessionTask, willPerformHTTPRedirection r: HTTPURLResponse, newRequest: URLRequest) async -> URLRequest? { nil }
}

final class HubHttp {
    private let base: String
    private let token: String?
    private let healthTimeout: TimeInterval
    private let callTimeout: TimeInterval
    private let session: URLSession

    // a session of its own: nothing cached, and a hung hub does not hold a request for URLSession's minute
    init(base: String, token: String?, healthTimeout: TimeInterval = 5, callTimeout: TimeInterval = 12) {
        self.base = base; self.token = token; self.healthTimeout = healthTimeout; self.callTimeout = callTimeout
        let c = URLSessionConfiguration.ephemeral
        c.urlCache = nil; c.requestCachePolicy = .reloadIgnoringLocalCacheData; c.httpShouldSetCookies = false; c.timeoutIntervalForResource = callTimeout
        session = URLSession(configuration: c, delegate: HubNoRedirect(), delegateQueue: nil)
    }
    deinit { session.invalidateAndCancel() }

    private func request(_ path: String, credential: Bool = true, timeout: TimeInterval) -> URLRequest? {
        guard let u = URL(string: base + path) else { return nil }
        var r = URLRequest(url: u, timeoutInterval: timeout)
        if credential, let token, !token.isEmpty { r.setValue("Bearer " + token, forHTTPHeaderField: "Authorization") }
        return r
    }
    private func send(_ r: URLRequest?) async -> (Int, Data) {
        guard let r, let (d, response) = try? await session.data(for: r) else { return (0, Data()) }
        return ((response as? HTTPURLResponse)?.statusCode ?? 0, d)
    }
    func status() async -> HubFetch { let (c, d) = await send(request("/hub/status", timeout: callTimeout)); return HubRescue.fetch(code: c, body: d) }
    func control(_ action: String) async -> HubReply {
        var r = request("/hub/control", timeout: callTimeout)
        r?.httpMethod = "POST"
        r?.setValue("application/json", forHTTPHeaderField: "Content-Type")
        r?.httpBody = try? JSONSerialization.data(withJSONObject: ["action": action])
        let (c, d) = await send(r)
        return HubRescue.reply(code: c, body: d)
    }
    func health() async -> HubHealth? { HubRescue.health(await send(request("/healthz", credential: false, timeout: healthTimeout)).1) }
}
