import Foundation

func check(_ condition: @autoclosure () -> Bool, _ label: String) {
    guard condition() else { fputs("FAIL: " + label + "\n", stderr); exit(1) }
    print("PASS " + label)
}

@main struct ConnectionTests {
    @MainActor static func main() async {
        let base = CommandLine.arguments[1]
        func find(_ token: String) async -> ViewCapability.Found {
            await ViewCapability.find(base + "/#token=" + token)
        }
        check(Pairing.socket("wss://hub.invalid:443/ws?token=abcdef")?.scheme == "wss", "secure socket address retains TLS")
        check(Pairing.http("wss://hub.invalid:443/ws?token=abcdef", path: "/client/capabilities", token: false)?.scheme == "https", "secure socket discovery uses HTTPS")
        let nested = "backplane://pair?url=" + ("https://hub.invalid:443/#token=abcdef".addingPercentEncoding(withAllowedCharacters: .alphanumerics)!)
        check(Pairing.viewSocket(nested) == Pairing.viewSocket("https://hub.invalid:443/#token=abcdef"), "QR wrapper and pasted link select the same socket")
        check(Pairing.socket("ftp://hub.invalid/#token=abcdef") == nil, "unsupported address scheme is rejected")
        let misleading = Pairing.socket("https://hub.invalid/#not_token=abcdef")!
        check(URLComponents(url: misleading, resolvingAgainstBaseURL: false)?.queryItems == nil, "a substring credential cannot replace the exact token field")
        let delayed = await find("abcdef")
        check(delayed == .supported, "capability reply after three seconds is accepted")
        let denied = await find("1111")
        check(denied == .retry(.authentication), "401 is an authentication failure, never an update requirement")
        let forbidden = await find("2222")
        check(forbidden == .retry(.authentication), "403 remains an authentication failure")
        let unavailable = await find("3333")
        check(unavailable == .retry(.http(503)), "starting or unavailable server remains retryable with its status")
        let redirect = await find("4444")
        check(redirect == .retry(.http(302)), "discovery never follows redirects with the credential")
        let legacy = await find("5555")
        check(legacy == .unsupported, "404 still identifies a hub without the capability endpoint")
        let oversized = await find("6666")
        check(oversized == .retry(.invalidResponse), "oversized response cannot masquerade as an old hub")
        let timedout = await find("9999")
        check(timedout == .retry(.timeout), "unresponsive hub has a bounded attempt and a timeout cause")
        let errors: [(Int, ConnectionFailure)] = [(NSURLErrorTimedOut, .timeout), (NSURLErrorCannotFindHost, .dns),
            (NSURLErrorNotConnectedToInternet, .network), (NSURLErrorServerCertificateUntrusted, .tls)]
        for (code, expected) in errors {
            let secretError = NSError(domain: NSURLErrorDomain, code: code, userInfo: [NSURLErrorFailingURLErrorKey:URL(string:"https://hub.invalid/?token=PRIVATE")!, NSLocalizedDescriptionKey:"PRIVATE"])
            let failure = ConnectionFailure.classify(secretError)
            check(failure == expected && !failure.reason.contains("PRIVATE") && !failure.code.contains("PRIVATE"), "Foundation error \(code) retains only its safe cause")
        }

        // A discovery parked on an unsupported hub must be retried after a
        // foreground/network wake: the same host may have been upgraded.
        var parkedAttempts = 0
        let parked = Hub(url: { parkedAttempts += 1; return .stop }, onConnecting: {}, onOpen: {}, onMessage: { _ in }, onClose: { _ in })
        parked.start()
        parked.start()
        try? await Task.sleep(for: .milliseconds(100))
        check(parkedAttempts == 1, "starting an active hub twice does not duplicate discovery")
        parked.wake()
        try? await Task.sleep(for: .milliseconds(100))
        check(parkedAttempts == 2, "wake rechecks a parked hub after an upgrade")
        parked.stop()
        parked.wake()
        try? await Task.sleep(for: .milliseconds(100))
        check(parkedAttempts == 2, "wake cannot revive a stopped hub")

        // A wake replaces an old in-flight discovery. Its late answer cannot
        // open a socket or report a failure in the new generation.
        var pending: CheckedContinuation<Hub.Next, Never>?
        var generations = 0, staleFailures = 0
        let fenced = Hub(url: {
            generations += 1
            if generations == 1 { return await withCheckedContinuation { pending = $0 } }
            return .stop
        }, onConnecting: {}, onOpen: {}, onMessage: { _ in }, onClose: { _ in staleFailures += 1 })
        fenced.start()
        try? await Task.sleep(for: .milliseconds(100))
        fenced.wake(quiet: -1)
        try? await Task.sleep(for: .milliseconds(100))
        pending?.resume(returning: .later(.network))
        try? await Task.sleep(for: .milliseconds(100))
        check(generations == 2 && staleFailures == 0, "wake fences a retired discovery")
        fenced.stop()

        // The actual Hub retries failed discovery, reads a real WebSocket frame,
        // and stops cleanly. No bridge, stored account, or user app participates.
        var attempts = 0, opens = 0, frames = 0
        var openedAt: [Date] = []
        var failures: [ConnectionFailure] = []
        let socket = URL(string: base.replacingOccurrences(of: "http://", with: "ws://") + "/ws")!
        let hub = Hub(url: {
            attempts += 1
            return attempts == 1 ? .later(.network) : .go(socket)
        }, onConnecting: {}, onOpen: { opens += 1; openedAt.append(Date()) }, onMessage: { _ in frames += 1 }, onClose: { failures.append($0) })
        hub.start()
        for _ in 0..<60 {
            if frames > 0 { break }
            try? await Task.sleep(for: .milliseconds(100))
        }
        check(attempts == 2 && opens == 1 && frames == 1 && failures == [.network], "native Hub recovers from failed discovery through a real socket")
        hub.reconnect(.view)
        hub.reconnect(.view) // duplicate failure while the retry is pending
        for _ in 0..<60 {
            if frames == 2 { break }
            try? await Task.sleep(for: .milliseconds(100))
        }
        check(attempts == 3 && opens == 2 && frames == 2 && failures == [.network, .view], "failed presentation replaces a live socket exactly once through backoff")
        hub.reconnect(.view)
        for _ in 0..<60 {
            if frames == 3 { break }
            try? await Task.sleep(for: .milliseconds(100))
        }
        check(attempts == 4 && openedAt[2].timeIntervalSince(openedAt[1]) >= 0.5, "greetings alone cannot reset presentation failure backoff")
        hub.ready()
        hub.reconnect(.view)
        hub.wake() // the foreground/network wake bypasses the pending backoff
        for _ in 0..<60 {
            if frames == 4 { break }
            try? await Task.sleep(for: .milliseconds(100))
        }
        check(attempts == 5 && opens == 4 && frames == 4, "foreground wake resumes a failed presentation without re-pairing")
        hub.reconnect(.view)
        hub.stop()
        let stoppedAttempts = attempts
        try? await Task.sleep(for: .milliseconds(400))
        check(attempts == stoppedAttempts, "stopped hub cannot reconnect from a stale retry")
    }
}
